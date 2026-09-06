//! Persistent index storage
//!
//! Provides save/load functionality for the trigram index to speed up restarts.
//! Includes file locking for safe concurrent access (exclusive writes, shared reads).

use anyhow::{Context, Result};
use bincode::Options;
use fs2::FileExt;
use roaring::RoaringBitmap;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::trigram::Trigram;
use crate::symbols::extractor::{PackedRef, Symbol};
use crate::utils::normalize_path_for_comparison;

/// One entry of the on-disk trigram directory: where a posting list lives
/// in the bitmap region. Fixed 16 bytes on disk (trigram, pad, offset, len).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirEntry {
    pub trigram: Trigram,
    pub offset: u64,
    pub len: u32,
}

const DIR_ENTRY_LEN: usize = 16;

/// The posting lists of a *loaded* index: the directory plus the mapped
/// bitmap region they point into. Bitmaps are deserialized straight from
/// the mapping by [`PersistedIndex::restore_trigram_index`]; nothing is
/// copied in between. Empty for an index built in memory for saving (the
/// live map is passed to [`PersistedIndex::save`] instead).
#[derive(Default)]
pub struct PersistedTrigramIndex {
    dir: Vec<DirEntry>,
    region: Option<Arc<memmap2::Mmap>>,
    region_start: usize,
}

impl PersistedTrigramIndex {
    /// Number of trigrams in the loaded directory.
    pub fn len(&self) -> usize {
        self.dir.len()
    }

    /// Whether the loaded directory is empty.
    pub fn is_empty(&self) -> bool {
        self.dir.is_empty()
    }

    /// The directory, sorted by trigram.
    pub fn entries(&self) -> &[DirEntry] {
        &self.dir
    }

    fn bitmap_bytes(&self, e: &DirEntry) -> &[u8] {
        let region = self
            .region
            .as_deref()
            .map(|m| &m[self.region_start..])
            .unwrap_or(&[]);
        &region[e.offset as usize..e.offset as usize + e.len as usize]
    }
}

// ---------------------------------------------------------------- CRC32

/// CRC-32 (IEEE 802.3, as in zlib/PNG), table driven.
fn crc32_table() -> &'static [u32; 256] {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, slot) in t.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
            *slot = c;
        }
        t
    })
}

fn crc32_update(mut crc: u32, bytes: &[u8]) -> u32 {
    let t = crc32_table();
    for &b in bytes {
        crc = t[((crc ^ b as u32) & 0xff) as usize] ^ (crc >> 8);
    }
    crc
}

/// CRC-32 of `bytes`.
pub fn crc32(bytes: &[u8]) -> u32 {
    !crc32_update(!0, bytes)
}

/// A writer that counts bytes and folds them into a CRC as they pass.
struct CrcWriter<W: std::io::Write> {
    inner: W,
    crc: u32,
    written: u64,
}

impl<W: std::io::Write> CrcWriter<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            crc: !0,
            written: 0,
        }
    }
    fn crc(&self) -> u32 {
        !self.crc
    }
}

impl<W: std::io::Write> std::io::Write for CrcWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.crc = crc32_update(self.crc, &buf[..n]);
        self.written += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// Serializable representation of file metadata
#[derive(Serialize, Deserialize, Clone)]
pub struct PersistedFileMetadata {
    /// Original file path, stored as raw bytes so a non-UTF-8 name (Latin-1
    /// on Unix) cannot make every checkpoint fail.
    #[serde(with = "path_bytes")]
    pub path: PathBuf,
    /// File modification time in nanoseconds since the Unix epoch (for the
    /// staleness check; whole seconds missed same-size edits within a second)
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
    /// Posting lists of a loaded index (not part of the metadata section).
    #[serde(skip)]
    pub trigram_index: PersistedTrigramIndex,
    /// Per-file symbol caches (parallel to `files`, indexed by position)
    #[serde(default)]
    pub symbols: Vec<Vec<Symbol>>,
    /// Resolved dependency edges as (from_file_idx, to_file_idx) pairs
    /// where indices are positions in the `files` Vec
    #[serde(default)]
    pub dependency_edges: Vec<(u32, u32)>,
    /// Imports that had not resolved when the index was saved, as
    /// (file position, importing path, import strings). Restored so the
    /// edge still appears once the target file is indexed after a reload.
    /// (Format v4 / magic FCSIDX02.)
    pub pending_imports: Vec<(u32, PathBuf, Vec<String>)>,
    /// Interned reference names; `PackedRef::name` indexes this table.
    /// (Format v6 / magic FCSIDX04.)
    pub reference_names: Vec<String>,
    /// Per-file symbol references (parallel to `files`, by position).
    pub references: Vec<Vec<PackedRef>>,
}

/// On-disk layout (all integers little-endian):
///
/// ```text
/// magic[8] version:u32 crc:u32 meta_len:u64 dir_count:u32 bitmaps_len:u64   (36-byte header)
/// META    bincode of `PersistedIndex` (everything but the posting lists)
/// DIR     dir_count x { trigram[3], pad[1], offset:u64, len:u32 }, sorted by trigram
/// BITMAPS the roaring bitmaps back to back; DIR offsets index this region
/// ```
///
/// `crc` covers META + DIR + BITMAPS. The directory is fixed-width and
/// sorted so it can be binary-searched in place, and bitmaps are read
/// directly out of the mapped file: neither saving nor loading materializes
/// a second copy of the posting lists.
const HEADER_LEN: usize = 8 + 4 + 4 + 8 + 4 + 8;

/// Fixed magic header written before the bincode body.
///
/// Validated *before* any bincode decoding so a corrupt, truncated, or
/// foreign file is rejected immediately — never letting a bogus length prefix
/// drive a multi-gigabyte allocation. The trailing digits are a format version;
/// bump them on any incompatible on-disk change.
const INDEX_MAGIC: &[u8; 8] = b"FCSIDX05";

/// Build the bincode options used for *both* save and load.
///
/// Fixint encoding + little-endian is stable across runs; the two sides must use
/// identical options. Compatibility with files written before the magic header
/// existed is intentionally dropped — those fail the magic check and trigger a
/// clean rebuild via `try_load`.
fn bincode_opts() -> impl Options {
    bincode::options().with_fixint_encoding()
}

impl PersistedIndex {
    /// Current persistence format version (bump this when format changes)
    pub const CURRENT_VERSION: u32 = 7;

    /// Create a new persisted index from the current state
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config_fingerprint: String,
        indexed_paths: Vec<String>,
        files: Vec<PersistedFileMetadata>,
        symbols: Vec<Vec<Symbol>>,
        dependency_edges: Vec<(u32, u32)>,
        pending_imports: Vec<(u32, PathBuf, Vec<String>)>,
        reference_names: Vec<String>,
        references: Vec<Vec<PackedRef>>,
    ) -> Result<Self> {
        Ok(Self {
            version: Self::CURRENT_VERSION,
            config_fingerprint,
            indexed_paths,
            files,
            trigram_index: PersistedTrigramIndex::default(),
            symbols,
            dependency_edges,
            pending_imports,
            reference_names,
            references,
        })
    }

    /// Save the index atomically.
    ///
    /// `trigrams` are the live posting lists (borrowed: nothing is copied
    /// before it is written). Writes to a sibling temp file (with an
    /// exclusive lock), flushes and fsyncs it, then renames over the target,
    /// so a reader holding a shared lock never observes a truncated file and
    /// a crash mid-write leaves the previous index intact.
    pub fn save(&self, path: &Path, trigrams: &FxHashMap<Trigram, RoaringBitmap>) -> Result<()> {
        use std::io::{Seek, SeekFrom, Write};

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create index directory: {}", parent.display())
            })?;
        }

        let tmp_path = path.with_extension("bin.tmp");

        {
            let file = std::fs::File::create(&tmp_path).with_context(|| {
                format!("Failed to create temp index file: {}", tmp_path.display())
            })?;

            // Acquire exclusive lock for writing
            file.lock_exclusive().with_context(|| {
                format!(
                    "Failed to acquire exclusive lock on: {}",
                    tmp_path.display()
                )
            })?;

            // Directory: sorted, with each bitmap's offset in the region.
            let mut entries: Vec<(&Trigram, &RoaringBitmap)> = trigrams.iter().collect();
            entries.sort_by_key(|(t, _)| t.as_bytes());
            let mut dir = Vec::with_capacity(entries.len());
            let mut offset = 0u64;
            for (t, bm) in &entries {
                let len = bm.serialized_size();
                dir.push(DirEntry {
                    trigram: **t,
                    offset,
                    len: u32::try_from(len).context("posting list too large")?,
                });
                offset += len as u64;
            }
            let bitmaps_len = offset;

            let err = |what: &str| format!("Failed to write index {what}: {}", tmp_path.display());

            // Header placeholder; the real one is written once the body
            // length and CRC are known.
            let mut writer = std::io::BufWriter::new(&file);
            writer
                .write_all(&[0u8; HEADER_LEN])
                .with_context(|| err("header"))?;

            let mut body = CrcWriter::new(writer);
            bincode_opts()
                .serialize_into(&mut body, self)
                .with_context(|| err("metadata"))?;
            let meta_len = body.written;
            for e in &dir {
                body.write_all(&e.trigram.as_bytes())
                    .with_context(|| err("directory"))?;
                body.write_all(&[0u8]).with_context(|| err("directory"))?;
                body.write_all(&e.offset.to_le_bytes())
                    .with_context(|| err("directory"))?;
                body.write_all(&e.len.to_le_bytes())
                    .with_context(|| err("directory"))?;
            }
            for (_, bm) in &entries {
                bm.serialize_into(&mut body)
                    .with_context(|| err("posting lists"))?;
            }
            let crc = body.crc();
            let mut writer = body.inner;

            // Flush the BufWriter explicitly so I/O errors (e.g. disk full) surface
            // here instead of being silently swallowed when the writer is dropped.
            writer.flush().with_context(|| err("body"))?;
            drop(writer);

            let mut header = Vec::with_capacity(HEADER_LEN);
            header.extend_from_slice(INDEX_MAGIC);
            header.extend_from_slice(&Self::CURRENT_VERSION.to_le_bytes());
            header.extend_from_slice(&crc.to_le_bytes());
            header.extend_from_slice(&meta_len.to_le_bytes());
            header.extend_from_slice(&(dir.len() as u32).to_le_bytes());
            header.extend_from_slice(&bitmaps_len.to_le_bytes());
            let mut f = &file;
            f.seek(SeekFrom::Start(0)).with_context(|| err("header"))?;
            f.write_all(&header).with_context(|| err("header"))?;
            file.sync_all()
                .with_context(|| format!("Failed to fsync index: {}", tmp_path.display()))?;
            // Exclusive lock released as `file` drops at end of scope.
        }

        // Atomic replace. On Windows, rename onto an existing file can fail, so
        // fall back to remove-then-rename.
        if let Err(e) = std::fs::rename(&tmp_path, path) {
            tracing::debug!(error = %e, "Direct rename failed; retrying after removing target");
            let _ = std::fs::remove_file(path);
            std::fs::rename(&tmp_path, path).with_context(|| {
                format!(
                    "Failed to atomically replace index file: {}",
                    path.display()
                )
            })?;
        }

        Ok(())
    }

    /// Load an index from a file with shared lock (allows multiple readers).
    ///
    /// The file is memory-mapped and validated (magic, version, section
    /// bounds, CRC) before anything is decoded. The metadata section is
    /// decoded eagerly; posting lists stay in the mapping until
    /// [`Self::restore_trigram_index`] deserializes them.
    pub fn load(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)
            .with_context(|| format!("Failed to open index file: {}", path.display()))?;

        // Acquire shared lock for reading (multiple readers allowed)
        file.lock_shared()
            .with_context(|| format!("Failed to acquire shared lock on: {}", path.display()))?;

        let rebuilt = |why: String| {
            anyhow::anyhow!("{why} in {}; the index will be rebuilt.", path.display())
        };

        // Header first, from a plain read: a foreign or truncated file is
        // rejected before it is mapped or anything is allocated for it.
        let mut header = [0u8; HEADER_LEN];
        {
            use std::io::Read;
            let mut r = &file;
            if r.read_exact(&mut header).is_err() {
                return Err(rebuilt(
                    "Index file too short (corrupt or old format)".into(),
                ));
            }
        }
        if header[..8] != *INDEX_MAGIC {
            return Err(rebuilt(
                "Index header mismatch (corrupt or old format)".into(),
            ));
        }
        let u32_at = |i: usize| u32::from_le_bytes(header[i..i + 4].try_into().unwrap());
        let u64_at = |i: usize| u64::from_le_bytes(header[i..i + 8].try_into().unwrap());
        let version = u32_at(8);
        if version != Self::CURRENT_VERSION {
            return Err(rebuilt(format!(
                "Index version mismatch: found {version}, expected {}",
                Self::CURRENT_VERSION
            )));
        }
        let crc = u32_at(12);
        let meta_len = u64_at(16) as usize;
        let dir_count = u32_at(24) as usize;
        let bitmaps_len = u64_at(28) as usize;
        let file_len = file.metadata().map(|m| m.len()).unwrap_or(0) as usize;
        let body_len = meta_len
            .checked_add(dir_count.saturating_mul(DIR_ENTRY_LEN))
            .and_then(|n| n.checked_add(bitmaps_len))
            .unwrap_or(usize::MAX);
        if body_len == usize::MAX || HEADER_LEN + body_len != file_len {
            return Err(rebuilt(
                "Index section lengths do not match the file size".into(),
            ));
        }

        // SAFETY: the file is only ever replaced by rename, never truncated
        // or rewritten in place, so the mapping stays valid for its lifetime.
        let mmap = unsafe { memmap2::Mmap::map(&file) }
            .with_context(|| format!("Failed to map index file: {}", path.display()))?;
        let body = &mmap[HEADER_LEN..];
        if crc32(body) != crc {
            return Err(rebuilt("Index checksum mismatch".into()));
        }

        let meta_bytes = &body[..meta_len];
        let mut index: Self = bincode_opts()
            .with_limit(meta_len.max(1) as u64)
            .deserialize(meta_bytes)
            .with_context(|| format!("Failed to deserialize index: {}", path.display()))?;
        if index.version != version {
            return Err(rebuilt(
                "Index version mismatch between header and body".into(),
            ));
        }

        let dir_bytes = &body[meta_len..meta_len + dir_count * DIR_ENTRY_LEN];
        let mut dir = Vec::with_capacity(dir_count);
        let mut prev: Option<[u8; 3]> = None;
        for rec in dir_bytes.as_chunks::<DIR_ENTRY_LEN>().0 {
            let trigram = Trigram::new([rec[0], rec[1], rec[2]]);
            let offset = u64::from_le_bytes(rec[4..12].try_into().unwrap());
            let len = u32::from_le_bytes(rec[12..16].try_into().unwrap());
            let end = offset.saturating_add(len as u64);
            if end > bitmaps_len as u64 || prev.is_some_and(|p| p >= trigram.as_bytes()) {
                return Err(rebuilt("Index trigram directory is corrupt".into()));
            }
            prev = Some(trigram.as_bytes());
            dir.push(DirEntry {
                trigram,
                offset,
                len,
            });
        }
        index.trigram_index = PersistedTrigramIndex {
            dir,
            region: Some(Arc::new(mmap)),
            region_start: HEADER_LEN + meta_len + dir_count * DIR_ENTRY_LEN,
        };

        // Lock is released when `file` drops; the mapping outlives it.
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

    /// Deserialize the posting lists straight out of the mapped bitmap
    /// region (in parallel) into a live map.
    pub fn restore_trigram_index(&self) -> Result<FxHashMap<Trigram, RoaringBitmap>> {
        use rayon::prelude::*;
        let ti = &self.trigram_index;
        let results: Result<Vec<_>> = ti
            .dir
            .par_iter()
            .map(|e| {
                let bitmap = RoaringBitmap::deserialize_from(ti.bitmap_bytes(e))
                    .with_context(|| format!("corrupt posting list for {:?}", e.trigram))?;
                Ok((e.trigram, bitmap))
            })
            .collect();
        Ok(results?.into_iter().collect())
    }
}

/// Get the modification time of a file in seconds since UNIX epoch
pub fn get_mtime(path: &Path) -> Result<u64> {
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("Failed to get metadata for: {}", path.display()))?;
    Ok(mtime_secs_of(&metadata))
}

/// Modification time in nanoseconds since the Unix epoch, as stored in the
/// persisted index (0 if unavailable). Use this on metadata obtained *at read
/// time* so the persisted value describes the content that was actually
/// indexed, not whatever is on disk when the index is saved.
pub fn mtime_secs_of(metadata: &std::fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

/// Serialize a path as raw bytes (lossless on Unix; UTF-8 of the lossy
/// string elsewhere) instead of failing on non-UTF-8 names.
mod path_bytes {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::path::{Path, PathBuf};

    pub fn serialize<S: Serializer>(path: &Path, s: S) -> Result<S::Ok, S::Error> {
        #[cfg(unix)]
        let bytes: &[u8] = {
            use std::os::unix::ffi::OsStrExt;
            path.as_os_str().as_bytes()
        };
        #[cfg(not(unix))]
        let owned = path.to_string_lossy().into_owned();
        #[cfg(not(unix))]
        let bytes: &[u8] = owned.as_bytes();
        serde_bytes_like::Bytes(bytes).serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<PathBuf, D::Error> {
        let bytes: Vec<u8> = Vec::<u8>::deserialize(d)?;
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            Ok(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
        }
        #[cfg(not(unix))]
        {
            Ok(PathBuf::from(String::from_utf8_lossy(&bytes).into_owned()))
        }
    }

    /// Minimal `serialize_bytes` wrapper (avoids a serde_bytes dependency).
    mod serde_bytes_like {
        pub struct Bytes<'a>(pub &'a [u8]);
        impl serde::Serialize for Bytes<'_> {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                // A Vec<u8> round-trips with the same bincode encoding.
                self.0.to_vec().serialize(s)
            }
        }
    }
}

/// Check if a file is stale (modified since indexing)
pub fn is_file_stale(path: &Path, stored_mtime: u64, stored_size: u64) -> bool {
    match std::fs::metadata(path) {
        Ok(metadata) => {
            let current_mtime = mtime_secs_of(&metadata);
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

            // One stat answers both "still there?" and "changed?".
            match std::fs::metadata(&file_meta.path) {
                Err(_) => (idx, FileStatus::Removed),
                Ok(meta)
                    if mtime_secs_of(&meta) != file_meta.mtime || meta.len() != file_meta.size =>
                {
                    (idx, FileStatus::Stale)
                }
                Ok(_) => (idx, FileStatus::Valid),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Roadmap 6.1: files written by earlier formats (older magic) are
    /// rejected up front with a clear message, never decoded.
    #[test]
    fn test_load_rejects_older_magic() {
        let temp = tempfile::TempDir::new().unwrap();
        for old_magic in [b"FCSIDX01", b"FCSIDX02", b"FCSIDX03", b"FCSIDX04"] {
            let p = temp
                .path()
                .join(format!("{}.bin", String::from_utf8_lossy(old_magic)));
            let mut bytes = old_magic.to_vec();
            bytes.extend_from_slice(&[0u8; 64]);
            std::fs::write(&p, bytes).unwrap();
            let err = match PersistedIndex::load(&p) {
                Ok(_) => panic!("old magic must be rejected"),
                Err(e) => e,
            };
            assert!(err.to_string().contains("rebuilt"), "{err}");
        }
    }

    /// Roadmap 6.1: a non-UTF-8 path no longer makes save fail, and
    /// round-trips exactly; mtimes are nanosecond-precise.
    #[cfg(unix)]
    #[test]
    fn test_non_utf8_path_round_trips_and_nanosecond_mtime() {
        use std::os::unix::ffi::OsStringExt;
        let temp = tempfile::TempDir::new().unwrap();
        let weird = PathBuf::from(std::ffi::OsString::from_vec(b"latin1_\xe9.rs".to_vec()));
        let persisted = PersistedIndex::new(
            "fp".to_string(),
            vec![],
            vec![PersistedFileMetadata {
                path: weird.clone(),
                mtime: 1_700_000_000_123_456_789,
                size: 3,
                source_base_path: None,
            }],
            vec![vec![]],
            vec![],
            vec![],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let p = temp.path().join("idx.bin");
        persisted
            .save(&p, &FxHashMap::default())
            .expect("non-UTF-8 path must be saveable");
        let loaded = PersistedIndex::load(&p).unwrap();
        assert_eq!(loaded.files[0].path, weird);
        assert_eq!(loaded.files[0].mtime, 1_700_000_000_123_456_789);
    }
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
        trigram_to_docs.insert(Trigram::new(*b"hel"), bitmap);

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
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("Failed to create persisted index");

        // Save
        persisted
            .save(&index_path, &trigram_to_docs)
            .expect("Failed to save index");

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
            .get(&Trigram::new(*b"hel"))
            .expect("Trigram not found");
        assert!(bitmap.contains(0));
        assert!(bitmap.contains(1));
    }

    fn sample_index() -> (PersistedIndex, FxHashMap<Trigram, RoaringBitmap>) {
        let mut trigram_to_docs: FxHashMap<Trigram, RoaringBitmap> = FxHashMap::default();
        let mut bitmap = RoaringBitmap::new();
        bitmap.insert(0);
        trigram_to_docs.insert(Trigram::new(*b"hel"), bitmap);
        let idx = PersistedIndex::new(
            "fp".to_string(),
            vec!["/test".to_string()],
            vec![PersistedFileMetadata {
                path: PathBuf::from("/test/file.rs"),
                mtime: 1,
                size: 1,
                source_base_path: Some("/test".to_string()),
            }],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("create persisted index");
        (idx, trigram_to_docs)
    }

    fn save_sample(path: &Path) {
        let (idx, map) = sample_index();
        idx.save(path, &map).expect("save sample index");
    }

    /// A fixed index used for the golden-file tests: every field populated,
    /// nothing environment-dependent.
    fn golden_index() -> (PersistedIndex, FxHashMap<Trigram, RoaringBitmap>) {
        use crate::symbols::extractor::SymbolType;
        let mut map: FxHashMap<Trigram, RoaringBitmap> = FxHashMap::default();
        map.insert(Trigram::new(*b"fn "), [0u32, 1].into_iter().collect());
        map.insert(Trigram::new(*b"hel"), [0u32].into_iter().collect());
        map.insert(Trigram::new(*b"wor"), (1u32..40).collect());
        let files = vec![
            PersistedFileMetadata {
                path: PathBuf::from("/fixture/src/lib.rs"),
                mtime: 1_700_000_000_000_000_001,
                size: 42,
                source_base_path: Some("/fixture".to_string()),
            },
            PersistedFileMetadata {
                path: PathBuf::from("/fixture/src/main.rs"),
                mtime: 1_700_000_000_000_000_002,
                size: 7,
                source_base_path: Some("/fixture".to_string()),
            },
        ];
        let symbols = vec![
            vec![Symbol {
                name: "hello".into(),
                symbol_type: SymbolType::Function,
                line: 3,
                column: 3,
                is_definition: true,
            }],
            vec![],
        ];
        let references = vec![
            vec![],
            vec![PackedRef {
                name: 0,
                line: 5,
                column: 4,
            }],
        ];
        let idx = PersistedIndex::new(
            "golden-fingerprint".to_string(),
            vec!["/fixture".to_string()],
            files,
            symbols,
            vec![(1, 0)],
            vec![(
                1,
                PathBuf::from("/fixture/src/main.rs"),
                vec!["missing".into()],
            )],
            vec!["hello".to_string()],
            references,
        )
        .unwrap();
        (idx, map)
    }

    fn golden_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/index-v7.fcsidx")
    }

    /// Roadmap 6.1: the on-disk format is pinned by a committed fixture.
    /// Saving the fixed index must produce the fixture byte for byte; a
    /// format change must bump the version and regenerate the fixture
    /// (`FCS_WRITE_GOLDEN=1 cargo test golden`).
    #[test]
    fn test_golden_fixture_is_bit_identical() {
        let temp = TempDir::new().unwrap();
        let p = temp.path().join("golden.fcsidx");
        let (idx, map) = golden_index();
        idx.save(&p, &map).unwrap();
        let bytes = std::fs::read(&p).unwrap();
        if std::env::var_os("FCS_WRITE_GOLDEN").is_some() {
            std::fs::write(golden_path(), &bytes).unwrap();
        }
        let golden = std::fs::read(golden_path()).expect("fixture tests/fixtures/index-v7.fcsidx");
        assert_eq!(&bytes[..8], INDEX_MAGIC);
        assert!(
            bytes == golden,
            "on-disk format drifted from the fixture (bump the version and regenerate)"
        );
    }

    /// The committed fixture loads with every section intact.
    #[test]
    fn test_golden_fixture_loads() {
        let loaded = PersistedIndex::load(&golden_path()).unwrap();
        assert_eq!(loaded.version, PersistedIndex::CURRENT_VERSION);
        assert_eq!(loaded.config_fingerprint, "golden-fingerprint");
        assert_eq!(loaded.indexed_paths, vec!["/fixture".to_string()]);
        assert_eq!(loaded.files.len(), 2);
        assert_eq!(loaded.files[1].path, PathBuf::from("/fixture/src/main.rs"));
        assert_eq!(loaded.files[0].mtime, 1_700_000_000_000_000_001);
        assert_eq!(loaded.symbols[0][0].name, "hello");
        assert_eq!(loaded.dependency_edges, vec![(1, 0)]);
        assert_eq!(loaded.pending_imports.len(), 1);
        assert_eq!(loaded.reference_names, vec!["hello".to_string()]);
        assert_eq!(loaded.references[1][0].line, 5);
        let dir = loaded.trigram_index.entries();
        assert_eq!(dir.len(), 3);
        assert!(dir
            .windows(2)
            .all(|w| w[0].trigram.as_bytes() < w[1].trigram.as_bytes()));
        let map = loaded.restore_trigram_index().unwrap();
        assert_eq!(map[&Trigram::new(*b"wor")].len(), 39);
        assert_eq!(
            map[&Trigram::new(*b"fn ")].iter().collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    /// A flipped byte anywhere in the body fails the checksum; a wrong
    /// section length fails the bounds check; both are reported as rebuild.
    #[test]
    fn test_load_rejects_corruption() {
        let temp = TempDir::new().unwrap();
        let p = temp.path().join("c.fcsidx");
        let good = std::fs::read(golden_path()).unwrap();
        for pos in [HEADER_LEN + 3, good.len() - 2] {
            let mut bad = good.clone();
            bad[pos] ^= 0x55;
            std::fs::write(&p, &bad).unwrap();
            let err = match PersistedIndex::load(&p) {
                Ok(_) => panic!("corrupt body must be rejected"),
                Err(e) => e.to_string(),
            };
            assert!(err.contains("checksum") && err.contains("rebuilt"), "{err}");
        }
        let mut bad = good.clone();
        bad[24] = bad[24].wrapping_add(1); // dir_count
        std::fs::write(&p, &bad).unwrap();
        let err = match PersistedIndex::load(&p) {
            Ok(_) => panic!("bad section length must be rejected"),
            Err(e) => e.to_string(),
        };
        assert!(err.contains("section lengths"), "{err}");
    }

    #[test]
    fn test_load_rejects_truncated_file() {
        let temp_dir = TempDir::new().expect("temp dir");
        let index_path = temp_dir.path().join("index.bin");
        save_sample(&index_path);

        // Truncate the file to a few bytes — header partial / body missing.
        let full = std::fs::read(&index_path).expect("read");
        std::fs::write(&index_path, &full[..3.min(full.len())]).expect("truncate");

        // Must return Err (not panic / abort), and try_load degrades to None.
        assert!(PersistedIndex::load(&index_path).is_err());
        assert!(PersistedIndex::try_load(&index_path).is_none());
    }

    #[test]
    fn test_load_rejects_random_bytes() {
        let temp_dir = TempDir::new().expect("temp dir");
        let index_path = temp_dir.path().join("index.bin");

        // Random/garbage bytes with no valid magic header.
        std::fs::write(&index_path, vec![0xABu8; 4096]).expect("write garbage");

        assert!(PersistedIndex::load(&index_path).is_err());
        assert!(PersistedIndex::try_load(&index_path).is_none());
    }

    #[test]
    fn test_load_rejects_bogus_length_prefix() {
        let temp_dir = TempDir::new().expect("temp dir");
        let index_path = temp_dir.path().join("index.bin");

        // Valid magic header followed by a giant length prefix and nothing else.
        // The byte limit must turn this into an Err rather than a huge allocation.
        let mut bytes = INDEX_MAGIC.to_vec();
        bytes.extend_from_slice(&u64::MAX.to_le_bytes());
        std::fs::write(&index_path, &bytes).expect("write");

        assert!(PersistedIndex::load(&index_path).is_err());
    }

    #[test]
    fn test_save_is_atomic_leaves_previous_on_failure() {
        let temp_dir = TempDir::new().expect("temp dir");
        let index_path = temp_dir.path().join("index.bin");

        // First save succeeds and is loadable.
        save_sample(&index_path);
        assert!(PersistedIndex::load(&index_path).is_ok());

        // A temp file must not be left behind after a successful save.
        let tmp = index_path.with_extension("bin.tmp");
        assert!(!tmp.exists(), "temp file should be cleaned up by rename");

        // Re-saving over an existing index works (covers Windows rename-over path).
        save_sample(&index_path);
        assert!(PersistedIndex::load(&index_path).is_ok());
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
