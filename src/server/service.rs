use crate::search::SearchEngine;
use anyhow::Result;
use std::sync::{Arc, Mutex};
use tonic::{Request, Response, Status};
use tokio_stream::wrappers::ReceiverStream;
use walkdir::WalkDir;

// Include the generated protobuf code
pub mod search_proto {
    tonic::include_proto!("search");
}

use search_proto::{
    code_search_server::{CodeSearch, CodeSearchServer},
    IndexRequest, IndexResponse, SearchRequest, SearchResult, MatchType,
};

pub struct CodeSearchService {
    engine: Arc<Mutex<SearchEngine>>,
}

impl CodeSearchService {
    pub fn new() -> Self {
        Self {
            engine: Arc::new(Mutex::new(SearchEngine::new())),
        }
    }
}

impl Default for CodeSearchService {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl CodeSearch for CodeSearchService {
    type SearchStream = ReceiverStream<Result<SearchResult, Status>>;

    async fn search(
        &self,
        request: Request<SearchRequest>,
    ) -> Result<Response<Self::SearchStream>, Status> {
        let req = request.into_inner();
        let query = req.query;
        let max_results = req.max_results.max(1).min(1000) as usize;

        let engine = self.engine.lock().unwrap();
        let matches = engine.search(&query, max_results);
        drop(engine); // Release lock before streaming

        let (tx, rx) = tokio::sync::mpsc::channel(128);

        // Spawn a task to stream results
        tokio::spawn(async move {
            for m in matches {
                let match_type = if m.is_symbol {
                    MatchType::SymbolDefinition
                } else {
                    MatchType::Text
                };

                let result = SearchResult {
                    file_path: m.file_path,
                    content: m.content,
                    line_number: m.line_number as i32,
                    score: m.score,
                    match_type: match_type as i32,
                };

                if tx.send(Ok(result)).await.is_err() {
                    break;
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn index(
        &self,
        request: Request<IndexRequest>,
    ) -> Result<Response<IndexResponse>, Status> {
        let req = request.into_inner();
        let mut engine = self.engine.lock().unwrap();

        let mut files_indexed = 0;
        let mut total_size = 0u64;

        for path in req.paths {
            // Walk the directory and index all files
            for entry in WalkDir::new(&path)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file() {
                    // Skip binary files and common non-text extensions
                    if let Some(ext) = entry.path().extension() {
                        let ext = ext.to_string_lossy().to_lowercase();
                        if matches!(
                            ext.as_str(),
                            "exe" | "so" | "dylib" | "dll" | "bin" | "o" | "a"
                        ) {
                            continue;
                        }
                    }

                    match engine.index_file(entry.path()) {
                        Ok(_) => {
                            files_indexed += 1;
                            if let Ok(metadata) = entry.metadata() {
                                total_size += metadata.len();
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to index {}: {}", entry.path().display(), e);
                        }
                    }
                }
            }
        }

        let stats = engine.get_stats();
        drop(engine);

        Ok(Response::new(IndexResponse {
            files_indexed,
            total_size: total_size as i64,
            message: format!(
                "Indexed {} files ({} bytes, {} trigrams)",
                files_indexed, total_size, stats.num_trigrams
            ),
        }))
    }
}

pub fn create_server() -> CodeSearchServer<CodeSearchService> {
    CodeSearchServer::new(CodeSearchService::new())
}
