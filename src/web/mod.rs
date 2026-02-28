//! Web UI and REST API module for Fast Code Search

mod api;

use crate::search::{ProgressBroadcaster, SearchEngine, SharedIndexingProgress};
use axum::{
    body::Body,
    extract::State,
    http::{header, Response, StatusCode},
    routing::get,
    Router,
};
use rust_embed::RustEmbed;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

#[derive(RustEmbed)]
#[folder = "static/"]
struct StaticAssets;

/// Shared application state - RwLock allows concurrent read access for searches
pub type AppState = Arc<RwLock<SearchEngine>>;

/// Combined state for handlers that need both engine and progress
#[derive(Clone)]
pub struct WebState {
    pub engine: AppState,
    pub progress: SharedIndexingProgress,
    pub progress_tx: ProgressBroadcaster,
    /// When set, static files are served from this directory on disk instead of
    /// the embedded assets.  Intended for development use only.
    pub static_dir: Option<PathBuf>,
}

/// Create the web router with all routes
pub fn create_router(
    engine: AppState,
    progress: SharedIndexingProgress,
    progress_tx: ProgressBroadcaster,
    static_dir: Option<PathBuf>,
) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let state = WebState {
        engine,
        progress,
        progress_tx,
        static_dir,
    };

    Router::new()
        // API routes
        .route("/api/search", get(api::search_handler))
        .route("/api/stats", get(api::stats_handler))
        .route("/api/status", get(api::status_handler))
        .route("/api/health", get(api::health_handler))
        .route("/api/diagnostics", get(api::diagnostics_handler))
        .route("/api/dependents", get(api::dependents_handler))
        .route("/api/dependencies", get(api::dependencies_handler))
        .route("/api/file", get(api::file_handler))
        // WebSocket for progress streaming
        .route("/ws/progress", get(api::ws_progress_handler))
        // Static files
        .route("/", get(index_handler))
        .route("/{*file}", get(static_handler))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Serve index.html
async fn index_handler(State(state): State<WebState>) -> Response<Body> {
    serve_static_file("index.html", state.static_dir.as_deref())
}

/// Serve static files from embedded assets or from disk
async fn static_handler(
    State(state): State<WebState>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> Response<Body> {
    serve_static_file(&path, state.static_dir.as_deref())
}

fn serve_static_file(path: &str, static_dir: Option<&std::path::Path>) -> Response<Body> {
    // Remove leading slash if present
    let path = path.trim_start_matches('/');

    // If a static directory is configured, serve directly from disk so that
    // UI changes are picked up without recompiling the server.
    if let Some(dir) = static_dir {
        // Reject paths with directory traversal components before hitting the
        // filesystem.  This is intentionally conservative: legitimate static
        // asset paths never contain "..".
        if path.contains("..") {
            return Response::builder()
                .status(StatusCode::FORBIDDEN)
                .body(Body::from("Forbidden"))
                .unwrap();
        }

        let file_path = dir.join(path);
        match std::fs::read(&file_path) {
            Ok(data) => {
                let mime = mime_guess::from_path(path).first_or_octet_stream();
                return Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, mime.as_ref())
                    .header(header::CACHE_CONTROL, "no-cache")
                    .body(Body::from(data))
                    .unwrap();
            }
            Err(e) => {
                tracing::warn!(
                    path = %file_path.display(),
                    error = %e,
                    "Failed to read static file from disk"
                );
                return Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::from("Not Found"))
                    .unwrap();
            }
        }
    }

    match StaticAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();

            // Use ETag based on content hash for proper cache invalidation
            let etag = format!("\"{:x}\"", md5::compute(&content.data));

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .header(
                    header::CACHE_CONTROL,
                    "public, max-age=3600, must-revalidate",
                )
                .header(header::ETAG, etag)
                .body(Body::from(content.data.to_vec()))
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Not Found"))
            .unwrap(),
    }
}
