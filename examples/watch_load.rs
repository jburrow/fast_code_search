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
//! `--capture-dir DIR` turns diagnostic captures on with the default
//! thresholds (as `[diagnostics] capture = true` does) and reports what was
//! captured: on a healthy run, nothing should be.
//!
//! `--mixed` searches the way the web handler does (`SearchEngine::run` with
//! the default limits), cycling through rare, common and regex queries, so
//! some searches scan most of the tree. A big `--burst` with a long
//! `--interval-ms` is a bulk change (a branch switch, a formatter run).

#[path = "support/load_tree.rs"]
mod load_tree;

use clap::Parser;
use fast_code_search::config::{DiagnosticsConfig, IndexerConfig};
use fast_code_search::diagnostics::capture;
use fast_code_search::search::{
    create_progress_broadcaster, engine_lock, run_background_indexer, watch_loop,
    BackgroundIndexerConfig, IndexingProgress, SearchEngine, WatchLoopConfig,
};
use load_tree::{generate, path_of, script, MIXED};
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
    /// After the changes stop, how long the index may take to match the
    /// disk again before the run is reported as stuck.
    #[arg(long, default_value_t = 300)]
    settle_seconds: u64,
    /// Persist the index here and checkpoint it every `--save-after`
    /// applied changes, as a server with `save_after_updates` does.
    #[arg(long)]
    index_path: Option<PathBuf>,
    /// See `--index-path`.
    #[arg(long, default_value_t = 10_000)]
    save_after: usize,
    /// Turn diagnostic captures on, writing to this folder, with the
    /// default `[diagnostics]` thresholds.
    #[arg(long)]
    capture_dir: Option<PathBuf>,
    /// After the build, time the self-checks (`check_invariants`, and
    /// `check_file` over this many files), then exit: the evidence for the
    /// scrub's defaults.
    #[arg(long)]
    integrity_cost: Option<usize>,
}

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

/// Files on disk the index does not have as they are now, and changes the
/// watcher reports as not applied yet.
fn stale_files(engine: &RwLock<SearchEngine>, root: &Path) -> (usize, usize) {
    let on_disk: Vec<PathBuf> = walk(root);
    let Some(engine) = engine_lock::read_within(engine, engine_lock::SEARCH_READ_WAIT) else {
        return (usize::MAX, usize::MAX);
    };
    let stale: Vec<&PathBuf> = on_disk
        .iter()
        .filter(|p| match engine.find_file_id_exact(p) {
            Some(id) => !engine.is_unchanged_on_disk(id, p),
            None => true,
        })
        .collect();
    if std::env::var_os("WATCH_LOAD_SHOW_STALE").is_some() {
        for p in stale.iter().take(5) {
            eprintln!(
                "  stale: {} (indexed: {})",
                p.display(),
                engine.find_file_id_exact(p).is_some()
            );
        }
    }
    let stale = stale.len();
    let indexed = engine.get_stats().num_files;
    // Deleted files still in the index.
    let gone = indexed.saturating_sub(on_disk.len());
    (stale + gone, engine.freshness().pending_changes)
}

fn walk(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out
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
    if let Some(dir) = &args.capture_dir {
        let mut settings = capture::Settings::resolve(&DiagnosticsConfig::default(), None);
        settings.dir = dir.clone();
        capture::init(settings).expect("capture dir");
    }
    generate(&args.dir, args.files);
    let root = args.dir.canonicalize().unwrap();
    let config = IndexerConfig {
        paths: vec![root.to_string_lossy().to_string()],
        index_path: args
            .index_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string()),
        save_after_updates: if args.index_path.is_some() {
            args.save_after
        } else {
            0
        },
        ..Default::default()
    };
    let engine = Arc::new(RwLock::new(SearchEngine::new()));
    let progress = Arc::new(RwLock::new(IndexingProgress::default()));
    let watchdog_stop = Arc::new(AtomicBool::new(false));
    capture::start_watchdog(capture::Watched {
        engine: engine.clone(),
        progress: Some(progress.clone()),
        shutdown: watchdog_stop.clone(),
    });
    let t = Instant::now();
    run_background_indexer(BackgroundIndexerConfig {
        indexer_config: config.clone(),
        engine: engine.clone(),
        progress,
        progress_tx: create_progress_broadcaster(),
        shutdown: Arc::new(AtomicBool::new(false)),
    });
    eprintln!(
        "indexed {} files in {:.1?}",
        engine.read().unwrap().get_stats().num_files,
        t.elapsed()
    );

    if let Some(n) = args.integrity_cost {
        let e = engine.read().unwrap();
        for _ in 0..3 {
            let t = Instant::now();
            let report = e.check_invariants();
            eprintln!(
                "check_invariants: {:.0?} over {} files, {} violations",
                t.elapsed(),
                report.files,
                report.total()
            );
        }
        let ids = e.live_file_ids();
        let ids = &ids[..n.min(ids.len())];
        let t = Instant::now();
        let current = ids
            .iter()
            .filter(|&&id| e.check_file(id) == Some(fast_code_search::search::FileCheck::Current))
            .count();
        let took = t.elapsed();
        eprintln!(
            "check_file: {} files in {:.0?} ({:.0} files/s on one thread), {} current",
            ids.len(),
            took,
            ids.len() as f64 / took.as_secs_f64(),
            current
        );
        return;
    }

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
                    let mut probe = capture::SearchProbe::start(
                        "load",
                        if mixed { "mixed" } else { "text" },
                        &query,
                    );
                    let guard: Option<Box<dyn std::ops::Deref<Target = SearchEngine>>> = if no_wait
                    {
                        engine.try_read().ok().map(|g| Box::new(g) as _)
                    } else {
                        engine_lock::read_within(&engine, engine_lock::SEARCH_READ_WAIT)
                            .map(|g| Box::new(g) as _)
                    };
                    match guard {
                        Some(guard) => {
                            probe.locked();
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
    // The searches keep going while the watcher catches up: a watcher that
    // stops making progress shows up here as files the index never gets.
    let streamed_for = t.elapsed();
    let settle = Instant::now();
    let mut last_report = Instant::now();
    let settled = loop {
        let (stale, pending) = stale_files(&engine, &root);
        if stale == 0 && pending == 0 {
            break true;
        }
        if settle.elapsed() > Duration::from_secs(args.settle_seconds) {
            eprintln!(
                "NOT SETTLED: {stale} files differ from the index, {pending} changes pending"
            );
            break false;
        }
        if last_report.elapsed() > Duration::from_secs(10) {
            eprintln!(
                "settling: {stale} files differ, {pending} changes pending, {:.0?}",
                settle.elapsed()
            );
            last_report = Instant::now();
        }
        std::thread::sleep(Duration::from_secs(1));
    };
    let settled_in = settle.elapsed();
    stop.store(true, Ordering::Relaxed);
    for r in readers {
        r.join().unwrap();
    }
    shutdown.store(true, Ordering::Release);
    let shutting_down = Instant::now();
    watch.join().unwrap();
    eprintln!(
        "streamed {streamed_for:.0?}; settled={settled} in {settled_in:.1?}; watcher stopped in {:.1?}",
        shutting_down.elapsed()
    );

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
    watchdog_stop.store(true, Ordering::Release);
    if let Some(dir) = &args.capture_dir {
        // Captures are written on threads of their own; let them finish.
        std::thread::sleep(Duration::from_secs(2));
        let (captures, _) = capture::load_captures(dir);
        let mut kinds: Vec<String> = captures
            .iter()
            .map(|(_, c)| c.trigger.kind.clone())
            .collect();
        kinds.sort();
        println!("captures={} {:?}", captures.len(), kinds);
    }
}
