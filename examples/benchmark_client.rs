//! Benchmark client for testing fast_code_search with large corpuses
//!
//! Usage:
//!   cargo run --example benchmark_client -- [corpus_dir]
//!
//! If corpus_dir is not specified, it defaults to ./test_corpus

use anyhow::Result;
use fast_code_search::server::search_proto::{
    code_search_client::CodeSearchClient, IndexRequest, SearchRequest,
};
use std::env;
use std::path::PathBuf;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    // Default to test_corpus directory
    let corpus_dir = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("./test_corpus")
    };

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║       Fast Code Search - Benchmark Client                  ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    // Connect to the gRPC server
    println!("Connecting to server at http://127.0.0.1:50051...");
    let mut client = CodeSearchClient::connect("http://127.0.0.1:50051").await?;
    println!("✓ Connected!\n");

    // Collect paths to index
    let mut paths_to_index: Vec<String> = Vec::new();

    if corpus_dir.exists() {
        for entry in std::fs::read_dir(&corpus_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                paths_to_index.push(path.to_string_lossy().to_string());
            }
        }
    }

    if paths_to_index.is_empty() {
        println!("⚠ No directories found in {:?}", corpus_dir);
        println!("  Run scripts/setup_test_corpus.bat first to clone test repos.");
        println!("  Or specify a directory: cargo run --example benchmark_client -- /path/to/code");
        return Ok(());
    }

    println!("Found {} directories to index:", paths_to_index.len());
    for path in &paths_to_index {
        println!("  • {}", path);
    }
    println!();

    // ========================================================================
    // Benchmark: Indexing
    // ========================================================================
    println!("┌────────────────────────────────────────────────────────────┐");
    println!("│ BENCHMARK: Indexing                                        │");
    println!("└────────────────────────────────────────────────────────────┘");

    let index_start = Instant::now();

    let index_request = IndexRequest {
        paths: paths_to_index.clone(),
    };

    let response = client.index(index_request).await?;
    let index_response = response.into_inner();

    let index_duration = index_start.elapsed();

    println!("✓ Indexing complete!");
    println!("  • Files indexed: {}", index_response.files_indexed);
    println!(
        "  • Total size: {} bytes ({:.2} MB)",
        index_response.total_size,
        index_response.total_size as f64 / 1_048_576.0
    );
    println!("  • Time: {:.2?}", index_duration);
    println!(
        "  • Throughput: {:.2} MB/s",
        (index_response.total_size as f64 / 1_048_576.0) / index_duration.as_secs_f64()
    );
    println!("  • Message: {}", index_response.message);
    println!();

    // ========================================================================
    // Benchmark: Search queries
    // ========================================================================
    println!("┌────────────────────────────────────────────────────────────┐");
    println!("│ BENCHMARK: Search Queries                                  │");
    println!("└────────────────────────────────────────────────────────────┘");

    let test_queries = vec![
        // Common patterns
        ("fn main", 50),
        ("class Error", 100),
        ("import os", 50),
        ("async fn", 50),
        ("def __init__", 50),
        // More specific
        ("HashMap", 30),
        ("println!", 30),
        ("TypeError", 30),
        // Complex patterns
        ("pub struct", 50),
        ("export default", 30),
    ];

    let mut total_search_time = std::time::Duration::ZERO;
    let mut total_results = 0;

    for (query, max_results) in &test_queries {
        let search_start = Instant::now();

        let search_request = SearchRequest {
            query: query.to_string(),
            max_results: *max_results,
        };

        let mut stream = client.search(search_request).await?.into_inner();

        let mut count = 0;
        while let Some(_result) = stream.message().await? {
            count += 1;
        }

        let search_duration = search_start.elapsed();
        total_search_time += search_duration;
        total_results += count;

        println!(
            "  Query: {:20} | Results: {:4} | Time: {:>10.2?}",
            format!("\"{}\"", query),
            count,
            search_duration
        );
    }

    println!();
    println!("Search Summary:");
    println!("  • Total queries: {}", test_queries.len());
    println!("  • Total results: {}", total_results);
    println!("  • Total time: {:.2?}", total_search_time);
    println!(
        "  • Avg query time: {:.2?}",
        total_search_time / test_queries.len() as u32
    );
    println!();

    // ========================================================================
    // Summary
    // ========================================================================
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║ BENCHMARK COMPLETE                                         ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!("  Index time:      {:.2?}", index_duration);
    println!(
        "  Search time:     {:.2?} (total for {} queries)",
        total_search_time,
        test_queries.len()
    );
    println!("  Files indexed:   {}", index_response.files_indexed);
    println!(
        "  Corpus size:     {:.2} MB",
        index_response.total_size as f64 / 1_048_576.0
    );

    Ok(())
}
