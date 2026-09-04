use anyhow::{Context, Result};
use clap::Parser;
use fast_code_search::config::Config;
use fast_code_search::diagnostics;
use fast_code_search::search::{
    apply_change, create_progress_broadcaster, run_background_indexer, save_after_watcher_shutdown,
    save_on_watcher_update, BackgroundIndexerConfig, FileWatcher, IndexingProgress,
    ProgressBroadcaster, SharedIndexingProgress, WatcherConfig,
};
use fast_code_search::server;
use fast_code_search::telemetry;
use fast_code_search::utils::SystemLimits;
use fast_code_search::web;
use std::path::PathBuf;
use tonic::transport::Server;
use tracing::{info, Level};

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

    /// Serve static UI files from this directory instead of the embedded assets.
    /// Useful during development: UI changes are visible without recompiling.
    /// Example: --static-dir static
    #[arg(long, value_name = "DIR")]
    static_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

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

    // Initialize tracing subscriber (must come after config load so TOML telemetry values are available)
    let log_level = if args.verbose {
        Level::DEBUG
    } else {
        Level::INFO
    };
    let tele = config.telemetry.clone().with_env_overrides();
    telemetry::init_telemetry(
        tele.enabled,
        &tele.otlp_endpoint,
        &tele.service_name,
        log_level,
    )?;

    // Initialize diagnostics server start time
    diagnostics::init_server_start_time();

    info!(
        server_address = %config.server.address,
        paths_count = config.indexer.paths.len(),
        "Configuration loaded"
    );

    if args.verbose {
        info!(paths = ?config.indexer.paths, "Paths to index");
        info!(exclude_patterns = ?config.indexer.exclude_patterns, "Exclude patterns");
    }

    // Check system limits on Linux and warn if too low
    let limits = SystemLimits::collect();
    limits.log_limits();
    if let Some(warning) = limits.check_and_warn() {
        eprintln!("{}", warning);
        eprintln!("The server will automatically stop indexing at 85% of the limit.");
        eprintln!("Press Ctrl+C to abort or wait 3 seconds to continue...");
        std::thread::sleep(std::time::Duration::from_secs(3));
    }

    let addr = config.server.address.parse()?;

    // Build the global rayon pool up front with 8 MB stacks. tree-sitter
    // recursion runs on this pool during indexing; if a search (par_iter) got
    // there first it would install the default 2 MB stacks instead.
    if let Err(e) = rayon::ThreadPoolBuilder::new()
        .stack_size(8 * 1024 * 1024)
        .build_global()
    {
        tracing::warn!(error = %e, "Global rayon pool already initialized");
    }

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

    // Shutdown coordination: SIGINT/SIGTERM flips the flag (observed by the
    // indexer and watcher threads) and the watch channel (observed by both
    // servers' graceful-shutdown futures).
    let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    {
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            shutdown_signal().await;
            shutdown.store(true, std::sync::atomic::Ordering::Release);
            let _ = shutdown_tx.send(true);
        });
    }
    let mut web_handle: Option<tokio::task::JoinHandle<()>> = None;
    let mut indexer_handle: Option<std::thread::JoinHandle<()>> = None;
    let mut watcher_handle: Option<std::thread::JoinHandle<()>> = None;

    // Start web server first if enabled (so UI is available during indexing)
    if config.server.enable_web_ui {
        let web_addr = config.server.web_address.clone();
        let web_engine = shared_engine.clone();
        let web_progress = shared_progress.clone();
        let web_progress_tx = progress_tx.clone();

        // CLI flag takes precedence over config file value
        let static_dir = args.static_dir.clone().or_else(|| {
            config
                .server
                .static_dir
                .as_ref()
                .map(std::path::PathBuf::from)
        });

        if let Some(ref dir) = static_dir {
            info!(dir = %dir.display(), "Serving static UI files from disk (development mode)");
        } else {
            info!("Serving static UI files from compiled binary");
        }

        info!(web_address = %web_addr, "Starting Web UI server");

        // Bind here (not inside the task) so a port conflict is fatal instead
        // of leaving a half-alive server with no REST API.
        let listener = tokio::net::TcpListener::bind(&web_addr)
            .await
            .with_context(|| format!("Failed to bind Web UI server to {web_addr}"))?;
        info!(address = %web_addr, "Web UI available at http://{}", web_addr);
        let web_shutdown_rx = shutdown_rx.clone();
        let cors_origins = config.server.cors_origins.clone();
        web_handle = Some(tokio::spawn(async move {
            let router = web::create_router_with_cors(
                web_engine,
                web_progress,
                web_progress_tx,
                static_dir,
                &cors_origins,
            );
            if let Err(e) = axum::serve(listener, router)
                .with_graceful_shutdown(wait_for_shutdown(web_shutdown_rx))
                .await
            {
                tracing::error!(error = %e, "Web UI server stopped unexpectedly");
            }
        }));
    }

    // Start background indexing if enabled
    if !args.no_auto_index && !config.indexer.paths.is_empty() {
        let indexer_config = config.indexer.clone();
        let index_engine = shared_engine.clone();
        let index_progress = shared_progress.clone();
        let index_progress_tx = progress_tx.clone();
        info!("Starting background indexing");

        let index_shutdown = shutdown.clone();
        indexer_handle = Some(std::thread::spawn(move || {
            run_background_indexer(BackgroundIndexerConfig {
                indexer_config,
                engine: index_engine,
                progress: index_progress,
                progress_tx: index_progress_tx,
                shutdown: index_shutdown,
            });
        }));
    } else if args.no_auto_index {
        info!("Auto-indexing disabled via --no-auto-index flag");
    } else {
        info!("No paths configured for indexing");
    }

    // Start file watcher for incremental re-indexing if configured
    if config.indexer.watch {
        let watch_engine = shared_engine.clone();
        let watch_paths: Vec<std::path::PathBuf> = config
            .indexer
            .paths
            .iter()
            .map(std::path::PathBuf::from)
            .collect();
        let watch_exclude = config.indexer.exclude_patterns.clone();
        let watch_indexer_config = config.indexer.clone();
        info!("Starting file watcher for incremental indexing");

        let watch_shutdown = shutdown.clone();
        // Same 8 MB stack as the rayon workers: update_file runs tree-sitter on
        // this thread, and a stack overflow is an abort, not a panic.
        let watcher_thread = std::thread::Builder::new()
            .name("file-watcher".into())
            .stack_size(8 * 1024 * 1024);
        let spawned = watcher_thread.spawn(move || {
            let watcher_config = WatcherConfig {
                paths: watch_paths,
                exclude_patterns: watch_exclude,
                ..WatcherConfig::default()
            };
            match FileWatcher::new(watcher_config) {
                Ok(watcher) => {
                    info!("File watcher started");
                    let mut watcher_updates_total: usize = 0;
                    loop {
                        if watch_shutdown.load(std::sync::atomic::Ordering::Acquire) {
                            info!("File watcher stopping for shutdown");
                            save_after_watcher_shutdown(
                                &watch_indexer_config,
                                &watch_engine,
                                watcher_updates_total,
                            );
                            break;
                        }
                        if let Some(change) =
                            watcher.recv_timeout(std::time::Duration::from_secs(1))
                        {
                            tracing::debug!(?change, "Applying file change to index");
                            let outcome = with_engine_write(&watch_engine, |engine| {
                                apply_change(engine, &change, &watch_indexer_config)
                            });
                            if let Some(outcome) = outcome.filter(|o| o.changed()) {
                                tracing::debug!(
                                    indexed = outcome.indexed,
                                    removed = outcome.removed,
                                    "File change applied"
                                );
                                watcher_updates_total += 1;
                                save_on_watcher_update(
                                    &watch_indexer_config,
                                    &watch_engine,
                                    watcher_updates_total,
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to start file watcher");
                    tracing::warn!(
                        "File watching is disabled — the server will not detect file changes \
                        automatically. For large codebases (600k+ files) the default OS inotify \
                        limit is often too low. Fix on Linux: \
                        sudo sysctl -w fs.inotify.max_user_watches=524288 \
                        (add to /etc/sysctl.conf to persist). \
                        To silence this warning, set `watch = false` in your config."
                    );
                }
            }
        });
        match spawned {
            Ok(h) => watcher_handle = Some(h),
            Err(e) => tracing::error!(error = %e, "Failed to spawn file watcher thread"),
        }
    }

    // Create gRPC service with shared engine
    let index_roots: Vec<PathBuf> = config.indexer.paths.iter().map(PathBuf::from).collect();
    let search_service =
        server::create_server_with_engine_scoped(shared_engine.clone(), index_roots);

    info!(version = env!("CARGO_PKG_VERSION"), address = %addr, "Fast Code Search Server starting");
    info!(grpc_endpoint = %format!("grpc://{}", addr), "gRPC endpoint");
    info!("Ready to accept connections");

    let serve_result = Server::builder()
        .trace_fn(|_| tracing::info_span!("grpc"))
        .add_service(search_service)
        .serve_with_shutdown(addr, wait_for_shutdown(shutdown_rx.clone()))
        .await;

    // Whether we got here via a signal or a server error, make sure every
    // background thread sees the flag, then wait for them so the final index
    // save completes before the process exits.
    shutdown.store(true, std::sync::atomic::Ordering::Release);
    info!("Shutting down: waiting for web server, indexer and watcher to finish");
    if let Some(h) = web_handle {
        let _ = h.await;
    }
    let join_threads = tokio::task::spawn_blocking(move || {
        if let Some(h) = indexer_handle {
            if h.join().is_err() {
                tracing::error!("Background indexer thread panicked during shutdown");
            }
        }
        if let Some(h) = watcher_handle {
            if h.join().is_err() {
                tracing::error!("File watcher thread panicked during shutdown");
            }
        }
    });
    let _ = join_threads.await;

    // Flush pending OTel spans on shutdown
    telemetry::shutdown_telemetry();
    info!("Shutdown complete");

    serve_result?;
    Ok(())
}

/// Run `f` under the engine write lock from the watcher thread.
///
/// Recovers from a poisoned lock (a panic elsewhere must not disable the
/// watcher forever) and catches panics inside `f` so one pathological file
/// cannot poison the lock for every search. Returns `None` if `f` panicked.
fn with_engine_write<F, R>(
    engine: &std::sync::Arc<std::sync::RwLock<fast_code_search::search::SearchEngine>>,
    f: F,
) -> Option<R>
where
    F: FnOnce(&mut fast_code_search::search::SearchEngine) -> R,
{
    let mut guard = engine.write().unwrap_or_else(|poisoned| {
        tracing::error!("Search engine lock was poisoned; recovering in file watcher");
        poisoned.into_inner()
    });
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&mut guard))) {
        Ok(result) => Some(result),
        Err(_) => {
            tracing::error!("File watcher update panicked; the change was skipped");
            None
        }
    }
}

/// Resolve when SIGINT (Ctrl+C) or, on Unix, SIGTERM is received.
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %e, "Failed to install Ctrl+C handler");
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to install SIGTERM handler");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("Received Ctrl+C, shutting down"),
        _ = terminate => info!("Received SIGTERM, shutting down"),
    }
}

/// Resolve once the shutdown watch channel is set (or its sender is gone).
async fn wait_for_shutdown(mut rx: tokio::sync::watch::Receiver<bool>) {
    while !*rx.borrow() {
        if rx.changed().await.is_err() {
            break;
        }
    }
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
