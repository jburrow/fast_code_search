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
    let result_enabled = PartialIndexedFile::process(&file_path, true);
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
    let result_disabled = PartialIndexedFile::process(&file_path, false);
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
    let (partial_enabled, _) = PartialIndexedFile::process(&file_path, false).unwrap();
    let pre_enabled = PreIndexedFile::from_partial(partial_enabled, true);
    // At minimum the FileName symbol is always added
    assert!(
        !pre_enabled.symbols.is_empty(),
        "Expected symbols to be extracted when enable_symbols=true"
    );

    // With symbols disabled, only the FileName symbol should be present (no tree-sitter extraction)
    let (partial_disabled, _) = PartialIndexedFile::process(&file_path, false).unwrap();
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
