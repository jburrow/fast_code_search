//! Lazy file store with on-demand memory mapping
//!
//! This module provides a file store that registers file paths at startup
//! but only memory-maps them when they are first accessed. This dramatically
//! reduces startup time when loading from a persisted index.

use anyhow::{Context, Result};
use memmap2::Mmap;
use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use tracing::warn;

/// Represents a lazily memory-mapped file
///
/// The file path is stored immediately, but the memory mapping is created
/// on first access. This allows registering thousands of files instantly
/// while only paying the I/O cost for files that are actually searched.
pub struct LazyMappedFile {
    /// The file path (always available)
    pub path: PathBuf,
    /// Lazily initialized memory map
    mmap: OnceLock<Result<Mmap, String>>,
    /// Cached result of UTF-8 validation
    utf8_valid: OnceLock<bool>,
}

impl LazyMappedFile {
    /// Create a new lazy file entry (does NOT open or map the file)
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            mmap: OnceLock::new(),
            utf8_valid: OnceLock::new(),
        }
    }

    /// Create a new lazy file entry with an already-mapped file (for immediate indexing)
    pub fn with_mmap(path: impl AsRef<Path>, mmap: Mmap) -> Self {
        let file = Self {
            path: path.as_ref().to_path_buf(),
            mmap: OnceLock::new(),
            utf8_valid: OnceLock::new(),
        };
        let _ = file.mmap.set(Ok(mmap));
        file
    }

    /// Ensure the file is memory-mapped, mapping it if necessary
    fn ensure_mapped(&self) -> Result<&Mmap> {
        let result = self.mmap.get_or_init(|| {
            match File::open(&self.path) {
                Ok(file) => {
                    // SAFETY: We're memory-mapping a file we just opened
                    match unsafe { Mmap::map(&file) } {
                        Ok(mmap) => Ok(mmap),
                        Err(e) => Err(format!("Failed to mmap {}: {}", self.path.display(), e)),
                    }
                }
                Err(e) => Err(format!("Failed to open {}: {}", self.path.display(), e)),
            }
        });

        match result {
            Ok(mmap) => Ok(mmap),
            Err(e) => anyhow::bail!("{}", e),
        }
    }

    /// Check if the file has been mapped yet
    pub fn is_mapped(&self) -> bool {
        self.mmap.get().is_some()
    }

    /// Get the content as a string slice (if valid UTF-8)
    /// This will trigger memory mapping if not already done.
    pub fn as_str(&self) -> Result<&str> {
        let mmap = self.ensure_mapped()?;

        let is_valid = *self
            .utf8_valid
            .get_or_init(|| std::str::from_utf8(mmap).is_ok());

        if is_valid {
            // SAFETY: We validated UTF-8 above and cached the result
            Ok(unsafe { std::str::from_utf8_unchecked(mmap) })
        } else {
            anyhow::bail!("File is not valid UTF-8: {}", self.path.display())
        }
    }

    /// Get the content as bytes
    /// This will trigger memory mapping if not already done.
    pub fn as_bytes(&self) -> Result<&[u8]> {
        let mmap = self.ensure_mapped()?;
        Ok(&mmap[..])
    }

    /// Get the file size
    /// This will trigger memory mapping if not already done.
    pub fn len(&self) -> Result<usize> {
        let mmap = self.ensure_mapped()?;
        Ok(mmap.len())
    }

    /// Get the file size if already mapped, without triggering a map
    pub fn len_if_mapped(&self) -> Option<usize> {
        self.mmap
            .get()
            .and_then(|r| r.as_ref().ok())
            .map(|m| m.len())
    }

    /// Check if the file is empty
    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }
}

/// Store for lazily memory-mapped files
///
/// Files are registered by path at startup, but only memory-mapped when
/// first accessed. This provides near-instant startup even with millions of files.
pub struct LazyFileStore {
    /// Files indexed by ID
    files: Vec<LazyMappedFile>,
    /// Map from path to file ID (for deduplication)
    path_to_id: HashMap<PathBuf, u32>,
    /// Statistics: number of files that have been mapped
    mapped_count: RwLock<usize>,
}

impl LazyFileStore {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            path_to_id: HashMap::new(),
            mapped_count: RwLock::new(0),
        }
    }

    /// Register a file path and return its ID (does NOT map the file)
    ///
    /// This is extremely fast as it only stores the path.
    /// The file will be memory-mapped on first access via `get()`.
    pub fn register_file(&mut self, path: impl AsRef<Path>) -> u32 {
        let path = path.as_ref();
        let path_buf = path.to_path_buf();

        // Check if already registered
        if let Some(&existing_id) = self.path_to_id.get(&path_buf) {
            return existing_id;
        }

        let id = self.files.len() as u32;
        self.path_to_id.insert(path_buf.clone(), id);
        self.files.push(LazyMappedFile::new(path_buf));
        id
    }

    /// Register multiple file paths in bulk (does NOT map any files)
    ///
    /// Returns a vector of file IDs in the same order as the input paths.
    pub fn register_files_bulk(&mut self, paths: &[PathBuf]) -> Vec<u32> {
        paths.iter().map(|p| self.register_file(p)).collect()
    }

    /// Add a file with immediate mapping (for fresh indexing)
    ///
    /// This is used during initial indexing when we need to read the file
    /// content immediately for trigram extraction.
    pub fn add_file(&mut self, path: impl AsRef<Path>) -> Result<u32> {
        let path = path.as_ref();

        // Canonicalize path to handle symlinks
        let canonical = match path.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                warn!(
                    "Failed to canonicalize path '{}': {}. Using original path.",
                    path.display(),
                    e
                );
                path.to_path_buf()
            }
        };

        // Check if already indexed
        if let Some(&existing_id) = self.path_to_id.get(&canonical) {
            return Ok(existing_id);
        }

        // Open and map the file immediately
        let file =
            File::open(path).with_context(|| format!("Failed to open file: {}", path.display()))?;
        let mmap = unsafe {
            Mmap::map(&file).with_context(|| format!("Failed to mmap file: {}", path.display()))?
        };

        let id = self.files.len() as u32;
        self.path_to_id.insert(canonical, id);
        self.files.push(LazyMappedFile::with_mmap(path, mmap));

        // Update mapped count
        if let Ok(mut count) = self.mapped_count.write() {
            *count += 1;
        }

        Ok(id)
    }

    /// Get a file by ID, triggering lazy mapping if needed
    pub fn get(&self, id: u32) -> Option<&LazyMappedFile> {
        let file = self.files.get(id as usize)?;

        // Track if we're about to map a new file
        let was_mapped = file.is_mapped();

        // Access triggers mapping (we don't call ensure_mapped here,
        // the caller will do that when they call as_str() or as_bytes())

        if !was_mapped && file.is_mapped() {
            if let Ok(mut count) = self.mapped_count.write() {
                *count += 1;
            }
        }

        Some(file)
    }

    /// Get the total number of registered files
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Get total size of all MAPPED files (not all registered files)
    pub fn total_mapped_size(&self) -> u64 {
        self.files
            .iter()
            .filter_map(|f| f.len_if_mapped())
            .map(|len| len as u64)
            .sum()
    }

    /// Get a file path by ID (always available, no I/O needed)
    pub fn get_path(&self, id: u32) -> Option<&Path> {
        self.files.get(id as usize).map(|f| f.path.as_path())
    }

    /// Get all file paths (no I/O needed)
    pub fn get_all_paths(&self) -> Vec<PathBuf> {
        self.files.iter().map(|f| f.path.clone()).collect()
    }

    /// Get the number of files that have been actually mapped
    pub fn mapped_count(&self) -> usize {
        self.mapped_count.read().map(|guard| *guard).unwrap_or(0)
    }

    /// Pre-reserve capacity for a known number of files
    pub fn reserve(&mut self, additional: usize) {
        self.files.reserve(additional);
        self.path_to_id.reserve(additional);
    }
}

impl Default for LazyFileStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_lazy_mapped_file_new() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "hello world").expect("Failed to write test file");

        let lazy = LazyMappedFile::new(&file_path);

        // Should NOT be mapped yet
        assert!(!lazy.is_mapped());
        assert!(lazy.len_if_mapped().is_none());

        // Access triggers mapping
        assert_eq!(lazy.as_str().unwrap(), "hello world");
        assert!(lazy.is_mapped());
        assert_eq!(lazy.len_if_mapped(), Some(11));
    }

    #[test]
    fn test_lazy_file_store_register() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");
        std::fs::write(&file1, "hello").expect("Failed to write file1");
        std::fs::write(&file2, "world").expect("Failed to write file2");

        let mut store = LazyFileStore::new();

        // Register files (should be instant, no I/O)
        let id1 = store.register_file(&file1);
        let id2 = store.register_file(&file2);

        assert_eq!(id1, 0);
        assert_eq!(id2, 1);
        assert_eq!(store.len(), 2);
        assert_eq!(store.mapped_count(), 0); // Nothing mapped yet!

        // Access one file
        let f1 = store.get(id1).unwrap();
        assert_eq!(f1.as_str().unwrap(), "hello");

        // Now one file is mapped
        assert!(store.get(id1).unwrap().is_mapped());
        assert!(!store.get(id2).unwrap().is_mapped());
    }

    #[test]
    fn test_lazy_file_store_bulk_register() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let paths: Vec<PathBuf> = (0..100)
            .map(|i| {
                let path = temp_dir.path().join(format!("file{}.txt", i));
                std::fs::write(&path, format!("content {}", i)).unwrap();
                path
            })
            .collect();

        let mut store = LazyFileStore::new();
        let ids = store.register_files_bulk(&paths);

        assert_eq!(ids.len(), 100);
        assert_eq!(store.len(), 100);
        assert_eq!(store.mapped_count(), 0); // Nothing mapped!

        // Access just one file
        let content = store.get(50).unwrap().as_str().unwrap();
        assert_eq!(content, "content 50");
    }

    #[test]
    fn test_lazy_file_store_duplicate_handling() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "hello").expect("Failed to write test file");

        let mut store = LazyFileStore::new();
        let id1 = store.register_file(&file_path);
        let id2 = store.register_file(&file_path);

        assert_eq!(id1, id2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_lazy_file_store_get_path() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "content").expect("Failed to write test file");

        let mut store = LazyFileStore::new();
        let id = store.register_file(&file_path);

        // get_path works without mapping
        let path = store.get_path(id).expect("Failed to get path");
        assert!(path.ends_with("test.txt"));
        assert_eq!(store.mapped_count(), 0); // Still not mapped!
    }

    #[test]
    fn test_lazy_file_nonexistent() {
        let lazy = LazyMappedFile::new("/nonexistent/path/to/file.txt");
        assert!(!lazy.is_mapped());

        // Trying to access will fail
        assert!(lazy.as_str().is_err());

        // But it's now "mapped" (with an error cached)
        assert!(lazy.is_mapped());
    }

    #[test]
    fn test_lazy_file_invalid_utf8() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("binary.bin");

        let mut file = std::fs::File::create(&file_path).expect("Failed to create file");
        file.write_all(&[0xFF, 0xFE, 0x00, 0x01]).unwrap();
        drop(file);

        let lazy = LazyMappedFile::new(&file_path);
        assert!(lazy.as_str().is_err());

        // as_bytes should work
        assert_eq!(lazy.as_bytes().unwrap(), &[0xFF, 0xFE, 0x00, 0x01]);
    }
}
