//! One search request, run the same way by every front end.
//!
//! The web API, the gRPC service and `fcs` (offline) each used to carry
//! their own copy of the request policy (default page size, caps, deadline,
//! offset check) and of the dispatch to the engine's search methods, and the
//! copies drifted (offline `fcs` never checked the offset limit, and gRPC
//! alone skipped the regex case helper). They now build a
//! [`SearchRequest`] and call [`SearchEngine::run`]; only the transport
//! (JSON, protobuf, terminal) differs.

use crate::search::{
    parse_query, regex_pattern_with_case, GraphDirection, GraphFilter, GraphScope, PathQuery,
    PathSearch, RankMode, SearchEngine, SearchLimits, SearchMatch, SearchRankingInfo,
    SearchSettings,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Page size when the request does not give one.
pub const DEFAULT_PAGE_SIZE: usize = 50;
/// Largest page one request may ask for.
pub const MAX_PAGE_SIZE: usize = 1000;
/// Longest deadline a request may ask for.
pub const MAX_TIMEOUT: Duration = Duration::from_secs(30);
/// Most path matches one request may ask for.
pub const MAX_PATH_MATCHES: usize = 100;

/// What kind of search to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchMode {
    /// Literal text (the query syntax applies: `file:`, `lang:`, `case:`…).
    #[default]
    Text,
    /// A regular expression.
    Regex,
    /// Symbol definitions whose name matches.
    Symbols,
    /// References (call sites, type mentions) of an identifier.
    References,
}

impl SearchMode {
    /// The mode for a front end's boolean flags; `references` cannot be
    /// combined with the other two.
    pub fn from_flags(regex: bool, symbols: bool, references: bool) -> Result<Self, String> {
        match (regex, symbols, references) {
            (_, _, true) if regex || symbols => {
                Err("references cannot be combined with regex or symbols".to_string())
            }
            (_, _, true) => Ok(Self::References),
            (_, true, _) => Ok(Self::Symbols),
            (true, _, _) => Ok(Self::Regex),
            _ => Ok(Self::Text),
        }
    }
}

/// A search as a front end received it. Unset numbers mean "the default".
#[derive(Debug, Clone, Default)]
pub struct SearchRequest {
    pub query: String,
    pub mode: SearchMode,
    /// Case-sensitive matching; overrides `case:` in the query. For a regex,
    /// `Some(false)` adds `(?i)`.
    pub case_sensitive: Option<bool>,
    /// Whole-word matching; overrides `word:` in the query.
    pub whole_word: Option<bool>,
    /// Semicolon-separated path globs to include / exclude.
    pub include: String,
    pub exclude: String,
    /// Page size (0 = [`DEFAULT_PAGE_SIZE`]; capped at [`MAX_PAGE_SIZE`]).
    pub max_results: usize,
    pub offset: usize,
    pub rank: RankMode,
    /// The client's deadline (`None` = none; capped at [`MAX_TIMEOUT`]).
    pub timeout: Option<Duration>,
    /// Candidate files fast ranking may open (`None` = the server default).
    pub max_files: Option<usize>,
    /// Also list up to this many files whose path matches (0 = none; capped
    /// at [`MAX_PATH_MATCHES`]). Only on the first page of a text or regex
    /// search.
    pub path_matches: usize,
    /// Graph filters as `PATH[:N|:*]`, a leading `-` to leave the files out:
    /// the same as `deps:` / `rdeps:` in the query, and the only way to
    /// scope a regex search (whose query is all pattern).
    pub deps: Vec<String>,
    pub rdeps: Vec<String>,
}

/// What a search found.
pub struct SearchOutcome {
    /// The requested page of matches.
    pub matches: Vec<SearchMatch>,
    pub info: SearchRankingInfo,
    /// Path matches, when asked for and applicable.
    pub paths: Option<PathSearch>,
    /// The files the search was limited to, when it had graph filters.
    pub scope: Option<Arc<GraphScope>>,
}

impl SearchOutcome {
    /// Whether a later page may hold more matches.
    pub fn has_more(&self, offset: usize) -> bool {
        match self.info.total_matches {
            Some(total) => offset + self.matches.len() < total,
            None => true, // truncated by budget/deadline: more may exist
        }
    }
}

impl SearchRequest {
    /// The page size this request gets.
    pub fn page_size(&self) -> usize {
        if self.max_results == 0 {
            DEFAULT_PAGE_SIZE
        } else {
            self.max_results.min(MAX_PAGE_SIZE)
        }
    }

    /// The limits this request runs under. `server_timeout` is the
    /// transport's own request timeout, if any: the engine stops just before
    /// it so an abandoned scan does not keep running.
    pub fn limits(
        &self,
        settings: &SearchSettings,
        server_timeout: Option<Duration>,
    ) -> Result<SearchLimits, String> {
        settings.check_offset(self.offset)?;
        let ceiling = server_timeout.map(|t| t.mul_f32(0.9));
        let timeout = match (self.timeout.map(|t| t.min(MAX_TIMEOUT)), ceiling) {
            (Some(t), Some(c)) => Some(t.min(c)),
            (t, c) => t.or(c),
        };
        let mut limits = SearchLimits::new(self.page_size()).with_offset(self.offset);
        if let Some(t) = timeout {
            limits = limits.with_timeout(t);
        }
        Ok(settings.apply(limits, self.max_files))
    }
}

impl SearchEngine {
    /// Run `req` with `limits` (from [`SearchRequest::limits`]). Errors are
    /// the client's: a bad regex or path filter.
    pub fn run(&self, req: &SearchRequest, limits: SearchLimits) -> Result<SearchOutcome, String> {
        let mut parsed = parse_query(&req.query);
        if let Some(c) = req.case_sensitive {
            parsed.options.case_sensitive = c;
        }
        if let Some(w) = req.whole_word {
            parsed.options.whole_word = w;
        }
        let pattern = regex_pattern_with_case(&req.query, req.case_sensitive);
        // A regex is all pattern: `deps:` inside it is text to match.
        let mut graph = if req.mode == SearchMode::Regex {
            Vec::new()
        } else {
            std::mem::take(&mut parsed.graph)
        };
        graph.extend(request_graph_filters(&req.deps, GraphDirection::Deps));
        graph.extend(request_graph_filters(&req.rdeps, GraphDirection::Rdeps));
        let scope = self.graph_scope(&graph)?.map(Arc::new);
        let limits = limits.with_scope(scope.clone());
        let filter_error = |e: anyhow::Error| format!("Invalid filter pattern: {e}");
        // The path pass gets the same time allowance as the content scan.
        let allowance = limits
            .deadline
            .map(|d| d.saturating_duration_since(Instant::now()));
        let (matches, info) = match req.mode {
            SearchMode::References => self
                .search_references_parsed(&parsed, &req.include, &req.exclude, limits)
                .map_err(filter_error)?,
            SearchMode::Symbols => self
                .search_symbols_parsed(&parsed, &req.include, &req.exclude, limits)
                .map_err(filter_error)?,
            // The engine's error already reads "Invalid regex pattern: …".
            SearchMode::Regex => self
                .search_regex_with_limits(&pattern, &req.include, &req.exclude, limits, req.rank)
                .map_err(|e| e.to_string())?,
            SearchMode::Text => self
                .search_parsed(&parsed, &req.include, &req.exclude, limits, req.rank)
                .map_err(filter_error)?,
        };

        let wants_paths = req.path_matches > 0
            && req.offset == 0
            && matches!(req.mode, SearchMode::Text | SearchMode::Regex);
        let paths = if wants_paths {
            let query = match req.mode {
                SearchMode::Regex => PathQuery::Regex(&pattern),
                _ => PathQuery::Terms(&parsed),
            };
            Some(
                self.search_paths(
                    query,
                    &req.include,
                    &req.exclude,
                    req.path_matches.min(MAX_PATH_MATCHES),
                    allowance.map(|a| Instant::now() + a),
                    scope.as_deref(),
                )
                .map_err(|e| e.to_string())?,
            )
        } else {
            None
        };
        Ok(SearchOutcome {
            matches,
            info,
            paths,
            scope,
        })
    }
}

/// A request's `deps` / `rdeps` list as graph filters.
fn request_graph_filters(
    args: &[String],
    direction: GraphDirection,
) -> impl Iterator<Item = GraphFilter> + '_ {
    args.iter()
        .map(|a| a.trim())
        .filter(|a| !a.is_empty() && *a != "-")
        .map(move |a| match a.strip_prefix('-') {
            Some(rest) => GraphFilter {
                direction,
                negate: true,
                arg: rest.to_string(),
            },
            None => GraphFilter {
                direction,
                negate: false,
                arg: a.to_string(),
            },
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_from_flags() {
        assert_eq!(
            SearchMode::from_flags(false, false, false),
            Ok(SearchMode::Text)
        );
        assert_eq!(
            SearchMode::from_flags(true, false, false),
            Ok(SearchMode::Regex)
        );
        assert_eq!(
            SearchMode::from_flags(false, true, false),
            Ok(SearchMode::Symbols)
        );
        assert_eq!(
            SearchMode::from_flags(false, false, true),
            Ok(SearchMode::References)
        );
        assert!(SearchMode::from_flags(true, false, true).is_err());
        assert!(SearchMode::from_flags(false, true, true).is_err());
    }

    #[test]
    fn page_size_defaults_and_caps() {
        let mut req = SearchRequest::default();
        assert_eq!(req.page_size(), DEFAULT_PAGE_SIZE);
        req.max_results = 5000;
        assert_eq!(req.page_size(), MAX_PAGE_SIZE);
        req.max_results = 7;
        assert_eq!(req.page_size(), 7);
    }

    #[test]
    fn limits_check_the_offset() {
        let settings = SearchSettings {
            max_offset: 10,
            ..SearchSettings::default()
        };
        let req = SearchRequest {
            offset: 11,
            ..SearchRequest::default()
        };
        assert!(req.limits(&settings, None).is_err());
    }
}
