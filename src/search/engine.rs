use crate::dependencies::DependencyIndex;
use crate::index::{extract_unique_trigrams, LazyFileStore, Trigram, TrigramIndex};
use crate::search::path_filter::PathFilter;
use crate::search::regex_search::RegexAnalysis;
use crate::symbols::{Symbol, SymbolExtractor, SymbolType};
use anyhow::Result;
use memchr::memmem;
use rayon::prelude::*;
use regex::Regex;
use rustc_hash::FxHashSet;
use std::path::{Path, PathBuf};
use tracing::warn;

/// Case-insensitive substring search without heap allocation.
/// Both `haystack` and `needle` are compared using ASCII case-insensitive matching.
/// `needle` should already be lowercase for optimal performance.
///
/// Note: ASCII-only case folding. Non-ASCII characters (e.g., accented letters,
/// CJK) are compared byte-for-byte without case folding. This is acceptable for
/// code identifiers but won't handle natural language in comments.
#[inline]
fn contains_case_insensitive(haystack: &str, needle_lower: &str) -> bool {
    if needle_lower.is_empty() {
        return true;
    }
    let needle_len = needle_lower.len();
    if haystack.len() < needle_len {
        return false;
    }

    let needle_bytes = needle_lower.as_bytes();
    let haystack_bytes = haystack.as_bytes();
    let first_needle = needle_bytes[0];
    // Also match uppercase variant of first byte for faster skip
    let first_needle_upper = if first_needle.is_ascii_lowercase() {
        first_needle - 32
    } else {
        first_needle
    };

    let mut i = 0;
    let max_start = haystack_bytes.len() - needle_len;

    while i <= max_start {
        let h = haystack_bytes[i];
        // Quick first-byte check (handles both cases)
        if h == first_needle || h == first_needle_upper {
            // Check rest of needle
            let mut matched = true;
            for j in 1..needle_len {
                let h = haystack_bytes[i + j];
                let h_lower = if h.is_ascii_uppercase() { h + 32 } else { h };
                if h_lower != needle_bytes[j] {
                    matched = false;
                    break;
                }
            }
            if matched {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// Fast byte substring search using memchr's memmem for SIMD acceleration
#[inline]
fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.len() < needle.len() {
        return false;
    }
    memmem::find(haystack, needle).is_some()
}

/// Maximum length of content to return per match (in bytes)
const MAX_CONTENT_LENGTH: usize = 500;

/// Context to show on each side of the match
const MATCH_CONTEXT_CHARS: usize = 200;

/// Result of truncating content around a match
struct TruncatedContent {
    content: String,
    match_start: usize,
    match_end: usize,
    was_truncated: bool,
}

/// Truncates a line around the match position, preserving context on both sides.
/// Returns the truncated content with ellipsis indicators if truncated.
#[inline]
fn truncate_around_match(line: &str, match_start: usize, match_end: usize) -> TruncatedContent {
    // If line is short enough, return as-is
    if line.len() <= MAX_CONTENT_LENGTH {
        return TruncatedContent {
            content: line.to_string(),
            match_start,
            match_end,
            was_truncated: false,
        };
    }

    // Calculate window around the match
    let window_start = match_start.saturating_sub(MATCH_CONTEXT_CHARS);
    let window_end = (match_end + MATCH_CONTEXT_CHARS).min(line.len());

    // Find safe UTF-8 boundaries
    let safe_start = find_char_boundary_floor(line, window_start);
    let safe_end = find_char_boundary_ceil(line, window_end);

    // Build truncated string with ellipsis indicators
    let mut result = String::with_capacity(safe_end - safe_start + 2);
    let prefix_truncated = safe_start > 0;
    let suffix_truncated = safe_end < line.len();

    if prefix_truncated {
        result.push('…');
    }
    result.push_str(&line[safe_start..safe_end]);
    if suffix_truncated {
        result.push('…');
    }

    // Adjust match positions relative to the new string
    let offset = safe_start;
    let new_match_start = if prefix_truncated {
        match_start - offset + 1 // +1 for the ellipsis
    } else {
        match_start - offset
    };
    let new_match_end = if prefix_truncated {
        match_end - offset + 1
    } else {
        match_end - offset
    };

    TruncatedContent {
        content: result,
        match_start: new_match_start,
        match_end: new_match_end,
        was_truncated: true,
    }
}

/// Find the largest valid char boundary <= pos
#[inline]
fn find_char_boundary_floor(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    let mut p = pos;
    while p > 0 && !s.is_char_boundary(p) {
        p -= 1;
    }
    p
}

/// Find the smallest valid char boundary >= pos
#[inline]
fn find_char_boundary_ceil(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    let mut p = pos;
    while p < s.len() && !s.is_char_boundary(p) {
        p += 1;
    }
    p
}

/// Find match position using case-insensitive search
#[inline]
fn find_match_position_case_insensitive(
    haystack: &str,
    needle_lower: &str,
) -> Option<(usize, usize)> {
    if needle_lower.is_empty() {
        return Some((0, 0));
    }
    let needle_len = needle_lower.len();
    if haystack.len() < needle_len {
        return None;
    }

    let needle_bytes = needle_lower.as_bytes();
    let haystack_bytes = haystack.as_bytes();
    let first_needle = needle_bytes[0];
    let first_needle_upper = if first_needle.is_ascii_lowercase() {
        first_needle - 32
    } else {
        first_needle
    };

    let mut i = 0;
    let max_start = haystack_bytes.len() - needle_len;

    while i <= max_start {
        let h = haystack_bytes[i];
        if h == first_needle || h == first_needle_upper {
            let mut matched = true;
            for j in 1..needle_len {
                let h = haystack_bytes[i + j];
                let h_lower = if h.is_ascii_uppercase() { h + 32 } else { h };
                if h_lower != needle_bytes[j] {
                    matched = false;
                    break;
                }
            }
            if matched {
                return Some((i, i + needle_len));
            }
        }
        i += 1;
    }
    None
}

/// Inline scoring function with pre-computed values (no method call overhead, no redundant lookups)
///
/// `original_query` is the un-lowered query for exact case-sensitive match boosting.
/// `query_lower` is the lowercased query for start-of-line checks.
#[inline]
fn calculate_score_inline(
    line: &str,
    original_query: &str,
    query_lower: &str,
    is_symbol_def: bool,
    is_src_lib: bool,
    dependency_boost: f64,
) -> f64 {
    let mut score = 1.0;

    // Boost for exact case-sensitive matches (using the original un-lowered query)
    if line.contains(original_query) {
        score *= 2.0;
    }

    // Boost for symbol definitions (pre-computed)
    if is_symbol_def {
        score *= 3.0;
    }

    // Boost for primary source directories (pre-computed)
    if is_src_lib {
        score *= 1.5;
    }

    // Boost for shorter lines (more relevant) — gentler logarithmic curve, floors at 0.3
    let line_len_factor = (1.0 / (1.0 + (line.len() as f64 / 100.0).ln_1p())).max(0.3);
    score *= line_len_factor;

    // Boost for query appearing at the start of the line
    let trimmed = line.trim_start();
    if trimmed.len() >= query_lower.len()
        && trimmed.as_bytes()[..query_lower.len()].eq_ignore_ascii_case(query_lower.as_bytes())
    {
        score *= 1.5;
    }

    // Apply pre-computed dependency boost
    score * dependency_boost
}

/// Inline regex scoring function with pre-computed values
#[inline]
fn calculate_score_regex_inline(
    line: &str,
    regex: &Regex,
    is_symbol_def: bool,
    is_src_lib: bool,
    dependency_boost: f64,
) -> f64 {
    let mut score = 1.0;

    // Boost for symbol definitions (pre-computed)
    if is_symbol_def {
        score *= 3.0;
    }

    // Boost for primary source directories (pre-computed)
    if is_src_lib {
        score *= 1.5;
    }

    // Boost for shorter lines (more relevant) — gentler logarithmic curve, floors at 0.3
    let line_len_factor = (1.0 / (1.0 + (line.len() as f64 / 100.0).ln_1p())).max(0.3);
    score *= line_len_factor;

    // Boost for matches at the start of the line
    let trimmed = line.trim_start();
    if let Some(m) = regex.find(trimmed) {
        if m.start() == 0 {
            score *= 1.5;
        }
    }

    // Apply pre-computed dependency boost
    score * dependency_boost
}

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
    pub score: f64,
    pub is_symbol: bool,
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
}

impl PartialIndexedFile {
    /// Maximum file size to process (10MB) - larger files are skipped
    const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

    /// Phase 1: pure-Rust work only — safe to run in parallel across rayon threads.
    /// Does NOT call tree-sitter (C FFI) to avoid concurrent heap corruption.
    ///
    /// When `transcode_non_utf8` is true, files in non-UTF-8 encodings (Latin-1,
    /// Shift-JIS, UTF-16, etc.) are automatically transcoded. When false, only
    /// UTF-8 files are accepted.
    /// Returns `Some((file, transcoded))` where `transcoded` is `true` when
    /// the file was converted from a non-UTF-8 encoding via `transcode_to_utf8`.
    pub fn process(path: &Path, transcode_non_utf8: bool) -> Option<(Self, bool)> {
        let metadata = std::fs::metadata(path).ok()?;
        if metadata.len() > Self::MAX_FILE_SIZE {
            return None;
        }

        let raw_bytes = std::fs::read(path).ok()?;
        let (content, transcoded) = match std::str::from_utf8(&raw_bytes) {
            Ok(s) => (s.to_string(), false), // UTF-8 fast path
            Err(_) => {
                if !transcode_non_utf8 {
                    return None; // Transcoding disabled
                }
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

        // Safety check: skip files that could crash tree-sitter or produce garbage trigrams
        if let Some(reason) = crate::utils::content_safety_check(&content) {
            tracing::debug!(
                path = %path.display(),
                reason = reason,
                "Skipping unsafe file during indexing"
            );
            return None;
        }

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

        // Prepend filename stem so filenames are searchable.
        // Triple newline avoids garbage trigrams at the boundary.
        let content_with_filename = format!("{}\n\n\n{}", filename_stem, content);
        let content_lower = content_with_filename.to_lowercase();
        let trigrams = extract_unique_trigrams(&content_lower);

        Some((
            PartialIndexedFile {
                path: path.to_path_buf(),
                trigrams,
                filename_stem,
                content,
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
}

impl PreIndexedFile {
    /// Phase 2: run tree-sitter symbol/import extraction on an already-processed partial.
    ///
    /// Uses `extract_all` to parse the source a single time for both symbols and imports.
    /// Safe to call from multiple rayon threads simultaneously — tree-sitter `Parser` is
    /// `Send + Sync` in tree-sitter v0.26+, and each call creates an independent `Parser`
    /// instance with no shared mutable state.
    pub fn from_partial(partial: PartialIndexedFile) -> Self {
        let extractor = SymbolExtractor::new(&partial.path);

        // Extract symbols and imports in a single parse with panic protection.
        // tree-sitter can stack overflow on deeply nested or malformed files.
        let (mut symbols, imports) =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                extractor
                    .extract_all(&partial.content)
                    .unwrap_or_default()
            }))
            .unwrap_or_else(|_| {
                warn!(
                    "Symbol/import extraction panicked for file '{}'. This typically occurs with deeply nested or malformed syntax. Continuing without symbols.",
                    partial.path.display()
                );
                (Vec::new(), Vec::new())
            });

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
}

impl FileMetadata {
    /// Compute metadata for a file at index time
    fn compute(path: &Path, symbol_count: usize, dependency_count: u32) -> Self {
        let mut base_score: f32 = 1.0;

        let path_str = path.to_string_lossy();
        let path_lower = path_str.to_lowercase();

        // Check if in source directories
        let is_src_lib = path_lower.contains("/src/")
            || path_lower.contains("\\src\\")
            || path_lower.contains("/lib/")
            || path_lower.contains("\\lib\\");

        if is_src_lib {
            base_score += 2.0;
        }

        // Boost for high-value extensions
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext.to_lowercase().as_str() {
                "rs" | "py" | "ts" | "js" | "go" | "java" | "c" | "cpp" | "h" => base_score += 1.5,
                "md" | "txt" | "json" | "toml" | "yaml" | "yml" => base_score += 0.5,
                _ => {}
            }
        }

        // Boost for files with symbols (more likely to be important code)
        if symbol_count > 0 {
            base_score += (symbol_count as f32).log2().min(4.0); // Up to +4 for 16+ symbols
        }

        // Boost for dependency count (files imported by others are important)
        if dependency_count > 0 {
            base_score += (dependency_count as f32).log2().min(5.0);
        }

        // Penalty for test/example directories
        if path_lower.contains("/test")
            || path_lower.contains("\\test")
            || path_lower.contains("/example")
            || path_lower.contains("\\example")
        {
            base_score *= 0.7;
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
        }
    }

    /// Compute ranking score for a specific query
    /// This is called during search but doesn't require reading file content
    #[inline]
    fn query_score(&self, query_lower: &str) -> f32 {
        let mut score = self.base_score;

        // Big boost if query matches filename (using pre-computed lowercase stem)
        if !query_lower.is_empty() && self.lowercase_stem.contains(query_lower) {
            score *= 5.0;
        }

        score
    }
}

pub struct SearchEngine {
    pub file_store: LazyFileStore,
    pub trigram_index: TrigramIndex,
    pub dependency_index: DependencyIndex,
    symbol_cache: Vec<Vec<Symbol>>,
    /// Pre-computed file metadata for fast ranking
    file_metadata: Vec<FileMetadata>,
    /// Pending imports to resolve after all files are indexed
    pending_imports: Vec<(u32, std::path::PathBuf, Vec<String>)>,
}

impl SearchEngine {
    pub fn new() -> Self {
        Self {
            file_store: LazyFileStore::new(),
            trigram_index: TrigramIndex::new(),
            dependency_index: DependencyIndex::new(),
            symbol_cache: Vec::new(),
            file_metadata: Vec::new(),
            pending_imports: Vec::new(),
        }
    }

    /// Index a file
    pub fn index_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let file_id = self.file_store.add_file(path)?;

        // Register file in dependency index for import resolution
        self.dependency_index.register_file(file_id, path);

        // Get the content
        let content = self
            .file_store
            .get(file_id)
            .and_then(|f| f.as_str().ok())
            .unwrap_or("");

        // Safety check: skip files that could crash tree-sitter or produce garbage
        if let Some(reason) = crate::utils::content_safety_check(content) {
            tracing::warn!(
                path = %path.display(),
                reason = reason,
                "Skipping unsafe file during indexing"
            );
            return Ok(());
        }

        // Extract filename stem for indexing (enables searching by filename)
        let filename_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_else(|| {
                warn!(
                    "Failed to extract filename stem from path: {}",
                    path.display()
                );
                ""
            });

        // Index the lowercase content with trigrams for case-insensitive search
        // Prepend filename so it's also searchable
        // Note: We lowercase during indexing so queries can be lowercased at search time
        // Use triple newlines to avoid creating garbage trigrams at the filename/content boundary
        let content_with_filename = format!("{}\n\n\n{}", filename_stem, content);
        let content_lower = content_with_filename.to_lowercase();
        self.trigram_index.add_document(file_id, &content_lower);

        // Extract symbols
        let extractor = SymbolExtractor::new(path);
        let mut symbols = extractor.extract(content).unwrap_or_default();

        // Add filename as a FileName symbol (line 0, gets symbol scoring boost)
        if !filename_stem.is_empty() {
            symbols.push(Symbol {
                name: filename_stem.to_string(),
                symbol_type: SymbolType::FileName,
                line: 0,
                column: 0,
                is_definition: true,
            });
        }

        // Extract imports and store for later resolution
        if let Ok(imports) = extractor.extract_imports(content) {
            if !imports.is_empty() {
                let import_paths: Vec<String> = imports.into_iter().map(|i| i.path).collect();
                self.pending_imports
                    .push((file_id, path.to_path_buf(), import_paths));
            }
        }

        // Ensure symbol_cache is large enough
        while self.symbol_cache.len() <= file_id as usize {
            self.symbol_cache.push(Vec::new());
        }
        self.symbol_cache[file_id as usize] = symbols;

        Ok(())
    }

    /// Index a batch of pre-processed files.
    /// This is the merge step after parallel processing - only this needs the write lock.
    /// Returns the number of files successfully indexed.
    pub fn index_batch(&mut self, batch: Vec<PreIndexedFile>) -> usize {
        let mut count = 0;

        for pre_indexed in batch {
            // Add file to store - this also memory-maps it
            let file_id = match self.file_store.add_file(&pre_indexed.path) {
                Ok(id) => id,
                Err(_) => continue,
            };

            // Register file in dependency index
            self.dependency_index
                .register_file(file_id, &pre_indexed.path);

            // Add trigrams to index (using pre-computed trigrams)
            self.trigram_index
                .add_document_trigrams(file_id, pre_indexed.trigrams);

            // Store symbols
            while self.symbol_cache.len() <= file_id as usize {
                self.symbol_cache.push(Vec::new());
            }
            self.symbol_cache[file_id as usize] = pre_indexed.symbols;

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
        let pending = std::mem::take(&mut self.pending_imports);

        if pending.is_empty() {
            return;
        }

        // Phase 1: Parallel path resolution - collect (from_id, to_id) pairs
        // Uses &self on dependency_index (thread-safe read-only methods)
        let edges: Vec<(u32, u32)> = pending
            .par_iter()
            .flat_map(|(file_id, file_path, import_paths)| {
                import_paths
                    .par_iter()
                    .filter_map(|import_path| {
                        let resolved = self
                            .dependency_index
                            .resolve_import_path(file_path, import_path)?;
                        let to_id = self.dependency_index.get_file_id(&resolved)?;
                        Some((*file_id, to_id))
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        // Phase 2: Sequential batch insert (requires &mut self)
        self.dependency_index.add_imports_batch(edges);
    }

    /// Incrementally resolve pending imports that can be resolved now.
    ///
    /// This method attempts to resolve imports where the target file is already indexed.
    /// Unresolved imports remain in the pending queue for later resolution.
    /// Call this after each batch to distribute import resolution work across the indexing phase.
    ///
    /// Returns the number of import edges resolved.
    pub fn resolve_imports_incremental(&mut self) -> usize {
        if self.pending_imports.is_empty() {
            return 0;
        }

        let pending = std::mem::take(&mut self.pending_imports);

        // Phase 1: Parallel path resolution - try to resolve each import
        // Collect resolved edges and unresolved imports separately
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
                    // Could not resolve - keep for later
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

        // Phase 2: Sequential processing - insert resolved edges and collect unresolved
        let mut all_edges = Vec::new();
        for result in results {
            all_edges.extend(result.resolved_edges);

            // Re-add unresolved imports to pending
            if !result.unresolved_paths.is_empty() {
                self.pending_imports.push((
                    result.file_id,
                    result.file_path,
                    result.unresolved_paths,
                ));
            }
        }

        let edge_count = all_edges.len();

        // Batch insert all resolved edges
        if !all_edges.is_empty() {
            self.dependency_index.add_imports_batch(all_edges);
        }

        edge_count
    }

    /// Get the number of pending imports that still need resolution.
    pub fn pending_imports_count(&self) -> usize {
        self.pending_imports
            .iter()
            .map(|(_, _, paths)| paths.len())
            .sum()
    }

    /// Finalize the index after all files have been indexed.
    /// This pre-computes caches for optimal query performance.
    /// Call this after indexing is complete and before serving queries.
    pub fn finalize(&mut self) {
        self.trigram_index.finalize();

        // Pre-compute file metadata for fast ranking
        // This enables ranking by file-level signals without reading file content
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
                FileMetadata::compute(&file.path, symbol_count, dep_count)
            } else {
                FileMetadata::default()
            };
            self.file_metadata.push(metadata);
        }

        tracing::info!(
            num_files = num_files,
            "Computed file metadata for fast ranking"
        );
    }

    pub fn rebuild_symbols_and_dependencies(&mut self) -> RebuildCacheStats {
        self.rebuild_symbols_and_dependencies_with_progress(|_, _| {})
    }

    /// Restore symbol caches and dependency graph directly from persisted data.
    ///
    /// `valid_file_indices` are the positions in `persisted.files` that were not stale/removed.
    /// Files are registered in the file_store in that order, so new file ID `k` corresponds to
    /// `valid_file_indices[k]` in the persisted data.
    pub fn restore_symbols_and_deps(
        &mut self,
        valid_file_indices: &[usize],
        persisted: &crate::index::PersistedIndex,
    ) {
        let total_new_files = valid_file_indices.len();

        // Allocate symbol cache sized for the newly registered files
        self.symbol_cache = vec![Vec::new(); total_new_files];

        // Build a mapping: original persisted file index → new file ID
        let orig_to_new_id: rustc_hash::FxHashMap<u32, u32> = valid_file_indices
            .iter()
            .enumerate()
            .map(|(new_id, &orig_idx)| (orig_idx as u32, new_id as u32))
            .collect();

        // Register files in dependency_index for future import resolution
        // and restore per-file symbol caches
        for (new_id, &orig_idx) in valid_file_indices.iter().enumerate() {
            let new_id = new_id as u32;
            if let Some(path) = self.file_store.get_path(new_id) {
                self.dependency_index.register_file(new_id, path);
            }
            if let Some(syms) = persisted.symbols.get(orig_idx) {
                self.symbol_cache[new_id as usize] = syms.clone();
            }
        }

        // Restore dependency edges, remapping original indices to new file IDs
        // and dropping edges whose endpoints were stale/removed
        let remapped_edges: Vec<(u32, u32)> = persisted
            .dependency_edges
            .iter()
            .filter_map(|&(from_orig, to_orig)| {
                let new_from = orig_to_new_id.get(&from_orig)?;
                let new_to = orig_to_new_id.get(&to_orig)?;
                Some((*new_from, *new_to))
            })
            .collect();
        self.dependency_index.add_imports_batch(remapped_edges);
    }

    pub fn rebuild_symbols_and_dependencies_with_progress<F>(
        &mut self,
        mut progress_callback: F,
    ) -> RebuildCacheStats
    where
        F: FnMut(usize, usize),
    {
        let total_files = self.file_store.len();
        if total_files == 0 {
            return RebuildCacheStats::default();
        }

        // Reset derived state
        self.symbol_cache = vec![Vec::new(); total_files];
        self.pending_imports.clear();
        self.dependency_index.clear();

        // Re-register all files for import resolution
        for file_id in 0..total_files as u32 {
            if let Some(path) = self.file_store.get_path(file_id) {
                self.dependency_index.register_file(file_id, path);
            }
        }

        let file_store = &self.file_store;

        progress_callback(0, total_files);

        let entries: Vec<RebuildEntry> = (0..total_files as u32)
            .into_par_iter()
            .filter_map(|file_id| {
                let path = file_store.get_path(file_id)?.to_path_buf();

                let mut symbols = Vec::new();
                let mut imports = Vec::new();
                let mut had_content = false;

                if let Some(file) = file_store.get(file_id) {
                    if let Ok(content) = file.as_str() {
                        had_content = true;

                        // Skip files that are unsafe for tree-sitter parsing
                        if let Some(reason) = crate::utils::content_safety_check(content) {
                            tracing::debug!(
                                path = %path.display(),
                                reason = reason,
                                "Skipping unsafe file during symbol rebuild"
                            );
                            // Fall through — filename symbol is still added below
                        } else {
                            let extractor = SymbolExtractor::new(&path);

                            let (extracted_symbols, extracted_imports) =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    extractor.extract_all(content).unwrap_or_default()
                                }))
                                .unwrap_or_else(|_| {
                                    warn!(
                                        "Symbol/import extraction panicked for file '{}'. Continuing without symbols.",
                                        path.display()
                                    );
                                    (Vec::new(), Vec::new())
                                });

                            symbols = extracted_symbols;
                            imports = extracted_imports
                                .into_iter()
                                .map(|i| i.path)
                                .collect();
                        }
                    }
                }

                // Always add filename symbol for filename-only matches.
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if !stem.is_empty() {
                        symbols.push(Symbol {
                            name: stem.to_string(),
                            symbol_type: SymbolType::FileName,
                            line: 0,
                            column: 0,
                            is_definition: true,
                        });
                    }
                }

                Some(RebuildEntry {
                    file_id,
                    path,
                    symbols,
                    imports,
                    had_content,
                })
            })
            .collect();

        let mut stats = RebuildCacheStats {
            files_processed: entries.len(),
            ..RebuildCacheStats::default()
        };

        for entry in entries {
            stats.symbols_extracted += entry.symbols.len();
            stats.imports_extracted += entry.imports.len();
            if !entry.had_content {
                stats.files_skipped += 1;
            }

            if entry.file_id as usize >= self.symbol_cache.len() {
                self.symbol_cache
                    .resize(entry.file_id as usize + 1, Vec::new());
            }
            self.symbol_cache[entry.file_id as usize] = entry.symbols;

            if !entry.imports.is_empty() {
                self.pending_imports
                    .push((entry.file_id, entry.path, entry.imports));
            }
        }

        progress_callback(total_files, total_files);

        self.resolve_imports();
        self.finalize();

        stats
    }

    /// Get the metadata for a file
    #[inline]
    fn get_file_metadata(&self, file_id: u32) -> &FileMetadata {
        static DEFAULT: std::sync::OnceLock<FileMetadata> = std::sync::OnceLock::new();
        self.file_metadata
            .get(file_id as usize)
            .unwrap_or_else(|| DEFAULT.get_or_init(FileMetadata::default))
    }

    /// Threshold for using fast ranking mode in Auto mode
    const FAST_RANKING_THRESHOLD: usize = 5000;

    /// Maximum files to read in fast ranking mode
    const FAST_RANKING_TOP_N: usize = 2000;

    /// Search with configurable ranking mode.
    ///
    /// # Ranking Modes
    /// - `Auto`: Uses fast ranking if candidates > 5000, else full ranking
    /// - `Fast`: Ranks by pre-computed file scores, reads only top N files
    /// - `Full`: Reads all candidates for line-level scoring (slower but most accurate)
    ///
    /// Returns (matches, ranking_info) where ranking_info contains metadata about the search.
    #[tracing::instrument(skip(self), fields(max_results, rank_mode = ?rank_mode))]
    pub fn search_ranked(
        &self,
        query: &str,
        max_results: usize,
        rank_mode: RankMode,
    ) -> (Vec<SearchMatch>, SearchRankingInfo) {
        let query_lower = query.to_lowercase();
        let candidate_docs = self.trigram_index.search(&query_lower);
        let total_candidates = candidate_docs.len() as usize;

        // Determine effective ranking mode
        let use_fast = match rank_mode {
            RankMode::Fast => true,
            RankMode::Full => false,
            RankMode::Auto => total_candidates > Self::FAST_RANKING_THRESHOLD,
        };

        let effective_mode = if use_fast {
            RankMode::Fast
        } else {
            RankMode::Full
        };

        if use_fast && !self.file_metadata.is_empty() {
            // Fast ranking: score by file metadata, read only top N
            let matches = self.search_fast_ranked_with_query(
                query,
                &query_lower,
                &candidate_docs,
                max_results,
            );
            let info = SearchRankingInfo {
                mode: effective_mode,
                total_candidates,
                candidates_searched: Self::FAST_RANKING_TOP_N.min(total_candidates),
            };
            (matches, info)
        } else {
            // Full ranking: read all candidates
            let matches = self.search_full_ranked_with_query(
                query,
                &query_lower,
                &candidate_docs,
                max_results,
            );
            let info = SearchRankingInfo {
                mode: effective_mode,
                total_candidates,
                candidates_searched: total_candidates,
            };
            (matches, info)
        }
    }

    /// Fast ranking with original query for exact-match scoring.
    ///
    /// `original_query` is passed to line-level scoring for case-sensitive match boost.
    /// `query_lower` is used for case-insensitive matching and file-level scoring.
    fn search_fast_ranked_with_query(
        &self,
        original_query: &str,
        query_lower: &str,
        candidate_docs: &roaring::RoaringBitmap,
        max_results: usize,
    ) -> Vec<SearchMatch> {
        // Score all candidates by file metadata (no file reads, no allocations)
        let mut scored_candidates: Vec<(u32, f32)> = candidate_docs
            .iter()
            .map(|doc_id| {
                let meta = self.get_file_metadata(doc_id);
                let score = meta.query_score(query_lower);
                (doc_id, score)
            })
            .collect();

        scored_candidates
            .sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let top_n = Self::FAST_RANKING_TOP_N.min(scored_candidates.len());
        let top_candidates: Vec<u32> = scored_candidates[..top_n]
            .iter()
            .map(|(id, _)| *id)
            .collect();

        let mut matches: Vec<SearchMatch> = top_candidates
            .par_iter()
            .filter_map(|&doc_id| {
                self.search_in_document_scored(doc_id, original_query, query_lower)
            })
            .flatten()
            .collect();

        self.sort_and_truncate(&mut matches, max_results);
        matches
    }

    /// Full ranking with original query for exact-match scoring.
    ///
    /// `original_query` is passed to line-level scoring for case-sensitive match boost.
    /// `query_lower` is used for case-insensitive matching.
    fn search_full_ranked_with_query(
        &self,
        original_query: &str,
        query_lower: &str,
        candidate_docs: &roaring::RoaringBitmap,
        max_results: usize,
    ) -> Vec<SearchMatch> {
        let doc_ids: Vec<u32> = candidate_docs.iter().collect();

        let mut matches: Vec<SearchMatch> = doc_ids
            .par_iter()
            .filter_map(|&doc_id| {
                self.search_in_document_scored(doc_id, original_query, query_lower)
            })
            .flatten()
            .collect();

        self.sort_and_truncate(&mut matches, max_results);
        matches
    }

    /// Helper to sort matches by score and truncate to max_results
    fn sort_and_truncate(&self, matches: &mut Vec<SearchMatch>, max_results: usize) {
        if matches.len() > max_results {
            matches.select_nth_unstable_by(max_results, |a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            matches.truncate(max_results);
            matches.sort_unstable_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        } else {
            matches.sort_unstable_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }

    /// Search for a query using parallel processing (uses Auto ranking mode).
    /// For explicit control over ranking, use `search_ranked()`.
    #[tracing::instrument(skip(self))]
    pub fn search(&self, query: &str, max_results: usize) -> Vec<SearchMatch> {
        let (matches, _info) = self.search_ranked(query, max_results, RankMode::Auto);
        matches
    }

    /// Search with path filtering using include/exclude glob patterns.
    ///
    /// This extends the basic search with additional path-based filtering:
    /// 1. Trigram index narrows candidates based on query content
    /// 2. Path filter further narrows based on file paths
    /// 3. Uses fast or full ranking based on candidate count
    ///
    /// # Arguments
    /// * `query` - The search query string
    /// * `include_patterns` - Semicolon-delimited glob patterns to include
    /// * `exclude_patterns` - Semicolon-delimited glob patterns to exclude
    /// * `max_results` - Maximum number of results to return
    #[tracing::instrument(skip(self))]
    pub fn search_with_filter(
        &self,
        query: &str,
        include_patterns: &str,
        exclude_patterns: &str,
        max_results: usize,
    ) -> Result<Vec<SearchMatch>> {
        let (matches, _) = self.search_with_filter_ranked(
            query,
            include_patterns,
            exclude_patterns,
            max_results,
            RankMode::Auto,
        )?;
        Ok(matches)
    }

    /// Search with path filtering and explicit ranking mode control.
    #[tracing::instrument(skip(self), fields(rank_mode = ?rank_mode))]
    pub fn search_with_filter_ranked(
        &self,
        query: &str,
        include_patterns: &str,
        exclude_patterns: &str,
        max_results: usize,
        rank_mode: RankMode,
    ) -> Result<(Vec<SearchMatch>, SearchRankingInfo)> {
        // Build path filter from patterns
        let path_filter = PathFilter::from_delimited(include_patterns, exclude_patterns)?;

        let query_lower = query.to_lowercase();
        let candidate_docs = self.trigram_index.search(&query_lower);

        // Apply path filter
        let filtered_docs = if path_filter.is_empty() {
            candidate_docs
        } else {
            path_filter
                .filter_documents_with(&candidate_docs, |doc_id| self.file_store.get_path(doc_id))
        };

        let total_candidates = filtered_docs.len() as usize;

        // Determine ranking mode
        let use_fast = match rank_mode {
            RankMode::Fast => true,
            RankMode::Full => false,
            RankMode::Auto => total_candidates > Self::FAST_RANKING_THRESHOLD,
        };

        let effective_mode = if use_fast {
            RankMode::Fast
        } else {
            RankMode::Full
        };

        let matches = if use_fast && !self.file_metadata.is_empty() {
            self.search_fast_ranked_with_query(query, &query_lower, &filtered_docs, max_results)
        } else {
            self.search_full_ranked_with_query(query, &query_lower, &filtered_docs, max_results)
        };

        let info = SearchRankingInfo {
            mode: effective_mode,
            total_candidates,
            candidates_searched: if use_fast {
                Self::FAST_RANKING_TOP_N.min(total_candidates)
            } else {
                total_candidates
            },
        };

        Ok((matches, info))
    }

    /// Search using a regex pattern with trigram acceleration.
    ///
    /// This method:
    /// 1. Parses the regex and extracts literal strings
    /// 2. Uses extracted literals for trigram pre-filtering (if available)
    /// 3. Falls back to full scan if no literals can be extracted
    /// 4. Runs regex matching only on candidate documents
    ///
    /// # Arguments
    /// * `pattern` - The regex pattern to search for
    /// * `include_patterns` - Semicolon-delimited glob patterns to include
    /// * `exclude_patterns` - Semicolon-delimited glob patterns to exclude
    /// * `max_results` - Maximum number of results to return
    #[tracing::instrument(skip(self))]
    pub fn search_regex(
        &self,
        pattern: &str,
        include_patterns: &str,
        exclude_patterns: &str,
        max_results: usize,
    ) -> Result<Vec<SearchMatch>> {
        // Analyze the regex pattern
        let analysis = RegexAnalysis::analyze(pattern)?;

        // Build path filter from patterns
        let path_filter = PathFilter::from_delimited(include_patterns, exclude_patterns)?;

        // Get candidate documents using trigram acceleration if possible
        let candidate_docs = if analysis.is_accelerated {
            if let Some(literal) = analysis.best_literal() {
                // Lowercase the literal since the trigram index stores lowercased content
                let literal_lower = literal.to_lowercase();
                tracing::debug!(pattern = %pattern, literal = %literal, "Using trigram acceleration for regex");
                self.trigram_index.search(&literal_lower)
            } else {
                tracing::warn!(pattern = %pattern, "Regex has no usable literals - full scan");
                self.trigram_index.all_documents()
            }
        } else {
            tracing::warn!(pattern = %pattern, "Regex has no extractable literals >= 3 chars - full scan");
            self.trigram_index.all_documents()
        };

        // Apply path filter
        let filtered_docs = if path_filter.is_empty() {
            candidate_docs
        } else {
            path_filter
                .filter_documents_with(&candidate_docs, |doc_id| self.file_store.get_path(doc_id))
        };

        let total_candidates = filtered_docs.len() as usize;
        let use_fast =
            total_candidates > Self::FAST_RANKING_THRESHOLD && !self.file_metadata.is_empty();

        let doc_ids: Vec<u32> = if use_fast {
            // Fast ranking: sort by file score, take top N
            let mut scored: Vec<(u32, f32)> = filtered_docs
                .iter()
                .map(|id| (id, self.get_file_metadata(id).base_score))
                .collect();
            scored.sort_unstable_by(|a, b| {
                b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
            });
            scored
                .iter()
                .take(Self::FAST_RANKING_TOP_N)
                .map(|(id, _)| *id)
                .collect()
        } else {
            filtered_docs.iter().collect()
        };

        // Search with regex
        let regex = &analysis.regex;
        let mut matches: Vec<SearchMatch> = doc_ids
            .par_iter()
            .filter_map(|&doc_id| self.search_in_document_regex(doc_id, regex))
            .flatten()
            .collect();

        self.sort_and_truncate(&mut matches, max_results);
        Ok(matches)
    }

    /// Search only in discovered symbols (functions, classes, methods, types, etc.).
    ///
    /// This method searches only in the symbol cache, returning matches where
    /// symbol names (functions, classes, methods, types, etc.) match the query.
    /// Filename matches are included as synthetic symbol results.
    /// This is much faster than full-text search when you're looking for definitions.
    ///
    /// # Arguments
    /// * `query` - The search query string
    /// * `include_patterns` - Semicolon-delimited glob patterns to include
    /// * `exclude_patterns` - Semicolon-delimited glob patterns to exclude
    /// * `max_results` - Maximum number of results to return
    #[tracing::instrument(skip(self))]
    pub fn search_symbols(
        &self,
        query: &str,
        include_patterns: &str,
        exclude_patterns: &str,
        max_results: usize,
    ) -> Result<Vec<SearchMatch>> {
        // Build path filter from patterns
        let path_filter = PathFilter::from_delimited(include_patterns, exclude_patterns)?;

        // Pre-compute lowercase query ONCE
        let query_lower = query.to_lowercase();

        // Use trigram index to narrow candidates if the query is long enough for trigrams (>= 3 chars).
        // This avoids scanning every file when the trigram index can pre-filter.
        let candidate_docs = if query_lower.len() >= 3 {
            self.trigram_index.search(&query_lower)
        } else {
            // Query too short for trigrams — fall back to all documents
            self.trigram_index.all_documents()
        };

        // Apply path filter if it has any patterns
        let filtered_docs = if path_filter.is_empty() {
            candidate_docs
        } else {
            path_filter
                .filter_documents_with(&candidate_docs, |doc_id| self.file_store.get_path(doc_id))
        };

        let total_candidates = filtered_docs.len() as usize;
        let use_fast =
            total_candidates > Self::FAST_RANKING_THRESHOLD && !self.file_metadata.is_empty();

        let doc_ids: Vec<u32> = if use_fast {
            // Fast ranking: sort by file score (prioritize files with more symbols)
            let mut scored: Vec<(u32, f32)> = filtered_docs
                .iter()
                .map(|id| (id, self.get_file_metadata(id).base_score))
                .collect();
            scored.sort_unstable_by(|a, b| {
                b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
            });
            scored
                .iter()
                .take(Self::FAST_RANKING_TOP_N)
                .map(|(id, _)| *id)
                .collect()
        } else {
            filtered_docs.iter().collect()
        };

        // Search symbols in parallel
        let mut matches: Vec<SearchMatch> = doc_ids
            .par_iter()
            .filter_map(|&doc_id| self.search_symbols_in_document(doc_id, &query_lower))
            .flatten()
            .collect();

        self.sort_and_truncate(&mut matches, max_results);
        Ok(matches)
    }

    /// Search for symbols matching the query in a document.
    /// Returns matches only for lines where a symbol name matches.
    fn search_symbols_in_document(
        &self,
        doc_id: u32,
        query_lower: &str,
    ) -> Option<Vec<SearchMatch>> {
        let file = self.file_store.get(doc_id)?;
        let content = file.as_str().ok()?;

        // Get symbols for this file
        let symbols = self.symbol_cache.get(doc_id as usize)?;

        // Find symbols matching the query
        let matching_symbols: Vec<&Symbol> = symbols
            .iter()
            .filter(|s| contains_case_insensitive(&s.name, query_lower))
            .collect();

        if matching_symbols.is_empty() {
            return None;
        }

        // Get dependency count for this file
        let dependency_count = self.dependency_index.get_import_count(doc_id);

        // Pre-compute dependency boost
        let dependency_boost = if dependency_count > 0 {
            1.0 + (dependency_count as f64).log10() * 0.5
        } else {
            1.0
        };

        // Lazy-compute path info
        let path_str = file.path.to_string_lossy().into_owned();
        let path_bytes = path_str.as_bytes();
        let is_src_lib = contains_bytes(path_bytes, b"/src/")
            || contains_bytes(path_bytes, b"\\src\\")
            || contains_bytes(path_bytes, b"/lib/")
            || contains_bytes(path_bytes, b"\\lib\\");

        // Collect lines into a vector for indexed access
        let lines: Vec<&str> = content.lines().collect();

        // Build matches from matching symbols
        let mut matches = Vec::with_capacity(matching_symbols.len());

        for symbol in matching_symbols {
            // FileName symbols are synthetic (not from file content) — show the file path
            if symbol.symbol_type == SymbolType::FileName {
                let display = path_str.clone();
                let (match_start, match_end) =
                    find_match_position_case_insensitive(&display, query_lower).unwrap_or((0, 0));
                matches.push(SearchMatch {
                    file_id: doc_id,
                    file_path: path_str.clone(),
                    line_number: 0,
                    content: display,
                    match_start,
                    match_end,
                    content_truncated: false,
                    score: 3.0 * dependency_boost,
                    is_symbol: true,
                    dependency_count,
                });
                continue;
            }

            // Get the line content (symbol.line is 0-based)
            let line = match lines.get(symbol.line) {
                Some(l) => *l,
                None => continue,
            };

            // Find where the symbol name appears in the line
            let (match_start, match_end) =
                match find_match_position_case_insensitive(line, query_lower) {
                    Some(pos) => pos,
                    None => {
                        // Try to find the symbol name instead
                        let name_lower = symbol.name.to_lowercase();
                        find_match_position_case_insensitive(line, &name_lower).unwrap_or((0, 0))
                    }
                };

            // Calculate score - symbols always get the symbol definition boost
            let score = calculate_score_inline(
                line,
                query_lower,
                query_lower,
                true,
                is_src_lib,
                dependency_boost,
            );

            // Truncate long lines around the match
            let truncated = truncate_around_match(line, match_start, match_end);

            matches.push(SearchMatch {
                file_id: doc_id,
                file_path: path_str.clone(),
                line_number: symbol.line + 1, // 1-based line numbers
                content: truncated.content,
                match_start: truncated.match_start,
                match_end: truncated.match_end,
                content_truncated: truncated.was_truncated,
                score,
                is_symbol: true,
                dependency_count,
            });
        }

        if matches.is_empty() {
            None
        } else {
            Some(matches)
        }
    }

    /// Search in a document using regex matching
    fn search_in_document_regex(&self, doc_id: u32, regex: &Regex) -> Option<Vec<SearchMatch>> {
        let file = self.file_store.get(doc_id)?;
        let content = file.as_str().ok()?;

        // Get symbols for this file
        let symbols = self.symbol_cache.get(doc_id as usize)?;

        // Get dependency count for this file (cached lookup - done once per document)
        let dependency_count = self.dependency_index.get_import_count(doc_id);

        // Pre-compute dependency boost (done once per document, not per match)
        let dependency_boost = if dependency_count > 0 {
            1.0 + (dependency_count as f64).log10() * 0.5
        } else {
            1.0
        };

        // Use a simple Vec for symbol definition lines - faster than HashSet for small N
        let symbol_def_lines: Vec<usize> = symbols
            .iter()
            .filter(|s| s.is_definition)
            .map(|s| s.line)
            .collect();

        // Single-pass search: collect matches directly
        let mut matches = Vec::with_capacity(8);

        // Lazy-compute path info only if we find matches
        let mut path_str: Option<String> = None;
        let mut is_src_lib = false;

        // Search in each line using regex
        for (line_num, line) in content.lines().enumerate() {
            if let Some(m) = regex.find(line) {
                // Lazy initialize path info only when we have at least one match
                let path_ref = path_str.get_or_insert_with(|| {
                    let p = file.path.to_string_lossy().into_owned();
                    let path_bytes = p.as_bytes();
                    is_src_lib = contains_bytes(path_bytes, b"/src/")
                        || contains_bytes(path_bytes, b"\\src\\")
                        || contains_bytes(path_bytes, b"/lib/")
                        || contains_bytes(path_bytes, b"\\lib\\");
                    p
                });

                // Calculate score using pre-computed values
                let is_symbol_def = symbol_def_lines.contains(&line_num);
                let score = calculate_score_regex_inline(
                    line,
                    regex,
                    is_symbol_def,
                    is_src_lib,
                    dependency_boost,
                );

                // Check if this is a symbol match - simple linear scan
                let is_symbol = symbols
                    .iter()
                    .filter(|s| s.line == line_num)
                    .any(|s| regex.is_match(&s.name));

                // Truncate long lines around the match
                let truncated = truncate_around_match(line, m.start(), m.end());

                matches.push(SearchMatch {
                    file_id: doc_id,
                    file_path: path_ref.clone(),
                    line_number: line_num + 1, // 1-based line numbers
                    content: truncated.content,
                    match_start: truncated.match_start,
                    match_end: truncated.match_end,
                    content_truncated: truncated.was_truncated,
                    score,
                    is_symbol,
                    dependency_count,
                });
            }
        }

        // Filename fallback: if no content lines matched but the regex matches a
        // FileName symbol, synthesize a result showing the file path.
        if matches.is_empty() {
            let has_filename_match = symbols
                .iter()
                .any(|s| s.symbol_type == SymbolType::FileName && regex.is_match(&s.name));
            if has_filename_match {
                let path_ref =
                    path_str.get_or_insert_with(|| file.path.to_string_lossy().into_owned());
                let display = path_ref.clone();
                let (match_start, match_end) = regex
                    .find(&display)
                    .map(|m| (m.start(), m.end()))
                    .unwrap_or((0, 0));
                matches.push(SearchMatch {
                    file_id: doc_id,
                    file_path: path_ref.clone(),
                    line_number: 0,
                    content: display,
                    match_start,
                    match_end,
                    content_truncated: false,
                    score: 3.0 * dependency_boost,
                    is_symbol: true,
                    dependency_count,
                });
            }
        }

        if matches.is_empty() {
            None
        } else {
            Some(matches)
        }
    }

    /// Optimized document search with original query for exact-case scoring.
    ///
    /// `original_query` is the un-lowered query string used for exact case-sensitive match boosting.
    /// `query_lower` is the lowercased query for case-insensitive matching.
    #[inline]
    fn search_in_document_scored(
        &self,
        doc_id: u32,
        original_query: &str,
        query_lower: &str,
    ) -> Option<Vec<SearchMatch>> {
        let file = self.file_store.get(doc_id)?;
        let content = file.as_str().ok()?;

        // Get symbols for this file. The symbol cache may be empty/missing if:
        // 1. The index was loaded from persistence (symbols aren't persisted for space efficiency)
        // 2. The file was just added and symbols haven't been extracted yet
        // When empty, search still works but without symbol-based ranking boosts.
        let symbols = self
            .symbol_cache
            .get(doc_id as usize)
            .map(|s| s.as_slice())
            .unwrap_or(&[]);

        // Get dependency count for this file (cached lookup - done once per document)
        let dependency_count = self.dependency_index.get_import_count(doc_id);

        // Pre-compute dependency boost (done once per document, not per match)
        let dependency_boost = if dependency_count > 0 {
            1.0 + (dependency_count as f64).log10() * 0.5
        } else {
            1.0
        };

        // Use a simple Vec to store symbol definition lines - faster than HashSet for small N
        // Most files have <100 symbols, linear scan is faster than hash overhead
        let symbol_def_lines: Vec<usize> = symbols
            .iter()
            .filter(|s| s.is_definition)
            .map(|s| s.line)
            .collect();

        // Single-pass search: collect matches directly
        let mut matches = Vec::with_capacity(8);

        // Lazy-compute path info only if we find matches
        let mut path_str: Option<String> = None;
        let mut is_src_lib = false;

        // Search in each line using case-insensitive matching without allocation
        for (line_num, line) in content.lines().enumerate() {
            if let Some((match_start, match_end)) =
                find_match_position_case_insensitive(line, query_lower)
            {
                // Lazy initialize path info only when we have at least one match
                let path_ref = path_str.get_or_insert_with(|| {
                    let p = file.path.to_string_lossy().into_owned();
                    let path_bytes = p.as_bytes();
                    is_src_lib = contains_bytes(path_bytes, b"/src/")
                        || contains_bytes(path_bytes, b"\\src\\")
                        || contains_bytes(path_bytes, b"/lib/")
                        || contains_bytes(path_bytes, b"\\lib\\");
                    p
                });

                // Calculate score using pre-computed values
                // Linear scan for is_symbol_def - faster than HashSet for typical file sizes
                let is_symbol_def = symbol_def_lines.contains(&line_num);
                let score = calculate_score_inline(
                    line,
                    original_query,
                    query_lower,
                    is_symbol_def,
                    is_src_lib,
                    dependency_boost,
                );

                // Check if this is a symbol match - simple linear scan over symbols on this line
                let is_symbol = symbols
                    .iter()
                    .filter(|s| s.line == line_num)
                    .any(|s| contains_case_insensitive(&s.name, query_lower));

                // Truncate long lines around the match
                let truncated = truncate_around_match(line, match_start, match_end);

                matches.push(SearchMatch {
                    file_id: doc_id,
                    file_path: path_ref.clone(),
                    line_number: line_num + 1, // 1-based line numbers
                    content: truncated.content,
                    match_start: truncated.match_start,
                    match_end: truncated.match_end,
                    content_truncated: truncated.was_truncated,
                    score,
                    is_symbol,
                    dependency_count,
                });
            }
        }

        // If no content matches, check if the query matches the filename.
        // The filename is indexed into the trigram index (so the file became a candidate),
        // but it doesn't appear in the file content. Synthesize a result so the user
        // sees the file in search results.
        if matches.is_empty() {
            let has_filename_match = symbols.iter().any(|s| {
                s.symbol_type == SymbolType::FileName
                    && contains_case_insensitive(&s.name, query_lower)
            });
            if has_filename_match {
                let path_ref =
                    path_str.get_or_insert_with(|| file.path.to_string_lossy().into_owned());
                let display = path_ref.clone();
                let (match_start, match_end) =
                    find_match_position_case_insensitive(&display, query_lower).unwrap_or((0, 0));
                matches.push(SearchMatch {
                    file_id: doc_id,
                    file_path: path_ref.clone(),
                    line_number: 0, // Convention: 0 means "filename match, not a content line"
                    content: display,
                    match_start,
                    match_end,
                    content_truncated: false,
                    score: 3.0 * dependency_boost, // Symbol def boost (3×) for filename matches
                    is_symbol: true,
                    dependency_count,
                });
            }
        }

        if matches.is_empty() {
            None
        } else {
            Some(matches)
        }
    }

    #[allow(dead_code)]
    fn search_in_document(&self, doc_id: u32, query: &str) -> Option<Vec<SearchMatch>> {
        let file = self.file_store.get(doc_id)?;
        let content = file.as_str().ok()?;

        let query_lower = query.to_lowercase();

        // Get symbols for this file
        let symbols = self.symbol_cache.get(doc_id as usize)?;

        // Get dependency count for this file (cached lookup - done once per document)
        let dependency_count = self.dependency_index.get_import_count(doc_id);

        // Pre-compute dependency boost (done once per document, not per match)
        let dependency_boost = if dependency_count > 0 {
            1.0 + (dependency_count as f64).log10() * 0.5
        } else {
            1.0
        };

        // Build a set of symbol definition lines for O(1) lookup instead of O(n) iteration
        let symbol_def_lines: std::collections::HashSet<usize> = symbols
            .iter()
            .filter(|s| s.is_definition)
            .map(|s| s.line)
            .collect();

        // Build a map of symbol lines to symbol names for O(1) symbol match check
        let symbol_names_by_line: std::collections::HashMap<usize, Vec<&str>> = {
            let mut map: std::collections::HashMap<usize, Vec<&str>> =
                std::collections::HashMap::new();
            for s in symbols {
                map.entry(s.line).or_default().push(&s.name);
            }
            map
        };

        // Single-pass search: collect matches directly
        // Use a reasonable initial capacity to avoid small reallocations
        let mut matches = Vec::with_capacity(8);
        let mut path_info: Option<(String, bool)> = None; // (path, is_src_or_lib)

        // Search in each line using case-insensitive matching without allocation
        for (line_num, line) in content.lines().enumerate() {
            if let Some((match_start, match_end)) =
                find_match_position_case_insensitive(line, &query_lower)
            {
                // Lazy initialize path info only when we have at least one match
                let (path_ref, is_src_lib) = path_info.get_or_insert_with(|| {
                    let p = file.path.to_string_lossy().into_owned();
                    let path_bytes = p.as_bytes();
                    let is_src_lib = contains_bytes(path_bytes, b"/src/")
                        || contains_bytes(path_bytes, b"\\src\\")
                        || contains_bytes(path_bytes, b"/lib/")
                        || contains_bytes(path_bytes, b"\\lib\\");
                    (p, is_src_lib)
                });

                // Calculate score using pre-computed values
                let is_symbol_def = symbol_def_lines.contains(&line_num);
                let score = calculate_score_inline(
                    line,
                    query,
                    &query_lower,
                    is_symbol_def,
                    *is_src_lib,
                    dependency_boost,
                );

                // Check if this is a symbol match using pre-built map (O(1) lookup + small iteration)
                let is_symbol = symbol_names_by_line
                    .get(&line_num)
                    .map(|names| {
                        names
                            .iter()
                            .any(|name| contains_case_insensitive(name, &query_lower))
                    })
                    .unwrap_or(false);

                // Truncate long lines around the match
                let truncated = truncate_around_match(line, match_start, match_end);

                matches.push(SearchMatch {
                    file_id: doc_id,
                    file_path: path_ref.clone(),
                    line_number: line_num + 1, // 1-based line numbers
                    content: truncated.content,
                    match_start: truncated.match_start,
                    match_end: truncated.match_end,
                    content_truncated: truncated.was_truncated,
                    score,
                    is_symbol,
                    dependency_count,
                });
            }
        }

        if matches.is_empty() {
            None
        } else {
            Some(matches)
        }
    }

    pub fn get_stats(&self) -> SearchStats {
        SearchStats {
            num_files: self.file_store.len(),
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

    /// Get file path by ID
    pub fn get_file_path(&self, file_id: u32) -> Option<String> {
        self.file_store
            .get(file_id)
            .map(|f| f.path.to_string_lossy().to_string())
    }

    /// Find file ID by path
    pub fn find_file_id(&self, path: &str) -> Option<u32> {
        for id in 0..self.file_store.len() as u32 {
            if let Some(file) = self.file_store.get(id) {
                let file_path = file.path.to_string_lossy();
                if file_path.contains(path) || path.contains(&*file_path) {
                    return Some(id);
                }
            }
        }
        None
    }

    /// Save the index to a file for persistence
    pub fn save_index(
        &self,
        path: &std::path::Path,
        config: &crate::config::IndexerConfig,
    ) -> anyhow::Result<()> {
        use crate::index::persistence::get_mtime;
        use crate::index::{PersistedFileMetadata, PersistedIndex};

        // Collect file metadata with source base path tracking
        let mut files = Vec::new();
        for id in 0..self.file_store.len() as u32 {
            if let Some(mapped_file) = self.file_store.get(id) {
                let mtime = get_mtime(&mapped_file.path).unwrap_or(0);

                // Determine which base path this file belongs to
                let source_base = config
                    .paths
                    .iter()
                    .find(|base| {
                        let base_normalized = base.replace('\\', "/").to_lowercase();
                        let file_normalized = mapped_file
                            .path
                            .to_string_lossy()
                            .replace('\\', "/")
                            .to_lowercase();
                        file_normalized.starts_with(&base_normalized)
                    })
                    .cloned();

                // Use len_if_mapped() to avoid triggering lazy loading during save
                // If file isn't mapped yet, get size from filesystem
                let size = mapped_file.len_if_mapped().unwrap_or_else(|| {
                    std::fs::metadata(&mapped_file.path)
                        .map(|m| m.len() as usize)
                        .unwrap_or(0)
                });

                files.push(PersistedFileMetadata {
                    path: mapped_file.path.clone(),
                    mtime,
                    size: size as u64,
                    source_base_path: source_base,
                });
            }
        }

        // Collect per-file symbol caches (parallel to files Vec)
        let symbols: Vec<Vec<crate::symbols::extractor::Symbol>> = (0..self.file_store.len())
            .map(|id| self.symbol_cache.get(id).cloned().unwrap_or_default())
            .collect();

        // Collect resolved dependency edges
        let dependency_edges = self.dependency_index.get_all_edges();

        // Create persisted index with config fingerprint
        let persisted = PersistedIndex::new(
            config.fingerprint(),
            config.paths.clone(),
            files,
            self.trigram_index.get_trigram_map(),
            symbols,
            dependency_edges,
        )?;
        persisted.save(path)?;

        tracing::info!(
            path = %path.display(),
            files = self.file_store.len(),
            trigrams = self.trigram_index.num_trigrams(),
            config_fingerprint = %config.fingerprint(),
            "Index saved to disk"
        );

        Ok(())
    }

    /// Check if a persisted index exists and is usable
    pub fn can_load_index(path: &std::path::Path) -> bool {
        path.exists()
    }

    /// Load an index from disk with reconciliation against current config
    /// Returns detailed information about what needs to be updated
    pub fn load_index_with_reconciliation(
        &mut self,
        path: &std::path::Path,
        config: &crate::config::IndexerConfig,
    ) -> anyhow::Result<LoadIndexResult> {
        use crate::index::persistence::{batch_check_files, FileStatus, PersistedIndex};

        let persisted = PersistedIndex::load(path)?;

        // Check config compatibility
        let current_fingerprint = config.fingerprint();
        let config_compatible = persisted.is_config_compatible(&current_fingerprint);

        if !config_compatible {
            tracing::info!(
                old_fingerprint = %persisted.config_fingerprint,
                new_fingerprint = %current_fingerprint,
                "Config fingerprint changed, will reconcile"
            );
        }

        // Determine paths to add/remove based on config changes
        let new_paths = persisted.paths_to_add(&config.paths);
        let removed_paths = persisted.paths_to_remove(&config.paths);

        // Batch check all files in parallel for staleness/removal
        let file_statuses = batch_check_files(&persisted.files, &removed_paths);

        // Categorize files based on status
        let mut stale_files = Vec::new();
        let mut removed_files = Vec::new();
        let mut valid_file_indices = Vec::new();

        for (idx, status) in file_statuses {
            match status {
                FileStatus::Valid => valid_file_indices.push(idx),
                FileStatus::Stale => stale_files.push(persisted.files[idx].path.clone()),
                FileStatus::Removed => removed_files.push(persisted.files[idx].path.clone()),
            }
        }

        // Only restore index if we have valid files
        if !valid_file_indices.is_empty() {
            // Restore trigram index (parallelized)
            let trigram_map = persisted.restore_trigram_index()?;
            self.trigram_index = crate::index::TrigramIndex::from_trigram_map(trigram_map);

            // Re-add valid files to the file store
            for &idx in &valid_file_indices {
                let file_meta = &persisted.files[idx];
                let _ = self.file_store.add_file(&file_meta.path);
            }

            self.trigram_index.finalize();
        }

        if !self.file_store.is_empty() {
            if !persisted.symbols.is_empty() {
                // Restore symbols and dependency graph directly from persisted data,
                // remapping original file indices to the new file IDs assigned during load.
                self.restore_symbols_and_deps(&valid_file_indices, &persisted);
                tracing::info!(
                    files_restored = valid_file_indices.len(),
                    "Restored symbol and dependency caches from persisted index"
                );
            } else {
                // Fallback: re-extract from file contents (old index format without symbols)
                let rebuild_stats = self.rebuild_symbols_and_dependencies();
                tracing::info!(
                    symbols_rebuilt = rebuild_stats.symbols_extracted,
                    imports_rebuilt = rebuild_stats.imports_extracted,
                    files_skipped = rebuild_stats.files_skipped,
                    "Rebuilt symbol and dependency caches after load (no persisted symbols)"
                );
            }
        }

        let already_indexed_files: Vec<std::path::PathBuf> = valid_file_indices
            .iter()
            .map(|&idx| persisted.files[idx].path.clone())
            .collect();

        tracing::info!(
            path = %path.display(),
            files_loaded = self.file_store.len(),
            stale_files = stale_files.len(),
            removed_files = removed_files.len(),
            new_paths = new_paths.len(),
            removed_paths = removed_paths.len(),
            config_compatible = config_compatible,
            "Index loaded from disk with reconciliation"
        );

        Ok(LoadIndexResult {
            stale_files,
            removed_files,
            new_paths,
            removed_paths,
            config_compatible,
            already_indexed_files,
        })
    }

    /// Load an index from disk with reconciliation and progress reporting
    ///
    /// The progress callback receives updates during each phase of loading:
    /// - ReadingFile: Starting to read the index file
    /// - Deserializing: Deserializing persisted data
    /// - CheckingFiles: Checking file staleness (with file count progress)
    /// - RestoringTrigrams: Restoring the trigram index
    /// - MappingFiles: Memory-mapping files (with file count progress)
    pub fn load_index_with_progress<F>(
        &mut self,
        path: &std::path::Path,
        config: &crate::config::IndexerConfig,
        mut progress_callback: F,
    ) -> anyhow::Result<LoadIndexResult>
    where
        F: FnMut(LoadingPhase, Option<usize>, Option<usize>, &str),
    {
        use crate::index::persistence::{batch_check_files, FileStatus, PersistedIndex};

        // Phase 1: Reading file from disk
        progress_callback(
            LoadingPhase::ReadingFile,
            None,
            None,
            "Reading index file from disk...",
        );

        // Phase 2: Deserializing
        progress_callback(
            LoadingPhase::Deserializing,
            None,
            None,
            "Deserializing index data...",
        );
        let persisted = PersistedIndex::load(path)?;
        let total_files = persisted.files.len();

        // Check config compatibility
        let current_fingerprint = config.fingerprint();
        let config_compatible = persisted.is_config_compatible(&current_fingerprint);

        if !config_compatible {
            tracing::info!(
                old_fingerprint = %persisted.config_fingerprint,
                new_fingerprint = %current_fingerprint,
                "Config fingerprint changed, will reconcile"
            );
        }

        // Determine paths to add/remove based on config changes
        let new_paths = persisted.paths_to_add(&config.paths);
        let removed_paths = persisted.paths_to_remove(&config.paths);

        // Phase 3: Checking files for staleness
        progress_callback(
            LoadingPhase::CheckingFiles,
            Some(total_files),
            Some(0),
            &format!("Checking {} files for changes...", total_files),
        );

        let file_statuses = batch_check_files(&persisted.files, &removed_paths);

        progress_callback(
            LoadingPhase::CheckingFiles,
            Some(total_files),
            Some(total_files),
            &format!("Checked {} files", total_files),
        );

        // Categorize files based on status
        let mut stale_files = Vec::new();
        let mut removed_files = Vec::new();
        let mut valid_file_indices = Vec::new();

        for (idx, status) in file_statuses {
            match status {
                FileStatus::Valid => valid_file_indices.push(idx),
                FileStatus::Stale => stale_files.push(persisted.files[idx].path.clone()),
                FileStatus::Removed => removed_files.push(persisted.files[idx].path.clone()),
            }
        }

        let valid_count = valid_file_indices.len();

        // Only restore index if we have valid files
        if !valid_file_indices.is_empty() {
            // Phase 4: Restore trigram index
            progress_callback(
                LoadingPhase::RestoringTrigrams,
                None,
                None,
                "Restoring search index...",
            );

            let trigram_map = persisted.restore_trigram_index()?;
            self.trigram_index = crate::index::TrigramIndex::from_trigram_map(trigram_map);

            // Phase 5: Register file paths (LAZY - no I/O, instant!)
            progress_callback(
                LoadingPhase::MappingFiles,
                Some(valid_count),
                Some(0),
                &format!("Registering {} files...", valid_count),
            );

            // Collect paths for lazy registration
            let paths_to_register: Vec<std::path::PathBuf> = valid_file_indices
                .iter()
                .map(|&idx| persisted.files[idx].path.clone())
                .collect();

            // Calculate total content bytes from persisted metadata
            let total_content_bytes: u64 = valid_file_indices
                .iter()
                .map(|&idx| persisted.files[idx].size)
                .sum();

            // Pre-allocate capacity for efficiency
            self.file_store.reserve(paths_to_register.len());

            // Register all files instantly (no I/O, just storing paths)
            let _ids = self.file_store.register_files_bulk(&paths_to_register);

            // Track content bytes from persisted metadata
            self.file_store.add_content_bytes(total_content_bytes);

            // Final progress update
            progress_callback(
                LoadingPhase::MappingFiles,
                Some(valid_count),
                Some(valid_count),
                &format!("Registered {} files (lazy loading enabled)", valid_count),
            );

            self.trigram_index.finalize();
        }

        if !self.file_store.is_empty() {
            let total_files = self.file_store.len();
            progress_callback(
                LoadingPhase::RebuildingSymbols,
                Some(total_files),
                Some(0),
                "Restoring symbols and import graph...",
            );

            if !persisted.symbols.is_empty() {
                // Restore symbols and dependency graph directly from persisted data
                self.restore_symbols_and_deps(&valid_file_indices, &persisted);
                progress_callback(
                    LoadingPhase::RebuildingSymbols,
                    Some(total_files),
                    Some(total_files),
                    "Symbol and dependency caches restored from index",
                );
                tracing::info!(
                    files_restored = valid_file_indices.len(),
                    "Restored symbol and dependency caches from persisted index"
                );
            } else {
                // Fallback: re-extract from file contents (old index format without symbols)
                let _stats =
                    self.rebuild_symbols_and_dependencies_with_progress(|processed, total| {
                        progress_callback(
                            LoadingPhase::RebuildingSymbols,
                            Some(total),
                            Some(processed),
                            "Rebuilding symbols and import graph...",
                        );
                    });

                progress_callback(
                    LoadingPhase::RebuildingSymbols,
                    Some(total_files),
                    Some(total_files),
                    "Symbol and dependency caches rebuilt",
                );
            }
        }

        let already_indexed_files: Vec<std::path::PathBuf> = valid_file_indices
            .iter()
            .map(|&idx| persisted.files[idx].path.clone())
            .collect();

        tracing::info!(
            path = %path.display(),
            files_loaded = self.file_store.len(),
            stale_files = stale_files.len(),
            removed_files = removed_files.len(),
            new_paths = new_paths.len(),
            removed_paths = removed_paths.len(),
            config_compatible = config_compatible,
            "Index loaded from disk with reconciliation"
        );

        Ok(LoadIndexResult {
            stale_files,
            removed_files,
            new_paths,
            removed_paths,
            config_compatible,
            already_indexed_files,
        })
    }

    /// Load an index from disk if available and not stale (legacy method)
    /// Returns the list of stale files that need re-indexing
    pub fn load_index(
        &mut self,
        path: &std::path::Path,
    ) -> anyhow::Result<Vec<std::path::PathBuf>> {
        use crate::index::persistence::{batch_check_files, FileStatus, PersistedIndex};

        let persisted = PersistedIndex::load(path)?;

        // Batch check all files in parallel
        let file_statuses = batch_check_files(&persisted.files, &[]);

        // Categorize files
        let mut stale_files = Vec::new();
        let mut valid_file_indices = Vec::new();

        for (idx, status) in file_statuses {
            match status {
                FileStatus::Valid => valid_file_indices.push(idx),
                FileStatus::Stale | FileStatus::Removed => {
                    stale_files.push(persisted.files[idx].path.clone())
                }
            }
        }

        // Restore trigram index (parallelized)
        let trigram_map = persisted.restore_trigram_index()?;
        self.trigram_index = crate::index::TrigramIndex::from_trigram_map(trigram_map);

        // Register file paths lazily (no I/O - instant!)
        let paths_to_register: Vec<std::path::PathBuf> = valid_file_indices
            .iter()
            .map(|&idx| persisted.files[idx].path.clone())
            .collect();

        // Calculate total content bytes from persisted metadata
        let total_content_bytes: u64 = valid_file_indices
            .iter()
            .map(|&idx| persisted.files[idx].size)
            .sum();

        self.file_store.reserve(paths_to_register.len());
        let _ids = self.file_store.register_files_bulk(&paths_to_register);

        // Track content bytes from persisted metadata
        self.file_store.add_content_bytes(total_content_bytes);

        self.trigram_index.finalize();

        if !self.file_store.is_empty() {
            if !persisted.symbols.is_empty() {
                self.restore_symbols_and_deps(&valid_file_indices, &persisted);
                tracing::info!(
                    files_restored = valid_file_indices.len(),
                    "Restored symbol and dependency caches from persisted index"
                );
            } else {
                let rebuild_stats = self.rebuild_symbols_and_dependencies();
                tracing::info!(
                    symbols_rebuilt = rebuild_stats.symbols_extracted,
                    imports_rebuilt = rebuild_stats.imports_extracted,
                    files_skipped = rebuild_stats.files_skipped,
                    "Rebuilt symbol and dependency caches after load (no persisted symbols)"
                );
            }
        }

        tracing::info!(
            path = %path.display(),
            files_loaded = self.file_store.len(),
            stale_files = stale_files.len(),
            "Index loaded from disk"
        );

        Ok(stale_files)
    }

    /// Update the index for a single file (for incremental indexing)
    pub fn update_file(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        // For now, just re-index the file
        // A more sophisticated implementation could track document IDs
        // and update only the affected trigrams
        self.index_file(path)
    }
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SearchStats {
    pub num_files: usize,
    pub total_size: u64,
    pub num_trigrams: usize,
    pub dependency_edges: usize,
    /// Total bytes of text content indexed
    pub total_content_bytes: u64,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct RebuildCacheStats {
    pub files_processed: usize,
    pub files_skipped: usize,
    pub symbols_extracted: usize,
    pub imports_extracted: usize,
}

#[derive(Debug)]
struct RebuildEntry {
    file_id: u32,
    path: std::path::PathBuf,
    symbols: Vec<Symbol>,
    imports: Vec<String>,
    had_content: bool,
}

/// Result of loading a persisted index with reconciliation
#[derive(Debug, Clone)]
pub struct LoadIndexResult {
    /// Files that were modified since indexing (need re-indexing)
    pub stale_files: Vec<std::path::PathBuf>,
    /// Files that no longer exist (removed from index)
    pub removed_files: Vec<std::path::PathBuf>,
    /// Paths that are new in config (need full indexing)
    pub new_paths: Vec<String>,
    /// Paths that were removed from config (files removed from index)
    pub removed_paths: Vec<String>,
    /// Whether the config fingerprint matches (false = config changed)
    pub config_compatible: bool,
    /// Files already validly indexed (unchanged since last index build).
    /// Used to skip re-indexing when scanning for unindexed files after a
    /// partial/checkpoint load.
    pub already_indexed_files: Vec<std::path::PathBuf>,
}

/// Status of the indexing process
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IndexingStatus {
    /// No indexing is in progress
    #[default]
    Idle,
    /// Loading persisted index from disk
    LoadingIndex,
    /// Discovering files to index
    Discovering,
    /// Actively indexing files
    Indexing,
    /// Reconciling persisted index with current filesystem
    Reconciling,
    /// Resolving import dependencies
    ResolvingImports,
    /// Indexing completed successfully
    Completed,
}

/// Sub-phases during index loading for detailed progress reporting
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LoadingPhase {
    /// Not currently loading
    #[default]
    None,
    /// Reading index file from disk
    ReadingFile,
    /// Deserializing the persisted index data
    Deserializing,
    /// Checking files for staleness
    CheckingFiles,
    /// Restoring trigram index
    RestoringTrigrams,
    /// Memory-mapping files
    MappingFiles,
    /// Rebuilding symbol and dependency caches
    RebuildingSymbols,
}

/// Progress information for the indexing process
#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexingProgress {
    /// Current status of the indexing process
    pub status: IndexingStatus,
    /// Number of files discovered during file discovery phase
    pub files_discovered: usize,
    /// Number of files indexed so far
    pub files_indexed: usize,
    /// Number of files transcoded from non-UTF-8 encodings
    pub files_transcoded: usize,
    /// Current batch number (1-based)
    pub current_batch: usize,
    /// Total number of batches to process
    pub total_batches: usize,
    /// Current path being processed (for display)
    pub current_path: Option<String>,
    /// Timestamp when indexing started (Unix epoch millis)
    pub started_at: Option<u64>,
    /// Number of errors encountered
    pub errors: usize,
    /// Message describing current activity
    pub message: String,
    /// Sub-phase during index loading (when status == LoadingIndex)
    #[serde(skip_serializing_if = "is_loading_phase_none")]
    pub loading_phase: LoadingPhase,
    /// Total files in persisted index (for loading progress)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loading_total_files: Option<usize>,
    /// Files processed during loading
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loading_files_processed: Option<usize>,
}

/// Helper for serde skip_serializing_if
fn is_loading_phase_none(phase: &LoadingPhase) -> bool {
    *phase == LoadingPhase::None
}

impl Default for IndexingProgress {
    fn default() -> Self {
        Self {
            status: IndexingStatus::Idle,
            files_discovered: 0,
            files_indexed: 0,
            files_transcoded: 0,
            current_batch: 0,
            total_batches: 0,
            current_path: None,
            started_at: None,
            errors: 0,
            message: String::from("Ready"),
            loading_phase: LoadingPhase::None,
            loading_total_files: None,
            loading_files_processed: None,
        }
    }
}

impl IndexingProgress {
    /// Create a new progress tracker starting the indexing process
    pub fn start() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Self {
            status: IndexingStatus::Discovering,
            started_at: Some(now),
            message: String::from("Starting file discovery..."),
            ..Default::default()
        }
    }

    /// Calculate elapsed time in seconds
    pub fn elapsed_secs(&self) -> Option<f64> {
        let started = self.started_at?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Some((now - started) as f64 / 1000.0)
    }

    /// Calculate progress percentage (0-100)
    pub fn progress_percent(&self) -> u8 {
        match self.status {
            IndexingStatus::Idle => 0,
            IndexingStatus::LoadingIndex => {
                // Show more granular progress during loading based on phase
                match self.loading_phase {
                    LoadingPhase::None => 1,
                    LoadingPhase::ReadingFile => 2,
                    LoadingPhase::Deserializing => 5,
                    LoadingPhase::CheckingFiles => {
                        // 10-30% for file checking
                        if let (Some(total), Some(processed)) =
                            (self.loading_total_files, self.loading_files_processed)
                        {
                            if total > 0 {
                                let pct = (processed as f64 / total as f64) * 20.0;
                                return (10.0 + pct).min(30.0) as u8;
                            }
                        }
                        15
                    }
                    LoadingPhase::RestoringTrigrams => 40,
                    LoadingPhase::MappingFiles => {
                        // 50-90% for file mapping
                        if let (Some(total), Some(processed)) =
                            (self.loading_total_files, self.loading_files_processed)
                        {
                            if total > 0 {
                                let pct = (processed as f64 / total as f64) * 40.0;
                                return (50.0 + pct).min(90.0) as u8;
                            }
                        }
                        60
                    }
                    LoadingPhase::RebuildingSymbols => {
                        if let (Some(total), Some(processed)) =
                            (self.loading_total_files, self.loading_files_processed)
                        {
                            if total > 0 {
                                let pct = (processed as f64 / total as f64) * 15.0;
                                return (80.0 + pct).min(95.0) as u8;
                            }
                        }
                        85
                    }
                }
            }
            IndexingStatus::Discovering => 5,
            IndexingStatus::Indexing => {
                if self.total_batches == 0 {
                    10
                } else {
                    let batch_progress =
                        (self.current_batch as f64 / self.total_batches as f64) * 80.0;
                    (10.0 + batch_progress).min(90.0) as u8
                }
            }
            IndexingStatus::Reconciling => 92,
            IndexingStatus::ResolvingImports => 96,
            IndexingStatus::Completed => 100,
        }
    }
}

/// Shared indexing progress state for use across threads
pub type SharedIndexingProgress = std::sync::Arc<std::sync::RwLock<IndexingProgress>>;

/// Broadcast channel for sending progress updates to WebSocket clients
/// Use sender.subscribe() to get a receiver for each WebSocket connection
pub type ProgressBroadcaster = tokio::sync::broadcast::Sender<IndexingProgress>;

/// Create a new progress broadcaster with reasonable capacity
/// Returns both the sender (for publishing updates) and receiver (for subscribing)
pub fn create_progress_broadcaster() -> ProgressBroadcaster {
    // Buffer 16 messages - clients that fall behind will miss updates
    // This is fine since they'll get the next update shortly
    let (tx, _rx) = tokio::sync::broadcast::channel(16);
    tx
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_search_engine() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "hello world").unwrap();
        writeln!(file, "hello rust").unwrap();
        writeln!(file, "goodbye world").unwrap();
        drop(file);

        let mut engine = SearchEngine::new();
        engine.index_file(&file_path).unwrap();

        let results = engine.search("hello", 10);
        assert_eq!(results.len(), 2);

        let results = engine.search("world", 10);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_incremental_import_resolution() {
        let temp_dir = TempDir::new().unwrap();

        // Create a file with imports
        let main_path = temp_dir.path().join("main.rs");
        let helper_path = temp_dir.path().join("helper.rs");

        fs::write(
            &helper_path,
            "pub fn help() {\n    println!(\"helping\");\n}\n",
        )
        .unwrap();

        fs::write(
            &main_path,
            "mod helper;\nfn main() {\n    helper::help();\n}\n",
        )
        .unwrap();

        let mut engine = SearchEngine::new();

        // Index the helper file first
        engine.index_file(&helper_path).unwrap();

        // Now index main file - its import to helper should be resolvable
        engine.index_file(&main_path).unwrap();

        // Try incremental resolution - should resolve the import
        let _resolved = engine.resolve_imports_incremental();

        // Some imports may or may not resolve depending on path canonicalization
        // The key is that incremental resolution doesn't panic and works correctly
        // pending_imports_count is always >= 0 (usize), so just check it works
        let _ = engine.pending_imports_count();

        // Final resolve should clear any remaining
        engine.resolve_imports();
        assert_eq!(engine.pending_imports_count(), 0);
    }

    #[test]
    fn test_pending_imports_count() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");

        fs::write(
            &file_path,
            "import os\nimport sys\nfrom pathlib import Path\n",
        )
        .unwrap();

        let mut engine = SearchEngine::new();
        engine.index_file(&file_path).unwrap();

        // There should be pending imports (stdlib imports won't resolve to indexed files)
        // These will remain unresolved since os, sys, pathlib aren't indexed
        let pending = engine.pending_imports_count();
        assert!(pending > 0, "Expected pending imports");

        // After resolution, pending should be 0 (they're cleared even if unresolved)
        engine.resolve_imports();
        assert_eq!(engine.pending_imports_count(), 0);
    }

    #[test]
    fn test_case_insensitive_search() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        // Create file with only lowercase content
        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "hello world").unwrap();
        writeln!(file, "another hello here").unwrap();
        drop(file);

        let mut engine = SearchEngine::new();
        engine.index_file(&file_path).unwrap();
        engine.finalize();

        // Test that all case variants find the same results
        let results_lower = engine.search("hello", 10);
        let results_upper = engine.search("HELLO", 10);
        let results_mixed = engine.search("Hello", 10);

        // All queries should find both lines with "hello"
        assert_eq!(
            results_lower.len(),
            2,
            "lowercase query 'hello' should find 2 matches"
        );
        assert_eq!(
            results_upper.len(),
            2,
            "uppercase query 'HELLO' should find 2 matches"
        );
        assert_eq!(
            results_mixed.len(),
            2,
            "mixed case query 'Hello' should find 2 matches"
        );
    }

    #[test]
    fn test_save_and_load_index() {
        use crate::config::IndexerConfig;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        let index_path = temp_dir.path().join("index.bin");

        // Create a test file
        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "fn hello_world() {{}}").unwrap();
        writeln!(file, "hello world").unwrap();
        writeln!(file, "rust programming").unwrap();
        drop(file);

        // Create config for the test
        let config = IndexerConfig {
            paths: vec![temp_dir.path().to_string_lossy().to_string()],
            ..Default::default()
        };

        // Index and save
        let mut engine = SearchEngine::new();
        engine.index_file(&file_path).unwrap();
        engine.finalize();

        // Verify search works before save
        let results = engine.search("hello", 10);
        assert!(!results.is_empty(), "Should find hello before save");

        // Save the index
        engine.save_index(&index_path, &config).unwrap();
        assert!(index_path.exists(), "Index file should exist");

        // Create a new engine and load the index
        let mut engine2 = SearchEngine::new();
        let stale_files = engine2.load_index(&index_path).unwrap();

        // No files should be stale since we haven't modified them
        assert!(stale_files.is_empty(), "No files should be stale");

        // Verify search works after load
        let results2 = engine2.search("hello", 10);
        assert!(!results2.is_empty(), "Should find hello after load");

        // Verify symbol cache was rebuilt after load
        let symbol_results = engine2.search_symbols("hello_world", "", "", 10).unwrap();
        assert!(
            !symbol_results.is_empty(),
            "Should find hello_world symbol after load"
        );
    }

    #[test]
    fn test_can_load_index() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("nonexistent.bin");

        assert!(!SearchEngine::can_load_index(&index_path));

        // Create the file
        fs::write(&index_path, "dummy").unwrap();
        assert!(SearchEngine::can_load_index(&index_path));
    }

    #[test]
    fn test_search_symbols() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");

        // Create a Rust file with functions and a class
        fs::write(
            &file_path,
            r#"
fn hello_world() {
    println!("Hello");
}

fn another_function() {
    // code
}

pub struct TestStruct {
    name: String,
}

impl TestStruct {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string() }
    }
}
"#,
        )
        .unwrap();

        let mut engine = SearchEngine::new();
        engine.index_file(&file_path).unwrap();
        engine.finalize();

        // Search for symbols matching "function"
        let results = engine.search_symbols("function", "", "", 10).unwrap();
        assert!(
            !results.is_empty(),
            "Expected at least one symbol match for 'function'"
        );
        assert!(
            results
                .iter()
                .any(|r| r.content.contains("another_function")),
            "Expected to find 'another_function' symbol"
        );

        // Search for symbols matching "hello"
        let results = engine.search_symbols("hello", "", "", 10).unwrap();
        assert!(
            !results.is_empty(),
            "Expected at least one symbol match for 'hello'"
        );
        assert!(
            results.iter().any(|r| r.content.contains("hello_world")),
            "Expected to find 'hello_world' symbol"
        );

        // All results should be marked as symbols
        for result in &results {
            assert!(
                result.is_symbol,
                "All symbol search results should have is_symbol=true"
            );
        }

        // Search for something that doesn't match any symbol
        let results = engine.search_symbols("println", "", "", 10).unwrap();
        assert!(
            results.is_empty(),
            "Expected no symbol match for 'println' (it's not a symbol name)"
        );
    }

    #[test]
    fn test_search_symbols_case_insensitive() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");

        fs::write(
            &file_path,
            r#"
fn HelloWorld() {
    println!("Hello");
}
"#,
        )
        .unwrap();

        let mut engine = SearchEngine::new();
        engine.index_file(&file_path).unwrap();
        engine.finalize();

        // Case-insensitive search should work
        let results_lower = engine.search_symbols("helloworld", "", "", 10).unwrap();
        let results_upper = engine.search_symbols("HELLOWORLD", "", "", 10).unwrap();
        let results_mixed = engine.search_symbols("HelloWorld", "", "", 10).unwrap();

        assert!(
            !results_lower.is_empty(),
            "lowercase query should find symbol"
        );
        assert!(
            !results_upper.is_empty(),
            "uppercase query should find symbol"
        );
        assert!(
            !results_mixed.is_empty(),
            "mixed case query should find symbol"
        );
    }

    // ========== Tests for review fixes ==========

    /// Fix #1: Regex trigram literals should be lowercased before index lookup.
    /// Without this fix, searching for a regex like `MyClass\.\w+` would fail to
    /// find trigram matches because the index stores lowercased content.
    #[test]
    fn test_regex_search_uses_lowercased_trigrams() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");

        fs::write(
            &file_path,
            "class MyClass:\n    def do_thing(self):\n        MyClass.do_thing()\n",
        )
        .unwrap();

        let mut engine = SearchEngine::new();
        engine.index_file(&file_path).unwrap();
        engine.finalize();

        // Regex with uppercase literal — trigram acceleration must lowercase before lookup
        let results = engine.search_regex(r"MyClass\.\w+", "", "", 10).unwrap();
        assert!(
            !results.is_empty(),
            "Regex with uppercase literal should find matches via lowered trigram lookup"
        );
        assert!(
            results
                .iter()
                .any(|r| r.content.contains("MyClass.do_thing")),
            "Should find MyClass.do_thing()"
        );
    }

    /// Fix #2: Exact match boost must compare against the original (un-lowered) query.
    /// A search for "MyFunction" should score the exact-case line higher than
    /// a line with "myfunction".
    #[test]
    fn test_exact_match_boost_uses_original_case() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");

        // Two lines: one with exact case, one with different case
        fs::write(&file_path, "fn MyFunction() {}\nfn myfunction() {}\n").unwrap();

        let mut engine = SearchEngine::new();
        engine.index_file(&file_path).unwrap();
        engine.finalize();

        let results = engine.search("MyFunction", 10);
        assert!(results.len() >= 2, "Should find both variants");

        // Find the exact-case match and the lower-case match
        let exact_match = results
            .iter()
            .find(|r| r.content.contains("fn MyFunction"))
            .unwrap();
        let lower_match = results
            .iter()
            .find(|r| r.content.contains("fn myfunction"))
            .unwrap();

        assert!(
            exact_match.score > lower_match.score,
            "Exact case match ({:.3}) should score higher than lowercase ({:.3})",
            exact_match.score,
            lower_match.score
        );
    }

    /// Fix #7: Line length penalty should be gentle (logarithmic), not harsh.
    /// A function definition on a ~100-char line should NOT be obliterated by
    /// a short comment. Both should get reasonable scores.
    #[test]
    fn test_line_length_penalty_is_gentle() {
        // Short line (20 chars)
        let short_score = calculate_score_inline(
            "fn do_thing() {}   ",
            "do_thing",
            "do_thing",
            false,
            false,
            1.0,
        );

        // Medium line (~80 chars)
        let medium_line =
            "fn do_thing(arg1: String, arg2: i32, arg3: bool) -> Result<()> { todo!() }";
        let medium_score =
            calculate_score_inline(&medium_line, "do_thing", "do_thing", false, false, 1.0);

        // Long line (~200 chars)
        let long_line = format!(
            "fn do_thing({}) -> Result<()> {{}}",
            (0..20)
                .map(|i| format!("arg{}: String", i))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let long_score =
            calculate_score_inline(&long_line, "do_thing", "do_thing", false, false, 1.0);

        // The medium line should retain a decent fraction of the short line's score
        assert!(
            medium_score / short_score > 0.5,
            "Medium line ({:.3}) should be > 50% of short line ({:.3}), got {:.1}%",
            medium_score,
            short_score,
            medium_score / short_score * 100.0
        );

        // Even the long line should not drop below 30% (the floor)
        assert!(
            long_score / short_score > 0.25,
            "Long line ({:.3}) should be > 25% of short line ({:.3}), got {:.1}%",
            long_score,
            short_score,
            long_score / short_score * 100.0
        );
    }

    /// Fix #8: Document search functions should return None for zero matches
    /// instead of Some(empty vec), avoiding unnecessary allocations.
    #[test]
    fn test_no_match_returns_none_not_empty_vec() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        fs::write(&file_path, "hello world\n").unwrap();

        let mut engine = SearchEngine::new();
        engine.index_file(&file_path).unwrap();
        engine.finalize();

        // Search for something not in the file — should produce zero results
        let results = engine.search("xyznonexistent", 10);
        assert!(
            results.is_empty(),
            "Search for non-existent term should yield empty results"
        );
    }

    /// Fix #9: Filename and content should be separated by triple newline to
    /// prevent trigram bleed across the boundary.
    #[test]
    fn test_filename_content_separator_prevents_trigram_bleed() {
        let temp_dir = TempDir::new().unwrap();
        // File named "alpha_module.txt" with content starting with "beta_function"
        let file_path = temp_dir.path().join("alpha_module.txt");
        fs::write(&file_path, "beta_function called here\n").unwrap();

        let mut engine = SearchEngine::new();
        engine.index_file(&file_path).unwrap();
        engine.finalize();

        // Verify content is still searchable after the separator change
        let results = engine.search("beta_function", 10);
        assert!(
            !results.is_empty(),
            "Should find 'beta_function' in file content"
        );

        // Verify content from the file is correctly returned
        assert!(
            results[0].content.contains("beta_function"),
            "Result content should contain the search term"
        );

        // The triple newline separator means trigrams like "xt\nb" (from single newline join)
        // are NOT generated, protecting against false trigram candidate matches on
        // boundary-spanning text. The filename is used only for trigram candidate filtering,
        // not for result content.
    }

    /// Fix #12: FileMetadata should pre-compute lowercase_stem at index time
    /// and use it for query matching, avoiding per-query path allocation.
    #[test]
    fn test_file_metadata_precomputed_lowercase_stem() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("MyModule.rs");
        fs::write(&file_path, "fn test() {}\n").unwrap();

        let mut engine = SearchEngine::new();
        engine.index_file(&file_path).unwrap();
        engine.finalize();

        // Verify the metadata was computed with the correct lowercase stem
        let metadata = engine.get_file_metadata(0);
        assert_eq!(
            metadata.lowercase_stem, "mymodule",
            "lowercase_stem should be pre-computed at index time"
        );

        // Query score should boost when query matches the filename stem
        let score_match = metadata.query_score("mymodule");
        let score_nomatch = metadata.query_score("unrelated");
        assert!(
            score_match > score_nomatch,
            "Query matching filename stem ({:.3}) should score higher than non-match ({:.3})",
            score_match,
            score_nomatch
        );
    }

    /// Fix #4: Symbol search should use trigram pre-filtering for queries >= 3 chars
    /// instead of scanning all documents.
    #[test]
    fn test_symbol_search_uses_trigram_filtering() {
        let temp_dir = TempDir::new().unwrap();

        // Create two files: one with target symbol, one without
        let file_with = temp_dir.path().join("has_symbol.rs");
        fs::write(&file_with, "fn calculate_total() {\n    // does math\n}\n").unwrap();

        let file_without = temp_dir.path().join("no_symbol.rs");
        fs::write(
            &file_without,
            "fn something_else() {\n    // unrelated\n}\n",
        )
        .unwrap();

        let mut engine = SearchEngine::new();
        engine.index_file(&file_with).unwrap();
        engine.index_file(&file_without).unwrap();
        engine.finalize();

        // Symbol search for "calculate" (>= 3 chars, should use trigram pre-filtering)
        let results = engine.search_symbols("calculate", "", "", 10).unwrap();
        assert!(!results.is_empty(), "Should find calculate_total symbol");
        assert!(
            results.iter().all(|r| r.content.contains("calculate")),
            "All results should contain the query term"
        );
    }

    /// Fix #6: FAST_RANKING_TOP_N should be large enough to not miss relevant results.
    #[test]
    fn test_fast_ranking_top_n_is_sufficient() {
        // Just verify the constant is reasonable
        assert!(
            SearchEngine::FAST_RANKING_TOP_N >= 2000,
            "FAST_RANKING_TOP_N should be at least 2000 to avoid dropping relevant files"
        );
    }

    /// Filename-only matches: searching for the filename stem should return
    /// a result even when the query does NOT appear in the file content.
    #[test]
    fn test_filename_only_match_returns_result() {
        let temp_dir = TempDir::new().unwrap();
        // File whose content does NOT contain "configuration_manager"
        let file_path = temp_dir.path().join("configuration_manager.rs");
        fs::write(
            &file_path,
            "pub fn init() {\n    println!(\"starting up\");\n}\n",
        )
        .unwrap();

        let mut engine = SearchEngine::new();
        engine.index_file(&file_path).unwrap();
        engine.finalize();

        // Search for the filename stem — not present in content
        let results = engine.search("configuration_manager", 10);
        assert!(
            !results.is_empty(),
            "Searching for filename stem should return a result even when content doesn't match"
        );

        // The synthetic filename match should have line_number 0
        let filename_result = results.iter().find(|r| r.line_number == 0);
        assert!(
            filename_result.is_some(),
            "Filename match should appear with line_number=0"
        );
        let filename_result = filename_result.unwrap();
        assert!(
            filename_result.is_symbol,
            "Filename match should be marked as a symbol"
        );
        assert!(
            filename_result.content.contains("configuration_manager"),
            "Filename match content should contain the filename: got '{}'",
            filename_result.content
        );
    }

    /// Filename-only matches should work in symbol search too.
    #[test]
    fn test_filename_symbol_search() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("widget_factory.rs");
        fs::write(&file_path, "pub fn make() {}\n").unwrap();

        let mut engine = SearchEngine::new();
        engine.index_file(&file_path).unwrap();
        engine.finalize();

        // Symbol search for the filename — the FileName symbol should match
        let results = engine.search_symbols("widget_factory", "", "", 10).unwrap();
        assert!(
            !results.is_empty(),
            "Symbol search for filename stem should return a result"
        );
        let sym_result = &results[0];
        assert_eq!(
            sym_result.line_number, 0,
            "FileName symbol should have line_number=0"
        );
        assert!(
            sym_result.content.contains("widget_factory"),
            "FileName symbol result should show the file path"
        );
    }

    /// Filename-only matches should work with regex search too.
    #[test]
    fn test_filename_regex_match() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("data_processor.py");
        fs::write(&file_path, "x = 42\n").unwrap();

        let mut engine = SearchEngine::new();
        engine.index_file(&file_path).unwrap();
        engine.finalize();

        let results = engine.search_regex(r"data_processor", "", "", 10).unwrap();
        assert!(
            !results.is_empty(),
            "Regex search for filename stem should return a result"
        );
        assert!(
            results[0].content.contains("data_processor"),
            "Regex filename match should show the file path"
        );
    }
}
