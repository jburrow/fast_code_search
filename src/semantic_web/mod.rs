//! Web server and REST API for semantic search

pub mod api;

pub use api::{
    create_semantic_progress_broadcaster, EngineState, SemanticProgress,
    SemanticProgressBroadcaster, SharedSemanticProgress, WebState,
};

use api::{
    diagnostics_handler, health_handler, search_handler, stats_handler, status_handler,
    ws_progress_handler,
};
use axum::{
    body::Body,
    http::{header, Response, StatusCode},
    routing::get,
    Router,
};
use rust_embed::RustEmbed;
use tower_http::cors::CorsLayer;

#[derive(RustEmbed)]
#[folder = "static/"]
struct StaticAssets;

/// Create the router for the semantic web server
pub fn create_router(state: WebState) -> Router {
    Router::new()
        .route("/api/search", get(search_handler))
        .route("/api/stats", get(stats_handler))
        .route("/api/status", get(status_handler))
        .route("/api/health", get(health_handler))
        .route("/api/diagnostics", get(diagnostics_handler))
        // WebSocket for progress streaming
        .route("/ws/progress", get(ws_progress_handler))
        // Static files
        .route("/", get(index_handler))
        .route("/{*file}", get(static_handler))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Serve semantic.html as the index
async fn index_handler() -> Response<Body> {
    serve_static_file("semantic.html")
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
