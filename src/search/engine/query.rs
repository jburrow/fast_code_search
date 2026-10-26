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

    /// Most documents read per parallel step of a scan (see
    /// `run_candidates`).
    const SCAN_CHUNK_MAX: usize = 4096;

    /// Search with configurable ranking mode.
    ///
    /// # Ranking Modes
    /// - `Auto` / `Fast`: reads candidate files best first by pre-computed
    ///   file scores until the match budget is spent, at most
    ///   `limits.max_files` of them
    /// - `Full`: the same order, never limited to `max_files`
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
        let query_lower = fold_str(query).into_owned();
        let candidate_docs = self.text_candidates(&query_lower);
        self.run_text_query(query, &query_lower, &candidate_docs, limits, rank_mode)
    }

    /// Candidate documents for a plain-text query (folded here too, so a
    /// caller holding a lowercased string gets the index's spelling).
    /// Queries shorter than 3 bytes produce no trigrams; fall back to all
    /// documents so short terms like `_` or `__` still return results.
    pub(super) fn text_candidates(&self, query_lower: &str) -> Cow<'_, roaring::RoaringBitmap> {
        let folded = fold_str(query_lower);
        if folded.len() >= 3 {
            Cow::Owned(self.trigram_index.search(&folded))
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

    /// Symbol search opens at least this many files (the best by metadata)
    /// before line-level scoring; more when the page or offset is larger.
    const SYMBOL_PREFILTER_MIN_FILES: usize = 200;
    /// Phase-2 budget per selected file (the per-document cap is 100).
    const SYMBOL_MATCHES_PER_FILE_BUDGET: usize = 20;
    /// Phase-1 weight of a filename match in symbol search: far below any
    /// real symbol, so filename-only files never displace those.
    const FILENAME_ONLY_PREFILTER: f64 = 1e-3;
}

/// Does a regex match starting at `at` lie after the file's last line? The
/// end of a file that ends in a newline (or is empty) starts no line, but an
/// empty-matching pattern (`^$`, `x*`) still matches there; reporting it
/// invented a blank line past the end of every such file.
fn past_last_line(text: &str, at: usize) -> bool {
    at == text.len() && (text.is_empty() || text.ends_with('\n'))
}

impl SearchEngine {
    /// Run a plain-text search for `terms` over `candidates`.
    pub(super) fn run_terms(
        &self,
        terms: &TermSet,
        candidates: &roaring::RoaringBitmap,
        limits: SearchLimits,
        rank_mode: RankMode,
    ) -> (Vec<SearchMatch>, SearchRankingInfo) {
        let primary_lower = terms.term.1.as_str();
        self.run_candidates(
            candidates,
            rank_mode,
            limits,
            |_, meta| meta.query_score(primary_lower),
            |doc_id, run| self.search_in_document_terms(doc_id, terms, run),
        )
    }

    /// Search with the query syntax (`file:`, `lang:`, `case:`, `word:`;
    /// the rest of the query is one literal). `include_patterns` /
    /// `exclude_patterns` are merged with the globs from the query.
    pub fn search_parsed(
        &self,
        parsed: &ParsedQuery,
        include_patterns: &str,
        exclude_patterns: &str,
        limits: SearchLimits,
        rank_mode: RankMode,
    ) -> Result<(Vec<SearchMatch>, SearchRankingInfo)> {
        if parsed.text.trim().is_empty() {
            return Ok((Vec::new(), SearchRankingInfo::empty(rank_mode)));
        }
        let limits = self.limits_with_query_scope(parsed, limits)?;
        let path_filter = merged_path_filter(parsed, include_patterns, exclude_patterns)?;
        // The index is lowercased, so candidate selection is
        // case-insensitive even for a case-sensitive search; verification
        // does the exact comparison.
        let candidates = self.apply_path_filter(self.text_candidates(&parsed.text), &path_filter);
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
        let limits = self.limits_with_query_scope(parsed, limits)?;
        let path_filter = merged_path_filter(parsed, include_patterns, exclude_patterns)?;
        let query_lower = fold_str(query).into_owned();
        let mut candidates =
            self.apply_path_filter(self.text_candidates(&query_lower), &path_filter);
        if let Some(scope) = &limits.scope {
            candidates = Cow::Owned(scope.restrict(&candidates));
        }
        let opts = parsed.options;

        // Phase 1, no I/O: score every matching symbol from the cache alone
        // (name match quality, kind, file metadata) and keep the files
        // holding the best ones. Reading every file with a substring match
        // — thousands for a short name — burned the match budget on
        // `FieldInfo` and `Fieldset` before the file defining `Field` was
        // opened.
        let name_matches = |name: &str| -> bool {
            match (opts.case_sensitive, opts.whole_word) {
                (true, true) => name == query,
                (true, false) => name.contains(query),
                (false, true) => {
                    name.eq_ignore_ascii_case(query) || *fold_str(name) == *query_lower
                }
                (false, false) => contains_case_insensitive(name, &query_lower),
            }
        };
        let w = &RankingWeights::DEFAULT;
        let mut scored: Vec<(f32, u32)> = candidates
            .iter()
            .filter_map(|id| {
                let symbols = self.symbol_cache.get(id as usize)?;
                let meta = self.get_file_metadata(id);
                let best = symbols
                    .iter()
                    .filter(|s| name_matches(&s.name))
                    .map(|s| {
                        let lower = fold_str(&s.name).into_owned();
                        let name_factor = if lower == query_lower {
                            w.symbol_exact_name
                        } else if lower.starts_with(&query_lower) {
                            w.symbol_prefix_name
                        } else {
                            1.0
                        };
                        let kind = match s.symbol_type {
                            SymbolType::Variable | SymbolType::Constant => w.symbol_value_kind,
                            // A file whose only match is its name is still a
                            // result (phase 2 reports it as a filename hit),
                            // but it is opened after files defining a
                            // matching symbol.
                            SymbolType::FileName => Self::FILENAME_ONLY_PREFILTER,
                            _ => 1.0,
                        };
                        (name_factor * kind) as f32 * meta.base_score.max(0.1)
                    })
                    .fold(None, |acc: Option<f32>, v| {
                        Some(acc.map_or(v, |a| a.max(v)))
                    })?;
                Some((best, id))
            })
            .collect();
        // Ties by path, as results are: file ids follow discovery order,
        // which differs between machines.
        scored.sort_unstable_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    self.get_file_metadata(a.1)
                        .display_path
                        .cmp(&self.get_file_metadata(b.1).display_path)
                })
                .then_with(|| a.1.cmp(&b.1))
        });
        // The best files by default; an explicit file limit (the web UI's
        // SEARCH ALL FILES, MCP's `max_files`) widens it, and full ranking
        // reads them all.
        let mut keep = (limits.offset.saturating_add(limits.max_results))
            .saturating_mul(8)
            .max(Self::SYMBOL_PREFILTER_MIN_FILES);
        if limits.every_file {
            keep = usize::MAX;
        } else if limits.max_files != usize::MAX {
            keep = keep.max(limits.max_files);
        }
        let selected: roaring::RoaringBitmap =
            scored.iter().take(keep).map(|(_, id)| *id).collect();
        // What could have been read: the files holding a matching symbol.
        let total_candidates = scored.len();

        // Phase 2: read only the selected files and score their lines. The
        // file set is already bounded, so the budget is sized to cover every
        // symbol in it rather than letting the scan order decide which of
        // the selected files get read.
        let phase2 = limits.clone().with_match_budget(
            limits
                .match_budget
                .max(selected.len() as usize * Self::SYMBOL_MATCHES_PER_FILE_BUDGET),
        );
        let (matches, mut info) = self.run_candidates(
            &selected,
            RankMode::Full,
            phase2,
            |_, meta| meta.base_score,
            |doc_id, run| {
                self.search_symbols_in_document_opts(doc_id, query, &query_lower, opts, run)
            },
        );
        info.total_candidates = total_candidates;
        // Files holding a matching symbol beyond the ones read were never
        // opened: say so, rather than report a total that looks complete.
        // The total is then unknown: those files may hold more.
        info.candidates_searched = selected.len() as usize;
        if scored.len() > selected.len() as usize {
            if !info.limits_hit.contains(&SearchLimit::FileCap) {
                info.limits_hit.push(SearchLimit::FileCap);
                info.limits_hit.sort();
            }
            info.total_matches = None;
            info.truncated_by_budget = true;
        }
        Ok((matches, info))
    }

    /// The one place every search goes through: orders the candidates best
    /// first, runs `per_doc` in parallel under the budget/deadline, then
    /// orders and pages the results deterministically.
    ///
    /// `fast_score` ranks candidate files by metadata alone (no I/O). Every
    /// mode reads files in that order, so when the budget stops the scan,
    /// the hits ranked are those of the best-looking files rather than of
    /// the first folders discovered (measured on a 372k-file corpus: 81%
    /// against 31% of the exhaustive top 50 at the same budget). Fast and
    /// auto ranking also stop after `limits.max_files` files when one is
    /// set.
    pub(super) fn run_candidates<S, F>(
        &self,
        candidates: &roaring::RoaringBitmap,
        rank_mode: RankMode,
        limits: SearchLimits,
        fast_score: S,
        per_doc: F,
    ) -> (Vec<SearchMatch>, SearchRankingInfo)
    where
        S: Fn(u32, &FileMetadata) -> f32,
        F: Fn(u32, &QueryRun) -> Option<Vec<SearchMatch>> + Sync,
    {
        let scoped;
        let candidates = match &limits.scope {
            Some(scope) => {
                scoped = scope.restrict(candidates);
                &scoped
            }
            None => candidates,
        };
        let boost = |id: u32| limits.scope.as_ref().map_or(1.0, |s| s.boost(id));
        let total_candidates = candidates.len() as usize;
        let use_fast = match rank_mode {
            RankMode::Fast | RankMode::Auto => true,
            RankMode::Full => false,
        };
        let effective_mode = if use_fast {
            RankMode::Fast
        } else {
            RankMode::Full
        };
        let file_cap = if use_fast {
            limits.max_files
        } else {
            usize::MAX
        };

        let doc_ids: Vec<u32> = if !self.file_metadata.is_empty() {
            // Order by file metadata (no reads); fast ranking opens at most
            // the top `max_files`.
            let mut scored: Vec<(u32, f32)> = candidates
                .iter()
                .map(|id| {
                    let score = fast_score(id, self.get_file_metadata(id));
                    (id, score * boost(id) as f32)
                })
                .collect();
            scored.sort_unstable_by(|a, b| {
                b.1.partial_cmp(&a.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.0.cmp(&b.0))
            });
            scored.iter().take(file_cap).map(|(id, _)| *id).collect()
        } else {
            // No metadata yet (freshly loaded index): discovery order.
            candidates.iter().take(file_cap).collect()
        };
        let candidates_searched = doc_ids.len();

        // Deterministic scan: documents are read in parallel in chunks, but
        // their hits are taken in candidate order, so the match budget always
        // keeps the same prefix of documents. The old single parallel pass
        // spent the budget on whichever files threads reached first, so a
        // query that hit the budget returned a different first page on
        // almost every run, and paging (each page re-runs the scan with a
        // larger budget) repeated some hits and skipped others. Each step
        // reads about as many documents as the remaining budget needs at the
        // hit rate so far, so a broad query reads little past its budget, and
        // steps double while hits are rare.
        let base_cap = limits.doc_cap();
        let mut budget = limits.match_budget;
        let mut budget_cut = false;
        let mut hit = std::collections::BTreeSet::new();
        if use_fast && candidates_searched < total_candidates {
            hit.insert(SearchLimit::FileCap);
        }
        let mut matches: Vec<SearchMatch> = Vec::new();
        // Files that may hold more hits than they gave (per-file cap or the
        // budget cut them), so their hit counts are lower bounds.
        let mut capped_files: FxHashSet<u32> = FxHashSet::default();
        let threads = rayon::current_num_threads().max(1);
        let (mut i, mut chunk) = (0usize, threads);
        let (mut docs_read, mut hits_seen) = (0usize, 0usize);
        'scan: while i < doc_ids.len() {
            // No document in this chunk can usefully emit more than the
            // budget left at its start (plus one, to learn that it had
            // more); fixed per chunk, so it is deterministic.
            let chunk_cap = base_cap.min(budget.saturating_add(1));
            let doc_run = QueryRun::new(
                &limits
                    .clone()
                    .with_match_budget(usize::MAX)
                    .with_per_doc_cap(chunk_cap),
            );
            if doc_run.exhausted() {
                hit.insert(SearchLimit::Deadline);
                break;
            }
            let end = (i + chunk).min(doc_ids.len());
            let per: Vec<Option<Vec<SearchMatch>>> = doc_ids[i..end]
                .par_iter()
                .map(|&doc_id| {
                    if doc_run.exhausted() {
                        return None;
                    }
                    per_doc(doc_id, &doc_run)
                })
                .collect();
            let stopped = doc_run.stopped_by();
            if stopped & QueryRun::DEADLINE != 0 {
                hit.insert(SearchLimit::Deadline);
            }
            // A file stopped at a cap below the per-file cap was cut by the
            // budget, which `budget_cut` reports below.
            if stopped & QueryRun::CAP != 0 && base_cap <= budget {
                hit.insert(SearchLimit::PerFileCap);
            }
            if stopped & QueryRun::BUDGET != 0 {
                hit.insert(SearchLimit::MatchBudget);
            }
            docs_read += end - i;
            for hits in per.into_iter().flatten() {
                if hits.len() >= chunk_cap || hits.len() > budget {
                    if let Some(m) = hits.first() {
                        capped_files.insert(m.file_id);
                    }
                }
                if hits.len() > budget {
                    // The file's best hits, not its first ones.
                    let mut hits = hits;
                    sort_doc_hits(&mut hits);
                    matches.extend(hits.into_iter().take(budget));
                    budget_cut = true;
                    break 'scan;
                }
                budget -= hits.len();
                hits_seen += hits.len();
                matches.extend(hits);
            }
            i = end;
            // Size the next step to what should fill the remaining budget at
            // the hit rate so far (hits past the budget are work thrown
            // away), at most double the last step (an early low hit rate
            // must not read thousands of files the budget never needed),
            // at least one document per thread.
            let grow = chunk.saturating_mul(2);
            let wanted = budget
                .saturating_mul(docs_read)
                .checked_div(hits_seen)
                .map_or(grow, |n| n.saturating_add(1).min(grow));
            chunk = wanted.clamp(threads, Self::SCAN_CHUNK_MAX);
        }
        let found = matches.len();
        if limits.scope.is_some() {
            for m in &mut matches {
                m.score *= boost(m.file_id);
            }
        }
        Self::sort_and_page(&mut matches, &limits, &capped_files);

        if budget_cut {
            hit.insert(SearchLimit::MatchBudget);
        }
        // The file cap leaves the total exact for the files that were
        // opened; every other limit makes it unknown.
        let truncated = hit.iter().any(|l| *l != SearchLimit::FileCap);
        (
            matches,
            SearchRankingInfo {
                mode: effective_mode,
                total_candidates,
                candidates_searched,
                truncated_by_budget: truncated,
                total_matches: if truncated { None } else { Some(found) },
                limits_hit: hit.into_iter().collect(),
            },
        )
    }

    /// Deterministic ordering and paging by `offset`/`max_results`.
    ///
    /// Plain rank order, with each file's hits kept together: files in the
    /// order of their best hit, and within a file every hit by score. No hit
    /// waits behind other files' hits. Ties break on the display path then
    /// line, so page N+1 never repeats or skips a result from page N.
    ///
    /// Every hit on the page carries how many hits its file has in the whole
    /// result (`file_hits`), so a file cut at a page boundary does not read
    /// as a file with only the hits shown.
    pub(super) fn sort_and_page(
        matches: &mut Vec<SearchMatch>,
        limits: &SearchLimits,
        capped_files: &FxHashSet<u32>,
    ) {
        // Ties break on the display path, not the file id: ids follow
        // discovery order, which differs between machines and runs, and a
        // result page must not.
        let by_score = |a: &SearchMatch, b: &SearchMatch| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.file_path.cmp(&b.file_path))
                .then_with(|| a.line_number.cmp(&b.line_number))
                .then_with(|| a.file_id.cmp(&b.file_id))
        };
        // Each file's place is where its best hit ranks; its other hits
        // follow it, still in score order. Only the page is returned, so
        // only the files it reaches have their hits sorted: sorting every
        // hit (tens of thousands for a common word) to return 50 was a
        // third of such a search.
        let mut by_file: FxHashMap<u32, Vec<usize>> = FxHashMap::default();
        for (i, m) in matches.iter().enumerate() {
            by_file.entry(m.file_id).or_default().push(i);
        }
        let mut files: Vec<(usize, Vec<usize>)> = by_file
            .into_values()
            .map(|hits| {
                let best = hits
                    .iter()
                    .copied()
                    .min_by(|&a, &b| by_score(&matches[a], &matches[b]))
                    .expect("a file in the map has a hit");
                (best, hits)
            })
            .collect();
        files.sort_unstable_by(|a, b| by_score(&matches[a.0], &matches[b.0]));

        let start = limits.offset;
        let end = start.saturating_add(limits.max_results);
        let mut page: Vec<SearchMatch> = Vec::with_capacity(limits.max_results.min(matches.len()));
        let mut pos = 0usize;
        for (_, mut hits) in files {
            if pos >= end {
                break;
            }
            let file_hits = hits.len();
            if pos + file_hits <= start {
                pos += file_hits;
                continue;
            }
            hits.sort_unstable_by(|&a, &b| by_score(&matches[a], &matches[b]));
            for i in hits {
                if pos >= end {
                    break;
                }
                if pos >= start {
                    let mut m = matches[i].clone();
                    m.file_hits = file_hits as u32;
                    m.file_hits_capped = capped_files.contains(&m.file_id);
                    page.push(m);
                }
                pos += 1;
            }
        }
        *matches = page;
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
        let query_lower = fold_str(query).into_owned();
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
                    .map(|f| path_filter.matches(&self.root_qualified_path(&f.path)))
                    .unwrap_or(false),
            };
            if keep {
                result.insert(doc_id);
            }
        }
        Cow::Owned(result)
    }

    /// Display path for a file id: precomputed when available.
    /// `ranking::path_signals` of the file, from its metadata when that
    /// has been computed: working them out means a path prefix walk per
    /// candidate file per search.
    pub(super) fn path_signals_for(&self, doc_id: u32, path: &Path) -> (bool, bool) {
        match self.file_metadata.get(doc_id as usize) {
            Some(meta) if !meta.display_path.is_empty() => (meta.is_src_lib, meta.is_test_path),
            _ => crate::search::ranking::path_signals(&self.root_relative_path(path)),
        }
    }

    pub(super) fn display_path_for(&self, doc_id: u32, path: &Path) -> String {
        match self.file_metadata.get(doc_id as usize) {
            Some(meta) if !meta.display_path.is_empty() => {
                meta.display_path[meta.display_start as usize..].to_string()
            }
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
                group_docs |= &*self.text_candidates(literal);
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
            |_, meta| meta.base_score,
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

    /// [`Self::search_symbols`] with explicit limits: the query as literal
    /// text (no operators), searched exactly as the web API, gRPC, MCP and
    /// `fcs` search symbols ([`Self::search_symbols_parsed`]). It used to
    /// run its own single-pass search, so the corpus benchmark and the
    /// validator timed a symbol search that no front end ran.
    pub fn search_symbols_with_limits(
        &self,
        query: &str,
        include_patterns: &str,
        exclude_patterns: &str,
        limits: SearchLimits,
    ) -> Result<(Vec<SearchMatch>, SearchRankingInfo)> {
        let parsed = ParsedQuery {
            text: query.to_string(),
            ..Default::default()
        };
        self.search_symbols_parsed(&parsed, include_patterns, exclude_patterns, limits)
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
        Ok(self.run_references(name, None, path_filter, limits))
    }

    /// [`Self::search_references`] for a parsed query: the text is the
    /// identifier, `file:` / `lang:` operators narrow the files and `kind:`
    /// the kinds of reference.
    pub fn search_references_parsed(
        &self,
        parsed: &ParsedQuery,
        include_patterns: &str,
        exclude_patterns: &str,
        limits: SearchLimits,
    ) -> Result<(Vec<SearchMatch>, SearchRankingInfo)> {
        let name = parsed.text.trim();
        if name.is_empty() {
            return Ok((Vec::new(), SearchRankingInfo::empty(RankMode::Full)));
        }
        let limits = self.limits_with_query_scope(parsed, limits)?;
        let path_filter = merged_path_filter(parsed, include_patterns, exclude_patterns)?;
        Ok(self.run_references(name, parsed.ref_kinds, path_filter, limits))
    }

    fn run_references(
        &self,
        name: &str,
        kinds: Option<u8>,
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
        let candidates = self.apply_path_filter(self.text_candidates(name), &path_filter);
        self.run_candidates(
            &candidates,
            RankMode::Full,
            limits,
            |_, meta| meta.base_score,
            |doc_id, run| self.references_in_document(doc_id, name_id, name, kinds, run),
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
        kinds: Option<u8>,
        run: &QueryRun,
    ) -> Option<Vec<SearchMatch>> {
        let wanted =
            |r: &PackedRef| r.name == name_id && kinds.is_none_or(|k| k & r.kind().bit() != 0);
        // Consult the reference list before touching file content: most
        // candidates mention the text without referencing the symbol.
        let refs = self.references_of(doc_id);
        if !refs.iter().any(wanted) {
            return None;
        }
        let file = self.file_store.get(doc_id)?;
        let content = file.as_str().ok()?;
        let dependency_count = self.dependency_index.get_import_count(doc_id);
        let display_path = self.make_display_path(&file.path);
        // Every hit is an equally good reference, so the file decides: the
        // precomputed score carries the dependency boost and the test,
        // example and fixture penalty, which puts a call site in shipped
        // code above the same call in a test. A flat score would leave the
        // order to file ids, which follow discovery order and differ
        // between machines.
        let score = f64::from(self.get_file_metadata(doc_id).base_score.max(f32::EPSILON));
        let lines: Vec<&str> = content.lines().collect();
        let mut matches = Vec::new();
        let mut last_line = usize::MAX;
        for r in refs.iter().filter(|r| wanted(r)) {
            let line_num = r.line as usize;
            if line_num == last_line {
                continue; // one result per line
            }
            let Some(line) = lines.get(line_num) else {
                continue; // file changed under us; the watcher will refresh it
            };
            // tree-sitter columns are byte offsets into the line as indexed.
            let start = r.column() as usize;
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
                score,
                is_symbol: false,
                is_reference: true,
                reference_kind: Some(r.kind()),
                dependency_count,
                file_hits: 0,
                file_hits_capped: false,
            });
        }
        (!matches.is_empty()).then_some(matches)
    }

    /// Search for symbols matching the query in a document.
    /// Returns matches only for lines where a symbol name matches.
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
                    name.eq_ignore_ascii_case(original_query) || *fold_str(name) == *query_lower
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

        let (is_src_lib, is_test_path) = self.path_signals_for(doc_id, &file.path);
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
                    reference_kind: None,
                    dependency_count,
                    file_hits: 0,
                    file_hits_capped: false,
                });
                continue;
            }

            // Get the line content (symbol.line is 0-based)
            let line = match lines.get(symbol.line) {
                Some(l) => *l,
                None => continue,
            };
            // Symbols are captured at index time. If the file changed on
            // disk since (watching off, or not caught up), the recorded line
            // may now hold something else: report the definition only while
            // its name is still on that line, never an unrelated line.
            if !line.contains(symbol.name.as_str()) {
                continue;
            }

            // Find where the symbol name appears in the line
            let (match_start, match_end) =
                match find_match_position_case_insensitive(line, query_lower) {
                    Some(pos) => pos,
                    None => {
                        // Try to find the symbol name instead
                        let name_lower = fold_str(&symbol.name).into_owned();
                        find_match_position_case_insensitive(line, &name_lower).unwrap_or((0, 0))
                    }
                };

            // Calculate score - symbols always get the symbol definition boost
            let base_score = calculate_score_inline(
                line,
                (match_start, match_end),
                original_query,
                query_lower,
                true,
                is_src_lib,
                is_test_path,
                dependency_boost,
            );

            // exact > prefix > substring, and definitions of types/functions
            // slightly above variables/constants with the same match quality.
            let w = &RankingWeights::DEFAULT;
            let name_lower = fold_str(&symbol.name).into_owned();
            let name_factor = if name_lower == query_lower {
                w.symbol_exact_name
            } else if name_lower.starts_with(query_lower) {
                w.symbol_prefix_name
            } else {
                1.0
            };
            let kind_factor = match symbol.symbol_type {
                SymbolType::Variable | SymbolType::Constant => w.symbol_value_kind,
                // `impl Runtime {` is a definition too, but the reader asking
                // for `Runtime` wants the struct first.
                _ if {
                    let t = line.trim_start();
                    let t = t.strip_prefix("unsafe ").unwrap_or(t).trim_start();
                    t.starts_with("impl ") || t.starts_with("impl<")
                } =>
                {
                    w.symbol_impl_kind
                }
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
                reference_kind: None,
                dependency_count,
                file_hits: 0,
                file_hits_capped: false,
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

        // The file's best hits up to its cap (not its first ones).
        let mut top = TopHits::new(run.per_doc_cap());
        let mut seen = 0usize;

        // Lazy-compute path info and symbol lookups only if we find matches
        let mut display_path: Option<String> = None;
        let mut is_src_lib = false;
        let mut is_test_path = false;
        let mut symbol_maps: Option<SymbolLineMaps<'_>> = None;

        {
            // Turn one matching line into a result. Returns false to stop
            // (the deadline passed).
            let mut emit = |line_num: usize, line: &str, m_start: usize, m_end: usize| -> bool {
                seen += 1;
                if seen.is_multiple_of(4096) && run.past_deadline() {
                    return false;
                }
                let path_ref = display_path.get_or_insert_with(|| {
                    (is_src_lib, is_test_path) = self.path_signals_for(doc_id, &file.path);
                    self.make_display_path(&file.path)
                });
                let maps = symbol_maps.get_or_insert_with(|| SymbolLineMaps::build(symbols));

                let is_symbol_def = maps.is_definition_line(line_num, line);
                let score = calculate_score_regex_inline(
                    line,
                    regex,
                    is_symbol_def,
                    is_src_lib,
                    is_test_path,
                    dependency_boost,
                );
                if !top.admits(score) {
                    return true;
                }

                // Is a symbol whose name matches the regex on this line?
                let is_symbol = maps
                    .names_on_line(line_num)
                    .is_some_and(|names| names.iter().any(|name| regex.is_match(name)));

                // Truncate long lines around the match
                let truncated = truncate_around_match(line, m_start, m_end);

                top.push(SearchMatch {
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
                    reference_kind: None,
                    dependency_count,
                    file_hits: 0,
                    file_hits_capped: false,
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
                    if past_last_line(text, m.start()) {
                        break;
                    }
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
                // Line-mode matching in one pass over the whole content:
                // the regex engine's prefilter and start-up cost is paid
                // once per file instead of once per line (a `\s+` pattern
                // was ten times slower than a plain literal over the same
                // candidates). The regex is compiled with `(?mR)` so `^`
                // and `$` keep their per-line meaning; one match is reported
                // per line, and a match that would span a line break is
                // resolved by re-running the regex on the lines it touches.
                let text: &str = &content;
                let mut pos = 0usize;
                let mut line_num = 0usize;
                'scan: while pos <= text.len() {
                    let Some(m) = regex.find_at(text, pos) else {
                        break;
                    };
                    if past_last_line(text, m.start()) {
                        break;
                    }
                    line_num += text[pos..m.start()].bytes().filter(|&b| b == b'\n').count();
                    let line_start = text[..m.start()].rfind('\n').map_or(0, |i| i + 1);
                    let mut line_end = text[m.start()..]
                        .find('\n')
                        .map_or(text.len(), |i| i + m.start());
                    if m.end() <= line_end {
                        let raw = &text[line_start..line_end];
                        let line = raw.strip_suffix('\r').unwrap_or(raw);
                        let m_start = (m.start() - line_start).min(line.len());
                        let m_end = (m.end() - line_start).min(line.len());
                        if !emit(line_num, line, m_start, m_end) {
                            break;
                        }
                    } else {
                        // The match crosses a line break (e.g. `\s+`): per-line
                        // semantics evaluate each line on its own.
                        let span_end = text[m.end()..]
                            .find('\n')
                            .map_or(text.len(), |i| i + m.end());
                        let mut ln = line_num;
                        // When the span runs to the end of a file that ends
                        // in a newline, `split` yields an empty piece after
                        // it that is no line of the file.
                        let span = &text[line_start..span_end];
                        let span = if span_end == text.len() {
                            span.strip_suffix('\n').unwrap_or(span)
                        } else {
                            span
                        };
                        for raw in span.split('\n') {
                            let line = raw.strip_suffix('\r').unwrap_or(raw);
                            if let Some(lm) = regex.find(line) {
                                if !emit(ln, line, lm.start(), lm.end()) {
                                    break 'scan;
                                }
                            }
                            ln += 1;
                        }
                        line_num = ln - 1;
                        line_end = span_end;
                    }
                    if line_end >= text.len() {
                        break;
                    }
                    pos = line_end + 1;
                    line_num += 1;
                }
            }
        }

        let mut matches = top.finish(run);

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
                    reference_kind: None,
                    dependency_count,
                    file_hits: 0,
                    file_hits_capped: false,
                });
            }
        }

        if matches.is_empty() {
            None
        } else {
            Some(matches)
        }
    }

    /// Plain-text document scan: every line holding the text, scored
    /// against it.
    pub(super) fn search_in_document_terms(
        &self,
        doc_id: u32,
        terms: &TermSet,
        run: &QueryRun,
    ) -> Option<Vec<SearchMatch>> {
        let (original_query, query_lower) = (terms.term.0.as_str(), terms.term.1.as_str());
        let opts = terms.opts;
        let file = self.file_store.get(doc_id)?;
        let content = file.as_str().ok()?;

        // Every hit line, in document order (the ASCII case-insensitive
        // scan when no option is set).
        let all_hits = if !opts.case_sensitive
            && !opts.whole_word
            && ascii_fold_suffices(&content, query_lower)
        {
            ascii_ci_line_hits(&content, query_lower)
        } else {
            line_hits(&content, original_query, query_lower, opts)
        };

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

        // The file's best hits up to its cap (not its first ones).
        let mut top = TopHits::new(run.per_doc_cap());

        // Everything below is built lazily on the FIRST match: most candidates
        // (trigram hits that fail verification, or files whose only hit is
        // beyond the budget) must cost nothing beyond the scan itself.
        let mut display_path: Option<String> = None;
        let mut is_src_lib = false;
        let mut is_test_path = false;
        let mut symbol_maps: Option<SymbolLineMaps<'_>> = None;

        for (n, hit) in all_hits.into_iter().enumerate() {
            if n % 4096 == 4095 && run.past_deadline() {
                break;
            }
            let (line_num, line, match_start, match_end) =
                (hit.line_num, hit.line, hit.start, hit.end);
            let path_ref = display_path.get_or_insert_with(|| {
                (is_src_lib, is_test_path) = self.path_signals_for(doc_id, &file.path);
                self.display_path_for(doc_id, &file.path)
            });
            let maps = symbol_maps.get_or_insert_with(|| SymbolLineMaps::build(symbols));

            let is_symbol_def = maps.is_definition_line(line_num, line);
            let score = calculate_score_inline(
                line,
                (match_start, match_end),
                original_query,
                query_lower,
                is_symbol_def,
                is_src_lib,
                is_test_path,
                dependency_boost,
            );
            if !top.admits(score) {
                continue;
            }
            let is_symbol = maps.names_on_line(line_num).is_some_and(|names| {
                names
                    .iter()
                    .any(|n| contains_case_insensitive(n, query_lower))
            });

            let truncated = truncate_around_match(line, match_start, match_end);
            top.push(SearchMatch {
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
                reference_kind: None,
                dependency_count,
                file_hits: 0,
                file_hits_capped: false,
            });
        }
        let mut matches = top.finish(run);

        // If no content matches, check if the query matches the filename.
        // The filename is indexed into the trigram index (so the file became a candidate),
        // but it doesn't appear in the file content. Synthesize a result so the user
        // sees the file in search results.
        if matches.is_empty() {
            let has_filename_match = symbols
                .iter()
                .any(|s| s.symbol_type == SymbolType::FileName && terms.matches_file_name(&s.name));
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
                    reference_kind: None,
                    dependency_count,
                    file_hits: 0,
                    file_hits_capped: false,
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
