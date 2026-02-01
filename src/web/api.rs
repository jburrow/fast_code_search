//! REST API handlers for Fast Code Search

use super::AppState;
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
    pub score: f64,
    pub match_type: &'static str,
}

/// Search response
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResultJson>,
    pub query: String,
    pub total_results: usize,
}

/// Index stats response
#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub num_files: usize,
    pub total_size: u64,
    pub num_trigrams: usize,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}

/// Handle search requests
pub async fn search_handler(
    State(engine): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, (StatusCode, String)> {
    let query = params.q.trim();

    if query.is_empty() {
        return Ok(Json(SearchResponse {
            results: vec![],
            query: String::new(),
            total_results: 0,
        }));
    }

    let max_results = params.max.min(1000).max(1);

    let engine = engine.lock().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to acquire engine lock: {}", e),
        )
    })?;

    let matches = engine.search(query, max_results);

    let results: Vec<SearchResultJson> = matches
        .into_iter()
        .map(|m| SearchResultJson {
            file_path: m.file_path,
            content: m.content,
            line_number: m.line_number,
            score: m.score,
            match_type: if m.is_symbol {
                "SYMBOL_DEFINITION"
            } else {
                "TEXT"
            },
        })
        .collect();

    let total_results = results.len();

    Ok(Json(SearchResponse {
        results,
        query: query.to_string(),
        total_results,
    }))
}

/// Handle stats requests
pub async fn stats_handler(
    State(engine): State<AppState>,
) -> Result<Json<StatsResponse>, (StatusCode, String)> {
    let engine = engine.lock().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to acquire engine lock: {}", e),
        )
    })?;

    let stats = engine.get_stats();

    Ok(Json(StatsResponse {
        num_files: stats.num_files,
        total_size: stats.total_size,
        num_trigrams: stats.num_trigrams,
    }))
}

/// Handle health check requests
pub async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy",
        version: env!("CARGO_PKG_VERSION"),
    })
}
