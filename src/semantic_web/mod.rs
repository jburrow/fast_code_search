//! Web server and REST API for semantic search

pub mod api;

use api::{health_handler, search_handler, stats_handler, WebState};
use axum::{
    routing::get,
    Router,
};
use tower_http::cors::CorsLayer;

/// Create the router for the semantic web server
pub fn create_router(state: WebState) -> Router {
    Router::new()
        .route("/api/search", get(search_handler))
        .route("/api/stats", get(stats_handler))
        .route("/api/health", get(health_handler))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
