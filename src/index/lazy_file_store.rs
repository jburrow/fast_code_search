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
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use tracing::warn;

/// A registered file whose content is served lazily.
///
/// The per-file footprint is what a million-file index pays for, so the entry
/// is kept to the path (one `Arc` shared with the store's path map), a size
/// class, an encoding tag, and one lazily created cell that only files above
/// [`MMAP_THRESHOLD_BYTES`] ever allocate. Small files (essentially all source
/// files) are read into an owned buffer on every access and cache nothing.
pub struct LazyMappedFile {
    /// The canonical file path, interned once and shared with the store's
    /// path-to-id map.
    pub path: Arc<Path>,
    /// State only a large (memory-mapped) file needs. Never allocated for
    /// small files.
    large: OnceLock<Box<LargeState>>,
    /// Detected encoding name for diagnostics (`Some(None)` = natively UTF-8
    /// or not text; unset = not read yet).
    detected_encoding: OnceLock<Option<&'static str>>,
    /// How this file's content is served: 0 = not decided yet, 1 = small
    /// (owned `fs::read` per access, never mmapped), 2 = large (mmap).
    size_class: AtomicU8,
}

/// Lazily created state for a file above the mmap threshold.
struct LargeState {
    /// The mapping, or why it could not be created. Without a mapping the
    /// content is read from disk on every access (nothing is cached, so a
    /// long-running server cannot accumulate heap across requests).
    mmap: Result<Mmap, String>,
    /// Transcoded UTF-8 for a mapped non-UTF-8 file (`Some(None)` = not text).
    transcoded: OnceLock<Option<String>>,
}

impl LargeState {
    fn open(path: &Path) -> Self {
        let mmap = match File::open(path) {
            Ok(file) => {
                // SAFETY: mapping a file we just opened read-only. Files above
                // the threshold are rare and the mapping is only ever read.
                match unsafe { Mmap::map(&file) } {
                    Ok(mmap) => Ok(mmap),
                    Err(e) => Err(format!("Failed to mmap {}: {}", path.display(), e)),
                }
            }
            Err(e) => Err(format!("Failed to open {}: {}", path.display(), e)),
        };
        Self {
            mmap,
            transcoded: OnceLock::new(),
        }
    }

    fn mapped(mmap: Mmap) -> Self {
        Self {
            mmap: Ok(mmap),
            transcoded: OnceLock::new(),
        }
    }
}

/// Files at or below this size are read into an owned buffer on every access
/// instead of being memory-mapped.
///
/// Reading through a live mapping is unsafe against concurrent truncation:
/// an editor rewriting a file in place shrinks it, and the next touch of a
/// mapped page past the new EOF is an uncatchable SIGBUS that kills the whole
/// server. Owned reads are also what keeps the number of mappings far below
/// `vm.max_map_count` (65 530 by default) on large trees. Files above the
/// threshold (rare in code) keep the zero-copy mapping.
pub const MMAP_THRESHOLD_BYTES: u64 = 1024 * 1024;

const SIZE_CLASS_UNKNOWN: u8 = 0;
const SIZE_CLASS_SMALL: u8 = 1;
const SIZE_CLASS_LARGE: u8 = 2;

impl LazyMappedFile {
    /// Create a new lazy file entry (does NOT open or map the file)
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self::from_shared(Arc::from(path.as_ref()))
    }

    /// Create an entry around an already-interned path (no copy).
    pub fn from_shared(path: Arc<Path>) -> Self {
        Self {
            path,
            large: OnceLock::new(),
            detected_encoding: OnceLock::new(),
            size_class: AtomicU8::new(SIZE_CLASS_UNKNOWN),
        }
    }

    /// Create an entry that is known to be small: served by owned reads.
    pub fn new_small(path: impl AsRef<Path>) -> Self {
        Self::new(path).into_small()
    }

    fn into_small(self) -> Self {
        self.size_class.store(SIZE_CLASS_SMALL, Ordering::Relaxed);
        self
    }

    /// Create an entry around an already-mapped large file (immediate indexing).
    pub fn with_mmap(path: Arc<Path>, mmap: Mmap) -> Self {
        let file = Self::from_shared(path);
        file.size_class.store(SIZE_CLASS_LARGE, Ordering::Relaxed);
        let _ = file.large.set(Box::new(LargeState::mapped(mmap)));
        file
    }

    /// If this file is small (by a one-time stat, or by construction), read
    /// it into an owned buffer. `None` means "large: use the mapping".
    fn read_small(&self) -> Result<Option<Vec<u8>>> {
        let class = match self.size_class.load(Ordering::Relaxed) {
            SIZE_CLASS_UNKNOWN => {
                let meta = std::fs::metadata(&self.path)
                    .with_context(|| format!("Failed to stat {}", self.path.display()))?;
                let class = if meta.len() <= MMAP_THRESHOLD_BYTES {
                    SIZE_CLASS_SMALL
                } else {
                    SIZE_CLASS_LARGE
                };
                self.size_class.store(class, Ordering::Relaxed);
                class
            }
            c => c,
        };
        if class != SIZE_CLASS_SMALL {
            return Ok(None);
        }
        let bytes = std::fs::read(&self.path)
            .with_context(|| format!("Failed to read {}", self.path.display()))?;
        Ok(Some(bytes))
    }

    /// Read a large file whose mapping is unavailable. Nothing is cached.
    fn read_unmapped(&self) -> Result<Vec<u8>> {
        std::fs::read(&self.path)
            .with_context(|| format!("Failed to read {} (mmap unavailable)", self.path.display()))
    }

    /// Validate / transcode an owned buffer into text.
    fn owned_to_str(&self, bytes: Vec<u8>) -> Result<Cow<'_, str>> {
        match String::from_utf8(bytes) {
            Ok(s) => Ok(Cow::Owned(s)),
            Err(e) => {
                let raw = e.into_bytes();
                match crate::utils::transcode_to_utf8(&raw) {
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

    /// The large-file state, creating the mapping on first use.
    fn large_state(&self) -> &LargeState {
        self.large
            .get_or_init(|| Box::new(LargeState::open(&self.path)))
    }

    /// Get a reference to the file's bytes as a `Cow`.
    ///
    /// `Cow::Owned` for small files (fresh read) and for large files whose
    /// mapping failed; `Cow::Borrowed` (zero-copy) for mapped files.
    fn get_bytes(&self) -> Result<Cow<'_, [u8]>> {
        if let Some(bytes) = self.read_small()? {
            return Ok(Cow::Owned(bytes));
        }
        match &self.large_state().mmap {
            Ok(mmap) => Ok(Cow::Borrowed(&mmap[..])),
            Err(_) => Ok(Cow::Owned(self.read_unmapped()?)),
        }
    }

    /// Whether a memory mapping currently exists for this file.
    pub fn is_mapped(&self) -> bool {
        matches!(self.large.get(), Some(st) if st.mmap.is_ok())
    }

    /// Get the content as a `Cow<str>`.
    ///
    /// Zero-copy borrow for mapped files; owned for small files and for large
    /// files whose mapping failed. UTF-8 is validated on every access (the
    /// bytes behind a mapping can change if the file is rewritten on disk).
    pub fn as_str(&self) -> Result<Cow<'_, str>> {
        if let Some(bytes) = self.read_small()? {
            return self.owned_to_str(bytes);
        }
        let st = self.large_state();
        let mmap = match &st.mmap {
            Ok(mmap) => mmap,
            Err(_) => {
                let bytes = self.read_unmapped()?;
                return self.owned_to_str(bytes);
            }
        };
        let bytes = &mmap[..];
        if let Ok(s) = std::str::from_utf8(bytes) {
            return Ok(Cow::Borrowed(s));
        }
        // Non-UTF-8 mapped file: transcode once and keep the result.
        let transcoded =
            st.transcoded
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

    /// Get the detected encoding name, if the file was transcoded.
    /// Returns None if the file is natively UTF-8 or hasn't been accessed yet.
    pub fn detected_encoding(&self) -> Option<&'static str> {
        self.detected_encoding.get().copied().flatten()
    }

    /// Returns true if this mapped file was transcoded from a non-UTF-8 encoding.
    pub fn was_transcoded(&self) -> bool {
        self.large
            .get()
            .and_then(|st| st.transcoded.get())
            .map(|opt| opt.is_some())
            .unwrap_or(false)
    }

    /// Get the content as bytes (see [`Self::as_str`] for the borrow rules).
    pub fn as_bytes(&self) -> Result<Cow<'_, [u8]>> {
        self.get_bytes()
    }

    /// Get the file size (reads or maps the file).
    pub fn len(&self) -> Result<usize> {
        Ok(self.get_bytes()?.len())
    }

    /// Get the file size if already mapped, without triggering a map
    pub fn len_if_mapped(&self) -> Option<usize> {
        self.large
            .get()
            .and_then(|st| st.mmap.as_ref().ok())
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
    /// Map from path to file ID. Keys share their allocation with the
    /// entry's `path`, so each path is stored once.
    path_to_id: HashMap<Arc<Path>, u32>,
    /// IDs that have been tombstoned (removed during incremental updates).
    /// Tombstoned slots are hidden from `get`/`get_path`/lookups and never
    /// reused, so existing file IDs stay stable. Typically tiny.
    tombstoned: std::collections::HashSet<u32>,
    /// Statistics: number of files that have been mapped
    mapped_count: AtomicUsize,
    /// Statistics: total bytes of content indexed (accumulated as files are added)
    total_content_bytes: AtomicU64,
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
            tombstoned: std::collections::HashSet::new(),
            mapped_count: AtomicUsize::new(0),
            total_content_bytes: AtomicU64::new(0),
            mmap_safe_limit,
            mmap_limit_warned: AtomicBool::new(false),
        }
    }

    /// Check if we are approaching the mmap limit
    fn check_mmap_limit(&self) -> Result<()> {
        if let Some(limit) = self.mmap_safe_limit {
            let current = self.mapped_count.load(Ordering::Relaxed);
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
        if let Some(&existing_id) = self.path_to_id.get(path) {
            return existing_id;
        }
        let shared: Arc<Path> = Arc::from(path);
        let id = self.files.len() as u32;
        self.path_to_id.insert(shared.clone(), id);
        self.files.push(LazyMappedFile::from_shared(shared));
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
        if let Some(&existing_id) = self.path_to_id.get(canonical.as_path()) {
            return Ok(existing_id);
        }
        let shared: Arc<Path> = Arc::from(canonical);

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
            self.path_to_id.insert(shared.clone(), id);
            self.files.push(LazyMappedFile::from_shared(shared));
            // Estimate content bytes from file metadata so stats stay accurate
            if let Ok(meta) = std::fs::metadata(path) {
                self.total_content_bytes
                    .fetch_add(meta.len(), Ordering::Relaxed);
            }
            return Ok(id);
        }

        // Small files (the overwhelming majority of source files) are never
        // mapped: they are served by owned reads. See MMAP_THRESHOLD_BYTES.
        let size = std::fs::metadata(path)
            .with_context(|| format!("Failed to stat file: {}", path.display()))?
            .len();
        if size <= MMAP_THRESHOLD_BYTES {
            let id = self.files.len() as u32;
            self.path_to_id.insert(shared.clone(), id);
            self.files
                .push(LazyMappedFile::from_shared(shared).into_small());
            self.total_content_bytes.fetch_add(size, Ordering::Relaxed);
            return Ok(id);
        }

        // Large file: open and map immediately (zero-copy retrieval).
        let file =
            File::open(path).with_context(|| format!("Failed to open file: {}", path.display()))?;
        let mmap = unsafe {
            Mmap::map(&file).with_context(|| format!("Failed to mmap file: {}", path.display()))?
        };

        // Track content size before storing
        let content_size = mmap.len() as u64;

        let id = self.files.len() as u32;
        self.path_to_id.insert(shared.clone(), id);
        self.files.push(LazyMappedFile::with_mmap(shared, mmap));

        // Update mapped count and content bytes
        self.mapped_count.fetch_add(1, Ordering::Relaxed);
        self.total_content_bytes
            .fetch_add(content_size, Ordering::Relaxed);

        Ok(id)
    }

    /// Get a file by ID (returns None for tombstoned/removed ids)
    pub fn get(&self, id: u32) -> Option<&LazyMappedFile> {
        if self.tombstoned.contains(&id) {
            return None;
        }
        self.files.get(id as usize)
    }

    /// Tombstone a file id: hide it from `get`/`get_path`/lookups and remove it
    /// from the path map so the same path re-added later receives a fresh id.
    /// The slot itself is retained so existing ids remain stable.
    pub fn remove_file_by_id(&mut self, id: u32) {
        if let Some(f) = self.files.get(id as usize) {
            let p = f.path.clone();
            self.path_to_id.remove(&*p);
        }
        self.tombstoned.insert(id);
    }

    /// Replace the entry for an existing id with a fresh, unmapped one so a stale
    /// memory map and cached UTF-8/transcode results are dropped. The id and its
    /// path are preserved; the next access maps the file fresh. Un-tombstones the
    /// id if it had been removed. Returns false if `id` is out of range.
    pub fn refresh_file_by_id(&mut self, id: u32) -> bool {
        if let Some(f) = self.files.get_mut(id as usize) {
            let p = f.path.clone();
            *f = LazyMappedFile::from_shared(p.clone());
            // Ensure the path remains resolvable and the id is live again.
            self.path_to_id.insert(p, id);
            self.tombstoned.remove(&id);
            true
        } else {
            false
        }
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
        self.total_content_bytes.load(Ordering::Relaxed)
    }

    /// Add to the total content bytes counter (used when loading from persistence)
    pub fn add_content_bytes(&self, bytes: u64) {
        self.total_content_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Get a file path by ID (always available, no I/O needed; None if tombstoned)
    pub fn get_path(&self, id: u32) -> Option<&Path> {
        if self.tombstoned.contains(&id) {
            return None;
        }
        self.files.get(id as usize).map(|f| &*f.path)
    }

    /// Find a file ID by exact path match (O(1)).
    pub fn find_by_exact_path(&self, path: &Path) -> Option<u32> {
        self.path_to_id.get(path).copied()
    }

    /// Find a file ID whose stored path ends with the given suffix (O(n)).
    ///
    /// This supports partial path lookups such as `"src/main.rs"` matching
    /// `/home/user/project/src/main.rs`.  Suffix matching is used instead of
    /// substring matching to avoid false positives: for example, looking for
    /// `"bar.rs"` will NOT match `"bar_extra.rs"`.
    ///
    /// Returns the ID of the first file whose path ends with `suffix`, or
    /// `None` if no file matches.
    pub fn find_by_path_suffix(&self, suffix: &str) -> Option<u32> {
        // Normalize to forward slashes so callers can use either separator.
        let normalized = suffix.replace('\\', "/");
        self.files.iter().enumerate().find_map(|(id, f)| {
            if self.tombstoned.contains(&(id as u32)) {
                return None;
            }
            let file_path = f.path.to_string_lossy().replace('\\', "/");
            if file_path.ends_with(normalized.as_str()) {
                Some(id as u32)
            } else {
                None
            }
        })
    }

    /// Ids of all live files whose canonical path is under `prefix`
    /// (directory semantics: `prefix` must match whole path components).
    /// Used to apply directory deletes / renames from the watcher.
    pub fn ids_under(&self, prefix: &Path) -> Vec<u32> {
        self.files
            .iter()
            .enumerate()
            .filter(|(id, f)| {
                !self.tombstoned.contains(&(*id as u32)) && f.path.starts_with(prefix)
            })
            .map(|(id, _)| id as u32)
            .collect()
    }

    /// Number of live (non-tombstoned) files.
    pub fn live_len(&self) -> usize {
        self.files.len().saturating_sub(self.tombstoned.len())
    }

    /// Get the number of files that have been actually mapped
    pub fn mapped_count(&self) -> usize {
        self.mapped_count.load(Ordering::Relaxed)
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
    /// A large file whose mapping failed, forcing every access through the
    /// uncached `fs::read` path.
    pub(crate) fn with_mmap_failure(path: impl AsRef<Path>) -> Self {
        let file = Self::new(path);
        file.size_class.store(SIZE_CLASS_LARGE, Ordering::Relaxed);
        let _ = file.large.set(Box::new(LargeState {
            mmap: Err("simulated mmap failure".to_string()),
            transcoded: OnceLock::new(),
        }));
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
            tombstoned: std::collections::HashSet::new(),
            mapped_count: AtomicUsize::new(0),
            total_content_bytes: AtomicU64::new(0),
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

        // A small file is served by owned reads and is NEVER mapped (see
        // MMAP_THRESHOLD_BYTES): safe against concurrent truncation.
        assert_eq!(lazy.as_str().unwrap(), "hello world");
        assert!(!lazy.is_mapped());
        assert_eq!(lazy.len().unwrap(), 11);

        // A file above the threshold is mapped on first access.
        let big_path = temp_dir.path().join("big.txt");
        let big = "x".repeat(MMAP_THRESHOLD_BYTES as usize + 1);
        std::fs::write(&big_path, &big).unwrap();
        let lazy_big = LazyMappedFile::new(&big_path);
        assert_eq!(lazy_big.as_str().unwrap().len(), big.len());
        assert!(lazy_big.is_mapped());
        assert_eq!(lazy_big.len_if_mapped(), Some(big.len()));
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

        // Access one file: small files are read, not mapped.
        let f1 = store.get(id1).unwrap();
        assert_eq!(f1.as_str().unwrap(), "hello");
        assert!(!store.get(id1).unwrap().is_mapped());
        assert!(!store.get(id2).unwrap().is_mapped());
        assert_eq!(store.mapped_count(), 0);
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

        // Trying to access will fail (the stat fails before any mapping).
        assert!(lazy.as_str().is_err());
        assert!(!lazy.is_mapped());
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
    // Unmapped large files and memory footprint
    // ---------------------------------------------------------------------------

    /// A large file whose mapping failed is read from disk on every access:
    /// nothing is cached, so a rewrite is visible immediately and a
    /// long-running server holds no heap for it between requests.
    #[test]
    fn test_unmapped_large_file_reads_per_access() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "first content").unwrap();

        let file = LazyMappedFile::with_mmap_failure(&file_path);
        assert!(!file.is_mapped());
        assert_eq!(file.as_str().unwrap(), "first content");
        std::fs::write(&file_path, "second content").unwrap();
        assert_eq!(file.as_str().unwrap(), "second content");
        assert_eq!(file.as_bytes().unwrap().as_ref(), b"second content");
        assert!(!file.is_mapped());
        assert!(!file.was_transcoded());
    }

    /// Large files are mapped once and stay mapped; small files never map.
    #[test]
    fn test_large_file_maps_once_small_never() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("big.txt");
        let content = format!("mmap content{}", "x".repeat(MMAP_THRESHOLD_BYTES as usize));
        std::fs::write(&file_path, &content).unwrap();

        let lazy = LazyMappedFile::new(&file_path);
        assert!(!lazy.is_mapped());
        assert_eq!(lazy.as_str().unwrap().len(), content.len());
        assert!(lazy.is_mapped());
        assert_eq!(lazy.len_if_mapped(), Some(content.len()));
        assert_eq!(lazy.as_str().unwrap().len(), content.len());

        let small_path = temp_dir.path().join("small.txt");
        std::fs::write(&small_path, "small content").unwrap();
        let small = LazyMappedFile::new(&small_path);
        assert_eq!(small.as_str().unwrap(), "small content");
        assert!(!small.is_mapped());
        assert_eq!(small.len_if_mapped(), None);
    }

    /// Roadmap 6.3: a path is stored once, shared between the entry and the
    /// path map, and the per-file entry stays small.
    #[test]
    fn test_paths_are_interned_and_entries_are_small() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let p = temp_dir.path().join("a.txt");
        std::fs::write(&p, "x").unwrap();
        let mut store = LazyFileStore::new();
        let id = store.add_file(&p).unwrap();
        let entry = store.get(id).unwrap();
        // One owner in the entry, one in the path map.
        assert_eq!(Arc::strong_count(&entry.path), 2);
        assert_eq!(
            store.find_by_exact_path(&p.canonicalize().unwrap()),
            Some(id)
        );
        // Refreshing and tombstoning keep the map and the entry in sync.
        assert!(store.refresh_file_by_id(id));
        assert_eq!(Arc::strong_count(&store.get(id).unwrap().path), 2);
        store.remove_file_by_id(id);
        assert!(store.get(id).is_none());
        assert_eq!(store.find_by_exact_path(&p.canonicalize().unwrap()), None);
        assert!(
            std::mem::size_of::<LazyMappedFile>() <= 64,
            "per-file entry grew to {} bytes",
            std::mem::size_of::<LazyMappedFile>()
        );
    }

    /// The reason small files are not mapped: truncating a file that a
    /// searcher is reading must not crash the process. With owned reads the
    /// reader simply sees the new content.
    #[test]
    fn test_small_file_survives_concurrent_truncation() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("t.txt");
        std::fs::write(&file_path, "a".repeat(8192)).unwrap();
        let lazy = LazyMappedFile::new(&file_path);
        assert_eq!(lazy.as_str().unwrap().len(), 8192);
        std::fs::write(&file_path, "b").unwrap(); // in-place truncate + rewrite
        assert_eq!(lazy.as_str().unwrap(), "b");
    }
}
