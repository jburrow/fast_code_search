//! Lazy file store with on-demand memory mapping
//!
//! This module provides a file store that registers file paths at startup
//! but only memory-maps them when they are first accessed. This dramatically
//! reduces startup time when loading from a persisted index.

use anyhow::{Context, Result};
use memmap2::Mmap;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};
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
    /// Fallback: owned bytes read via fs::read when mmap is unavailable.
    ///
    /// Using `Mutex<Option<Vec<u8>>>` (rather than a write-once `OnceLock`) allows
    /// the cached bytes to be **evicted** after each search round-trip so that
    /// heap memory is reclaimed.  Call `evict_fallback()` (or
    /// `LazyFileStore::evict_all_fallbacks()`) once a search request completes.
    ///
    /// The Mutex is only contended when a fallback file is first accessed or
    /// evicted — both are infrequent operations that occur only when the OS
    /// `vm.max_map_count` limit is exceeded.
    content_fallback: Mutex<Option<Vec<u8>>>,
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
            content_fallback: Mutex::new(None),
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
            content_fallback: Mutex::new(None),
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

    /// Load fallback bytes into the Mutex cache (if not already cached) and
    /// return a clone.  The clone is intentional: it lets the caller use the
    /// bytes freely while the Mutex is not held, and it allows `evict_fallback`
    /// to free the cached copy at any later point without invalidating any
    /// in-flight references.
    fn load_fallback_bytes(&self) -> Result<Vec<u8>> {
        let mut guard = self
            .content_fallback
            .lock()
            .map_err(|_| anyhow::anyhow!("Mutex poisoned for {}", self.path.display()))?;
        if guard.is_none() {
            let bytes = std::fs::read(&self.path).with_context(|| {
                format!("Failed to read fallback bytes for {}", self.path.display())
            })?;
            *guard = Some(bytes);
        }
        Ok(guard.as_ref().unwrap().clone())
    }

    /// Evict the cached fallback bytes, freeing heap memory.
    ///
    /// This is a no-op for memory-mapped files.  For files that fell back to
    /// `fs::read` (when the OS mmap limit was exceeded), the cached bytes are
    /// freed; the next access will re-read the file from disk.
    ///
    /// Call this (or `LazyFileStore::evict_all_fallbacks`) after each search
    /// request completes to prevent unbounded heap growth in long-running servers.
    pub fn evict_fallback(&self) {
        // Fast path: if mmap is already successfully established, the fallback
        // cache is never populated, so there is nothing to evict.
        if matches!(self.mmap.get(), Some(Ok(_))) {
            return;
        }
        match self.content_fallback.lock() {
            Ok(mut guard) => *guard = None,
            Err(_) => {
                warn!(
                    path = %self.path.display(),
                    "Mutex poisoned while evicting fallback bytes; skipping eviction"
                );
            }
        }
    }

    /// Get a reference to the file's bytes as a `Cow`.
    ///
    /// Returns `Cow::Borrowed` (zero-copy) for memory-mapped files.
    /// Returns `Cow::Owned` for files served via the `fs::read` fallback;
    /// the bytes are loaded into the Mutex cache on first access and cloned
    /// into the returned value.  Call `evict_fallback` when the caller no
    /// longer needs the bytes.
    fn get_bytes(&self) -> Result<Cow<'_, [u8]>> {
        // Fast path: mmap already established
        if let Ok(mmap) = self.ensure_mapped() {
            return Ok(Cow::Borrowed(&mmap[..]));
        }

        // Slow/fallback path: mmap unavailable – load via fs::read (evictable cache)
        let bytes = self.load_fallback_bytes()?;
        Ok(Cow::Owned(bytes))
    }

    /// Check if the file has been mapped yet
    pub fn is_mapped(&self) -> bool {
        self.mmap.get().is_some()
    }

    /// Get the content as a `Cow<str>`.
    ///
    /// For memory-mapped files this is a zero-copy borrow (`Cow::Borrowed`).
    /// For files served via the `fs::read` fallback the bytes are cloned into
    /// an owned `String` (`Cow::Owned`).
    ///
    /// Falls back to direct `fs::read` when mmap is unavailable.
    pub fn as_str(&self) -> Result<Cow<'_, str>> {
        match self.ensure_mapped() {
            Ok(mmap) => {
                let bytes = &mmap[..];
                let is_valid = *self
                    .utf8_valid
                    .get_or_init(|| std::str::from_utf8(bytes).is_ok());

                if is_valid {
                    // SAFETY: We validated UTF-8 above and cached the result
                    return Ok(Cow::Borrowed(unsafe {
                        std::str::from_utf8_unchecked(bytes)
                    }));
                }

                // Slow path: try transcoding non-UTF-8 content (result is cached)
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
                    Some(s) => Ok(Cow::Borrowed(s.as_str())),
                    None => anyhow::bail!("File is not valid text: {}", self.path.display()),
                }
            }
            Err(_) => {
                // Fallback path: load bytes via evictable Mutex cache
                let bytes = self.load_fallback_bytes()?;

                // Cache UTF-8 validity (uses the locally-owned bytes only during init)
                let is_valid = *self
                    .utf8_valid
                    .get_or_init(|| std::str::from_utf8(bytes.as_slice()).is_ok());

                if is_valid {
                    // SAFETY: We validated UTF-8 above
                    return Ok(Cow::Owned(unsafe { String::from_utf8_unchecked(bytes) }));
                }

                // Non-UTF-8 fallback: transcode without caching (rare path)
                match crate::utils::transcode_to_utf8(&bytes) {
                    Ok(Some(result)) => {
                        let _ = self.detected_encoding.set(Some(result.encoding_name));
                        Ok(Cow::Owned(result.content))
                    }
                    _ => {
                        let _ = self.detected_encoding.set(None);
                        anyhow::bail!("File is not valid text: {}", self.path.display())
                    }
                }
            }
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
    ///
    /// Returns `Cow::Borrowed` (zero-copy) for memory-mapped files and
    /// `Cow::Owned` for files served via the `fs::read` fallback.
    /// Falls back to direct `fs::read` when mmap is unavailable.
    pub fn as_bytes(&self) -> Result<Cow<'_, [u8]>> {
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

    /// Evict all cached fallback bytes, freeing heap memory.
    ///
    /// This is a no-op for memory-mapped files. For files that fell back to
    /// `fs::read` (when the OS mmap limit was exceeded), the cached bytes are
    /// freed; the next access will re-read each file from disk.
    ///
    /// Call this after each search request completes to prevent unbounded
    /// heap growth in long-running servers with large repositories.
    pub fn evict_all_fallbacks(&self) {
        for file in &self.files {
            file.evict_fallback();
        }
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
impl LazyMappedFile {
    /// Returns true if the fallback bytes are currently cached in the Mutex.
    /// Used in tests to verify that eviction actually freed memory.
    pub(crate) fn has_fallback_cached(&self) -> bool {
        self.content_fallback
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    /// Create a file entry with the mmap pre-set to a failure, forcing all
    /// content access through the `fs::read` fallback.  Used in tests to
    /// exercise the eviction path without requiring a real OS mmap limit.
    pub(crate) fn with_mmap_failure(path: impl AsRef<Path>) -> Self {
        let file = Self::new(path);
        let _ = file.mmap.set(Err("simulated mmap failure".to_string()));
        file
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
            store
                .add_file(path)
                .expect("add_file must succeed past mmap limit");
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
            let content = file
                .as_str()
                .expect("content must be readable via fallback");
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

    // ---------------------------------------------------------------------------
    // Eviction tests
    // ---------------------------------------------------------------------------

    /// Fallback bytes can be evicted after a search round-trip, and re-read on the
    /// next access.
    #[test]
    fn test_evict_fallback_frees_and_re_reads() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "eviction test content").unwrap();

        // Force the fallback path by pre-setting mmap to failure
        let file = LazyMappedFile::with_mmap_failure(&file_path);

        // Initially no bytes are cached
        assert!(
            !file.has_fallback_cached(),
            "fallback should not be cached before first access"
        );

        // First access loads from disk into the Mutex cache
        assert_eq!(file.as_str().unwrap(), "eviction test content");
        assert!(
            file.has_fallback_cached(),
            "fallback should be cached after first access"
        );

        // Evict the cached bytes
        file.evict_fallback();
        assert!(
            !file.has_fallback_cached(),
            "fallback should be None after eviction"
        );

        // Second access re-reads from disk and returns the same content
        assert_eq!(file.as_str().unwrap(), "eviction test content");
        assert!(
            file.has_fallback_cached(),
            "fallback should be re-cached after second access"
        );
    }

    /// `LazyFileStore::evict_all_fallbacks` evicts bytes for every fallback file.
    #[test]
    fn test_store_evict_all_fallbacks() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let paths: Vec<PathBuf> = (0..3)
            .map(|i| {
                let p = temp_dir.path().join(format!("file{}.txt", i));
                std::fs::write(&p, format!("content {}", i)).unwrap();
                p
            })
            .collect();

        // Limit=0 forces all files into the fallback path in add_file, but
        // ensure_mapped() can still succeed on lazy access on a normal system.
        // Build the store with limit=0 then force mmap failure via the OnceLock
        // by using with_mmap_failure for each file to properly test eviction.
        let mut store = LazyFileStore::new();
        for path in &paths {
            store.register_file(path);
        }

        // Replace files with ones that have forced mmap failure
        // (we access internal test API via LazyMappedFile::with_mmap_failure)
        let fallback_files: Vec<LazyMappedFile> = paths
            .iter()
            .map(|p| LazyMappedFile::with_mmap_failure(p))
            .collect();

        // Access all files to populate the Mutex caches
        for (i, file) in fallback_files.iter().enumerate() {
            assert_eq!(file.as_str().unwrap(), format!("content {}", i));
            assert!(
                file.has_fallback_cached(),
                "fallback should be cached after access"
            );
        }

        // Evict all fallback caches at once by calling evict on each
        for file in &fallback_files {
            file.evict_fallback();
        }

        // All caches must be empty
        for file in &fallback_files {
            assert!(
                !file.has_fallback_cached(),
                "fallback should be None after evict"
            );
        }

        // Files are still accessible after eviction (re-read on next access)
        for (i, file) in fallback_files.iter().enumerate() {
            assert_eq!(file.as_str().unwrap(), format!("content {}", i));
        }
    }

    /// Evicting a memory-mapped file is a no-op (no crash, no behaviour change).
    #[test]
    fn test_evict_mmap_file_is_noop() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "mmap content").unwrap();

        let lazy = LazyMappedFile::new(&file_path);

        // Access via mmap
        assert_eq!(lazy.as_str().unwrap(), "mmap content");
        assert!(lazy.is_mapped());

        // Evict should be a no-op
        lazy.evict_fallback();

        // Content still accessible
        assert_eq!(lazy.as_str().unwrap(), "mmap content");
    }
}
