//! Web UI and REST API module for Fast Code Search

mod api;

use crate::search::{SearchEngine, SharedIndexingProgress};
use axum::{
    body::Body,
    http::{header, Response, StatusCode},
    routing::get,
    Router,
};
use rust_embed::RustEmbed;
use std::sync::{Arc, RwLock};
use tower_http::cors::{Any, CorsLayer};

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
}

/// Create the web router with all routes
pub fn create_router(engine: AppState, progress: SharedIndexingProgress) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let state = WebState { engine, progress };

    Router::new()
        // API routes
        .route("/api/search", get(api::search_handler))
        .route("/api/stats", get(api::stats_handler))
        .route("/api/status", get(api::status_handler))
        .route("/api/health", get(api::health_handler))
        .route("/api/dependents", get(api::dependents_handler))
        .route("/api/dependencies", get(api::dependencies_handler))
        // Static files
        .route("/", get(index_handler))
        .route("/{*file}", get(static_handler))
        .layer(cors)
        .with_state(state)
}

/// Serve index.html
async fn index_handler() -> Response<Body> {
    serve_static_file("index.html")
}

/// Serve static files from embedded assets
async fn static_handler(axum::extract::Path(path): axum::extract::Path<String>) -> Response<Body> {
    serve_static_file(&path)
}

fn serve_static_file(path: &str) -> Response<Body> {
    // Remove leading slash if present
    let path = path.trim_start_matches('/');

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
