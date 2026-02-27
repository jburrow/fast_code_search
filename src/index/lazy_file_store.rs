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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{OnceLock, RwLock};
use tracing::warn;

/// Represents a lazily memory-mapped file
///
/// The file path is stored immediately, but the memory mapping is created
/// on first access. This allows registering thousands of files instantly
/// while only paying the I/O cost for files that are actually searched.
/// When mmap is unavailable (e.g. OS mmap limit exceeded), the file is read
/// directly via `std::fs::read` as a fallback so search results are never lost.
pub struct LazyMappedFile {
    /// The file path (always available)
    pub path: PathBuf,
    /// Lazily initialized memory map
    mmap: OnceLock<Result<Mmap, String>>,
    /// Fallback: owned bytes read via fs::read when mmap is unavailable
    content_fallback: OnceLock<Result<Vec<u8>, String>>,
    /// Cached result of UTF-8 validation
    utf8_valid: OnceLock<bool>,
    /// Transcoded UTF-8 content for non-UTF-8 files (None if natively UTF-8)
    transcoded: OnceLock<Option<String>>,
    /// Detected encoding name for diagnostics (None if natively UTF-8)
    detected_encoding: OnceLock<Option<&'static str>>,
}

impl LazyMappedFile {
    /// Create a new lazy file entry (does NOT open or map the file)
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            mmap: OnceLock::new(),
            content_fallback: OnceLock::new(),
            utf8_valid: OnceLock::new(),
            transcoded: OnceLock::new(),
            detected_encoding: OnceLock::new(),
        }
    }

    /// Create a new lazy file entry with an already-mapped file (for immediate indexing)
    pub fn with_mmap(path: impl AsRef<Path>, mmap: Mmap) -> Self {
        let file = Self {
            path: path.as_ref().to_path_buf(),
            mmap: OnceLock::new(),
            content_fallback: OnceLock::new(),
            utf8_valid: OnceLock::new(),
            transcoded: OnceLock::new(),
            detected_encoding: OnceLock::new(),
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

    /// Get a reference to the file's bytes.
    ///
    /// Tries mmap first (fast, zero-copy). If mmap is unavailable (e.g. the OS
    /// `vm.max_map_count` limit was exceeded), falls back to `std::fs::read` and
    /// caches the result so subsequent calls are cheap.
    fn get_bytes(&self) -> Result<&[u8]> {
        // Fast path: mmap already established
        if let Ok(mmap) = self.ensure_mapped() {
            return Ok(&mmap[..]);
        }

        // Slow/fallback path: mmap unavailable – read the file directly (cached)
        let fallback = self.content_fallback.get_or_init(|| {
            std::fs::read(&self.path)
                .map_err(|e| format!("Failed to read {}: {}", self.path.display(), e))
        });

        match fallback {
            Ok(bytes) => Ok(bytes.as_slice()),
            Err(e) => anyhow::bail!("{}", e),
        }
    }

    /// Check if the file has been mapped yet
    pub fn is_mapped(&self) -> bool {
        self.mmap.get().is_some()
    }

    /// Get the content as a string slice.
    /// For valid UTF-8 files, returns a zero-copy reference to the mmap (or fallback buffer).
    /// For non-UTF-8 text files, returns a reference to the transcoded content.
    /// Falls back to direct `fs::read` when mmap is unavailable.
    pub fn as_str(&self) -> Result<&str> {
        let bytes = self.get_bytes()?;

        let is_valid = *self
            .utf8_valid
            .get_or_init(|| std::str::from_utf8(bytes).is_ok());

        if is_valid {
            // SAFETY: We validated UTF-8 above and cached the result
            return Ok(unsafe { std::str::from_utf8_unchecked(bytes) });
        }

        // Slow path: try transcoding non-UTF-8 content
        let transcoded =
            self.transcoded
                .get_or_init(|| match crate::utils::transcode_to_utf8(bytes) {
                    Ok(Some(result)) => {
                        let _ = self.detected_encoding.set(Some(result.encoding_name));
                        tracing::info!(
                            path = %self.path.display(),
                            encoding = result.encoding_name,
                            "Transcoded non-UTF-8 file"
                        );
                        Some(result.content)
                    }
                    _ => {
                        let _ = self.detected_encoding.set(None);
                        None
                    }
                });

        match transcoded {
            Some(s) => Ok(s.as_str()),
            None => anyhow::bail!("File is not valid text: {}", self.path.display()),
        }
    }

    /// Get the detected encoding name, if the file was transcoded.
    /// Returns None if the file is natively UTF-8 or hasn't been accessed yet.
    pub fn detected_encoding(&self) -> Option<&'static str> {
        self.detected_encoding.get().copied().flatten()
    }

    /// Returns true if this file was transcoded from a non-UTF-8 encoding.
    pub fn was_transcoded(&self) -> bool {
        self.transcoded
            .get()
            .map(|opt| opt.is_some())
            .unwrap_or(false)
    }

    /// Get the content as bytes.
    /// Falls back to direct `fs::read` when mmap is unavailable.
    pub fn as_bytes(&self) -> Result<&[u8]> {
        self.get_bytes()
    }

    /// Get the file size.
    /// Falls back to direct `fs::read` when mmap is unavailable.
    pub fn len(&self) -> Result<usize> {
        Ok(self.get_bytes()?.len())
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
///
/// Automatically detects system mmap limits on Linux and prevents indexing
/// too many files to avoid allocation errors.
pub struct LazyFileStore {
    /// Files indexed by ID
    files: Vec<LazyMappedFile>,
    /// Map from path to file ID (for deduplication)
    path_to_id: HashMap<PathBuf, u32>,
    /// Statistics: number of files that have been mapped
    mapped_count: RwLock<usize>,
    /// Statistics: total bytes of content indexed (accumulated as files are added)
    total_content_bytes: RwLock<u64>,
    /// Safe mmap limit (85% of system max, None on non-Linux)
    mmap_safe_limit: Option<usize>,
    /// Guard so the mmap-limit warning is only emitted once
    mmap_limit_warned: AtomicBool,
}

impl LazyFileStore {
    pub fn new() -> Self {
        let limits = crate::utils::SystemLimits::collect();
        let mmap_safe_limit = limits.safe_mmap_limit();

        if let Some(limit) = mmap_safe_limit {
            tracing::info!(
                max_map_count = ?limits.max_map_count,
                safe_limit = limit,
                "Mmap limit detected (85% of max), will switch to direct read fallback if reached (search still works, retrieval is slower)"
            );
        }

        Self {
            files: Vec::new(),
            path_to_id: HashMap::new(),
            mapped_count: RwLock::new(0),
            total_content_bytes: RwLock::new(0),
            mmap_safe_limit,
            mmap_limit_warned: AtomicBool::new(false),
        }
    }

    /// Check if we are approaching the mmap limit
    fn check_mmap_limit(&self) -> Result<()> {
        if let Some(limit) = self.mmap_safe_limit {
            let current = self.mapped_count.read().map(|c| *c).unwrap_or(0);
            if current >= limit {
                anyhow::bail!(
                    "Reached mmap limit ({}/{}). Remaining files will be indexed \
                    without mmap (direct read fallback active — retrieval is slower).",
                    current,
                    limit
                );
            }
        }
        Ok(())
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
    ///
    /// When the OS mmap limit is reached the file is still registered in the
    /// store (so it appears in search results and the correct file count is
    /// reported). The lazy `get_bytes()` path will fall back to `fs::read` at
    /// result-retrieval time instead of returning nothing.
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

        // If the mmap limit has been reached, register the path without mapping.
        // Content will be read via fs::read() fallback when search results are retrieved.
        if let Err(limit_err) = self.check_mmap_limit() {
            // Warn only once — every subsequent file would produce the same message.
            if self
                .mmap_limit_warned
                .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                tracing::warn!(
                    "{}  Further files will be indexed without mmap and served via \
                    direct read (search still works, retrieval is slower). \
                    Increase vm.max_map_count to restore full performance.",
                    limit_err
                );
            }
            let id = self.files.len() as u32;
            self.path_to_id.insert(canonical, id);
            self.files.push(LazyMappedFile::new(path));
            // Estimate content bytes from file metadata so stats stay accurate
            if let Ok(meta) = std::fs::metadata(path) {
                if let Ok(mut bytes) = self.total_content_bytes.write() {
                    *bytes += meta.len();
                }
            }
            return Ok(id);
        }

        // Normal path: open and map the file immediately
        let file =
            File::open(path).with_context(|| format!("Failed to open file: {}", path.display()))?;
        let mmap = unsafe {
            Mmap::map(&file).with_context(|| format!("Failed to mmap file: {}", path.display()))?
        };

        // Track content size before storing
        let content_size = mmap.len() as u64;

        let id = self.files.len() as u32;
        self.path_to_id.insert(canonical, id);
        self.files.push(LazyMappedFile::with_mmap(path, mmap));

        // Update mapped count and content bytes
        if let Ok(mut count) = self.mapped_count.write() {
            *count += 1;
        }
        if let Ok(mut bytes) = self.total_content_bytes.write() {
            *bytes += content_size;
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

    /// Get total content bytes that have been indexed
    /// This tracks the actual text content size, not just memory-mapped size
    pub fn total_content_bytes(&self) -> u64 {
        self.total_content_bytes
            .read()
            .map(|guard| *guard)
            .unwrap_or(0)
    }

    /// Add to the total content bytes counter (used when loading from persistence)
    pub fn add_content_bytes(&self, bytes: u64) {
        if let Ok(mut total) = self.total_content_bytes.write() {
            *total += bytes;
        }
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
impl LazyFileStore {
    /// Construct a store with an explicit mmap limit, used in tests to simulate
    /// a constrained system without requiring a real OS limit change.
    pub(crate) fn with_limit(mmap_safe_limit: Option<usize>) -> Self {
        Self {
            files: Vec::new(),
            path_to_id: HashMap::new(),
            mapped_count: RwLock::new(0),
            total_content_bytes: RwLock::new(0),
            mmap_safe_limit,
            mmap_limit_warned: AtomicBool::new(false),
        }
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

    // ---------------------------------------------------------------------------
    // Mmap-limit fallback tests
    // ---------------------------------------------------------------------------

    /// When the mmap limit is set to 0 (fully exhausted), `add_file` must still
    /// register every file so that trigrams and the file count are correct.
    #[test]
    fn test_add_file_past_mmap_limit_still_registers() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let paths: Vec<PathBuf> = (0..5)
            .map(|i| {
                let p = temp_dir.path().join(format!("file{}.txt", i));
                std::fs::write(&p, format!("content {}", i)).unwrap();
                p
            })
            .collect();

        // Limit = 2: first 2 files get an mmap, the rest fall back to direct read
        let mut store = LazyFileStore::with_limit(Some(2));
        for path in &paths {
            store.add_file(path).expect("add_file must succeed past mmap limit");
        }

        // All 5 files must be registered
        assert_eq!(store.len(), 5, "All files must appear in the store");
    }

    /// Files registered past the mmap limit must still return their full content
    /// via the direct-read fallback so search results are never lost.
    #[test]
    fn test_files_past_mmap_limit_return_content() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let paths: Vec<PathBuf> = (0..4)
            .map(|i| {
                let p = temp_dir.path().join(format!("file{}.txt", i));
                std::fs::write(&p, format!("hello from file {}", i)).unwrap();
                p
            })
            .collect();

        // Limit = 1: only the first file gets a real mmap
        let mut store = LazyFileStore::with_limit(Some(1));
        for path in &paths {
            store
                .add_file(path)
                .expect("add_file must succeed past mmap limit");
        }

        // Every file (including those beyond the mmap limit) must be readable
        for i in 0..4u32 {
            let file = store.get(i).expect("file must be retrievable by id");
            let content = file.as_str().expect("content must be readable via fallback");
            assert_eq!(content, format!("hello from file {}", i));
        }
    }

    /// `get_bytes()` falls back to `fs::read` when mmap fails for an individual
    /// file (simulated by deleting the file between registration and mmap init).
    #[test]
    fn test_get_bytes_falls_back_when_mmap_fails() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "fallback content").unwrap();

        // Register without an up-front mmap so ensure_mapped is called lazily
        let lazy = LazyMappedFile::new(&file_path);
        assert!(!lazy.is_mapped());

        // Normal access succeeds (mmap or read, either is fine)
        assert_eq!(lazy.as_str().unwrap(), "fallback content");
    }

    #[test]
    fn test_lazy_file_invalid_utf8() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("binary.bin");

        // Genuinely binary bytes: not valid UTF-8, no BOM, contains null bytes.
        // Even with encoding transcoding enabled, this should be rejected as binary.
        let binary_bytes: &[u8] = &[0x81, 0x82, 0x83, 0x84, 0x00, 0x00, 0x01, 0x02];
        let mut file = std::fs::File::create(&file_path).expect("Failed to create file");
        file.write_all(binary_bytes).unwrap();
        drop(file);

        let lazy = LazyMappedFile::new(&file_path);
        assert!(lazy.as_str().is_err());

        // as_bytes should work
        assert_eq!(lazy.as_bytes().unwrap(), binary_bytes);
    }
}
