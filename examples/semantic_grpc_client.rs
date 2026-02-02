//! Example gRPC client for semantic search
//!
//! Demonstrates how to connect to the semantic search gRPC service
//! and perform searches.
//!
//! Usage:
//!   cargo run --example semantic_grpc_client "your search query"

use fast_code_search::semantic_server::service::semantic_search::{
    semantic_code_search_client::SemanticCodeSearchClient, SemanticSearchRequest, StatsRequest,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let query = if args.len() > 1 {
        args[1].clone()
    } else {
        "authentication".to_string()
    };

    println!("Connecting to semantic search service at http://127.0.0.1:50052");

    // Connect to the server
    let mut client = SemanticCodeSearchClient::connect("http://127.0.0.1:50052").await?;

    // Get stats first
    println!("\nFetching server stats...");
    let stats_response = client.get_stats(StatsRequest {}).await?;
    let stats = stats_response.into_inner();

    println!("Server Statistics:");
    println!("  - Files indexed: {}", stats.num_files);
    println!("  - Code chunks: {}", stats.num_chunks);
    println!("  - Embedding dimension: {}", stats.embedding_dim);
    println!("  - Cache size: {}", stats.cache_size);

    // Perform search
    println!("\nSearching for: \"{}\"", query);
    let request = SemanticSearchRequest {
        query: query.clone(),
        max_results: 10,
    };

    let mut stream = client.search(request).await?.into_inner();

    println!("\nSearch Results:");
    println!("{}", "=".repeat(80));

    let mut result_count = 0;
    while let Some(result) = stream.message().await? {
        result_count += 1;
        println!("\nResult #{} (Score: {:.4})", result_count, result.similarity_score);
        println!("File: {}", result.file_path);
        println!("Lines: {}-{}", result.start_line, result.end_line);
        
        if !result.symbol_name.is_empty() {
            println!("Symbol: {}", result.symbol_name);
        }
        
        println!("\nContent:");
        println!("{}", "-".repeat(80));
        
        // Show first 5 lines of content
        let lines: Vec<&str> = result.content.lines().take(5).collect();
        for line in lines {
            println!("{}", line);
        }
        
        if result.content.lines().count() > 5 {
            println!("... ({} more lines)", result.content.lines().count() - 5);
        }
        
        println!("{}", "-".repeat(80));
    }

    if result_count == 0 {
        println!("\nNo results found for query: \"{}\"", query);
        println!("Make sure the server has indexed some files.");
    } else {
        println!("\nTotal results: {}", result_count);
    }

    Ok(())
}
