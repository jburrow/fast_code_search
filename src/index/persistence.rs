//! Persistent index storage
//!
//! Provides save/load functionality for the trigram index to speed up restarts.

use anyhow::{Context, Result};
use roaring::RoaringBitmap;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::trigram::Trigram;

/// Serializable representation of the trigram index
#[derive(Serialize, Deserialize)]
pub struct PersistedTrigramIndex {
    /// Map from trigram bytes to serialized roaring bitmap
    trigram_to_docs: HashMap<[u8; 3], Vec<u8>>,
}

/// Serializable representation of file metadata
#[derive(Serialize, Deserialize)]
pub struct PersistedFileMetadata {
    /// Original file path
    pub path: PathBuf,
    /// File modification time (for staleness check)
    pub mtime: u64,
    /// File size
    pub size: u64,
}

/// Complete persisted index state
#[derive(Serialize, Deserialize)]
pub struct PersistedIndex {
    /// Version for forward compatibility
    pub version: u32,
    /// File metadata for staleness detection
    pub files: Vec<PersistedFileMetadata>,
    /// Trigram index data
    pub trigram_index: PersistedTrigramIndex,
}

impl PersistedIndex {
    /// Current persistence format version
    pub const CURRENT_VERSION: u32 = 1;

    /// Create a new persisted index from the current state
    pub fn new(
        files: Vec<PersistedFileMetadata>,
        trigram_to_docs: &FxHashMap<Trigram, RoaringBitmap>,
    ) -> Result<Self> {
        let mut serialized_trigrams = HashMap::new();

        for (trigram, bitmap) in trigram_to_docs {
            let mut buf = Vec::new();
            bitmap.serialize_into(&mut buf)?;
            serialized_trigrams.insert(trigram.as_bytes(), buf);
        }

        Ok(Self {
            version: Self::CURRENT_VERSION,
            files,
            trigram_index: PersistedTrigramIndex {
                trigram_to_docs: serialized_trigrams,
            },
        })
    }

    /// Save the index to a file
    pub fn save(&self, path: &Path) -> Result<()> {
        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create index directory: {}", parent.display())
            })?;
        }

        let file = std::fs::File::create(path)
            .with_context(|| format!("Failed to create index file: {}", path.display()))?;
        let writer = std::io::BufWriter::new(file);

        bincode::serialize_into(writer, self)
            .with_context(|| format!("Failed to serialize index: {}", path.display()))?;

        Ok(())
    }

    /// Load an index from a file
    pub fn load(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)
            .with_context(|| format!("Failed to open index file: {}", path.display()))?;
        let reader = std::io::BufReader::new(file);

        let index: Self = bincode::deserialize_from(reader)
            .with_context(|| format!("Failed to deserialize index: {}", path.display()))?;

        if index.version != Self::CURRENT_VERSION {
            anyhow::bail!(
                "Index version mismatch: found {}, expected {}",
                index.version,
                Self::CURRENT_VERSION
            );
        }

        Ok(index)
    }

    /// Restore the trigram index from persisted data
    pub fn restore_trigram_index(&self) -> Result<FxHashMap<Trigram, RoaringBitmap>> {
        let mut result = FxHashMap::default();

        for (trigram_bytes, bitmap_data) in &self.trigram_index.trigram_to_docs {
            let trigram = Trigram::new(*trigram_bytes);
            let bitmap = RoaringBitmap::deserialize_from(&bitmap_data[..])?;
            result.insert(trigram, bitmap);
        }

        Ok(result)
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
        }];

        let persisted =
            PersistedIndex::new(files, &trigram_to_docs).expect("Failed to create persisted index");

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
