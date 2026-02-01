use crate::config::IndexerConfig;
use crate::search::SearchEngine;
use anyhow::Result;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{debug, info, warn};
use walkdir::WalkDir;

// Include the generated protobuf code
pub mod search_proto {
    tonic::include_proto!("search");
}

use search_proto::{
    code_search_server::{CodeSearch, CodeSearchServer},
    IndexRequest, IndexResponse, MatchType, SearchRequest, SearchResult,
};

pub struct CodeSearchService {
    engine: Arc<RwLock<SearchEngine>>,
}

impl CodeSearchService {
    pub fn new() -> Self {
        Self {
            engine: Arc::new(RwLock::new(SearchEngine::new())),
        }
    }

    /// Create a service with an existing shared engine
    pub fn with_engine(engine: Arc<RwLock<SearchEngine>>) -> Self {
        Self { engine }
    }

    /// Get the shared engine reference
    pub fn engine(&self) -> Arc<RwLock<SearchEngine>> {
        Arc::clone(&self.engine)
    }

    /// Create a new service and perform initial indexing based on config
    pub fn new_with_indexing(indexer_config: &IndexerConfig) -> Self {
        let service = Self::new();

        if indexer_config.paths.is_empty() {
            info!("No paths configured for auto-indexing");
            return service;
        }

        let total_start = Instant::now();
        info!(
            "Starting auto-indexing of {} path(s)",
            indexer_config.paths.len()
        );

        let mut total_files = 0u64;
        let mut total_size = 0u64;

        for path_str in &indexer_config.paths {
            let path = Path::new(path_str);
            if !path.exists() {
                warn!(path = %path_str, "Configured path does not exist, skipping");
                continue;
            }

            info!(path = %path_str, "Indexing path");
            let path_start = Instant::now();
            let mut path_files = 0u64;
            let mut path_size = 0u64;

            let mut engine = service.engine.write().unwrap();

            for entry in WalkDir::new(path)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if !entry.file_type().is_file() {
                    continue;
                }

                let entry_path = entry.path();

                // Check exclude patterns
                let path_str_check = entry_path.to_string_lossy();
                let should_exclude = indexer_config.exclude_patterns.iter().any(|pattern| {
                    glob::Pattern::new(pattern)
                        .map(|p| p.matches(&path_str_check))
                        .unwrap_or(false)
                        || path_str_check.contains(pattern.trim_matches('*').trim_matches('/'))
                });

                if should_exclude {
                    debug!(path = %entry_path.display(), "Excluded by pattern");
                    continue;
                }

                // Skip binary files
                if let Some(ext) = entry_path.extension() {
                    let ext = ext.to_string_lossy().to_lowercase();
                    if matches!(
                        ext.as_str(),
                        "exe"
                            | "so"
                            | "dylib"
                            | "dll"
                            | "bin"
                            | "o"
                            | "a"
                            | "lib"
                            | "png"
                            | "jpg"
                            | "jpeg"
                            | "gif"
                            | "ico"
                            | "bmp"
                            | "zip"
                            | "tar"
                            | "gz"
                            | "7z"
                            | "rar"
                            | "pdf"
                            | "doc"
                            | "docx"
                    ) {
                        continue;
                    }
                }

                // Check file size
                if let Ok(metadata) = entry.metadata() {
                    if metadata.len() > indexer_config.max_file_size {
                        debug!(
                            path = %entry_path.display(),
                            size = metadata.len(),
                            max = indexer_config.max_file_size,
                            "File too large, skipping"
                        );
                        continue;
                    }
                    path_size += metadata.len();
                }

                match engine.index_file(entry_path) {
                    Ok(_) => {
                        path_files += 1;
                    }
                    Err(e) => {
                        debug!(path = %entry_path.display(), error = %e, "Failed to index file");
                    }
                }
            }

            drop(engine);

            let path_duration = path_start.elapsed();
            info!(
                path = %path_str,
                files = path_files,
                size_mb = format!("{:.2}", path_size as f64 / 1_048_576.0),
                duration_secs = format!("{:.2}", path_duration.as_secs_f64()),
                "Completed indexing path"
            );

            total_files += path_files;
            total_size += path_size;
        }

        let engine = service.engine.read().unwrap();
        let stats = engine.get_stats();
        drop(engine);

        let total_duration = total_start.elapsed();
        info!(
            total_files = total_files,
            total_size_mb = format!("{:.2}", total_size as f64 / 1_048_576.0),
            trigrams = stats.num_trigrams,
            duration_secs = format!("{:.2}", total_duration.as_secs_f64()),
            "Auto-indexing complete"
        );

        service
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
        let max_results = req.max_results.clamp(1, 1000) as usize;
        let include_patterns = req.include_paths.join(";");
        let exclude_patterns = req.exclude_paths.join(";");
        let is_regex = req.is_regex;

        // Use read lock for concurrent search access
        let engine = self.engine.read().unwrap();
        
        // Choose search method based on regex flag
        let matches = if is_regex {
            // Use regex search with optional path filtering
            engine
                .search_regex(&query, &include_patterns, &exclude_patterns, max_results)
                .map_err(|e| Status::invalid_argument(format!("Invalid regex pattern: {}", e)))?
        } else if include_patterns.is_empty() && exclude_patterns.is_empty() {
            // Plain text search without filtering
            engine.search(&query, max_results)
        } else {
            // Plain text search with path filtering
            engine
                .search_with_filter(&query, &include_patterns, &exclude_patterns, max_results)
                .map_err(|e| Status::invalid_argument(format!("Invalid filter pattern: {}", e)))?
        };
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
        info!(paths = ?req.paths, "Received index request");
        let start = Instant::now();
        // Use write lock for indexing operations
        let mut engine = self.engine.write().unwrap();

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

        let duration = start.elapsed();
        info!(
            files = files_indexed,
            size_bytes = total_size,
            trigrams = stats.num_trigrams,
            duration_secs = format!("{:.2}", duration.as_secs_f64()),
            "Index request completed"
        );

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

pub fn create_server_with_indexing(
    indexer_config: &IndexerConfig,
) -> CodeSearchServer<CodeSearchService> {
    CodeSearchServer::new(CodeSearchService::new_with_indexing(indexer_config))
}

/// Create a shared engine with indexing, returns the Arc for sharing with web server
pub fn create_indexed_engine(indexer_config: &IndexerConfig) -> Arc<RwLock<SearchEngine>> {
    let service = CodeSearchService::new_with_indexing(indexer_config);
    service.engine()
}

/// Create gRPC server with an existing shared engine
pub fn create_server_with_engine(
    engine: Arc<RwLock<SearchEngine>>,
) -> CodeSearchServer<CodeSearchService> {
    CodeSearchServer::new(CodeSearchService::with_engine(engine))
}
