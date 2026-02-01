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
            use std::sync::mpsc;
            use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
            use std::sync::Arc;

            // Configure rayon with larger stack size (8MB) to handle tree-sitter recursion
            rayon::ThreadPoolBuilder::new()
                .stack_size(8 * 1024 * 1024)
                .build_global()
                .ok(); // Ignore error if already initialized

            let total_start = Instant::now();
            info!("Background indexing {} path(s)", indexer_config.paths.len());

            // Initialize progress
            if let Ok(mut p) = index_progress.write() {
                *p = IndexingProgress::start();
            }
            
            // Pre-compile exclude patterns once
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
            
            // Channel for streaming file paths from discovery to indexing
            // Bounded channel provides backpressure if indexing falls behind
            const CHANNEL_BUFFER: usize = 5000;
            const BATCH_SIZE: usize = 500;
            
            let (tx, rx) = mpsc::sync_channel::<std::path::PathBuf>(CHANNEL_BUFFER);
            
            // Shared counters for progress tracking
            let files_discovered = Arc::new(AtomicUsize::new(0));
            let discovery_done = Arc::new(AtomicBool::new(false));
            
            // Spawn discovery thread
            let discovery_files_discovered = files_discovered.clone();
            let discovery_done_flag = discovery_done.clone();
            let discovery_progress = index_progress.clone();
            
            let discovery_handle = std::thread::spawn(move || {
                for path_str in &indexer_config.paths {
                    let path = std::path::Path::new(path_str);
                    if !path.exists() {
                        tracing::warn!(path = %path_str, "Path does not exist, skipping");
                        continue;
                    }
                    
                    info!(path = %path_str, "Discovering files");
                    
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
                        
                        // Check exclude patterns
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
                        
                        // Send to indexing pipeline (blocks if channel is full)
                        if tx.send(entry_path.to_path_buf()).is_err() {
                            break; // Receiver dropped, stop discovery
                        }
                        
                        let count = discovery_files_discovered.fetch_add(1, Ordering::Relaxed) + 1;
                        
                        // Update progress periodically (every 1000 files)
                        if count % 1000 == 0 {
                            if let Ok(mut p) = discovery_progress.write() {
                                p.files_discovered = count;
                                p.message = format!("Discovering... {} files found", count);
                            }
                        }
                    }
                }
                
                // Signal discovery complete
                discovery_done_flag.store(true, Ordering::Release);
                let final_count = discovery_files_discovered.load(Ordering::Relaxed);
                info!(file_count = final_count, "File discovery completed");
            });
            
            // Main indexing loop - processes files as they arrive
            let mut batch: Vec<std::path::PathBuf> = Vec::with_capacity(BATCH_SIZE);
            let mut total_indexed = 0usize;
            let mut batch_num = 0usize;
            
            // Update status to indexing (discovery + indexing running concurrently)
            if let Ok(mut p) = index_progress.write() {
                p.status = IndexingStatus::Indexing;
                p.message = String::from("Discovering and indexing files...");
            }
            
            loop {
                // Try to receive with timeout to allow checking discovery status
                match rx.recv_timeout(std::time::Duration::from_millis(100)) {
                    Ok(path) => {
                        batch.push(path);
                        
                        // Process batch when full
                        if batch.len() >= BATCH_SIZE {
                            batch_num += 1;
                            let batch_start = Instant::now();
                            
                            // Update progress
                            let discovered = files_discovered.load(Ordering::Relaxed);
                            if let Ok(mut p) = index_progress.write() {
                                p.current_batch = batch_num;
                                p.files_discovered = discovered;
                                p.message = format!("Indexing batch {} ({} files)...", batch_num, batch.len());
                            }
                            
                            // Process in parallel
                            let pre_indexed: Vec<PreIndexedFile> = batch
                                .par_iter()
                                .filter_map(|path| PreIndexedFile::process(path))
                                .collect();
                            
                            let batch_indexed_count = pre_indexed.len();
                            
                            // Merge into engine
                            {
                                let mut engine = index_engine.write().unwrap();
                                engine.index_batch(pre_indexed);
                            }
                            
                            total_indexed += batch_indexed_count;
                            
                            // Update progress
                            if let Ok(mut p) = index_progress.write() {
                                p.files_indexed = total_indexed;
                            }
                            
                            if batch_num % 10 == 0 {
                                info!(
                                    batch = batch_num,
                                    files_indexed = total_indexed,
                                    files_discovered = discovered,
                                    batch_ms = batch_start.elapsed().as_millis(),
                                    "Batch indexed"
                                );
                            }
                            
                            batch.clear();
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        // Check if discovery is done and channel is empty
                        if discovery_done.load(Ordering::Acquire) && rx.try_recv().is_err() {
                            break;
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }
            
            // Process remaining files in final batch
            if !batch.is_empty() {
                batch_num += 1;
                let batch_start = Instant::now();
                
                let pre_indexed: Vec<PreIndexedFile> = batch
                    .par_iter()
                    .filter_map(|path| PreIndexedFile::process(path))
                    .collect();
                
                let batch_indexed_count = pre_indexed.len();
                
                {
                    let mut engine = index_engine.write().unwrap();
                    engine.index_batch(pre_indexed);
                }
                
                total_indexed += batch_indexed_count;
                
                info!(
                    batch = batch_num,
                    files_indexed = total_indexed,
                    batch_ms = batch_start.elapsed().as_millis(),
                    "Final batch indexed"
                );
            }
            
            // Wait for discovery thread to finish
            let _ = discovery_handle.join();
            
            let final_discovered = files_discovered.load(Ordering::Relaxed);
            
            // Update total batches now that we know the final count
            if let Ok(mut p) = index_progress.write() {
                p.files_discovered = final_discovered;
                p.total_batches = batch_num;
                p.current_batch = batch_num;
            }
            
            // Resolve all import relationships after indexing completes
            {
                if let Ok(mut p) = index_progress.write() {
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
                total_indexed as f64 / elapsed.as_secs_f64()
            } else {
                0.0
            };

            info!(
                elapsed_secs = elapsed.as_secs_f64(),
                files_indexed = total_indexed,
                files_discovered = final_discovered,
                files_per_sec = files_per_sec,
                "Background indexing completed"
            );

            // Update progress: completed
            {
                if let Ok(mut p) = index_progress.write() {
                    p.status = IndexingStatus::Completed;
                    p.files_indexed = total_indexed;
                    p.files_discovered = final_discovered;
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
