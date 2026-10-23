use crate::dependencies::DependencyIndex;
use crate::index::{extract_unique_trigrams_lowercase, LazyFileStore, Trigram, TrigramIndex};
use crate::search::fold::{ascii_fold_suffices, fold_char, fold_str};
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

mod freshness;
mod impact;
mod invariants;
mod paths;
mod persist;
mod progress;
mod query;
mod scope;
mod text;

pub use freshness::{
    unix_now_ms, IndexFreshness, PathFreshness, ReadFailures, WatchBacklog, MAX_READ_ATTEMPTS,
};
pub use impact::{
    ImpactFileSummary, ImpactOptions, ImpactReport, ImpactScope, SymbolImpact, TierCounts, UseLine,
    UseTier, UserFile, DEFAULT_IMPACT_FILES, DEFAULT_IMPACT_LINES, MAX_IMPACT_SYMBOLS,
};
pub use invariants::{FileCheck, InvariantReport};
pub use paths::{PathMatch, PathQuery, PathSearch};
pub use progress::*;
pub use scope::{GraphScope, MAX_SCOPE_HOPS};
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
    /// For a reference: what kind of use it is (call, type, value,
    /// attribute).
    pub reference_kind: Option<crate::symbols::extractor::RefKind>,
    pub dependency_count: u32,
    /// Hits this search found in the match's file, across every page (a
    /// file cut at a page boundary has only some of them on the page).
    /// Filled in when the results are paged.
    pub file_hits: u32,
    /// The file may hold more hits than `file_hits`: it stopped at the
    /// per-file cap or the match budget.
    pub file_hits_capped: bool,
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
    /// Every limit that cut this search short, in [`SearchLimit`] order.
    /// Empty when the results are complete.
    pub limits_hit: Vec<SearchLimit>,
}

/// A limit that stopped a search before it saw everything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SearchLimit {
    /// Fast ranking opened only the best `max_files` candidate files
    /// (`server.max_fast_rank_files`, the request's `max_files`). The total
    /// counts matches in the files that were opened.
    FileCap,
    /// The match budget ran out: the results are the best of those seen.
    MatchBudget,
    /// A file had more matching lines than one file may contribute.
    PerFileCap,
    /// The deadline passed (the request's timeout, or the server's).
    Deadline,
    /// The index was still being built or reconciled with the disk: files
    /// not indexed yet could not match.
    Indexing,
}

impl SearchLimit {
    /// Stable name used by the web API, gRPC, `fcs --json` and MCP.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FileCap => "max_files",
            Self::MatchBudget => "match_budget",
            Self::PerFileCap => "per_file_cap",
            Self::Deadline => "timeout",
            Self::Indexing => "indexing",
        }
    }

    /// Parse [`Self::as_str`].
    pub fn parse(s: &str) -> Option<Self> {
        [
            Self::FileCap,
            Self::MatchBudget,
            Self::PerFileCap,
            Self::Deadline,
            Self::Indexing,
        ]
        .into_iter()
        .find(|l| l.as_str() == s)
    }

    /// One sentence for people, naming the limit and how to get past it.
    pub fn describe(self, info: &SearchRankingInfo) -> String {
        match self {
            Self::FileCap => format!(
                "searched the best {} of {} candidate files (max_files); narrow the query or raise the file limit to search more",
                info.candidates_searched, info.total_candidates
            ),
            Self::MatchBudget => {
                "stopped after collecting enough matches to rank (match budget); these are the best of those seen".to_string()
            }
            Self::PerFileCap => {
                "at least one file had more matching lines than one file may show (per-file cap)".to_string()
            }
            Self::Deadline => {
                "stopped at the time limit (timeout); results come from the files searched so far".to_string()
            }
            Self::Indexing => {
                "the index is still being built (indexing); files not indexed yet are not searched, so search again when it completes".to_string()
            }
        }
    }
}

impl SearchRankingInfo {
    /// Whether any limit cut the search short.
    pub fn is_capped(&self) -> bool {
        !self.limits_hit.is_empty()
    }
}

impl SearchRankingInfo {
    fn empty(mode: RankMode) -> Self {
        Self {
            mode,
            total_candidates: 0,
            candidates_searched: 0,
            truncated_by_budget: false,
            total_matches: Some(0),
            limits_hit: Vec::new(),
        }
    }
}

/// Limits that bound the work a single query may do.
///
/// `match_budget` caps the number of matches materialized across all
/// documents (each costs a `String`); once it is exhausted no further
/// documents are opened. `deadline` stops the scan at a wall-clock time.
/// Both are reported through [`SearchRankingInfo::truncated_by_budget`], and
/// every limit that cut a search is named in
/// [`SearchRankingInfo::limits_hit`].
#[derive(Debug, Clone)]
pub struct SearchLimits {
    /// Page size.
    pub max_results: usize,
    /// Results to skip (for paging); the ordering is deterministic.
    pub offset: usize,
    /// Maximum matches to materialize before stopping.
    pub match_budget: usize,
    /// Wall-clock deadline for the scan.
    pub deadline: Option<std::time::Instant>,
    /// Most hits one document may contribute; `None` means the default
    /// (see `doc_cap`).
    pub per_doc_cap: Option<usize>,
    /// Most candidate files fast ranking opens (the best by metadata;
    /// `usize::MAX` = no limit, the default).
    pub max_files: usize,
    /// Only search these files (from `deps:` / `rdeps:`).
    pub scope: Option<std::sync::Arc<GraphScope>>,
    /// How the match budget and per-file cap follow from the page.
    pub budget_rule: BudgetRule,
    /// Full ranking: never stop at a file limit. Symbol search then reads
    /// every file holding a matching symbol instead of the best ones.
    pub every_file: bool,
}

/// How much a search collects to rank a page (`[search]` in the config).
///
/// The defaults are measured, not guessed: on a 372k-file corpus (Linux,
/// LLVM, Rust, Kubernetes) a 32,768-match floor put 81% of the exhaustive
/// top 50 on page 1 for tokens with over 100k hits (10% at the old 512),
/// at about 80 ms median; raising it further bought under one point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetRule {
    /// Never budget fewer matches than this.
    pub min: usize,
    /// Budget this many times the page end, between `min` and `max`.
    pub multiplier: usize,
    /// Ceiling on the derived budget (pages ending at
    /// [`SearchLimits::STABLE_PAGING_FROM`] or deeper get exactly this).
    pub max: usize,
    /// Most hits one file contributes (its best, by score) to pages ending
    /// before [`SearchLimits::STABLE_PAGING_FROM`]. The default equals that
    /// page end, so no file's hit that could rank in the first 1000
    /// results is left out.
    pub per_file: usize,
}

impl BudgetRule {
    /// The defaults (`[search]` with nothing set).
    pub const DEFAULT: BudgetRule = BudgetRule {
        min: 32_768,
        multiplier: 8,
        max: 100_000,
        per_file: SearchLimits::STABLE_PAGING_FROM,
    };
}

impl Default for BudgetRule {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl SearchLimits {
    /// Most hits one document may contribute (its best ones, by score):
    /// `per_doc_cap`, or by default the budget rule's `per_file` (1000) for
    /// every page ending before [`Self::STABLE_PAGING_FROM`], and the
    /// page's budget from there on. Both are the same for every page in
    /// their range, so offset pages rank the same sample and fit together;
    /// a cap that grew with the page end ranked a different sample for
    /// each page, and walking a busy file by `offset` repeated some of its
    /// hits and never showed others.
    pub fn doc_cap(&self) -> usize {
        self.per_doc_cap.unwrap_or_else(|| {
            let end = self.offset.saturating_add(self.max_results);
            if end >= Self::STABLE_PAGING_FROM {
                self.derived_budget(self.offset, self.max_results)
            } else {
                self.budget_rule.per_file.max(1)
            }
        })
    }

    /// Ceiling on the ranking headroom derived from page size and offset by
    /// default ([`BudgetRule::max`]). A deeper page still gets enough budget
    /// to reach it; an explicit [`Self::with_match_budget`] may exceed it
    /// (benchmarks, tests).
    pub const MAX_DERIVED_BUDGET: usize = BudgetRule::DEFAULT.max;
    /// Pages ending here or deeper all get the same budget
    /// ([`BudgetRule::max`]) and per-document cap, so they rank the
    /// same sample of a broad query's hits and offset pages fit together.
    /// A budget that grew with the offset ranked a different sample for
    /// each page, so LOAD MORE past the API's 1000-result list repeated
    /// some hits and skipped others. Shorter lists keep a smaller budget
    /// (fast first pages); each is fetched whole, from offset 0.
    pub const STABLE_PAGING_FROM: usize = 1000;

    /// Limits for a page of `max_results` with the default budget rule and
    /// no file cap.
    pub fn new(max_results: usize) -> Self {
        let max_results = max_results.max(1);
        let mut limits = Self {
            max_results,
            offset: 0,
            match_budget: 0,
            deadline: None,
            per_doc_cap: None,
            max_files: usize::MAX,
            scope: None,
            budget_rule: BudgetRule::DEFAULT,
            every_file: false,
        };
        limits.match_budget = limits.derived_budget(0, max_results);
        limits
    }

    /// Derive the budget and per-file cap from `rule` (the server's
    /// `[search]` settings) instead of the defaults.
    pub fn with_budget_rule(mut self, rule: BudgetRule) -> Self {
        self.budget_rule = rule;
        self.match_budget = self.derived_budget(self.offset, self.max_results);
        self
    }

    /// Only search the files in `scope`.
    pub fn with_scope(mut self, scope: Option<std::sync::Arc<GraphScope>>) -> Self {
        self.scope = scope;
        self
    }

    /// Budget for a page of `max_results` starting at `offset`: the rule's
    /// multiple of the page end for ranking headroom, between its `min` and
    /// `max`, or exactly `max` once the page ends at
    /// [`Self::STABLE_PAGING_FROM`] or deeper, and never less than the page
    /// end plus `min`, so a deep page can always be reached. Servers bound
    /// how deep a client may page with [`SearchSettings::max_offset`].
    fn derived_budget(&self, offset: usize, max_results: usize) -> usize {
        let BudgetRule {
            min,
            multiplier,
            max,
            ..
        } = self.budget_rule;
        let (min, max) = (min.max(1), max.max(min.max(1)));
        let end = offset.saturating_add(max_results);
        if end < Self::STABLE_PAGING_FROM {
            // One budget for every page in this range (the one the last of
            // them would get), so they all rank the same sample.
            return Self::STABLE_PAGING_FROM
                .saturating_mul(multiplier.max(1))
                .clamp(min, max)
                .max(Self::STABLE_PAGING_FROM);
        }
        max.max(end.saturating_add(min))
    }

    /// Skip the first `offset` results; the budget grows to cover the page.
    /// Callers serving clients should check the offset against
    /// [`SearchSettings::check_offset`] first.
    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self.match_budget = self
            .match_budget
            .max(self.derived_budget(self.offset, self.max_results));
        self
    }

    /// Override the match budget (`usize::MAX` = unbounded, the old behaviour).
    pub fn with_match_budget(mut self, budget: usize) -> Self {
        self.match_budget = budget.max(1);
        self
    }

    /// Override the per-document hit cap (`usize::MAX` = every hit, for
    /// tools that need the complete result set, such as the ripgrep
    /// backtest).
    pub fn with_per_doc_cap(mut self, cap: usize) -> Self {
        self.per_doc_cap = Some(cap.max(1));
        self
    }

    /// Every hit: no page size, budget or per-document cap. For
    /// correctness tooling only; a broad query materializes everything.
    pub fn exhaustive() -> Self {
        Self::new(usize::MAX)
            .with_match_budget(usize::MAX)
            .with_per_doc_cap(usize::MAX)
    }

    /// Open at most `n` candidate files under fast ranking (`0` = no limit).
    pub fn with_max_files(mut self, n: usize) -> Self {
        self.max_files = if n == 0 { usize::MAX } else { n };
        self
    }

    /// Never stop at a file limit (full ranking; see [`Self::every_file`]).
    pub fn with_every_file(mut self, every_file: bool) -> Self {
        self.every_file = every_file;
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

/// Server-wide search limits, from `[search]` in the configuration. Every
/// surface (REST, MCP, gRPC, `fcs`) applies them to client requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchSettings {
    /// Candidate files fast ranking opens when the request does not say
    /// (`usize::MAX` = no limit, the default: read until the budget is
    /// spent).
    pub fast_rank_files: usize,
    /// Most candidate files a request may ask fast ranking to open
    /// (`usize::MAX` = no limit).
    pub max_fast_rank_files: usize,
    /// Deepest `offset` a request may page to (`usize::MAX` = no limit).
    pub max_offset: usize,
    /// Match budget and per-file cap.
    pub budget: BudgetRule,
    /// Page size when a request does not give one.
    pub default_page_size: usize,
    /// Largest page one request may ask for.
    pub max_page_size: usize,
    /// Most path matches one request may ask for.
    pub max_path_matches: usize,
    /// Longest deadline a request may ask for.
    pub max_timeout: std::time::Duration,
    /// Most context lines around each hit.
    pub max_context_lines: usize,
}

impl Default for SearchSettings {
    fn default() -> Self {
        Self {
            fast_rank_files: usize::MAX,
            max_fast_rank_files: usize::MAX,
            max_offset: Self::DEFAULT_MAX_OFFSET,
            budget: BudgetRule::DEFAULT,
            default_page_size: 50,
            max_page_size: 1000,
            max_path_matches: 100,
            max_timeout: std::time::Duration::from_secs(30),
            max_context_lines: 10,
        }
    }
}

impl SearchSettings {
    /// Default deepest offset. Each page re-runs the search with a budget
    /// that covers the offset, so a deep page costs a scan of that many
    /// matches.
    pub const DEFAULT_MAX_OFFSET: usize = 100_000;

    /// Settings from the configuration, where 0 means no limit for the file
    /// counts and the offset. `[search]` wins over the deprecated `[server]`
    /// spellings of `fast_rank_files`, `max_fast_rank_files` and
    /// `max_page_offset`.
    pub fn from_config(c: &crate::config::Config) -> Self {
        let unlimited_if_zero = |n: usize| if n == 0 { usize::MAX } else { n };
        let (s, server) = (&c.search, &c.server);
        let d = Self::default();
        Self {
            fast_rank_files: unlimited_if_zero(
                s.fast_rank_files.or(server.fast_rank_files).unwrap_or(0),
            ),
            max_fast_rank_files: unlimited_if_zero(
                s.max_fast_rank_files
                    .or(server.max_fast_rank_files)
                    .unwrap_or(0),
            ),
            max_offset: unlimited_if_zero(
                s.max_page_offset
                    .or(server.max_page_offset)
                    .unwrap_or(Self::DEFAULT_MAX_OFFSET),
            ),
            budget: BudgetRule {
                min: s.min_match_budget.max(1),
                multiplier: s.match_budget_multiplier.max(1),
                max: s.max_match_budget.max(s.min_match_budget.max(1)),
                per_file: s.per_file_matches.max(1),
            },
            default_page_size: s.default_page_size.clamp(1, s.max_page_size.max(1)),
            max_page_size: s.max_page_size.max(1),
            max_path_matches: s.max_path_matches,
            max_timeout: if s.max_timeout_secs == 0 {
                d.max_timeout
            } else {
                std::time::Duration::from_secs(s.max_timeout_secs)
            },
            max_context_lines: s.max_context_lines,
        }
    }

    /// The page size a request asking for `requested` gets (0 = the
    /// default), capped at [`Self::max_page_size`].
    pub fn page_size(&self, requested: usize) -> usize {
        if requested == 0 {
            self.default_page_size
        } else {
            requested.min(self.max_page_size)
        }
    }

    /// Files fast ranking may open for a request asking for `requested`
    /// (`None` or 0 = the server's default), capped by the server's limit.
    pub fn fast_rank_files_for(&self, requested: Option<usize>) -> usize {
        requested
            .filter(|&n| n > 0)
            .unwrap_or(self.fast_rank_files)
            .min(self.max_fast_rank_files)
    }

    /// Reject an `offset` deeper than the server allows.
    pub fn check_offset(&self, offset: usize) -> std::result::Result<(), String> {
        if offset > self.max_offset {
            return Err(format!(
                "offset must be at most {} (search.max_page_offset); narrow the query instead of paging deeper",
                self.max_offset
            ));
        }
        Ok(())
    }

    /// `limits` with this server's offset-independent settings applied for
    /// a request asking fast ranking to open `requested_files`.
    pub fn apply(&self, limits: SearchLimits, requested_files: Option<usize>) -> SearchLimits {
        let n = self.fast_rank_files_for(requested_files);
        limits
            .with_budget_rule(self.budget)
            .with_max_files(if n == usize::MAX { 0 } else { n })
    }
}

/// Shared per-query state consulted by every worker: remaining match budget
/// and deadline. Cheap enough to check per document and per match.
pub struct QueryRun {
    remaining: std::sync::atomic::AtomicUsize,
    deadline: Option<std::time::Instant>,
    /// [`QueryRun::CAP`], [`QueryRun::BUDGET`] and [`QueryRun::DEADLINE`]
    /// bits for the limits that stopped this run.
    truncated: std::sync::atomic::AtomicU8,
    /// Most hits one document may contribute. Grows with the page offset so
    /// "load more" can reach a busy file's later lines.
    per_doc_cap: usize,
}

impl QueryRun {
    fn new(limits: &SearchLimits) -> Self {
        Self {
            remaining: std::sync::atomic::AtomicUsize::new(limits.match_budget),
            deadline: limits.deadline,
            truncated: std::sync::atomic::AtomicU8::new(0),
            per_doc_cap: limits.doc_cap(),
        }
    }

    /// Reserve one unit of budget for a document that already holds
    /// `held` hits. A document at its cap marks the run truncated: the
    /// total is then unknown, rather than reported as an exact count that
    /// silently leaves out the file's remaining lines.
    pub fn take_doc_match(&self, held: usize) -> bool {
        if held >= self.per_doc_cap {
            self.mark(Self::CAP);
            return false;
        }
        self.take_match()
    }

    /// True once the budget is spent or the deadline passed; callers must
    /// not open further documents.
    pub fn exhausted(&self) -> bool {
        use std::sync::atomic::Ordering::Relaxed;
        if self.remaining.load(Relaxed) == 0 {
            self.mark(Self::BUDGET);
            return true;
        }
        if let Some(d) = self.deadline {
            if std::time::Instant::now() >= d {
                self.mark(Self::DEADLINE);
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
                self.mark(Self::BUDGET);
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

    const CAP: u8 = 1;
    const BUDGET: u8 = 2;
    const DEADLINE: u8 = 4;

    fn mark(&self, bit: u8) {
        self.truncated
            .fetch_or(bit, std::sync::atomic::Ordering::Relaxed);
    }

    /// The bits of the limits that stopped this run (0 = none).
    fn stopped_by(&self) -> u8 {
        self.truncated.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Most hits one document may contribute (see [`SearchLimits::doc_cap`]).
    pub(super) fn per_doc_cap(&self) -> usize {
        self.per_doc_cap
    }

    /// A document had more hits than its cap; the best were kept.
    pub(super) fn mark_capped(&self) {
        self.mark(Self::CAP);
    }

    /// The deadline has passed (checked every so often inside a big file).
    pub(super) fn past_deadline(&self) -> bool {
        self.deadline
            .is_some_and(|d| std::time::Instant::now() >= d)
    }
}

/// One document's hits, keeping its best `cap` by score rather than its
/// first `cap` in file order. A file with more hits than the cap used to
/// contribute its earliest lines, so page 1 showed a busy file's weakest
/// hits while its best ones (further down) were never read.
///
/// Memory stays bounded at twice the cap: when the buffer fills it is
/// sorted and cut back to the cap, and from then on a hit is only built
/// when it beats the weakest one kept. Ties go to the earlier line, as in
/// the final ordering.
pub(super) struct TopHits {
    cap: usize,
    items: Vec<SearchMatch>,
    /// Score a new hit must beat once the buffer has been cut.
    floor: Option<f64>,
}

impl TopHits {
    pub(super) fn new(cap: usize) -> Self {
        Self {
            cap: cap.max(1),
            items: Vec::with_capacity(8),
            floor: None,
        }
    }

    /// Whether a hit scoring `score` would be kept (so is worth building).
    #[inline]
    pub(super) fn admits(&self, score: f64) -> bool {
        self.floor.is_none_or(|f| score > f)
    }

    pub(super) fn push(&mut self, m: SearchMatch) {
        self.items.push(m);
        if self.items.len() >= self.cap.saturating_mul(2) {
            self.compact();
        }
    }

    fn compact(&mut self) {
        sort_doc_hits(&mut self.items);
        self.items.truncate(self.cap);
        self.floor = self.items.last().map(|m| m.score);
    }

    /// The kept hits, best first; marks the run capped when any were left
    /// out.
    pub(super) fn finish(mut self, run: &QueryRun) -> Vec<SearchMatch> {
        let overflowed = self.floor.is_some() || self.items.len() > self.cap;
        if overflowed {
            run.mark_capped();
        }
        self.compact();
        self.items
    }
}

/// One document's hits best first, ties by line.
pub(super) fn sort_doc_hits(hits: &mut [SearchMatch]) {
    hits.sort_unstable_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.line_number.cmp(&b.line_number))
    });
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
    /// Modification time (nanoseconds since the epoch) of the file *as
    /// read*.
    pub mtime: u64,
    /// Read so soon after it was modified that its size and time cannot
    /// vouch for its content later (within 3 seconds of the change).
    pub racy: bool,
    /// Size in bytes of the file *as read*.
    pub size: u64,
    /// Full-precision modification time of the file *as read*, used to tell
    /// whether the file changed between the read and the merge (`mtime` is
    /// whole seconds, too coarse for that).
    pub modified: Option<std::time::SystemTime>,
}

/// A file read this soon after it was modified may be changed again within
/// the same timestamp tick (a second or two on FAT, HFS+ and some network
/// shares), and a later stat would then see the same size and time with
/// different content. Such a file is marked *racy*: a reconcile or restart
/// reads it again instead of trusting its size and time, as git does with
/// "racily clean" index entries.
const RACY_WINDOW: std::time::Duration = std::time::Duration::from_secs(3);

fn is_racy(modified: std::time::SystemTime) -> bool {
    match std::time::SystemTime::now().duration_since(modified) {
        Ok(age) => age < RACY_WINDOW,
        // Modified "in the future" (clock skew): don't trust it either.
        Err(_) => true,
    }
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
        Self::process_checked(path, transcode_non_utf8, max_file_size)
            .ok()
            .flatten()
    }

    /// [`Self::process`], telling a file that could not be read (`Err`:
    /// permission denied, a sharing violation, an I/O error) apart from one
    /// that is not indexed (`Ok(None)`: gone, too large, binary). The
    /// watcher retries the first and drops the second.
    pub fn process_checked(
        path: &Path,
        transcode_non_utf8: bool,
        max_file_size: u64,
    ) -> std::io::Result<Option<(Self, bool)>> {
        match Self::process_inner(path, transcode_non_utf8, max_file_size) {
            Ok(file) => Ok(Some(file)),
            Err(Unindexed::Skipped) => Ok(None),
            Err(Unindexed::Unreadable(e)) => Err(e),
        }
    }

    fn process_inner(
        path: &Path,
        transcode_non_utf8: bool,
        max_file_size: u64,
    ) -> Result<(Self, bool), Unindexed> {
        let max_size = if max_file_size == 0 {
            Self::DEFAULT_MAX_FILE_SIZE
        } else {
            max_file_size
        };
        let metadata = std::fs::metadata(path)?;
        if metadata.len() > max_size {
            return Err(Unindexed::Skipped);
        }
        // Capture what we are about to read so persistence records the indexed
        // content's identity, not the on-disk state at save time.
        let size = metadata.len();
        let mtime = crate::index::persistence::mtime_secs_of(&metadata);
        let modified = metadata.modified().ok();
        let racy = modified.is_none_or(is_racy);

        crate::faults::io("parse.read", path)?;
        let raw_bytes = std::fs::read(path)?;
        // Attempt zero-copy consume: String::from_utf8 reuses the Vec allocation when valid.
        let (content, transcoded) = match String::from_utf8(raw_bytes) {
            Ok(s) => (crate::utils::strip_utf8_bom_owned(s), false), // UTF-8 fast path — zero-copy consume
            Err(e) => {
                if !transcode_non_utf8 {
                    return Err(Unindexed::Skipped); // Transcoding disabled
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
                    _ => return Err(Unindexed::Skipped), // Binary or unrecognizable
                }
            }
        };

        // Binary content masquerading as UTF-8 is not indexed at all.
        if crate::utils::is_binary_content(&content) {
            tracing::debug!(
                path = %path.display(),
                "Skipping binary-looking file during indexing"
            );
            return Err(Unindexed::Skipped);
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

        Ok((
            PartialIndexedFile {
                path: path.to_path_buf(),
                trigrams,
                filename_stem,
                content,
                tree_sitter_safe,
                mtime,
                racy,
                size,
                modified,
            },
            transcoded,
        ))
    }
}

/// Why [`PartialIndexedFile::process_inner`] produced no file.
enum Unindexed {
    /// Not indexed by the rules: gone, too large, binary, not UTF-8.
    Skipped,
    /// Could not be read.
    Unreadable(std::io::Error),
}

impl From<std::io::Error> for Unindexed {
    fn from(e: std::io::Error) -> Self {
        // A file that is gone is simply not indexed; its delete event (or
        // the sibling check) removes it.
        if e.kind() == std::io::ErrorKind::NotFound {
            Unindexed::Skipped
        } else {
            Unindexed::Unreadable(e)
        }
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
    /// Modification time (nanoseconds since the epoch) of the content that
    /// was indexed
    pub mtime: u64,
    /// See [`PartialIndexedFile::racy`].
    pub racy: bool,
    /// Size in bytes of the content that was indexed
    pub size: u64,
    /// Full-precision modification time of the content that was indexed
    /// (see [`PartialIndexedFile::modified`]).
    pub modified: Option<std::time::SystemTime>,
}

/// How the engine reads and parses a file, copied out of the engine so
/// files can be parsed without holding its lock (see
/// [`SearchEngine::install_parsed`]).
#[derive(Debug, Clone, Copy)]
pub struct FileParser {
    transcode_non_utf8: bool,
    max_file_size: u64,
    enable_symbols: bool,
}

impl FileParser {
    /// Read and parse `path`; `None` when it is unreadable, binary or too
    /// large to index. Reads owned content (no live mmap, so a file being
    /// rewritten concurrently cannot SIGBUS us).
    pub fn parse(&self, path: &Path) -> Option<PreIndexedFile> {
        self.parse_checked(path).ok().flatten()
    }

    /// [`Self::parse`], with `Err` for a file that could not be read (see
    /// [`PartialIndexedFile::process_checked`]).
    pub fn parse_checked(&self, path: &Path) -> std::io::Result<Option<PreIndexedFile>> {
        Ok(
            PartialIndexedFile::process_checked(path, self.transcode_non_utf8, self.max_file_size)?
                .map(|(partial, _)| PreIndexedFile::from_partial(partial, self.enable_symbols)),
        )
    }
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
            racy: partial.racy,
            size: partial.size,
            modified: partial.modified,
        }
    }

    /// Is the file on disk still the content this was built from? False if
    /// it was deleted or rewritten since it was read.
    fn still_on_disk(&self) -> bool {
        match std::fs::metadata(&self.path) {
            Ok(meta) => meta.len() == self.size && meta.modified().ok() == self.modified,
            Err(_) => false,
        }
    }
}

/// Ranking mode for search queries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RankMode {
    /// The default: the same as [`RankMode::Fast`].
    #[default]
    Auto,
    /// Read candidate files best first (by a per-file score that needs no
    /// I/O) until the match budget is spent, opening at most `max_files`.
    Fast,
    /// As `Fast`, but never limited to `max_files`.
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
    /// Root-qualified path (`<root name>/<relative>`), precomputed so path
    /// filters and result construction never allocate per candidate. Path
    /// filters always match this form.
    pub display_path: String,
    /// Byte offset in `display_path` where the path shown in results starts:
    /// 0, or past `<root name>/` when the root folder's name is hidden.
    pub display_start: u32,
}

impl FileMetadata {
    /// Compute metadata for a file at index time
    ///
    /// `relative` is the path under its indexed root: the folders above the
    /// root must not change a file's score.
    fn compute(
        path: &Path,
        relative: &str,
        (display_path, display_start): (String, usize),
        symbol_count: usize,
        dependency_count: u32,
    ) -> Self {
        let w = &FileScoreWeights::DEFAULT;
        let mut base_score: f32 = w.base;

        // Check if in source directories
        let (is_src_lib, is_test) = crate::search::ranking::path_signals(relative);

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

        // Penalty for test/example/mock directories (same predicate as the
        // line-level score)
        if is_test {
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
            display_start: display_start as u32,
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

/// Importers still to resolve again at the end of a build; see
/// [`SearchEngine::begin_settling_imports`].
#[derive(Debug, Default)]
pub struct ImportSettling {
    importers: Vec<u32>,
}

impl ImportSettling {
    /// Importers not resolved yet.
    pub fn remaining(&self) -> usize {
        self.importers.len()
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
    /// (mtime, size) of each file's content *as indexed*, by file id.
    /// `(0, 0)` means unknown (fall back to a stat at save time).
    indexed_meta: Vec<(u64, u64)>,
    /// Files whose (mtime, size) cannot vouch for their content: they were
    /// read within [`RACY_WINDOW`] of being modified.
    racy_files: roaring::RoaringBitmap,
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
    /// Indices into `waiting_imports` of imports written as project paths
    /// (`./x`, `crate::x`): the few that point at a broken import, counted
    /// per `rdeps:`/`deps:` scope without reading every parked import.
    waiting_local: Vec<usize>,
    /// Every import of every file, as written, by file id. The graph's
    /// edges are a function of these and of which files exist; keeping
    /// them lets a change recompute exactly the importers it can affect, so
    /// a watched index converges to what a fresh build would draw (an
    /// import that fell back to a package, `from . import m` reaching
    /// `__init__.py`, moves to `m.py` when that file appears, and back when
    /// it goes).
    import_specs: FxHashMap<u32, (std::path::PathBuf, Vec<String>)>,
    /// Lowercase path segment of an import -> files with such an import.
    /// May hold ids of files since removed or re-indexed; checked on use.
    spec_keys: FxHashMap<String, FxHashSet<u32>>,
    /// Files whose imports must be resolved again (they imported a file
    /// that was removed).
    dirty_importers: FxHashSet<u32>,
    /// The initial build has finished resolving: from then on every change
    /// recomputes the importers it affects.
    graph_settled: bool,
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
    /// The name each root is shown under, parallel to `root_paths`: its
    /// folder name, or as many trailing folders as it takes to tell roots
    /// with the same name apart (see [`crate::utils::root_labels`]).
    root_labels: Vec<String>,
    /// Whether displayed paths start with the indexed root folder's own name
    /// (default: true). Only honoured with a single root; see
    /// [`Self::make_display_path`].
    pub show_root_name: bool,
    /// When the last full scan of the indexed folders started (Unix
    /// millis): every change made on disk before then is in the index.
    /// `None` until the first build or reconcile completes.
    pub synced_at_ms: Option<u64>,
    /// The startup build or reconcile is running: searches only see the
    /// files indexed so far, and say so (`SearchLimit::Indexing`).
    pub building: bool,
    /// When an incremental update (the file watcher) last changed the
    /// index (Unix millis).
    pub updated_at_ms: Option<u64>,
    /// Whether a file watcher is keeping the index in step with disk.
    pub watching: bool,
    /// The changes that watcher has seen and not applied yet.
    pub watch_backlog: WatchBacklog,
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
            racy_files: roaring::RoaringBitmap::new(),
            file_metadata: Vec::new(),
            pending_imports: Vec::new(),
            waiting_imports: Vec::new(),
            waiting_keys: FxHashMap::default(),
            waiting_local: Vec::new(),
            import_specs: FxHashMap::default(),
            spec_keys: FxHashMap::default(),
            dirty_importers: FxHashSet::default(),
            graph_settled: false,
            recently_added_stems: Vec::new(),
            regex_cache: std::sync::Mutex::new(RegexCache::new(64)),
            generation: 0,
            enable_symbols: true,
            transcode_non_utf8: true,
            max_file_size: PartialIndexedFile::DEFAULT_MAX_FILE_SIZE,
            root_paths: Vec::new(),
            root_labels: Vec::new(),
            show_root_name: true,
            synced_at_ms: None,
            building: false,
            updated_at_ms: None,
            watching: false,
            watch_backlog: WatchBacklog::default(),
        }
    }

    /// An empty engine with this one's settings (roots, symbol, encoding
    /// and size settings), to load a saved index into without holding this
    /// engine's lock; see [`Self::adopt_index`].
    pub fn empty_with_settings(&self) -> Self {
        let mut fresh = Self::new();
        fresh.enable_symbols = self.enable_symbols;
        fresh.transcode_non_utf8 = self.transcode_non_utf8;
        fresh.max_file_size = self.max_file_size;
        fresh.show_root_name = self.show_root_name;
        fresh.root_paths = self.root_paths.clone();
        fresh.root_labels = self.root_labels.clone();
        fresh
    }

    /// Take over the index `loaded` holds (files, postings, symbols, graph,
    /// freshness times), keeping this engine's live state: whether it is
    /// building or watching, and the watcher's backlog counters. Loading a
    /// saved index takes seconds on a large tree; loading it into a side
    /// engine and adopting it under one short write lock keeps searches
    /// answered meanwhile (they see the index being built, and say so).
    pub fn adopt_index(&mut self, mut loaded: SearchEngine) {
        loaded.building = self.building;
        loaded.watching = self.watching;
        loaded.watch_backlog = std::mem::take(&mut self.watch_backlog);
        *self = loaded;
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
            self.root_labels = crate::utils::root_labels(&self.root_paths);
        }
    }

    /// The indexed roots with the names their files are shown under.
    pub fn roots_with_labels(&self) -> impl Iterator<Item = (&Path, &str)> {
        self.root_paths
            .iter()
            .map(PathBuf::as_path)
            .zip(self.root_labels.iter().map(String::as_str))
    }

    /// `path` relative to the indexed root holding it (no root name), with
    /// forward slashes; the whole path when it is under no root. Ranking
    /// reads this, so where a repository sits on disk (`~/src/…`,
    /// `/mnt/tests/…`) cannot change its scores.
    pub fn root_relative_path(&self, path: &Path) -> String {
        for root in &self.root_paths {
            if let Ok(rel) = path.strip_prefix(root) {
                return rel.to_string_lossy().replace('\\', "/");
            }
        }
        path.to_string_lossy().replace('\\', "/")
    }

    /// Whether displayed paths leave off the root folder's name: only when
    /// `show_root_name` is off and exactly one root is indexed, since with
    /// several roots two `src/main.rs` would be indistinguishable.
    pub fn hides_root_name(&self) -> bool {
        !self.show_root_name && self.root_paths.len() == 1
    }

    /// The root-qualified path of `path` and the byte offset where its
    /// displayed form starts (see [`Self::make_display_path`]).
    fn qualified_display_path(&self, path: &Path) -> (String, usize) {
        for (root, label) in self.roots_with_labels() {
            if let Ok(relative) = path.strip_prefix(root) {
                let relative_str = relative.to_string_lossy().replace('\\', "/");
                // Include the root's name so the path is workspace-relative
                // (e.g. `project/src/main.rs`).
                if !label.is_empty() {
                    if relative_str.is_empty() {
                        // Path IS the root directory (or a single-file root)
                        return (label.to_string(), 0);
                    }
                    let start = if self.hides_root_name() {
                        label.len() + 1
                    } else {
                        0
                    };
                    return (format!("{label}/{relative_str}"), start);
                }
                // Root has no file_name (e.g. filesystem root "/") — fall back
                // to the bare relative path.
                return (relative_str, 0);
            }
        }
        // Fallback: return the full path with forward slashes
        (path.to_string_lossy().replace('\\', "/"), 0)
    }

    /// Root-qualified path of a (canonical) stored file path, always starting
    /// with the root folder's own name (`project/src/main.rs`) whatever
    /// `show_root_name` says. This is what path filters match against, so
    /// `file:` / `-file:` behave the same whether or not the name is shown.
    pub fn root_qualified_path(&self, path: &Path) -> String {
        self.qualified_display_path(path).0
    }

    /// Convert a (canonical) stored file path into a workspace-relative display
    /// string using forward slashes.
    ///
    /// By default the display path includes the root folder's own name as the
    /// first component — mirroring the VSCode workspace experience where
    /// opening `/workspace/project` shows files as `project/src/main.rs`. This
    /// makes results unambiguous when multiple root paths share the same
    /// internal layout (e.g. two repos both containing a `src/main.rs`). With
    /// `show_root_name` off and a single root, the name is left off
    /// (`src/main.rs`).
    ///
    /// If `path` is under one of the registered root paths the absolute prefix
    /// *above* the root folder is stripped. E.g. root `/workspace/project`,
    /// file `/workspace/project/src/main.rs` → `project/src/main.rs`. When no
    /// root matches, the full path is returned with backslashes normalised to
    /// forward slashes so that the output is always OS-agnostic.
    pub fn make_display_path(&self, path: &Path) -> String {
        let (mut qualified, start) = self.qualified_display_path(path);
        qualified.drain(..start);
        qualified
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
    ///
    /// Files are read outside the write lock, so by the time a batch is
    /// merged a file may have been deleted or rewritten, and the watcher may
    /// already have applied that change. Content that is no longer on disk
    /// is never installed: deleted files are dropped and rewritten ones are
    /// read again here.
    pub fn index_batch(&mut self, batch: Vec<PreIndexedFile>) -> usize {
        let (batch, changed): (Vec<_>, Vec<_>) =
            batch.into_iter().partition(PreIndexedFile::still_on_disk);
        let reread: Vec<PathBuf> = changed
            .into_iter()
            .map(|pre| pre.path)
            .filter(|p| p.is_file())
            .collect();
        let mut count = self.merge_batch(batch);
        if !reread.is_empty() {
            // Read under the lock, so nothing the watcher does can slip in
            // between; a file that changes yet again is merged as read.
            let transcode = self.transcode_non_utf8;
            let max_size = self.max_file_size;
            let symbols = self.enable_symbols;
            let fresh: Vec<PreIndexedFile> = reread
                .par_iter()
                .filter_map(|p| PartialIndexedFile::process(p, transcode, max_size))
                .map(|(partial, _)| PreIndexedFile::from_partial(partial, symbols))
                .collect();
            count += self.merge_batch(fresh);
        }
        count
    }

    fn merge_batch(&mut self, batch: Vec<PreIndexedFile>) -> usize {
        let mut count = 0;
        self.generation += 1;

        for pre_indexed in batch {
            // Add file to store - this also memory-maps it
            let ids_before = self.file_store.len();
            let file_id = match self.file_store.add_file(&pre_indexed.path) {
                Ok(id) => id,
                Err(_) => continue,
            };
            // A known path (the watcher beat discovery during the initial
            // build, or the same batch was fed twice) is re-indexed under
            // its existing id: strip the stale postings, symbols and
            // out-edges first — unioning the new trigrams onto the old
            // posting lists kept the file a false candidate for text it no
            // longer contains — while keeping the edges into it.
            let dependents = if (file_id as usize) < ids_before {
                let dependents = self.strip_for_reindex(file_id);
                self.file_store.refresh_file_by_id(file_id);
                dependents
            } else {
                Vec::new()
            };
            self.set_indexed_meta(
                file_id,
                pre_indexed.mtime,
                pre_indexed.size,
                pre_indexed.racy,
            );
            self.note_added_file(&pre_indexed.path);

            // Register file in dependency index under the store's canonical
            // path (no realpath walk per file).
            if let Some(canonical) = self.file_store.get_path(file_id).map(Path::to_path_buf) {
                self.dependency_index
                    .register_canonical_file(file_id, canonical);
            }
            if !dependents.is_empty() {
                self.dependency_index
                    .add_imports_batch(dependents.iter().map(|&from| (from, file_id)).collect());
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
            self.record_import_specs(file_id, &pre_indexed.path, &pre_indexed.imports);
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
        let mut settling = self.begin_settling_imports();
        while !self.settle_imports_step(&mut settling, usize::MAX) {}
    }

    /// Start [`Self::resolve_imports`] so it can run in steps, each under
    /// its own write lock: on a large tree resolving every importer again
    /// takes seconds, and one lock hold for all of it turned searches away
    /// at the end of every build. Searches between the steps see a graph
    /// part way through being settled, while the index still reports
    /// itself as building.
    ///
    /// The graph counts as settled from here on, so a file the watcher
    /// adds or removes between steps re-resolves the importers that
    /// mention it, as it would after the build; the importers still to do
    /// are resolved against the files indexed when their step runs.
    pub fn begin_settling_imports(&mut self) -> ImportSettling {
        // Queued and parked imports of files whose import list is not
        // known get one final attempt now; every file with a known list is
        // resolved from scratch in the steps below, so its queued imports
        // need no attempt here (on a loaded index that is every file, and
        // resolving them all at once here was one long lock hold). Still
        // unresolved imports stay parked so they are persisted and retried
        // after a later incremental add.
        let specs = &self.import_specs;
        self.pending_imports
            .retain(|(id, _, _)| !specs.contains_key(id));
        self.resolve_pending_now();
        let orphans: Vec<usize> = (0..self.waiting_imports.len())
            .filter(|&i| {
                self.waiting_imports[i]
                    .as_ref()
                    .is_some_and(|(id, _, _)| !self.import_specs.contains_key(id))
            })
            .collect();
        self.retry_waiting(orphans);
        self.recently_added_stems.clear();
        // Imports resolved while the build was still adding files may have
        // fallen back to a package whose module was indexed later; resolve
        // every importer once more against the complete file set, so the
        // graph does not depend on the order files were indexed in.
        let mut importers: Vec<u32> = self.import_specs.keys().copied().collect();
        importers.sort_unstable();
        self.dirty_importers.clear();
        self.graph_settled = true;
        ImportSettling { importers }
    }

    /// Resolve up to `max` more importers of `settling`; true when all are
    /// done (the waiting list is then compacted).
    pub fn settle_imports_step(&mut self, settling: &mut ImportSettling, max: usize) -> bool {
        let take = settling.importers.len().min(max);
        let step: FxHashSet<u32> = settling.importers.drain(..take).collect();
        self.recompute_importers(step);
        if settling.importers.is_empty() {
            self.compact_waiting_imports();
            return true;
        }
        false
    }

    /// Remember `imports` as `id`'s complete import list.
    fn record_import_specs(&mut self, id: u32, path: &Path, imports: &[String]) {
        if imports.is_empty() {
            self.import_specs.remove(&id);
            return;
        }
        for import in imports {
            for key in Self::import_keys(import) {
                self.spec_keys.entry(key).or_default().insert(id);
            }
        }
        self.import_specs
            .insert(id, (path.to_path_buf(), imports.to_vec()));
    }

    /// Resolve the imports of `ids` from scratch: drop their out-edges and
    /// parked imports, then resolve every import they have against the
    /// files indexed now. Files whose import list is not known (loaded from
    /// an index saved before lists were kept) are parsed again. Returns the
    /// number of edges added.
    fn recompute_importers(&mut self, ids: FxHashSet<u32>) -> usize {
        let ids: Vec<u32> = ids
            .into_iter()
            .filter(|&id| self.file_store.get(id).is_some())
            .collect();
        debug_assert!(
            ids.iter()
                .all(|&id| self.dependency_index.path_of(id).is_some()),
            "an importer is being resolved while it is out of the dependency index"
        );
        if ids.is_empty() {
            return 0;
        }
        for &id in &ids {
            if !self.import_specs.contains_key(&id) {
                if let Some((path, specs)) = self.extract_import_specs(id) {
                    self.record_import_specs(id, &path, &specs);
                }
            }
        }
        // Their parked imports are parked again below if still unresolved;
        // those of removed files go for good.
        let set: FxHashSet<u32> = ids.iter().copied().collect();
        let store = &self.file_store;
        for slot in self.waiting_imports.iter_mut() {
            if slot
                .as_ref()
                .is_some_and(|(id, _, _)| set.contains(id) || store.get(*id).is_none())
            {
                *slot = None;
            }
        }
        let mut touched = Vec::new();
        for &id in &ids {
            touched.extend(self.dependency_index.remove_out_edges(id));
            if let Some((path, specs)) = self.import_specs.get(&id) {
                self.pending_imports.push((id, path.clone(), specs.clone()));
            }
        }
        let added = self.resolve_pending_now();
        // In-edge counts feed the ranking metadata of old and new targets.
        for &id in &ids {
            touched.extend(self.dependency_index.get_dependencies(id));
        }
        touched.sort_unstable();
        touched.dedup();
        if !self.file_metadata.is_empty() {
            for id in touched {
                self.refresh_file_metadata(id);
            }
        }
        added
    }

    /// Parse `id`'s imports from its indexed content.
    fn extract_import_specs(&self, id: u32) -> Option<(std::path::PathBuf, Vec<String>)> {
        if !self.enable_symbols {
            return None;
        }
        let path = self.file_store.get_path(id)?.to_path_buf();
        let file = self.file_store.get(id)?;
        let content = file.as_str().ok()?;
        let specs = crate::symbols::SymbolExtractor::new_for_source(&path, Some(&content))
            .extract_imports(&content)
            .ok()?
            .into_iter()
            .map(|i| i.path)
            .collect();
        Some((path, specs))
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
        if self.graph_settled {
            // Every importer whose imports mention a name just indexed, or
            // that imported a file just removed, is resolved again: a new
            // file can take an import over from a package it fell back to.
            let mut affected = std::mem::take(&mut self.dirty_importers);
            for stem in &self.recently_added_stems {
                if let Some(ids) = self.spec_keys.get(stem) {
                    affected.extend(ids.iter().copied());
                }
            }
            self.recently_added_stems.clear();
            edges += self.recompute_importers(affected);
            return edges;
        }
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
                    | "index.mts"
                    | "index.cts"
                    | "index.d.ts"
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
        if crate::dependencies::is_local_spec(&import) {
            self.waiting_local.push(idx);
        }
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
            if let Some((from, to)) = edge {
                // The importing file may have been removed since it parked
                // the import; never add an edge from a dead id.
                if self.file_store.get(from).is_some() {
                    edges.push((from, to));
                }
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
        self.waiting_local.clear();
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

    /// Size on disk of file `id` as indexed (0 when unknown).
    pub fn indexed_size(&self, id: u32) -> u64 {
        match self.indexed_meta.get(id as usize) {
            Some(&(_, size)) => size,
            None => 0,
        }
    }

    pub fn get_stats(&self) -> SearchStats {
        let total_size = (0..self.file_store.len() as u32)
            .filter(|&id| self.file_store.get(id).is_some())
            .map(|id| self.indexed_size(id))
            .sum();
        SearchStats {
            num_files: self.file_store.live_len(),
            total_size,
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

    /// Root-relative display path of an indexed file: the precomputed one
    /// (no allocation, which graph listings rely on when they scan every
    /// file), else built from its path.
    pub fn display_path(&self, file_id: u32) -> Option<std::borrow::Cow<'_, str>> {
        let file = self.file_store.get(file_id)?;
        Some(match self.file_metadata.get(file_id as usize) {
            Some(m) if !m.display_path.is_empty() => {
                std::borrow::Cow::Borrowed(&m.display_path[m.display_start as usize..])
            }
            _ => std::borrow::Cow::Owned(self.make_display_path(&file.path)),
        })
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
    /// `<root ending in project>/src/main.rs`, looked up exactly. With the
    /// root name hidden, `src/main.rs` is first tried under the single root.
    fn find_by_display_path(&self, display: &str) -> Option<u32> {
        let display = display.replace('\\', "/");
        if self.hides_root_name() {
            if let Some(id) = self.find_file_id_exact(&self.root_paths[0].join(&display)) {
                return Some(id);
            }
        }
        for (root, root_name) in self.roots_with_labels() {
            if root_name.is_empty() {
                continue;
            }
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
                root.to_path_buf()
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
    /// Re-index `paths` (known files under their existing ids, unknown ones
    /// as new files) as one batch: every stale posting list is stripped in a
    /// single pass over the index instead of one pass per file, and the
    /// files are read and parsed in parallel before the merge. This is what
    /// a watcher burst (branch switch, formatter run) goes through; the
    /// per-file [`Self::update_file`] path cost a full posting-list scan per
    /// modified file. Returns `(indexed, removed)`.
    pub fn update_files(&mut self, paths: &[PathBuf]) -> (usize, usize) {
        let parser = self.file_parser();
        let mut unique: Vec<&PathBuf> = Vec::with_capacity(paths.len());
        let mut seen = std::collections::HashSet::new();
        for path in paths {
            if seen.insert(path) {
                unique.push(path);
            }
        }
        let parsed = unique
            .par_iter()
            .map(|p| ((*p).clone(), parser.parse(p)))
            .collect();
        self.install_parsed(parsed)
    }

    /// The settings files are read and parsed with, so a caller can parse
    /// without holding the engine lock and hand the results to
    /// [`Self::install_parsed`].
    pub fn file_parser(&self) -> FileParser {
        FileParser {
            transcode_non_utf8: self.transcode_non_utf8,
            max_file_size: self.max_file_size,
            enable_symbols: self.enable_symbols,
        }
    }

    /// Install files that were read and parsed ahead of time (see
    /// [`Self::file_parser`]): a path already indexed is re-indexed under
    /// its id, or dropped when it is now unreadable (`None`); a new path is
    /// added. Ids are looked up here, under the caller's write lock, so the
    /// index may have changed since the files were parsed. Every stale
    /// posting list is stripped in one pass. Returns `(indexed, removed)`.
    pub fn install_parsed(
        &mut self,
        parsed: Vec<(PathBuf, Option<PreIndexedFile>)>,
    ) -> (usize, usize) {
        if parsed.is_empty() {
            return (0, 0);
        }
        let mut known: Vec<(u32, PathBuf, Option<PreIndexedFile>)> = Vec::new();
        let mut fresh: Vec<PreIndexedFile> = Vec::new();
        let mut fresh_paths = std::collections::HashSet::new();
        let mut doomed = roaring::RoaringBitmap::new();
        for (path, pre) in parsed {
            match self.find_file_id_exact(&path) {
                Some(id) => {
                    if doomed.insert(id) {
                        known.push((id, path, pre));
                    }
                }
                None => {
                    if let Some(pre) = pre {
                        if fresh_paths.insert(path) {
                            fresh.push(pre);
                        }
                    }
                }
            }
        }

        // One pass over the posting lists for every known id, then the
        // per-id caches and edges (keeping the edges *into* each file).
        self.trigram_index.remove_documents(&doomed);
        // A dependent that is itself in the batch re-resolves its own
        // imports when it is installed; restoring its edge as well would
        // record it twice.
        let dependents_of: Vec<Vec<u32>> = known
            .iter()
            .map(|(id, _, _)| {
                let mut deps = self.strip_caches_and_edges(*id);
                deps.retain(|d| !doomed.contains(*d));
                deps
            })
            .collect();

        let mut indexed = 0;
        let mut removed = 0;
        for ((id, path, pre), dependents) in known.into_iter().zip(dependents_of) {
            match pre {
                Some(pre) => {
                    self.file_store.refresh_file_by_id(id);
                    self.install_reindexed(id, &path, pre, &dependents);
                    indexed += 1;
                }
                None => {
                    // Now binary, oversized or unreadable: drop it, parking
                    // its dependents' edges in case a readable file returns.
                    if let Some(p) = self.file_store.get_path(id).map(Path::to_path_buf) {
                        self.park_dependents(p, &dependents);
                    }
                    self.forget_id(id);
                    removed += 1;
                }
            }
        }

        if !fresh.is_empty() {
            indexed += self.index_batch(fresh);
        }
        if indexed + removed > 0 {
            self.resolve_imports_incremental();
        }
        (indexed, removed)
    }

    pub fn update_file(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        let Some(id) = self.find_file_id_exact(path) else {
            // Not yet indexed — treat as a fresh add, then resolve what can
            // be resolved now: the new file's own imports, and any parked
            // import waiting for a file with this name (including the edges
            // of a file that was deleted and has just come back).
            self.index_file(path)?;
            self.resolve_imports_incremental();
            return Ok(());
        };

        // Strip all stale data for this id first. The edges *into* the file
        // are snapshotted: the files importing it did not change, so those
        // edges are restored once the file is registered again (dropping
        // them zeroed the dependency boost of every popular module on each
        // edit).
        let dependents = self.strip_for_reindex(id);

        // Read fresh owned content + safety check (no live mmap — avoids SIGBUS if
        // the file is being rewritten concurrently).
        let (partial, _transcoded) =
            match PartialIndexedFile::process(path, self.transcode_non_utf8, self.max_file_size) {
                Some(p) => p,
                None => {
                    // File is now binary/unsafe/oversized/unreadable — drop it
                    // from the index entirely rather than keeping stale
                    // content; its dependents' edges are parked so they
                    // return if a readable file appears here again.
                    if let Some(p) = self.file_store.get_path(id).map(Path::to_path_buf) {
                        self.park_dependents(p, &dependents);
                    }
                    self.forget_id(id);
                    return Ok(());
                }
            };
        let pre = PreIndexedFile::from_partial(partial, self.enable_symbols);

        // Refresh the store entry so the stale mapping + caches are discarded,
        // then re-register and re-index under the same id.
        self.file_store.refresh_file_by_id(id);
        self.install_reindexed(id, path, pre, &dependents);
        self.resolve_imports_incremental();
        Ok(())
    }

    /// Drop the trigrams, symbols, references and dependency edges of `id`
    /// ahead of re-indexing it under the same id. Returns the files that
    /// import `id`, whose edges the caller restores with
    /// [`Self::install_reindexed`].
    fn strip_for_reindex(&mut self, id: u32) -> Vec<u32> {
        self.trigram_index.remove_document(id);
        self.strip_caches_and_edges(id)
    }

    /// [`Self::strip_for_reindex`] without the posting-list pass, for callers
    /// that removed several ids from the trigram index in one pass.
    fn strip_caches_and_edges(&mut self, id: u32) -> Vec<u32> {
        if let Some(slot) = self.symbol_cache.get_mut(id as usize) {
            slot.clear();
        }
        if let Some(slot) = self.reference_cache.get_mut(id as usize) {
            slot.clear();
        }
        let dependents = self.dependency_index.get_dependents(id);
        self.dependency_index.remove_file(id);
        dependents
    }

    /// Register `pre` under the existing id `id`: metadata, dependency path,
    /// trigrams, symbols, references and imports, plus the restored edges
    /// from `dependents`.
    fn install_reindexed(
        &mut self,
        id: u32,
        path: &std::path::Path,
        pre: PreIndexedFile,
        dependents: &[u32],
    ) {
        self.set_indexed_meta(id, pre.mtime, pre.size, pre.racy);
        self.note_added_file(path);
        if let Some(canonical) = self.file_store.get_path(id).map(Path::to_path_buf) {
            self.dependency_index.register_canonical_file(id, canonical);
        }
        if !dependents.is_empty() {
            self.dependency_index
                .add_imports_batch(dependents.iter().map(|&from| (from, id)).collect());
        }

        self.trigram_index.add_document_trigrams(id, pre.trigrams);
        while self.symbol_cache.len() <= id as usize {
            self.symbol_cache.push(Vec::new());
        }
        self.symbol_cache[id as usize] = pre.symbols;
        self.store_references(id, pre.references);

        // Resolved by the caller once every file of the batch is back in
        // the index: resolving here, mid-batch, would resolve importers
        // that are stripped and not yet installed.
        self.record_import_specs(id, path, &pre.imports);
        if !pre.imports.is_empty() {
            self.pending_imports
                .push((id, path.to_path_buf(), pre.imports));
        }

        // Keep fast-mode ranking signals current for the touched file.
        self.refresh_file_metadata(id);
        self.generation += 1;
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
        let doomed = self.vanished_in(dirs);
        self.remove_files_by_ids(&doomed)
    }

    /// The indexed files directly in one of `dirs` that no longer exist on
    /// disk (see [`Self::prune_vanished_in`]). Reads only, so the watcher
    /// can stat under a read lock.
    pub fn vanished_in(&self, dirs: &[PathBuf]) -> Vec<u32> {
        dirs.par_iter()
            .flat_map_iter(|dir| {
                let prefix = canonicalize_lossy(dir);
                let ids = self.file_store.ids_in_dir(&prefix);
                // One directory listing instead of a stat per indexed file.
                let listed: Option<std::collections::HashSet<std::ffi::OsString>> =
                    std::fs::read_dir(&prefix).ok().map(|entries| {
                        entries
                            .filter_map(|e| e.ok().map(|e| e.file_name()))
                            .collect()
                    });
                ids.into_iter().filter(move |&id| {
                    let Some(path) = self.file_store.get_path(id) else {
                        return false;
                    };
                    match (&listed, path.file_name()) {
                        (Some(names), Some(name)) => !names.contains(name),
                        // Unlistable directory: ask about each file.
                        _ => !path.exists(),
                    }
                })
            })
            .collect()
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
            .map(|r| PackedRef::new(self.intern_ref_name(&r.name), r.line, r.column, r.kind))
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
    ///
    /// The edges *into* the file are parked as imports of its former path
    /// (see [`Self::park_dependents`]), so a file that is deleted and later
    /// recreated regains its dependents when it is indexed again.
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
        let dependents = self.dependency_index.get_dependents(id);
        let path = self.file_store.get_path(id).map(Path::to_path_buf);
        self.dependency_index.remove_file(id);
        self.file_store.remove_file_by_id(id);
        for other in dependents_changed {
            self.refresh_file_metadata(other);
        }
        self.import_specs.remove(&id);
        self.dirty_importers.extend(dependents.iter().copied());
        if let Some(path) = path {
            self.park_dependents(path, &dependents);
        }
    }

    /// Re-queue the edges from `dependents` into the file that lived at
    /// `target_path` as parked imports naming that absolute path. The
    /// dependency index resolves an absolute import only to that exact
    /// indexed path, so the edges come back as soon as a file is indexed
    /// there again (the retry is keyed on the path's stem like any other
    /// parked import).
    fn park_dependents(&mut self, target_path: PathBuf, dependents: &[u32]) {
        if dependents.is_empty() {
            return;
        }
        let import = target_path.to_string_lossy().into_owned();
        for &from in dependents {
            let Some(from_path) = self.file_store.get_path(from).map(Path::to_path_buf) else {
                continue;
            };
            self.park_import(from, from_path, import.clone());
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
                let display = self.qualified_display_path(&file.path);
                let relative = self.root_relative_path(&file.path);
                FileMetadata::compute(&file.path, &relative, display, symbol_count, dep_count)
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
                let display = self.qualified_display_path(&file.path);
                let relative = self.root_relative_path(&file.path);
                FileMetadata::compute(&file.path, &relative, display, symbol_count, dep_count)
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
