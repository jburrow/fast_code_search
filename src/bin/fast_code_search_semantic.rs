//! Fast Code Search Semantic Server
//!
//! Natural language code search using embeddings and vector similarity.
//! Runs as a separate service on port 50052 (gRPC) and 8081 (Web UI).

use anyhow::Result;
use clap::Parser;
use fast_code_search::diagnostics;
use fast_code_search::semantic::{SemanticConfig, SemanticSearchEngine};
use fast_code_search::semantic_web::{
    self, create_semantic_progress_broadcaster, SemanticProgress, SemanticProgressBroadcaster,
    SharedSemanticProgress, WebState,
};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

/// Check if content appears to be binary (contains null bytes or high ratio of non-printable chars)
fn is_binary_content(content: &str) -> bool {
    // Check first 8KB for binary indicators
    let check_len = content.len().min(8192);
    let sample = &content[..check_len];

    let mut non_text_count = 0;
    for byte in sample.bytes() {
        // Null bytes are a strong indicator of binary content
        if byte == 0 {
            return true;
        }
        // Count non-printable, non-whitespace characters (excluding common control chars)
        if byte < 32 && !matches!(byte, b'\t' | b'\n' | b'\r') {
            non_text_count += 1;
        }
    }

    // If more than 10% non-text characters, likely binary
    non_text_count > check_len / 10
}

/// Fast Code Search Semantic Server
#[derive(Parser, Debug)]
#[command(name = "fast_code_search_semantic")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Server listen address (overrides config)
    #[arg(short, long, value_name = "ADDR")]
    address: Option<String>,

    /// Additional paths to index
    #[arg(short, long = "index", value_name = "PATH")]
    index_paths: Vec<String>,

    /// Skip automatic indexing
    #[arg(long)]
    no_auto_index: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Generate template config and exit
    #[arg(long, value_name = "FILE")]
    init: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.verbose {
        Level::DEBUG
    } else {
        Level::INFO
    };

    FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();

    // Initialize diagnostics server start time
    diagnostics::init_server_start_time();

    // Handle --init flag
    if let Some(init_path) = args.init {
        let path = if init_path.as_os_str().is_empty() {
            PathBuf::from("fast_code_search_semantic.toml")
        } else {
            init_path
        };

        if path.exists() {
            eprintln!("Error: Config file already exists: {}", path.display());
            eprintln!("Remove it first or choose a different path.");
            std::process::exit(1);
        }

        SemanticConfig::write_template(&path)?;
        println!("✓ Generated config file: {}", path.display());
        println!("\nEdit the file to configure your semantic search, then start:");
        println!(
            "  cargo run --release --bin fast_code_search_semantic -- --config {}",
            path.display()
        );
        return Ok(());
    }

    // Load configuration
    let config = load_config(&args)?;

    info!(
        server_address = %config.server.web_address,
        "Semantic Search Configuration loaded"
    );

    // Create semantic search engine
    info!("Initializing semantic search engine...");
    let mut engine =
        SemanticSearchEngine::new(config.indexer.chunk_size, config.indexer.chunk_overlap);

    // Try to load existing index if configured
    if let Some(ref index_path) = config.indexer.index_path {
        let path = std::path::Path::new(index_path);
        if path.with_extension("index").exists() {
            info!(path = %index_path, "Loading existing index");
            match engine.load_index(path) {
                Ok(_) => {
                    let stats = engine.get_stats();
                    info!(
                        files = stats.num_files,
                        chunks = stats.num_chunks,
                        "Index loaded successfully"
                    );
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to load index, will rebuild");
                }
            }
        }
    }

    let shared_engine = Arc::new(RwLock::new(engine));

    // Create shared progress state
    let shared_progress: SharedSemanticProgress =
        Arc::new(RwLock::new(SemanticProgress::default()));
    let progress_tx: SemanticProgressBroadcaster = create_semantic_progress_broadcaster();

    // Start background indexing if enabled
    if !args.no_auto_index && !config.indexer.paths.is_empty() {
        let indexer_config = config.indexer.clone();
        let index_engine = Arc::clone(&shared_engine);
        let index_progress = Arc::clone(&shared_progress);
        let index_progress_tx = progress_tx.clone();

        info!(
            "Starting background indexing of {} path(s)",
            indexer_config.paths.len()
        );

        // Helper to update and broadcast progress
        fn broadcast_progress(
            progress: &SharedSemanticProgress,
            tx: &SemanticProgressBroadcaster,
            update_fn: impl FnOnce(&mut SemanticProgress),
        ) {
            if let Ok(mut p) = progress.write() {
                update_fn(&mut p);
                let _ = tx.send(p.clone());
            }
        }

        // Use a larger stack size (16MB) to handle tree-sitter recursion on deeply nested files
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .name("semantic-indexer".to_string())
            .spawn(move || {
                use walkdir::WalkDir;

                // Initialize progress
                broadcast_progress(&index_progress, &index_progress_tx, |p| {
                    p.status = "indexing".to_string();
                    p.is_indexing = true;
                    p.message = "Starting indexing...".to_string();
                });

                let binary_extensions: std::collections::HashSet<&str> = [
                    "exe", "dll", "so", "dylib", "bin", "o", "a", "png", "jpg", "jpeg", "gif",
                    "ico", "bmp", "zip", "tar", "gz", "7z", "rar", "woff", "woff2", "ttf", "eot",
                    "pdf",
                ]
                .into_iter()
                .collect();

                let exclude_patterns: Vec<String> = indexer_config
                    .exclude_patterns
                    .iter()
                    .map(|p| p.trim_matches('*').trim_matches('/').to_string())
                    .collect();

                let mut total_indexed = 0;
                let mut total_chunks = 0;

                for path_str in &indexer_config.paths {
                    let path = std::path::Path::new(path_str);
                    if !path.exists() {
                        tracing::warn!(path = %path_str, "Path does not exist, skipping");
                        continue;
                    }

                    info!(path = %path_str, "Indexing path");

                    for entry in WalkDir::new(path)
                        .follow_links(true)
                        .into_iter()
                        .filter_map(|e| e.ok())
                    {
                        if !entry.file_type().is_file() {
                            continue;
                        }

                        let entry_path = entry.path();
                        let path_str_check = entry_path.to_string_lossy();

                        // Check exclude patterns
                        if exclude_patterns
                            .iter()
                            .any(|pattern| path_str_check.contains(pattern))
                        {
                            continue;
                        }

                        // Skip binary files
                        if let Some(ext) = entry_path.extension() {
                            let ext = ext.to_string_lossy().to_lowercase();
                            if binary_extensions.contains(ext.as_str()) {
                                continue;
                            }
                        }

                        // Skip very large files that could cause parsing issues (> 1MB)
                        if let Ok(metadata) = entry_path.metadata() {
                            if metadata.len() > 1024 * 1024 {
                                tracing::debug!(
                                    path = %entry_path.display(),
                                    size = metadata.len(),
                                    "Skipping large file"
                                );
                                continue;
                            }
                        }

                        // Index the file, catching any panics from tree-sitter parsing
                        if let Ok(content) = std::fs::read_to_string(entry_path) {
                            // Skip binary files that slipped through (e.g., cache files without extensions)
                            if is_binary_content(&content) {
                                tracing::debug!(
                                    path = %entry_path.display(),
                                    "Skipping binary file"
                                );
                                continue;
                            }

                            let entry_path_owned = entry_path.to_path_buf();
                            let result =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    let mut engine = index_engine.write().unwrap();
                                    engine.index_file(&entry_path_owned, &content)
                                }));

                            match result {
                                Ok(Ok(num_chunks)) => {
                                    total_indexed += 1;
                                    total_chunks += num_chunks;

                                    if total_indexed % 100 == 0 {
                                        info!(
                                            files = total_indexed,
                                            chunks = total_chunks,
                                            "Indexing progress"
                                        );
                                        broadcast_progress(
                                            &index_progress,
                                            &index_progress_tx,
                                            |p| {
                                                p.files_indexed = total_indexed;
                                                p.chunks_indexed = total_chunks;
                                                p.current_path =
                                                    Some(entry_path_owned.display().to_string());
                                                p.message = format!(
                                                    "Indexing {} files, {} chunks...",
                                                    total_indexed, total_chunks
                                                );
                                            },
                                        );
                                    }
                                }
                                Ok(Err(e)) => {
                                    tracing::warn!(
                                        path = %entry_path.display(),
                                        error = %e,
                                        "Failed to index file"
                                    );
                                }
                                Err(_) => {
                                    tracing::warn!(
                                        path = %entry_path.display(),
                                        "File parsing caused a panic, skipping"
                                    );
                                }
                            }
                        }
                    }
                }

                info!(
                    files_indexed = total_indexed,
                    total_chunks = total_chunks,
                    "Background indexing completed"
                );

                // Update progress: completed
                broadcast_progress(&index_progress, &index_progress_tx, |p| {
                    p.status = "completed".to_string();
                    p.is_indexing = false;
                    p.files_indexed = total_indexed;
                    p.chunks_indexed = total_chunks;
                    p.current_path = None;
                    p.progress_percent = 100;
                    p.message = format!(
                        "Indexing complete: {} files, {} chunks",
                        total_indexed, total_chunks
                    );
                });

                // Save index if configured
                if let Some(ref index_path) = indexer_config.index_path {
                    info!(path = %index_path, "Saving index to disk");
                    let engine = index_engine.read().unwrap();
                    if let Err(e) = engine.save_index(std::path::Path::new(index_path)) {
                        tracing::error!(error = %e, "Failed to save index");
                    } else {
                        info!("Index saved successfully");
                    }
                }
            })
            .expect("Failed to spawn indexing thread");
    } else if args.no_auto_index {
        info!("Auto-indexing disabled via --no-auto-index flag");
    } else {
        info!("No paths configured for indexing");
    }

    // Start gRPC server
    let grpc_addr = config.server.address.clone();
    let grpc_engine = Arc::clone(&shared_engine);

    info!(grpc_address = %grpc_addr, "Starting gRPC server");

    tokio::spawn(async move {
        use fast_code_search::semantic_server::{SemanticCodeSearchServer, SemanticSearchService};
        use tonic::transport::Server;

        let service = SemanticSearchService::new(grpc_engine);
        let addr = grpc_addr.parse().expect("Invalid gRPC server address");

        info!(address = %grpc_addr, "Semantic gRPC server listening on {}", grpc_addr);

        Server::builder()
            .add_service(SemanticCodeSearchServer::new(service))
            .serve(addr)
            .await
            .expect("gRPC server failed");
    });

    // Start web server
    if config.server.enable_web_ui {
        let web_addr = config.server.web_address.clone();
        let web_engine = Arc::clone(&shared_engine);
        let web_progress = Arc::clone(&shared_progress);
        let web_progress_tx = progress_tx.clone();

        info!(web_address = %web_addr, "Starting Web UI server");

        tokio::spawn(async move {
            let state = WebState {
                engine: web_engine,
                progress: web_progress,
                progress_tx: web_progress_tx,
            };
            let router = semantic_web::create_router(state);
            let listener = tokio::net::TcpListener::bind(&web_addr)
                .await
                .expect("Failed to bind Web UI");

            info!(address = %web_addr, "Semantic Web UI available at http://{}", web_addr);

            axum::serve(listener, router)
                .await
                .expect("Web UI server failed");
        });
    }

    info!("Semantic Search Server ready");
    info!("Press Ctrl+C to stop");

    // Keep the main thread alive
    tokio::signal::ctrl_c().await?;
    info!("Shutting down...");

    Ok(())
}

fn load_config(args: &Args) -> Result<SemanticConfig> {
    let base_config = if let Some(ref config_path) = args.config {
        if !config_path.exists() {
            anyhow::bail!(
                "Config file not found: {}\nUse --init {} to generate.",
                config_path.display(),
                config_path.display()
            );
        }
        info!(path = %config_path.display(), "Loading config");
        SemanticConfig::from_file(config_path)?
    } else {
        match SemanticConfig::from_default_locations()? {
            Some((config, path)) => {
                info!(path = %path.display(), "Loading config from default location");
                config
            }
            None => {
                info!("No config file found, using defaults");
                SemanticConfig::default()
            }
        }
    };

    Ok(base_config.with_overrides(args.address.clone(), args.index_paths.clone()))
}
