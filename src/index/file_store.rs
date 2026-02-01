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
        let canonical = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf());

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
