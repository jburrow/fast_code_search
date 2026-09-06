//! Split out of the search engine module; see `engine/mod.rs`.

use super::*;
use std::borrow::Cow;

impl SearchEngine {
    /// Get the metadata for a file
    #[inline]
    pub(super) fn get_file_metadata(&self, file_id: u32) -> &FileMetadata {
        static DEFAULT: std::sync::OnceLock<FileMetadata> = std::sync::OnceLock::new();
        self.file_metadata
            .get(file_id as usize)
            .unwrap_or_else(|| DEFAULT.get_or_init(FileMetadata::default))
    }

    /// Threshold for using fast ranking mode in Auto mode
    pub(super) const FAST_RANKING_THRESHOLD: usize = 5000;

    /// Maximum files to read in fast ranking mode
    pub(super) const FAST_RANKING_TOP_N: usize = 2000;

    /// Maximum matches to collect per document during a search.
    /// Prevents OOM on large indexes when a common keyword appears on
    /// thousands of lines in the same file.
    pub(super) const MAX_MATCHES_PER_DOC: usize = 100;

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
        // Empty / whitespace-only queries match nothing. Without this guard they
        // fall into the short-query branch, pull in ALL documents, and the empty
        // needle "matches" every line — a full-corpus scan returning garbage.
        if query.trim().is_empty() {
            return (Vec::new(), SearchRankingInfo::empty(rank_mode));
        }

        self.search_ranked_with_limits(query, SearchLimits::new(max_results), rank_mode)
    }

    /// [`Self::search_ranked`] with explicit limits (budget, deadline, offset).
    pub fn search_ranked_with_limits(
        &self,
        query: &str,
        limits: SearchLimits,
        rank_mode: RankMode,
    ) -> (Vec<SearchMatch>, SearchRankingInfo) {
        if query.trim().is_empty() {
            return (Vec::new(), SearchRankingInfo::empty(rank_mode));
        }
        let query_lower = query.to_lowercase();
        let candidate_docs = self.text_candidates(&query_lower);
        self.run_text_query(query, &query_lower, &candidate_docs, limits, rank_mode)
    }

    /// Candidate documents for a plain-text query. Queries shorter than 3
    /// bytes produce no trigrams; fall back to all documents so short terms
    /// like `_` or `__` still return results.
    pub(super) fn text_candidates(&self, query_lower: &str) -> Cow<'_, roaring::RoaringBitmap> {
        if query_lower.len() >= 3 {
            Cow::Owned(self.trigram_index.search(query_lower))
        } else {
            self.trigram_index.all_documents()
        }
    }

    /// Shared body of the plain-text searches (with or without a path filter).
    pub(super) fn run_text_query(
        &self,
        query: &str,
        query_lower: &str,
        candidates: &roaring::RoaringBitmap,
        limits: SearchLimits,
        rank_mode: RankMode,
    ) -> (Vec<SearchMatch>, SearchRankingInfo) {
        let terms = TermSet::single(query, query_lower);
        self.run_terms(&terms, candidates, limits, rank_mode)
    }

    pub(super) fn run_terms(
        &self,
        terms: &TermSet,
        candidates: &roaring::RoaringBitmap,
        limits: SearchLimits,
        rank_mode: RankMode,
    ) -> (Vec<SearchMatch>, SearchRankingInfo) {
        let primary_lower = terms.terms.first().map(|(_, l)| l.as_str()).unwrap_or("");
        self.run_candidates(
            candidates,
            rank_mode,
            limits,
            |meta| meta.query_score(primary_lower),
            |doc_id, run| self.search_in_document_terms(doc_id, terms, run),
        )
    }

    /// Search with the full query syntax (`file:`, `lang:`, `-term`, `case:`,
    /// `word:`, quoted phrases, several AND-ed terms). `include_patterns` /
    /// `exclude_patterns` are merged with the globs from the query.
    pub fn search_parsed(
        &self,
        parsed: &ParsedQuery,
        include_patterns: &str,
        exclude_patterns: &str,
        limits: SearchLimits,
        rank_mode: RankMode,
    ) -> Result<(Vec<SearchMatch>, SearchRankingInfo)> {
        if parsed.terms.iter().all(|t| t.trim().is_empty()) {
            return Ok((Vec::new(), SearchRankingInfo::empty(rank_mode)));
        }
        let path_filter = merged_path_filter(parsed, include_patterns, exclude_patterns)?;
        // Candidates: files whose trigrams contain EVERY term (the index is
        // lowercased, so candidate selection is case-insensitive even for a
        // case-sensitive search; verification does the exact comparison).
        let mut candidates: Option<roaring::RoaringBitmap> = None;
        for term in &parsed.terms {
            let docs = self.text_candidates(&term.to_lowercase());
            candidates = Some(match candidates {
                Some(acc) => acc & &*docs,
                None => docs.into_owned(),
            });
        }
        let candidates =
            self.apply_path_filter(Cow::Owned(candidates.unwrap_or_default()), &path_filter);
        let terms = TermSet::from_parsed(parsed);
        Ok(self.run_terms(&terms, &candidates, limits, rank_mode))
    }

    /// Symbol search honouring the query's globs and matching options
    /// (case-sensitive compares names exactly; whole-word requires the name
    /// to equal the term).
    pub fn search_symbols_parsed(
        &self,
        parsed: &ParsedQuery,
        include_patterns: &str,
        exclude_patterns: &str,
        limits: SearchLimits,
    ) -> Result<(Vec<SearchMatch>, SearchRankingInfo)> {
        let query = parsed.primary();
        if query.trim().is_empty() {
            return Ok((Vec::new(), SearchRankingInfo::empty(RankMode::Full)));
        }
        let path_filter = merged_path_filter(parsed, include_patterns, exclude_patterns)?;
        let query_lower = query.to_lowercase();
        let candidates = self.apply_path_filter(self.text_candidates(&query_lower), &path_filter);
        let opts = parsed.options;
        Ok(self.run_candidates(
            &candidates,
            RankMode::Full,
            limits,
            |meta| meta.base_score,
            |doc_id, run| {
                self.search_symbols_in_document_opts(doc_id, query, &query_lower, opts, run)
            },
        ))
    }

    /// The one place every search goes through: picks fast vs full ranking,
    /// orders candidates for fast mode, runs `per_doc` in parallel under the
    /// budget/deadline, then orders and pages the results deterministically.
    ///
    /// `fast_score` ranks candidate files by metadata alone (no I/O) when fast
    /// mode limits how many files are opened.
    pub(super) fn run_candidates<S, F>(
        &self,
        candidates: &roaring::RoaringBitmap,
        rank_mode: RankMode,
        limits: SearchLimits,
        fast_score: S,
        per_doc: F,
    ) -> (Vec<SearchMatch>, SearchRankingInfo)
    where
        S: Fn(&FileMetadata) -> f32,
        F: Fn(u32, &QueryRun) -> Option<Vec<SearchMatch>> + Sync,
    {
        let total_candidates = candidates.len() as usize;
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

        let doc_ids: Vec<u32> = if use_fast && !self.file_metadata.is_empty() {
            // Fast ranking: order by file metadata (no reads), open the top N.
            let mut scored: Vec<(u32, f32)> = candidates
                .iter()
                .map(|id| (id, fast_score(self.get_file_metadata(id))))
                .collect();
            scored.sort_unstable_by(|a, b| {
                b.1.partial_cmp(&a.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.0.cmp(&b.0))
            });
            scored
                .iter()
                .take(Self::FAST_RANKING_TOP_N)
                .map(|(id, _)| *id)
                .collect()
        } else if use_fast {
            // Fast ranking requested but no metadata yet (freshly loaded index):
            // cap the number of files opened rather than reading everything.
            candidates.iter().take(Self::FAST_RANKING_TOP_N).collect()
        } else {
            candidates.iter().collect()
        };
        let candidates_searched = doc_ids.len();

        let run = QueryRun::new(&limits);
        let mut matches: Vec<SearchMatch> = doc_ids
            .par_iter()
            .filter_map(|&doc_id| {
                if run.exhausted() {
                    return None;
                }
                per_doc(doc_id, &run)
            })
            .flatten()
            .collect();
        let found = matches.len();
        Self::sort_and_page(&mut matches, &limits);

        let truncated = run.was_truncated();
        (
            matches,
            SearchRankingInfo {
                mode: effective_mode,
                total_candidates,
                candidates_searched,
                truncated_by_budget: truncated,
                total_matches: if truncated { None } else { Some(found) },
            },
        )
    }

    /// Deterministic ordering — score desc, then file id, then line — and
    /// paging by `offset`/`max_results`. Ties no longer reorder run to run,
    /// so page N+1 never repeats or skips a result from page N.
    pub(super) fn sort_and_page(matches: &mut Vec<SearchMatch>, limits: &SearchLimits) {
        let cmp = |a: &SearchMatch, b: &SearchMatch| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.file_id.cmp(&b.file_id))
                .then_with(|| a.line_number.cmp(&b.line_number))
        };
        let keep = limits.offset.saturating_add(limits.max_results);
        if matches.len() > keep {
            matches.select_nth_unstable_by(keep, cmp);
            matches.truncate(keep);
        }
        matches.sort_unstable_by(cmp);
        if limits.offset > 0 {
            let drop = limits.offset.min(matches.len());
            matches.drain(..drop);
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
        // Empty / whitespace-only queries match nothing (see search_ranked).
        if query.trim().is_empty() {
            return Ok((Vec::new(), SearchRankingInfo::empty(rank_mode)));
        }

        self.search_with_filter_ranked_limits(
            query,
            include_patterns,
            exclude_patterns,
            SearchLimits::new(max_results),
            rank_mode,
        )
    }

    /// [`Self::search_with_filter_ranked`] with explicit limits.
    pub fn search_with_filter_ranked_limits(
        &self,
        query: &str,
        include_patterns: &str,
        exclude_patterns: &str,
        limits: SearchLimits,
        rank_mode: RankMode,
    ) -> Result<(Vec<SearchMatch>, SearchRankingInfo)> {
        if query.trim().is_empty() {
            return Ok((Vec::new(), SearchRankingInfo::empty(rank_mode)));
        }
        let path_filter = PathFilter::from_delimited(include_patterns, exclude_patterns)?;
        let query_lower = query.to_lowercase();
        let candidates = self.apply_path_filter(self.text_candidates(&query_lower), &path_filter);
        Ok(self.run_text_query(query, &query_lower, &candidates, limits, rank_mode))
    }

    /// Narrow a candidate set by include/exclude globs on display paths.
    pub(super) fn apply_path_filter<'a>(
        &self,
        candidates: Cow<'a, roaring::RoaringBitmap>,
        path_filter: &PathFilter,
    ) -> Cow<'a, roaring::RoaringBitmap> {
        if path_filter.is_empty() {
            return candidates;
        }
        // Precomputed display paths (no allocation) when metadata exists for
        // the id; otherwise (fresh index before finalize) build it.
        let mut result = roaring::RoaringBitmap::new();
        for doc_id in candidates.iter() {
            let keep = match self.file_metadata.get(doc_id as usize) {
                Some(meta) if !meta.display_path.is_empty() => {
                    path_filter.matches(&meta.display_path)
                }
                _ => self
                    .file_store
                    .get(doc_id)
                    .map(|f| path_filter.matches(&self.make_display_path(&f.path)))
                    .unwrap_or(false),
            };
            if keep {
                result.insert(doc_id);
            }
        }
        Cow::Owned(result)
    }

    /// Display path for a file id: precomputed when available.
    pub(super) fn display_path_for(&self, doc_id: u32, path: &Path) -> String {
        match self.file_metadata.get(doc_id as usize) {
            Some(meta) if !meta.display_path.is_empty() => meta.display_path.clone(),
            _ => self.make_display_path(path),
        }
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
    /// Compile (or fetch from the small LRU) the analysis for `pattern`.
    pub(super) fn cached_regex(&self, pattern: &str) -> Result<std::sync::Arc<RegexAnalysis>> {
        if let Ok(mut cache) = self.regex_cache.lock() {
            if let Some(a) = cache.get(pattern) {
                return Ok(a);
            }
        }
        let analysis = std::sync::Arc::new(RegexAnalysis::analyze(pattern)?);
        if let Ok(mut cache) = self.regex_cache.lock() {
            cache.put(pattern, analysis.clone());
        }
        Ok(analysis)
    }

    /// Compute candidate documents for a regex from its sound literal constraints.
    ///
    /// Returns `None` when the regex has no usable constraints (caller should fall
    /// back to a full scan). Otherwise returns the intersection across constraints
    /// of the union of each constraint's per-literal trigram matches.
    pub(super) fn regex_candidate_docs(
        &self,
        analysis: &RegexAnalysis,
    ) -> Option<roaring::RoaringBitmap> {
        if analysis.constraints.is_empty() {
            return None;
        }
        let mut result: Option<roaring::RoaringBitmap> = None;
        for group in &analysis.constraints {
            // Union of trigram matches for the alternatives in this constraint.
            let mut group_docs = roaring::RoaringBitmap::new();
            for literal in group {
                // Trigram index stores lowercased content.
                group_docs |= self.trigram_index.search(&literal.to_lowercase());
            }
            result = Some(match result {
                Some(acc) => acc & group_docs,
                None => group_docs,
            });
            if result.as_ref().is_some_and(|r| r.is_empty()) {
                break; // intersection already empty
            }
        }
        result
    }

    #[tracing::instrument(skip(self))]
    pub fn search_regex(
        &self,
        pattern: &str,
        include_patterns: &str,
        exclude_patterns: &str,
        max_results: usize,
    ) -> Result<Vec<SearchMatch>> {
        let (m, _) = self.search_regex_with_limits(
            pattern,
            include_patterns,
            exclude_patterns,
            SearchLimits::new(max_results),
            RankMode::Auto,
        )?;
        Ok(m)
    }

    /// [`Self::search_regex`] with explicit limits and ranking mode.
    pub fn search_regex_with_limits(
        &self,
        pattern: &str,
        include_patterns: &str,
        exclude_patterns: &str,
        limits: SearchLimits,
        rank_mode: RankMode,
    ) -> Result<(Vec<SearchMatch>, SearchRankingInfo)> {
        let analysis = self.cached_regex(pattern)?;
        let path_filter = PathFilter::from_delimited(include_patterns, exclude_patterns)?;

        // The candidate set is the INTERSECTION across constraints, and the UNION
        // of the trigram matches within each constraint. This keeps alternations
        // correct (e.g. `hello|world` keeps files that contain only `world`).
        let candidate_docs = match self.regex_candidate_docs(&analysis) {
            Some(docs) => {
                tracing::debug!(pattern = %pattern, "Using trigram acceleration for regex");
                Cow::Owned(docs)
            }
            None => {
                tracing::debug!(pattern = %pattern, "Regex has no sound literal constraints - full scan");
                self.trigram_index.all_documents()
            }
        };
        let candidates = self.apply_path_filter(candidate_docs, &path_filter);
        let regex = &analysis.regex;
        let multiline = analysis.multiline;
        Ok(self.run_candidates(
            &candidates,
            rank_mode,
            limits,
            |meta| meta.base_score,
            |doc_id, run| self.search_in_document_regex(doc_id, regex, multiline, run),
        ))
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
        let (m, _) = self.search_symbols_with_limits(
            query,
            include_patterns,
            exclude_patterns,
            SearchLimits::new(max_results),
        )?;
        Ok(m)
    }

    /// [`Self::search_symbols`] with explicit limits.
    ///
    /// Symbol search never truncates the candidate set by file score before
    /// matching: a document is only *opened* when one of its cached symbol
    /// names matches, so scanning every candidate's symbol cache is cheap and
    /// an exact symbol in a low-scoring file is never excluded.
    pub fn search_symbols_with_limits(
        &self,
        query: &str,
        include_patterns: &str,
        exclude_patterns: &str,
        limits: SearchLimits,
    ) -> Result<(Vec<SearchMatch>, SearchRankingInfo)> {
        if query.trim().is_empty() {
            return Ok((Vec::new(), SearchRankingInfo::empty(RankMode::Full)));
        }
        let path_filter = PathFilter::from_delimited(include_patterns, exclude_patterns)?;
        let query_lower = query.to_lowercase();
        let candidates = self.apply_path_filter(self.text_candidates(&query_lower), &path_filter);
        Ok(self.run_candidates(
            &candidates,
            RankMode::Full,
            limits,
            |meta| meta.base_score,
            |doc_id, run| self.search_symbols_in_document(doc_id, query, &query_lower, run),
        ))
    }

    /// Symbol references: the lines where the identifier `name` is used
    /// (called, mentioned as a type, implemented), as reported by the
    /// grammars' tags queries. Exact, case-sensitive identifier match;
    /// definitions are not included (symbol search finds those). One result
    /// per line, ordered by file score then position.
    pub fn search_references(
        &self,
        name: &str,
        include_patterns: &str,
        exclude_patterns: &str,
        limits: SearchLimits,
    ) -> Result<(Vec<SearchMatch>, SearchRankingInfo)> {
        let path_filter = PathFilter::from_delimited(include_patterns, exclude_patterns)?;
        Ok(self.run_references(name, path_filter, limits))
    }

    /// [`Self::search_references`] for a parsed query: the first term is the
    /// identifier, `file:` / `lang:` operators narrow the files.
    pub fn search_references_parsed(
        &self,
        parsed: &ParsedQuery,
        include_patterns: &str,
        exclude_patterns: &str,
        limits: SearchLimits,
    ) -> Result<(Vec<SearchMatch>, SearchRankingInfo)> {
        let Some(name) = parsed.terms.iter().find(|t| !t.trim().is_empty()) else {
            return Ok((Vec::new(), SearchRankingInfo::empty(RankMode::Full)));
        };
        let path_filter = merged_path_filter(parsed, include_patterns, exclude_patterns)?;
        Ok(self.run_references(name, path_filter, limits))
    }

    fn run_references(
        &self,
        name: &str,
        path_filter: PathFilter,
        limits: SearchLimits,
    ) -> (Vec<SearchMatch>, SearchRankingInfo) {
        let name = name.trim();
        let Some(name_id) = (!name.is_empty())
            .then(|| self.reference_name_id(name))
            .flatten()
        else {
            return (Vec::new(), SearchRankingInfo::empty(RankMode::Full));
        };
        // The identifier's text must occur in the file, so the trigram index
        // narrows the candidates before any reference list is scanned.
        let candidates =
            self.apply_path_filter(self.text_candidates(&name.to_lowercase()), &path_filter);
        self.run_candidates(
            &candidates,
            RankMode::Full,
            limits,
            |meta| meta.base_score,
            |doc_id, run| self.references_in_document(doc_id, name_id, name, run),
        )
    }

    /// Reference hits of `name` (interned as `name_id`) in one document.
    ///
    /// Positions were captured by tree-sitter at index time; the file may
    /// have changed on disk since (watching off, or the watcher has not
    /// caught up). A recorded position is only reported when the current
    /// line still holds `name` exactly there, so a stale column can neither
    /// slice mid-character (a panic that turned the whole request into a
    /// 500) nor highlight unrelated text.
    fn references_in_document(
        &self,
        doc_id: u32,
        name_id: u32,
        name: &str,
        run: &QueryRun,
    ) -> Option<Vec<SearchMatch>> {
        // Consult the reference list before touching file content: most
        // candidates mention the text without referencing the symbol.
        let refs = self.references_of(doc_id);
        if !refs.iter().any(|r| r.name == name_id) {
            return None;
        }
        let file = self.file_store.get(doc_id)?;
        let content = file.as_str().ok()?;
        let dependency_count = self.dependency_index.get_import_count(doc_id);
        let display_path = self.make_display_path(&file.path);
        let lines: Vec<&str> = content.lines().collect();
        let mut matches = Vec::new();
        let mut last_line = usize::MAX;
        for r in refs.iter().filter(|r| r.name == name_id) {
            let line_num = r.line as usize;
            if line_num == last_line {
                continue; // one result per line
            }
            let Some(line) = lines.get(line_num) else {
                continue; // file changed under us; the watcher will refresh it
            };
            // tree-sitter columns are byte offsets into the line as indexed.
            let start = r.column as usize;
            let end = start + name.len();
            if line.get(start..end) != Some(name) {
                continue; // stale position: the content moved under it
            }
            if !run.take_match() {
                break;
            }
            last_line = line_num;
            let truncated = truncate_around_match(line, start, end);
            matches.push(SearchMatch {
                file_id: doc_id,
                file_path: display_path.clone(),
                line_number: line_num + 1,
                content: truncated.content,
                match_start: truncated.match_start,
                match_end: truncated.match_end,
                content_truncated: truncated.was_truncated,
                line_match_start: start,
                line_match_end: end,
                match_column: char_column(line, start),
                score: 1.0,
                is_symbol: false,
                is_reference: true,
                dependency_count,
            });
        }
        (!matches.is_empty()).then_some(matches)
    }

    /// Search for symbols matching the query in a document.
    /// Returns matches only for lines where a symbol name matches.
    pub(super) fn search_symbols_in_document(
        &self,
        doc_id: u32,
        original_query: &str,
        query_lower: &str,
        run: &QueryRun,
    ) -> Option<Vec<SearchMatch>> {
        self.search_symbols_in_document_opts(
            doc_id,
            original_query,
            query_lower,
            SearchOptions::default(),
            run,
        )
    }

    pub(super) fn search_symbols_in_document_opts(
        &self,
        doc_id: u32,
        original_query: &str,
        query_lower: &str,
        opts: SearchOptions,
        run: &QueryRun,
    ) -> Option<Vec<SearchMatch>> {
        // Consult the symbol cache BEFORE touching file content: most
        // candidates have no matching symbol and must cost no I/O.
        let symbols = self.symbol_cache.get(doc_id as usize)?;
        let name_matches = |name: &str| -> bool {
            match (opts.case_sensitive, opts.whole_word) {
                (true, true) => name == original_query,
                (true, false) => name.contains(original_query),
                (false, true) => {
                    name.eq_ignore_ascii_case(original_query) || name.to_lowercase() == query_lower
                }
                (false, false) => contains_case_insensitive(name, query_lower),
            }
        };
        let matching_symbols: Vec<&Symbol> =
            symbols.iter().filter(|s| name_matches(&s.name)).collect();
        if matching_symbols.is_empty() {
            return None;
        }

        let file = self.file_store.get(doc_id)?;
        let content = file.as_str().ok()?;

        // Get dependency count for this file
        let dependency_count = self.dependency_index.get_import_count(doc_id);

        // Pre-compute dependency boost
        let dependency_boost = RankingWeights::DEFAULT.dependency_boost(dependency_count);

        // Lazy-compute path info
        let raw_path_str = file.path.to_string_lossy().into_owned();
        let path_bytes = raw_path_str.as_bytes();
        let is_src_lib = contains_bytes(path_bytes, b"/src/")
            || contains_bytes(path_bytes, b"\\src\\")
            || contains_bytes(path_bytes, b"/lib/")
            || contains_bytes(path_bytes, b"\\lib\\");
        let display_path = self.make_display_path(&file.path);

        // Collect lines into a vector for indexed access
        let lines: Vec<&str> = content.lines().collect();

        // Build matches from matching symbols
        let mut matches = Vec::with_capacity(matching_symbols.len());

        for symbol in matching_symbols {
            if !run.take_match() {
                break;
            }
            // FileName symbols are synthetic (not from file content) — show the file path
            if symbol.symbol_type == SymbolType::FileName {
                let display = display_path.clone();
                let (match_start, match_end) =
                    find_match_position_case_insensitive(&display, query_lower).unwrap_or((0, 0));
                let match_column = char_column(&display, match_start);
                matches.push(SearchMatch {
                    file_id: doc_id,
                    file_path: display_path.clone(),
                    line_number: 0,
                    content: display,
                    match_start,
                    match_end,
                    content_truncated: false,
                    line_match_start: match_start,
                    line_match_end: match_end,
                    match_column,
                    score: RankingWeights::DEFAULT.filename_hit * dependency_boost,
                    is_symbol: true,
                    is_reference: false,
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
            let base_score = calculate_score_inline(
                line,
                original_query,
                query_lower,
                true,
                is_src_lib,
                dependency_boost,
            );

            // exact > prefix > substring, and definitions of types/functions
            // slightly above variables/constants with the same match quality.
            let w = &RankingWeights::DEFAULT;
            let name_lower = symbol.name.to_lowercase();
            let name_factor = if name_lower == query_lower {
                w.symbol_exact_name
            } else if name_lower.starts_with(query_lower) {
                w.symbol_prefix_name
            } else {
                1.0
            };
            let kind_factor = match symbol.symbol_type {
                SymbolType::Variable | SymbolType::Constant => w.symbol_value_kind,
                _ => 1.0,
            };
            let score = base_score * name_factor * kind_factor;

            // One row per line: several symbols on one line (e.g. a struct and
            // its impl, or a re-exported alias) keep the best-scoring one.
            if let Some(existing) = matches
                .iter_mut()
                .find(|m: &&mut SearchMatch| m.line_number == symbol.line + 1)
            {
                if score > existing.score {
                    existing.score = score;
                }
                continue;
            }

            // Truncate long lines around the match
            let truncated = truncate_around_match(line, match_start, match_end);

            matches.push(SearchMatch {
                file_id: doc_id,
                file_path: display_path.clone(),
                line_number: symbol.line + 1, // 1-based line numbers
                content: truncated.content,
                match_start: truncated.match_start,
                match_end: truncated.match_end,
                content_truncated: truncated.was_truncated,
                line_match_start: match_start,
                line_match_end: match_end,
                match_column: char_column(line, match_start),
                score,
                is_symbol: true,
                is_reference: false,
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
    pub(super) fn search_in_document_regex(
        &self,
        doc_id: u32,
        regex: &Regex,
        multiline: bool,
        run: &QueryRun,
    ) -> Option<Vec<SearchMatch>> {
        let file = self.file_store.get(doc_id)?;
        let content = file.as_str().ok()?;

        // Symbols for this file. The cache slot may be missing (index loaded
        // without symbols, or a file added before its symbols were stored);
        // the text path treats that as "no symbols" and so must this one,
        // otherwise every regex hit in such a file is silently dropped.
        let symbols = self
            .symbol_cache
            .get(doc_id as usize)
            .map(|s| s.as_slice())
            .unwrap_or(&[]);

        // Get dependency count for this file (cached lookup - done once per document)
        let dependency_count = self.dependency_index.get_import_count(doc_id);

        // Pre-compute dependency boost (done once per document, not per match)
        let dependency_boost = RankingWeights::DEFAULT.dependency_boost(dependency_count);

        // Use a simple Vec to store symbol definition lines - faster than HashSet for small N
        // Most files have <100 symbols, linear scan is faster than hash overhead
        // The synthetic FileName symbol lives at line 0 and must NOT count as a
        // definition line, otherwise every match on the first line of every
        // file gets the definition boost.
        let symbol_def_lines: Vec<usize> = symbols
            .iter()
            .filter(|s| s.is_definition && s.symbol_type != SymbolType::FileName)
            .map(|s| s.line)
            .collect();

        // For files with many definition symbols, promote to a HashSet for O(1) lookups.
        // Linear Vec::contains is O(n); with >32 defs across thousands of lines this
        // becomes the hot path.  The HashSet build cost (~32 inserts) is paid back
        // immediately on the first line scan.
        let use_hashset = symbol_def_lines.len() > 32;
        let symbol_def_set: FxHashSet<usize> = if use_hashset {
            symbol_def_lines.iter().copied().collect()
        } else {
            FxHashSet::default()
        };

        // Pre-compute a per-line symbol-name map for O(1) `is_symbol` lookup.
        // Without this, checking whether a matching line contains a symbol whose
        // name matches the regex requires an O(n_symbols) scan for every match.
        let symbol_names_by_line: FxHashMap<usize, Vec<&str>> = symbols
            .iter()
            .filter(|s| s.symbol_type != crate::symbols::SymbolType::FileName)
            .fold(FxHashMap::default(), |mut map, s| {
                map.entry(s.line).or_default().push(s.name.as_str());
                map
            });

        // Single-pass search: collect matches directly
        let mut matches = Vec::with_capacity(8);

        // Lazy-compute path info only if we find matches
        let mut display_path: Option<String> = None;
        let mut is_src_lib = false;

        {
            // Turn one matching line into a result. Returns false once the
            // query's match budget is exhausted.
            let mut emit = |line_num: usize, line: &str, m_start: usize, m_end: usize| -> bool {
                // Cap per-document results (a broad regex matching thousands
                // of lines must not grow memory unboundedly) and honour the
                // query's match budget.
                if matches.len() >= Self::MAX_MATCHES_PER_DOC || !run.take_match() {
                    return false;
                }
                // Lazy initialize path info only when we have at least one match
                let path_ref = display_path.get_or_insert_with(|| {
                    let raw = file.path.to_string_lossy().into_owned();
                    let path_bytes = raw.as_bytes();
                    is_src_lib = contains_bytes(path_bytes, b"/src/")
                        || contains_bytes(path_bytes, b"\\src\\")
                        || contains_bytes(path_bytes, b"/lib/")
                        || contains_bytes(path_bytes, b"\\lib\\");
                    self.make_display_path(&file.path)
                });

                // Calculate score using pre-computed values
                let is_symbol_def = if use_hashset {
                    symbol_def_set.contains(&line_num)
                } else {
                    symbol_def_lines.contains(&line_num)
                };
                let score = calculate_score_regex_inline(
                    line,
                    regex,
                    is_symbol_def,
                    is_src_lib,
                    dependency_boost,
                );

                // Check if this is a symbol match using the pre-computed per-line map (O(1) lookup)
                let is_symbol = symbol_names_by_line
                    .get(&line_num)
                    .map(|names| names.iter().any(|name| regex.is_match(name)))
                    .unwrap_or(false);

                // Truncate long lines around the match
                let truncated = truncate_around_match(line, m_start, m_end);

                matches.push(SearchMatch {
                    file_id: doc_id,
                    file_path: path_ref.clone(),
                    line_number: line_num + 1, // 1-based line numbers
                    content: truncated.content,
                    match_start: truncated.match_start,
                    match_end: truncated.match_end,
                    content_truncated: truncated.was_truncated,
                    line_match_start: m_start,
                    line_match_end: m_end,
                    match_column: char_column(line, m_start),
                    score,
                    is_symbol,
                    is_reference: false,
                    dependency_count,
                });
                true
            };

            if multiline {
                // Whole-content matching: each match is reported on the line
                // where it starts (one result per line), with the in-line
                // offsets clamped to that line.
                let text: &str = &content;
                let mut line_num = 0usize;
                let mut scan_pos = 0usize;
                let mut last_line: Option<usize> = None;
                for m in regex.find_iter(text) {
                    line_num += text[scan_pos..m.start()]
                        .bytes()
                        .filter(|&b| b == b'\n')
                        .count();
                    scan_pos = m.start();
                    if last_line == Some(line_num) {
                        continue;
                    }
                    let line_start = text[..m.start()].rfind('\n').map_or(0, |i| i + 1);
                    let line_end = text[m.start()..]
                        .find('\n')
                        .map_or(text.len(), |i| i + m.start());
                    let line = text[line_start..line_end]
                        .strip_suffix('\r')
                        .unwrap_or(&text[line_start..line_end]);
                    let m_start = m.start() - line_start;
                    let m_end = (m.end().min(line_end) - line_start).min(line.len());
                    if !emit(line_num, line, m_start, m_end) {
                        break;
                    }
                    last_line = Some(line_num);
                }
            } else {
                // Search in each line using regex
                for (line_num, line) in content.lines().enumerate() {
                    if let Some(m) = regex.find(line) {
                        if !emit(line_num, line, m.start(), m.end()) {
                            break;
                        }
                    }
                }
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
                    display_path.get_or_insert_with(|| self.make_display_path(&file.path));
                let display = path_ref.clone();
                let (match_start, match_end) = regex
                    .find(&display)
                    .map(|m| (m.start(), m.end()))
                    .unwrap_or((0, 0));
                let match_column = char_column(&display, match_start);
                matches.push(SearchMatch {
                    file_id: doc_id,
                    file_path: path_ref.clone(),
                    line_number: 0,
                    content: display,
                    match_start,
                    match_end,
                    content_truncated: false,
                    line_match_start: match_start,
                    line_match_end: match_end,
                    match_column,
                    score: RankingWeights::DEFAULT.filename_hit * dependency_boost,
                    is_symbol: true,
                    is_reference: false,
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

    /// Term-set aware document scan: every term must occur somewhere in the
    /// file and no excluded term may; lines matching any term are returned,
    /// scored against the primary term.
    pub(super) fn search_in_document_terms(
        &self,
        doc_id: u32,
        terms: &TermSet,
        run: &QueryRun,
    ) -> Option<Vec<SearchMatch>> {
        let (original_query, query_lower) =
            terms.terms.first().map(|(o, l)| (o.as_str(), l.as_str()))?;
        let opts = terms.opts;
        let file = self.file_store.get(doc_id)?;
        let content = file.as_str().ok()?;

        // File-level NOT check before any per-line work.
        if terms
            .exclude
            .iter()
            .any(|(o, l)| !line_hits(&content, o, l, opts).is_empty())
        {
            return None;
        }

        // Hits to report, each with the number of distinct terms on its line.
        //
        // One term: every hit line, in document order (the ASCII
        // case-insensitive scan when no option is set).
        //
        // Several terms (file-level AND): every term must hit somewhere or
        // the file is skipped; a line is reported once, by the earliest term
        // that matched it, and lines are ordered by how many distinct terms
        // they contain (most first, then document order) so that lines
        // holding every term are emitted before the per-document cap and the
        // match budget can cut anything off. The count also scales the
        // line's score below, so those lines rank first globally.
        let mut all_hits: Vec<(LineHit<'_>, usize)>;
        if terms.terms.len() == 1 {
            let hits = if !opts.case_sensitive && !opts.whole_word && query_lower.is_ascii() {
                ascii_ci_line_hits(&content, query_lower)
            } else {
                line_hits(&content, original_query, query_lower, opts)
            };
            all_hits = hits.into_iter().map(|h| (h, 1)).collect();
        } else {
            let mut by_line: FxHashMap<usize, (LineHit<'_>, usize)> = FxHashMap::default();
            for (o, l) in &terms.terms {
                let hits = line_hits(&content, o, l, opts);
                if hits.is_empty() {
                    return None; // AND: a term is missing from the file
                }
                for hit in hits {
                    by_line.entry(hit.line_num).or_insert((hit, 0)).1 += 1;
                }
            }
            all_hits = by_line.into_values().collect();
            all_hits.sort_by_key(|(h, n)| (std::cmp::Reverse(*n), h.line_num));
        }

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
        let dependency_boost = RankingWeights::DEFAULT.dependency_boost(dependency_count);

        // Single-pass search: collect matches directly
        let mut matches = Vec::with_capacity(8);

        // Everything below is built lazily on the FIRST match: most candidates
        // (trigram hits that fail verification, or files whose only hit is
        // beyond the budget) must cost nothing beyond the scan itself.
        let mut display_path: Option<String> = None;
        let mut is_src_lib = false;
        let mut symbol_maps: Option<SymbolLineMaps<'_>> = None;

        // Per-hit body; `term_hits` is the number of distinct query terms on
        // the line (always 1 for a single-term query). Returns false to stop.
        let mut emit = |line_num: usize,
                        line: &str,
                        match_start: usize,
                        match_end: usize,
                        term_hits: usize|
         -> bool {
            if matches.len() >= Self::MAX_MATCHES_PER_DOC || !run.take_match() {
                return false;
            }
            let path_ref = display_path.get_or_insert_with(|| {
                let raw = file.path.to_string_lossy().into_owned();
                let path_bytes = raw.as_bytes();
                is_src_lib = contains_bytes(path_bytes, b"/src/")
                    || contains_bytes(path_bytes, b"\\src\\")
                    || contains_bytes(path_bytes, b"/lib/")
                    || contains_bytes(path_bytes, b"\\lib\\");
                self.display_path_for(doc_id, &file.path)
            });
            let maps = symbol_maps.get_or_insert_with(|| SymbolLineMaps::build(symbols));

            let is_symbol_def = maps.is_definition_line(line_num);
            let mut score = calculate_score_inline(
                line,
                original_query,
                query_lower,
                is_symbol_def,
                is_src_lib,
                dependency_boost,
            );
            if term_hits > 1 {
                // A line holding several of the query's terms outranks
                // one holding a single term (single-term scores are
                // untouched).
                score *= term_hits as f64;
            }
            let is_symbol = maps.names_on_line(line_num).is_some_and(|names| {
                names
                    .iter()
                    .any(|n| contains_case_insensitive(n, query_lower))
            });

            let truncated = truncate_around_match(line, match_start, match_end);
            matches.push(SearchMatch {
                file_id: doc_id,
                file_path: path_ref.clone(),
                line_number: line_num + 1, // 1-based line numbers
                content: truncated.content,
                match_start: truncated.match_start,
                match_end: truncated.match_end,
                content_truncated: truncated.was_truncated,
                line_match_start: match_start,
                line_match_end: match_end,
                match_column: char_column(line, match_start),
                score,
                is_symbol,
                is_reference: false,
                dependency_count,
            });
            true
        };

        for (hit, term_hits) in all_hits {
            if !emit(hit.line_num, hit.line, hit.start, hit.end, term_hits) {
                break;
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
                    display_path.get_or_insert_with(|| self.display_path_for(doc_id, &file.path));
                let display = path_ref.clone();
                let (match_start, match_end) =
                    find_match_position_case_insensitive(&display, query_lower).unwrap_or((0, 0));
                let match_column = char_column(&display, match_start);
                matches.push(SearchMatch {
                    file_id: doc_id,
                    file_path: path_ref.clone(),
                    line_number: 0, // Convention: 0 means "filename match, not a content line"
                    content: display,
                    match_start,
                    match_end,
                    content_truncated: false,
                    line_match_start: match_start,
                    line_match_end: match_end,
                    match_column,
                    score: RankingWeights::DEFAULT.filename_hit * dependency_boost, // Symbol def boost (3×) for filename matches
                    is_symbol: true,
                    is_reference: false,
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
}
