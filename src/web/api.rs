//! REST API handlers for Fast Code Search

use super::WebState;
use crate::diagnostics::{
    self, ConfigSummary, DiagnosticsQuery, ExtensionBreakdown, HealthStatus,
    KeywordDiagnosticsResponse, KeywordIndexDiagnostics, TestResult, TestSummary,
};
use crate::search::{
    engine_lock, IndexFreshness, IndexingStatus, PathFreshness, RankMode, SearchEngine,
    SearchLimits,
};
use axum::{
    extract::{
        ws::{Message, WebSocket},
        Query, State, WebSocketUpgrade,
    },
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use futures_util::{SinkExt, StreamExt};
use rand::seq::IndexedRandom;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

/// JSON error body returned by all API handlers for non-2xx responses, so error
/// and success responses have a consistent (JSON) content type.
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Unified API error. Renders as `{ "error": ... }` with the appropriate status,
/// and attaches `Retry-After: 1` to 503s so clients back off briefly while the
/// index is being updated instead of hammering the server.
///
/// Handlers keep producing `(StatusCode, String)` tuples; the `?` operator
/// converts them here via the `From` impl, so call sites need no changes.
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl From<(StatusCode, String)> for ApiError {
    fn from((status, message): (StatusCode, String)) -> Self {
        Self { status, message }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let mut resp = (
            self.status,
            Json(ErrorResponse {
                error: self.message,
            }),
        )
            .into_response();
        if self.status == StatusCode::SERVICE_UNAVAILABLE {
            resp.headers_mut().insert(
                axum::http::header::RETRY_AFTER,
                axum::http::HeaderValue::from_static("1"),
            );
        }
        resp
    }
}

/// `axum::extract::Query` whose rejection is the same JSON `{ "error": … }`
/// envelope every handler error uses (a plain-text 400 from the built-in
/// extractor was the one inconsistent response on the API).
pub struct ApiQuery<T>(pub T);

impl<S, T> axum::extract::FromRequestParts<S> for ApiQuery<T>
where
    T: serde::de::DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        match Query::<T>::from_request_parts(parts, state).await {
            Ok(Query(v)) => Ok(ApiQuery(v)),
            Err(rej) => Err(ApiError::from((
                StatusCode::BAD_REQUEST,
                format!("Invalid query parameters: {}", rej.body_text()),
            ))),
        }
    }
}

/// Search query parameters
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// The search query string
    q: String,
    /// Maximum number of results (0 or absent = the server's
    /// `search.default_page_size`, 50; capped at `search.max_page_size`)
    #[serde(default)]
    max: usize,
    /// Semicolon-delimited glob patterns for paths to include
    #[serde(default)]
    include: String,
    /// Semicolon-delimited glob patterns for paths to exclude
    #[serde(default)]
    exclude: String,
    /// Whether to treat the query as a regex pattern
    #[serde(default)]
    regex: bool,
    /// Whether to search only in symbols (function/class names)
    #[serde(default)]
    symbols: bool,
    /// Find references (call sites, type mentions) of the identifier in
    /// `q` instead of text matches. Results have `match_type`
    /// `SYMBOL_REFERENCE`.
    #[serde(default)]
    references: bool,
    /// Ranking mode: "auto" (default), "fast", or "full"
    #[serde(default)]
    rank: String,
    /// Number of context lines to return before and after each match (default: 0)
    #[serde(default)]
    context: usize,
    /// Results to skip before the returned page (default: 0). Ordering is
    /// deterministic, so `offset=max` returns the next page.
    #[serde(default)]
    offset: usize,
    /// Stop scanning after this many milliseconds and return the best
    /// matches seen so far (0 = no deadline; capped server-side).
    #[serde(default)]
    timeout_ms: u64,
    /// Case-sensitive matching (overrides `case:` in the query).
    #[serde(default)]
    case: Option<bool>,
    /// Whole-word matching (overrides `word:` in the query).
    #[serde(default)]
    word: Option<bool>,
    /// Also return up to this many files whose *path* matches the query, in
    /// `files` (0 = none, the default; at most `search.max_path_matches`,
    /// 100). Only on the first page
    /// (`offset=0`) of a text or regex search.
    #[serde(default)]
    files: usize,
    /// Candidate files fast ranking may open (default: the server's
    /// `fast_rank_files`, 2000; capped by `max_fast_rank_files`).
    #[serde(default)]
    max_files: Option<usize>,
    /// Only search these files and what they import: semicolon-separated
    /// `PATH[:N|:*]` (N hops, 1 by default), `-PATH` to leave them out.
    /// The same as `deps:` in the query; for a regex, the only way.
    #[serde(default)]
    deps: String,
    /// As `deps`, for the files that import them (`rdeps:`).
    #[serde(default)]
    rdeps: String,
}

/// Split a semicolon-separated `deps` / `rdeps` parameter.
pub fn split_graph_param(param: &str) -> Vec<String> {
    param
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

/// Records a search's wall time when dropped, so the metric covers the
/// search's real lifetime even when the HTTP response was abandoned.
struct SearchTimer {
    metrics: std::sync::Arc<super::metrics::Metrics>,
    start: std::time::Instant,
}

impl Drop for SearchTimer {
    fn drop(&mut self) {
        self.metrics.record_search(self.start.elapsed());
    }
}

/// Maximum lines of context on each side for `/api/context` (hover preview /
/// "show more" windows). Larger windows should use `/api/file`.
const MAX_CONTEXT_WINDOW_LINES: usize = 200;

/// Search result for JSON response (also the shape the `fcs` CLI consumes,
/// from the server or from an in-process offline search).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultJson {
    pub file_path: String,
    pub content: String,
    pub line_number: usize,
    /// Start position of match in content
    pub match_start: usize,
    /// End position of match in content
    pub match_end: usize,
    /// Whether content was truncated from original line
    pub content_truncated: bool,
    /// Byte offset of the match start within the FULL line (not the possibly
    /// truncated `content`); within the display path for filename hits.
    pub line_match_start: usize,
    /// Byte offset of the match end within the full line.
    pub line_match_end: usize,
    /// 0-based character column of the match start within the full line
    /// (use this to place an editor cursor).
    pub match_column: usize,
    pub score: f64,
    pub match_type: std::borrow::Cow<'static, str>,
    /// For a reference: `call`, `type`, `value` or `attribute`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_kind: Option<std::borrow::Cow<'static, str>>,
    pub dependency_count: u32,
    /// Hits the search found in this file across all pages; a file cut at
    /// a page boundary has only some of them on this page.
    #[serde(default)]
    pub file_hits: u32,
    /// The file may hold more hits than `file_hits` (per-file cap or match
    /// budget reached).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub file_hits_capped: bool,
    /// Context lines before and after the match (only present when context > 0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_lines: Option<Vec<String>>,
    /// 1-based line number of the first context line (only present when context > 0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_start_line: Option<usize>,
    /// Import hops from the `deps:` / `rdeps:` anchor (0 = the anchor or its
    /// package entry file); only on a graph-scoped search.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hops: Option<u32>,
}

/// A file whose path matched the query (the `files` parameter of `/api/search`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMatchJson {
    /// The path as results show it.
    pub file_path: String,
    pub score: f64,
    /// `[start, end)` byte offsets of `file_path` that matched.
    pub match_ranges: Vec<[usize; 2]>,
}

/// Search response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResultJson>,
    /// Files whose path matches the query, best first (only when asked for
    /// with `files`). Separate from `results`, which are content hits.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<FileMatchJson>,
    /// How many files' paths matched in all (only when `files` was asked for).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_files: Option<usize>,
    /// How current the index is, so clients can warn that recent writes
    /// may be missing when no file watcher is running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<IndexFreshness>,
    pub query: String,
    pub total_results: usize,
    /// True when more results exist beyond this page: either `total_matches`
    /// exceeds `offset + total_results`, or the search stopped early on its
    /// match budget / deadline (`truncated_by_budget`).
    pub has_more: bool,
    /// Offset that was applied to this page.
    pub offset: usize,
    /// Total matches found across all searched files (before paging), when
    /// the search ran to completion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_matches: Option<usize>,
    /// True when the match budget or deadline stopped the scan early.
    pub truncated_by_budget: bool,
    /// Every limit that cut this search short, so a capped result never
    /// passes for a complete one. Absent when nothing was cut.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limits_hit: Vec<LimitHitJson>,
    /// Time taken by the search in milliseconds
    pub elapsed_ms: f64,
    /// Ranking mode used: "auto", "fast", or "full"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank_mode: Option<String>,
    /// Total candidate files considered
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_candidates: Option<usize>,
    /// Files actually searched (may be less in fast mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidates_searched: Option<usize>,
    /// Most candidate files this server lets a request open (`max_files`),
    /// when it sets a limit. A client may ask for up to this many.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_files_allowed: Option<usize>,
    /// The files a `deps:` / `rdeps:` search was limited to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ScopeJson>,
}

/// What a graph-scoped search searched.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScopeJson {
    /// The filters, normalized: `rdeps:src/a.rs:1`.
    pub filters: Vec<String>,
    /// Files in the scope (absent when every filter is negated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<usize>,
    /// Imports written as project paths that resolve to no file, in files
    /// of the scope: the scope may be missing files behind them.
    #[serde(default)]
    pub unresolved_local: usize,
}

impl ScopeJson {
    pub fn from_scope(scope: &crate::search::GraphScope) -> Self {
        Self {
            filters: scope.filters.clone(),
            files: scope.file_count(),
            unresolved_local: scope.unresolved_local,
        }
    }
}

/// A limit that cut a search short.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LimitHitJson {
    /// `max_files`, `match_budget`, `per_file_cap` or `timeout`.
    pub limit: String,
    /// What it cut, for people.
    pub detail: String,
}

impl LimitHitJson {
    pub fn all(info: &crate::search::SearchRankingInfo) -> Vec<Self> {
        info.limits_hit
            .iter()
            .map(|l| Self {
                limit: l.as_str().to_string(),
                detail: l.describe(info),
            })
            .collect()
    }
}

/// Index stats response
#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub num_files: usize,
    pub total_size: u64,
    pub num_trigrams: usize,
    pub dependency_edges: usize,
    /// Total bytes of text content indexed
    pub total_content_bytes: u64,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub server_type: &'static str,
    /// Startup problems recorded for `/api/diagnostics` (busy port, index
    /// owned by another server, config warnings), so the UI can point at them.
    pub problems: usize,
}

/// Indexing status response
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub status: String,
    pub files_discovered: usize,
    pub files_indexed: usize,
    pub files_transcoded: usize,
    pub current_batch: usize,
    pub total_batches: usize,
    pub current_path: Option<String>,
    pub progress_percent: u8,
    pub elapsed_secs: Option<f64>,
    pub errors: usize,
    pub message: String,
    pub is_indexing: bool,
    // Stats fields (included to avoid separate HTTP request)
    pub num_files: usize,
    pub total_size: u64,
    pub num_trigrams: usize,
    pub dependency_edges: usize,
    /// Total bytes of text content indexed
    pub total_content_bytes: u64,
    /// How current the index is (absent while the engine is busy).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<IndexFreshness>,
    /// Per-path comparison with the disk, for the `paths` that were asked about.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<PathStatus>,
}

/// `/api/status` query: `paths=a.rs;src/b.rs` also reports, for each path,
/// whether the index still matches the file on disk.
#[derive(Debug, Default, Deserialize)]
pub struct StatusQuery {
    #[serde(default)]
    pub paths: Option<String>,
}

/// Whether one path's indexed content matches the disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathStatus {
    pub path: String,
    pub state: PathFreshness,
}

/// Most paths one status request checks (each costs a stat).
pub const MAX_STATUS_PATHS: usize = 100;

/// Compare each of `paths` with the disk (one stat each).
pub fn path_statuses(engine: &SearchEngine, paths: &[String]) -> Vec<PathStatus> {
    paths
        .iter()
        .take(MAX_STATUS_PATHS)
        .map(|p| PathStatus {
            path: p.clone(),
            state: engine.path_freshness(p),
        })
        .collect()
}

/// Cached, per-generation part of the diagnostics response.
#[derive(Debug, Clone)]
pub struct DiagnosticsCache {
    pub generation: u64,
    pub files_by_extension: Vec<ExtensionBreakdown>,
    pub memory: crate::search::MemoryBreakdown,
}

/// Readiness: 200 once the index can serve results (a build/reconcile has
/// completed, or an index is loaded and no build is running), 503 otherwise.
/// `/api/health` stays a pure liveness check.
pub async fn ready_handler(State(state): State<WebState>) -> Result<Json<ReadyResponse>, ApiError> {
    let (ready, status, num_files) = readiness(&state);
    let body = ReadyResponse {
        ready,
        status,
        num_files,
    };
    if ready {
        Ok(Json(body))
    } else {
        Err(ApiError::from((
            StatusCode::SERVICE_UNAVAILABLE,
            format!(
                "Index not ready (status: {}, files: {})",
                body.status, body.num_files
            ),
        )))
    }
}

/// Readiness response
#[derive(Debug, Serialize)]
pub struct ReadyResponse {
    pub ready: bool,
    pub status: String,
    pub num_files: usize,
}

/// (ready, status name, live file count) without blocking a worker thread.
fn readiness(state: &WebState) -> (bool, String, usize) {
    let status = state
        .progress
        .try_read()
        .map(|p| p.status)
        .unwrap_or_default();
    // A momentarily held write lock (a watcher batch, a checkpoint) must not
    // read as "no files": fall back to the last count we saw.
    let num_files = match state.engine.try_read() {
        Ok(e) => {
            let n = e.get_stats().num_files;
            state
                .last_known_files
                .store(n, std::sync::atomic::Ordering::Relaxed);
            n
        }
        Err(_) => state
            .last_known_files
            .load(std::sync::atomic::Ordering::Relaxed),
    };
    // Searches are served between batches while a persisted index is
    // reconciled or imports are resolved, so those states are ready too
    // once there is something to search.
    let ready = match status {
        IndexingStatus::Completed => true,
        IndexingStatus::Idle | IndexingStatus::Reconciling | IndexingStatus::ResolvingImports => {
            num_files > 0
        }
        _ => false,
    };
    (ready, format!("{status:?}").to_lowercase(), num_files)
}

/// Prometheus text exposition of request counters and index gauges.
pub async fn metrics_handler(State(state): State<WebState>) -> impl IntoResponse {
    let (ready, status, _) = readiness(&state);
    let indexing = !matches!(status.as_str(), "completed" | "idle");
    let gauges = state
        .engine
        .try_read()
        .map(|e| {
            let s = e.get_stats();
            super::metrics::IndexGauges {
                files: s.num_files as u64,
                trigrams: s.num_trigrams as u64,
                dependency_edges: s.dependency_edges as u64,
                content_bytes: s.total_content_bytes,
                indexing,
                ready,
            }
        })
        .unwrap_or(super::metrics::IndexGauges {
            indexing,
            ready,
            ..Default::default()
        });
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        state.metrics.render(&gauges),
    )
}

/// Handle search requests
pub async fn search_handler(
    State(state): State<WebState>,
    ApiQuery(params): ApiQuery<SearchQuery>,
) -> Result<Json<SearchResponse>, ApiError> {
    let query = params.q.trim().to_string();

    if query.is_empty() {
        return Ok(Json(SearchResponse {
            results: vec![],
            files: vec![],
            total_files: None,
            index: None,
            query: String::new(),
            total_results: 0,
            has_more: false,
            offset: 0,
            total_matches: Some(0),
            truncated_by_budget: false,
            limits_hit: Vec::new(),
            elapsed_ms: 0.0,
            rank_mode: None,
            total_candidates: None,
            candidates_searched: None,
            max_files_allowed: None,
            scope: None,
        }));
    }

    // Paging, caps and the deadline are the same for every front end (see
    // `search::request`); the engine stops just under the HTTP request
    // timeout so a scan whose response was abandoned ends on its own.
    let mode =
        crate::search::SearchMode::from_flags(params.regex, params.symbols, params.references)
            .map_err(|e| ApiError::from((StatusCode::BAD_REQUEST, e)))?;
    let request = crate::search::SearchRequest {
        query: query.clone(),
        mode,
        case_sensitive: params.case,
        whole_word: params.word,
        include: params.include,
        exclude: params.exclude,
        max_results: params.max,
        offset: params.offset,
        rank: RankMode::parse(&params.rank),
        timeout: (params.timeout_ms > 0)
            .then(|| std::time::Duration::from_millis(params.timeout_ms)),
        max_files: params.max_files,
        path_matches: params.files.min(state.search_settings.max_path_matches),
        deps: split_graph_param(&params.deps),
        rdeps: split_graph_param(&params.rdeps),
    };
    let settings = state.search_settings;
    let limits = request
        .limits(&settings, Some(state.request_timeout))
        .map_err(|e| ApiError::from((StatusCode::BAD_REQUEST, e)))?;
    let max_files_allowed =
        (settings.max_fast_rank_files != usize::MAX).then_some(settings.max_fast_rank_files);
    let context_lines = params.context.min(settings.max_context_lines);
    let offset = request.offset;

    // Concurrency limit: each search occupies a blocking-pool thread, so
    // beyond the configured number we answer 503 + Retry-After immediately
    // rather than letting requests pile up and starve the other endpoints.
    let permit = match state.search_permits.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => {
            state.metrics.record_rejected();
            return Err(ApiError::from((
                StatusCode::SERVICE_UNAVAILABLE,
                "Too many concurrent searches, please try again shortly".to_string(),
            )));
        }
    };
    let metrics = state.metrics.clone();
    let timer = SearchTimer {
        metrics: metrics.clone(),
        start: std::time::Instant::now(),
    };

    let engine = state.engine.clone();
    let outcome = tokio::task::spawn_blocking(move || -> Result<_, (StatusCode, String)> {
        // The permit and the latency timer live inside the blocking task so
        // they reflect the search's real lifetime: if the HTTP layer times
        // out and drops the response future, the slot stays taken and the
        // request is still counted until the scan actually ends.
        let _permit = permit;
        let _timer = timer;
        // Start timing the search
        let start_time = std::time::Instant::now();

        // Times the search for diagnostic captures (inert unless they are on).
        let mut probe = crate::diagnostics::capture::SearchProbe::start(
            "rest",
            request.mode.name(),
            &request.query,
        );

        // Use try_read to avoid blocking when a write lock is held during indexing.
        // Blocking here would cause threads to pile up and exhaust the thread pool.
        let engine = try_read_engine(&engine)?;
        probe.locked();

        let outcome = engine
            .run(&request, limits)
            .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
        probe.searched(&outcome.info, outcome.matches.len());
        let has_more = outcome.has_more(offset);
        let ranking_info = outcome.info;
        let (files, total_files) = match outcome.paths {
            Some(found) => (file_matches_to_json(found.matches), Some(found.total)),
            None => (Vec::new(), None),
        };
        // Context lines: each file is read and split ONCE per request (the
        // result already carries the file id — no path lookup, no per-result
        // re-read when many hits come from one file).
        let results = results_to_json(
            &engine,
            outcome.matches,
            context_lines,
            outcome.scope.as_deref(),
        );
        let total_results = results.len();
        let elapsed_ms = start_time.elapsed().as_secs_f64() * 1000.0;

        Ok(Json(SearchResponse {
            results,
            files,
            total_files,
            index: Some(engine.freshness()),
            query,
            total_results,
            has_more,
            offset,
            total_matches: ranking_info.total_matches,
            truncated_by_budget: ranking_info.truncated_by_budget,
            limits_hit: LimitHitJson::all(&ranking_info),
            elapsed_ms,
            rank_mode: Some(format!("{:?}", ranking_info.mode).to_lowercase()),
            total_candidates: Some(ranking_info.total_candidates),
            candidates_searched: Some(ranking_info.candidates_searched),
            max_files_allowed,
            scope: outcome.scope.as_deref().map(ScopeJson::from_scope),
        }))
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task join error: {}", e),
        )
    })?
    .map_err(ApiError::from);

    if let Err(e) = &outcome {
        match e.status {
            StatusCode::SERVICE_UNAVAILABLE => metrics.record_unavailable(),
            s if s.is_client_error() => metrics.record_client_error(),
            _ => {}
        }
    }
    outcome
}

/// Turn path matches into the API's shape.
pub fn file_matches_to_json(matches: Vec<crate::search::PathMatch>) -> Vec<FileMatchJson> {
    matches
        .into_iter()
        .map(|m| FileMatchJson {
            file_path: m.file_path,
            score: m.score,
            match_ranges: m.ranges.into_iter().map(|(s, e)| [s, e]).collect(),
        })
        .collect()
}

/// Turn engine matches into the API's result shape, attaching `context_lines`
/// lines of context around each hit (read from the engine's file store),
/// and each hit's hops when the search was graph-scoped.
pub fn results_to_json(
    engine: &SearchEngine,
    matches: Vec<crate::search::SearchMatch>,
    context_lines: usize,
    scope: Option<&crate::search::GraphScope>,
) -> Vec<SearchResultJson> {
    let mut line_cache: HashMap<u32, Option<Vec<String>>> = HashMap::new();
    matches
        .into_iter()
        .map(|m| {
            let (ctx_lines, ctx_start) = if context_lines > 0 && m.line_number > 0 {
                let lines = line_cache.entry(m.file_id).or_insert_with(|| {
                    engine.file_store.get(m.file_id).and_then(|f| {
                        f.as_str()
                            .ok()
                            .map(|c| c.lines().map(str::to_string).collect())
                    })
                });
                match lines {
                    Some(all_lines) => {
                        let total = all_lines.len();
                        let match_idx =
                            m.line_number.saturating_sub(1).min(total.saturating_sub(1));
                        let start_idx = match_idx.saturating_sub(context_lines);
                        let end_idx = (match_idx + context_lines + 1).min(total);
                        (
                            Some(all_lines[start_idx..end_idx].to_vec()),
                            Some(start_idx + 1),
                        )
                    }
                    None => (None, None),
                }
            } else {
                (None, None)
            };

            SearchResultJson {
                file_path: m.file_path,
                content: m.content,
                line_number: m.line_number,
                match_start: m.match_start,
                match_end: m.match_end,
                content_truncated: m.content_truncated,
                line_match_start: m.line_match_start,
                line_match_end: m.line_match_end,
                match_column: m.match_column,
                score: m.score,
                match_type: std::borrow::Cow::Borrowed(if m.is_reference {
                    "SYMBOL_REFERENCE"
                } else if m.is_symbol {
                    "SYMBOL_DEFINITION"
                } else {
                    "TEXT"
                }),
                reference_kind: m
                    .reference_kind
                    .map(|k| std::borrow::Cow::Borrowed(k.as_str())),
                dependency_count: m.dependency_count,
                file_hits: m.file_hits,
                file_hits_capped: m.file_hits_capped,
                context_lines: ctx_lines,
                context_start_line: ctx_start,
                hops: scope.and_then(|s| s.hops(m.file_id)),
            }
        })
        .collect()
}

/// Handle stats requests
pub async fn stats_handler(State(state): State<WebState>) -> Result<Json<StatsResponse>, ApiError> {
    let engine = state.engine.clone();
    tokio::task::spawn_blocking(move || -> Result<_, (StatusCode, String)> {
        let engine = try_read_engine(&engine)?;

        let stats = engine.get_stats();

        Ok(Json(StatsResponse {
            num_files: stats.num_files,
            total_size: stats.total_size,
            num_trigrams: stats.num_trigrams,
            dependency_edges: stats.dependency_edges,
            total_content_bytes: stats.total_content_bytes,
        }))
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task join error: {}", e),
        )
    })?
    .map_err(ApiError::from)
}

/// Handle health check requests
pub async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy",
        version: env!("CARGO_PKG_VERSION"),
        server_type: "keyword",
        problems: diagnostics::service_status(None).problems.len(),
    })
}

/// Handle indexing status requests
pub async fn status_handler(
    State(state): State<WebState>,
    ApiQuery(params): ApiQuery<StatusQuery>,
) -> Result<Json<StatusResponse>, ApiError> {
    let wanted: Vec<String> = params
        .paths
        .as_deref()
        .unwrap_or("")
        .split(';')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(String::from)
        .collect();
    // Stats come from a try_read below; the per-path stats touch the disk,
    // so they run on the blocking pool first.
    let paths = if wanted.is_empty() {
        Vec::new()
    } else {
        let engine = state.engine.clone();
        tokio::task::spawn_blocking(move || -> Result<_, (StatusCode, String)> {
            let engine = try_read_engine(&engine)?;
            Ok(path_statuses(&engine, &wanted))
        })
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Task join error: {}", e),
            )
        })??
    };

    // Use try_read so we never block a tokio worker thread on a std::sync::RwLock.
    // The progress lock is written by the background indexer; if it is momentarily
    // held we return 503 rather than stalling the async executor.
    let progress = state.progress.try_read().map_err(|e| match e {
        std::sync::TryLockError::WouldBlock => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Index progress is being updated, please retry shortly".to_string(),
        ),
        std::sync::TryLockError::Poisoned(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to acquire progress read lock: {}", e),
        ),
    })?;

    let status_str = match progress.status {
        IndexingStatus::Idle => "idle",
        IndexingStatus::LoadingIndex => "loading_index",
        IndexingStatus::Discovering => "discovering",
        IndexingStatus::Indexing => "indexing",
        IndexingStatus::Reconciling => "reconciling",
        IndexingStatus::ResolvingImports => "resolving_imports",
        IndexingStatus::Completed => "completed",
    };

    let is_indexing = matches!(
        progress.status,
        IndexingStatus::LoadingIndex
            | IndexingStatus::Discovering
            | IndexingStatus::Indexing
            | IndexingStatus::Reconciling
            | IndexingStatus::ResolvingImports
    );

    // Get stats from the engine if available (try_read to avoid blocking during indexing)
    let (num_files, total_size, num_trigrams, dependency_edges, total_content_bytes, index) = {
        match state.engine.try_read() {
            Ok(engine) => {
                let stats = engine.get_stats();
                (
                    stats.num_files,
                    stats.total_size,
                    stats.num_trigrams,
                    stats.dependency_edges,
                    stats.total_content_bytes,
                    Some(engine.freshness()),
                )
            }
            Err(_) => (0, 0, 0, 0, 0, None),
        }
    };

    Ok(Json(StatusResponse {
        status: status_str.to_string(),
        files_discovered: progress.files_discovered,
        files_indexed: progress.files_indexed,
        files_transcoded: progress.files_transcoded,
        current_batch: progress.current_batch,
        total_batches: progress.total_batches,
        current_path: progress.current_path.clone(),
        progress_percent: progress.progress_percent(),
        elapsed_secs: progress.elapsed_secs(),
        errors: progress.errors,
        message: progress.message.clone(),
        is_indexing,
        num_files,
        total_size,
        num_trigrams,
        dependency_edges,
        total_content_bytes,
        index,
        paths,
    }))
}

/// Query parameters for dependency endpoints
#[derive(Debug, Deserialize)]
pub struct DependencyQuery {
    /// File path to look up
    file: String,
}

/// Dependency response
#[derive(Debug, Serialize)]
pub struct DependencyResponse {
    pub file: String,
    pub files: Vec<String>,
    pub count: usize,
}

/// `file=` must name something: an empty or blank value used to fall
/// through to the suffix lookup and return an arbitrary indexed file.
pub(super) fn require_file_param(file: &str) -> Result<(), ApiError> {
    if file.trim().is_empty() {
        return Err(ApiError::from((
            StatusCode::BAD_REQUEST,
            "file must not be empty".to_string(),
        )));
    }
    Ok(())
}

/// Get files that depend on (import) the specified file
pub async fn dependents_handler(
    State(state): State<WebState>,
    ApiQuery(params): ApiQuery<DependencyQuery>,
) -> Result<Json<DependencyResponse>, ApiError> {
    require_file_param(&params.file)?;
    let engine = state.engine.clone();
    tokio::task::spawn_blocking(move || -> Result<_, (StatusCode, String)> {
        let engine = try_read_engine(&engine)?;

        let file_id = engine.find_file_id(&params.file).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("File not found: {}", params.file),
            )
        })?;

        let dependent_ids = engine.get_dependents(file_id);
        let files: Vec<String> = dependent_ids
            .iter()
            .filter_map(|&id| engine.get_file_path(id))
            .collect();

        let count = files.len();

        Ok(Json(DependencyResponse {
            file: params.file,
            files,
            count,
        }))
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task join error: {}", e),
        )
    })?
    .map_err(ApiError::from)
}

/// Get files that the specified file depends on (imports)
pub async fn dependencies_handler(
    State(state): State<WebState>,
    ApiQuery(params): ApiQuery<DependencyQuery>,
) -> Result<Json<DependencyResponse>, ApiError> {
    require_file_param(&params.file)?;
    let engine = state.engine.clone();
    tokio::task::spawn_blocking(move || -> Result<_, (StatusCode, String)> {
        let engine = try_read_engine(&engine)?;

        let file_id = engine.find_file_id(&params.file).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("File not found: {}", params.file),
            )
        })?;

        let dependency_ids = engine.get_dependencies(file_id);
        let files: Vec<String> = dependency_ids
            .iter()
            .filter_map(|&id| engine.get_file_path(id))
            .collect();

        let count = files.len();

        Ok(Json(DependencyResponse {
            file: params.file,
            files,
            count,
        }))
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task join error: {}", e),
        )
    })?
    .map_err(ApiError::from)
}

/// Query parameters for file content endpoint
#[derive(Debug, Deserialize)]
pub struct FileQuery {
    /// File path to retrieve
    file: String,
}

/// File content response
#[derive(Debug, Serialize)]
pub struct FileResponse {
    pub file: String,
    pub content: String,
    pub size_bytes: usize,
    pub line_count: usize,
}

/// Return the full content of a file by path
pub async fn file_handler(
    State(state): State<WebState>,
    ApiQuery(params): ApiQuery<FileQuery>,
) -> Result<Json<FileResponse>, ApiError> {
    require_file_param(&params.file)?;
    let engine = state.engine.clone();
    tokio::task::spawn_blocking(move || -> Result<_, (StatusCode, String)> {
        let engine = try_read_engine(&engine)?;

        let file_id = engine.find_file_id(&params.file).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("File not found: {}", params.file),
            )
        })?;

        let mapped = engine.file_store.get(file_id).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("File not found in store: {}", params.file),
            )
        })?;

        let content = mapped.as_str().map_err(|e| {
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("File is not valid UTF-8: {}", e),
            )
        })?;

        let size_bytes = content.len();
        let line_count = content.lines().count();
        let file_path = engine.make_display_path(&mapped.path);

        Ok(Json(FileResponse {
            file: file_path,
            content: content.to_string(),
            size_bytes,
            line_count,
        }))
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task join error: {}", e),
        )
    })?
    .map_err(ApiError::from)
}

/// Query parameters for context (lines around a hit) endpoint
#[derive(Debug, Deserialize)]
pub struct ContextQuery {
    /// File path
    file: String,
    /// 1-based line number of the match
    line: usize,
    /// Number of lines of context to return before and after (default 5)
    #[serde(default = "default_context_lines")]
    context: usize,
}

fn default_context_lines() -> usize {
    5
}

/// Context response: a window of lines around the matched line
#[derive(Debug, Serialize)]
pub struct ContextResponse {
    pub file: String,
    pub start_line: usize,
    pub lines: Vec<String>,
    pub match_line: usize,
}

/// Return a window of lines around a matched line for the hover tooltip
pub async fn context_handler(
    State(state): State<WebState>,
    ApiQuery(params): ApiQuery<ContextQuery>,
) -> Result<Json<ContextResponse>, ApiError> {
    require_file_param(&params.file)?;
    let engine = state.engine.clone();
    tokio::task::spawn_blocking(move || -> Result<_, (StatusCode, String)> {
        let engine = try_read_engine(&engine)?;

        let file_id = engine.find_file_id(&params.file).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("File not found: {}", params.file),
            )
        })?;

        let mapped = engine.file_store.get(file_id).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("File not found in store: {}", params.file),
            )
        })?;

        let content = mapped.as_str().map_err(|e| {
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("File is not valid UTF-8: {}", e),
            )
        })?;

        let all_lines: Vec<&str> = content.lines().collect();
        let total = all_lines.len();

        // line is 1-based; clamp to valid range. The window is capped so a
        // client cannot pull an entire file through the "lightweight" endpoint
        // (or overflow usize with context=usize::MAX).
        let context = params.context.min(MAX_CONTEXT_WINDOW_LINES);
        let match_idx = params.line.saturating_sub(1).min(total.saturating_sub(1));
        let start_idx = match_idx.saturating_sub(context);
        let end_idx = match_idx
            .saturating_add(context)
            .saturating_add(1)
            .min(total);

        let lines: Vec<String> = all_lines[start_idx..end_idx]
            .iter()
            .map(|l| l.to_string())
            .collect();
        let start_line = start_idx + 1; // 1-based

        Ok(Json(ContextResponse {
            // The same workspace-relative form search results and /api/file
            // use, not the absolute host path.
            file: engine.make_display_path(&mapped.path),
            start_line,
            lines,
            match_line: params.line,
        }))
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task join error: {}", e),
        )
    })?
    .map_err(ApiError::from)
}

/// WebSocket upgrade handler for progress streaming
pub async fn ws_progress_handler(
    State(state): State<WebState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    // Inbound frames are ignored, so there is no reason to buffer the
    // defaults (64 MiB messages / 16 MiB frames) per connection.
    ws.max_message_size(4 * 1024)
        .max_frame_size(4 * 1024)
        .on_upgrade(|socket| handle_progress_socket(socket, state))
}

/// Helper to get stats from engine
fn get_stats_from_engine(engine: &super::AppState) -> ProgressStats {
    engine
        .try_read()
        .ok()
        .map(|e| {
            let stats = e.get_stats();
            ProgressStats {
                num_files: stats.num_files,
                total_size: stats.total_size,
                num_trigrams: stats.num_trigrams,
                dependency_edges: stats.dependency_edges,
                total_content_bytes: stats.total_content_bytes,
                index: Some(e.freshness()),
            }
        })
        .unwrap_or_default()
}

/// The engine's read lock for a request, waiting up to
/// [`engine_lock::SEARCH_READ_WAIT`] for a writer (a watcher batch step, a
/// checkpoint) to finish. Call it on a blocking thread, never a tokio
/// worker.
///
/// A writer still holding the lock after that becomes a 503 with
/// `Retry-After`. A *poisoned* lock is recovered rather than turned into a
/// permanent 500: every indexing path is wrapped in `catch_unwind`, so poison
/// only means some other thread panicked, not that the index is unusable.
pub(super) fn try_read_engine(
    engine: &std::sync::RwLock<SearchEngine>,
) -> Result<engine_lock::ReadGuard<'_, SearchEngine>, (StatusCode, String)> {
    engine_lock::read_within(engine, engine_lock::SEARCH_READ_WAIT).ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "Index is currently being updated, please try again shortly".to_string(),
        )
    })
}

/// Handle a WebSocket connection for progress updates
async fn handle_progress_socket(socket: WebSocket, state: WebState) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to progress broadcast channel
    let mut rx = state.progress_tx.subscribe();

    // Clone engine for use in spawned task
    let engine = state.engine.clone();

    // Send initial progress state immediately (clone before await to avoid holding lock).
    // Use try_read to avoid blocking the async executor on a std::sync::RwLock.
    let initial_json = {
        let stats = get_stats_from_engine(&state.engine);
        state.progress.try_read().ok().and_then(|progress| {
            let status_response = progress_to_status(&progress, stats);
            serde_json::to_string(&status_response).ok()
        })
    };
    if let Some(json) = initial_json {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    // Spawn a task to forward broadcast messages to the WebSocket. A ping
    // every 30 s keeps half-open connections from lingering until TCP
    // gives up (the client answers pongs automatically).
    let send_task = tokio::spawn(async move {
        let mut ping = tokio::time::interval(std::time::Duration::from_secs(30));
        ping.tick().await; // first tick fires immediately; skip it
        loop {
            let event = tokio::select! {
                _ = ping.tick() => {
                    if sender.send(Message::Ping(Vec::new().into())).await.is_err() {
                        break;
                    }
                    continue;
                }
                event = rx.recv() => event,
            };
            match event {
                Ok(progress) => {
                    let stats = get_stats_from_engine(&engine);
                    let status_response = progress_to_status(&progress, stats);
                    match serde_json::to_string(&status_response) {
                        Ok(json) => {
                            if sender.send(Message::Text(json.into())).await.is_err() {
                                break; // Client disconnected
                            }
                        }
                        Err(_) => continue,
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    // We fell behind, just continue with next message
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break; // Channel closed
                }
            }
        }
    });

    // Wait for client to close connection or send a close message
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Close(_)) => break,
            Err(_) => break,
            _ => {} // Ignore other messages (ping/pong handled automatically)
        }
    }

    // Clean up
    send_task.abort();
}

/// Stats for inclusion in StatusResponse
#[derive(Debug, Clone, Default)]
pub struct ProgressStats {
    pub num_files: usize,
    pub total_size: u64,
    pub num_trigrams: usize,
    pub dependency_edges: usize,
    pub total_content_bytes: u64,
    pub index: Option<IndexFreshness>,
}

/// Convert IndexingProgress to StatusResponse with stats
fn progress_to_status(
    progress: &crate::search::IndexingProgress,
    stats: ProgressStats,
) -> StatusResponse {
    let status_str = match progress.status {
        IndexingStatus::Idle => "idle",
        IndexingStatus::LoadingIndex => "loading_index",
        IndexingStatus::Discovering => "discovering",
        IndexingStatus::Indexing => "indexing",
        IndexingStatus::Reconciling => "reconciling",
        IndexingStatus::ResolvingImports => "resolving_imports",
        IndexingStatus::Completed => "completed",
    };

    let is_indexing = matches!(
        progress.status,
        IndexingStatus::LoadingIndex
            | IndexingStatus::Discovering
            | IndexingStatus::Indexing
            | IndexingStatus::Reconciling
            | IndexingStatus::ResolvingImports
    );

    StatusResponse {
        status: status_str.to_string(),
        files_discovered: progress.files_discovered,
        files_indexed: progress.files_indexed,
        files_transcoded: progress.files_transcoded,
        current_batch: progress.current_batch,
        total_batches: progress.total_batches,
        current_path: progress.current_path.clone(),
        progress_percent: progress.progress_percent(),
        elapsed_secs: progress.elapsed_secs(),
        errors: progress.errors,
        message: progress.message.clone(),
        is_indexing,
        // Include stats
        num_files: stats.num_files,
        total_size: stats.total_size,
        num_trigrams: stats.num_trigrams,
        dependency_edges: stats.dependency_edges,
        total_content_bytes: stats.total_content_bytes,
        index: stats.index,
        paths: Vec::new(),
    }
}

/// Truncate a string to at most `max_bytes` bytes, ensuring the cut point falls
/// on a UTF-8 character boundary to avoid panics with multi-byte characters.
/// Plain-text search for `term` restricted to `file_path` (an absolute
/// stored path), for the diagnostics self-tests.
fn search_within_file(
    engine: &SearchEngine,
    term: &str,
    file_path: &str,
) -> Vec<crate::search::SearchMatch> {
    let display = engine.make_display_path(std::path::Path::new(file_path));
    let parsed = crate::search::parse_query(term);
    engine
        .search_parsed(&parsed, &display, "", SearchLimits::new(5), RankMode::Full)
        .map(|(matches, _)| matches)
        .unwrap_or_default()
}

fn safe_truncate(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Handle diagnostics requests with self-tests
pub async fn diagnostics_handler(
    State(state): State<WebState>,
    ApiQuery(params): ApiQuery<DiagnosticsQuery>,
) -> Result<Json<KeywordDiagnosticsResponse>, ApiError> {
    let sample_count = params.sample_count.clamp(1, 20);
    let force_refresh = params.force_refresh;
    let engine = state.engine.clone();
    let indexer_config = state.indexer_config.clone();
    let diagnostics_cache = state.diagnostics_cache.clone();

    tokio::task::spawn_blocking(move || -> Result<_, (StatusCode, String)> {
        // Use try_read to avoid blocking when a write lock is held during indexing.
        let engine = try_read_engine(&engine)?;

        // Get basic stats
        let stats = engine.get_stats();

        // Extension breakdown: an O(files) walk, cached per engine generation
        // (every mutation bumps it) unless the client forces a refresh.
        let generation = engine.generation();
        let cached = if force_refresh {
            None
        } else {
            diagnostics_cache
                .lock()
                .ok()
                .and_then(|c| c.as_ref().filter(|c| c.generation == generation).cloned())
        };
        let (files_by_extension, memory) = match cached {
            Some(c) => (c.files_by_extension, c.memory),
            None => {
                let mut ext_map: HashMap<String, (usize, u64)> = HashMap::new();
                for file_id in 0..engine.file_store.len() as u32 {
                    if let Some(mapped_file) = engine.file_store.get(file_id) {
                        let ext = mapped_file
                            .path
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("(none)")
                            .to_lowercase();
                        let entry = ext_map.entry(ext).or_insert((0, 0));
                        entry.0 += 1;
                        // The size as indexed: no file is read for this.
                        entry.1 += engine.indexed_size(file_id);
                    }
                }
                let mut v: Vec<ExtensionBreakdown> = ext_map
                    .into_iter()
                    .map(|(ext, (count, bytes))| ExtensionBreakdown {
                        extension: ext,
                        count,
                        total_bytes: bytes,
                    })
                    .collect();
                v.sort_by_key(|f| std::cmp::Reverse(f.count));
                v.truncate(20); // Top 20 extensions
                let memory = engine.memory_breakdown();
                if let Ok(mut c) = diagnostics_cache.lock() {
                    *c = Some(DiagnosticsCache {
                        generation,
                        files_by_extension: v.clone(),
                        memory: memory.clone(),
                    });
                }
                (v, memory)
            }
        };

        // Sample by id first; only the sampled files get a path String.
        let mut rng = rand::rng();
        let live_ids: Vec<u32> = (0..engine.file_store.len() as u32)
            .filter(|&id| engine.file_store.get(id).is_some())
            .collect();
        let path_of = |id: u32| -> String {
            engine
                .file_store
                .get(id)
                .map(|f| f.path.to_string_lossy().to_string())
                .unwrap_or_default()
        };
        let sample_count_actual = sample_count.min(live_ids.len());
        // Shown as in search results (relative to the indexed folder), not as
        // absolute paths on the server's disk.
        let sample_files: Vec<String> = live_ids
            .choose_multiple(&mut rng, sample_count_actual)
            .map(|&id| {
                engine
                    .display_path(id)
                    .map(|p| p.into_owned())
                    .unwrap_or_else(|| path_of(id))
            })
            .collect();
        // Small pool for the self-tests below (they pick from it at random).
        let all_file_paths: Vec<(u32, String)> = live_ids
            .choose_multiple(&mut rng, 32.min(live_ids.len()))
            .map(|&id| (id, path_of(id)))
            .collect();

        // Real configuration when the router was built by the server; the
        // embedded/test router has none.
        let config = indexer_config
            .as_deref()
            .map(ConfigSummary::from)
            .unwrap_or_else(|| ConfigSummary {
                indexed_paths: vec!["(not available: router built without config)".to_string()],
                include_extensions: vec![],
                exclude_patterns: vec![],
                max_file_size_bytes: 0,
                index_path: None,
                watch_enabled: false,
            });

        // Run self-tests
        let mut self_tests = Vec::new();

        // Test 1: Random file search - pick a random indexed file and search for part of its filename
        // (Filenames are indexed as searchable content, so this tests that feature)
        if !all_file_paths.is_empty() {
            let test_start = Instant::now();
            let (test_file_id, test_file_path) = all_file_paths.choose(&mut rng).unwrap();
            let test_file_path = test_file_path.clone(); // Clone to avoid borrow issues
            let _ = test_file_id; // Suppress unused warning

            // Extract filename stem for search
            let file_name = std::path::Path::new(&test_file_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("test");

            // Take first 8 bytes or full name if shorter, respecting UTF-8 boundaries
            let search_term = safe_truncate(file_name, 8);

            // Restrict the search to the sampled file: the test asks "is this
            // file findable?", not "does it outrank every other file for a
            // common prefix" (which a healthy 60k-file index rightly fails).
            let search_results = search_within_file(&engine, search_term, &test_file_path);
            let found = !search_results.is_empty();

            let test = if found {
                TestResult::passed(
                    "random_file_search",
                    test_start.elapsed(),
                    format!(
                        "Found file '{}' when searching for '{}'",
                        test_file_path, search_term
                    ),
                )
            } else {
                TestResult::failed(
                    "random_file_search",
                    test_start.elapsed(),
                    format!(
                        "Could not find file '{}' when searching for '{}' ({} results returned)",
                        test_file_path,
                        search_term,
                        search_results.len()
                    ),
                )
                .with_details(format!(
                    "Searched for '{}', expected to find file at path containing '{}'",
                    search_term, test_file_path
                ))
            };
            self_tests.push(test);
        }

        // Test 2: Content sample search - read a line from a random file and search for it
        if !all_file_paths.is_empty() {
            let test_start = Instant::now();
            let (test_file_id, test_file_path) = all_file_paths.choose(&mut rng).unwrap();
            let test_file_id = *test_file_id;
            let test_file_path = test_file_path.clone();

            let mut test_result = None;

            if let Some(mapped_file) = engine.file_store.get(test_file_id) {
                if let Ok(content) = mapped_file.as_str() {
                    // Find a suitable line (non-empty, not too short, avoid problematic patterns)
                    let lines: Vec<&str> = content
                        .lines()
                        .filter(|l| {
                            let trimmed = l.trim();
                            trimmed.len() > 10
                                && trimmed.len() < 100
                                && !trimmed.starts_with("//")
                                && !trimmed.starts_with('#')
                                && !trimmed.starts_with("/*")
                                && !trimmed.starts_with('*')
                                && !trimmed.contains("...")
                                && !trimmed.contains("…")
                                && trimmed.chars().filter(|c| c.is_alphanumeric()).count() >= 8
                        })
                        .collect();

                    if let Some(sample_line) = lines.choose(&mut rng) {
                        // Take a substring to search for, respecting UTF-8 boundaries
                        let search_term = sample_line.trim();
                        let search_term_slice = safe_truncate(search_term, 30);

                        let search_results =
                            search_within_file(&engine, search_term_slice, &test_file_path);
                        let found = !search_results.is_empty();

                        test_result = Some(if found {
                            TestResult::passed(
                                "content_sample_search",
                                test_start.elapsed(),
                                format!(
                                    "Found content from '{}' in search results",
                                    test_file_path
                                ),
                            )
                        } else {
                            TestResult::failed(
                                "content_sample_search",
                                test_start.elapsed(),
                                format!(
                                    "Content search did not return expected file ({} results)",
                                    search_results.len()
                                ),
                            )
                            .with_details(format!(
                                "Searched for '{}...' from file '{}'",
                                safe_truncate(search_term_slice, 20),
                                test_file_path
                            ))
                        });
                    }
                }
            }

            if let Some(tr) = test_result {
                self_tests.push(tr);
            } else {
                self_tests.push(TestResult::passed(
                    "content_sample_search",
                    test_start.elapsed(),
                    "Skipped - no suitable content found for sampling".to_string(),
                ));
            }
        }

        // Test 3: Index integrity - verify file IDs resolve to valid paths
        {
            let test_start = Instant::now();
            let mut valid_count = 0;
            let mut invalid_count = 0;
            let check_count = 10.min(engine.file_store.len());

            for file_id in 0..check_count as u32 {
                if engine.get_file_path(file_id).is_some() {
                    valid_count += 1;
                } else {
                    invalid_count += 1;
                }
            }

            let test = if invalid_count == 0 {
                TestResult::passed(
                    "index_integrity",
                    test_start.elapsed(),
                    format!(
                        "All {} sampled file IDs resolve to valid paths",
                        valid_count
                    ),
                )
            } else {
                TestResult::failed(
                    "index_integrity",
                    test_start.elapsed(),
                    format!(
                        "{} of {} file IDs failed to resolve",
                        invalid_count, check_count
                    ),
                )
            };
            self_tests.push(test);
        }

        // Test 4: Trigram index sanity - verify trigram count is reasonable
        {
            let test_start = Instant::now();
            let num_trigrams = stats.num_trigrams;
            let num_files = stats.num_files;

            // A reasonable heuristic: should have trigrams if we have files
            let test = if num_files == 0 {
                TestResult::passed(
                    "trigram_index",
                    test_start.elapsed(),
                    "No files indexed yet".to_string(),
                )
            } else if num_trigrams > 0 {
                TestResult::passed(
                    "trigram_index",
                    test_start.elapsed(),
                    format!(
                        "Trigram index healthy: {} unique trigrams for {} files",
                        num_trigrams, num_files
                    ),
                )
            } else {
                TestResult::failed(
                    "trigram_index",
                    test_start.elapsed(),
                    format!("No trigrams indexed despite having {} files", num_files),
                )
            };
            self_tests.push(test);
        }

        // Test 5: Regex search functionality
        {
            let test_start = Instant::now();
            // Try a simple regex that should match common patterns
            let regex_result = engine.search_regex(r"fn\s+\w+", "", "", 10);

            let test = match regex_result {
                Ok(results) => TestResult::passed(
                    "regex_search",
                    test_start.elapsed(),
                    format!(
                        "Regex search operational ({} results for 'fn\\s+\\w+')",
                        results.len()
                    ),
                ),
                Err(e) => TestResult::failed(
                    "regex_search",
                    test_start.elapsed(),
                    format!("Regex search failed: {}", e),
                ),
            };
            self_tests.push(test);
        }

        // Calculate overall health status
        let test_summary = TestSummary::from_results(&self_tests);
        let service = diagnostics::service_status(indexer_config.as_deref());
        let status = if test_summary.failed == 0 && service.problems.is_empty() {
            HealthStatus::Healthy
        } else if test_summary.failed <= test_summary.total / 2 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Unhealthy
        };

        let response = KeywordDiagnosticsResponse {
            status,
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_secs: diagnostics::get_uptime_secs(),
            uptime_human: diagnostics::format_uptime(diagnostics::get_uptime_secs()),
            generated_at: diagnostics::get_timestamp(),
            config,
            service,
            captures: diagnostics::capture::status(),
            scrub: {
                let s = engine
                    .watch_backlog
                    .scrub
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .clone();
                s.enabled.then_some(s)
            },
            index: KeywordIndexDiagnostics {
                num_files: stats.num_files,
                total_size_bytes: stats.total_size,
                total_size_human: diagnostics::format_bytes(stats.total_size),
                num_trigrams: stats.num_trigrams,
                dependency_edges: stats.dependency_edges,
                files_by_extension,
                sample_files,
                memory: Some(memory),
                resident_bytes: diagnostics::resident_bytes(),
            },
            self_tests,
            test_summary,
        };

        Ok(Json(response))
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task join error: {}", e),
        )
    })?
    .map_err(ApiError::from)
}

/// Query for `/api/check`.
#[derive(Debug, Deserialize)]
pub struct CheckQuery {
    /// Files whose content to read and check, spread over the index
    /// (default 2,000; 0 skips the content check).
    #[serde(default)]
    pub content: Option<usize>,
    /// Check the content of every file (slow on a large index).
    #[serde(default)]
    pub all: bool,
    /// Walk the disk and compare the indexed file set (default true).
    #[serde(default)]
    pub discovery: Option<bool>,
    /// Example paths kept per kind of difference (default 5, at most 100).
    #[serde(default)]
    pub examples: Option<usize>,
}

/// `GET /api/check`: check the live index against itself and the disk (see
/// [`crate::search::integrity`]). One check runs at a time; a second gets
/// 429. Reads only: searches go on, and the watcher waits at most for one
/// short read-locked step.
pub async fn check_handler(
    State(state): State<WebState>,
    ApiQuery(params): ApiQuery<CheckQuery>,
) -> Result<Json<crate::search::integrity::IntegrityReport>, ApiError> {
    use std::sync::atomic::{AtomicBool, Ordering};
    static RUNNING: AtomicBool = AtomicBool::new(false);
    if RUNNING.swap(true, Ordering::AcqRel) {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "a check is already running; try again when it has finished".to_string(),
        )
            .into());
    }
    struct Done;
    impl Drop for Done {
        fn drop(&mut self) {
            RUNNING.store(false, Ordering::Release);
        }
    }
    let config = state.indexer_config.clone();
    let opts = crate::search::integrity::IntegrityOptions {
        content_files: if params.all {
            None
        } else {
            Some(params.content.unwrap_or(2_000))
        },
        discovery: params.discovery.unwrap_or(true) && config.is_some(),
        examples: params.examples.unwrap_or(5).min(100),
    };
    let engine = state.engine.clone();
    let report = tokio::task::spawn_blocking(move || {
        let _done = Done;
        let config = config.map(|c| (*c).clone()).unwrap_or_default();
        crate::search::integrity::check(&engine, &config, &opts)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("check failed: {e}"),
        )
    })?;
    Ok(Json(report))
}
