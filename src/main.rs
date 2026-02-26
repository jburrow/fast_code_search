use anyhow::Result;
use clap::Parser;
use fast_code_search::config::Config;
use fast_code_search::diagnostics;
use fast_code_search::search::{
    create_progress_broadcaster, run_background_indexer, BackgroundIndexerConfig, FileChange,
    FileWatcher, IndexingProgress, ProgressBroadcaster, SharedIndexingProgress, WatcherConfig,
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
            run_background_indexer(BackgroundIndexerConfig {
                indexer_config,
                engine: index_engine,
                progress: index_progress,
                progress_tx: index_progress_tx,
            });
        });
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
        info!("Starting file watcher for incremental indexing");

        std::thread::spawn(move || {
            let watcher_config = WatcherConfig {
                paths: watch_paths,
                exclude_patterns: watch_exclude,
                ..WatcherConfig::default()
            };
            match FileWatcher::new(watcher_config) {
                Ok(watcher) => {
                    info!("File watcher started");
                    loop {
                        match watcher.recv_timeout(std::time::Duration::from_secs(1)) {
                            Some(FileChange::Modified(path)) => {
                                tracing::debug!(path = %path.display(), "File modified, updating index");
                                if let Ok(mut engine) = watch_engine.write() {
                                    if let Err(e) = engine.update_file(&path) {
                                        tracing::warn!(
                                            path = %path.display(),
                                            error = %e,
                                            "Failed to update file in index"
                                        );
                                    }
                                }
                            }
                            Some(FileChange::Renamed { from: _, to }) => {
                                tracing::debug!(path = %to.display(), "File renamed, indexing new path");
                                if let Ok(mut engine) = watch_engine.write() {
                                    if let Err(e) = engine.update_file(&to) {
                                        tracing::warn!(
                                            path = %to.display(),
                                            error = %e,
                                            "Failed to index renamed file"
                                        );
                                    }
                                }
                            }
                            Some(FileChange::Deleted(path)) => {
                                // Engine does not yet support file removal from index;
                                // log for observability and no-op.
                                tracing::debug!(
                                    path = %path.display(),
                                    "File deleted (removal from index not yet supported)"
                                );
                            }
                            None => {} // recv_timeout returned nothing, loop again
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to start file watcher");
                }
            }
        });
    }

    // Create gRPC service with shared engine
    let search_service = server::create_server_with_engine(shared_engine.clone());

    info!(version = env!("CARGO_PKG_VERSION"), address = %addr, "Fast Code Search Server starting");
    info!(grpc_endpoint = %format!("grpc://{}", addr), "gRPC endpoint");
    info!("Ready to accept connections");

    Server::builder()
        .trace_fn(|_| tracing::info_span!("grpc"))
        .add_service(search_service)
        .serve(addr)
        .await?;

    // Flush pending OTel spans on shutdown
    telemetry::shutdown_telemetry();

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
