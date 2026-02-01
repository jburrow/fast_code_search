mod index;
mod search;
mod server;
mod symbols;

use anyhow::Result;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<()> {
    let addr = "0.0.0.0:50051".parse()?;
    let search_service = server::create_server();

    println!("Fast Code Search Server listening on {}", addr);
    println!("Ready to index and search code!");
    println!("gRPC endpoint: grpc://{}", addr);

    Server::builder()
        .add_service(search_service)
        .serve(addr)
        .await?;

    Ok(())
}

