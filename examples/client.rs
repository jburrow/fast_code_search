use anyhow::Result;
use fast_code_search::server::search_proto::{
    code_search_client::CodeSearchClient, IndexRequest, SearchRequest,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Connect to the gRPC server
    let mut client = CodeSearchClient::connect("http://127.0.0.1:50051").await?;

    // Index a directory
    println!("Indexing current directory...");
    let index_request = IndexRequest {
        paths: vec![".".to_string()],
    };

    let response = client.index(index_request).await?;
    let index_response = response.into_inner();
    println!("Index complete: {}", index_response.message);

    // Search for a query
    println!("\nSearching for 'SearchEngine'...");
    let search_request = SearchRequest {
        query: "SearchEngine".to_string(),
        max_results: 10,
    };

    let mut stream = client.search(search_request).await?.into_inner();

    // Stream results
    let mut count = 0;
    while let Some(result) = stream.message().await? {
        count += 1;
        println!("\n[{}] {}:{}", count, result.file_path, result.line_number);
        println!("  Score: {:.2}", result.score);
        println!("  Content: {}", result.content.trim());
    }

    println!("\nFound {} results", count);
    Ok(())
}
