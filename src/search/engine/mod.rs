use crate::dependencies::DependencyIndex;
use crate::index::{extract_unique_trigrams_lowercase, LazyFileStore, Trigram, TrigramIndex};
use crate::search::path_filter::PathFilter;
use crate::search::query_syntax::{ParsedQuery, SearchOptions};
use crate::search::ranking::{FileScoreWeights, RankingWeights};
use crate::search::regex_search::RegexAnalysis;
use crate::symbols::extractor::{PackedRef, SymbolRef};
use crate::symbols::{Symbol, SymbolExtractor, SymbolType};
use anyhow::Result;
use memchr::memmem;
use rayon::prelude::*;
use regex::Regex;
use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};
use tracing::warn;

mod persist;
mod progress;
mod query;
mod text;

pub use progress::*;
use text::*;

#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub file_id: u32,
    pub file_path: String,
    pub line_number: usize,
    pub content: String,
    /// Start position of the match within the (possibly truncated) content
    pub match_start: usize,
    /// End position of the match within the (possibly truncated) content
    pub match_end: usize,
    /// Whether the content was truncated from the original line
    pub content_truncated: bool,
    /// Byte offset of the match start within the FULL (untruncated) line.
    /// For filename hits (`line_number == 0`) this is within the display path.
    pub line_match_start: usize,
    /// Byte offset of the match end within the full line.
    pub line_match_end: usize,
    /// 0-based character (Unicode scalar) column of the match start within
    /// the full line — what editors want for cursor placement.
    pub match_column: usize,
    pub score: f64,
    pub is_symbol: bool,
    /// The match is a symbol *reference* (call site, type mention) found by
    /// [`SearchEngine::search_references`], not a text or definition hit.
    pub is_reference: bool,
    pub dependency_count: u32,
}

/// Information about how a search was ranked
#[derive(Debug, Clone)]
pub struct SearchRankingInfo {
    /// The ranking mode that was used
    pub mode: RankMode,
    /// Total number of candidate documents from trigram index
    pub total_candidates: usize,
    /// Number of candidates actually searched (read from disk)
    pub candidates_searched: usize,
    /// True when the match budget or deadline stopped the search early, so
    /// the result set is a *sample* of the best matches among those seen.
    pub truncated_by_budget: bool,
    /// Total matches found across all searched candidates (before paging),
    /// or `None` when the search was truncated by the budget.
    pub total_matches: Option<usize>,
}

impl SearchRankingInfo {
    fn empty(mode: RankMode) -> Self {
        Self {
            mode,
            total_candidates: 0,
            candidates_searched: 0,
            truncated_by_budget: false,
            total_matches: Some(0),
        }
    }
}

/// Limits that bound the work a single query may do.
///
/// `match_budget` caps the number of matches materialized across all
/// documents (each costs a `String`); once it is exhausted no further
/// documents are opened. `deadline` stops the scan at a wall-clock time.
/// Both are reported through [`SearchRankingInfo::truncated_by_budget`].
#[derive(Debug, Clone, Copy)]
pub struct SearchLimits {
    /// Page size.
    pub max_results: usize,
    /// Results to skip (for paging); the ordering is deterministic.
    pub offset: usize,
    /// Maximum matches to materialize before stopping.
    pub match_budget: usize,
    /// Wall-clock deadline for the scan.
    pub deadline: Option<std::time::Instant>,
}

impl SearchLimits {
    /// Default budget multiplier: enough headroom above the requested page
    /// (plus offset) that ranking still sees a broad sample.
    const BUDGET_MULTIPLIER: usize = 8;
    /// Never budget fewer matches than this, so tiny pages still rank well.
    const MIN_BUDGET: usize = 512;
    /// Largest offset a client may page to. Deeper pages would need a budget
    /// (and a scan) proportional to the offset; past this point a client
    /// should narrow the query instead.
    pub const MAX_OFFSET: usize = 10_000;
    /// Ceiling on the budget derived from page size and offset. An explicit
    /// [`Self::with_match_budget`] may still exceed it (benchmarks, tests).
    pub const MAX_DERIVED_BUDGET: usize = 100_000;

    /// Limits for a page of `max_results` with the default budget.
    pub fn new(max_results: usize) -> Self {
        let max_results = max_results.max(1);
        Self {
            max_results,
            offset: 0,
            match_budget: Self::derived_budget(0, max_results),
            deadline: None,
        }
    }

    /// Budget for a page of `max_results` starting at `offset`: enough
    /// headroom for ranking, bounded above so a deep offset cannot turn one
    /// request into an unbounded scan.
    fn derived_budget(offset: usize, max_results: usize) -> usize {
        offset
            .saturating_add(max_results)
            .saturating_mul(Self::BUDGET_MULTIPLIER)
            .clamp(Self::MIN_BUDGET, Self::MAX_DERIVED_BUDGET)
    }

    /// Skip the first `offset` results (the budget grows to cover the page,
    /// up to [`Self::MAX_DERIVED_BUDGET`]). Offsets beyond
    /// [`Self::MAX_OFFSET`] are clamped; callers that want to reject them
    /// should validate before building the limits.
    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = offset.min(Self::MAX_OFFSET);
        self.match_budget = self
            .match_budget
            .max(Self::derived_budget(self.offset, self.max_results));
        self
    }

    /// Override the match budget (`usize::MAX` = unbounded, the old behaviour).
    pub fn with_match_budget(mut self, budget: usize) -> Self {
        self.match_budget = budget.max(1);
        self
    }

    /// Stop scanning at `deadline`.
    pub fn with_deadline(mut self, deadline: std::time::Instant) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// Stop scanning after `dur` from now.
    pub fn with_timeout(self, dur: std::time::Duration) -> Self {
        self.with_deadline(std::time::Instant::now() + dur)
    }
}

/// Shared per-query state consulted by every worker: remaining match budget
/// and deadline. Cheap enough to check per document and per match.
pub struct QueryRun {
    remaining: std::sync::atomic::AtomicUsize,
    deadline: Option<std::time::Instant>,
    truncated: std::sync::atomic::AtomicBool,
}

impl QueryRun {
    fn new(limits: &SearchLimits) -> Self {
        Self {
            remaining: std::sync::atomic::AtomicUsize::new(limits.match_budget),
            deadline: limits.deadline,
            truncated: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// True once the budget is spent or the deadline passed; callers must
    /// not open further documents.
    pub fn exhausted(&self) -> bool {
        use std::sync::atomic::Ordering::Relaxed;
        if self.remaining.load(Relaxed) == 0 {
            self.truncated.store(true, Relaxed);
            return true;
        }
        if let Some(d) = self.deadline {
            if std::time::Instant::now() >= d {
                self.truncated.store(true, Relaxed);
                return true;
            }
        }
        false
    }

    /// Reserve one unit of budget for a match. Returns `false` (and marks
    /// the run truncated) when none is left; the caller stops scanning.
    pub fn take_match(&self) -> bool {
        use std::sync::atomic::Ordering::Relaxed;
        let mut cur = self.remaining.load(Relaxed);
        loop {
            if cur == 0 {
                self.truncated.store(true, Relaxed);
                return false;
            }
            match self
                .remaining
                .compare_exchange_weak(cur, cur - 1, Relaxed, Relaxed)
            {
                Ok(_) => return true,
                Err(actual) => cur = actual,
            }
        }
    }

    fn was_truncated(&self) -> bool {
        self.truncated.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// Tiny LRU of compiled regex analyses keyed by pattern.
struct RegexCache {
    capacity: usize,
    order: std::collections::VecDeque<String>,
    entries: FxHashMap<String, std::sync::Arc<RegexAnalysis>>,
}

impl RegexCache {
    fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            order: std::collections::VecDeque::new(),
            entries: FxHashMap::default(),
        }
    }

    fn get(&mut self, pattern: &str) -> Option<std::sync::Arc<RegexAnalysis>> {
        let hit = self.entries.get(pattern)?.clone();
        if let Some(pos) = self.order.iter().position(|p| p == pattern) {
            let key = self.order.remove(pos).expect("position is valid");
            self.order.push_back(key);
        }
        Some(hit)
    }

    fn put(&mut self, pattern: &str, analysis: std::sync::Arc<RegexAnalysis>) {
        if self.entries.insert(pattern.to_string(), analysis).is_none() {
            self.order.push_back(pattern.to_string());
        }
        while self.order.len() > self.capacity {
            if let Some(evicted) = self.order.pop_front() {
                self.entries.remove(&evicted);
            }
        }
    }
}

/// Result of attempting to resolve imports for a single file.
/// Used internally by resolve_imports_incremental.
struct ImportResolutionResult {
    /// ID of the file that has the imports
    file_id: u32,
    /// Path to the file
    file_path: PathBuf,
    /// Import paths that could not be resolved (target not indexed yet)
    unresolved_paths: Vec<String>,
    /// Successfully resolved edges (from_id, to_id)
    resolved_edges: Vec<(u32, u32)>,
}

/// Intermediate result from phase 1 (parallel, pure-Rust, no FFI).
/// Holds file content and trigrams. Tree-sitter is NOT called here.
pub struct PartialIndexedFile {
    pub path: PathBuf,
    pub trigrams: FxHashSet<Trigram>,
    pub filename_stem: String,
    /// Raw file content kept for phase 2 symbol extraction
    pub content: String,
    /// Whether the content passed the structural tree-sitter safety check.
    /// `false` means the file is indexed for text search only (no symbols /
    /// imports) because a >100 KB line or extreme nesting could crash or
    /// stall the C parsers.
    pub tree_sitter_safe: bool,
    /// Modification time (seconds since epoch) of the file *as read*.
    pub mtime: u64,
    /// Size in bytes of the file *as read*.
    pub size: u64,
}

impl PartialIndexedFile {
    /// Default maximum file size to process (10MB) when no explicit limit is given.
    pub const DEFAULT_MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

    /// Phase 1: pure-Rust work only — safe to run in parallel across rayon threads.
    /// Does NOT call tree-sitter (C FFI) to avoid concurrent heap corruption.
    ///
    /// When `transcode_non_utf8` is true, files in non-UTF-8 encodings (Latin-1,
    /// Shift-JIS, UTF-16, etc.) are automatically transcoded. When false, only
    /// UTF-8 files are accepted.
    ///
    /// `max_file_size` is the configured byte cap (0 means "use the default cap");
    /// files larger than it are skipped so a configured limit above the old
    /// hardcoded 10 MB is actually honored instead of being silently capped.
    /// Returns `Some((file, transcoded))` where `transcoded` is `true` when
    /// the file was converted from a non-UTF-8 encoding via `transcode_to_utf8`.
    pub fn process(
        path: &Path,
        transcode_non_utf8: bool,
        max_file_size: u64,
    ) -> Option<(Self, bool)> {
        let max_size = if max_file_size == 0 {
            Self::DEFAULT_MAX_FILE_SIZE
        } else {
            max_file_size
        };
        let metadata = std::fs::metadata(path).ok()?;
        if metadata.len() > max_size {
            return None;
        }
        // Capture what we are about to read so persistence records the indexed
        // content's identity, not the on-disk state at save time.
        let size = metadata.len();
        let mtime = crate::index::persistence::mtime_secs_of(&metadata);

        let raw_bytes = std::fs::read(path).ok()?;
        // Attempt zero-copy consume: String::from_utf8 reuses the Vec allocation when valid.
        let (content, transcoded) = match String::from_utf8(raw_bytes) {
            Ok(s) => (s, false), // UTF-8 fast path — zero-copy consume
            Err(e) => {
                if !transcode_non_utf8 {
                    return None; // Transcoding disabled
                }
                let raw_bytes = e.into_bytes(); // Recover the original bytes
                match crate::utils::transcode_to_utf8(&raw_bytes) {
                    Ok(Some(result)) => {
                        tracing::debug!(
                            path = %path.display(),
                            encoding = result.encoding_name,
                            "Transcoded non-UTF-8 file for indexing"
                        );
                        (result.content, true)
                    }
                    _ => return None, // Binary or unrecognizable
                }
            }
        };

        // Binary content masquerading as UTF-8 is not indexed at all.
        if crate::utils::is_binary_content(&content) {
            tracing::debug!(
                path = %path.display(),
                "Skipping binary-looking file during indexing"
            );
            return None;
        }

        // Structural checks only gate tree-sitter: a file with a >100 KB line or
        // extreme nesting (large JSON fixtures, generated code) stays fully
        // text-searchable but gets no symbol / import extraction.
        let tree_sitter_safe = match crate::utils::tree_sitter_safety_check(&content) {
            None => true,
            Some(reason) => {
                tracing::debug!(
                    path = %path.display(),
                    reason = reason,
                    "Indexing file for text search only (skipping symbol extraction)"
                );
                false
            }
        };

        let filename_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_else(|| {
                warn!(
                    "Failed to extract filename stem from path: {}",
                    path.display()
                );
                ""
            })
            .to_string();

        // Extract trigrams from filename stem and content separately to avoid
        // an intermediate concatenated-string allocation.
        // Filenames are searchable because stem trigrams are added to the same set.
        // The old approach prepended the stem with triple newlines to avoid spurious
        // cross-boundary trigrams; separating extraction is equivalent — no boundary
        // trigrams are generated at all, which is strictly better.
        // Filename stem + content, lowercased the way queries are (no
        // whole-buffer lowercase copy for ASCII content).
        let mut trigrams = extract_unique_trigrams_lowercase(&filename_stem);
        trigrams.extend(extract_unique_trigrams_lowercase(&content));

        Some((
            PartialIndexedFile {
                path: path.to_path_buf(),
                trigrams,
                filename_stem,
                content,
                tree_sitter_safe,
                mtime,
                size,
            },
            transcoded,
        ))
    }
}

/// Pre-processed file data ready to be merged into the engine.
/// Built from a PartialIndexedFile by adding symbols/imports (tree-sitter).
pub struct PreIndexedFile {
    /// Path to the file
    pub path: PathBuf,
    /// Unique trigrams extracted from the content
    pub trigrams: FxHashSet<Trigram>,
    /// Extracted symbols
    pub symbols: Vec<Symbol>,
    /// Extracted import paths
    pub imports: Vec<String>,
    /// References (call sites, type mentions) reported by the tags query
    pub references: Vec<SymbolRef>,
    /// Modification time (seconds since epoch) of the content that was indexed
    pub mtime: u64,
    /// Size in bytes of the content that was indexed
    pub size: u64,
}

impl PreIndexedFile {
    /// Phase 2: run tree-sitter symbol/import extraction on an already-processed partial.
    ///
    /// Uses `extract_all` to parse the source a single time for both symbols and imports.
    /// Safe to call from multiple rayon threads simultaneously — tree-sitter `Parser` is
    /// `Send + Sync` in tree-sitter v0.26+, and each call creates an independent `Parser`
    /// instance with no shared mutable state.
    ///
    /// # Parameters
    ///
    /// - `partial`: The partially-processed file from phase 1 (content + trigrams).
    /// - `enable_symbols`: When `false`, tree-sitter symbol/import extraction is skipped
    ///   entirely to reduce CPU and memory usage. The filename symbol is always added
    ///   regardless, since it is used for path-based search scoring without tree-sitter.
    pub fn from_partial(partial: PartialIndexedFile, enable_symbols: bool) -> Self {
        let (mut symbols, imports, references) = if enable_symbols && partial.tree_sitter_safe {
            let extractor = SymbolExtractor::new_for_source(&partial.path, Some(&partial.content));

            // Extract symbols, imports and references in a single parse with
            // panic protection: tree-sitter can stack overflow on deeply
            // nested or malformed files.
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                extractor
                    .extract_all_with_refs(&partial.content)
                    .unwrap_or_default()
            }))
            .unwrap_or_else(|_| {
                warn!(
                    "Symbol/import extraction panicked for file '{}'. This typically occurs with deeply nested or malformed syntax. Continuing without symbols.",
                    partial.path.display()
                );
                (Vec::new(), Vec::new(), Vec::new())
            })
        } else {
            (Vec::new(), Vec::new(), Vec::new())
        };

        // Add filename as a FileName symbol (line 0, gets symbol scoring boost)
        if !partial.filename_stem.is_empty() {
            symbols.push(Symbol {
                name: partial.filename_stem.clone(),
                symbol_type: SymbolType::FileName,
                line: 0,
                column: 0,
                is_definition: true,
            });
        }

        PreIndexedFile {
            path: partial.path,
            trigrams: partial.trigrams,
            symbols,
            imports: imports.into_iter().map(|i| i.path).collect(),
            references,
            mtime: partial.mtime,
            size: partial.size,
        }
    }
}

/// Ranking mode for search queries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RankMode {
    /// Automatically choose based on candidate count (fast if >5000 candidates)
    #[default]
    Auto,
    /// Fast file-level ranking (no file reads for ranking, reads only top candidates)
    Fast,
    /// Full line-level ranking (reads all candidate files)
    Full,
}

impl RankMode {
    /// Parse from string (for API parameter)
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "fast" => RankMode::Fast,
            "full" => RankMode::Full,
            _ => RankMode::Auto,
        }
    }
}

/// Pre-computed file metadata for fast ranking without file reads.
/// Populated once during finalize(), used during search.
#[derive(Debug, Clone, Default)]
pub struct FileMetadata {
    /// Number of symbol definitions in this file
    pub symbol_count: u16,
    /// Whether file is in src/ or lib/ directory
    pub is_src_lib: bool,
    /// Pre-computed base score for ranking
    pub base_score: f32,
    /// Lowercase filename stem for efficient query matching (avoids per-query allocation)
    pub lowercase_stem: String,
    /// Root-relative display path, precomputed so path filters and result
    /// construction never allocate per candidate.
    pub display_path: String,
}

impl FileMetadata {
    /// Compute metadata for a file at index time
    fn compute(
        path: &Path,
        display_path: String,
        symbol_count: usize,
        dependency_count: u32,
    ) -> Self {
        let w = &FileScoreWeights::DEFAULT;
        let mut base_score: f32 = w.base;

        let path_str = path.to_string_lossy();
        let path_lower = path_str.to_lowercase();

        // Check if in source directories
        let is_src_lib = path_lower.contains("/src/")
            || path_lower.contains("\\src\\")
            || path_lower.contains("/lib/")
            || path_lower.contains("\\lib\\");

        if is_src_lib {
            base_score += w.src_lib_dir;
        }

        // Boost for high-value extensions
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext.to_lowercase().as_str() {
                "rs" | "py" | "ts" | "js" | "go" | "java" | "c" | "cpp" | "h" => {
                    base_score += w.code_extension
                }
                "md" | "txt" | "json" | "toml" | "yaml" | "yml" => base_score += w.doc_extension,
                _ => {}
            }
        }

        // Boost for files with symbols (more likely to be important code).
        // (File-level scores are additive log2 terms; line-level scores use the
        // multiplicative `RankingWeights::dependency_boost`. The two are on
        // different scales by design: this one only orders which files to
        // open in fast mode, it never appears in a result's score.)
        if symbol_count > 0 {
            base_score += (symbol_count as f32).log2().min(w.symbol_log2_cap);
        }

        // Boost for dependency count (files imported by others are important)
        if dependency_count > 0 {
            base_score += (dependency_count as f32).log2().min(w.dependency_log2_cap);
        }

        // Penalty for test/example directories
        if path_lower.contains("/test")
            || path_lower.contains("\\test")
            || path_lower.contains("/example")
            || path_lower.contains("\\example")
        {
            base_score *= w.test_example_penalty;
        }

        // Pre-compute lowercase stem for efficient filename matching during search
        let lowercase_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        FileMetadata {
            symbol_count: symbol_count.min(u16::MAX as usize) as u16,
            is_src_lib,
            base_score,
            lowercase_stem,
            display_path,
        }
    }

    /// Compute ranking score for a specific query
    /// This is called during search but doesn't require reading file content
    #[inline]
    fn query_score(&self, query_lower: &str) -> f32 {
        let mut score = self.base_score;

        // Big boost if query matches filename (using pre-computed lowercase stem)
        if !query_lower.is_empty() && self.lowercase_stem.contains(query_lower) {
            score *= FileScoreWeights::DEFAULT.filename_match;
        }

        score
    }
}

pub struct SearchEngine {
    pub file_store: LazyFileStore,
    pub trigram_index: TrigramIndex,
    pub dependency_index: DependencyIndex,
    symbol_cache: Vec<Vec<Symbol>>,
    /// Per-file symbol references (call sites, type mentions), names
    /// interned through `ref_names` / `ref_name_ids`. Parallel to
    /// `symbol_cache`; a slot is cleared when its file is removed.
    reference_cache: Vec<Vec<PackedRef>>,
    /// Interned reference names, indexed by `PackedRef::name`.
    ref_names: Vec<String>,
    ref_name_ids: FxHashMap<String, u32>,
    /// (mtime secs, size) of each file's content *as indexed*, by file id.
    /// `(0, 0)` means unknown (fall back to a stat at save time).
    indexed_meta: Vec<(u64, u64)>,
    /// Pre-computed file metadata for fast ranking
    file_metadata: Vec<FileMetadata>,
    /// Pending imports to resolve after all files are indexed
    pending_imports: Vec<(u32, std::path::PathBuf, Vec<String>)>,
    /// Imports that could not be resolved yet (target not indexed, or an
    /// external package). Retried only when a file whose stem matches one of
    /// the import's path segments is indexed, instead of on every batch.
    /// Entries are `None` once resolved; `waiting_keys` maps a lowercase
    /// segment to indices into this Vec.
    waiting_imports: Vec<Option<(u32, std::path::PathBuf, String)>>,
    waiting_keys: FxHashMap<String, Vec<usize>>,
    /// Lowercased stems of files added since the last incremental resolution.
    recently_added_stems: Vec<String>,
    /// Recently compiled regexes (search-as-you-type resends the same pattern).
    regex_cache: std::sync::Mutex<RegexCache>,
    /// Bumped on every mutation (batch merge, update, removal, load) so
    /// derived views (diagnostics breakdowns) can be cached per generation.
    generation: u64,
    /// Whether tree-sitter symbol extraction is enabled (default: true)
    pub enable_symbols: bool,
    /// Whether non-UTF-8 files are transcoded during single-file indexing (default: true)
    pub transcode_non_utf8: bool,
    /// Maximum file size (bytes) accepted during single-file indexing (default 10MB)
    pub max_file_size: u64,
    /// Canonical root paths used to produce root-relative display paths
    root_paths: Vec<PathBuf>,
}

impl SearchEngine {
    pub fn new() -> Self {
        Self {
            file_store: LazyFileStore::new(),
            trigram_index: TrigramIndex::new(),
            dependency_index: DependencyIndex::new(),
            symbol_cache: Vec::new(),
            reference_cache: Vec::new(),
            ref_names: Vec::new(),
            ref_name_ids: FxHashMap::default(),
            indexed_meta: Vec::new(),
            file_metadata: Vec::new(),
            pending_imports: Vec::new(),
            waiting_imports: Vec::new(),
            waiting_keys: FxHashMap::default(),
            recently_added_stems: Vec::new(),
            regex_cache: std::sync::Mutex::new(RegexCache::new(64)),
            generation: 0,
            enable_symbols: true,
            transcode_non_utf8: true,
            max_file_size: PartialIndexedFile::DEFAULT_MAX_FILE_SIZE,
            root_paths: Vec::new(),
        }
    }

    /// Register an indexed root path so that `make_display_path` can produce
    /// workspace-relative paths (e.g. `project/src/main.rs` instead of
    /// `/workspace/project/src/main.rs`).
    ///
    /// The path is canonicalized on registration so that it matches the
    /// canonical paths stored in `LazyMappedFile.path`.
    pub fn add_root_path(&mut self, path: impl AsRef<Path>) {
        let canonical = path
            .as_ref()
            .canonicalize()
            .unwrap_or_else(|_| path.as_ref().to_path_buf());
        if !self.root_paths.contains(&canonical) {
            self.root_paths.push(canonical);
        }
    }

    /// Convert a (canonical) stored file path into a workspace-relative display
    /// string using forward slashes.
    ///
    /// The display path always includes the root folder's own name as the first
    /// component — mirroring the VSCode workspace experience where opening
    /// `/workspace/project` shows files as `project/src/main.rs`.  This makes
    /// results unambiguous when multiple root paths share the same internal
    /// layout (e.g. two repos both containing a `src/main.rs`).
    ///
    /// If `path` is under one of the registered root paths the absolute prefix
    /// *above* the root folder is stripped, preserving the root folder name.
    /// E.g. root `/workspace/project`, file `/workspace/project/src/main.rs`
    /// → `project/src/main.rs`.  When no root matches, the full path is
    /// returned with backslashes normalised to forward slashes so that the
    /// output is always OS-agnostic.
    pub fn make_display_path(&self, path: &Path) -> String {
        for root in &self.root_paths {
            if let Ok(relative) = path.strip_prefix(root) {
                // Include the root folder name so the display path is
                // workspace-relative (e.g. `project/src/main.rs`).
                if let Some(root_name) = root.file_name() {
                    let root_name_str = root_name.to_string_lossy();
                    let relative_str = relative.to_string_lossy().replace('\\', "/");
                    if relative_str.is_empty() {
                        // Path IS the root directory itself
                        return root_name_str.to_string();
                    }
                    return format!("{}/{}", root_name_str, relative_str);
                }
                // Root has no file_name (e.g. filesystem root "/") — fall back
                // to the bare relative path.
                return relative.to_string_lossy().replace('\\', "/");
            }
        }
        // Fallback: return the full path with forward slashes
        path.to_string_lossy().replace('\\', "/")
    }

    /// Index a file.
    ///
    /// Reads the file content into an **owned buffer** (via `PartialIndexedFile::process`)
    /// rather than extracting through a live memory map. Holding an mmap across trigram /
    /// symbol extraction is unsafe: if the file is truncated on disk concurrently (editors
    /// and build tools routinely truncate-and-rewrite), touching mapped pages past the new
    /// EOF raises an uncatchable SIGBUS / access violation. Reading owned bytes up front
    /// avoids that entirely.
    ///
    /// The content safety check runs *before* the file is registered in any index, so an
    /// unsafe/binary/oversized file never leaks a permanent file id with no trigrams.
    pub fn index_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();

        // Phase 1: owned read + safety check + trigram extraction (no live mmap).
        // `process` returns None for binary/unsafe/oversized files — skip silently.
        let (partial, _transcoded) =
            match PartialIndexedFile::process(path, self.transcode_non_utf8, self.max_file_size) {
                Some(p) => p,
                None => return Ok(()),
            };

        // Phase 2: tree-sitter symbol/import extraction with panic protection
        // (from_partial wraps tree-sitter in catch_unwind).
        let pre = PreIndexedFile::from_partial(partial, self.enable_symbols);

        // Merge into the engine: registers the file and adds trigrams/symbols/imports.
        self.index_batch(vec![pre]);

        Ok(())
    }

    /// Index a batch of pre-processed files.
    /// This is the merge step after parallel processing - only this needs the write lock.
    /// Returns the number of files successfully indexed.
    pub fn index_batch(&mut self, batch: Vec<PreIndexedFile>) -> usize {
        let mut count = 0;
        self.generation += 1;

        for pre_indexed in batch {
            // Add file to store - this also memory-maps it
            let file_id = match self.file_store.add_file(&pre_indexed.path) {
                Ok(id) => id,
                Err(_) => continue,
            };
            self.set_indexed_meta(file_id, pre_indexed.mtime, pre_indexed.size);
            self.note_added_file(&pre_indexed.path);

            // Register file in dependency index under the store's canonical
            // path (no realpath walk per file).
            if let Some(canonical) = self.file_store.get_path(file_id).map(Path::to_path_buf) {
                self.dependency_index
                    .register_canonical_file(file_id, canonical);
            }

            // Add trigrams to index (using pre-computed trigrams)
            self.trigram_index
                .add_document_trigrams(file_id, pre_indexed.trigrams);

            // Store symbols
            while self.symbol_cache.len() <= file_id as usize {
                self.symbol_cache.push(Vec::new());
            }
            self.symbol_cache[file_id as usize] = pre_indexed.symbols;
            self.store_references(file_id, pre_indexed.references);

            // Store imports for later resolution
            if !pre_indexed.imports.is_empty() {
                self.pending_imports
                    .push((file_id, pre_indexed.path, pre_indexed.imports));
            }

            count += 1;
        }

        count
    }

    /// Resolve all pending imports after indexing is complete.
    /// Call this after all files have been indexed to build the dependency graph.
    ///
    /// Uses two-phase parallel resolution:
    /// 1. Parallel path resolution using rayon (CPU-bound, thread-safe)
    /// 2. Sequential graph insertion (requires &mut self)
    pub fn resolve_imports(&mut self) {
        // Everything queued for this batch, plus one final attempt at every
        // parked import (the last file may have landed without a matching
        // stem, e.g. a `.d.ts`). Still-unresolved imports stay parked so they
        // are persisted and retried after a later incremental add.
        self.resolve_pending_now();
        let all: Vec<usize> = (0..self.waiting_imports.len())
            .filter(|&i| self.waiting_imports[i].is_some())
            .collect();
        self.retry_waiting(all);
        self.recently_added_stems.clear();
        self.compact_waiting_imports();
    }

    /// Resolve the imports of files added since the last call; park those
    /// that do not resolve. Returns the number of edges added.
    fn resolve_pending_now(&mut self) -> usize {
        if self.pending_imports.is_empty() {
            return 0;
        }
        let pending = std::mem::take(&mut self.pending_imports);
        let results: Vec<ImportResolutionResult> = pending
            .into_par_iter()
            .map(|(file_id, file_path, import_paths)| {
                let mut resolved_edges = Vec::new();
                let mut unresolved_paths = Vec::new();
                for import_path in import_paths {
                    if let Some(resolved) = self
                        .dependency_index
                        .resolve_import_path(&file_path, &import_path)
                    {
                        if let Some(to_id) = self.dependency_index.get_file_id(&resolved) {
                            resolved_edges.push((file_id, to_id));
                            continue;
                        }
                    }
                    unresolved_paths.push(import_path);
                }
                ImportResolutionResult {
                    file_id,
                    file_path,
                    unresolved_paths,
                    resolved_edges,
                }
            })
            .collect();

        let mut all_edges = Vec::new();
        for result in results {
            all_edges.extend(result.resolved_edges);
            for import in result.unresolved_paths {
                self.park_import(result.file_id, result.file_path.clone(), import);
            }
        }
        let edge_count = all_edges.len();
        if !all_edges.is_empty() {
            self.dependency_index.add_imports_batch(all_edges);
        }
        edge_count
    }

    /// Incrementally resolve pending imports that can be resolved now.
    ///
    /// This method attempts to resolve imports where the target file is already indexed.
    /// Unresolved imports remain in the pending queue for later resolution.
    /// Call this after each batch to distribute import resolution work across the indexing phase.
    ///
    /// Returns the number of import edges resolved.
    pub fn resolve_imports_incremental(&mut self) -> usize {
        // 1. This batch's own imports.
        let mut edges = self.resolve_pending_now();
        // 2. Only the parked imports that could now resolve, i.e. whose path
        //    mentions a stem that was just indexed. Cost is proportional to
        //    the batch, not to the (ever-growing) set of unresolvable
        //    stdlib/package imports.
        let candidates = self.take_retry_candidates();
        edges += self.retry_waiting(candidates);
        edges
    }

    /// Get the number of pending imports that still need resolution.
    pub fn pending_imports_count(&self) -> usize {
        let pending: usize = self
            .pending_imports
            .iter()
            .map(|(_, _, paths)| paths.len())
            .sum();
        pending + self.waiting_imports_count()
    }

    /// Imports parked because their target is not (yet) indexed.
    pub fn waiting_imports_count(&self) -> usize {
        self.waiting_imports.iter().filter(|e| e.is_some()).count()
    }

    /// Record that a file with this path was (re)indexed, so imports waiting
    /// on its name are retried by the next incremental resolution.
    fn note_added_file(&mut self, path: &Path) {
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            self.recently_added_stems.push(stem.to_ascii_lowercase());
        }
        // `foo/mod.rs`, `pkg/__init__.py`, `dir/index.ts` are addressed by
        // their directory name.
        if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
            if matches!(
                name,
                "mod.rs"
                    | "__init__.py"
                    | "index.ts"
                    | "index.tsx"
                    | "index.js"
                    | "index.jsx"
                    | "index.mjs"
                    | "index.cjs"
            ) {
                if let Some(dir) = path
                    .parent()
                    .and_then(|d| d.file_name())
                    .and_then(|s| s.to_str())
                {
                    self.recently_added_stems.push(dir.to_ascii_lowercase());
                }
            }
        }
    }

    /// Lowercase path segments of an import string; a parked import is
    /// retried when a file whose stem equals any of them is indexed.
    fn import_keys(import: &str) -> Vec<String> {
        import
            .split(|c: char| {
                c == ':'
                    || c == '/'
                    || c == '.'
                    || c == '\\'
                    || c == '{'
                    || c == ','
                    || c == '}'
                    || c.is_whitespace()
            })
            .map(|s| s.trim_end_matches(".rs").trim_end_matches(".py"))
            .filter(|s| {
                !s.is_empty() && *s != "*" && *s != "crate" && *s != "super" && *s != "self"
            })
            .map(|s| s.to_ascii_lowercase())
            .collect()
    }

    /// Park an unresolved import until a file it might refer to appears.
    fn park_import(&mut self, file_id: u32, path: std::path::PathBuf, import: String) {
        let keys = Self::import_keys(&import);
        let idx = self.waiting_imports.len();
        self.waiting_imports.push(Some((file_id, path, import)));
        for key in keys {
            self.waiting_keys.entry(key).or_default().push(idx);
        }
    }

    /// Parked imports whose keys match any recently added file stem
    /// (consumes the stem list). Returns waiting-list indices, deduplicated.
    fn take_retry_candidates(&mut self) -> Vec<usize> {
        let stems = std::mem::take(&mut self.recently_added_stems);
        let mut seen = FxHashSet::default();
        let mut out = Vec::new();
        for stem in stems {
            if let Some(indices) = self.waiting_keys.get(&stem) {
                for &i in indices {
                    if self.waiting_imports.get(i).is_some_and(Option::is_some) && seen.insert(i) {
                        out.push(i);
                    }
                }
            }
        }
        out
    }

    /// Try to resolve the given waiting entries; resolved ones are cleared
    /// and their edges inserted. Returns the number of edges added.
    fn retry_waiting(&mut self, indices: Vec<usize>) -> usize {
        if indices.is_empty() {
            return 0;
        }
        let attempts: Vec<(usize, u32, std::path::PathBuf, String)> = indices
            .into_iter()
            .filter_map(|i| {
                self.waiting_imports[i]
                    .as_ref()
                    .map(|(id, p, imp)| (i, *id, p.clone(), imp.clone()))
            })
            .collect();
        let results: Vec<(usize, Option<(u32, u32)>)> = attempts
            .par_iter()
            .map(|(i, from_id, path, import)| {
                let edge = self
                    .dependency_index
                    .resolve_import_path(path, import)
                    .and_then(|r| self.dependency_index.get_file_id(&r))
                    .map(|to| (*from_id, to));
                (*i, edge)
            })
            .collect();
        let mut edges = Vec::new();
        for (i, edge) in results {
            if let Some(e) = edge {
                edges.push(e);
                self.waiting_imports[i] = None;
            }
        }
        let n = edges.len();
        if n > 0 {
            self.dependency_index.add_imports_batch(edges);
        }
        n
    }

    /// Drop resolved slots and rebuild the key map (call occasionally, e.g.
    /// at finalize; parked entries are few relative to the file count).
    fn compact_waiting_imports(&mut self) {
        if self.waiting_imports.iter().all(Option::is_some) {
            return;
        }
        let live: Vec<(u32, std::path::PathBuf, String)> =
            self.waiting_imports.drain(..).flatten().collect();
        self.waiting_keys.clear();
        for (id, path, import) in live {
            self.park_import(id, path, import);
        }
    }

    /// Imports still waiting, grouped per file, for persistence.
    pub fn waiting_imports_by_file(&self) -> Vec<(u32, std::path::PathBuf, Vec<String>)> {
        let mut by_id: FxHashMap<u32, (std::path::PathBuf, Vec<String>)> = FxHashMap::default();
        for (id, path, import) in self.waiting_imports.iter().flatten() {
            by_id
                .entry(*id)
                .or_insert_with(|| (path.clone(), Vec::new()))
                .1
                .push(import.clone());
        }
        for (id, path, imports) in &self.pending_imports {
            by_id
                .entry(*id)
                .or_insert_with(|| (path.clone(), Vec::new()))
                .1
                .extend(imports.iter().cloned());
        }
        by_id
            .into_iter()
            .map(|(id, (path, imports))| (id, path, imports))
            .collect()
    }

    /// Finalize the index after all files have been indexed.
    /// This pre-computes caches for optimal query performance.
    /// Call this after indexing is complete and before serving queries.
    pub fn finalize(&mut self) {
        self.trigram_index.finalize();

        // Pre-compute file metadata for fast ranking
        // This enables ranking by file-level signals without reading file content
        self.compute_all_file_metadata();
        self.generation += 1;

        tracing::info!(
            num_files = self.file_store.len(),
            "Computed file metadata for fast ranking"
        );

        // Release over-allocated Vec capacity accumulated during incremental push().
        // Vec doubles on growth; after indexing N files the backing store may be 2N slots.
        // Shrinking here can save tens to hundreds of MB on large codebases.
        self.pending_imports.shrink_to_fit();
        // Shrink the outer symbol_cache Vec and every inner Vec<Symbol>.
        // Inner Vecs are built by tree-sitter extraction which also uses push(),
        // so each may carry up to 2× over-allocation.
        for syms in &mut self.symbol_cache {
            syms.shrink_to_fit();
        }
        self.symbol_cache.shrink_to_fit();
    }

    pub fn get_stats(&self) -> SearchStats {
        SearchStats {
            num_files: self.file_store.live_len(),
            total_size: self.file_store.total_mapped_size(),
            num_trigrams: self.trigram_index.num_trigrams(),
            dependency_edges: self.dependency_index.total_edges(),
            total_content_bytes: self.file_store.total_content_bytes(),
        }
    }

    /// Get files that depend on the given file (import it)
    pub fn get_dependents(&self, file_id: u32) -> Vec<u32> {
        self.dependency_index.get_dependents(file_id)
    }

    /// Get files that the given file depends on (imports)
    pub fn get_dependencies(&self, file_id: u32) -> Vec<u32> {
        self.dependency_index.get_dependencies(file_id)
    }

    /// Get file path by ID as a root-relative display string.
    pub fn get_file_path(&self, file_id: u32) -> Option<String> {
        self.file_store
            .get(file_id)
            .map(|f| self.make_display_path(&f.path))
    }

    /// Find file ID by path.
    ///
    /// Tries an exact match first (O(1)), then falls back to suffix matching
    /// (O(n)) so that partial paths like `"src/main.rs"` resolve correctly.
    /// Suffix matching uses `ends_with` to avoid false positives from substring
    /// matches (e.g. looking for `"bar.rs"` will NOT match `"bar_extra.rs"`).
    pub fn find_file_id(&self, path: &str) -> Option<u32> {
        // 1. Exact path match (O(1)), then its canonical form.
        if let Some(id) = self.find_file_id_exact(std::path::Path::new(path)) {
            return Some(id);
        }
        // 2. Display path (`<root name>/<relative>`, what search results and
        //    the UI round-trip) reversed onto its root: still O(roots), no
        //    per-file work, and unambiguous when two roots share a suffix.
        if let Some(id) = self.find_by_display_path(path) {
            return Some(id);
        }
        // 3. Suffix / partial-path match (O(n)) as a last resort.
        self.file_store.find_by_path_suffix(path)
    }

    /// Reverse [`Self::make_display_path`]: `project/src/main.rs` ->
    /// `<root ending in project>/src/main.rs`, looked up exactly.
    fn find_by_display_path(&self, display: &str) -> Option<u32> {
        let display = display.replace('\\', "/");
        for root in &self.root_paths {
            let Some(root_name) = root.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let rest = if display == root_name {
                ""
            } else if let Some(rest) = display
                .strip_prefix(root_name)
                .and_then(|r| r.strip_prefix('/'))
            {
                rest
            } else {
                continue;
            };
            let candidate = if rest.is_empty() {
                root.clone()
            } else {
                root.join(rest)
            };
            if let Some(id) = self.find_file_id_exact(&candidate) {
                return Some(id);
            }
        }
        None
    }

    /// Update the index for a single file (for incremental indexing).
    ///
    /// If the file is already indexed, its stale trigrams, symbols, and
    /// dependency edges are removed and the store entry is refreshed (dropping the
    /// old mmap and cached UTF-8/transcode results) before re-extracting from the
    /// current content — all under the SAME file id so existing ids stay stable.
    /// A brand-new file is indexed normally.
    pub fn update_file(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        let Some(id) = self.find_file_id_exact(path) else {
            // Not yet indexed — treat as a fresh add.
            return self.index_file(path);
        };

        // Strip all stale data for this id first.
        self.trigram_index.remove_document(id);
        if let Some(slot) = self.symbol_cache.get_mut(id as usize) {
            slot.clear();
        }
        if let Some(slot) = self.reference_cache.get_mut(id as usize) {
            slot.clear();
        }
        self.dependency_index.remove_file(id);

        // Read fresh owned content + safety check (no live mmap — avoids SIGBUS if
        // the file is being rewritten concurrently).
        let (partial, _transcoded) =
            match PartialIndexedFile::process(path, self.transcode_non_utf8, self.max_file_size) {
                Some(p) => p,
                None => {
                    // File is now binary/unsafe/oversized/unreadable — drop it from
                    // the index entirely rather than keeping stale content.
                    self.file_store.remove_file_by_id(id);
                    return Ok(());
                }
            };
        let pre = PreIndexedFile::from_partial(partial, self.enable_symbols);

        // Refresh the store entry so the stale mapping + caches are discarded, and
        // re-register for import resolution.
        self.file_store.refresh_file_by_id(id);
        self.set_indexed_meta(id, pre.mtime, pre.size);
        self.note_added_file(path);
        if let Some(canonical) = self.file_store.get_path(id).map(Path::to_path_buf) {
            self.dependency_index.register_canonical_file(id, canonical);
        }

        // Re-add trigrams and symbols under the same id.
        self.trigram_index.add_document_trigrams(id, pre.trigrams);
        while self.symbol_cache.len() <= id as usize {
            self.symbol_cache.push(Vec::new());
        }
        self.symbol_cache[id as usize] = pre.symbols;
        self.store_references(id, pre.references);

        if !pre.imports.is_empty() {
            self.pending_imports
                .push((id, path.to_path_buf(), pre.imports));
            self.resolve_imports_incremental();
        }

        // Keep fast-mode ranking signals current for the touched file.
        self.refresh_file_metadata(id);
        self.generation += 1;
        Ok(())
    }

    /// Monotonic mutation counter (see the `generation` field).
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Remove a file from the index entirely (for delete / rename-away events).
    ///
    /// Drops the file's trigrams, symbols, dependency edges, and store slot
    /// (tombstoned so its id is never reused). Returns true if a matching file
    /// was found and removed.
    pub fn remove_file(&mut self, path: &std::path::Path) -> bool {
        let Some(id) = self.find_file_id_exact(path) else {
            return false;
        };
        self.remove_by_id(id);
        true
    }

    /// O(1) lookup by canonical path for file-system paths (watcher events,
    /// index/update/remove). Unlike [`Self::find_file_id`] there is no suffix
    /// fallback: a partial match must never be mistaken for the file being
    /// updated, and the suffix scan allocates per indexed file.
    pub fn find_file_id_exact(&self, path: &std::path::Path) -> Option<u32> {
        if let Some(id) = self.file_store.find_by_exact_path(path) {
            return Some(id);
        }
        let canonical = canonicalize_lossy(path);
        if canonical != path {
            return self.file_store.find_by_exact_path(&canonical);
        }
        None
    }

    /// Remove every indexed file under directory `dir` (for directory delete /
    /// rename-away events, where the watcher reports only the directory).
    /// Returns the number of files removed.
    pub fn remove_files_under(&mut self, dir: &std::path::Path) -> usize {
        let ids = self.file_ids_under(dir);
        self.remove_files_by_ids(&ids)
    }

    /// Drop every indexed file that is a direct child of one of `dirs` and no
    /// longer exists on disk. Returns how many were removed.
    ///
    /// Watcher backends lose events: FSEvents on macOS may report only the
    /// destination half of a rename, leaving the old name indexed as a
    /// zombie whose content can never be read again. Checking the siblings
    /// of every changed path costs one stat per file in those directories
    /// and makes the index self-healing against lost deletes.
    pub fn prune_vanished_in(&mut self, dirs: &[PathBuf]) -> usize {
        let mut doomed = Vec::new();
        for dir in dirs {
            let prefix = canonicalize_lossy(dir);
            for id in self.file_store.ids_under(&prefix) {
                let Some(path) = self.file_store.get_path(id) else {
                    continue;
                };
                if path.parent() == Some(prefix.as_path()) && !path.exists() {
                    doomed.push(id);
                }
            }
        }
        self.remove_files_by_ids(&doomed)
    }

    /// Ids of every live file under directory `dir` (any form of the path;
    /// it is canonicalized lossily so it need not still exist).
    pub fn file_ids_under(&self, dir: &std::path::Path) -> Vec<u32> {
        let prefix = canonicalize_lossy(dir);
        self.file_store.ids_under(&prefix)
    }

    fn remove_by_id(&mut self, id: u32) {
        self.trigram_index.remove_document(id);
        self.forget_id(id);
    }

    /// Remove many files in one pass over the trigram index (see
    /// [`TrigramIndex::remove_documents`]). Unknown/tombstoned ids are ignored.
    pub fn remove_files_by_ids(&mut self, ids: &[u32]) -> usize {
        let mut doomed = roaring::RoaringBitmap::new();
        for &id in ids {
            if self.file_store.get(id).is_some() {
                doomed.insert(id);
            }
        }
        if doomed.is_empty() {
            return 0;
        }
        self.trigram_index.remove_documents(&doomed);
        for id in doomed.iter() {
            self.forget_id(id);
        }
        doomed.len() as usize
    }

    /// Intern a reference name, returning its id.
    fn intern_ref_name(&mut self, name: &str) -> u32 {
        if let Some(&id) = self.ref_name_ids.get(name) {
            return id;
        }
        let id = self.ref_names.len() as u32;
        self.ref_names.push(name.to_string());
        self.ref_name_ids.insert(name.to_string(), id);
        id
    }

    /// Replace the reference list of `id` with `refs` (names interned).
    pub(super) fn store_references(&mut self, id: u32, refs: Vec<SymbolRef>) {
        let packed: Vec<PackedRef> = refs
            .into_iter()
            .map(|r| PackedRef {
                name: self.intern_ref_name(&r.name),
                line: r.line,
                column: r.column,
            })
            .collect();
        while self.reference_cache.len() <= id as usize {
            self.reference_cache.push(Vec::new());
        }
        self.reference_cache[id as usize] = packed;
    }

    /// Install a persisted name table and per-file reference lists (fresh
    /// engine only: ids in `references` must index `names`).
    pub(super) fn install_references(&mut self, names: Vec<String>, slots: usize) {
        self.ref_name_ids = names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.clone(), i as u32))
            .collect();
        self.ref_names = names;
        self.reference_cache = vec![Vec::new(); slots];
    }

    pub(super) fn set_packed_references(&mut self, id: u32, refs: Vec<PackedRef>) {
        while self.reference_cache.len() <= id as usize {
            self.reference_cache.push(Vec::new());
        }
        self.reference_cache[id as usize] = refs;
    }

    /// The interned id of a reference name, if any file references it.
    pub fn reference_name_id(&self, name: &str) -> Option<u32> {
        self.ref_name_ids.get(name).copied()
    }

    /// References recorded for a file (empty for unknown / removed ids).
    pub fn references_of(&self, id: u32) -> &[PackedRef] {
        self.reference_cache
            .get(id as usize)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// The persisted view of the reference name table.
    pub(super) fn reference_names(&self) -> &[String] {
        &self.ref_names
    }

    /// Total number of recorded references across live files.
    pub fn reference_count(&self) -> usize {
        self.reference_cache.iter().map(Vec::len).sum()
    }

    /// Everything except the trigram postings: symbols, metadata, dependency
    /// edges and the store slot (tombstoned so the id is never reused).
    fn forget_id(&mut self, id: u32) {
        self.generation += 1;
        if let Some(slot) = self.symbol_cache.get_mut(id as usize) {
            slot.clear();
        }
        if let Some(slot) = self.reference_cache.get_mut(id as usize) {
            slot.clear();
        }
        if let Some(meta) = self.file_metadata.get_mut(id as usize) {
            *meta = FileMetadata::default();
        }
        // Files that imported this one lose an out-edge but their own metadata
        // (symbol count, in-edges) is unchanged; files it imported lose an
        // in-edge, so refresh their dependency-count-based base score.
        let dependents_changed: Vec<u32> = self.dependency_index.get_dependencies(id);
        self.dependency_index.remove_file(id);
        self.file_store.remove_file_by_id(id);
        for other in dependents_changed {
            self.refresh_file_metadata(other);
        }
    }

    /// Recompute the fast-ranking metadata for one file (after an incremental
    /// add/update, or when its in-edge count changed). Cheap: one path walk
    /// plus two map lookups; no file I/O.
    pub fn refresh_file_metadata(&mut self, id: u32) {
        let idx = id as usize;
        let meta = match self.file_store.get(id) {
            Some(file) => {
                let symbol_count = self.symbol_cache.get(idx).map(|s| s.len()).unwrap_or(0);
                let dep_count = self.dependency_index.get_import_count(id);
                let display = self.make_display_path(&file.path);
                FileMetadata::compute(&file.path, display, symbol_count, dep_count)
            }
            None => FileMetadata::default(),
        };
        if self.file_metadata.len() <= idx {
            self.file_metadata
                .resize_with(idx + 1, FileMetadata::default);
        }
        self.file_metadata[idx] = meta;
    }

    /// Compute fast-ranking metadata for every live file. Called by
    /// `finalize()` and after a persisted load so Fast mode never falls back
    /// to "first N ids in id order" while the background reconcile runs.
    pub fn compute_all_file_metadata(&mut self) {
        let num_files = self.file_store.len();
        self.file_metadata = Vec::with_capacity(num_files);
        for file_id in 0..num_files as u32 {
            let metadata = if let Some(file) = self.file_store.get(file_id) {
                let symbol_count = self
                    .symbol_cache
                    .get(file_id as usize)
                    .map(|s| s.len())
                    .unwrap_or(0);
                let dep_count = self.dependency_index.get_import_count(file_id);
                let display = self.make_display_path(&file.path);
                FileMetadata::compute(&file.path, display, symbol_count, dep_count)
            } else {
                FileMetadata::default()
            };
            self.file_metadata.push(metadata);
        }
        self.file_metadata.shrink_to_fit();
    }
}

/// Canonicalize a path that may no longer exist (deleted or renamed away):
/// canonicalize the longest existing ancestor and re-append the remaining
/// components, so the result is comparable with the canonical paths stored
/// in the file store.
pub fn canonicalize_lossy(path: &std::path::Path) -> PathBuf {
    if let Ok(c) = path.canonicalize() {
        return c;
    }
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    let mut cur = path.to_path_buf();
    loop {
        if let Ok(c) = cur.canonicalize() {
            let mut out = c;
            for comp in tail.iter().rev() {
                out.push(comp);
            }
            return out;
        }
        match (cur.file_name(), cur.parent()) {
            (Some(name), Some(parent)) => {
                tail.push(name.to_os_string());
                cur = parent.to_path_buf();
            }
            _ => return path.to_path_buf(),
        }
    }
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
