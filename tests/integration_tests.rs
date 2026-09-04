//! Integration tests for Fast Code Search
//!
//! These tests spin up real gRPC and HTTP servers, index test files,
//! and validate queries through both interfaces.

use anyhow::Result;
use fast_code_search::{
    search::{create_progress_broadcaster, IndexingProgress, SearchEngine},
    server::{
        create_server_with_engine,
        search_proto::{code_search_client::CodeSearchClient, IndexRequest, SearchRequest},
    },
    web::{create_router, AppState},
};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tonic::transport::Server;

/// Test file content - Rust source with a searchable function
const RUST_TEST_FILE: &str = r#"
/// A sample function for testing search
fn find_me_in_search() {
    println!("Hello from test!");
}

pub struct TestStruct {
    pub name: String,
    pub value: i32,
}

impl TestStruct {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            value: 42,
        }
    }
}
"#;

/// Test file content - Python source
const PYTHON_TEST_FILE: &str = r#"
def search_target_function():
    """A Python function to find in search"""
    return "found me"

class SearchableClass:
    def __init__(self, name):
        self.name = name
    
    def greet(self):
        return f"Hello, {self.name}"
"#;

/// Test file content - JavaScript source
const JS_TEST_FILE: &str = r#"
function javascriptSearchTarget() {
    console.log("JS function for testing");
}

class JsTestClass {
    constructor(value) {
        this.value = value;
    }
    
    getValue() {
        return this.value;
    }
}

module.exports = { javascriptSearchTarget, JsTestClass };
"#;

/// Setup context containing server addresses and temp directory handle
struct TestContext {
    grpc_url: String,
    http_url: String,
    _temp_dir: TempDir, // Keep alive for test duration
}

/// Creates a temporary directory with test files, indexes them, and starts both servers.
/// Returns the gRPC and HTTP URLs along with the temp directory handle.
async fn setup_test_server() -> Result<TestContext> {
    // Create temp directory with test files
    let temp_dir = TempDir::new()?;

    std::fs::write(temp_dir.path().join("test_file.rs"), RUST_TEST_FILE)?;
    std::fs::write(temp_dir.path().join("test_file.py"), PYTHON_TEST_FILE)?;
    std::fs::write(temp_dir.path().join("test_file.js"), JS_TEST_FILE)?;

    // Create shared engine and index test files
    let engine: AppState = Arc::new(RwLock::new(SearchEngine::new()));
    {
        let mut eng = engine.write().unwrap();
        eng.index_file(temp_dir.path().join("test_file.rs"))?;
        eng.index_file(temp_dir.path().join("test_file.py"))?;
        eng.index_file(temp_dir.path().join("test_file.js"))?;
        eng.resolve_imports();
    }

    let progress = Arc::new(RwLock::new(IndexingProgress::default()));
    let progress_tx = create_progress_broadcaster();

    // Start gRPC server on random port
    let grpc_listener = TcpListener::bind("127.0.0.1:0").await?;
    let grpc_addr = grpc_listener.local_addr()?;
    let grpc_service = create_server_with_engine(engine.clone());

    tokio::spawn(async move {
        Server::builder()
            .add_service(grpc_service)
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(
                grpc_listener,
            ))
            .await
            .expect("gRPC server failed");
    });

    // Start HTTP server on random port
    let http_listener = TcpListener::bind("127.0.0.1:0").await?;
    let http_addr = http_listener.local_addr()?;
    let router = create_router(engine, progress, progress_tx, None);

    tokio::spawn(async move {
        axum::serve(http_listener, router)
            .await
            .expect("HTTP server failed");
    });

    // Allow servers to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    Ok(TestContext {
        grpc_url: format!("http://{}", grpc_addr),
        http_url: format!("http://{}", http_addr),
        _temp_dir: temp_dir,
    })
}

// =============================================================================
// gRPC Tests
// =============================================================================

#[tokio::test]
async fn test_grpc_search_finds_rust_function() -> Result<()> {
    let ctx = setup_test_server().await?;

    let mut client = CodeSearchClient::connect(ctx.grpc_url).await?;

    let request = SearchRequest {
        query: "find_me_in_search".to_string(),
        max_results: 10,
        include_paths: vec![],
        exclude_paths: vec![],
        is_regex: false,
        symbols_only: false,
    };

    let mut stream = client.search(request).await?.into_inner();

    let mut results = vec![];
    while let Some(result) = stream.message().await? {
        results.push(result);
    }

    assert!(!results.is_empty(), "Expected at least one search result");
    assert!(
        results[0].file_path.contains("test_file.rs"),
        "Expected result from test_file.rs, got: {}",
        results[0].file_path
    );
    assert!(
        results[0].content.contains("find_me_in_search"),
        "Expected content to contain query"
    );

    Ok(())
}

#[tokio::test]
async fn test_grpc_search_finds_python_function() -> Result<()> {
    let ctx = setup_test_server().await?;

    let mut client = CodeSearchClient::connect(ctx.grpc_url).await?;

    let request = SearchRequest {
        query: "search_target_function".to_string(),
        max_results: 10,
        include_paths: vec![],
        exclude_paths: vec![],
        is_regex: false,
        symbols_only: false,
    };

    let mut stream = client.search(request).await?.into_inner();

    let mut results = vec![];
    while let Some(result) = stream.message().await? {
        results.push(result);
    }

    assert!(!results.is_empty(), "Expected at least one search result");
    assert!(
        results[0].file_path.contains("test_file.py"),
        "Expected result from test_file.py, got: {}",
        results[0].file_path
    );

    Ok(())
}

#[tokio::test]
async fn test_grpc_search_empty_query_returns_empty() -> Result<()> {
    let ctx = setup_test_server().await?;

    let mut client = CodeSearchClient::connect(ctx.grpc_url).await?;

    let request = SearchRequest {
        query: "".to_string(),
        max_results: 10,
        include_paths: vec![],
        exclude_paths: vec![],
        is_regex: false,
        symbols_only: false,
    };

    let mut stream = client.search(request).await?.into_inner();

    let mut results = vec![];
    while let Some(result) = stream.message().await? {
        results.push(result);
    }

    assert!(results.is_empty(), "Expected no results for empty query");

    Ok(())
}

#[tokio::test]
async fn test_grpc_search_no_match_returns_empty() -> Result<()> {
    let ctx = setup_test_server().await?;

    let mut client = CodeSearchClient::connect(ctx.grpc_url).await?;

    let request = SearchRequest {
        query: "this_string_definitely_does_not_exist_xyz123".to_string(),
        max_results: 10,
        include_paths: vec![],
        exclude_paths: vec![],
        is_regex: false,
        symbols_only: false,
    };

    let mut stream = client.search(request).await?.into_inner();

    let mut results = vec![];
    while let Some(result) = stream.message().await? {
        results.push(result);
    }

    assert!(
        results.is_empty(),
        "Expected no results for non-matching query"
    );

    Ok(())
}

#[tokio::test]
async fn test_grpc_index_request() -> Result<()> {
    let ctx = setup_test_server().await?;

    let mut client = CodeSearchClient::connect(ctx.grpc_url).await?;

    // Index the temp directory (already indexed, but this tests the RPC)
    let request = IndexRequest {
        paths: vec![ctx._temp_dir.path().to_string_lossy().to_string()],
    };

    let response = client.index(request).await?.into_inner();

    assert!(
        response.files_indexed > 0,
        "Expected at least one file to be indexed"
    );
    assert!(!response.message.is_empty(), "Expected a status message");

    Ok(())
}

/// Roadmap 1.10: the shipped server scopes the gRPC `Index` RPC to the
/// configured index roots, so a network client cannot index (and then read
/// back through /api/file) arbitrary host paths.
#[tokio::test]
async fn test_grpc_index_rejects_paths_outside_scope() -> Result<()> {
    use fast_code_search::server::create_server_with_engine_scoped;

    let inside = TempDir::new()?;
    let outside = TempDir::new()?;
    std::fs::write(inside.path().join("in.rs"), "fn inside_scope() {}\n")?;
    std::fs::write(outside.path().join("out.rs"), "fn outside_scope() {}\n")?;

    let engine: AppState = Arc::new(RwLock::new(SearchEngine::new()));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let service =
        create_server_with_engine_scoped(engine.clone(), vec![inside.path().to_path_buf()]);
    tokio::spawn(async move {
        Server::builder()
            .add_service(service)
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .expect("gRPC server failed");
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut client = CodeSearchClient::connect(format!("http://{addr}")).await?;

    let err = client
        .index(IndexRequest {
            paths: vec![outside.path().to_string_lossy().to_string()],
        })
        .await
        .expect_err("out-of-scope path must be rejected");
    assert_eq!(err.code(), tonic::Code::PermissionDenied, "{err:?}");
    assert!(engine.read().unwrap().search("outside_scope", 5).is_empty());

    let ok = client
        .index(IndexRequest {
            paths: vec![inside.path().to_string_lossy().to_string()],
        })
        .await?
        .into_inner();
    assert_eq!(ok.files_indexed, 1);
    assert_eq!(engine.read().unwrap().search("inside_scope", 5).len(), 1);
    Ok(())
}

// =============================================================================
// HTTP/REST Tests
// =============================================================================

#[tokio::test]
async fn test_http_search_finds_results() -> Result<()> {
    let ctx = setup_test_server().await?;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/api/search", ctx.http_url))
        .query(&[("q", "TestStruct")])
        .send()
        .await?;

    assert!(response.status().is_success(), "Expected 200 OK");

    let body: serde_json::Value = response.json().await?;

    assert!(
        body["total_results"].as_u64().unwrap() > 0,
        "Expected at least one result"
    );
    assert!(
        body["query"].as_str().unwrap() == "TestStruct",
        "Expected query to be echoed back"
    );
    assert!(
        !body["results"].as_array().unwrap().is_empty(),
        "Expected results array to have items"
    );

    Ok(())
}

/// Roadmap 1.9: `/api/context` must cap the window and never overflow.
/// `context=usize::MAX` used to compute `match_idx + context + 1` (a panic in
/// debug builds, a wrapped index in release) and could otherwise return the
/// whole file through the "lightweight" endpoint.
#[tokio::test]
async fn test_http_context_caps_window_and_survives_overflow() -> Result<()> {
    let ctx = setup_test_server().await?;
    let client = reqwest::Client::new();

    // Discover an indexed file path via search.
    let body: serde_json::Value = client
        .get(format!("{}/api/search", ctx.http_url))
        .query(&[("q", "TestStruct")])
        .send()
        .await?
        .json()
        .await?;
    let file = body["results"][0]["file_path"]
        .as_str()
        .unwrap()
        .to_string();

    let response = client
        .get(format!("{}/api/context", ctx.http_url))
        .query(&[
            ("file", file.as_str()),
            ("line", "1"),
            ("context", &usize::MAX.to_string()),
        ])
        .send()
        .await?;
    assert_eq!(response.status(), 200, "overflowing context must not fail");
    let body: serde_json::Value = response.json().await?;
    let n = body["lines"].as_array().unwrap().len();
    assert!(n <= 401, "window must be capped (got {n} lines)");
    assert_eq!(body["start_line"].as_u64().unwrap(), 1);
    Ok(())
}

/// Roadmap 1.9: a pathological regex is rejected at compile time by the
/// configured size limit and surfaces as a 400 JSON error, not a multi-hundred
/// megabyte compilation on the search thread.
#[tokio::test]
async fn test_http_regex_size_limit_returns_400() -> Result<()> {
    let ctx = setup_test_server().await?;
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/api/search", ctx.http_url))
        .query(&[("q", "(a{1000}){1000}"), ("regex", "true")])
        .send()
        .await?;
    assert_eq!(response.status(), 400, "oversized regex must be rejected");
    let body: serde_json::Value = response.json().await?;
    assert!(
        body["error"].as_str().unwrap_or("").contains("regex"),
        "error body should mention the regex: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn test_http_search_empty_query() -> Result<()> {
    let ctx = setup_test_server().await?;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/api/search", ctx.http_url))
        .query(&[("q", "")])
        .send()
        .await?;

    assert!(response.status().is_success(), "Expected 200 OK");

    let body: serde_json::Value = response.json().await?;

    assert_eq!(
        body["total_results"].as_u64().unwrap(),
        0,
        "Expected zero results for empty query"
    );

    Ok(())
}

#[tokio::test]
async fn test_http_search_javascript() -> Result<()> {
    let ctx = setup_test_server().await?;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/api/search", ctx.http_url))
        .query(&[("q", "javascriptSearchTarget")])
        .send()
        .await?;

    assert!(response.status().is_success());

    let body: serde_json::Value = response.json().await?;

    let results = body["results"].as_array().unwrap();
    assert!(!results.is_empty(), "Expected at least one JS result");
    assert!(
        results[0]["file_path"]
            .as_str()
            .unwrap()
            .contains("test_file.js"),
        "Expected result from test_file.js"
    );

    Ok(())
}

#[tokio::test]
async fn test_http_stats_endpoint() -> Result<()> {
    let ctx = setup_test_server().await?;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/api/stats", ctx.http_url))
        .send()
        .await?;

    assert!(response.status().is_success(), "Expected 200 OK");

    let body: serde_json::Value = response.json().await?;

    assert!(
        body["num_files"].as_u64().unwrap() >= 3,
        "Expected at least 3 indexed files"
    );
    assert!(
        body["num_trigrams"].as_u64().unwrap() > 0,
        "Expected trigrams to be indexed"
    );

    Ok(())
}

#[tokio::test]
async fn test_http_health_endpoint() -> Result<()> {
    let ctx = setup_test_server().await?;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/api/health", ctx.http_url))
        .send()
        .await?;

    assert!(response.status().is_success(), "Expected 200 OK");

    let body: serde_json::Value = response.json().await?;

    assert_eq!(
        body["status"].as_str().unwrap(),
        "healthy",
        "Expected healthy status"
    );
    assert!(body["version"].as_str().is_some(), "Expected version field");

    Ok(())
}

#[tokio::test]
async fn test_http_status_endpoint() -> Result<()> {
    let ctx = setup_test_server().await?;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/api/status", ctx.http_url))
        .send()
        .await?;

    assert!(response.status().is_success(), "Expected 200 OK");

    let body: serde_json::Value = response.json().await?;

    // Status should indicate idle (not currently indexing)
    assert!(body["status"].as_str().is_some(), "Expected status field");
    assert!(
        body.get("is_indexing").is_some(),
        "Expected is_indexing field"
    );

    Ok(())
}

// =============================================================================
// Cross-language search tests
// =============================================================================

#[tokio::test]
async fn test_search_across_languages() -> Result<()> {
    let ctx = setup_test_server().await?;

    let client = reqwest::Client::new();

    // Search for "class" which appears in all three files
    let response = client
        .get(format!("{}/api/search", ctx.http_url))
        .query(&[("q", "class"), ("max", "20")])
        .send()
        .await?;

    assert!(response.status().is_success());

    let body: serde_json::Value = response.json().await?;
    let results = body["results"].as_array().unwrap();

    // Should find matches in multiple files
    let file_paths: Vec<&str> = results
        .iter()
        .map(|r| r["file_path"].as_str().unwrap())
        .collect();

    // Verify we got results from at least two different file types
    let has_rs = file_paths.iter().any(|p| p.ends_with(".rs"));
    let has_py = file_paths.iter().any(|p| p.ends_with(".py"));
    let has_js = file_paths.iter().any(|p| p.ends_with(".js"));
    let matched_types = [has_rs, has_py, has_js].iter().filter(|&&b| b).count();

    assert!(
        matched_types >= 2,
        "Expected results from at least two file types (rs/py/js), got: {:?}",
        file_paths
    );

    Ok(())
}

// =============================================================================
// Symbols-only search tests
// =============================================================================

#[tokio::test]
async fn test_http_search_symbols_only() -> Result<()> {
    let ctx = setup_test_server().await?;

    let client = reqwest::Client::new();

    // Search for "find_me" with symbols mode - should find the function
    let response = client
        .get(format!("{}/api/search", ctx.http_url))
        .query(&[("q", "find_me"), ("symbols", "true")])
        .send()
        .await?;

    assert!(response.status().is_success());

    let body: serde_json::Value = response.json().await?;
    let results = body["results"].as_array().unwrap();

    // Should find the function definition
    assert!(
        !results.is_empty(),
        "Expected at least one symbol match for 'find_me'"
    );

    // All results should be symbol definitions
    for result in results {
        assert_eq!(
            result["match_type"].as_str().unwrap(),
            "SYMBOL_DEFINITION",
            "Expected all results to be symbol definitions in symbols-only mode"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_http_search_symbols_only_no_text_match() -> Result<()> {
    let ctx = setup_test_server().await?;

    let client = reqwest::Client::new();

    // Search for "println" with symbols mode - should NOT find it (it's not a symbol name)
    let response = client
        .get(format!("{}/api/search", ctx.http_url))
        .query(&[("q", "println"), ("symbols", "true")])
        .send()
        .await?;

    assert!(response.status().is_success());

    let body: serde_json::Value = response.json().await?;

    assert_eq!(
        body["total_results"].as_u64().unwrap(),
        0,
        "Expected no results when searching for 'println' in symbols-only mode (it's not a symbol name)"
    );

    Ok(())
}

#[tokio::test]
async fn test_grpc_search_symbols_only() -> Result<()> {
    let ctx = setup_test_server().await?;

    let mut client = CodeSearchClient::connect(ctx.grpc_url).await?;

    // Search for a function name that exists in the test files
    let request = SearchRequest {
        query: "find_me".to_string(),
        max_results: 10,
        include_paths: vec![],
        exclude_paths: vec![],
        is_regex: false,
        symbols_only: true,
    };

    let mut stream = client.search(request).await?.into_inner();

    let mut results = vec![];
    while let Some(result) = stream.message().await? {
        results.push(result);
    }

    // Should find the function definition
    assert!(
        !results.is_empty(),
        "Expected at least one symbol result for 'find_me'"
    );

    // All results should be symbol matches
    for result in &results {
        assert_eq!(
            result.match_type,
            1, // SYMBOL_DEFINITION
            "Expected all gRPC results to be symbol definitions"
        );
    }

    Ok(())
}

// =============================================================================
// Regex search tests
// =============================================================================

#[tokio::test]
async fn test_http_regex_search() -> Result<()> {
    let ctx = setup_test_server().await?;

    let client = reqwest::Client::new();

    // Search for a regex pattern matching function names
    let response = client
        .get(format!("{}/api/search", ctx.http_url))
        .query(&[("q", r"fn \w+"), ("regex", "true")])
        .send()
        .await?;

    assert!(response.status().is_success());

    let body: serde_json::Value = response.json().await?;
    let results = body["results"].as_array().unwrap();

    // Should find Rust function definitions
    assert!(
        !results.is_empty(),
        "Expected at least one result for regex pattern"
    );

    Ok(())
}

#[tokio::test]
async fn test_grpc_regex_search() -> Result<()> {
    let ctx = setup_test_server().await?;

    let mut client = CodeSearchClient::connect(ctx.grpc_url).await?;

    // Search for class definitions across languages
    let request = SearchRequest {
        query: r"class \w+".to_string(),
        max_results: 10,
        include_paths: vec![],
        exclude_paths: vec![],
        is_regex: true,
        symbols_only: false,
    };

    let mut stream = client.search(request).await?.into_inner();

    let mut results = vec![];
    while let Some(result) = stream.message().await? {
        results.push(result);
    }

    // Should find class definitions in Python and JavaScript
    assert!(
        !results.is_empty(),
        "Expected at least one result for class regex pattern"
    );

    Ok(())
}

// =============================================================================
// Path filtering tests
// =============================================================================

#[tokio::test]
async fn test_http_search_with_include_filter() -> Result<()> {
    let ctx = setup_test_server().await?;

    let client = reqwest::Client::new();

    // Search only in Python files
    let response = client
        .get(format!("{}/api/search", ctx.http_url))
        .query(&[("q", "class"), ("include", "*.py")])
        .send()
        .await?;

    assert!(response.status().is_success());

    let body: serde_json::Value = response.json().await?;
    let results = body["results"].as_array().unwrap();

    // All results should be from .py files
    for result in results {
        let path = result["file_path"].as_str().unwrap();
        assert!(
            path.ends_with(".py"),
            "Expected only Python files, got: {}",
            path
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_http_search_with_exclude_filter() -> Result<()> {
    let ctx = setup_test_server().await?;

    let client = reqwest::Client::new();

    // Search but exclude JavaScript files
    let response = client
        .get(format!("{}/api/search", ctx.http_url))
        .query(&[("q", "function"), ("exclude", "*.js")])
        .send()
        .await?;

    assert!(response.status().is_success());

    let body: serde_json::Value = response.json().await?;
    let results = body["results"].as_array().unwrap();

    // No results should be from .js files
    for result in results {
        let path = result["file_path"].as_str().unwrap();
        assert!(
            !path.ends_with(".js"),
            "Expected no JavaScript files, got: {}",
            path
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_grpc_search_with_path_filters() -> Result<()> {
    let ctx = setup_test_server().await?;

    let mut client = CodeSearchClient::connect(ctx.grpc_url).await?;

    // Include only Rust files
    let request = SearchRequest {
        query: "struct".to_string(),
        max_results: 10,
        include_paths: vec!["*.rs".to_string()],
        exclude_paths: vec![],
        is_regex: false,
        symbols_only: false,
    };

    let mut stream = client.search(request).await?.into_inner();

    let mut results = vec![];
    while let Some(result) = stream.message().await? {
        results.push(result);
    }

    // All results should be from .rs files
    for result in &results {
        assert!(
            result.file_path.ends_with(".rs"),
            "Expected only Rust files, got: {}",
            result.file_path
        );
    }

    Ok(())
}

// =============================================================================
// Max results limiting tests
// =============================================================================

#[tokio::test]
async fn test_http_search_max_results_limit() -> Result<()> {
    let ctx = setup_test_server().await?;

    let client = reqwest::Client::new();

    // Search with max results limit
    let response = client
        .get(format!("{}/api/search", ctx.http_url))
        .query(&[("q", "e"), ("max", "2")]) // 'e' should match many lines
        .send()
        .await?;

    assert!(response.status().is_success());

    let body: serde_json::Value = response.json().await?;
    let results = body["results"].as_array().unwrap();

    // Should respect max results limit
    assert!(
        results.len() <= 2,
        "Expected at most 2 results, got {}",
        results.len()
    );

    Ok(())
}

#[tokio::test]
async fn test_grpc_search_max_results_limit() -> Result<()> {
    let ctx = setup_test_server().await?;

    let mut client = CodeSearchClient::connect(ctx.grpc_url).await?;

    // Search with max results limit
    let request = SearchRequest {
        query: "e".to_string(), // Common character
        max_results: 3,
        include_paths: vec![],
        exclude_paths: vec![],
        is_regex: false,
        symbols_only: false,
    };

    let mut stream = client.search(request).await?.into_inner();

    let mut results = vec![];
    while let Some(result) = stream.message().await? {
        results.push(result);
    }

    // Should respect max results limit
    assert!(
        results.len() <= 3,
        "Expected at most 3 results, got {}",
        results.len()
    );

    Ok(())
}

// =============================================================================
// Dependency tracking tests
// =============================================================================

#[tokio::test]
async fn test_http_dependencies_endpoint() -> Result<()> {
    let ctx = setup_test_server().await?;

    let client = reqwest::Client::new();

    // Query dependencies for a file
    let response = client
        .get(format!("{}/api/dependencies", ctx.http_url))
        .query(&[("file", "test_file.js")])
        .send()
        .await?;

    assert!(response.status().is_success());

    let body: serde_json::Value = response.json().await?;

    // Response should have expected structure
    assert!(body["file"].as_str().is_some());
    assert!(
        body["files"].as_array().is_some(),
        "Expected 'files' array in response"
    );
    assert!(
        body["count"].as_u64().is_some(),
        "Expected 'count' field in response"
    );

    Ok(())
}

#[tokio::test]
async fn test_http_dependents_endpoint() -> Result<()> {
    let ctx = setup_test_server().await?;

    let client = reqwest::Client::new();

    // Query dependents for a file
    let response = client
        .get(format!("{}/api/dependents", ctx.http_url))
        .query(&[("file", "test_file.py")])
        .send()
        .await?;

    assert!(response.status().is_success());

    let body: serde_json::Value = response.json().await?;

    // Response should have expected structure
    assert!(body["file"].as_str().is_some());
    assert!(
        body["files"].as_array().is_some(),
        "Expected 'files' array in response"
    );
    assert!(
        body["count"].as_u64().is_some(),
        "Expected 'count' field in response"
    );

    Ok(())
}

// =============================================================================
// Diagnostics and monitoring tests
// =============================================================================

#[tokio::test]
async fn test_http_diagnostics_endpoint() -> Result<()> {
    let ctx = setup_test_server().await?;

    let client = reqwest::Client::new();

    // Get diagnostics
    let response = client
        .get(format!("{}/api/diagnostics", ctx.http_url))
        .send()
        .await?;

    assert!(response.status().is_success());

    let body: serde_json::Value = response.json().await?;

    // Check for expected diagnostic fields based on KeywordDiagnosticsResponse struct
    // HealthStatus is an enum that serializes as a string ("healthy", "degraded", or "unhealthy")
    assert!(
        body["status"].as_str().is_some(),
        "Expected status string (HealthStatus enum) in diagnostics"
    );
    assert!(
        body["version"].as_str().is_some(),
        "Expected version string"
    );
    assert!(
        body["uptime_secs"].as_u64().is_some(),
        "Expected uptime_secs"
    );
    assert!(body["config"].is_object(), "Expected config object");
    assert!(
        body["index"].is_object(),
        "Expected index diagnostics object"
    );

    Ok(())
}

#[tokio::test]
async fn test_http_diagnostics_with_test_mode() -> Result<()> {
    let ctx = setup_test_server().await?;

    let client = reqwest::Client::new();

    // Get diagnostics with self-test enabled
    let response = client
        .get(format!("{}/api/diagnostics", ctx.http_url))
        .query(&[("test", "true")])
        .send()
        .await?;

    assert!(response.status().is_success());

    let body: serde_json::Value = response.json().await?;

    // Check for self-test results (self_tests is an array, test_summary is the summary)
    assert!(
        body["self_tests"].as_array().is_some(),
        "Expected self_tests array when test=true"
    );
    assert!(
        body["test_summary"].is_object(),
        "Expected test_summary object"
    );

    Ok(())
}

// =============================================================================
// Non-UTF-8 Encoding Transcoding Tests
// =============================================================================

#[tokio::test]
async fn test_index_latin1_file() -> Result<()> {
    // Create a temp dir with a Latin-1 encoded file
    let temp_dir = TempDir::new()?;

    // Write "café résumé" in Latin-1 encoding
    // café: 63 61 66 E9, space: 20, résumé: 72 E9 73 75 6D E9
    let latin1_bytes: &[u8] = &[
        0x63, 0x61, 0x66, 0xE9, 0x20, 0x72, 0xE9, 0x73, 0x75, 0x6D, 0xE9,
    ];
    let file_path = temp_dir.path().join("latin1_file.txt");
    std::fs::write(&file_path, latin1_bytes)?;

    // Index the file
    let engine: AppState = Arc::new(RwLock::new(SearchEngine::new()));
    {
        let mut eng = engine.write().unwrap();
        eng.index_file(&file_path)?;
    }

    // Search for content — the transcoded text should be searchable
    let eng = engine.read().unwrap();
    let results = eng.search("caf", 10);
    assert!(
        !results.is_empty(),
        "Expected Latin-1 transcoded file to be searchable"
    );

    Ok(())
}

#[tokio::test]
async fn test_index_utf16_le_file() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Write "Hello World" in UTF-16 LE with BOM
    let utf16le: &[u8] = &[
        0xFF, 0xFE, // BOM
        0x48, 0x00, // H
        0x65, 0x00, // e
        0x6C, 0x00, // l
        0x6C, 0x00, // l
        0x6F, 0x00, // o
        0x20, 0x00, // space
        0x57, 0x00, // W
        0x6F, 0x00, // o
        0x72, 0x00, // r
        0x6C, 0x00, // l
        0x64, 0x00, // d
    ];
    let file_path = temp_dir.path().join("utf16_file.txt");
    std::fs::write(&file_path, utf16le)?;

    let engine: AppState = Arc::new(RwLock::new(SearchEngine::new()));
    {
        let mut eng = engine.write().unwrap();
        eng.index_file(&file_path)?;
    }

    let eng = engine.read().unwrap();
    let results = eng.search("Hello World", 10);
    assert!(
        !results.is_empty(),
        "Expected UTF-16 LE transcoded file to be searchable"
    );

    Ok(())
}

#[tokio::test]
async fn test_index_shift_jis_file() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Encode a longer Japanese text in Shift-JIS for reliable detection
    let text = "日本語のテストです。これは日本語のテキストです。";
    let (encoded, _, _) = encoding_rs::SHIFT_JIS.encode(text);
    let file_path = temp_dir.path().join("shift_jis_file.txt");
    std::fs::write(&file_path, &*encoded)?;

    let engine: AppState = Arc::new(RwLock::new(SearchEngine::new()));
    {
        let mut eng = engine.write().unwrap();
        eng.index_file(&file_path)?;
    }

    let eng = engine.read().unwrap();
    let results = eng.search("日本語", 10);
    assert!(
        !results.is_empty(),
        "Expected Shift-JIS transcoded file to be searchable"
    );

    Ok(())
}

#[tokio::test]
async fn test_config_disable_transcoding() -> Result<()> {
    use fast_code_search::search::engine::PartialIndexedFile;

    let temp_dir = TempDir::new()?;

    // Write a Latin-1 file
    let latin1_bytes: &[u8] = &[0x63, 0x61, 0x66, 0xE9]; // "café"
    let file_path = temp_dir.path().join("latin1.txt");
    std::fs::write(&file_path, latin1_bytes)?;

    // With transcoding enabled, should succeed
    let result_enabled = PartialIndexedFile::process(&file_path, true, 0);
    assert!(
        result_enabled.is_some(),
        "Expected transcoding to succeed when enabled"
    );
    let (_, transcoded) = result_enabled.unwrap();
    assert!(
        transcoded,
        "Expected transcoded flag to be true for non-UTF-8 file"
    );

    // With transcoding disabled, should return None for non-UTF-8 files
    let result_disabled = PartialIndexedFile::process(&file_path, false, 0);
    assert!(
        result_disabled.is_none(),
        "Expected non-UTF-8 file to be skipped when transcoding disabled"
    );

    Ok(())
}

#[tokio::test]
async fn test_config_disable_symbols() -> Result<()> {
    use fast_code_search::search::{PartialIndexedFile, PreIndexedFile};
    use tempfile::TempDir;

    let temp_dir = TempDir::new()?;
    let file_path = temp_dir.path().join("test.rs");
    std::fs::write(
        &file_path,
        "pub fn my_function() { println!(\"hello\"); }\n",
    )?;

    // With symbols enabled (default), FileName symbol + parsed symbols should be present
    let (partial_enabled, _) = PartialIndexedFile::process(&file_path, false, 0).unwrap();
    let pre_enabled = PreIndexedFile::from_partial(partial_enabled, true);
    // At minimum the FileName symbol is always added
    assert!(
        !pre_enabled.symbols.is_empty(),
        "Expected symbols to be extracted when enable_symbols=true"
    );

    // With symbols disabled, only the FileName symbol should be present (no tree-sitter extraction)
    let (partial_disabled, _) = PartialIndexedFile::process(&file_path, false, 0).unwrap();
    let pre_disabled = PreIndexedFile::from_partial(partial_disabled, false);
    // FileName symbol is always added even when symbols are disabled
    assert_eq!(
        pre_disabled.symbols.len(),
        1,
        "Expected only the FileName symbol when enable_symbols=false"
    );
    // No imports resolved when symbols are disabled
    assert!(
        pre_disabled.imports.is_empty(),
        "Expected no imports when enable_symbols=false"
    );

    // Verify that SearchEngine.enable_symbols defaults to true
    let engine = fast_code_search::search::SearchEngine::new();
    assert!(
        engine.enable_symbols,
        "SearchEngine should have enable_symbols=true by default"
    );

    Ok(())
}

// =============================================================================
// Super Integration Test
//
// One comprehensive canary test that exercises the full system with a realistic
// multi-language, multi-directory corpus.  A unique sentinel token
// (CANARY_TOKEN) is planted in every file so we can count and filter results
// with confidence.
// =============================================================================

/// A token guaranteed to appear exactly once in each corpus file.
/// Searching for it must always return one match per file.
const CANARY_TOKEN: &str = "SUPER_CANARY_92f7e3b1";

/// src/auth.rs  – Rust authentication module
const CORPUS_AUTH_RS: &str = r#"
//! Authentication module (SUPER_CANARY_92f7e3b1)

pub struct AuthManager {
    secret: String,
}

impl AuthManager {
    pub fn new(secret: &str) -> Self {
        Self { secret: secret.to_string() }
    }

    pub fn authenticate(&self, token: &str) -> bool {
        token == self.secret
    }

    pub fn verify_token(&self, token: &str) -> Result<bool, String> {
        if token.is_empty() {
            return Err("empty token".into());
        }
        Ok(self.authenticate(token))
    }
}
"#;

/// src/database.rs  – Rust database module
const CORPUS_DATABASE_RS: &str = r#"
//! Database access layer (SUPER_CANARY_92f7e3b1)

pub struct DatabasePool {
    url: String,
    max_connections: usize,
}

impl DatabasePool {
    pub fn new(url: &str, max_connections: usize) -> Self {
        Self { url: url.to_string(), max_connections }
    }

    pub fn execute_query(&self, sql: &str) -> Vec<String> {
        // stub: return empty result set
        let _ = sql;
        vec![]
    }

    pub fn execute_sql_transaction(&self, statements: &[&str]) -> Result<(), String> {
        for stmt in statements {
            if stmt.is_empty() {
                return Err("empty statement".into());
            }
        }
        Ok(())
    }
}
"#;

/// src/main.rs  – Rust entry point
const CORPUS_MAIN_RS: &str = r#"
//! Application entry point (SUPER_CANARY_92f7e3b1)

mod auth;
mod database;

fn main() {
    let auth = auth::AuthManager::new("s3cr3t");
    let db   = database::DatabasePool::new("postgres://localhost/app", 10);
    println!("auth={} db={}", auth.authenticate("s3cr3t"), db.execute_query("SELECT 1").len());
}
"#;

/// lib/utils.py  – Python utilities
const CORPUS_UTILS_PY: &str = r#"
# Utility helpers (SUPER_CANARY_92f7e3b1)

def helper_calculate_hash(data: bytes) -> str:
    import hashlib
    return hashlib.sha256(data).hexdigest()

class UserValidator:
    def __init__(self, rules):
        self.rules = rules

    def validate(self, user):
        return all(rule(user) for rule in self.rules)
"#;

/// lib/models.py  – Python domain models
const CORPUS_MODELS_PY: &str = r#"
# Domain models (SUPER_CANARY_92f7e3b1)

class User:
    def __init__(self, name: str, email: str):
        self.name  = name
        self.email = email

    def display(self) -> str:
        return f"{self.name} <{self.email}>"

class Post:
    def __init__(self, title: str, body: str, author: 'User'):
        self.title  = title
        self.body   = body
        self.author = author
"#;

/// frontend/app.js  – JavaScript front-end
const CORPUS_APP_JS: &str = r#"
// Front-end application (SUPER_CANARY_92f7e3b1)

class AppController {
    constructor(config) {
        this.config = config;
    }

    renderDashboard(container) {
        container.innerHTML = '<h1>Dashboard</h1>';
    }

    fetchData(endpoint) {
        return fetch(endpoint).then(r => r.json());
    }
}

module.exports = { AppController };
"#;

/// tests/auth_test.rs  – Rust unit tests
const CORPUS_AUTH_TEST_RS: &str = r#"
//! Auth unit tests (SUPER_CANARY_92f7e3b1)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authenticate_valid() {
        let mgr = AuthManager::new("pass");
        assert!(mgr.authenticate("pass"));
    }

    #[test]
    fn test_authenticate_invalid() {
        let mgr = AuthManager::new("pass");
        assert!(!mgr.authenticate("wrong"));
    }
}
"#;

/// Helper: build the rich corpus under `root`, index it, and start servers.
/// Returns a `TestContext` pointing at both servers.
async fn setup_super_test_server() -> Result<TestContext> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    // Create directory layout
    std::fs::create_dir_all(root.join("src"))?;
    std::fs::create_dir_all(root.join("lib"))?;
    std::fs::create_dir_all(root.join("frontend"))?;
    std::fs::create_dir_all(root.join("tests"))?;

    // Write corpus files
    std::fs::write(root.join("src/auth.rs"), CORPUS_AUTH_RS)?;
    std::fs::write(root.join("src/database.rs"), CORPUS_DATABASE_RS)?;
    std::fs::write(root.join("src/main.rs"), CORPUS_MAIN_RS)?;
    std::fs::write(root.join("lib/utils.py"), CORPUS_UTILS_PY)?;
    std::fs::write(root.join("lib/models.py"), CORPUS_MODELS_PY)?;
    std::fs::write(root.join("frontend/app.js"), CORPUS_APP_JS)?;
    std::fs::write(root.join("tests/auth_test.rs"), CORPUS_AUTH_TEST_RS)?;

    // Index every file
    let engine: AppState = Arc::new(RwLock::new(SearchEngine::new()));
    {
        let mut eng = engine.write().unwrap();
        eng.index_file(root.join("src/auth.rs"))?;
        eng.index_file(root.join("src/database.rs"))?;
        eng.index_file(root.join("src/main.rs"))?;
        eng.index_file(root.join("lib/utils.py"))?;
        eng.index_file(root.join("lib/models.py"))?;
        eng.index_file(root.join("frontend/app.js"))?;
        eng.index_file(root.join("tests/auth_test.rs"))?;
        eng.resolve_imports();
    }

    let progress = Arc::new(RwLock::new(IndexingProgress::default()));
    let progress_tx = create_progress_broadcaster();

    // gRPC server
    let grpc_listener = TcpListener::bind("127.0.0.1:0").await?;
    let grpc_addr = grpc_listener.local_addr()?;
    let grpc_service = create_server_with_engine(engine.clone());
    tokio::spawn(async move {
        Server::builder()
            .add_service(grpc_service)
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(
                grpc_listener,
            ))
            .await
            .expect("gRPC server failed");
    });

    // HTTP server
    let http_listener = TcpListener::bind("127.0.0.1:0").await?;
    let http_addr = http_listener.local_addr()?;
    let router = create_router(engine, progress, progress_tx, None);
    tokio::spawn(async move {
        axum::serve(http_listener, router)
            .await
            .expect("HTTP server failed");
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    Ok(TestContext {
        grpc_url: format!("http://{}", grpc_addr),
        http_url: format!("http://{}", http_addr),
        _temp_dir: temp_dir,
    })
}

/// The one super integration test.
///
/// It exercises every major feature of the search engine end-to-end:
///   • Cross-language text search via gRPC and HTTP
///   • Symbol-only search (gRPC + HTTP)
///   • Regex search (gRPC + HTTP)
///   • Path include / exclude filtering (HTTP + gRPC)
///   • Max-results capping
///   • Canary-token coverage: every indexed file is reachable
///   • HTTP infrastructure endpoints: /health, /stats, /status, /diagnostics, /dependencies
///   • gRPC index RPC
///   • Negative cases: empty query, no-match query, symbol search for non-symbol
#[tokio::test]
async fn test_super_integration() -> Result<()> {
    let ctx = setup_super_test_server().await?;
    let http = reqwest::Client::new();

    // ── 1. Health endpoint ──────────────────────────────────────────────────
    {
        let body: serde_json::Value = http
            .get(format!("{}/api/health", ctx.http_url))
            .send()
            .await?
            .json()
            .await?;
        assert_eq!(body["status"].as_str().unwrap(), "healthy");
        assert!(body["version"].as_str().is_some(), "health.version missing");
    }

    // ── 2. Stats: all 7 files must be indexed ───────────────────────────────
    {
        let body: serde_json::Value = http
            .get(format!("{}/api/stats", ctx.http_url))
            .send()
            .await?
            .json()
            .await?;
        let num_files = body["num_files"].as_u64().unwrap();
        assert!(num_files >= 7, "Expected ≥7 indexed files, got {num_files}");
        assert!(
            body["num_trigrams"].as_u64().unwrap() > 0,
            "stats.num_trigrams must be > 0"
        );
    }

    // ── 3. Status endpoint is present and well-formed ───────────────────────
    {
        let body: serde_json::Value = http
            .get(format!("{}/api/status", ctx.http_url))
            .send()
            .await?
            .json()
            .await?;
        assert!(body["status"].as_str().is_some(), "status.status missing");
        assert!(
            body.get("is_indexing").is_some(),
            "status.is_indexing missing"
        );
    }

    // ── 4. Diagnostics (basic) ───────────────────────────────────────────────
    {
        let body: serde_json::Value = http
            .get(format!("{}/api/diagnostics", ctx.http_url))
            .send()
            .await?
            .json()
            .await?;
        assert!(body["status"].as_str().is_some());
        assert!(body["version"].as_str().is_some());
        assert!(body["uptime_secs"].as_u64().is_some());
        assert!(body["config"].is_object());
        assert!(body["index"].is_object());
    }

    // ── 5. Diagnostics with self-test ────────────────────────────────────────
    {
        let body: serde_json::Value = http
            .get(format!("{}/api/diagnostics", ctx.http_url))
            .query(&[("test", "true")])
            .send()
            .await?
            .json()
            .await?;
        assert!(
            body["self_tests"].as_array().is_some(),
            "diagnostics self_tests array missing"
        );
        assert!(
            body["test_summary"].is_object(),
            "diagnostics test_summary missing"
        );
    }

    // ── 6. CANARY search: token appears in every file ────────────────────────
    // HTTP – must return exactly 7 matches (one per file)
    {
        let body: serde_json::Value = http
            .get(format!("{}/api/search", ctx.http_url))
            .query(&[("q", CANARY_TOKEN), ("max", "50")])
            .send()
            .await?
            .json()
            .await?;
        assert_eq!(
            body["query"].as_str().unwrap(),
            CANARY_TOKEN,
            "HTTP response must echo the query"
        );
        let total = body["total_results"].as_u64().unwrap();
        assert_eq!(
            total, 7,
            "Expected exactly 7 canary matches (one per file), got {total}"
        );
    }

    // ── 7. gRPC canary search ────────────────────────────────────────────────
    {
        let mut client = CodeSearchClient::connect(ctx.grpc_url.clone()).await?;
        let req = SearchRequest {
            query: CANARY_TOKEN.to_string(),
            max_results: 50,
            include_paths: vec![],
            exclude_paths: vec![],
            is_regex: false,
            symbols_only: false,
        };
        let mut stream = client.search(req).await?.into_inner();
        let mut results = vec![];
        while let Some(r) = stream.message().await? {
            results.push(r);
        }
        assert_eq!(
            results.len(),
            7,
            "gRPC: expected 7 canary results, got {}",
            results.len()
        );
        // Every result must contain the token in its content
        for r in &results {
            assert!(
                r.content.contains(CANARY_TOKEN),
                "gRPC result content missing canary token: {}",
                r.content
            );
            assert!(r.score > 0.0, "Score must be positive");
        }
    }

    // ── 8. HTTP symbol-only search: AuthManager ──────────────────────────────
    // "AuthManager" is a struct/type definition – symbols mode must find it
    {
        let body: serde_json::Value = http
            .get(format!("{}/api/search", ctx.http_url))
            .query(&[("q", "AuthManager"), ("symbols", "true")])
            .send()
            .await?
            .json()
            .await?;
        let results = body["results"].as_array().unwrap();
        assert!(
            !results.is_empty(),
            "symbols search for 'AuthManager' must return at least one result"
        );
        for r in results {
            assert_eq!(
                r["match_type"].as_str().unwrap(),
                "SYMBOL_DEFINITION",
                "symbols-only mode must return SYMBOL_DEFINITION, got: {}",
                r["match_type"]
            );
        }
        // The top result must come from auth.rs (definition) or auth_test.rs (usage)
        let top_path = results[0]["file_path"].as_str().unwrap();
        assert!(
            top_path.contains("auth"),
            "Top symbol result should be from an auth file, got: {top_path}"
        );
    }

    // ── 9. gRPC symbol-only search: DatabasePool ────────────────────────────
    {
        let mut client = CodeSearchClient::connect(ctx.grpc_url.clone()).await?;
        let req = SearchRequest {
            query: "DatabasePool".to_string(),
            max_results: 10,
            include_paths: vec![],
            exclude_paths: vec![],
            is_regex: false,
            symbols_only: true,
        };
        let mut stream_sym = client.search(req).await?.into_inner();
        let mut results = vec![];
        while let Some(r) = stream_sym.message().await? {
            results.push(r);
        }
        assert!(
            !results.is_empty(),
            "gRPC symbols search for 'DatabasePool' must hit"
        );
        for r in &results {
            assert_eq!(
                r.match_type, 1,
                "gRPC symbols result must be match_type 1 (SYMBOL_DEFINITION)"
            );
        }
    }

    // ── 10. HTTP: symbols mode must NOT find inline code ────────────────────
    // "execute_query" is a method body call – in symbols-only mode it should
    // appear only as a SYMBOL_DEFINITION, not as a plain text hit.
    // Searching for a string that's *only* in comments should return nothing.
    {
        let body: serde_json::Value = http
            .get(format!("{}/api/search", ctx.http_url))
            .query(&[("q", "stub: return empty"), ("symbols", "true")])
            .send()
            .await?
            .json()
            .await?;
        assert_eq!(
            body["total_results"].as_u64().unwrap(),
            0,
            "Symbols-only search for inline comment text must return 0 results"
        );
    }

    // ── 11. HTTP regex search: Rust fn definitions ───────────────────────────
    {
        let body: serde_json::Value = http
            .get(format!("{}/api/search", ctx.http_url))
            .query(&[("q", r"pub fn \w+"), ("regex", "true"), ("max", "20")])
            .send()
            .await?
            .json()
            .await?;
        let results = body["results"].as_array().unwrap();
        assert!(
            !results.is_empty(),
            "Regex 'pub fn \\w+' must match Rust functions"
        );
        // All hits must be from Rust files
        for r in results {
            let path = r["file_path"].as_str().unwrap();
            assert!(
                path.ends_with(".rs"),
                "Regex 'pub fn' should only hit .rs files, got: {path}"
            );
        }
    }

    // ── 12. gRPC regex search: class definitions across languages ───────────
    {
        let mut client = CodeSearchClient::connect(ctx.grpc_url.clone()).await?;
        let req = SearchRequest {
            query: r"class \w+".to_string(),
            max_results: 20,
            include_paths: vec![],
            exclude_paths: vec![],
            is_regex: true,
            symbols_only: false,
        };
        let mut stream_regex = client.search(req).await?.into_inner();
        let mut results = vec![];
        while let Some(r) = stream_regex.message().await? {
            results.push(r);
        }
        assert!(
            !results.is_empty(),
            "gRPC regex 'class \\w+' must find py/js classes"
        );
        let has_py = results.iter().any(|r| r.file_path.ends_with(".py"));
        let has_js = results.iter().any(|r| r.file_path.ends_with(".js"));
        assert!(has_py, "Regex class search must include Python files");
        assert!(has_js, "Regex class search must include JavaScript files");
    }

    // ── 13. HTTP include filter: only Rust files ─────────────────────────────
    {
        let body: serde_json::Value = http
            .get(format!("{}/api/search", ctx.http_url))
            .query(&[("q", CANARY_TOKEN), ("include", "*.rs"), ("max", "50")])
            .send()
            .await?
            .json()
            .await?;
        let results = body["results"].as_array().unwrap();
        assert!(
            !results.is_empty(),
            "include=*.rs must return at least one result"
        );
        for r in results {
            let path = r["file_path"].as_str().unwrap();
            assert!(
                path.ends_with(".rs"),
                "include=*.rs filter must exclude non-Rust files, got: {path}"
            );
        }
        // Should have found exactly 3 Rust files (auth.rs, database.rs, main.rs, auth_test.rs = 4)
        let count = body["total_results"].as_u64().unwrap();
        assert_eq!(
            count, 4,
            "include=*.rs must match exactly 4 Rust files, got {count}"
        );
    }

    // ── 14. HTTP exclude filter: omit test files ─────────────────────────────
    {
        let body: serde_json::Value = http
            .get(format!("{}/api/search", ctx.http_url))
            .query(&[("q", CANARY_TOKEN), ("exclude", "*/tests/*"), ("max", "50")])
            .send()
            .await?
            .json()
            .await?;
        let results = body["results"].as_array().unwrap();
        // After excluding tests/ we expect 6 results (7 - auth_test.rs)
        let count = body["total_results"].as_u64().unwrap();
        assert_eq!(
            count, 6,
            "exclude=*/tests/* must remove exactly 1 file (auth_test.rs), got {count}"
        );
        for r in results {
            let path = r["file_path"].as_str().unwrap();
            assert!(
                !path.contains("tests"),
                "excluded tests/ file appeared in results: {path}"
            );
        }
    }

    // ── 15. gRPC include filter: only Python files ───────────────────────────
    {
        let mut client = CodeSearchClient::connect(ctx.grpc_url.clone()).await?;
        let req = SearchRequest {
            query: CANARY_TOKEN.to_string(),
            max_results: 50,
            include_paths: vec!["*.py".to_string()],
            exclude_paths: vec![],
            is_regex: false,
            symbols_only: false,
        };
        let mut stream_py = client.search(req).await?.into_inner();
        let mut results = vec![];
        while let Some(r) = stream_py.message().await? {
            results.push(r);
        }
        assert_eq!(
            results.len(),
            2,
            "gRPC include=*.py must return exactly 2 Python files, got {}",
            results.len()
        );
        for r in &results {
            assert!(
                r.file_path.ends_with(".py"),
                "gRPC include=*.py: non-Python file in results: {}",
                r.file_path
            );
        }
    }

    // ── 16. gRPC exclude filter: omit JavaScript ─────────────────────────────
    {
        let mut client = CodeSearchClient::connect(ctx.grpc_url.clone()).await?;
        let req = SearchRequest {
            query: CANARY_TOKEN.to_string(),
            max_results: 50,
            include_paths: vec![],
            exclude_paths: vec!["*.js".to_string()],
            is_regex: false,
            symbols_only: false,
        };
        let mut stream_nojs = client.search(req).await?.into_inner();
        let mut results = vec![];
        while let Some(r) = stream_nojs.message().await? {
            results.push(r);
        }
        assert_eq!(
            results.len(),
            6,
            "gRPC exclude=*.js must return 6 results (7 - app.js), got {}",
            results.len()
        );
        for r in &results {
            assert!(
                !r.file_path.ends_with(".js"),
                "gRPC exclude=*.js: JS file still appeared: {}",
                r.file_path
            );
        }
    }

    // ── 17. Max-results capping ───────────────────────────────────────────────
    {
        let body: serde_json::Value = http
            .get(format!("{}/api/search", ctx.http_url))
            .query(&[("q", CANARY_TOKEN), ("max", "3")])
            .send()
            .await?
            .json()
            .await?;
        let returned = body["results"].as_array().unwrap().len();
        assert!(returned <= 3, "max=3 must cap results at 3, got {returned}");
    }

    // ── 18. gRPC max-results capping ─────────────────────────────────────────
    {
        let mut client = CodeSearchClient::connect(ctx.grpc_url.clone()).await?;
        let req = SearchRequest {
            query: CANARY_TOKEN.to_string(),
            max_results: 2,
            include_paths: vec![],
            exclude_paths: vec![],
            is_regex: false,
            symbols_only: false,
        };
        let mut stream_max = client.search(req).await?.into_inner();
        let mut results = vec![];
        while let Some(r) = stream_max.message().await? {
            results.push(r);
        }
        assert!(
            results.len() <= 2,
            "gRPC max_results=2 must cap at 2, got {}",
            results.len()
        );
    }

    // ── 19. Empty query returns no results ────────────────────────────────────
    {
        let body: serde_json::Value = http
            .get(format!("{}/api/search", ctx.http_url))
            .query(&[("q", "")])
            .send()
            .await?
            .json()
            .await?;
        assert_eq!(
            body["total_results"].as_u64().unwrap(),
            0,
            "Empty query must return 0 results"
        );
    }

    // ── 20. No-match query returns no results ─────────────────────────────────
    {
        let body: serde_json::Value = http
            .get(format!("{}/api/search", ctx.http_url))
            .query(&[("q", "xyzzy_NOTFOUND_42abc987")])
            .send()
            .await?
            .json()
            .await?;
        assert_eq!(
            body["total_results"].as_u64().unwrap(),
            0,
            "Non-matching query must return 0 results"
        );
    }

    // ── 21. gRPC empty query returns empty stream ─────────────────────────────
    {
        let mut client = CodeSearchClient::connect(ctx.grpc_url.clone()).await?;
        let req = SearchRequest {
            query: "".to_string(),
            max_results: 10,
            include_paths: vec![],
            exclude_paths: vec![],
            is_regex: false,
            symbols_only: false,
        };
        let mut stream_empty = client.search(req).await?.into_inner();
        let mut results = vec![];
        while let Some(r) = stream_empty.message().await? {
            results.push(r);
        }
        assert!(
            results.is_empty(),
            "gRPC empty query must return no results"
        );
    }

    // ── 22. gRPC index RPC ────────────────────────────────────────────────────
    {
        let mut client = CodeSearchClient::connect(ctx.grpc_url.clone()).await?;
        let req = IndexRequest {
            paths: vec![ctx._temp_dir.path().to_string_lossy().to_string()],
        };
        let response = client.index(req).await?.into_inner();
        assert!(
            response.files_indexed > 0,
            "gRPC IndexRequest must report >0 files indexed"
        );
        assert!(
            !response.message.is_empty(),
            "gRPC IndexResponse must have a message"
        );
    }

    // ── 23. Line-number accuracy ──────────────────────────────────────────────
    // "pub struct AuthManager" is on line 4 of CORPUS_AUTH_RS (1-based).
    {
        let body: serde_json::Value = http
            .get(format!("{}/api/search", ctx.http_url))
            .query(&[("q", "pub struct AuthManager")])
            .send()
            .await?
            .json()
            .await?;
        let results = body["results"].as_array().unwrap();
        assert!(!results.is_empty(), "Must find 'pub struct AuthManager'");
        let line = results[0]["line_number"].as_u64().unwrap();
        assert!(
            line > 0,
            "Line numbers must be 1-based and positive, got {line}"
        );
        // Line 4 in the source (comment, blank, blank, struct)
        assert_eq!(line, 4, "AuthManager struct must be on line 4, got {line}");
    }

    // ── 24. Match content is populated ────────────────────────────────────────
    {
        let body: serde_json::Value = http
            .get(format!("{}/api/search", ctx.http_url))
            .query(&[("q", "execute_sql_transaction")])
            .send()
            .await?
            .json()
            .await?;
        let results = body["results"].as_array().unwrap();
        assert!(!results.is_empty(), "Must find 'execute_sql_transaction'");
        let content = results[0]["content"].as_str().unwrap();
        assert!(
            content.contains("execute_sql_transaction"),
            "Result content must contain the matched term, got: {content}"
        );
        assert!(!content.is_empty(), "Result content must not be empty");
    }

    // ── 25. Dependencies endpoint ─────────────────────────────────────────────
    {
        let body: serde_json::Value = http
            .get(format!("{}/api/dependencies", ctx.http_url))
            .query(&[("file", "app.js")])
            .send()
            .await?
            .json()
            .await?;
        assert!(body["file"].as_str().is_some(), "dependencies.file missing");
        assert!(
            body["files"].as_array().is_some(),
            "dependencies.files missing"
        );
        assert!(
            body["count"].as_u64().is_some(),
            "dependencies.count missing"
        );
    }

    // ── 26. Dependents endpoint ───────────────────────────────────────────────
    {
        let body: serde_json::Value = http
            .get(format!("{}/api/dependents", ctx.http_url))
            .query(&[("file", "utils.py")])
            .send()
            .await?
            .json()
            .await?;
        assert!(body["file"].as_str().is_some(), "dependents.file missing");
        assert!(
            body["files"].as_array().is_some(),
            "dependents.files missing"
        );
        assert!(body["count"].as_u64().is_some(), "dependents.count missing");
    }

    // ── 27. Cross-language search ─────────────────────────────────────────────
    // "class" appears in Python and JavaScript files only
    {
        let body: serde_json::Value = http
            .get(format!("{}/api/search", ctx.http_url))
            .query(&[("q", "class"), ("max", "50")])
            .send()
            .await?
            .json()
            .await?;
        let results = body["results"].as_array().unwrap();
        let has_py = results
            .iter()
            .any(|r| r["file_path"].as_str().unwrap().ends_with(".py"));
        let has_js = results
            .iter()
            .any(|r| r["file_path"].as_str().unwrap().ends_with(".js"));
        assert!(
            has_py,
            "Cross-language 'class' search must return Python results"
        );
        assert!(
            has_js,
            "Cross-language 'class' search must return JS results"
        );
    }

    // ── 28. Symbol scoring boost: symbol definitions outrank plain text ───────
    // Searching "AuthManager" should rank auth.rs (struct definition) first
    {
        let body: serde_json::Value = http
            .get(format!("{}/api/search", ctx.http_url))
            .query(&[("q", "AuthManager"), ("max", "10")])
            .send()
            .await?
            .json()
            .await?;
        let results = body["results"].as_array().unwrap();
        assert!(!results.is_empty(), "Must find results for 'AuthManager'");
        // The top result should have a higher score than any non-symbol result
        let top_score = results[0]["score"].as_f64().unwrap();
        assert!(top_score > 0.0, "Top result score must be positive");
        // There must be a SYMBOL_DEFINITION in the top results
        let has_symbol = results
            .iter()
            .any(|r| r["match_type"].as_str().unwrap() == "SYMBOL_DEFINITION");
        assert!(
            has_symbol,
            "Results for 'AuthManager' must include at least one SYMBOL_DEFINITION"
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Index correctness: reload remapping (2.1) and incremental updates (2.2)
// ---------------------------------------------------------------------------

/// 2.1: After a save/reload where one file became stale, the trigram doc ids
/// must be remapped so every remaining file's tokens still resolve to the
/// CORRECT file (regression: stale file shifted all later ids).
#[tokio::test]
async fn test_reload_remaps_trigram_ids_after_stale_file() -> Result<()> {
    use fast_code_search::config::IndexerConfig;
    use fast_code_search::search::SearchEngine;
    use tempfile::TempDir;

    let temp = TempDir::new()?;
    let mut paths = Vec::new();
    for i in 0..5 {
        let p = temp.path().join(format!("file{}.rs", i));
        std::fs::write(&p, format!("fn unique_token_{i}() {{ let x = {i}; }}\n"))?;
        paths.push(p);
    }

    let index_path = temp.path().join("index.bin");
    let config = IndexerConfig {
        paths: vec![temp.path().to_string_lossy().to_string()],
        ..Default::default()
    };

    {
        let mut eng = SearchEngine::new();
        for p in &paths {
            eng.index_file(p)?;
        }
        eng.save_index(&index_path, &config)?;
    }

    // Make file #1 stale (different size => detected as stale on reload).
    std::fs::write(&paths[1], "fn unique_token_1_modified_substantially() {}\n")?;

    let mut eng2 = SearchEngine::new();
    eng2.load_index_with_reconciliation(&index_path, &config)?;

    // Each unchanged file's unique token must resolve to its OWN file.
    for i in [0usize, 2, 3, 4] {
        let results = eng2.search(&format!("unique_token_{i}"), 10);
        assert!(
            results
                .iter()
                .any(|m| m.file_path.ends_with(&format!("file{}.rs", i))),
            "token {i} must map to file{i}.rs after reload; got {:?}",
            results.iter().map(|m| &m.file_path).collect::<Vec<_>>()
        );
    }

    Ok(())
}

/// Roadmap 1.8: directory rename and delete events (the watcher reports only
/// the directory path) must remove every file under the old path and index
/// every eligible file under the new one. Previously both were silent no-ops.
#[tokio::test]
async fn test_directory_rename_and_delete_update_index() -> Result<()> {
    use fast_code_search::config::IndexerConfig;
    use fast_code_search::search::{apply_change, FileChange, SearchEngine};
    use tempfile::TempDir;

    let temp = TempDir::new()?;
    let old_dir = temp.path().join("old_mod");
    std::fs::create_dir_all(old_dir.join("nested"))?;
    std::fs::write(old_dir.join("a.rs"), "fn dir_token_a() {}\n")?;
    std::fs::write(old_dir.join("b.rs"), "fn dir_token_b() {}\n")?;
    std::fs::write(old_dir.join("nested/c.rs"), "fn dir_token_c() {}\n")?;
    let other = temp.path().join("other.rs");
    std::fs::write(&other, "fn other_token() {}\n")?;

    let config = IndexerConfig {
        paths: vec![temp.path().to_string_lossy().to_string()],
        ..Default::default()
    };
    let mut eng = SearchEngine::new();
    for p in [
        old_dir.join("a.rs"),
        old_dir.join("b.rs"),
        old_dir.join("nested/c.rs"),
        other.clone(),
    ] {
        eng.index_file(&p)?;
    }
    assert_eq!(eng.search("dir_token_c", 10).len(), 1);

    // Rename the directory on disk, then apply the event the watcher emits.
    let new_dir = temp.path().join("new_mod");
    std::fs::rename(&old_dir, &new_dir)?;
    let outcome = apply_change(
        &mut eng,
        &FileChange::Renamed {
            from: old_dir.clone(),
            to: new_dir.clone(),
        },
        &config,
    );
    assert_eq!((outcome.removed, outcome.indexed), (3, 3), "{outcome:?}");

    for tok in ["dir_token_a", "dir_token_b", "dir_token_c"] {
        let hits = eng.search(tok, 10);
        assert_eq!(hits.len(), 1, "{tok}: {hits:?}");
        assert!(
            hits[0].file_path.contains("new_mod") && !hits[0].file_path.contains("old_mod"),
            "{tok} must resolve under the new directory: {}",
            hits[0].file_path
        );
    }
    assert_eq!(
        eng.search("other_token", 10).len(),
        1,
        "unrelated file untouched"
    );

    // Delete the directory; the watcher reports only the directory path.
    std::fs::remove_dir_all(&new_dir)?;
    let outcome = apply_change(&mut eng, &FileChange::Deleted(new_dir.clone()), &config);
    assert_eq!(outcome.removed, 3, "{outcome:?}");
    for tok in ["dir_token_a", "dir_token_b", "dir_token_c"] {
        assert!(eng.search(tok, 10).is_empty(), "{tok} must be gone");
    }
    assert_eq!(eng.search("other_token", 10).len(), 1);
    Ok(())
}

/// Roadmap 2.6: imports that were still unresolved at save time survive a
/// checkpoint restore, so a file indexed after the reload still gains its
/// incoming edge.
#[tokio::test]
async fn test_unresolved_imports_survive_reload() -> Result<()> {
    use fast_code_search::config::IndexerConfig;
    use fast_code_search::search::SearchEngine;
    use tempfile::TempDir;

    let temp = TempDir::new()?;
    let main_rs = temp.path().join("main.rs");
    let helper_rs = temp.path().join("helper.rs");
    std::fs::write(&main_rs, "mod helper;\nfn main() {}\n")?;
    let index_path = temp.path().join("index.bin");
    let config = IndexerConfig {
        paths: vec![temp.path().to_string_lossy().to_string()],
        ..Default::default()
    };
    {
        let mut eng = SearchEngine::new();
        eng.index_file(&main_rs)?;
        eng.resolve_imports();
        assert_eq!(eng.waiting_imports_count(), 1);
        eng.save_index(&index_path, &config)?;
    }

    let mut eng2 = SearchEngine::new();
    eng2.load_index_with_reconciliation(&index_path, &config)?;
    assert_eq!(eng2.waiting_imports_count(), 1, "parked import restored");

    std::fs::write(&helper_rs, "pub fn help() {}\n")?;
    eng2.index_file(&helper_rs)?;
    eng2.resolve_imports_incremental();
    let helper_id = eng2.find_file_id(&helper_rs.to_string_lossy()).unwrap();
    let main_id = eng2.find_file_id(&main_rs.to_string_lossy()).unwrap();
    assert_eq!(eng2.get_dependents(helper_id), vec![main_id]);
    assert_eq!(eng2.waiting_imports_count(), 0);
    Ok(())
}

/// Roadmap 2.3: watcher paths are matched by canonical path, never by suffix.
/// A non-canonical spelling of an indexed file must update it in place (no
/// duplicate id, no trigram accumulation), and a different file whose path
/// merely ends with the same suffix must not be mistaken for it.
#[tokio::test]
async fn test_update_file_uses_canonical_exact_match() -> Result<()> {
    use fast_code_search::search::SearchEngine;
    use tempfile::TempDir;

    let temp = TempDir::new()?;
    std::fs::create_dir_all(temp.path().join("sub"))?;
    let file = temp.path().join("sub/main.rs");
    std::fs::write(&file, "fn canon_v1() {}\n")?;
    let mut eng = SearchEngine::new();
    eng.index_file(&file)?;
    let files_before = eng.get_stats().num_files;

    // 50 updates through a non-canonical spelling of the same path.
    let dotted = temp.path().join("sub").join(".").join("main.rs");
    for i in 0..50 {
        std::fs::write(&file, format!("fn canon_v{i}_more() {{}}\n"))?;
        eng.update_file(&dotted)?;
    }
    assert_eq!(eng.get_stats().num_files, files_before, "no duplicate ids");
    assert!(eng.search("canon_v1() ", 10).is_empty(), "old content gone");
    assert_eq!(eng.search("canon_v49_more", 10).len(), 1);

    // A path that only shares a suffix is NOT this file.
    let other_dir = temp.path().join("elsewhere");
    std::fs::create_dir_all(&other_dir)?;
    let other = other_dir.join("main.rs");
    std::fs::write(&other, "fn other_main_token() {}\n")?;
    assert!(
        !eng.remove_file(&other),
        "an unindexed file with a matching suffix must not remove the indexed one"
    );
    assert_eq!(eng.search("canon_v49_more", 10).len(), 1);
    Ok(())
}

/// Roadmap 2.1: a burst of watcher events is coalesced per path and applied
/// as one batch: a rename followed by a modify of the new path indexes the
/// file once, deletes are removed in one pass, and a delete after a modify of
/// the same path wins.
#[tokio::test]
async fn test_apply_changes_batches_and_coalesces() -> Result<()> {
    use fast_code_search::config::IndexerConfig;
    use fast_code_search::search::{apply_changes, FileChange, SearchEngine};
    use tempfile::TempDir;

    let temp = TempDir::new()?;
    let mk = |name: &str, body: &str| -> Result<std::path::PathBuf> {
        let p = temp.path().join(name);
        std::fs::write(&p, body)?;
        Ok(p)
    };
    let a = mk("a.rs", "fn batch_a() {}\n")?;
    let b = mk("b.rs", "fn batch_b() {}\n")?;
    let c = mk("c.rs", "fn batch_c() {}\n")?;
    let d = mk("d.rs", "fn batch_d() {}\n")?;
    let config = IndexerConfig {
        paths: vec![temp.path().to_string_lossy().to_string()],
        ..Default::default()
    };
    let mut eng = SearchEngine::new();
    for p in [&a, &b, &c, &d] {
        eng.index_file(p)?;
    }
    eng.finalize();

    // Burst: delete a and b; rename c -> e then "modify" e; modify d then delete d.
    let e = temp.path().join("e.rs");
    std::fs::rename(&c, &e)?;
    std::fs::write(&e, "fn batch_e_new() {}\n")?;
    std::fs::remove_file(&a)?;
    std::fs::remove_file(&b)?;
    std::fs::remove_file(&d)?;
    let changes = vec![
        FileChange::Deleted(a.clone()),
        FileChange::Modified(d.clone()),
        FileChange::Renamed {
            from: c.clone(),
            to: e.clone(),
        },
        FileChange::Modified(e.clone()),
        FileChange::Deleted(b.clone()),
        FileChange::Deleted(d.clone()),
    ];
    let outcome = apply_changes(&mut eng, &changes, &config);
    assert_eq!(outcome.removed, 4, "{outcome:?}"); // a, b, c(old), d
    assert_eq!(outcome.indexed, 1, "{outcome:?}"); // e, once

    for gone in ["batch_a", "batch_b", "batch_c", "batch_d"] {
        assert!(eng.search(gone, 10).is_empty(), "{gone} must be gone");
    }
    let hits = eng.search("batch_e_new", 10);
    assert_eq!(hits.len(), 1);
    assert!(hits[0].file_path.ends_with("e.rs"));
    Ok(())
}

/// Roadmap 1.8 / 2.4: the watcher applies the same eligibility rules as the
/// initial build. A changed file with a non-included extension is not indexed
/// and an excluded path is dropped.
#[tokio::test]
async fn test_watcher_change_respects_eligibility() -> Result<()> {
    use fast_code_search::config::IndexerConfig;
    use fast_code_search::search::{apply_change, FileChange, SearchEngine};
    use tempfile::TempDir;

    let temp = TempDir::new()?;
    let log = temp.path().join("run.log");
    std::fs::write(&log, "log_only_token\n")?;
    let rs = temp.path().join("keep.rs");
    std::fs::write(&rs, "fn keep_token() {}\n")?;

    let config = IndexerConfig {
        paths: vec![temp.path().to_string_lossy().to_string()],
        include_extensions: vec!["rs".to_string()],
        ..Default::default()
    };
    let mut eng = SearchEngine::new();
    let o = apply_change(&mut eng, &FileChange::Modified(log.clone()), &config);
    assert!(
        !o.changed(),
        "non-included extension must not be indexed: {o:?}"
    );
    assert!(eng.search("log_only_token", 10).is_empty());
    let o = apply_change(&mut eng, &FileChange::Modified(rs.clone()), &config);
    assert_eq!(o.indexed, 1);
    assert_eq!(eng.search("keep_token", 10).len(), 1);
    Ok(())
}

/// Roadmap 1.4: the persisted mtime/size must describe the content that was
/// indexed, not the on-disk state at save time. A file edited between indexing
/// and saving must be reported stale on reload (previously the fresh stat was
/// persisted alongside the old trigrams and the edit was never detected).
#[tokio::test]
async fn test_edit_between_index_and_save_is_detected_as_stale() -> Result<()> {
    use fast_code_search::config::IndexerConfig;
    use fast_code_search::search::SearchEngine;
    use tempfile::TempDir;

    let temp = TempDir::new()?;
    let file = temp.path().join("edited.rs");
    std::fs::write(&file, "fn before_token() {}\n")?;
    let index_path = temp.path().join("index.bin");
    let config = IndexerConfig {
        paths: vec![temp.path().to_string_lossy().to_string()],
        ..Default::default()
    };

    {
        let mut eng = SearchEngine::new();
        eng.index_file(&file)?;
        // Edit AFTER indexing, BEFORE saving (different size so the check does
        // not depend on second-granularity mtimes).
        std::fs::write(&file, "fn after_token_with_longer_name() {}\n")?;
        eng.save_index(&index_path, &config)?;
    }

    let mut eng2 = SearchEngine::new();
    let result = eng2.load_index_with_reconciliation(&index_path, &config)?;
    assert!(
        result.stale_files.iter().any(|p| p.ends_with("edited.rs")),
        "edited file must be reported stale; got stale={:?}",
        result.stale_files
    );
    Ok(())
}

/// Roadmap 1.1 (P0): saving after `remove_file` must persist a consistent
/// index. The file table is compacted over tombstones, so trigram bitmaps,
/// symbols and dependency edges must be remapped onto positions; otherwise
/// every file after the removed one is misattributed (or lost) on reload.
#[tokio::test]
async fn test_save_after_remove_keeps_ids_consistent() -> Result<()> {
    use fast_code_search::config::IndexerConfig;
    use fast_code_search::search::SearchEngine;
    use tempfile::TempDir;

    let temp = TempDir::new()?;
    let mut paths = Vec::new();
    for i in 0..4 {
        let p = temp.path().join(format!("file{}.rs", i));
        std::fs::write(&p, format!("fn unique_token_{i}() {{ let x = {i}; }}\n"))?;
        paths.push(p);
    }
    let index_path = temp.path().join("index.bin");
    let config = IndexerConfig {
        paths: vec![temp.path().to_string_lossy().to_string()],
        ..Default::default()
    };

    {
        let mut eng = SearchEngine::new();
        for p in &paths {
            eng.index_file(p)?;
        }
        // Remove the middle file (id 1) the way the watcher does, then save.
        assert!(eng.remove_file(&paths[1]));
        std::fs::remove_file(&paths[1])?;
        eng.save_index(&index_path, &config)?;
    }

    let mut eng2 = SearchEngine::new();
    eng2.load_index_with_reconciliation(&index_path, &config)?;

    for i in [0usize, 2, 3] {
        let results = eng2.search(&format!("unique_token_{i}"), 10);
        let got: Vec<&str> = results.iter().map(|m| m.file_path.as_str()).collect();
        assert_eq!(
            got.len(),
            1,
            "token {i} must hit exactly one file after reload; got {got:?}"
        );
        assert!(
            got[0].ends_with(&format!("file{i}.rs")),
            "token {i} must map to file{i}.rs after reload; got {got:?}"
        );
        // Symbols were persisted in the same compacted order.
        let syms = eng2.search_symbols(&format!("unique_token_{i}"), "", "", 10)?;
        assert!(
            syms.iter()
                .any(|m| m.file_path.ends_with(&format!("file{i}.rs"))),
            "symbol unique_token_{i} must resolve to file{i}.rs; got {:?}",
            syms.iter().map(|m| &m.file_path).collect::<Vec<_>>()
        );
    }
    // The removed file must not come back.
    assert!(eng2.search("unique_token_1", 10).is_empty());

    Ok(())
}

/// 2.2: Modifying, deleting, and renaming files updates the index so searches
/// reflect only the current on-disk content.
#[tokio::test]
async fn test_incremental_update_remove_rename() -> Result<()> {
    use fast_code_search::search::SearchEngine;
    use tempfile::TempDir;

    // Replace a file's content via write-temp-then-rename, mimicking how editors
    // (and the watcher's intended flow) modify files. A plain in-place truncate
    // fails on Windows while the engine holds a memory map on the file.
    fn atomic_write(path: &std::path::Path, content: &str) -> Result<()> {
        let tmp = path.with_extension("tmp_write");
        std::fs::write(&tmp, content)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    let temp = TempDir::new()?;
    let a = temp.path().join("a.rs");
    let b = temp.path().join("b.rs");
    std::fs::write(&a, "fn alpha_token() {}\n")?;
    std::fs::write(&b, "fn beta_token() {}\n")?;

    let mut eng = SearchEngine::new();
    eng.index_file(&a)?;
    eng.index_file(&b)?;

    assert!(!eng.search("alpha_token", 10).is_empty());
    assert!(!eng.search("beta_token", 10).is_empty());

    // Modify a.rs: replace its content entirely.
    atomic_write(&a, "fn gamma_token() {}\n")?;
    eng.update_file(&a)?;
    assert!(
        eng.search("alpha_token", 10).is_empty(),
        "old content must be gone after update"
    );
    assert!(
        !eng.search("gamma_token", 10).is_empty(),
        "new content must be searchable after update"
    );

    // Delete b.rs.
    assert!(
        eng.remove_file(&b),
        "remove_file should find and remove b.rs"
    );
    assert!(
        eng.search("beta_token", 10).is_empty(),
        "deleted file must not match"
    );

    // Rename a.rs -> c.rs (delete old path, index new).
    let c = temp.path().join("c.rs");
    std::fs::rename(&a, &c)?;
    eng.remove_file(&a);
    eng.update_file(&c)?;
    let res = eng.search("gamma_token", 10);
    assert!(
        res.iter().any(|m| m.file_path.ends_with("c.rs")),
        "renamed file must be searchable under its new path; got {:?}",
        res.iter().map(|m| &m.file_path).collect::<Vec<_>>()
    );
    assert!(
        !res.iter().any(|m| m.file_path.ends_with("a.rs")),
        "old path must no longer appear"
    );

    Ok(())
}

/// 3.1: regex alternation must return complete results — a file that contains
/// only one branch of `hello|world` must still be found (regression: the old
/// single-best-literal pre-filter dropped it).
#[tokio::test]
async fn test_regex_alternation_returns_all_branches() -> Result<()> {
    use fast_code_search::search::SearchEngine;
    use tempfile::TempDir;

    let temp = TempDir::new()?;
    let only_hello = temp.path().join("h.rs");
    let only_world = temp.path().join("w.rs");
    std::fs::write(&only_hello, "fn hello_here() {}\n")?;
    std::fs::write(&only_world, "fn world_here() {}\n")?;

    let mut eng = SearchEngine::new();
    eng.index_file(&only_hello)?;
    eng.index_file(&only_world)?;

    let results = eng.search_regex("hello|world", "", "", 50)?;
    assert!(
        results.iter().any(|m| m.file_path.ends_with("h.rs")),
        "must find the file containing only 'hello'"
    );
    assert!(
        results.iter().any(|m| m.file_path.ends_with("w.rs")),
        "must find the file containing only 'world' (regression)"
    );

    Ok(())
}
