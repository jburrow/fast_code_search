//! Lazy file store: registers file paths and reads their content on demand.
//!
//! Paths are registered at startup (or as files are indexed) and content is
//! read from disk only when a search or the viewer needs it. This keeps
//! startup instant when loading a persisted index, and a million-file index
//! holds no file content between requests.

use anyhow::{Context, Result};
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use tracing::warn;

/// A registered file whose content is read from disk on every access.
///
/// Content is always read into an owned buffer, never through a memory map.
/// Files above 1 MiB used to be mapped, and an editor or tool rewriting one
/// in place (truncate, then write: lockfiles, bundles, generated JSON) while
/// a search read it made the next touch of a page past the new end of file
/// an uncatchable SIGBUS that killed the server. An owned read just sees
/// the file as it is. A mapping also kept serving a replaced file's old
/// inode, and counted toward `vm.max_map_count`.
///
/// The per-file footprint is what a million-file index pays for, so the entry
/// is kept to the path (one `Arc` shared with the store's path map) and an
/// encoding tag.
pub struct LazyMappedFile {
    /// The canonical file path, interned once and shared with the store's
    /// path-to-id map.
    pub path: Arc<Path>,
    /// Detected encoding name for diagnostics (`Some(None)` = natively UTF-8
    /// or not text; unset = not read yet).
    detected_encoding: OnceLock<Option<&'static str>>,
}

impl LazyMappedFile {
    /// Create a new lazy file entry (does not read the file).
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self::from_shared(Arc::from(path.as_ref()))
    }

    /// Create an entry around an interned path.
    pub fn from_shared(path: Arc<Path>) -> Self {
        Self {
            path,
            detected_encoding: OnceLock::new(),
        }
    }

    /// Validate / transcode an owned buffer into text.
    fn owned_to_str(&self, bytes: Vec<u8>) -> Result<Cow<'_, str>> {
        match String::from_utf8(bytes) {
            Ok(s) => Ok(Cow::Owned(crate::utils::strip_utf8_bom_owned(s))),
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

    /// The file's bytes as they are on disk now.
    fn read(&self) -> Result<Vec<u8>> {
        std::fs::read(&self.path).with_context(|| format!("Failed to read {}", self.path.display()))
    }

    /// The content as text, read from disk now. A leading UTF-8 byte order
    /// mark is not part of the text; non-UTF-8 text is transcoded.
    pub fn as_str(&self) -> Result<Cow<'_, str>> {
        self.owned_to_str(self.read()?)
    }

    /// Get the detected encoding name, if the file was transcoded.
    /// Returns None if the file is natively UTF-8 or hasn't been read yet.
    pub fn detected_encoding(&self) -> Option<&'static str> {
        self.detected_encoding.get().copied().flatten()
    }

    /// Returns true if the file was transcoded from a non-UTF-8 encoding
    /// when it was last read.
    pub fn was_transcoded(&self) -> bool {
        self.detected_encoding().is_some()
    }

    /// The content as bytes, read from disk now.
    pub fn as_bytes(&self) -> Result<Cow<'_, [u8]>> {
        Ok(Cow::Owned(self.read()?))
    }

    /// The file's size on disk now.
    pub fn len(&self) -> Result<usize> {
        Ok(std::fs::metadata(&self.path)
            .with_context(|| format!("Failed to stat {}", self.path.display()))?
            .len() as usize)
    }

    /// Check if the file is empty
    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }
}

/// Store of indexed files: paths are registered up front, content is read
/// when it is needed (see [`LazyMappedFile`]). Registering is near instant
/// even with millions of files.
pub struct LazyFileStore {
    /// Files indexed by ID
    files: Vec<LazyMappedFile>,
    /// Map from path to file ID. Keys share their allocation with the
    /// entry's `path`, so each path is stored once.
    path_to_id: HashMap<Arc<Path>, u32>,
    /// Live files by directory, so the watcher's "everything under this
    /// directory" and "the files in this directory" lookups cost the size of
    /// the answer rather than a scan of every indexed path (a scan per
    /// changed directory held the write lock for seconds on large trees).
    dirs: DirIndex,
    /// IDs that have been tombstoned (removed during incremental updates).
    /// Tombstoned slots are hidden from `get`/`get_path`/lookups and never
    /// reused, so existing file IDs stay stable. Typically tiny.
    tombstoned: std::collections::HashSet<u32>,
    /// Statistics: total bytes of content indexed (accumulated as files are added)
    total_content_bytes: AtomicU64,
}

impl LazyFileStore {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            path_to_id: HashMap::new(),
            dirs: DirIndex::default(),
            tombstoned: std::collections::HashSet::new(),
            total_content_bytes: AtomicU64::new(0),
        }
    }

    /// Register a file path and return its ID (does not read the file).
    pub fn register_file(&mut self, path: impl AsRef<Path>) -> u32 {
        let path = path.as_ref();
        if let Some(&existing_id) = self.path_to_id.get(path) {
            return existing_id;
        }
        let shared: Arc<Path> = Arc::from(path);
        let id = self.files.len() as u32;
        self.link(shared.clone(), id);
        self.files.push(LazyMappedFile::from_shared(shared));
        id
    }

    /// Register multiple file paths in bulk (does not read any files)
    ///
    /// Returns a vector of file IDs in the same order as the input paths.
    pub fn register_files_bulk(&mut self, paths: &[PathBuf]) -> Vec<u32> {
        paths.iter().map(|p| self.register_file(p)).collect()
    }

    /// Add a file found during indexing: its canonical path is registered
    /// and its size counted. The content is read when it is needed.
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
        let size = std::fs::metadata(path)
            .with_context(|| format!("Failed to stat file: {}", path.display()))?
            .len();
        let shared: Arc<Path> = Arc::from(canonical);
        let id = self.files.len() as u32;
        self.link(shared.clone(), id);
        self.files.push(LazyMappedFile::from_shared(shared));
        self.total_content_bytes.fetch_add(size, Ordering::Relaxed);
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
        if let Some(f) = self.files.get_mut(id as usize) {
            let p = f.path.clone();
            if self.path_to_id.get(&*p) == Some(&id) {
                self.path_to_id.remove(&*p);
                self.dirs.remove(&p, id);
            }
            *f = LazyMappedFile::from_shared(p);
        }
        self.tombstoned.insert(id);
    }

    /// Replace the entry for an existing id with a fresh one, dropping what it
    /// recorded about the old content (its encoding). The id and its path are
    /// preserved. Un-tombstones the id if it had been removed. Returns false
    /// if `id` is out of range.
    pub fn refresh_file_by_id(&mut self, id: u32) -> bool {
        if let Some(f) = self.files.get_mut(id as usize) {
            let p = f.path.clone();
            *f = LazyMappedFile::from_shared(p.clone());
            // Ensure the path remains resolvable and the id is live again.
            self.link(p, id);
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
    /// `/home/user/project/src/main.rs`. The suffix must be aligned to a
    /// path component: `"bar.rs"` does not match `"bar_extra.rs"` and
    /// `"e/mod.rs"` does not match `".../message/mod.rs"`. An empty suffix
    /// matches nothing.
    ///
    /// Returns the ID of the first file whose path ends with `suffix`, or
    /// `None` if no file matches.
    pub fn find_by_path_suffix(&self, suffix: &str) -> Option<u32> {
        // Normalize to forward slashes so callers can use either separator.
        let normalized = suffix.replace('\\', "/");
        let normalized = normalized.trim_start_matches('/');
        if normalized.is_empty() {
            return None;
        }
        self.files.iter().enumerate().find_map(|(id, f)| {
            if self.tombstoned.contains(&(id as u32)) {
                return None;
            }
            let file_path = f.path.to_string_lossy().replace('\\', "/");
            let aligned = file_path
                .strip_suffix(normalized)
                .is_some_and(|rest| rest.is_empty() || rest.ends_with('/'));
            aligned.then_some(id as u32)
        })
    }

    /// Ids of all live files whose canonical path is under `prefix`
    /// (directory semantics: `prefix` must match whole path components).
    /// Used to apply directory deletes / renames from the watcher.
    pub fn ids_under(&self, prefix: &Path) -> Vec<u32> {
        let mut ids = self.dirs.ids_under(prefix);
        // `prefix` itself may be a file (component-wise `starts_with`).
        ids.extend(self.path_to_id.get(prefix));
        ids
    }

    /// Ids of the live files directly in directory `dir` (not in its
    /// subdirectories).
    pub fn ids_in_dir(&self, dir: &Path) -> Vec<u32> {
        self.dirs.files_in(dir).to_vec()
    }

    /// Point `path` at `id` in the path map and the directory index.
    fn link(&mut self, path: Arc<Path>, id: u32) {
        match self.path_to_id.insert(path.clone(), id) {
            Some(old) if old == id => {}
            Some(old) => {
                self.dirs.remove(&path, old);
                self.dirs.insert(&path, id);
            }
            None => self.dirs.insert(&path, id),
        }
    }

    /// Number of live (non-tombstoned) files.
    pub fn live_len(&self) -> usize {
        self.files.len().saturating_sub(self.tombstoned.len())
    }

    /// Pre-reserve capacity for a known number of files
    pub fn reserve(&mut self, additional: usize) {
        self.files.reserve(additional);
        self.path_to_id.reserve(additional);
    }

    /// Ids of every live (non-tombstoned) file.
    pub fn live_ids(&self) -> impl Iterator<Item = u32> + '_ {
        (0..self.files.len() as u32).filter(|id| !self.tombstoned.contains(id))
    }

    /// Check that the path map, the directory index and the tombstones
    /// agree. Each problem is pushed to `out` as (kind, detail).
    pub fn check_consistency(&self, out: &mut Vec<(&'static str, String)>) {
        for id in self.live_ids() {
            let path = &self.files[id as usize].path;
            match self.path_to_id.get(&**path) {
                Some(&found) if found == id => {}
                found => out.push((
                    "path_map",
                    format!("file {id} ({}) maps back to {found:?}", path.display()),
                )),
            }
            let in_dir = path
                .parent()
                .map(|d| self.dirs.files_in(d).iter().filter(|&&f| f == id).count())
                .unwrap_or(1);
            if in_dir != 1 {
                out.push((
                    "dir_index",
                    format!(
                        "file {id} ({}) listed {in_dir} times in its folder",
                        path.display()
                    ),
                ));
            }
        }
        if self.path_to_id.len() != self.live_len() {
            out.push((
                "path_map",
                format!(
                    "{} paths mapped for {} live files",
                    self.path_to_id.len(),
                    self.live_len()
                ),
            ));
        }
        for (path, &id) in &self.path_to_id {
            if self.tombstoned.contains(&id) {
                out.push((
                    "path_map",
                    format!("{} maps to removed file {id}", path.display()),
                ));
            }
        }
        let dir_files: usize = self.dirs.nodes.values().map(|n| n.files.len()).sum();
        if dir_files != self.live_len() {
            out.push((
                "dir_index",
                format!(
                    "{dir_files} files in the folder index for {} live files",
                    self.live_len()
                ),
            ));
        }
        if let Some(&bad) = self
            .tombstoned
            .iter()
            .find(|&&id| id as usize >= self.files.len())
        {
            out.push((
                "tombstones",
                format!("removed id {bad} was never allocated"),
            ));
        }
    }
}

/// Live file ids by parent directory, plus each directory's subdirectories,
/// so a subtree is walked without visiting unrelated paths.
#[derive(Default)]
struct DirIndex {
    nodes: HashMap<Arc<Path>, DirNode>,
}

#[derive(Default)]
struct DirNode {
    files: Vec<u32>,
    subdirs: Vec<Arc<Path>>,
}

impl DirIndex {
    fn insert(&mut self, path: &Path, id: u32) {
        let Some(parent) = path.parent() else {
            return;
        };
        if let Some(node) = self.nodes.get_mut(parent) {
            node.files.push(id);
            return;
        }
        let parent: Arc<Path> = Arc::from(parent);
        self.nodes.insert(
            parent.clone(),
            DirNode {
                files: vec![id],
                subdirs: Vec::new(),
            },
        );
        // Hook the new directory into its ancestors, creating those that
        // are new as well.
        let mut child = parent;
        while let Some(up) = child.parent() {
            if let Some(node) = self.nodes.get_mut(up) {
                node.subdirs.push(child);
                return;
            }
            let up: Arc<Path> = Arc::from(up);
            self.nodes.insert(
                up.clone(),
                DirNode {
                    files: Vec::new(),
                    subdirs: vec![child],
                },
            );
            child = up;
        }
    }

    fn remove(&mut self, path: &Path, id: u32) {
        let Some(parent) = path.parent() else {
            return;
        };
        let Some(node) = self.nodes.get_mut(parent) else {
            return;
        };
        if let Some(pos) = node.files.iter().position(|&f| f == id) {
            node.files.swap_remove(pos);
        }
        // Drop directories left empty, so renamed-away trees don't linger.
        let mut dir: Arc<Path> = Arc::from(parent);
        loop {
            match self.nodes.get(&*dir) {
                Some(n) if n.files.is_empty() && n.subdirs.is_empty() => {}
                _ => return,
            }
            self.nodes.remove(&*dir);
            let Some(up) = dir.parent() else {
                return;
            };
            let Some(node) = self.nodes.get_mut(up) else {
                return;
            };
            node.subdirs.retain(|d| **d != *dir);
            dir = Arc::from(up);
        }
    }

    fn files_in(&self, dir: &Path) -> &[u32] {
        self.nodes.get(dir).map(|n| &n.files[..]).unwrap_or(&[])
    }

    fn ids_under(&self, dir: &Path) -> Vec<u32> {
        let mut out = Vec::new();
        let Some(root) = self.nodes.get(dir) else {
            return out;
        };
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            out.extend_from_slice(&node.files);
            stack.extend(node.subdirs.iter().filter_map(|d| self.nodes.get(&**d)));
        }
        out
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
        assert_eq!(lazy.as_str().unwrap(), "hello world");
        assert_eq!(lazy.len().unwrap(), 11);
        assert!(!lazy.was_transcoded());
    }

    /// Review 1.10: a suffix lookup only matches whole path components, so
    /// `e/mod.rs` does not resolve to `message/mod.rs`; an empty suffix
    /// resolves to nothing rather than to an arbitrary file.
    #[test]
    fn test_find_by_path_suffix_is_component_aligned() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("message");
        std::fs::create_dir_all(&dir).unwrap();
        let mod_rs = dir.join("mod.rs");
        let bar_extra = dir.join("bar_extra.rs");
        std::fs::write(&mod_rs, "a").unwrap();
        std::fs::write(&bar_extra, "b").unwrap();
        let mut store = LazyFileStore::new();
        let mod_id = store.add_file(&mod_rs).unwrap();
        let bar_id = store.add_file(&bar_extra).unwrap();

        assert_eq!(store.find_by_path_suffix("mod.rs"), Some(mod_id));
        assert_eq!(store.find_by_path_suffix("message/mod.rs"), Some(mod_id));
        assert_eq!(store.find_by_path_suffix("/message/mod.rs"), Some(mod_id));
        assert_eq!(store.find_by_path_suffix("message\\mod.rs"), Some(mod_id));
        assert_eq!(store.find_by_path_suffix("bar_extra.rs"), Some(bar_id));
        let full = mod_rs.canonicalize().unwrap();
        assert_eq!(
            store.find_by_path_suffix(&full.to_string_lossy()),
            Some(mod_id)
        );
        assert_eq!(store.find_by_path_suffix("e/mod.rs"), None);
        assert_eq!(store.find_by_path_suffix("bar.rs"), None);
        assert_eq!(store.find_by_path_suffix(".rs"), None);
        assert_eq!(store.find_by_path_suffix(""), None);
        assert_eq!(store.find_by_path_suffix("/"), None);
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

        // Access one file.
        let f1 = store.get(id1).unwrap();
        assert_eq!(f1.as_str().unwrap(), "hello");
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
    }

    #[test]
    fn test_lazy_file_nonexistent() {
        let lazy = LazyMappedFile::new("/nonexistent/path/to/file.txt");
        assert!(lazy.as_str().is_err());
        assert!(lazy.len().is_err());
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
    // Reads, rewrites and memory footprint
    // ---------------------------------------------------------------------------

    /// Every access reads the file as it is on disk now, whatever its size:
    /// a rewrite is visible immediately and nothing is held between reads.
    #[test]
    fn test_files_are_read_fresh_on_every_access() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("big.txt");
        let first = format!("first {}", "x".repeat(2 * 1024 * 1024));
        std::fs::write(&file_path, &first).unwrap();
        let file = LazyMappedFile::new(&file_path);
        assert_eq!(file.as_str().unwrap().len(), first.len());
        std::fs::write(&file_path, "second content").unwrap();
        assert_eq!(file.as_str().unwrap(), "second content");
        assert_eq!(file.as_bytes().unwrap().as_ref(), b"second content");
        assert_eq!(file.len().unwrap(), 14);
    }

    /// A large file rewritten in place (truncated, then written: what most
    /// tools do to lockfiles, bundles and generated JSON) while searches read
    /// it. Through a memory map the next touch of a page past the new end of
    /// file was a SIGBUS that killed the server (this test process too).
    #[test]
    fn test_large_file_rewritten_in_place_while_read() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("lock.json");
        let content = "\"needle\": 1,\n".repeat(200_000); // ~2.6 MB
        std::fs::write(&file_path, &content).unwrap();
        let file = Arc::new(LazyMappedFile::new(&file_path));
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let readers: Vec<_> = (0..4)
            .map(|_| {
                let (file, stop) = (Arc::clone(&file), Arc::clone(&stop));
                std::thread::spawn(move || {
                    let mut reads = 0usize;
                    while !stop.load(Ordering::Relaxed) {
                        if let Ok(text) = file.as_str() {
                            std::hint::black_box(text.matches("needle").count());
                        }
                        reads += 1;
                    }
                    reads
                })
            })
            .collect();
        let until = std::time::Instant::now() + std::time::Duration::from_millis(500);
        while std::time::Instant::now() < until {
            let mut f = std::fs::File::create(&file_path).unwrap(); // truncates
            f.write_all(&content.as_bytes()[..4096]).unwrap();
            f.flush().unwrap();
            f.write_all(&content.as_bytes()[4096..]).unwrap();
        }
        stop.store(true, Ordering::Relaxed);
        for r in readers {
            assert!(r.join().unwrap() > 0);
        }
        assert_eq!(file.as_str().unwrap().len(), content.len());
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
