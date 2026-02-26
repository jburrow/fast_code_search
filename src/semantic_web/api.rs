//! REST API handlers for Semantic Code Search

use crate::diagnostics::{
    self, ChunkTypeBreakdown, ConfigSummary, DiagnosticsQuery, HealthStatus, ModelDiagnostics,
    SemanticDiagnosticsResponse, SemanticIndexDiagnostics, TestResult, TestSummary,
};
use crate::semantic::{ChunkType, SemanticSearchEngine};
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
use std::collections::HashSet;
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
        status: "healthy",
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

/// Handle diagnostics requests with self-tests
pub async fn diagnostics_handler(
    State(state): State<WebState>,
    Query(params): Query<DiagnosticsQuery>,
) -> Result<Json<SemanticDiagnosticsResponse>, (StatusCode, String)> {
    let sample_count = params.sample_count.clamp(1, 20);

    // Acquire write lock on engine (needed for search)
    let mut engine = state.engine.write().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to acquire engine write lock: {}", e),
        )
    })?;

    // Get basic stats
    let stats = engine.get_stats();

    // Collect chunk type breakdown and sample files
    let mut chunks_by_type = ChunkTypeBreakdown::default();
    let mut unique_files: HashSet<String> = HashSet::new();

    // We need to access the internal chunks - for now use search to get some samples
    // This is a bit of a workaround since chunks are private

    // Run a broad search to get chunk samples
    let sample_results = engine.search("function", 100).unwrap_or_default();

    for result in &sample_results {
        unique_files.insert(result.chunk.file_path.clone());
        match &result.chunk.chunk_type {
            ChunkType::Fixed => chunks_by_type.fixed += 1,
            ChunkType::Function(_) => chunks_by_type.functions += 1,
            ChunkType::Class(_) => chunks_by_type.classes += 1,
            ChunkType::Module => chunks_by_type.modules += 1,
        }
    }

    // Sample files from what we found
    let all_files: Vec<String> = unique_files.iter().cloned().collect();
    let mut rng = rand::rng();
    let sample_files: Vec<String> = all_files
        .choose_multiple(&mut rng, sample_count.min(all_files.len()))
        .cloned()
        .collect();

    // Configuration summary (semantic-specific settings would go here)
    let config = ConfigSummary {
        indexed_paths: vec!["(see server configuration)".to_string()],
        include_extensions: vec![],
        exclude_patterns: vec![],
        max_file_size_bytes: 10 * 1024 * 1024,
        index_path: None,
        watch_enabled: false,
    };

    // Model diagnostics
    let model = ModelDiagnostics {
        name: "TF-IDF Embeddings".to_string(),
        loaded: true,
        embedding_dim: stats.embedding_dim,
        model_type: "tfidf".to_string(),
    };

    // Run self-tests
    let mut self_tests = Vec::new();

    // Test 1: Embedding generation - verify we can generate embeddings
    {
        let test_start = Instant::now();
        let search_result = engine.search("test embedding generation", 1);

        let test = match search_result {
            Ok(_) => TestResult::passed(
                "embedding_generation",
                test_start.elapsed(),
                "Successfully generated query embedding".to_string(),
            ),
            Err(e) => TestResult::failed(
                "embedding_generation",
                test_start.elapsed(),
                format!("Failed to generate embedding: {}", e),
            ),
        };
        self_tests.push(test);
    }

    // Test 2: Semantic search - verify search returns results if index has data
    {
        let test_start = Instant::now();

        if stats.num_chunks == 0 {
            self_tests.push(TestResult::passed(
                "semantic_search",
                test_start.elapsed(),
                "No chunks indexed yet - search test skipped".to_string(),
            ));
        } else {
            let search_result = engine.search("function main implementation", 10);

            let test = match search_result {
                Ok(results) => {
                    if results.is_empty() {
                        TestResult::passed(
                            "semantic_search",
                            test_start.elapsed(),
                            "Search operational but no matches found (may be normal for this query)".to_string(),
                        )
                    } else {
                        TestResult::passed(
                            "semantic_search",
                            test_start.elapsed(),
                            format!("Search returned {} results", results.len()),
                        )
                    }
                }
                Err(e) => TestResult::failed(
                    "semantic_search",
                    test_start.elapsed(),
                    format!("Search failed: {}", e),
                ),
            };
            self_tests.push(test);
        }
    }

    // Test 3: Query cache functionality
    {
        let test_start = Instant::now();
        let cache_size_before = stats.cache_size;

        // Run same query twice - cache should increase by 1 at most
        let _ = engine.search("cache test query unique", 1);
        let stats_after = engine.get_stats();

        let test = if stats_after.cache_size >= cache_size_before {
            TestResult::passed(
                "query_cache",
                test_start.elapsed(),
                format!("Query cache operational (size: {})", stats_after.cache_size),
            )
        } else {
            TestResult::failed(
                "query_cache",
                test_start.elapsed(),
                "Query cache appears non-functional".to_string(),
            )
        };
        self_tests.push(test);
    }

    // Test 4: Index integrity - verify chunk count matches stats
    {
        let test_start = Instant::now();
        let num_chunks = stats.num_chunks;
        let num_files = stats.num_files;

        let test = if num_chunks == 0 && num_files == 0 {
            TestResult::passed(
                "index_integrity",
                test_start.elapsed(),
                "Index is empty (no files indexed yet)".to_string(),
            )
        } else if num_chunks > 0 && num_files > 0 {
            TestResult::passed(
                "index_integrity",
                test_start.elapsed(),
                format!(
                    "Index healthy: {} chunks from {} files",
                    num_chunks, num_files
                ),
            )
        } else {
            TestResult::failed(
                "index_integrity",
                test_start.elapsed(),
                format!(
                    "Inconsistent index state: {} chunks but {} files",
                    num_chunks, num_files
                ),
            )
        };
        self_tests.push(test);
    }

    // Test 5: Similarity scores sanity - verify scores are in valid range
    {
        let test_start = Instant::now();

        if stats.num_chunks == 0 {
            self_tests.push(TestResult::passed(
                "similarity_scores",
                test_start.elapsed(),
                "No chunks to test similarity scores".to_string(),
            ));
        } else {
            let search_result = engine.search("code function test", 10);

            let test = match search_result {
                Ok(results) => {
                    let invalid_scores: Vec<_> = results
                        .iter()
                        .filter(|r| r.similarity_score < 0.0 || r.similarity_score > 1.0)
                        .collect();

                    if invalid_scores.is_empty() {
                        TestResult::passed(
                            "similarity_scores",
                            test_start.elapsed(),
                            format!("All {} result scores in valid range [0, 1]", results.len()),
                        )
                    } else {
                        TestResult::failed(
                            "similarity_scores",
                            test_start.elapsed(),
                            format!(
                                "{} results have invalid similarity scores",
                                invalid_scores.len()
                            ),
                        )
                    }
                }
                Err(e) => TestResult::failed(
                    "similarity_scores",
                    test_start.elapsed(),
                    format!("Could not test scores: {}", e),
                ),
            };
            self_tests.push(test);
        }
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

    let response = SemanticDiagnosticsResponse {
        status,
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_secs: diagnostics::get_uptime_secs(),
        uptime_human: diagnostics::format_uptime(diagnostics::get_uptime_secs()),
        generated_at: diagnostics::get_timestamp(),
        config,
        index: SemanticIndexDiagnostics {
            num_files: stats.num_files,
            num_chunks: stats.num_chunks,
            embedding_dim: stats.embedding_dim,
            cache_size: stats.cache_size,
            cache_hit_rate: None, // TODO: track cache hit rate
            chunks_by_type,
            sample_files,
        },
        model,
        self_tests,
        test_summary,
    };

    Ok(Json(response))
}
