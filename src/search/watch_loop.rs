//! The file-watch loop: receive change events, gather each burst, apply it to
//! the engine, and checkpoint the index. Only installing a burst takes the
//! write lock; working out what it means and reading the files do not, so
//! searches keep being answered while changes stream in.
//!
//! This used to live inline in `main.rs`, where no test could reach it; the
//! tests re-implemented their own copy of the loop instead. The server and
//! the watch tests (`tests/watch_harness.rs`) now run this same code.

use crate::config::IndexerConfig;
use crate::search::background_indexer::{save_after_watcher_shutdown, save_on_watcher_update};
use crate::search::engine_lock;
use crate::search::incremental::{
    apply_plan, parse_planned, plan_changes, ChangeOutcome, ChangePlan,
};
use crate::search::watcher::{FileChange, FileWatcher, WatcherConfig};
use crate::search::SearchEngine;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::{debug, error, info};

/// A burst of events (branch switch, formatter run, generated files) is
/// gathered until no event has arrived for this long, so it is applied under
/// ONE write lock with one posting scan.
pub const DEFAULT_BATCH_WINDOW: Duration = Duration::from_millis(200);

/// A burst is applied after at most this long even if events keep coming,
/// so a busy tree still shows its changes.
const MAX_BURST: Duration = Duration::from_secs(2);

/// Settings for [`run`].
#[derive(Clone)]
pub struct WatchLoopConfig {
    /// The indexer settings: eligibility rules and checkpointing.
    pub indexer: IndexerConfig,
    /// The engine to keep in step with the disk.
    pub engine: Arc<RwLock<SearchEngine>>,
    /// Set to stop the loop; it is checked at least once a second.
    pub shutdown: Arc<AtomicBool>,
    /// The quiet gap that ends a burst; see [`DEFAULT_BATCH_WINDOW`].
    pub batch_window: Duration,
}

/// Start watching the configured index paths with the configured excludes.
pub fn start_watcher(indexer: &IndexerConfig) -> Result<FileWatcher> {
    FileWatcher::new(WatcherConfig {
        paths: indexer.paths.iter().map(PathBuf::from).collect(),
        exclude_patterns: indexer.exclude_patterns.clone(),
    })
}

/// Run the watch loop on the calling thread until `shutdown` is set.
///
/// Marks the engine as watching while it runs and saves pending updates on
/// the way out.
pub fn run(mut watcher: FileWatcher, cfg: WatchLoopConfig) {
    info!("File watcher started");
    let queued = watcher.queued_counter();
    let applying = with_engine_write(&cfg.engine, |engine| {
        engine.watching = true;
        engine.watch_backlog.queued = queued;
        Arc::clone(&engine.watch_backlog.applying)
    })
    .unwrap_or_default();
    let mut updates_since_save: usize = 0;
    loop {
        if cfg.shutdown.load(Ordering::Acquire) {
            info!("File watcher stopping for shutdown");
            with_engine_write(&cfg.engine, |engine| engine.watching = false);
            save_after_watcher_shutdown(&cfg.indexer, &cfg.engine, updates_since_save);
            return;
        }
        let mut changes = match watcher.recv_timeout(Duration::from_secs(1)) {
            Some(first) => gather_burst(&watcher, first, cfg.batch_window, &applying),
            None => Vec::new(),
        };
        // Events were lost: reconcile the watched roots with the disk.
        let rescan = watcher.take_rescan();
        applying.fetch_add(rescan.len(), Ordering::AcqRel);
        changes.extend(rescan.into_iter().map(FileChange::Modified));
        if changes.is_empty() {
            continue;
        }
        debug!(count = changes.len(), "Applying file changes to index");

        // New directories need their own watches on platforms that watch
        // per directory.
        for change in &changes {
            if let FileChange::Modified(p) | FileChange::Renamed { to: p, .. } = change {
                if p.is_dir() {
                    watcher.ensure_watched(p);
                }
            }
        }
        let report = apply_burst(&cfg.engine, &changes, &cfg.indexer);
        // Already cleared, unless planning panicked before it took over the
        // count of the gathered events.
        applying.store(0, Ordering::Release);
        if let Some(report) = report.filter(|r| r.outcome.changed()) {
            let outcome = report.outcome;
            debug!(
                events = changes.len(),
                steps = report.steps,
                longest_hold_ms = report.longest_hold.as_millis() as u64,
                elapsed_ms = report.elapsed.as_millis() as u64,
                indexed = outcome.indexed,
                removed = outcome.removed,
                "File changes applied"
            );
            updates_since_save += outcome.indexed + outcome.removed;
            if save_on_watcher_update(&cfg.indexer, &cfg.engine, updates_since_save) {
                updates_since_save = 0;
            }
        }
    }
}

/// What applying a burst did and how long it kept searches out.
#[derive(Debug, Clone, Copy, Default)]
pub struct BurstReport {
    /// Files indexed and removed.
    pub outcome: ChangeOutcome,
    /// Write-lock steps taken.
    pub steps: usize,
    /// The longest single write-lock hold: the longest a search could
    /// have waited on this burst.
    pub longest_hold: Duration,
    /// From start to finish, planning and parsing included.
    pub elapsed: Duration,
}

/// Apply a burst the way the server does: work out what it means under a
/// read lock (stats, directory listings and walks), read and parse the
/// files under no lock, and take the write lock only to install the
/// result, a step at a time. Searches are answered throughout, except
/// during the short steps. The index may change between the steps (the
/// initial build runs alongside); the install looks every path up again,
/// and anything that changed on disk meanwhile has an event of its own
/// coming.
///
/// Until a change is installed it counts in the engine's
/// `watch_backlog.applying`, which searches report: first as the events,
/// then as the files they came to. The count is cleared when the burst ends.
///
/// `None` if a step panicked; the rest of the burst is then skipped.
pub fn apply_burst(
    engine: &Arc<RwLock<SearchEngine>>,
    changes: &[FileChange],
    indexer: &IndexerConfig,
) -> Option<BurstReport> {
    let started = Instant::now();
    let pool = watch_pool();
    let (plan, parser, applying) = guarded("planning", || {
        let engine = engine
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let applying = Arc::clone(&engine.watch_backlog.applying);
        pool.install(|| {
            (
                plan_changes(&engine, changes, indexer),
                engine.file_parser(),
                applying,
            )
        })
    })?;
    // Cleared however the burst ends.
    struct Cleared(Arc<AtomicUsize>);
    impl Drop for Cleared {
        fn drop(&mut self) {
            self.0.store(0, Ordering::Release);
        }
    }
    let applying = Cleared(applying);
    applying
        .0
        .store(plan.remove.len() + plan.index.len(), Ordering::Release);
    let parsed = guarded("parsing", || pool.install(|| parse_planned(parser, &plan)))?;

    // The removals go in one step and the files in steps of at most
    // INSTALL_CHUNK, each under its own write lock, so a big burst never
    // holds searches back for longer than one step.
    let mut report = BurstReport::default();
    let removals = ChangePlan {
        remove: plan.remove,
        index: Vec::new(),
    };
    let removing = removals.remove.len();
    install(engine, removals, Vec::new(), &mut report)?;
    applied(&applying.0, removing);
    let mut parsed = parsed;
    while !parsed.is_empty() {
        // Searches that queued during the last step go first.
        std::thread::sleep(STEP_GAP);
        let rest = parsed.split_off(parsed.len().min(INSTALL_CHUNK));
        let step = parsed.len();
        install(engine, ChangePlan::default(), parsed, &mut report)?;
        applied(&applying.0, step);
        parsed = rest;
    }
    report.elapsed = started.elapsed();
    Some(report)
}

/// Take `n` installed changes off the `applying` count.
fn applied(applying: &AtomicUsize, n: usize) {
    let _ = applying.fetch_update(Ordering::AcqRel, Ordering::Acquire, |c| {
        Some(c.saturating_sub(n))
    });
}

/// Files installed per write-lock step; see [`apply_burst`].
pub const INSTALL_CHUNK: usize = 512;

/// The pause between install steps. Longer than a waiting search's
/// longest back-off (see [`crate::search::engine_lock`]), so the searches
/// that arrived during a step are answered before the next one starts.
const STEP_GAP: Duration = Duration::from_millis(20);

/// One install step of [`apply_burst`], added to `report`.
fn install(
    engine: &Arc<RwLock<SearchEngine>>,
    plan: ChangePlan,
    parsed: Vec<(PathBuf, Option<crate::search::PreIndexedFile>)>,
    report: &mut BurstReport,
) -> Option<()> {
    if plan.remove.is_empty() && parsed.is_empty() {
        return Some(());
    }
    let (outcome, held) = with_engine_write(engine, |engine| {
        let held = Instant::now();
        let outcome = apply_plan(engine, plan, parsed);
        (outcome, held.elapsed())
    })?;
    report.outcome += outcome;
    report.steps += 1;
    report.longest_hold = report.longest_hold.max(held);
    Some(())
}

/// The threads the watcher plans and parses on. Searches scan files on
/// rayon's global pool; parsing a burst there queued every search behind
/// the whole burst. Half the cores, so searches keep the rest.
fn watch_pool() -> &'static rayon::ThreadPool {
    static POOL: std::sync::OnceLock<rayon::ThreadPool> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        let threads = std::thread::available_parallelism()
            .map(|n| n.get() / 2)
            .unwrap_or(1)
            .max(1);
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .thread_name(|i| format!("fcs-watch-{i}"))
            .build()
            .expect("watcher thread pool")
    })
}

/// Run `f`, turning a panic into a skipped batch rather than a dead watcher.
fn guarded<R>(step: &str, f: impl FnOnce() -> R) -> Option<R> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(result) => Some(result),
        Err(_) => {
            error!("File watcher {step} panicked; the change was skipped");
            None
        }
    }
}

/// `first` plus every event that follows it with gaps shorter than `quiet`,
/// up to [`MAX_BURST`], each counted in `applying` as it is taken off the
/// watcher's queue, so searches keep reporting it until it is applied.
fn gather_burst(
    watcher: &FileWatcher,
    first: FileChange,
    quiet: Duration,
    applying: &AtomicUsize,
) -> Vec<FileChange> {
    applying.fetch_add(1, Ordering::AcqRel);
    let mut changes = vec![first];
    let deadline = Instant::now() + MAX_BURST;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return changes;
        }
        match watcher.recv_timeout(quiet.min(remaining)) {
            Some(c) => {
                applying.fetch_add(1, Ordering::AcqRel);
                changes.push(c);
            }
            None => return changes,
        }
    }
}

/// Run `f` under the engine write lock, recovering a poisoned lock and
/// turning a panic into a skipped change rather than a dead watcher.
fn with_engine_write<R>(
    engine: &Arc<RwLock<SearchEngine>>,
    f: impl FnOnce(&mut SearchEngine) -> R,
) -> Option<R> {
    let mut guard = engine_lock::write(engine);
    guarded("update", || f(&mut guard))
}
