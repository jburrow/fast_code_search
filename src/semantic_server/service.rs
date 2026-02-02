//! gRPC service implementation for semantic code search

use crate::semantic::SemanticSearchEngine;
use std::path::Path;
use std::sync::{Arc, RwLock};
use tonic::{Request, Response, Status};

pub mod semantic_search {
    tonic::include_proto!("semantic_search");
}

use semantic_search::{
    semantic_code_search_server::SemanticCodeSearch, ChunkType, ReloadIndexRequest,
    ReloadIndexResponse, SemanticSearchRequest, SemanticSearchResult, StatsRequest, StatsResponse,
};

/// gRPC service for semantic code search
pub struct SemanticSearchService {
    engine: Arc<RwLock<SemanticSearchEngine>>,
}

impl SemanticSearchService {
    pub fn new(engine: Arc<RwLock<SemanticSearchEngine>>) -> Self {
        Self { engine }
    }
}

#[tonic::async_trait]
impl SemanticCodeSearch for SemanticSearchService {
    type SearchStream =
        tokio_stream::wrappers::ReceiverStream<Result<SemanticSearchResult, Status>>;

    async fn search(
        &self,
        request: Request<SemanticSearchRequest>,
    ) -> Result<Response<Self::SearchStream>, Status> {
        let req = request.into_inner();
        let query = req.query;
        let max_results = req.max_results.clamp(1, 100) as usize; // Clamp between 1-100

        // Perform search
        let results = {
            let mut engine = self
                .engine
                .write()
                .map_err(|e| Status::internal(format!("Lock error: {}", e)))?;

            engine
                .search(&query, max_results)
                .map_err(|e| Status::internal(format!("Search error: {}", e)))?
        };

        // Create streaming response
        let (tx, rx) = tokio::sync::mpsc::channel(128);

        tokio::spawn(async move {
            for result in results {
                let (chunk_type, symbol_name) = match &result.chunk.chunk_type {
                    crate::semantic::ChunkType::Fixed => (ChunkType::Fixed as i32, String::new()),
                    crate::semantic::ChunkType::Function(name) => {
                        (ChunkType::Function as i32, name.clone())
                    }
                    crate::semantic::ChunkType::Class(name) => {
                        (ChunkType::Class as i32, name.clone())
                    }
                    crate::semantic::ChunkType::Module => (ChunkType::Module as i32, String::new()),
                };

                let grpc_result = SemanticSearchResult {
                    file_path: result.chunk.file_path,
                    content: result.chunk.text,
                    start_line: result.chunk.start_line as i32,
                    end_line: result.chunk.end_line as i32,
                    similarity_score: result.similarity_score,
                    chunk_type,
                    symbol_name,
                };

                if tx.send(Ok(grpc_result)).await.is_err() {
                    break; // Client disconnected
                }
            }
        });

        Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(
            rx,
        )))
    }

    async fn get_stats(
        &self,
        _request: Request<StatsRequest>,
    ) -> Result<Response<StatsResponse>, Status> {
        let stats = {
            let engine = self
                .engine
                .read()
                .map_err(|e| Status::internal(format!("Lock error: {}", e)))?;

            engine.get_stats()
        };

        let response = StatsResponse {
            num_files: stats.num_files as i32,
            num_chunks: stats.num_chunks as i32,
            embedding_dim: stats.embedding_dim as i32,
            cache_size: stats.cache_size as i32,
        };

        Ok(Response::new(response))
    }

    async fn reload_index(
        &self,
        request: Request<ReloadIndexRequest>,
    ) -> Result<Response<ReloadIndexResponse>, Status> {
        let req = request.into_inner();
        let index_path = req.index_path;

        if index_path.is_empty() {
            return Err(Status::invalid_argument("index_path cannot be empty"));
        }

        let result = {
            let mut engine = self
                .engine
                .write()
                .map_err(|e| Status::internal(format!("Lock error: {}", e)))?;

            engine.load_index(Path::new(&index_path))
        };

        match result {
            Ok(_) => {
                let stats = {
                    let engine = self
                        .engine
                        .read()
                        .map_err(|e| Status::internal(format!("Lock error: {}", e)))?;
                    engine.get_stats()
                };

                Ok(Response::new(ReloadIndexResponse {
                    success: true,
                    message: format!("Index loaded successfully from {}", index_path),
                    num_chunks: stats.num_chunks as i32,
                }))
            }
            Err(e) => Ok(Response::new(ReloadIndexResponse {
                success: false,
                message: format!("Failed to load index: {}", e),
                num_chunks: 0,
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic::SemanticSearchEngine;

    #[tokio::test]
    async fn test_get_stats() {
        let engine = SemanticSearchEngine::new(10, 2);
        let service = SemanticSearchService::new(Arc::new(RwLock::new(engine)));

        let request = Request::new(StatsRequest {});
        let response = service.get_stats(request).await.unwrap();

        let stats = response.into_inner();
        assert_eq!(stats.num_files, 0);
        assert_eq!(stats.num_chunks, 0);
        assert!(stats.embedding_dim > 0);
    }
}
