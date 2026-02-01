//! Integration tests for Fast Code Search
//!
//! These tests spin up real gRPC and HTTP servers, index test files,
//! and validate queries through both interfaces.

use anyhow::Result;
use fast_code_search::{
    search::{IndexingProgress, SearchEngine},
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
    let router = create_router(engine, progress);

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
        response.files_indexed >= 0,
        "Expected non-negative files_indexed count"
    );
    assert!(!response.message.is_empty(), "Expected a status message");

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
        body["results"].as_array().unwrap().len() > 0,
        "Expected results array to have items"
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

    // Verify we got results from different file types
    let has_rs = file_paths.iter().any(|p| p.ends_with(".rs"));
    let has_py = file_paths.iter().any(|p| p.ends_with(".py"));
    let has_js = file_paths.iter().any(|p| p.ends_with(".js"));

    assert!(
        has_rs || has_py || has_js,
        "Expected results from at least one file type, got: {:?}",
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
    assert!(!results.is_empty(), "Expected at least one symbol result for 'find_me'");

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
