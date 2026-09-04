use crate::config::IndexerConfig;
use crate::search::{RankMode, SearchEngine, SearchLimits};
use anyhow::Result;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{info, warn};

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
    /// When set, the `Index` RPC only accepts paths under one of these
    /// (canonical) roots. `None` means unrestricted (library/test use).
    allowed_index_roots: Option<Vec<std::path::PathBuf>>,
    /// Eligibility rules (excludes, extensions, size, gitignore, batch size)
    /// for the `Index` RPC; defaults when the service was built without one.
    indexer_config: Option<IndexerConfig>,
}

impl CodeSearchService {
    pub fn new() -> Self {
        Self {
            engine: Arc::new(RwLock::new(SearchEngine::new())),
            allowed_index_roots: None,
            indexer_config: None,
        }
    }

    /// Create a service with an existing shared engine
    pub fn with_engine(engine: Arc<RwLock<SearchEngine>>) -> Self {
        Self {
            engine,
            allowed_index_roots: None,
            indexer_config: None,
        }
    }

    /// Like [`Self::with_engine`], but the `Index` RPC only accepts paths under
    /// `roots` (canonicalized here; non-existent roots are kept as given).
    pub fn with_engine_scoped(
        engine: Arc<RwLock<SearchEngine>>,
        roots: Vec<std::path::PathBuf>,
    ) -> Self {
        let roots = roots
            .into_iter()
            .map(|r| crate::search::engine::canonicalize_lossy(&r))
            .collect();
        Self {
            engine,
            allowed_index_roots: Some(roots),
            indexer_config: None,
        }
    }

    /// The shipped configuration: `Index` scoped to `config.paths` and
    /// applying the same eligibility rules as the initial build.
    pub fn with_engine_config(engine: Arc<RwLock<SearchEngine>>, config: &IndexerConfig) -> Self {
        let roots = config.paths.iter().map(std::path::PathBuf::from).collect();
        let mut s = Self::with_engine_scoped(engine, roots);
        s.indexer_config = Some(config.clone());
        s
    }

    /// Get the shared engine reference
    pub fn engine(&self) -> Arc<RwLock<SearchEngine>> {
        Arc::clone(&self.engine)
    }
}

/// Take the engine write lock, recovering from poison (every writer is
/// panic-guarded, so poison only means another thread panicked).
fn write_engine(engine: &RwLock<SearchEngine>) -> std::sync::RwLockWriteGuard<'_, SearchEngine> {
    engine.write().unwrap_or_else(|poisoned| {
        warn!("Search engine lock poisoned during Index RPC; recovering");
        poisoned.into_inner()
    })
}

impl Default for CodeSearchService {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl CodeSearch for CodeSearchService {
    type SearchStream = ReceiverStream<Result<SearchResult, Status>>;

    #[tracing::instrument(skip(self, request), fields(query, max_results))]
    async fn search(
        &self,
        request: Request<SearchRequest>,
    ) -> Result<Response<Self::SearchStream>, Status> {
        let req = request.into_inner();
        let query = req.query.trim().to_string();
        // proto3 default 0 = "unset": use the REST default page size rather
        // than clamping to a single result.
        let max_results = if req.max_results <= 0 {
            50
        } else {
            req.max_results.min(1000) as usize
        };
        let include_patterns = req.include_paths.join(";");
        let exclude_patterns = req.exclude_paths.join(";");
        let is_regex = req.is_regex;
        let symbols_only = req.symbols_only;
        let rank_mode = RankMode::parse(&req.rank);
        let mut limits = SearchLimits::new(max_results).with_offset(req.offset.max(0) as usize);
        if req.deadline_ms > 0 {
            limits = limits.with_timeout(std::time::Duration::from_millis(
                (req.deadline_ms as u64).min(30_000),
            ));
        }
        {
            let span = tracing::Span::current();
            span.record("query", query.as_str());
            span.record("max_results", max_results);
        }

        // Return empty stream immediately for empty queries, consistent with REST API.
        if query.is_empty() {
            let (_, rx) = tokio::sync::mpsc::channel::<Result<SearchResult, Status>>(1);
            return Ok(Response::new(ReceiverStream::new(rx)));
        }

        // Move CPU-intensive search work onto a blocking thread so tokio worker
        // threads are not starved under concurrent load.  The RwLockReadGuard is
        // not Send, so we clone the Arc and acquire the lock inside the closure.
        let engine_arc = std::sync::Arc::clone(&self.engine);
        let matches = tokio::task::spawn_blocking(move || {
            // Use try_read to avoid blocking when a write lock is held during indexing.
            let engine = match engine_arc.try_read() {
                Ok(guard) => guard,
                Err(std::sync::TryLockError::WouldBlock) => {
                    return Err(Status::unavailable(
                        "Index is currently being updated, please retry shortly",
                    ));
                }
                Err(std::sync::TryLockError::Poisoned(poisoned)) => {
                    warn!("Search engine lock was poisoned; recovering for search");
                    poisoned.into_inner()
                }
            };

            // Same four modes and limits as /api/search.
            let (matches, _info) = if symbols_only {
                engine
                    .search_symbols_with_limits(
                        &query,
                        &include_patterns,
                        &exclude_patterns,
                        limits,
                    )
                    .map_err(|e| {
                        Status::invalid_argument(format!("Invalid filter pattern: {}", e))
                    })?
            } else if is_regex {
                engine
                    .search_regex_with_limits(
                        &query,
                        &include_patterns,
                        &exclude_patterns,
                        limits,
                        rank_mode,
                    )
                    .map_err(|e| {
                        Status::invalid_argument(format!("Invalid regex pattern: {}", e))
                    })?
            } else if include_patterns.is_empty() && exclude_patterns.is_empty() {
                engine.search_ranked_with_limits(&query, limits, rank_mode)
            } else {
                engine
                    .search_with_filter_ranked_limits(
                        &query,
                        &include_patterns,
                        &exclude_patterns,
                        limits,
                        rank_mode,
                    )
                    .map_err(|e| {
                        Status::invalid_argument(format!("Invalid filter pattern: {}", e))
                    })?
            };

            Ok::<_, Status>(matches)
        })
        .await
        .map_err(|e| Status::internal(format!("Search task panicked: {}", e)))??;

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
                    match_start: m.match_start as i32,
                    match_end: m.match_end as i32,
                    line_match_start: m.line_match_start as i32,
                    line_match_end: m.line_match_end as i32,
                    match_column: m.match_column as i32,
                    dependency_count: m.dependency_count,
                    content_truncated: m.content_truncated,
                };

                if tx.send(Ok(result)).await.is_err() {
                    break;
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    #[tracing::instrument(skip(self, request))]
    async fn index(
        &self,
        request: Request<IndexRequest>,
    ) -> Result<Response<IndexResponse>, Status> {
        let req = request.into_inner();
        info!(paths = ?req.paths, "Received index request");
        let start = Instant::now();

        // Scope check: a network client must not be able to index (and then
        // read back via /api/file) arbitrary paths on the host.
        if let Some(roots) = &self.allowed_index_roots {
            for path in &req.paths {
                let canonical = std::path::Path::new(path)
                    .canonicalize()
                    .map_err(|e| Status::invalid_argument(format!("Cannot index {path}: {e}")))?;
                if !roots.iter().any(|root| canonical.starts_with(root)) {
                    warn!(path = %path, "Rejected index request outside configured roots");
                    return Err(Status::permission_denied(format!(
                        "{path} is outside the configured index paths"
                    )));
                }
            }
        }

        // Discover eligible files WITHOUT the engine lock (exclude patterns,
        // include extensions, binary/size rules and .gitignore from the
        // indexer config), then index in batches, taking the write lock only
        // for each merge so searches keep being served in between.
        let engine_arc = std::sync::Arc::clone(&self.engine);
        let indexer_config = self.indexer_config.clone().unwrap_or_default();
        let paths = req.paths.clone();
        let (files_indexed, total_size, stats) = tokio::task::spawn_blocking(move || {
            use crate::search::{
                FileDiscoveryConfig, FileDiscoveryIterator, PartialIndexedFile, PreIndexedFile,
            };
            use rayon::prelude::*;

            let discovery = FileDiscoveryConfig {
                paths: paths.clone(),
                exclude_patterns: indexer_config.exclude_patterns.clone(),
                include_extensions: indexer_config.include_extensions.clone(),
                max_file_size: Some(if indexer_config.max_file_size == 0 {
                    PartialIndexedFile::DEFAULT_MAX_FILE_SIZE
                } else {
                    indexer_config.max_file_size
                }),
                respect_gitignore: indexer_config.respect_gitignore,
                ..Default::default()
            };
            let files: Vec<std::path::PathBuf> = FileDiscoveryIterator::new(&discovery)
                .filter(|p| !indexer_config.is_file_excluded(p))
                .collect();

            {
                let mut engine = write_engine(&engine_arc);
                for path in &paths {
                    engine.add_root_path(std::path::Path::new(path));
                }
            }

            let mut files_indexed = 0i32;
            let mut total_size = 0u64;
            for chunk in files.chunks(indexer_config.batch_size.max(1)) {
                // Phase 1 (parallel, no lock): read + trigrams + symbols.
                let pre: Vec<PreIndexedFile> = chunk
                    .par_iter()
                    .filter_map(|p| {
                        let (partial, _) = PartialIndexedFile::process(
                            p,
                            indexer_config.transcode_non_utf8,
                            indexer_config.max_file_size,
                        )?;
                        Some(PreIndexedFile::from_partial(
                            partial,
                            indexer_config.enable_symbols,
                        ))
                    })
                    .collect();
                total_size += pre.iter().map(|p| p.size).sum::<u64>();
                // Phase 2 (short write lock): merge.
                let mut engine = write_engine(&engine_arc);
                files_indexed += engine.index_batch(pre) as i32;
                engine.resolve_imports_incremental();
            }
            let stats = {
                let mut engine = write_engine(&engine_arc);
                engine.resolve_imports();
                engine.finalize();
                engine.get_stats()
            };
            Ok::<_, Status>((files_indexed, total_size, stats))
        })
        .await
        .map_err(|e| Status::internal(format!("Index task panicked: {}", e)))??;

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

/// Create gRPC server with an existing shared engine
pub fn create_server_with_engine(
    engine: Arc<RwLock<SearchEngine>>,
) -> CodeSearchServer<CodeSearchService> {
    CodeSearchServer::new(CodeSearchService::with_engine(engine))
}

/// Create the gRPC server the binary ships: `Index` scoped to the configured
/// paths with the indexer's eligibility rules.
pub fn create_server_with_engine_config(
    engine: Arc<RwLock<SearchEngine>>,
    config: &IndexerConfig,
) -> CodeSearchServer<CodeSearchService> {
    CodeSearchServer::new(CodeSearchService::with_engine_config(engine, config))
}

/// Create a gRPC server whose `Index` RPC is restricted to `roots`.
pub fn create_server_with_engine_scoped(
    engine: Arc<RwLock<SearchEngine>>,
    roots: Vec<std::path::PathBuf>,
) -> CodeSearchServer<CodeSearchService> {
    CodeSearchServer::new(CodeSearchService::with_engine_scoped(engine, roots))
}
