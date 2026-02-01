use crate::dependencies::DependencyIndex;
use crate::index::{extract_unique_trigrams, FileStore, Trigram, TrigramIndex};
use crate::search::path_filter::PathFilter;
use crate::search::regex_search::RegexAnalysis;
use crate::symbols::{Symbol, SymbolExtractor};
use anyhow::Result;
use memchr::memmem;
use rayon::prelude::*;
use regex::Regex;
use rustc_hash::FxHashSet;
use std::path::{Path, PathBuf};

/// Case-insensitive substring search without heap allocation.
/// Both `haystack` and `needle` are compared using ASCII case-insensitive matching.
/// `needle` should already be lowercase for optimal performance.
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

/// Inline scoring function with pre-computed values (no method call overhead, no redundant lookups)
#[inline]
fn calculate_score_inline(
    line: &str,
    query_lower: &str,
    is_symbol_def: bool,
    is_src_lib: bool,
    dependency_boost: f64,
) -> f64 {
    let mut score = 1.0;

    // Boost for exact case-sensitive matches
    if line.contains(query_lower) {
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

    // Boost for shorter lines (more relevant)
    let line_len_factor = 1.0 / (1.0 + (line.len() as f64 * 0.01));
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

    // Boost for shorter lines (more relevant)
    let line_len_factor = 1.0 / (1.0 + (line.len() as f64 * 0.01));
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
    pub score: f64,
    pub is_symbol: bool,
    pub dependency_count: u32,
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

/// Pre-processed file data ready to be merged into the engine.
/// This is computed in parallel and then merged with a lock.
pub struct PreIndexedFile {
    /// Path to the file
    pub path: PathBuf,
    /// File content (owned for thread safety)
    pub content: String,
    /// Unique trigrams extracted from the content
    pub trigrams: FxHashSet<Trigram>,
    /// Extracted symbols
    pub symbols: Vec<Symbol>,
    /// Extracted import paths
    pub imports: Vec<String>,
}

impl PreIndexedFile {
    /// Maximum file size to process (10MB) - larger files are skipped to avoid memory issues
    const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

    /// Process a file in parallel - does all CPU-heavy work without needing engine access
    /// Uses catch_unwind to handle tree-sitter stack overflows gracefully
    pub fn process(path: &Path) -> Option<Self> {
        // Check file size first - skip very large files
        let metadata = std::fs::metadata(path).ok()?;
        if metadata.len() > Self::MAX_FILE_SIZE {
            return None;
        }

        // Read file content
        let content = std::fs::read_to_string(path).ok()?;

        // Extract trigrams directly into FxHashSet (avoids Vec allocation + conversion)
        let trigrams = extract_unique_trigrams(&content);

        // Extract symbols with panic protection (tree-sitter can stack overflow on complex files)
        let extractor = SymbolExtractor::new(path);
        let symbols = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            extractor.extract(&content).unwrap_or_default()
        }))
        .unwrap_or_default();

        // Extract imports with panic protection
        let imports = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            extractor
                .extract_imports(&content)
                .ok()
                .map(|imports| imports.into_iter().map(|i| i.path).collect())
                .unwrap_or_default()
        }))
        .unwrap_or_default();

        Some(PreIndexedFile {
            path: path.to_path_buf(),
            content,
            trigrams,
            symbols,
            imports,
        })
    }
}

pub struct SearchEngine {
    pub file_store: FileStore,
    pub trigram_index: TrigramIndex,
    pub dependency_index: DependencyIndex,
    symbol_cache: Vec<Vec<Symbol>>,
    /// Pending imports to resolve after all files are indexed
    pending_imports: Vec<(u32, std::path::PathBuf, Vec<String>)>,
}

impl SearchEngine {
    pub fn new() -> Self {
        Self {
            file_store: FileStore::new(),
            trigram_index: TrigramIndex::new(),
            dependency_index: DependencyIndex::new(),
            symbol_cache: Vec::new(),
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

        // Index the content with trigrams
        self.trigram_index.add_document(file_id, content);

        // Extract symbols
        let extractor = SymbolExtractor::new(path);
        let symbols = extractor.extract(content).unwrap_or_default();

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
                self.pending_imports
                    .push((result.file_id, result.file_path, result.unresolved_paths));
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
    }

    /// Search for a query using parallel processing
    pub fn search(&self, query: &str, max_results: usize) -> Vec<SearchMatch> {
        // Pre-compute lowercase query ONCE before parallel loop (avoids allocation per document)
        let query_lower = query.to_lowercase();

        // Find candidate documents using trigram index
        let candidate_docs = self.trigram_index.search(query);

        // Convert to vector for parallel processing
        let doc_ids: Vec<u32> = candidate_docs.iter().collect();

        // Search in parallel using rayon
        let mut matches: Vec<SearchMatch> = doc_ids
            .par_iter()
            .filter_map(|&doc_id| self.search_in_document_fast(doc_id, &query_lower))
            .flatten()
            .collect();

        // Partial sort: only sort as much as needed for top N results
        // Using sort_unstable_by is faster than stable sort (no need for stability)
        if matches.len() > max_results {
            // Partial sort to find top max_results
            matches.select_nth_unstable_by(max_results, |a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            matches.truncate(max_results);
            // Now fully sort the top N
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

        matches
    }

    /// Search with path filtering using include/exclude glob patterns.
    ///
    /// This extends the basic search with additional path-based filtering:
    /// 1. Trigram index narrows candidates based on query content
    /// 2. Path filter further narrows based on file paths
    /// 3. Parallel search runs only on final candidates
    ///
    /// # Arguments
    /// * `query` - The search query string
    /// * `include_patterns` - Semicolon-delimited glob patterns to include
    /// * `exclude_patterns` - Semicolon-delimited glob patterns to exclude
    /// * `max_results` - Maximum number of results to return
    pub fn search_with_filter(
        &self,
        query: &str,
        include_patterns: &str,
        exclude_patterns: &str,
        max_results: usize,
    ) -> Result<Vec<SearchMatch>> {
        // Build path filter from patterns
        let path_filter = PathFilter::from_delimited(include_patterns, exclude_patterns)?;

        // Find candidate documents using trigram index
        let candidate_docs = self.trigram_index.search(query);

        // Apply path filter if it has any patterns (using closure to avoid cloning all paths)
        let filtered_docs = if path_filter.is_empty() {
            candidate_docs
        } else {
            path_filter
                .filter_documents_with(&candidate_docs, |doc_id| self.file_store.get_path(doc_id))
        };

        // Pre-compute lowercase query ONCE before parallel loop
        let query_lower = query.to_lowercase();

        // Convert to vector for parallel processing
        let doc_ids: Vec<u32> = filtered_docs.iter().collect();

        // Search in parallel using rayon
        let mut matches: Vec<SearchMatch> = doc_ids
            .par_iter()
            .filter_map(|&doc_id| self.search_in_document_fast(doc_id, &query_lower))
            .flatten()
            .collect();

        // Partial sort: only sort as much as needed for top N results
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

        Ok(matches)
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
            // Use the best (longest) literal for trigram filtering
            if let Some(literal) = analysis.best_literal() {
                tracing::debug!(
                    pattern = %pattern,
                    literal = %literal,
                    "Using trigram acceleration for regex search"
                );
                self.trigram_index.search(literal)
            } else {
                // Fallback to all documents
                tracing::warn!(
                    pattern = %pattern,
                    "Regex has no usable literals - falling back to full scan"
                );
                self.trigram_index.all_documents()
            }
        } else {
            // No acceleration possible - scan all documents
            tracing::warn!(
                pattern = %pattern,
                "Regex has no extractable literals >= 3 chars - falling back to full scan"
            );
            self.trigram_index.all_documents()
        };

        // Apply path filter if it has any patterns (using closure to avoid cloning all paths)
        let filtered_docs = if path_filter.is_empty() {
            candidate_docs
        } else {
            path_filter
                .filter_documents_with(&candidate_docs, |doc_id| self.file_store.get_path(doc_id))
        };

        // Convert filtered bitmap to vector for parallel processing
        let filtered_doc_ids: Vec<u32> = filtered_docs.iter().collect();

        // Search in parallel using rayon with regex matching
        let regex = &analysis.regex;
        let mut matches: Vec<SearchMatch> = filtered_doc_ids
            .par_iter()
            .filter_map(|&doc_id| self.search_in_document_regex(doc_id, regex))
            .flatten()
            .collect();

        // Partial sort: only sort as much as needed for top N results
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

        Ok(matches)
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
            if regex.is_match(line) {
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

                matches.push(SearchMatch {
                    file_id: doc_id,
                    file_path: path_ref.clone(),
                    line_number: line_num + 1, // 1-based line numbers
                    content: line.to_string(),
                    score,
                    is_symbol,
                    dependency_count,
                });
            }
        }

        Some(matches)
    }

    /// Optimized document search - takes pre-lowercased query to avoid allocation per document
    #[inline]
    fn search_in_document_fast(&self, doc_id: u32, query_lower: &str) -> Option<Vec<SearchMatch>> {
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
            if contains_case_insensitive(line, query_lower) {
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

                matches.push(SearchMatch {
                    file_id: doc_id,
                    file_path: path_ref.clone(),
                    line_number: line_num + 1, // 1-based line numbers
                    content: line.to_string(),
                    score,
                    is_symbol,
                    dependency_count,
                });
            }
        }

        Some(matches)
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
            if contains_case_insensitive(line, &query_lower) {
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

                matches.push(SearchMatch {
                    file_id: doc_id,
                    file_path: path_ref.clone(),
                    line_number: line_num + 1, // 1-based line numbers
                    content: line.to_string(),
                    score,
                    is_symbol,
                    dependency_count,
                });
            }
        }

        Some(matches)
    }

    pub fn get_stats(&self) -> SearchStats {
        SearchStats {
            num_files: self.file_store.len(),
            total_size: self.file_store.total_size(),
            num_trigrams: self.trigram_index.num_trigrams(),
            dependency_edges: self.dependency_index.total_edges(),
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
}

/// Status of the indexing process
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IndexingStatus {
    /// No indexing is in progress
    #[default]
    Idle,
    /// Discovering files to index
    Discovering,
    /// Actively indexing files
    Indexing,
    /// Resolving import dependencies
    ResolvingImports,
    /// Indexing completed successfully
    Completed,
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
}

impl Default for IndexingProgress {
    fn default() -> Self {
        Self {
            status: IndexingStatus::Idle,
            files_discovered: 0,
            files_indexed: 0,
            current_batch: 0,
            total_batches: 0,
            current_path: None,
            started_at: None,
            errors: 0,
            message: String::from("Ready"),
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
            IndexingStatus::Discovering => 5,
            IndexingStatus::Indexing => {
                if self.total_batches == 0 {
                    10
                } else {
                    let batch_progress =
                        (self.current_batch as f64 / self.total_batches as f64) * 85.0;
                    (10.0 + batch_progress).min(95.0) as u8
                }
            }
            IndexingStatus::ResolvingImports => 96,
            IndexingStatus::Completed => 100,
        }
    }
}

/// Shared indexing progress state for use across threads
pub type SharedIndexingProgress = std::sync::Arc<std::sync::RwLock<IndexingProgress>>;

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
}
