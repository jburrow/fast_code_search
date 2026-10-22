//! Searches while the watcher applies a stream of changes.
//!
//! Generates (or reuses) a tree of small Python scripts, builds the index the
//! way the server does, starts the real watch loop, and then rewrites,
//! creates and deletes files at a steady rate while reader threads search
//! the way the web and gRPC handlers do. Reports how many searches were
//! answered, how many were turned away as "index is being updated", and the
//! search latency percentiles.
//!
//! ```text
//! cargo run --release --example watch_load -- --dir /tmp/fcs-load --files 200000 \
//!     [--burst 200] [--interval-ms 100] [--seconds 20] [--readers 4] [--mixed]
//! ```
//!
//! `--mixed` searches the way the web handler does (`SearchEngine::run` with
//! the default limits), cycling through rare, common and regex queries, so
//! some searches scan most of the tree. A big `--burst` with a long
//! `--interval-ms` is a bulk change (a branch switch, a formatter run).

use clap::Parser;
use fast_code_search::config::IndexerConfig;
use fast_code_search::search::{
    create_progress_broadcaster, engine_lock, run_background_indexer, watch_loop,
    BackgroundIndexerConfig, IndexingProgress, SearchEngine, WatchLoopConfig,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
struct Args {
    /// Where the generated tree lives; reused when it already holds `--files`.
    #[arg(long)]
    dir: PathBuf,
    /// Number of generated scripts.
    #[arg(long, default_value_t = 100_000)]
    files: usize,
    /// Files rewritten per tick (plus one create and one delete).
    #[arg(long, default_value_t = 200)]
    burst: usize,
    /// Pause between ticks.
    #[arg(long, default_value_t = 100)]
    interval_ms: u64,
    /// How long changes stream in.
    #[arg(long, default_value_t = 20)]
    seconds: u64,
    /// Concurrent searching threads.
    #[arg(long, default_value_t = 4)]
    readers: usize,
    /// Turn a search away the moment a writer holds the lock (how the
    /// handlers behaved before they waited for it).
    #[arg(long)]
    no_wait: bool,
    /// Search through `SearchEngine::run` with a mix of rare, common and
    /// regex queries instead of one cheap literal.
    #[arg(long)]
    mixed: bool,
}

/// The `--mixed` queries: (regex, text). Common words make a search read
/// most of the tree; the rest are cheap.
const MIXED: &[(bool, &str)] = &[
    (false, "task_17_"),
    (false, "return value"),
    (true, r"default_\w+\(\)"),
    (false, "revision"),
    (true, r"task_\d+_amber"),
    (false, "helpers.default_cobalt"),
];

/// One search the way the web handler runs it.
fn mixed_search(engine: &SearchEngine, n: usize) -> usize {
    use fast_code_search::search::{SearchMode, SearchRequest, SearchSettings};
    let (regex, text) = MIXED[n % MIXED.len()];
    let request = SearchRequest {
        query: text.to_string(),
        mode: if regex {
            SearchMode::Regex
        } else {
            SearchMode::Text
        },
        ..Default::default()
    };
    let limits = request
        .limits(&SearchSettings::default(), Some(Duration::from_secs(30)))
        .unwrap();
    engine
        .run(&request, limits)
        .map(|o| o.matches.len())
        .unwrap_or(0)
}

const WORDS: &[&str] = &[
    "amber", "basalt", "cobalt", "dune", "ember", "fjord", "garnet", "harbor", "indigo", "jasper",
];

fn script(i: usize, rev: u64) -> String {
    let w = WORDS[i % WORDS.len()];
    let v = WORDS[(i / 7 + rev as usize) % WORDS.len()];
    format!(
        "import os\nfrom . import helpers\n\n\ndef task_{i}_{w}(ctx):\n    \"\"\"{w} {v} revision {rev}\"\"\"\n    value = ctx.get(\"{v}_{i}\")\n    if value is None:\n        return helpers.default_{w}()\n    return value * {rev}\n"
    )
}

fn path_of(root: &Path, i: usize) -> PathBuf {
    root.join(format!("pkg{:04}", i / 1000))
        .join(format!("job_{i}.py"))
}

fn generate(root: &Path, files: usize) {
    let marker = root.join(format!(".generated-{files}"));
    if marker.exists() {
        eprintln!("reusing {} ({files} files)", root.display());
        return;
    }
    let _ = std::fs::remove_dir_all(root);
    let t = Instant::now();
    use rayon::prelude::*;
    (0..files.div_ceil(1000)).into_par_iter().for_each(|d| {
        let dir = root.join(format!("pkg{d:04}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("helpers.py"),
            "def default_amber():\n    return 0\n",
        )
        .unwrap();
        for i in d * 1000..((d + 1) * 1000).min(files) {
            std::fs::write(path_of(root, i), script(i, 0)).unwrap();
        }
    });
    std::fs::write(marker, "").unwrap();
    eprintln!("generated {files} files in {:.1?}", t.elapsed());
}

fn percentile(sorted: &[Duration], p: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

fn main() {
    let args = Args::parse();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
    generate(&args.dir, args.files);
    let root = args.dir.canonicalize().unwrap();
    let config = IndexerConfig {
        paths: vec![root.to_string_lossy().to_string()],
        ..Default::default()
    };
    let engine = Arc::new(RwLock::new(SearchEngine::new()));
    let t = Instant::now();
    run_background_indexer(BackgroundIndexerConfig {
        indexer_config: config.clone(),
        engine: engine.clone(),
        progress: Arc::new(RwLock::new(IndexingProgress::default())),
        progress_tx: create_progress_broadcaster(),
        shutdown: Arc::new(AtomicBool::new(false)),
    });
    eprintln!(
        "indexed {} files in {:.1?}",
        engine.read().unwrap().get_stats().num_files,
        t.elapsed()
    );

    let shutdown = Arc::new(AtomicBool::new(false));
    let watcher = watch_loop::start_watcher(&config).expect("watcher");
    let watch = {
        let cfg = WatchLoopConfig {
            indexer: config.clone(),
            engine: engine.clone(),
            shutdown: shutdown.clone(),
            batch_window: watch_loop::DEFAULT_BATCH_WINDOW,
        };
        std::thread::spawn(move || watch_loop::run(watcher, cfg))
    };

    let stop = Arc::new(AtomicBool::new(false));
    let served = Arc::new(AtomicU64::new(0));
    let turned_away = Arc::new(AtomicU64::new(0));
    let latencies = Arc::new(Mutex::new(Vec::<Duration>::new()));
    let readers: Vec<_> = (0..args.readers)
        .map(|r| {
            let (no_wait, mixed) = (args.no_wait, args.mixed);
            let (engine, stop, served, turned_away, latencies) = (
                engine.clone(),
                stop.clone(),
                served.clone(),
                turned_away.clone(),
                latencies.clone(),
            );
            std::thread::spawn(move || {
                let mut n = r;
                while !stop.load(Ordering::Relaxed) {
                    n += 1;
                    let query = format!("task_{}_", n * 7919 % 1000);
                    let t = Instant::now();
                    let guard = if no_wait {
                        engine.try_read().ok()
                    } else {
                        engine_lock::read_within(&engine, engine_lock::SEARCH_READ_WAIT)
                    };
                    match guard {
                        Some(guard) => {
                            if mixed {
                                std::hint::black_box(mixed_search(&guard, n));
                            } else {
                                std::hint::black_box(guard.search(&query, 50));
                            }
                            drop(guard);
                            served.fetch_add(1, Ordering::Relaxed);
                            latencies.lock().unwrap().push(t.elapsed());
                        }
                        None => {
                            turned_away.fetch_add(1, Ordering::Relaxed);
                            // A client retrying after a short pause.
                            std::thread::sleep(Duration::from_millis(250));
                        }
                    }
                }
            })
        })
        .collect();

    let t = Instant::now();
    let mut rev = 1u64;
    let mut next_new = args.files;
    let mut next_delete = 0usize;
    let mut changed = 0usize;
    while t.elapsed() < Duration::from_secs(args.seconds) {
        for k in 0..args.burst {
            let i = (rev as usize * 104_729 + k * 7_919) % args.files;
            if std::fs::write(path_of(&root, i), script(i, rev)).is_ok() {
                changed += 1;
            }
        }
        let _ = std::fs::write(
            path_of(&root, next_new % args.files).with_file_name(format!("new_{next_new}.py")),
            script(next_new, rev),
        );
        next_new += 1;
        let _ = std::fs::remove_file(path_of(&root, next_delete * 997 % args.files));
        next_delete += 1;
        rev += 1;
        std::thread::sleep(Duration::from_millis(args.interval_ms));
    }
    // Let the last burst land.
    std::thread::sleep(Duration::from_secs(3));
    stop.store(true, Ordering::Relaxed);
    for r in readers {
        r.join().unwrap();
    }
    shutdown.store(true, Ordering::Release);
    watch.join().unwrap();

    let mut lat = latencies.lock().unwrap().clone();
    lat.sort();
    let served = served.load(Ordering::Relaxed);
    let away = turned_away.load(Ordering::Relaxed);
    println!(
        "files={} changed={} searches served={} turned_away={} ({:.1}%) mean={:.1?} p50={:.1?} p95={:.1?} p99={:.1?} max={:.1?}",
        args.files,
        changed,
        served,
        away,
        100.0 * away as f64 / (served + away).max(1) as f64,
        lat.iter().sum::<Duration>() / (lat.len().max(1) as u32),
        percentile(&lat, 0.5),
        percentile(&lat, 0.95),
        percentile(&lat, 0.99),
        lat.last().copied().unwrap_or_default(),
    );
}
