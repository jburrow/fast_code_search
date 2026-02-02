use anyhow::Result;
use clap::Parser;
use fast_code_search::config::Config;
use fast_code_search::search::{
    create_progress_broadcaster, IndexingProgress, IndexingStatus, PreIndexedFile,
    ProgressBroadcaster, SharedIndexingProgress,
};
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
    let shared_engine = std::sync::Arc::new(std::sync::RwLock::new(
        fast_code_search::search::SearchEngine::new(),
    ));

    // Create shared indexing progress state for UI visibility
    let shared_progress: SharedIndexingProgress =
        std::sync::Arc::new(std::sync::RwLock::new(IndexingProgress::default()));

    // Create broadcast channel for WebSocket progress updates
    let progress_tx: ProgressBroadcaster = create_progress_broadcaster();

    // Start web server first if enabled (so UI is available during indexing)
    if config.server.enable_web_ui {
        let web_addr = config.server.web_address.clone();
        let web_engine = shared_engine.clone();
        let web_progress = shared_progress.clone();
        let web_progress_tx = progress_tx.clone();
        info!(web_address = %web_addr, "Starting Web UI server");

        tokio::spawn(async move {
            let router = web::create_router(web_engine, web_progress, web_progress_tx);
            let listener = tokio::net::TcpListener::bind(&web_addr)
                .await
                .expect("Failed to bind Web UI server to address");
            info!(address = %web_addr, "Web UI available at http://{}", web_addr);
            axum::serve(listener, router)
                .await
                .expect("Web UI server failed");
        });
    }

    // Start background indexing if enabled
    if !args.no_auto_index && !config.indexer.paths.is_empty() {
        let indexer_config = config.indexer.clone();
        let index_engine = shared_engine.clone();
        let index_progress = shared_progress.clone();
        let index_progress_tx = progress_tx.clone();
        info!("Starting background indexing");

        std::thread::spawn(move || {
            use fast_code_search::search::LoadIndexResult;
            use rayon::prelude::*;
            use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
            use std::sync::mpsc;
            use std::sync::Arc;
            use std::time::Instant;

            // Configure rayon with larger stack size (8MB) to handle tree-sitter recursion
            rayon::ThreadPoolBuilder::new()
                .stack_size(8 * 1024 * 1024)
                .build_global()
                .ok(); // Ignore error if already initialized

            let total_start = Instant::now();
            info!("Background indexing {} path(s)", indexer_config.paths.len());

            // Helper to update progress and broadcast to WebSocket clients
            fn broadcast_progress(
                progress: &SharedIndexingProgress,
                tx: &ProgressBroadcaster,
                update_fn: impl FnOnce(&mut IndexingProgress),
            ) {
                if let Ok(mut p) = progress.write() {
                    update_fn(&mut p);
                    // Broadcast to WebSocket clients (ignore errors if no subscribers)
                    let _ = tx.send(p.clone());
                }
            }

            // Initialize progress
            broadcast_progress(&index_progress, &index_progress_tx, |p| {
                *p = IndexingProgress::start()
            });

            // Try to load persisted index if configured
            let mut load_result: Option<LoadIndexResult> = None;
            let mut loaded_from_persistence = false;

            if let Some(ref index_path_str) = indexer_config.index_path {
                let index_path = std::path::Path::new(index_path_str);

                if index_path.exists() {
                    broadcast_progress(&index_progress, &index_progress_tx, |p| {
                        p.status = IndexingStatus::LoadingIndex;
                        p.message = String::from("Loading persisted index...");
                    });

                    info!(path = %index_path.display(), "Attempting to load persisted index");

                    match index_engine.write() {
                        Ok(mut engine) => {
                            match engine.load_index_with_reconciliation(index_path, &indexer_config)
                            {
                                Ok(result) => {
                                    let files_loaded = engine.get_stats().num_files;
                                    info!(
                                        files_loaded = files_loaded,
                                        stale_files = result.stale_files.len(),
                                        removed_files = result.removed_files.len(),
                                        new_paths = result.new_paths.len(),
                                        config_compatible = result.config_compatible,
                                        "Loaded persisted index"
                                    );

                                    broadcast_progress(&index_progress, &index_progress_tx, |p| {
                                        p.files_indexed = files_loaded;
                                        p.message = format!(
                                            "Loaded {} files from cache, reconciling...",
                                            files_loaded
                                        );
                                    });

                                    loaded_from_persistence = true;
                                    load_result = Some(result);
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        error = %e,
                                        "Failed to load persisted index, will rebuild"
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "Failed to acquire engine lock");
                        }
                    }
                }
            }

            // Pre-compile exclude patterns once
            let exclude_patterns: Vec<String> = indexer_config
                .exclude_patterns
                .iter()
                .map(|p| p.trim_matches('*').trim_matches('/').to_string())
                .collect();

            // Binary extensions to skip
            let binary_extensions: std::collections::HashSet<&str> = [
                "exe", "dll", "so", "dylib", "bin", "o", "a", "png", "jpg", "jpeg", "gif", "ico",
                "bmp", "zip", "tar", "gz", "7z", "rar", "woff", "woff2", "ttf", "eot", "pdf",
            ]
            .into_iter()
            .collect();

            // Determine what needs to be indexed
            let paths_to_index: Vec<String> = if let Some(ref result) = load_result {
                // If we loaded from persistence, only index new paths
                result.new_paths.clone()
            } else {
                // Full indexing required
                indexer_config.paths.clone()
            };

            // Collect stale files that need re-indexing
            let stale_files: Vec<std::path::PathBuf> = load_result
                .as_ref()
                .map(|r| r.stale_files.clone())
                .unwrap_or_default();

            // Channel for streaming file paths from discovery to indexing
            // Bounded channel provides backpressure if indexing falls behind
            const CHANNEL_BUFFER: usize = 5000;
            const BATCH_SIZE: usize = 500;

            let (tx, rx) = mpsc::sync_channel::<std::path::PathBuf>(CHANNEL_BUFFER);

            // Shared counters for progress tracking
            let files_discovered = Arc::new(AtomicUsize::new(0));
            let discovery_done = Arc::new(AtomicBool::new(false));

            // Clone for discovery thread
            let discovery_exclude_patterns = exclude_patterns.clone();
            let discovery_binary_extensions = binary_extensions.clone();

            // Spawn discovery thread
            let discovery_files_discovered = files_discovered.clone();
            let discovery_done_flag = discovery_done.clone();
            let discovery_progress = index_progress.clone();
            let discovery_progress_tx = index_progress_tx.clone();

            let discovery_handle = std::thread::spawn(move || {
                // First, send stale files that need re-indexing
                for stale_path in stale_files {
                    if stale_path.exists() {
                        if tx.send(stale_path).is_err() {
                            return; // Receiver dropped
                        }
                        discovery_files_discovered.fetch_add(1, Ordering::Relaxed);
                    }
                }

                // Then discover files from paths that need indexing
                for path_str in &paths_to_index {
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
                        let should_exclude = discovery_exclude_patterns
                            .iter()
                            .any(|pattern| path_str_check.contains(pattern));

                        if should_exclude {
                            continue;
                        }

                        // Skip binary files
                        if let Some(ext) = entry_path.extension() {
                            let ext = ext.to_string_lossy().to_lowercase();
                            if discovery_binary_extensions.contains(ext.as_str()) {
                                continue;
                            }
                        }

                        // Send to indexing pipeline (blocks if channel is full)
                        if tx.send(entry_path.to_path_buf()).is_err() {
                            break; // Receiver dropped, stop discovery
                        }

                        let count = discovery_files_discovered.fetch_add(1, Ordering::Relaxed) + 1;

                        // Update progress periodically (every 1000 files)
                        if count.is_multiple_of(1000) {
                            if let Ok(mut p) = discovery_progress.write() {
                                p.files_discovered = count;
                                p.message = format!("Discovering... {} files found", count);
                                let _ = discovery_progress_tx.send(p.clone());
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

            // Update status based on whether we're reconciling or full indexing
            broadcast_progress(&index_progress, &index_progress_tx, |p| {
                if loaded_from_persistence {
                    p.status = IndexingStatus::Reconciling;
                    p.message = String::from("Reconciling index with filesystem...");
                } else {
                    p.status = IndexingStatus::Indexing;
                    p.message = String::from("Discovering and indexing files...");
                }
            });

            loop {
                // Try to receive with timeout to allow checking discovery status
                match rx.recv_timeout(std::time::Duration::from_millis(100)) {
                    Ok(path) => {
                        batch.push(path);

                        // Process batch when full
                        if batch.len() >= BATCH_SIZE {
                            batch_num += 1;
                            let batch_start = Instant::now();
                            let batch_len = batch.len();

                            // Update progress
                            let discovered = files_discovered.load(Ordering::Relaxed);
                            broadcast_progress(&index_progress, &index_progress_tx, |p| {
                                p.current_batch = batch_num;
                                p.files_discovered = discovered;
                                p.message = format!(
                                    "Indexing batch {} ({} files)...",
                                    batch_num, batch_len
                                );
                            });

                            // Process in parallel
                            let pre_indexed: Vec<PreIndexedFile> = batch
                                .par_iter()
                                .filter_map(|path| PreIndexedFile::process(path))
                                .collect();

                            let batch_indexed_count = pre_indexed.len();

                            // Merge into engine and incrementally resolve imports
                            {
                                let mut engine = index_engine
                                    .write()
                                    .expect("Failed to acquire write lock on search engine");
                                engine.index_batch(pre_indexed);
                                // Incrementally resolve imports that can be resolved now
                                // This distributes import resolution work across the indexing phase
                                engine.resolve_imports_incremental();
                            }

                            total_indexed += batch_indexed_count;

                            // Update progress
                            broadcast_progress(&index_progress, &index_progress_tx, |p| {
                                p.files_indexed = total_indexed;
                            });

                            if batch_num.is_multiple_of(10) {
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
                    let mut engine = index_engine
                        .write()
                        .expect("Failed to acquire write lock on search engine");
                    engine.index_batch(pre_indexed);
                    // Resolve imports for the final batch
                    engine.resolve_imports_incremental();
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
            broadcast_progress(&index_progress, &index_progress_tx, |p| {
                p.files_discovered = final_discovered;
                p.total_batches = batch_num;
                p.current_batch = batch_num;
            });

            // Resolve any remaining import relationships that couldn't be resolved incrementally
            // Most imports should already be resolved during the indexing phase
            {
                let mut engine = index_engine
                    .write()
                    .expect("Failed to acquire write lock on search engine");
                let pending_count = engine.pending_imports_count();

                if pending_count > 0 {
                    broadcast_progress(&index_progress, &index_progress_tx, |p| {
                        p.status = IndexingStatus::ResolvingImports;
                        p.current_path = None;
                        p.message = format!(
                            "Resolving {} remaining import dependencies...",
                            pending_count
                        );
                    });

                    info!(
                        pending_imports = pending_count,
                        "Resolving remaining import dependencies..."
                    );
                    engine.resolve_imports();
                }

                // Finalize the index for optimal query performance
                info!("Finalizing index...");
                engine.finalize();

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

            // Get indexed text size from the engine
            let indexed_size = {
                let engine = index_engine
                    .read()
                    .expect("Failed to acquire read lock on search engine");
                engine.get_stats().total_size
            };

            // Get current process memory usage
            let process_memory = {
                use sysinfo::{Pid, ProcessesToUpdate, System};
                let pid = Pid::from_u32(std::process::id());
                let mut sys = System::new();
                sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
                sys.process(pid).map(|p| p.memory()).unwrap_or(0)
            };

            // Format sizes for human readability
            let format_bytes = |bytes: u64| -> String {
                if bytes >= 1_073_741_824 {
                    format!("{:.2} GB", bytes as f64 / 1_073_741_824.0)
                } else if bytes >= 1_048_576 {
                    format!("{:.2} MB", bytes as f64 / 1_048_576.0)
                } else if bytes >= 1024 {
                    format!("{:.2} KB", bytes as f64 / 1024.0)
                } else {
                    format!("{} bytes", bytes)
                }
            };

            // Format numbers with thousand separators
            let format_number = |n: usize| -> String {
                let s = n.to_string();
                let mut result = String::new();
                for (i, c) in s.chars().rev().enumerate() {
                    if i > 0 && i % 3 == 0 {
                        result.push('_');
                    }
                    result.push(c);
                }
                result.chars().rev().collect()
            };

            info!(
                elapsed_secs = format!("{:.1}", elapsed.as_secs_f64()),
                files_indexed = %format_number(total_indexed),
                files_discovered = %format_number(final_discovered),
                files_per_sec = format!("{:.0}", files_per_sec),
                indexed_size = %format_bytes(indexed_size),
                process_memory = %format_bytes(process_memory),
                "Background indexing completed"
            );

            // Save index after build if configured
            if indexer_config.save_after_build {
                if let Some(ref index_path_str) = indexer_config.index_path {
                    let index_path = std::path::Path::new(index_path_str);
                    info!(path = %index_path.display(), "Saving index to disk...");

                    match index_engine.read() {
                        Ok(engine) => {
                            if let Err(e) = engine.save_index(index_path, &indexer_config) {
                                tracing::error!(
                                    error = %e,
                                    path = %index_path.display(),
                                    "Failed to save index"
                                );
                            } else {
                                info!(
                                    path = %index_path.display(),
                                    files = engine.get_stats().num_files,
                                    "Index saved successfully"
                                );
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "Failed to acquire read lock to save index");
                        }
                    }
                }
            }

            // Update progress: completed
            broadcast_progress(&index_progress, &index_progress_tx, |p| {
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
            });
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
