//! REST API handlers for Fast Code Search

use super::WebState;
use crate::diagnostics::{
    self, ConfigSummary, DiagnosticsQuery, ExtensionBreakdown, HealthStatus,
    KeywordDiagnosticsResponse, KeywordIndexDiagnostics, TestResult, TestSummary,
};
use crate::search::{IndexingStatus, RankMode};
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

/// Search query parameters
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// The search query string
    q: String,
    /// Maximum number of results (default: 50)
    #[serde(default = "default_max_results")]
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
    /// Ranking mode: "auto" (default), "fast", or "full"
    #[serde(default)]
    rank: String,
}

fn default_max_results() -> usize {
    50
}

/// Search result for JSON response
#[derive(Debug, Serialize)]
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
    pub score: f64,
    pub match_type: &'static str,
    pub dependency_count: u32,
}

/// Search response
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResultJson>,
    pub query: String,
    pub total_results: usize,
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
}

/// Handle search requests
pub async fn search_handler(
    State(state): State<WebState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, (StatusCode, String)> {
    let query = params.q.trim().to_string();

    if query.is_empty() {
        return Ok(Json(SearchResponse {
            results: vec![],
            query: String::new(),
            total_results: 0,
            elapsed_ms: 0.0,
            rank_mode: None,
            total_candidates: None,
            candidates_searched: None,
        }));
    }

    let max_results = params.max.clamp(1, 1000);
    let include_patterns = params.include;
    let exclude_patterns = params.exclude;
    let is_regex = params.regex;
    let symbols_only = params.symbols;

    // Parse ranking mode
    let rank_mode = match params.rank.to_lowercase().as_str() {
        "fast" => RankMode::Fast,
        "full" => RankMode::Full,
        _ => RankMode::Auto, // Default to auto
    };

    let engine = state.engine.clone();
    tokio::task::spawn_blocking(move || {
        // Start timing the search
        let start_time = std::time::Instant::now();

        // Use try_read to avoid blocking when a write lock is held during indexing.
        // Blocking here would cause threads to pile up and exhaust the thread pool.
        let engine = engine.try_read().map_err(|e| match e {
            std::sync::TryLockError::WouldBlock => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Index is currently being updated, please try again shortly".to_string(),
            ),
            std::sync::TryLockError::Poisoned(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to acquire engine read lock: {}", e),
            ),
        })?;

        // Choose search method based on flags
        let (matches, ranking_info) = if symbols_only {
            // Search only in discovered symbols
            let m = engine
                .search_symbols(&query, &include_patterns, &exclude_patterns, max_results)
                .map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        format!("Invalid filter pattern: {}", e),
                    )
                })?;
            (m, None)
        } else if is_regex {
            // Use regex search with optional path filtering
            let m = engine
                .search_regex(&query, &include_patterns, &exclude_patterns, max_results)
                .map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        format!("Invalid regex pattern: {}", e),
                    )
                })?;
            (m, None)
        } else if include_patterns.is_empty() && exclude_patterns.is_empty() {
            // Plain text search with ranking
            let (m, info) = engine.search_ranked(&query, max_results, rank_mode);
            (m, Some(info))
        } else {
            // Plain text search with path filtering and ranking
            let (m, info) = engine
                .search_with_filter_ranked(
                    &query,
                    &include_patterns,
                    &exclude_patterns,
                    max_results,
                    rank_mode,
                )
                .map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        format!("Invalid filter pattern: {}", e),
                    )
                })?;
            (m, Some(info))
        };

        let results: Vec<SearchResultJson> = matches
            .into_iter()
            .map(|m| SearchResultJson {
                file_path: m.file_path,
                content: m.content,
                line_number: m.line_number,
                match_start: m.match_start,
                match_end: m.match_end,
                content_truncated: m.content_truncated,
                score: m.score,
                match_type: if m.is_symbol {
                    "SYMBOL_DEFINITION"
                } else {
                    "TEXT"
                },
                dependency_count: m.dependency_count,
            })
            .collect();

        let total_results = results.len();
        let elapsed_ms = start_time.elapsed().as_secs_f64() * 1000.0;

        Ok(Json(SearchResponse {
            results,
            query,
            total_results,
            elapsed_ms,
            rank_mode: ranking_info
                .as_ref()
                .map(|r| format!("{:?}", r.mode).to_lowercase()),
            total_candidates: ranking_info.as_ref().map(|r| r.total_candidates),
            candidates_searched: ranking_info.as_ref().map(|r| r.candidates_searched),
        }))
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task join error: {}", e),
        )
    })?
}

/// Handle stats requests
pub async fn stats_handler(
    State(state): State<WebState>,
) -> Result<Json<StatsResponse>, (StatusCode, String)> {
    let engine = state.engine.clone();
    tokio::task::spawn_blocking(move || {
        let engine = engine.try_read().map_err(|e| match e {
            std::sync::TryLockError::WouldBlock => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Index is currently being updated, please try again shortly".to_string(),
            ),
            std::sync::TryLockError::Poisoned(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to acquire engine read lock: {}", e),
            ),
        })?;

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
}

/// Handle health check requests
pub async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// Handle indexing status requests
pub async fn status_handler(
    State(state): State<WebState>,
) -> Result<Json<StatusResponse>, (StatusCode, String)> {
    let progress = state.progress.read().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to acquire progress read lock: {}", e),
        )
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
    let (num_files, total_size, num_trigrams, dependency_edges, total_content_bytes) = {
        match state.engine.try_read() {
            Ok(engine) => {
                let stats = engine.get_stats();
                (
                    stats.num_files,
                    stats.total_size,
                    stats.num_trigrams,
                    stats.dependency_edges,
                    stats.total_content_bytes,
                )
            }
            Err(_) => (0, 0, 0, 0, 0),
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

/// Get files that depend on (import) the specified file
pub async fn dependents_handler(
    State(state): State<WebState>,
    Query(params): Query<DependencyQuery>,
) -> Result<Json<DependencyResponse>, (StatusCode, String)> {
    let engine = state.engine.clone();
    tokio::task::spawn_blocking(move || {
        let engine = engine.try_read().map_err(|e| match e {
            std::sync::TryLockError::WouldBlock => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Index is currently being updated, please try again shortly".to_string(),
            ),
            std::sync::TryLockError::Poisoned(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to acquire engine read lock: {}", e),
            ),
        })?;

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
}

/// Get files that the specified file depends on (imports)
pub async fn dependencies_handler(
    State(state): State<WebState>,
    Query(params): Query<DependencyQuery>,
) -> Result<Json<DependencyResponse>, (StatusCode, String)> {
    let engine = state.engine.clone();
    tokio::task::spawn_blocking(move || {
        let engine = engine.try_read().map_err(|e| match e {
            std::sync::TryLockError::WouldBlock => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Index is currently being updated, please try again shortly".to_string(),
            ),
            std::sync::TryLockError::Poisoned(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to acquire engine read lock: {}", e),
            ),
        })?;

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
}

/// WebSocket upgrade handler for progress streaming
pub async fn ws_progress_handler(
    State(state): State<WebState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_progress_socket(socket, state))
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
            }
        })
        .unwrap_or_default()
}

/// Handle a WebSocket connection for progress updates
async fn handle_progress_socket(socket: WebSocket, state: WebState) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to progress broadcast channel
    let mut rx = state.progress_tx.subscribe();

    // Clone engine for use in spawned task
    let engine = state.engine.clone();

    // Send initial progress state immediately (clone before await to avoid holding lock)
    let initial_json = {
        let stats = get_stats_from_engine(&state.engine);
        state.progress.read().ok().and_then(|progress| {
            let status_response = progress_to_status(&progress, stats);
            serde_json::to_string(&status_response).ok()
        })
    };
    if let Some(json) = initial_json {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    // Spawn a task to forward broadcast messages to the WebSocket
    let send_task = tokio::spawn(async move {
        loop {
            match rx.recv().await {
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
    }
}

/// Handle diagnostics requests with self-tests
pub async fn diagnostics_handler(
    State(state): State<WebState>,
    Query(params): Query<DiagnosticsQuery>,
) -> Result<Json<KeywordDiagnosticsResponse>, (StatusCode, String)> {
    let sample_count = params.sample_count.clamp(1, 20);
    let engine = state.engine.clone();

    tokio::task::spawn_blocking(move || {
        // Use try_read to avoid blocking when a write lock is held during indexing.
        let engine = engine.try_read().map_err(|e| match e {
            std::sync::TryLockError::WouldBlock => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Index is currently being updated, please try again shortly".to_string(),
            ),
            std::sync::TryLockError::Poisoned(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to acquire engine read lock: {}", e),
            ),
        })?;

        // Get basic stats
        let stats = engine.get_stats();

        // Build extension breakdown
        let mut ext_map: HashMap<String, (usize, u64)> = HashMap::new();
        let mut all_file_paths: Vec<(u32, String)> = Vec::new();

        for file_id in 0..engine.file_store.len() as u32 {
            if let Some(mapped_file) = engine.file_store.get(file_id) {
                let path_str = mapped_file.path.to_string_lossy().to_string();
                all_file_paths.push((file_id, path_str.clone()));

                let ext = mapped_file
                    .path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("(none)")
                    .to_lowercase();

                let entry = ext_map.entry(ext).or_insert((0, 0));
                entry.0 += 1;
                // Use len_if_mapped() to avoid triggering lazy loading during diagnostics
                entry.1 += mapped_file.len_if_mapped().unwrap_or(0) as u64;
            }
        }

        // Convert to sorted extension breakdown
        let mut files_by_extension: Vec<ExtensionBreakdown> = ext_map
            .into_iter()
            .map(|(ext, (count, bytes))| ExtensionBreakdown {
                extension: ext,
                count,
                total_bytes: bytes,
            })
            .collect();
        files_by_extension.sort_by(|a, b| b.count.cmp(&a.count));
        files_by_extension.truncate(20); // Top 20 extensions

        // Sample random files for display
        let mut rng = rand::rng();
        let sample_count_actual = sample_count.min(all_file_paths.len());
        let sampled: Vec<&(u32, String)> = all_file_paths
            .choose_multiple(&mut rng, sample_count_actual)
            .collect();
        let sample_files: Vec<String> = sampled.into_iter().map(|(_, p)| p.clone()).collect();

        // Get config summary from progress state if available (we don't have direct config access here)
        // For now, provide a minimal config summary
        let config = ConfigSummary {
            indexed_paths: vec!["(see server configuration)".to_string()],
            include_extensions: vec![],
            exclude_patterns: vec![],
            max_file_size_bytes: 10 * 1024 * 1024, // default
            index_path: None,
            watch_enabled: false,
        };

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

            // Take first 8 chars or full name if shorter
            let search_term = if file_name.len() > 8 {
                &file_name[..8]
            } else {
                file_name
            };

            let search_results = engine.search(search_term, 100);
            let found = search_results.iter().any(|r| {
                r.file_path.contains(&test_file_path) || test_file_path.contains(&r.file_path)
            });

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
                        // Take a substring to search for (avoid special chars at boundaries)
                        let search_term = sample_line.trim();
                        let search_term_slice = if search_term.len() > 30 {
                            &search_term[..30]
                        } else {
                            search_term
                        };

                        let search_results = engine.search(search_term_slice, 50);
                        let found = search_results.iter().any(|r| {
                            r.file_path.contains(&test_file_path)
                                || test_file_path.contains(&r.file_path)
                        });

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
                                &search_term_slice[..search_term_slice.len().min(20)],
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
        let status = if test_summary.failed == 0 {
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
            index: KeywordIndexDiagnostics {
                num_files: stats.num_files,
                total_size_bytes: stats.total_size,
                total_size_human: diagnostics::format_bytes(stats.total_size),
                num_trigrams: stats.num_trigrams,
                dependency_edges: stats.dependency_edges,
                files_by_extension,
                sample_files,
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
}
