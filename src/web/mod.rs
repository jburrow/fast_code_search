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
    /// Whole-request timeout; searches get an engine deadline just under it
    /// so an abandoned request stops scanning on its own.
    pub request_timeout: std::time::Duration,
    /// Request counters and latency histogram for `/metrics`.
    pub metrics: Arc<metrics::Metrics>,
    /// The indexer configuration, for `/api/diagnostics` (None in embedded /
    /// test routers).
    pub indexer_config: Option<Arc<crate::config::IndexerConfig>>,
    /// Extension breakdown cached per engine generation.
    pub diagnostics_cache: Arc<std::sync::Mutex<Option<api::DiagnosticsCache>>>,
    /// Last file count observed by `/api/ready`, used while the engine's
    /// write lock is briefly held.
    pub last_known_files: Arc<std::sync::atomic::AtomicUsize>,
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
    /// Semaphore bounding concurrent searches. Pass the same one to the gRPC
    /// service so both surfaces share `max_concurrent_searches`; `None`
    /// creates a private one sized from `max_concurrent_searches`.
    pub search_permits: Option<Arc<tokio::sync::Semaphore>>,
}

impl Default for RouterOptions {
    fn default() -> Self {
        Self {
            cors_origins: Vec::new(),
            max_concurrent_searches: 64,
            request_timeout: std::time::Duration::from_secs(30),
            body_limit: 64 * 1024,
            indexer_config: None,
            search_permits: None,
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
        search_permits: opts
            .search_permits
            .clone()
            .unwrap_or_else(|| Arc::new(tokio::sync::Semaphore::new(opts.max_concurrent_searches))),
        request_timeout: opts.request_timeout,
        metrics: Arc::new(metrics::Metrics::new()),
        indexer_config: opts.indexer_config.clone().map(Arc::new),
        diagnostics_cache: Arc::new(std::sync::Mutex::new(None)),
        last_known_files: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
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
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            request_timeout_middleware,
        ))
        .layer(axum::middleware::from_fn(security_headers_middleware))
        // 5xx responses are logged at debug rather than error: a readiness
        // probe answering 503 while the index builds is expected, and the
        // handlers already log genuine failures.
        .layer(
            TraceLayer::new_for_http().on_failure(
                tower_http::trace::DefaultOnFailure::new().level(tracing::Level::DEBUG),
            ),
        )
        .with_state(state)
}

/// Cache policy for an embedded asset. The HTML, scripts and stylesheets
/// are served under unversioned URLs and fetched independently, so they
/// must revalidate on every load (the ETag makes that a cheap 304) or a
/// client can run a new page against an hour-old script after an upgrade.
/// Fonts and images may be held for an hour.
fn cache_policy(mime: &str) -> &'static str {
    if mime.starts_with("text/")
        || mime.starts_with("application/javascript")
        || mime.starts_with("application/json")
    {
        "no-cache"
    } else {
        "public, max-age=3600, must-revalidate"
    }
}

/// Conservative security headers on every response: the UI renders
/// arbitrary indexed file content, so never let a browser sniff a
/// content type or frame the app.
async fn security_headers_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut resp = next.run(req).await;
    let headers = resp.headers_mut();
    headers
        .entry(header::X_CONTENT_TYPE_OPTIONS)
        .or_insert(axum::http::HeaderValue::from_static("nosniff"));
    headers
        .entry(header::X_FRAME_OPTIONS)
        .or_insert(axum::http::HeaderValue::from_static("DENY"));
    headers
        .entry(header::REFERRER_POLICY)
        .or_insert(axum::http::HeaderValue::from_static("same-origin"));
    resp
}

/// Whole-request timeout that answers with the API's JSON error envelope
/// (504 + `Retry-After`) instead of an empty 408. Search handlers also give
/// the engine a deadline just under this, so the scan stops on its own when
/// the response is abandoned here.
async fn request_timeout_middleware(
    axum::extract::State(state): axum::extract::State<WebState>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    match tokio::time::timeout(state.request_timeout, next.run(req)).await {
        Ok(resp) => resp,
        Err(_) => {
            let mut resp = (
                axum::http::StatusCode::GATEWAY_TIMEOUT,
                axum::Json(api::ErrorResponse {
                    error: format!(
                        "Request timed out after {} s; narrow the query or pass a smaller timeout_ms",
                        state.request_timeout.as_secs()
                    ),
                }),
            )
                .into_response();
            resp.headers_mut().insert(
                axum::http::header::RETRY_AFTER,
                axum::http::HeaderValue::from_static("1"),
            );
            resp
        }
    }
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
            let cache_control = cache_policy(mime.as_ref());
            if let Some(inm) = req_headers.get(header::IF_NONE_MATCH) {
                if inm.to_str().map(|v| v == etag).unwrap_or(false) {
                    return Response::builder()
                        .status(StatusCode::NOT_MODIFIED)
                        .header(header::ETAG, etag)
                        .header(header::CACHE_CONTROL, cache_control)
                        .body(Body::empty())
                        .unwrap();
                }
            }

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .header(header::CACHE_CONTROL, cache_control)
                .header(header::ETAG, etag)
                // `data` is a `Cow<'static, [u8]>`: no copy of the (multi-MB
                // font) asset per request.
                .body(Body::from(content.data))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole-request timeout answers with the JSON error envelope and a
    /// `Retry-After` header (not an empty 408), so clients can treat it like
    /// any other API error. Uses a deliberately slow route so the outcome
    /// does not depend on how fast a real search happens to be.
    #[tokio::test]
    async fn request_timeout_answers_with_json_504() {
        let state = WebState {
            engine: Arc::new(RwLock::new(SearchEngine::new())),
            progress: Arc::new(RwLock::new(crate::search::IndexingProgress::default())),
            progress_tx: crate::search::create_progress_broadcaster(),
            static_dir: None,
            search_permits: Arc::new(tokio::sync::Semaphore::new(1)),
            request_timeout: std::time::Duration::from_millis(20),
            metrics: Arc::new(metrics::Metrics::new()),
            indexer_config: None,
            diagnostics_cache: Arc::new(std::sync::Mutex::new(None)),
            last_known_files: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        };
        let router = Router::new()
            .route(
                "/slow",
                get(|| async {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    "done"
                }),
            )
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                request_timeout_middleware,
            ))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });

        let resp = reqwest::get(format!("http://{addr}/slow")).await.unwrap();
        assert_eq!(resp.status(), 504);
        assert_eq!(
            resp.headers()
                .get(header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok()),
            Some("1")
        );
        let body: serde_json::Value = resp.json().await.unwrap();
        assert!(
            body["error"].as_str().unwrap_or("").contains("timed out"),
            "{body}"
        );
    }

    #[test]
    fn cache_policy_revalidates_code_and_holds_fonts() {
        assert_eq!(cache_policy("text/html"), "no-cache");
        assert_eq!(cache_policy("application/javascript"), "no-cache");
        assert_eq!(cache_policy("text/css; charset=utf-8"), "no-cache");
        assert_eq!(
            cache_policy("font/woff2"),
            "public, max-age=3600, must-revalidate"
        );
    }
}
