//! REST API handlers for Fast Code Search

use super::WebState;
use crate::search::IndexingStatus;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

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
}

/// Index stats response
#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub num_files: usize,
    pub total_size: u64,
    pub num_trigrams: usize,
    pub dependency_edges: usize,
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
    pub current_batch: usize,
    pub total_batches: usize,
    pub current_path: Option<String>,
    pub progress_percent: u8,
    pub elapsed_secs: Option<f64>,
    pub errors: usize,
    pub message: String,
    pub is_indexing: bool,
}

/// Handle search requests
pub async fn search_handler(
    State(state): State<WebState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, (StatusCode, String)> {
    let query = params.q.trim();

    if query.is_empty() {
        return Ok(Json(SearchResponse {
            results: vec![],
            query: String::new(),
            total_results: 0,
            elapsed_ms: 0.0,
        }));
    }

    let max_results = params.max.clamp(1, 1000);
    let include_patterns = params.include.as_str();
    let exclude_patterns = params.exclude.as_str();
    let is_regex = params.regex;
    let symbols_only = params.symbols;

    // Start timing the search
    let start_time = std::time::Instant::now();

    // Use read lock for concurrent search access
    let engine = state.engine.read().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to acquire engine read lock: {}", e),
        )
    })?;

    // Choose search method based on flags
    let matches = if symbols_only {
        // Search only in discovered symbols
        engine
            .search_symbols(query, include_patterns, exclude_patterns, max_results)
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("Invalid filter pattern: {}", e),
                )
            })?
    } else if is_regex {
        // Use regex search with optional path filtering
        engine
            .search_regex(query, include_patterns, exclude_patterns, max_results)
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("Invalid regex pattern: {}", e),
                )
            })?
    } else if include_patterns.is_empty() && exclude_patterns.is_empty() {
        // Plain text search without filtering
        engine.search(query, max_results)
    } else {
        // Plain text search with path filtering
        engine
            .search_with_filter(query, include_patterns, exclude_patterns, max_results)
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("Invalid filter pattern: {}", e),
                )
            })?
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
        query: query.to_string(),
        total_results,
        elapsed_ms,
    }))
}

/// Handle stats requests
pub async fn stats_handler(
    State(state): State<WebState>,
) -> Result<Json<StatsResponse>, (StatusCode, String)> {
    // Use read lock for concurrent access
    let engine = state.engine.read().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to acquire engine read lock: {}", e),
        )
    })?;

    let stats = engine.get_stats();

    Ok(Json(StatsResponse {
        num_files: stats.num_files,
        total_size: stats.total_size,
        num_trigrams: stats.num_trigrams,
        dependency_edges: stats.dependency_edges,
    }))
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

    Ok(Json(StatusResponse {
        status: status_str.to_string(),
        files_discovered: progress.files_discovered,
        files_indexed: progress.files_indexed,
        current_batch: progress.current_batch,
        total_batches: progress.total_batches,
        current_path: progress.current_path.clone(),
        progress_percent: progress.progress_percent(),
        elapsed_secs: progress.elapsed_secs(),
        errors: progress.errors,
        message: progress.message.clone(),
        is_indexing,
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
    // Use read lock for concurrent access
    let engine = state.engine.read().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to acquire engine read lock: {}", e),
        )
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
}

/// Get files that the specified file depends on (imports)
pub async fn dependencies_handler(
    State(state): State<WebState>,
    Query(params): Query<DependencyQuery>,
) -> Result<Json<DependencyResponse>, (StatusCode, String)> {
    // Use read lock for concurrent access
    let engine = state.engine.read().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to acquire engine read lock: {}", e),
        )
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
}
