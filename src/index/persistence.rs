//! Persistent index storage
//!
//! Provides save/load functionality for the trigram index to speed up restarts.
//! Includes file locking for safe concurrent access (exclusive writes, shared reads).

use anyhow::{Context, Result};
use fs2::FileExt;
use roaring::RoaringBitmap;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::trigram::Trigram;
use crate::symbols::extractor::Symbol;
use crate::utils::normalize_path_for_comparison;

/// Serializable representation of the trigram index
#[derive(Serialize, Deserialize)]
pub struct PersistedTrigramIndex {
    /// Map from trigram bytes to serialized roaring bitmap
    trigram_to_docs: HashMap<[u8; 3], Vec<u8>>,
}

impl PersistedTrigramIndex {
    /// Get the number of trigrams in the index (for benchmarking)
    pub fn len(&self) -> usize {
        self.trigram_to_docs.len()
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.trigram_to_docs.is_empty()
    }
}

/// Serializable representation of file metadata
#[derive(Serialize, Deserialize, Clone)]
pub struct PersistedFileMetadata {
    /// Original file path
    pub path: PathBuf,
    /// File modification time (for staleness check)
    pub mtime: u64,
    /// File size
    pub size: u64,
    /// The base path from config that this file belongs to
    #[serde(default)]
    pub source_base_path: Option<String>,
}

/// Complete persisted index state
#[derive(Serialize, Deserialize)]
pub struct PersistedIndex {
    /// Version for forward compatibility
    pub version: u32,
    /// Configuration fingerprint for detecting config changes
    #[serde(default)]
    pub config_fingerprint: String,
    /// The indexed base paths from config (for reconciliation)
    #[serde(default)]
    pub indexed_paths: Vec<String>,
    /// File metadata for staleness detection
    pub files: Vec<PersistedFileMetadata>,
    /// Trigram index data
    pub trigram_index: PersistedTrigramIndex,
    /// Per-file symbol caches (parallel to `files`, indexed by position)
    #[serde(default)]
    pub symbols: Vec<Vec<Symbol>>,
    /// Resolved dependency edges as (from_file_idx, to_file_idx) pairs
    /// where indices are positions in the `files` Vec
    #[serde(default)]
    pub dependency_edges: Vec<(u32, u32)>,
}

impl PersistedIndex {
    /// Current persistence format version (bump this when format changes)
    pub const CURRENT_VERSION: u32 = 3;

    /// Create a new persisted index from the current state
    pub fn new(
        config_fingerprint: String,
        indexed_paths: Vec<String>,
        files: Vec<PersistedFileMetadata>,
        trigram_to_docs: &FxHashMap<Trigram, RoaringBitmap>,
        symbols: Vec<Vec<Symbol>>,
        dependency_edges: Vec<(u32, u32)>,
    ) -> Result<Self> {
        let mut serialized_trigrams = HashMap::new();

        for (trigram, bitmap) in trigram_to_docs {
            let mut buf = Vec::new();
            bitmap.serialize_into(&mut buf)?;
            serialized_trigrams.insert(trigram.as_bytes(), buf);
        }

        Ok(Self {
            version: Self::CURRENT_VERSION,
            config_fingerprint,
            indexed_paths,
            files,
            trigram_index: PersistedTrigramIndex {
                trigram_to_docs: serialized_trigrams,
            },
            symbols,
            dependency_edges,
        })
    }

    /// Save the index to a file with exclusive lock
    pub fn save(&self, path: &Path) -> Result<()> {
        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create index directory: {}", parent.display())
            })?;
        }

        let file = std::fs::File::create(path)
            .with_context(|| format!("Failed to create index file: {}", path.display()))?;

        // Acquire exclusive lock for writing
        file.lock_exclusive()
            .with_context(|| format!("Failed to acquire exclusive lock on: {}", path.display()))?;

        let writer = std::io::BufWriter::new(&file);
        bincode::serialize_into(writer, self)
            .with_context(|| format!("Failed to serialize index: {}", path.display()))?;

        // Lock is automatically released when file is dropped
        Ok(())
    }

    /// Load an index from a file with shared lock (allows multiple readers)
    pub fn load(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)
            .with_context(|| format!("Failed to open index file: {}", path.display()))?;

        // Acquire shared lock for reading (multiple readers allowed)
        file.lock_shared()
            .with_context(|| format!("Failed to acquire shared lock on: {}", path.display()))?;

        let reader = std::io::BufReader::new(&file);
        let index: Self = bincode::deserialize_from(reader)
            .with_context(|| format!("Failed to deserialize index: {}", path.display()))?;

        // Lock is automatically released when file is dropped

        if index.version != Self::CURRENT_VERSION {
            anyhow::bail!(
                "Index version mismatch: found {}, expected {}. The index will be rebuilt.",
                index.version,
                Self::CURRENT_VERSION
            );
        }

        Ok(index)
    }

    /// Try to load an index, returning None on any error (graceful degradation)
    pub fn try_load(path: &Path) -> Option<Self> {
        match Self::load(path) {
            Ok(index) => Some(index),
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "Failed to load persisted index, will rebuild"
                );
                None
            }
        }
    }

    /// Check if the config has changed since the index was built
    pub fn is_config_compatible(&self, current_fingerprint: &str) -> bool {
        self.config_fingerprint == current_fingerprint
    }

    /// Get paths that were in the old config but not in the new config (need removal)
    pub fn paths_to_remove(&self, current_paths: &[String]) -> Vec<String> {
        let current_set: std::collections::HashSet<_> = current_paths
            .iter()
            .map(|p| normalize_path_for_comparison(p))
            .collect();

        self.indexed_paths
            .iter()
            .filter(|p| {
                let normalized = normalize_path_for_comparison(p);
                !current_set.contains(&normalized)
            })
            .cloned()
            .collect()
    }

    /// Get paths that are in the new config but weren't in the old config (need indexing)
    pub fn paths_to_add(&self, current_paths: &[String]) -> Vec<String> {
        let indexed_set: std::collections::HashSet<_> = self
            .indexed_paths
            .iter()
            .map(|p| normalize_path_for_comparison(p))
            .collect();

        current_paths
            .iter()
            .filter(|p| {
                let normalized = normalize_path_for_comparison(p);
                !indexed_set.contains(&normalized)
            })
            .cloned()
            .collect()
    }

    /// Restore the trigram index from persisted data (parallelized for performance)
    pub fn restore_trigram_index(&self) -> Result<FxHashMap<Trigram, RoaringBitmap>> {
        use rayon::prelude::*;

        // Parallel deserialization of trigrams
        let results: Result<Vec<_>> = self
            .trigram_index
            .trigram_to_docs
            .par_iter()
            .map(|(trigram_bytes, bitmap_data)| {
                let trigram = Trigram::new(*trigram_bytes);
                let bitmap = RoaringBitmap::deserialize_from(&bitmap_data[..])?;
                Ok((trigram, bitmap))
            })
            .collect();

        // Collect into FxHashMap
        Ok(results?.into_iter().collect())
    }
}

/// Get the modification time of a file in seconds since UNIX epoch
pub fn get_mtime(path: &Path) -> Result<u64> {
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("Failed to get metadata for: {}", path.display()))?;
    let mtime = metadata
        .modified()
        .with_context(|| format!("Failed to get mtime for: {}", path.display()))?;
    Ok(mtime
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0))
}

/// Check if a file is stale (modified since indexing)
pub fn is_file_stale(path: &Path, stored_mtime: u64, stored_size: u64) -> bool {
    match std::fs::metadata(path) {
        Ok(metadata) => {
            let current_mtime = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let current_size = metadata.len();

            current_mtime != stored_mtime || current_size != stored_size
        }
        Err(_) => true, // File doesn't exist or can't be read
    }
}

/// File classification result after checking staleness
#[derive(Debug)]
pub enum FileStatus {
    Valid,
    Stale,
    Removed,
}

/// Batch check file staleness in parallel for better performance
pub fn batch_check_files(
    files: &[PersistedFileMetadata],
    removed_paths: &[String],
) -> Vec<(usize, FileStatus)> {
    use rayon::prelude::*;

    // Normalize removed paths once for comparison
    let removed_normalized: Vec<String> = removed_paths
        .iter()
        .map(|p| p.replace('\\', "/").to_lowercase())
        .collect();

    files
        .par_iter()
        .enumerate()
        .map(|(idx, file_meta)| {
            // Check if file is from a removed path
            if let Some(ref base) = file_meta.source_base_path {
                let base_normalized = base.replace('\\', "/").to_lowercase();
                if removed_normalized.contains(&base_normalized) {
                    return (idx, FileStatus::Removed);
                }
            }

            // Check if file exists and is stale
            if !file_meta.path.exists() {
                (idx, FileStatus::Removed)
            } else if is_file_stale(&file_meta.path, file_meta.mtime, file_meta.size) {
                (idx, FileStatus::Stale)
            } else {
                (idx, FileStatus::Valid)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_persisted_index_save_and_load() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let index_path = temp_dir.path().join("test_index.bin");

        // Create a simple index
        let mut trigram_to_docs: FxHashMap<Trigram, RoaringBitmap> = FxHashMap::default();
        let mut bitmap = RoaringBitmap::new();
        bitmap.insert(0);
        bitmap.insert(1);
        trigram_to_docs.insert(Trigram::new([b'h', b'e', b'l']), bitmap);

        let files = vec![PersistedFileMetadata {
            path: PathBuf::from("/test/file.rs"),
            mtime: 12345,
            size: 100,
            source_base_path: Some("/test".to_string()),
        }];

        let persisted = PersistedIndex::new(
            "test_fingerprint".to_string(),
            vec!["/test".to_string()],
            files,
            &trigram_to_docs,
            Vec::new(),
            Vec::new(),
        )
        .expect("Failed to create persisted index");

        // Save
        persisted.save(&index_path).expect("Failed to save index");

        // Load
        let loaded = PersistedIndex::load(&index_path).expect("Failed to load index");

        assert_eq!(loaded.version, PersistedIndex::CURRENT_VERSION);
        assert_eq!(loaded.files.len(), 1);
        assert_eq!(loaded.files[0].path, PathBuf::from("/test/file.rs"));

        // Restore trigram index
        let restored = loaded
            .restore_trigram_index()
            .expect("Failed to restore trigram index");
        let bitmap = restored
            .get(&Trigram::new([b'h', b'e', b'l']))
            .expect("Trigram not found");
        assert!(bitmap.contains(0));
        assert!(bitmap.contains(1));
    }

    #[test]
    fn test_is_file_stale() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "hello").expect("Failed to write file");

        let mtime = get_mtime(&file_path).expect("Failed to get mtime");
        let size = std::fs::metadata(&file_path)
            .expect("Failed to get metadata")
            .len();

        // File should not be stale
        assert!(!is_file_stale(&file_path, mtime, size));

        // Different size should be stale
        assert!(is_file_stale(&file_path, mtime, size + 1));

        // Non-existent file should be stale
        assert!(is_file_stale(
            &temp_dir.path().join("nonexistent.txt"),
            0,
            0
        ));
    }
}
