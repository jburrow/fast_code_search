use anyhow::{Context, Result};
use memmap2::Mmap;
use std::fs::File;
use std::path::{Path, PathBuf};

/// Represents a memory-mapped file
pub struct MappedFile {
    pub path: PathBuf,
    pub mmap: Mmap,
}

impl MappedFile {
    /// Create a new memory-mapped file
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let file = File::open(path)
            .with_context(|| format!("Failed to open file: {}", path.display()))?;
        
        let mmap = unsafe { 
            Mmap::map(&file)
                .with_context(|| format!("Failed to mmap file: {}", path.display()))?
        };

        Ok(Self {
            path: path.to_path_buf(),
            mmap,
        })
    }

    /// Get the content as a string slice (if valid UTF-8)
    pub fn as_str(&self) -> Result<&str> {
        std::str::from_utf8(&self.mmap)
            .with_context(|| format!("File is not valid UTF-8: {}", self.path.display()))
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
}

impl FileStore {
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    /// Add a file to the store and return its ID
    pub fn add_file(&mut self, path: impl AsRef<Path>) -> Result<u32> {
        let mapped = MappedFile::new(path)?;
        let id = self.files.len() as u32;
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
}

impl Default for FileStore {
    fn default() -> Self {
        Self::new()
    }
}
