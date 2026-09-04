//! Web UI and REST API module for Fast Code Search

mod api;
pub mod metrics;

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
    /// Bounds concurrent searches (each holds a blocking-pool thread).
    pub search_permits: Arc<tokio::sync::Semaphore>,
    /// Request counters and latency histogram for `/metrics`.
    pub metrics: Arc<metrics::Metrics>,
    /// The indexer configuration, for `/api/diagnostics` (None in embedded /
    /// test routers).
    pub indexer_config: Option<Arc<crate::config::IndexerConfig>>,
    /// Extension breakdown cached per engine generation.
    pub diagnostics_cache: Arc<std::sync::Mutex<Option<api::DiagnosticsCache>>>,
}

/// Knobs for [`create_router_with_options`].
#[derive(Debug, Clone)]
pub struct RouterOptions {
    /// CORS origins (`"*"` = any; empty = same-origin only).
    pub cors_origins: Vec<String>,
    /// Searches allowed to execute at once; further requests get 503.
    pub max_concurrent_searches: usize,
    /// Whole-request timeout.
    pub request_timeout: std::time::Duration,
    /// Maximum request body size in bytes (the API is GET-only; this just
    /// closes the door on oversized bodies).
    pub body_limit: usize,
    /// Indexer configuration to report on `/api/diagnostics`.
    pub indexer_config: Option<crate::config::IndexerConfig>,
}

impl Default for RouterOptions {
    fn default() -> Self {
        Self {
            cors_origins: Vec::new(),
            max_concurrent_searches: 64,
            request_timeout: std::time::Duration::from_secs(30),
            body_limit: 64 * 1024,
            indexer_config: None,
        }
    }
}

impl From<&crate::config::ServerConfig> for RouterOptions {
    fn from(c: &crate::config::ServerConfig) -> Self {
        Self {
            cors_origins: c.cors_origins.clone(),
            max_concurrent_searches: c.max_concurrent_searches,
            request_timeout: std::time::Duration::from_secs(c.request_timeout_secs.max(1)),
            ..Default::default()
        }
    }
}

/// Create the web router with all routes and no cross-origin access
/// (same-origin only, which is all the embedded UI needs).
pub fn create_router(
    engine: AppState,
    progress: SharedIndexingProgress,
    progress_tx: ProgressBroadcaster,
    static_dir: Option<PathBuf>,
) -> Router {
    create_router_with_cors(engine, progress, progress_tx, static_dir, &[])
}

/// Create the web router, allowing cross-origin API access from
/// `cors_origins` (`"*"` = any origin; empty = same-origin only).
pub fn create_router_with_cors(
    engine: AppState,
    progress: SharedIndexingProgress,
    progress_tx: ProgressBroadcaster,
    static_dir: Option<PathBuf>,
    cors_origins: &[String],
) -> Router {
    let opts = RouterOptions {
        cors_origins: cors_origins.to_vec(),
        ..Default::default()
    };
    create_router_with_options(engine, progress, progress_tx, static_dir, &opts)
}

/// Create the web router with explicit limits (see [`RouterOptions`]).
pub fn create_router_with_options(
    engine: AppState,
    progress: SharedIndexingProgress,
    progress_tx: ProgressBroadcaster,
    static_dir: Option<PathBuf>,
    opts: &RouterOptions,
) -> Router {
    let cors = build_cors_layer(&opts.cors_origins);

    let state = WebState {
        engine,
        progress,
        progress_tx,
        static_dir,
        search_permits: Arc::new(tokio::sync::Semaphore::new(opts.max_concurrent_searches)),
        metrics: Arc::new(metrics::Metrics::new()),
        indexer_config: opts.indexer_config.clone().map(Arc::new),
        diagnostics_cache: Arc::new(std::sync::Mutex::new(None)),
    };

    let router = Router::new()
        // API routes
        .route("/api/search", get(api::search_handler))
        .route("/api/stats", get(api::stats_handler))
        .route("/api/status", get(api::status_handler))
        .route("/api/health", get(api::health_handler))
        .route("/api/ready", get(api::ready_handler))
        .route("/metrics", get(api::metrics_handler))
        .route("/api/diagnostics", get(api::diagnostics_handler))
        .route("/api/dependents", get(api::dependents_handler))
        .route("/api/dependencies", get(api::dependencies_handler))
        .route("/api/file", get(api::file_handler))
        .route("/api/context", get(api::context_handler))
        // WebSocket for progress streaming
        .route("/ws/progress", get(api::ws_progress_handler))
        // Static files
        .route("/", get(index_handler))
        .route("/{*file}", get(static_handler));
    let router = match cors {
        Some(cors) => router.layer(cors),
        None => router,
    };
    router
        .layer(tower_http::limit::RequestBodyLimitLayer::new(
            opts.body_limit,
        ))
        .layer(tower_http::timeout::TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            opts.request_timeout,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Translate the configured origin list into a CORS layer. Invalid origin
/// strings are logged and skipped; an empty (or all-invalid) list yields no
/// layer at all, i.e. browsers refuse cross-origin reads.
fn build_cors_layer(cors_origins: &[String]) -> Option<CorsLayer> {
    if cors_origins.is_empty() {
        return None;
    }
    if cors_origins.iter().any(|o| o == "*") {
        tracing::warn!("CORS: allowing any origin to read the API (cors_origins = [\"*\"])");
        return Some(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );
    }
    let origins: Vec<header::HeaderValue> = cors_origins
        .iter()
        .filter_map(|o| match header::HeaderValue::from_str(o) {
            Ok(v) => Some(v),
            Err(_) => {
                tracing::warn!(origin = %o, "CORS: ignoring invalid origin");
                None
            }
        })
        .collect();
    if origins.is_empty() {
        return None;
    }
    tracing::info!(origins = ?cors_origins, "CORS: allowing listed origins");
    Some(
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods(Any)
            .allow_headers(Any),
    )
}

/// Serve index.html
async fn index_handler(
    State(state): State<WebState>,
    headers: header::HeaderMap,
) -> Response<Body> {
    serve_static_file("index.html", state.static_dir.as_deref(), &headers)
}

/// Serve static files from embedded assets or from disk
async fn static_handler(
    State(state): State<WebState>,
    axum::extract::Path(path): axum::extract::Path<String>,
    headers: header::HeaderMap,
) -> Response<Body> {
    serve_static_file(&path, state.static_dir.as_deref(), &headers)
}

fn serve_static_file(
    path: &str,
    static_dir: Option<&std::path::Path>,
    req_headers: &header::HeaderMap,
) -> Response<Body> {
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

            // ETag uses rust_embed's COMPILE-TIME sha256 hash — no per-request
            // hashing (the previous code hashed every asset, incl. the multi-MB
            // font, on every request).
            let etag = format!("\"{}\"", hex_encode(&content.metadata.sha256_hash()));

            // Honor conditional requests: return 304 when the client's cached
            // ETag matches, so unchanged assets aren't resent.
            if let Some(inm) = req_headers.get(header::IF_NONE_MATCH) {
                if inm.to_str().map(|v| v == etag).unwrap_or(false) {
                    return Response::builder()
                        .status(StatusCode::NOT_MODIFIED)
                        .header(header::ETAG, etag)
                        .header(
                            header::CACHE_CONTROL,
                            "public, max-age=3600, must-revalidate",
                        )
                        .body(Body::empty())
                        .unwrap();
                }
            }

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

/// Lowercase hex-encode bytes (for ETag values).
fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{:02x}", b);
    }
    s
}
