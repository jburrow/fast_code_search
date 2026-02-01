use anyhow::Result;
use clap::Parser;
use fast_code_search::config::Config;
use fast_code_search::search::{IndexingProgress, IndexingStatus, PreIndexedFile, SharedIndexingProgress};
use fast_code_search::server;
use fast_code_search::web;
use std::path::PathBuf;
use tonic::transport::Server;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

/// Fast Code Search Server - High-performance code search service
#[derive(Parser, Debug)]
#[command(name = "fast_code_search")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Server listen address (overrides config file)
    #[arg(short, long, value_name = "ADDR")]
    address: Option<String>,

    /// Additional paths to index (can be repeated, adds to config file paths)
    #[arg(short, long = "index", value_name = "PATH")]
    index_paths: Vec<String>,

    /// Skip automatic indexing on startup
    #[arg(long)]
    no_auto_index: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Generate a template configuration file and exit
    #[arg(long, value_name = "FILE")]
    init: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize tracing subscriber
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

    // Handle --init flag: generate template config and exit
    if let Some(init_path) = args.init {
        let path = if init_path.as_os_str().is_empty() {
            PathBuf::from("fast_code_search.toml")
        } else {
            init_path
        };

        if path.exists() {
            eprintln!("Error: Config file already exists: {}", path.display());
            eprintln!("Remove it first or choose a different path.");
            std::process::exit(1);
        }

        Config::write_template(&path)?;
        println!("✓ Generated config file: {}", path.display());
        println!("\nEdit the file to add your project paths, then start the server with:");
        println!("  cargo run --release -- --config {}", path.display());
        return Ok(());
    }

    // Load configuration
    let config = load_config(&args)?;

    info!(
        server_address = %config.server.address,
        paths_count = config.indexer.paths.len(),
        "Configuration loaded"
    );

    if args.verbose {
        info!(paths = ?config.indexer.paths, "Paths to index");
        info!(exclude_patterns = ?config.indexer.exclude_patterns, "Exclude patterns");
    }

    let addr = config.server.address.parse()?;

    // Create shared engine (empty initially, will be indexed in background)
    // Using RwLock allows concurrent read access during searches while only blocking for writes (indexing)
    let shared_engine = std::sync::Arc::new(std::sync::RwLock::new(fast_code_search::search::SearchEngine::new()));

    // Create shared indexing progress state for UI visibility
    let shared_progress: SharedIndexingProgress =
        std::sync::Arc::new(std::sync::RwLock::new(IndexingProgress::default()));

    // Start web server first if enabled (so UI is available during indexing)
    if config.server.enable_web_ui {
        let web_addr = config.server.web_address.clone();
        let web_engine = shared_engine.clone();
        let web_progress = shared_progress.clone();
        info!(web_address = %web_addr, "Starting Web UI server");
        
        tokio::spawn(async move {
            let router = web::create_router(web_engine, web_progress);
            let listener = tokio::net::TcpListener::bind(&web_addr).await.unwrap();
            info!(address = %web_addr, "Web UI available at http://{}", web_addr);
            axum::serve(listener, router).await.unwrap();
        });
    }

    // Start background indexing if enabled
    if !args.no_auto_index && !config.indexer.paths.is_empty() {
        let indexer_config = config.indexer.clone();
        let index_engine = shared_engine.clone();
        let index_progress = shared_progress.clone();
        info!("Starting background indexing");
        
        std::thread::spawn(move || {
            use rayon::prelude::*;
            use std::time::Instant;
            let total_start = Instant::now();
            info!("Background indexing {} path(s)", indexer_config.paths.len());

            // Helper to update progress state
            let update_progress = |updater: Box<dyn FnOnce(&mut IndexingProgress) + Send>| {
                if let Ok(mut progress) = index_progress.write() {
                    updater(&mut progress);
                }
            };

            // Initialize progress - starting discovery phase
            update_progress(Box::new(|p| {
                *p = IndexingProgress::start();
            }));
            
            // Pre-compile exclude patterns once (avoids recompilation per file)
            let exclude_patterns: Vec<String> = indexer_config
                .exclude_patterns
                .iter()
                .map(|p| p.trim_matches('*').trim_matches('/').to_string())
                .collect();
            
            // Binary extensions to skip
            let binary_extensions: std::collections::HashSet<&str> = [
                "exe", "dll", "so", "dylib", "bin", "o", "a",
                "png", "jpg", "jpeg", "gif", "ico", "bmp", 
                "zip", "tar", "gz", "7z", "rar",
                "woff", "woff2", "ttf", "eot", "pdf",
            ].into_iter().collect();
            
            // Phase 1: Collect all file paths to index (single-threaded, I/O bound)
            let collect_start = Instant::now();
            let mut files_to_index: Vec<std::path::PathBuf> = Vec::new();
            
            for path_str in &indexer_config.paths {
                let path = std::path::Path::new(path_str);
                if !path.exists() {
                    tracing::warn!(path = %path_str, "Path does not exist, skipping");
                    update_progress(Box::new(|p| p.errors += 1));
                    continue;
                }
                
                info!(path = %path_str, "Discovering files");
                let path_str_owned = path_str.clone();
                update_progress(Box::new(move |p| {
                    p.current_path = Some(path_str_owned.clone());
                    p.message = format!("Discovering files in {}...", path_str_owned);
                }));
                
                for entry in walkdir::WalkDir::new(path)
                    .follow_links(true)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    if !entry.file_type().is_file() {
                        continue;
                    }
                    
                    let entry_path = entry.path();
                    let path_str_check = entry_path.to_string_lossy();
                    
                    // Check exclude patterns (using pre-compiled patterns)
                    let should_exclude = exclude_patterns.iter().any(|pattern| {
                        path_str_check.contains(pattern)
                    });
                    
                    if should_exclude {
                        continue;
                    }
                    
                    // Skip binary files
                    if let Some(ext) = entry_path.extension() {
                        let ext = ext.to_string_lossy().to_lowercase();
                        if binary_extensions.contains(ext.as_str()) {
                            continue;
                        }
                    }
                    
                    files_to_index.push(entry_path.to_path_buf());
                }
            }
            
            let file_count = files_to_index.len();
            info!(
                file_count = file_count,
                elapsed_ms = collect_start.elapsed().as_millis(),
                "File discovery completed"
            );

            // Update progress: discovery complete, moving to indexing phase
            update_progress(Box::new(move |p| {
                p.status = IndexingStatus::Indexing;
                p.files_discovered = file_count;
                p.current_path = None;
                p.message = format!("Discovered {} files, starting indexing...", file_count);
            }));
            
            // Phase 2: Index files in batches with parallel processing
            // Batch size balances parallelism overhead vs lock contention
            const BATCH_SIZE: usize = 500;
            let batch_count = (file_count + BATCH_SIZE - 1) / BATCH_SIZE;

            // Update progress with batch count
            let index_progress_ref = index_progress.clone();
            if let Ok(mut p) = index_progress_ref.write() {
                p.total_batches = batch_count;
            }

            let mut total_indexed = 0usize;
            
            for (batch_idx, batch) in files_to_index.chunks(BATCH_SIZE).enumerate() {
                let batch_start = Instant::now();

                // Update progress at start of batch
                let batch_num = batch_idx + 1;
                let index_progress_ref = index_progress.clone();
                if let Ok(mut p) = index_progress_ref.write() {
                    p.current_batch = batch_num;
                    p.message = format!("Indexing batch {}/{} ({} files)...", 
                        batch_num, batch_count, batch.len());
                    // Show the first file of the batch as current path
                    if let Some(first) = batch.first() {
                        p.current_path = first.file_name()
                            .map(|n| n.to_string_lossy().to_string());
                    }
                }
                
                // Process files in parallel - CPU-heavy work (read, trigrams, symbols)
                let pre_indexed: Vec<PreIndexedFile> = batch
                    .par_iter()
                    .filter_map(|path| PreIndexedFile::process(path))
                    .collect();

                let batch_indexed_count = pre_indexed.len();
                
                // Merge batch into engine with single write lock acquisition
                {
                    let mut engine = index_engine.write().unwrap();
                    engine.index_batch(pre_indexed);
                }

                total_indexed += batch_indexed_count;

                // Update progress after batch
                let index_progress_ref = index_progress.clone();
                if let Ok(mut p) = index_progress_ref.write() {
                    p.files_indexed = total_indexed;
                }
                
                if batch_idx % 10 == 0 || batch_idx == batch_count - 1 {
                    info!(
                        batch = batch_idx + 1,
                        total_batches = batch_count,
                        files_indexed = total_indexed,
                        batch_ms = batch_start.elapsed().as_millis(),
                        "Batch indexed"
                    );
                }
            }
            
            // Resolve all import relationships after indexing completes
            {
                // Update progress: resolving imports
                let index_progress_ref = index_progress.clone();
                if let Ok(mut p) = index_progress_ref.write() {
                    p.status = IndexingStatus::ResolvingImports;
                    p.current_path = None;
                    p.message = String::from("Resolving import dependencies...");
                }

                let mut engine = index_engine.write().unwrap();
                info!("Resolving import dependencies...");
                engine.resolve_imports();
                let stats = engine.get_stats();
                info!(
                    dependency_edges = stats.dependency_edges,
                    "Import resolution completed"
                );
            }
            
            let elapsed = total_start.elapsed();
            let files_per_sec = if elapsed.as_secs_f64() > 0.0 {
                file_count as f64 / elapsed.as_secs_f64()
            } else {
                0.0
            };

            info!(
                elapsed_secs = elapsed.as_secs_f64(),
                files_indexed = file_count,
                files_per_sec = files_per_sec,
                "Background indexing completed"
            );

            // Update progress: completed
            {
                if let Ok(mut p) = index_progress.write() {
                    p.status = IndexingStatus::Completed;
                    p.files_indexed = total_indexed;
                    p.current_path = None;
                    p.message = format!(
                        "Indexing complete: {} files in {:.1}s ({:.0} files/sec)",
                        total_indexed,
                        elapsed.as_secs_f64(),
                        files_per_sec
                    );
                }
            }
        });
    } else if args.no_auto_index {
        info!("Auto-indexing disabled via --no-auto-index flag");
    } else {
        info!("No paths configured for indexing");
    }

    // Create gRPC service with shared engine
    let search_service = server::create_server_with_engine(shared_engine.clone());

    info!(address = %addr, "Fast Code Search Server starting");
    info!(grpc_endpoint = %format!("grpc://{}", addr), "gRPC endpoint");
    info!("Ready to accept connections");

    Server::builder()
        .add_service(search_service)
        .serve(addr)
        .await?;

    Ok(())
}

fn load_config(args: &Args) -> Result<Config> {
    let base_config = if let Some(ref config_path) = args.config {
        // Explicit config file specified
        if !config_path.exists() {
            anyhow::bail!(
                "Config file not found: {}\nUse --init {} to generate a template.",
                config_path.display(),
                config_path.display()
            );
        }
        info!(path = %config_path.display(), "Loading config from file");
        Config::from_file(config_path)?
    } else {
        // Try default locations
        match Config::from_default_locations()? {
            Some((config, path)) => {
                info!(path = %path.display(), "Loading config from default location");
                config
            }
            None => {
                info!("No config file found, using defaults");
                Config::default()
            }
        }
    };

    // Apply CLI overrides
    Ok(base_config.with_overrides(args.address.clone(), args.index_paths.clone()))
}
