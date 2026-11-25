//! Single-port mode (`server.single_port`): the gRPC API served on the web
//! port, next to the web UI, REST API and MCP.
//!
//! Requests are told apart by `content-type`: gRPC always sends
//! `application/grpc` (optionally `+proto` or parameters), which no browser,
//! REST or MCP client sends. Everything else goes to the web router
//! unchanged, with its own layers (timeouts, body limit, CORS, headers).

use axum::{extract::Request, http::header, response::Response, Router};
use std::time::Duration;
use tower::ServiceExt;

/// Whether `req` is a gRPC call (`application/grpc`, `application/grpc+proto`,
/// `application/grpc;...`). gRPC-Web (`application/grpc-web`) is not: the
/// gRPC server does not speak it on its own port either.
pub fn is_grpc_request<B>(req: &axum::http::Request<B>) -> bool {
    let Some(ct) = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    match ct.strip_prefix("application/grpc") {
        Some(rest) => rest.is_empty() || rest.starts_with('+') || rest.starts_with(';'),
        None => false,
    }
}

/// One router that sends gRPC calls to `grpc` and everything else to `web`.
///
/// `request_timeout` stands in for the gRPC server's own timeout (which only
/// applies on the separate port): a call that has not answered in time gets
/// the same `CANCELLED: Timeout expired` status.
pub fn with_grpc(web: Router, grpc: tonic::service::Routes, request_timeout: Duration) -> Router {
    let grpc = grpc
        .prepare()
        .into_axum_router()
        .layer(axum::middleware::from_fn_with_state(
            request_timeout,
            grpc_timeout,
        ))
        .layer(
            tower_http::trace::TraceLayer::new_for_grpc().make_span_with(
                |req: &Request| tracing::info_span!("grpc", path = %req.uri().path()),
            ),
        );
    Router::new().fallback_service(tower::service_fn(move |req: Request| {
        let target = if is_grpc_request(&req) {
            grpc.clone()
        } else {
            web.clone()
        };
        target.oneshot(req)
    }))
}

async fn grpc_timeout(
    axum::extract::State(limit): axum::extract::State<Duration>,
    req: Request,
    next: axum::middleware::Next,
) -> Response {
    match tokio::time::timeout(limit, next.run(req)).await {
        Ok(resp) => resp,
        Err(_) => tonic::Status::cancelled("Timeout expired").into_http::<axum::body::Body>(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(ct: Option<&str>) -> axum::http::Request<()> {
        let mut b = axum::http::Request::builder().uri("/x");
        if let Some(ct) = ct {
            b = b.header(header::CONTENT_TYPE, ct);
        }
        b.body(()).unwrap()
    }

    #[test]
    fn grpc_is_told_apart_by_content_type() {
        assert!(is_grpc_request(&req(Some("application/grpc"))));
        assert!(is_grpc_request(&req(Some("application/grpc+proto"))));
        assert!(is_grpc_request(&req(Some(
            "application/grpc; charset=utf-8"
        ))));
        assert!(!is_grpc_request(&req(Some("application/grpc-web"))));
        assert!(!is_grpc_request(&req(Some("application/json"))));
        assert!(!is_grpc_request(&req(None)));
    }
}
