//! REST API handlers for Semantic Code Search

use crate::semantic::SemanticSearchEngine;
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
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::time::Instant;

/// Shared engine state
pub type EngineState = Arc<RwLock<SemanticSearchEngine>>;

/// Semantic indexing progress
#[derive(Debug, Clone, Serialize, Default)]
pub struct SemanticProgress {
    pub status: String,
    pub files_indexed: usize,
    pub chunks_indexed: usize,
    pub current_path: Option<String>,
    pub message: String,
    pub is_indexing: bool,
    pub progress_percent: u8,
    // Stats fields (included to avoid separate HTTP request)
    pub num_files: usize,
    pub num_chunks: usize,
    pub embedding_dim: usize,
    pub cache_size: usize,
}

/// Shared progress state
pub type SharedSemanticProgress = Arc<RwLock<SemanticProgress>>;

/// Broadcast channel for progress updates
pub type SemanticProgressBroadcaster = tokio::sync::broadcast::Sender<SemanticProgress>;

/// Create a new progress broadcaster
pub fn create_semantic_progress_broadcaster() -> SemanticProgressBroadcaster {
    let (tx, _rx) = tokio::sync::broadcast::channel(16);
    tx
}

/// Combined web state
#[derive(Clone)]
pub struct WebState {
    pub engine: EngineState,
    pub progress: SharedSemanticProgress,
    pub progress_tx: SemanticProgressBroadcaster,
}

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
        let mut engine = state.engine.write().unwrap();
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
        let engine = state.engine.read().unwrap();
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

/// Handle status requests (for indexing progress)
pub async fn status_handler(
    State(state): State<WebState>,
) -> Result<Json<SemanticProgress>, (StatusCode, String)> {
    let progress = state.progress.read().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to read progress: {}", e),
        )
    })?;

    Ok(Json(progress.clone()))
}

/// WebSocket upgrade handler for progress streaming
pub async fn ws_progress_handler(
    State(state): State<WebState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_progress_socket(socket, state))
}

/// Helper to enrich progress with current stats from engine
fn enrich_progress_with_stats(
    mut progress: SemanticProgress,
    engine: &EngineState,
) -> SemanticProgress {
    if let Ok(engine) = engine.read() {
        let stats = engine.get_stats();
        progress.num_files = stats.num_files;
        progress.num_chunks = stats.num_chunks;
        progress.embedding_dim = stats.embedding_dim;
        progress.cache_size = stats.cache_size;
    }
    progress
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
        state.progress.read().ok().and_then(|progress| {
            let enriched = enrich_progress_with_stats(progress.clone(), &state.engine);
            serde_json::to_string(&enriched).ok()
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
                    let enriched = enrich_progress_with_stats(progress, &engine);
                    match serde_json::to_string(&enriched) {
                        Ok(json) => {
                            if sender.send(Message::Text(json.into())).await.is_err() {
                                break; // Client disconnected
                            }
                        }
                        Err(_) => continue,
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    // Wait for client to close connection
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Close(_)) => break,
            Err(_) => break,
            _ => {}
        }
    }

    send_task.abort();
}
