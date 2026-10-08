use anyhow::Result;
use clap::Parser;
use fast_code_search::config::Config;
use fast_code_search::diagnostics;
use fast_code_search::search::{
    apply_changes, create_progress_broadcaster, run_background_indexer,
    save_after_watcher_shutdown, save_on_watcher_update, BackgroundIndexerConfig, FileWatcher,
    IndexingProgress, ProgressBroadcaster, SharedIndexingProgress, WatcherConfig,
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

    /// gRPC listen address (overrides config file)
    #[arg(short, long, value_name = "ADDR")]
    address: Option<String>,

    /// Web UI / REST listen address (overrides config file)
    #[arg(long, value_name = "ADDR")]
    web_address: Option<String>,

    /// Additional paths to index (can be repeated, adds to config file paths)
    #[arg(short, long = "index", value_name = "PATH")]
    index_paths: Vec<String>,

    /// Skip automatic indexing on startup
    #[arg(long)]
    no_auto_index: bool,

    /// Do not start the gRPC API (same as `enable_grpc = false` in the config)
    #[arg(long)]
    no_grpc: bool,

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

    // Load configuration. Logging is not set up yet (it needs the config's
    // telemetry section), so where the config came from and any warnings are
    // returned and logged below instead of being lost.
    let (mut config, config_source, config_warnings) = load_config(&args)?;

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

    info!(source = %config_source, "Configuration source");
    for w in &config_warnings {
        tracing::warn!("Config: {w}");
        diagnostics::record_problem(format!("Config: {w}"));
    }
    info!(
        server_address = %config.server.address,
        paths_count = config.indexer.paths.len(),
        "Configuration loaded"
    );
    log_storage_health(&config.indexer);
    let _index_lock = lock_index(&mut config);

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
    // Shared by the REST router and the gRPC service so
    // `max_concurrent_searches` bounds the process, not each surface (or
    // each gRPC connection) separately.
    let search_permits = std::sync::Arc::new(tokio::sync::Semaphore::new(
        config.server.max_concurrent_searches.max(1),
    ));
    let request_timeout = std::time::Duration::from_secs(config.server.request_timeout_secs.max(1));

    let mut web_handle: Option<tokio::task::JoinHandle<()>> = None;
    let mut indexer_handle: Option<std::thread::JoinHandle<()>> = None;
    let mut watcher_handle: Option<std::thread::JoinHandle<()>> = None;

    // CLI flag takes precedence over config file value
    let static_dir = args.static_dir.clone().or_else(|| {
        config
            .server
            .static_dir
            .as_ref()
            .map(std::path::PathBuf::from)
    });

    // Start web server first if enabled (so UI is available during indexing)
    let mut web_listener: Option<tokio::net::TcpListener> = None;
    let mut web_status = "disabled".to_string();
    if config.server.enable_web_ui {
        let web_addr = config.server.web_address.clone();

        if let Some(ref dir) = static_dir {
            info!(dir = %dir.display(), "Serving static UI files from disk (development mode)");
        } else {
            info!("Serving static UI files from compiled binary");
        }

        info!(web_address = %web_addr, "Starting Web UI server");

        // Bind here (not inside the task) so a port conflict is reported up
        // front. It is not fatal on its own: indexing and the gRPC API still
        // run, and startup only fails if no API could be started at all.
        match tokio::net::TcpListener::bind(&web_addr).await {
            Ok(listener) => {
                info!(address = %web_addr, "Web UI available at http://{}", web_addr);
                web_status = format!("http://{web_addr}");
                web_listener = Some(listener);
            }
            Err(e) => {
                let msg = format!(
                    "Web UI / REST API NOT started: cannot listen on {web_addr}: {e}. {}",
                    diagnostics::port_hint(&e, &web_addr, "server.web_address", "--web-address")
                );
                tracing::error!("{msg}");
                diagnostics::record_problem(msg);
                web_status = format!("NOT running ({web_addr}: {e})");
            }
        }
    }
    if let Some(listener) = web_listener {
        let web_addr = config.server.web_address.clone();
        let web_engine = shared_engine.clone();
        let web_progress = shared_progress.clone();
        let web_progress_tx = progress_tx.clone();
        let web_shutdown_rx = shutdown_rx.clone();
        let mut router_options = web::RouterOptions::from(&config.server);
        router_options.indexer_config = Some(config.indexer.clone());
        router_options.search_permits = Some(search_permits.clone());
        web_handle = Some(tokio::spawn(async move {
            let router = web::create_router_with_options(
                web_engine,
                web_progress,
                web_progress_tx,
                static_dir,
                &router_options,
            );
            if let Err(e) = axum::serve(listener, router)
                .with_graceful_shutdown(wait_for_shutdown(web_shutdown_rx))
                .await
            {
                tracing::error!(error = %e, address = %web_addr, "Web UI server stopped unexpectedly");
            }
        }));
    }

    // Bind the gRPC port before indexing starts, for the same reason: a busy
    // port is reported now, and only disables gRPC rather than shutting the
    // whole server down (which used to interrupt indexing at 0 files).
    let mut grpc_listener: Option<tokio::net::TcpListener> = None;
    let grpc_status = if !config.server.enable_grpc {
        info!("gRPC API disabled (enable_grpc = false or --no-grpc)");
        "disabled".to_string()
    } else {
        let grpc_addr = &config.server.address;
        match tokio::net::TcpListener::bind(grpc_addr).await {
            Ok(listener) => {
                grpc_listener = Some(listener);
                format!("grpc://{grpc_addr}")
            }
            Err(e) => {
                let msg = format!(
                    "gRPC API NOT started: cannot listen on {grpc_addr}: {e}. \
                     The rest of the server keeps running without it. {}",
                    diagnostics::port_hint(&e, grpc_addr, "server.address", "--address")
                );
                tracing::error!("{msg}");
                diagnostics::record_problem(msg);
                format!("NOT running ({grpc_addr}: {e})")
            }
        }
    };

    if grpc_listener.is_none() && web_handle.is_none() {
        if config.server.enable_grpc || config.server.enable_web_ui {
            anyhow::bail!(
                "No API could be started (gRPC: {grpc_status}; web UI: {web_status}), \
                 so nothing could search the index. See the errors above."
            );
        }
        tracing::warn!("Both the gRPC API and the web UI are disabled; only indexing will run");
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
                Ok(mut watcher) => {
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
                        if let Some(first) = watcher.recv_timeout(std::time::Duration::from_secs(1))
                        {
                            // Gather the rest of the burst (branch switch, formatter
                            // run, generated files) for a short window so it is
                            // applied under ONE write lock with one posting scan.
                            let mut changes = vec![first];
                            let deadline = std::time::Instant::now()
                                + std::time::Duration::from_millis(WATCH_BATCH_WINDOW_MS);
                            loop {
                                let remaining =
                                    deadline.saturating_duration_since(std::time::Instant::now());
                                if remaining.is_zero() {
                                    break;
                                }
                                match watcher.recv_timeout(remaining) {
                                    Some(c) => changes.push(c),
                                    None => break,
                                }
                            }
                            tracing::debug!(
                                count = changes.len(),
                                "Applying file changes to index"
                            );
                            // New directories need their own watches on
                            // platforms that watch per directory.
                            for change in &changes {
                                if let fast_code_search::search::FileChange::Modified(p)
                                | fast_code_search::search::FileChange::Renamed {
                                    to: p, ..
                                } = change
                                {
                                    if p.is_dir() {
                                        watcher.ensure_watched(p);
                                    }
                                }
                            }
                            let outcome = with_engine_write(&watch_engine, |engine| {
                                apply_changes(engine, &changes, &watch_indexer_config)
                            });
                            if let Some(outcome) = outcome.filter(|o| o.changed()) {
                                tracing::debug!(
                                    events = changes.len(),
                                    indexed = outcome.indexed,
                                    removed = outcome.removed,
                                    "File changes applied"
                                );
                                watcher_updates_total += outcome.indexed + outcome.removed;
                                if save_on_watcher_update(
                                    &watch_indexer_config,
                                    &watch_engine,
                                    watcher_updates_total,
                                ) {
                                    watcher_updates_total = 0;
                                }
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
    let search_service = server::create_server_with_engine_config_limits(
        shared_engine.clone(),
        &config.indexer,
        search_permits.clone(),
        request_timeout,
    );

    let persistence = match config.indexer.index_path.as_deref() {
        Some(p) if config.indexer.index_read_only => {
            format!("{p} (read-only: another server holds its lock)")
        }
        Some(p) => p.to_string(),
        None => "OFF: memory only, rebuilt on every start (index_path not set)".to_string(),
    };
    diagnostics::set_service_endpoints(&grpc_status, &web_status, &persistence);
    info!(
        version = env!("CARGO_PKG_VERSION"),
        web_ui = %web_status,
        grpc = %grpc_status,
        index = %persistence,
        "Fast Code Search Server ready"
    );

    // Standard gRPC health service (grpc.health.v1) so load balancers and
    // grpcurl can probe liveness the usual way.
    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<fast_code_search::server::search_proto::code_search_server::CodeSearchServer<
            fast_code_search::server::CodeSearchService,
        >>()
        .await;

    let serve_result = match grpc_listener {
        Some(listener) => Server::builder()
            .timeout(request_timeout)
            .concurrency_limit_per_connection(config.server.max_concurrent_searches.max(1))
            .trace_fn(|req| tracing::info_span!("grpc", path = %req.uri().path()))
            .add_service(health_service)
            .add_service(search_service)
            .serve_with_incoming_shutdown(
                tonic::transport::server::TcpIncoming::from(listener),
                wait_for_shutdown(shutdown_rx.clone()),
            )
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "gRPC server stopped unexpectedly");
                e
            }),
        None => {
            wait_for_shutdown(shutdown_rx.clone()).await;
            Ok(())
        }
    };

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

    // A second Ctrl+C during a slow shutdown save used to be swallowed
    // (the runtime owns SIGINT), leaving SIGKILL as the only way out.
    tokio::spawn(async {
        if tokio::signal::ctrl_c().await.is_ok() {
            tracing::warn!("Second Ctrl+C: exiting immediately without saving");
            std::process::exit(130);
        }
    });
}

/// Resolve once the shutdown watch channel is set (or its sender is gone).
async fn wait_for_shutdown(mut rx: tokio::sync::watch::Receiver<bool>) {
    while !*rx.borrow() {
        if rx.changed().await.is_err() {
            break;
        }
    }
}

/// How long the watcher keeps collecting events after the first one before
/// applying the batch. The debouncer already coalesces per-file noise; this
/// window groups *different* files touched by one operation.
const WATCH_BATCH_WINDOW_MS: u64 = 200;

/// Load the config and apply CLI overrides. Returns the config, a
/// description of where it came from, and validation warnings, for the
/// caller to log once tracing is initialised.
fn load_config(args: &Args) -> Result<(Config, String, Vec<String>)> {
    let base_config = if let Some(ref config_path) = args.config {
        // Explicit config file specified
        if !config_path.exists() {
            anyhow::bail!(
                "Config file not found: {}\nUse --init {} to generate a template.",
                config_path.display(),
                config_path.display()
            );
        }
        let source = format!("{} (--config)", config_path.display());
        (Config::from_file(config_path)?, source)
    } else {
        // Try default locations
        match Config::from_default_locations()? {
            Some((config, path)) => (config, format!("{} (default location)", path.display())),
            None => (
                Config::default(),
                "none: no --config given and no file at $FCS_CONFIG, ./fast_code_search.toml \
                 or ~/.config/fast_code_search/config.toml, so built-in defaults are used"
                    .to_string(),
            ),
        }
    };
    let (base_config, source) = base_config;

    // Apply CLI overrides
    let mut config = base_config.with_overrides(
        args.address.clone(),
        args.web_address.clone(),
        args.index_paths.clone(),
    );
    if args.no_grpc {
        config.server.enable_grpc = false;
    }
    let warnings = config.validate()?;
    Ok((config, source, warnings))
}

/// Take the lock on `index_path` for the life of the process. If another
/// server holds it, say which one and continue read-only: this server loads
/// the index but never saves over the other's.
fn lock_index(
    config: &mut fast_code_search::config::Config,
) -> Option<fast_code_search::index_lock::IndexLock> {
    use fast_code_search::index_lock::{describe_owner, IndexLock, LockError};
    let index_path = config.indexer.index_path.clone()?;
    let enabled = |on: bool, addr: &str| {
        if on {
            addr.to_string()
        } else {
            "off".to_string()
        }
    };
    let owner = describe_owner(
        &enabled(config.server.enable_grpc, &config.server.address),
        &enabled(config.server.enable_web_ui, &config.server.web_address),
    );
    match IndexLock::acquire(std::path::Path::new(&index_path), &owner) {
        Ok(lock) => {
            tracing::debug!(lock = %lock.path().display(), "Index lock acquired");
            Some(lock)
        }
        Err(e @ LockError::HeldByAnother { .. }) => {
            let msg = format!(
                "Index NOT writable: {e}. This server will load the index but never save \
                 it, so the two cannot overwrite each other. Stop the other server \
                 (often an earlier one still running), or give this config its own \
                 index_path."
            );
            tracing::error!("{msg}");
            diagnostics::record_problem(msg);
            config.indexer.index_read_only = true;
            None
        }
        Err(e) => {
            let msg = format!("Index lock unavailable, continuing without it: {e}");
            tracing::warn!("{msg}");
            diagnostics::record_problem(msg);
            None
        }
    }
}

/// Warn about filesystems that are out of inodes or nearly full: the one the
/// index is saved to, and the ones holding the indexed source.
fn log_storage_health(indexer: &fast_code_search::config::IndexerConfig) {
    // Not recorded for diagnostics: the page re-checks storage on each load.
    for problem in fast_code_search::storage::storage_problems(indexer) {
        tracing::warn!("Storage: {problem}");
    }
}
