//! REST API handlers for Semantic Code Search

use crate::semantic::SemanticSearchEngine;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::time::Instant;

pub type WebState = Arc<RwLock<SemanticSearchEngine>>;

/// Search query parameters
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// The search query string
    q: String,
    /// Maximum number of results (default: 20)
    #[serde(default = "default_max_results")]
    max: usize,
}

fn default_max_results() -> usize {
    20
}

/// Search result for JSON
#[derive(Debug, Serialize)]
pub struct SearchResultJson {
    pub file_path: String,
    pub content: String,
    pub start_line: usize,
    pub end_line: usize,
    pub similarity_score: f32,
    pub chunk_type: String,
    pub symbol_name: Option<String>,
}

/// Search response
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResultJson>,
    pub query: String,
    pub total_results: usize,
    pub elapsed_ms: f64,
}

/// Stats response
#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub num_files: usize,
    pub num_chunks: usize,
    pub embedding_dim: usize,
    pub cache_size: usize,
}

/// Health response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}

/// Handle search requests
pub async fn search_handler(
    State(state): State<WebState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, (StatusCode, String)> {
    let start = Instant::now();

    let results = {
        let mut engine = state.write().unwrap();
        engine
            .search(&params.q, params.max)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    let json_results: Vec<SearchResultJson> = results
        .into_iter()
        .map(|r| {
            let (chunk_type, symbol_name) = match &r.chunk.chunk_type {
                crate::semantic::ChunkType::Fixed => ("fixed".to_string(), None),
                crate::semantic::ChunkType::Function(name) => {
                    ("function".to_string(), Some(name.clone()))
                }
                crate::semantic::ChunkType::Class(name) => {
                    ("class".to_string(), Some(name.clone()))
                }
                crate::semantic::ChunkType::Module => ("module".to_string(), None),
            };

            SearchResultJson {
                file_path: r.chunk.file_path,
                content: r.chunk.text,
                start_line: r.chunk.start_line,
                end_line: r.chunk.end_line,
                similarity_score: r.similarity_score,
                chunk_type,
                symbol_name,
            }
        })
        .collect();

    let response = SearchResponse {
        total_results: json_results.len(),
        results: json_results,
        query: params.q,
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
    };

    Ok(Json(response))
}

/// Handle stats requests
pub async fn stats_handler(
    State(state): State<WebState>,
) -> Result<Json<StatsResponse>, (StatusCode, String)> {
    let stats = {
        let engine = state.read().unwrap();
        engine.get_stats()
    };

    Ok(Json(StatsResponse {
        num_files: stats.num_files,
        num_chunks: stats.num_chunks,
        embedding_dim: stats.embedding_dim,
        cache_size: stats.cache_size,
    }))
}

/// Handle health check requests
pub async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}
