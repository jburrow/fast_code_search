use anyhow::{Context, Result};
use memmap2::Mmap;
use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Represents a memory-mapped file
pub struct MappedFile {
    pub path: PathBuf,
    pub mmap: Mmap,
    /// Cached result of UTF-8 validation (validated once, reused on subsequent calls)
    utf8_valid: OnceLock<bool>,
}

impl MappedFile {
    /// Create a new memory-mapped file
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let file =
            File::open(path).with_context(|| format!("Failed to open file: {}", path.display()))?;

        let mmap = unsafe {
            Mmap::map(&file).with_context(|| format!("Failed to mmap file: {}", path.display()))?
        };

        Ok(Self {
            path: path.to_path_buf(),
            mmap,
            utf8_valid: OnceLock::new(),
        })
    }

    /// Get the content as a string slice (if valid UTF-8)
    /// UTF-8 validation is cached after the first call for performance.
    pub fn as_str(&self) -> Result<&str> {
        let is_valid = *self
            .utf8_valid
            .get_or_init(|| std::str::from_utf8(&self.mmap).is_ok());

        if is_valid {
            // SAFETY: We validated UTF-8 above and cached the result
            Ok(unsafe { std::str::from_utf8_unchecked(&self.mmap) })
        } else {
            anyhow::bail!("File is not valid UTF-8: {}", self.path.display())
        }
    }

    /// Get the content as bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.mmap
    }

    /// Get the file size
    pub fn len(&self) -> usize {
        self.mmap.len()
    }

    /// Check if the file is empty
    pub fn is_empty(&self) -> bool {
        self.mmap.is_empty()
    }
}

/// Store for all memory-mapped files
pub struct FileStore {
    files: Vec<MappedFile>,
    /// Map from canonicalized path to file ID to prevent duplicate indexing
    path_to_id: HashMap<PathBuf, u32>,
}

impl FileStore {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            path_to_id: HashMap::new(),
        }
    }

    /// Add a file to the store and return its ID.
    /// If the file was already added (by canonical path), returns the existing ID.
    pub fn add_file(&mut self, path: impl AsRef<Path>) -> Result<u32> {
        let path = path.as_ref();

        // Canonicalize path to handle symlinks and different path representations
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        // Check if already indexed
        if let Some(&existing_id) = self.path_to_id.get(&canonical) {
            return Ok(existing_id);
        }

        let mapped = MappedFile::new(path)?;
        let id = self.files.len() as u32;
        self.path_to_id.insert(canonical, id);
        self.files.push(mapped);
        Ok(id)
    }

    /// Get a file by ID
    pub fn get(&self, id: u32) -> Option<&MappedFile> {
        self.files.get(id as usize)
    }

    /// Get the total number of files
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Get total size of all files
    pub fn total_size(&self) -> u64 {
        self.files.iter().map(|f| f.len() as u64).sum()
    }

    /// Get a file path by ID (borrowed, no allocation)
    pub fn get_path(&self, id: u32) -> Option<&Path> {
        self.files.get(id as usize).map(|f| f.path.as_path())
    }

    /// Get all file paths as a vector (for path filtering)
    /// Note: This clones all paths - prefer get_path() for single lookups
    pub fn get_all_paths(&self) -> Vec<PathBuf> {
        self.files.iter().map(|f| f.path.clone()).collect()
    }
}

impl Default for FileStore {
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
    fn test_mapped_file_new() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "hello world").expect("Failed to write test file");

        let mapped = MappedFile::new(&file_path).expect("Failed to create MappedFile");
        assert_eq!(mapped.len(), 11);
        assert!(!mapped.is_empty());
        assert_eq!(mapped.as_str().expect("Failed to read as str"), "hello world");
        assert_eq!(mapped.as_bytes(), b"hello world");
    }

    #[test]
    fn test_mapped_file_empty() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("empty.txt");
        std::fs::write(&file_path, "").expect("Failed to write test file");

        let mapped = MappedFile::new(&file_path).expect("Failed to create MappedFile");
        assert_eq!(mapped.len(), 0);
        assert!(mapped.is_empty());
        assert_eq!(mapped.as_str().expect("Failed to read as str"), "");
    }

    #[test]
    fn test_mapped_file_invalid_utf8() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("binary.bin");
        
        // Write invalid UTF-8 bytes
        let mut file = std::fs::File::create(&file_path).expect("Failed to create file");
        file.write_all(&[0xFF, 0xFE, 0x00, 0x01]).expect("Failed to write bytes");
        drop(file);

        let mapped = MappedFile::new(&file_path).expect("Failed to create MappedFile");
        assert!(mapped.as_str().is_err());
        // But as_bytes should still work
        assert_eq!(mapped.as_bytes(), &[0xFF, 0xFE, 0x00, 0x01]);
    }

    #[test]
    fn test_mapped_file_nonexistent() {
        let result = MappedFile::new("/nonexistent/path/to/file.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_file_store_new() {
        let store = FileStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.total_size(), 0);
    }

    #[test]
    fn test_file_store_add_and_get() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "hello world").expect("Failed to write test file");

        let mut store = FileStore::new();
        let id = store.add_file(&file_path).expect("Failed to add file");
        
        assert_eq!(id, 0);
        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());
        assert_eq!(store.total_size(), 11);
        
        let file = store.get(id).expect("Failed to get file");
        assert_eq!(file.as_str().expect("Failed to read as str"), "hello world");
    }

    #[test]
    fn test_file_store_duplicate_handling() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "hello world").expect("Failed to write test file");

        let mut store = FileStore::new();
        let id1 = store.add_file(&file_path).expect("Failed to add file first time");
        let id2 = store.add_file(&file_path).expect("Failed to add file second time");
        
        // Same file added twice should return same ID
        assert_eq!(id1, id2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_file_store_multiple_files() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");
        std::fs::write(&file1, "hello").expect("Failed to write file1");
        std::fs::write(&file2, "world").expect("Failed to write file2");

        let mut store = FileStore::new();
        let id1 = store.add_file(&file1).expect("Failed to add file1");
        let id2 = store.add_file(&file2).expect("Failed to add file2");
        
        assert_eq!(id1, 0);
        assert_eq!(id2, 1);
        assert_eq!(store.len(), 2);
        assert_eq!(store.total_size(), 10); // "hello" (5) + "world" (5)
    }

    #[test]
    fn test_file_store_get_path() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "content").expect("Failed to write test file");

        let mut store = FileStore::new();
        let id = store.add_file(&file_path).expect("Failed to add file");
        
        let path = store.get_path(id).expect("Failed to get path");
        assert!(path.ends_with("test.txt"));
        
        // Non-existent ID should return None
        assert!(store.get_path(999).is_none());
    }

    #[test]
    fn test_file_store_get_all_paths() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");
        std::fs::write(&file1, "hello").expect("Failed to write file1");
        std::fs::write(&file2, "world").expect("Failed to write file2");

        let mut store = FileStore::new();
        store.add_file(&file1).expect("Failed to add file1");
        store.add_file(&file2).expect("Failed to add file2");
        
        let paths = store.get_all_paths();
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn test_file_store_get_nonexistent() {
        let store = FileStore::new();
        assert!(store.get(0).is_none());
        assert!(store.get(100).is_none());
    }

    #[test]
    fn test_file_store_default() {
        let store = FileStore::default();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }
}
