use anyhow::Result;
use clap::Parser;
use fast_code_search::config::Config;
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
    let shared_engine = std::sync::Arc::new(std::sync::Mutex::new(fast_code_search::search::SearchEngine::new()));

    // Start web server first if enabled (so UI is available during indexing)
    if config.server.enable_web_ui {
        let web_addr = config.server.web_address.clone();
        let web_engine = shared_engine.clone();
        info!(web_address = %web_addr, "Starting Web UI server");
        
        tokio::spawn(async move {
            let router = web::create_router(web_engine);
            let listener = tokio::net::TcpListener::bind(&web_addr).await.unwrap();
            info!(address = %web_addr, "Web UI available at http://{}", web_addr);
            axum::serve(listener, router).await.unwrap();
        });
    }

    // Start background indexing if enabled
    if !args.no_auto_index && !config.indexer.paths.is_empty() {
        let indexer_config = config.indexer.clone();
        let index_engine = shared_engine.clone();
        info!("Starting background indexing");
        
        std::thread::spawn(move || {
            use std::time::Instant;
            let total_start = Instant::now();
            info!("Background indexing {} path(s)", indexer_config.paths.len());
            
            for path_str in &indexer_config.paths {
                let path = std::path::Path::new(path_str);
                if !path.exists() {
                    tracing::warn!(path = %path_str, "Path does not exist, skipping");
                    continue;
                }
                
                info!(path = %path_str, "Indexing path");
                
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
                    let should_exclude = indexer_config.exclude_patterns.iter().any(|pattern| {
                        path_str_check.contains(pattern.trim_matches('*').trim_matches('/'))
                    });
                    
                    if should_exclude {
                        continue;
                    }
                    
                    // Skip binary files
                    if let Some(ext) = entry_path.extension() {
                        let ext = ext.to_string_lossy().to_lowercase();
                        if matches!(ext.as_str(), "exe" | "dll" | "so" | "dylib" | "bin" | "o" | "a" | 
                            "png" | "jpg" | "jpeg" | "gif" | "ico" | "bmp" | "zip" | "tar" | "gz" | "7z") {
                            continue;
                        }
                    }
                    
                    // Index the file (method reads content internally)
                    let mut engine = index_engine.lock().unwrap();
                    let _ = engine.index_file(entry_path);
                }
            }
            
            // Resolve all import relationships after indexing completes
            {
                let mut engine = index_engine.lock().unwrap();
                info!("Resolving import dependencies...");
                engine.resolve_imports();
                let stats = engine.get_stats();
                info!(
                    dependency_edges = stats.dependency_edges,
                    "Import resolution completed"
                );
            }
            
            let elapsed = total_start.elapsed();
            info!(elapsed_secs = elapsed.as_secs_f64(), "Background indexing completed");
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
