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
use crate::search::engine::ReadFailures;
use crate::search::engine_lock;
use crate::search::incremental::{
    apply_plan, parse_planned_checked, plan_changes, ChangeOutcome, ChangePlan,
};
use crate::search::watcher::{FileChange, FileWatcher, WatcherConfig};
use crate::search::SearchEngine;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

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
    let (applying, read_failures) = with_engine_write(&cfg.engine, |engine| {
        engine.watching = true;
        engine.watch_backlog.queued = queued;
        (
            Arc::clone(&engine.watch_backlog.applying),
            Arc::clone(&engine.watch_backlog.read_failures),
        )
    })
    .unwrap_or_default();
    let mut updates_since_save: usize = 0;
    // A burst whose apply panicked, to try once more (see `PANIC_RETRY`).
    let mut after_panic: Option<(Vec<FileChange>, Instant)> = None;
    loop {
        if cfg.shutdown.load(Ordering::Acquire) {
            info!("File watcher stopping for shutdown");
            with_engine_write(&cfg.engine, |engine| engine.watching = false);
            save_after_watcher_shutdown(&cfg.indexer, &cfg.engine, updates_since_save);
            return;
        }
        // Wake for the next read retry or panic retry, if one is due sooner.
        let wait = [
            read_failures.next_due(),
            after_panic.as_ref().map(|(_, at)| *at),
        ]
        .into_iter()
        .flatten()
        .map(|at| at.saturating_duration_since(Instant::now()))
        .fold(Duration::from_secs(1), Duration::min);
        let mut changes = match watcher.recv_timeout(wait) {
            Some(first) => gather_burst(&watcher, first, cfg.batch_window, &applying),
            None => Vec::new(),
        };
        // Events were lost: reconcile the watched roots with the disk.
        let rescan = watcher.take_rescan();
        applying.fetch_add(rescan.len(), Ordering::AcqRel);
        changes.extend(rescan.into_iter().map(FileChange::Modified));
        // Files the scrub found out of step with the disk.
        let requeued: Vec<PathBuf> = read_engine(&cfg.engine, |e| {
            std::mem::take(
                &mut *e
                    .watch_backlog
                    .requeue
                    .lock()
                    .unwrap_or_else(|p| p.into_inner()),
            )
        })
        .unwrap_or_default();
        applying.fetch_add(requeued.len(), Ordering::AcqRel);
        changes.extend(requeued.into_iter().map(FileChange::Modified));
        // Files that could not be read last time (already counted as
        // pending while they wait).
        let now = Instant::now();
        changes.extend(
            read_failures
                .take_due(now)
                .into_iter()
                .map(FileChange::Modified),
        );
        let retrying = match after_panic.take() {
            Some((burst, at)) if at <= now => burst,
            other => {
                after_panic = other;
                Vec::new()
            }
        };
        if cfg!(feature = "failpoints") {
            let before = changes.len();
            changes.retain(|c| {
                !change_paths(c)
                    .iter()
                    .any(|p| crate::faults::fires("watch.lose", p))
            });
            applied(&applying, before - changes.len());
        }
        if changes.is_empty() && retrying.is_empty() {
            continue;
        }
        if !retrying.is_empty() {
            // Apply the panicked burst on its own, a change at a time: a
            // change that panics again is the culprit and is left out.
            retry_after_panic(&cfg, &read_failures, &retrying);
            updates_since_save += retrying.len();
        }
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
        if report.is_none() {
            // Not dropped: tried again shortly (a panic can be a race with
            // the disk changing under the burst), and counted until then.
            error!(
                events = changes.len(),
                "File watcher failed applying a burst of changes; trying it again in {} s",
                PANIC_RETRY.as_secs()
            );
            applying.fetch_add(changes.len(), Ordering::AcqRel);
            after_panic = Some((changes.clone(), Instant::now() + PANIC_RETRY));
        }
        if let Some(r) = &report {
            crate::diagnostics::capture::record_burst(
                changes.len(),
                r.outcome.indexed,
                r.outcome.removed,
                r.steps,
                r.longest_hold,
                r.elapsed,
            );
        }
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

/// How long after a burst panicked it is tried again.
const PANIC_RETRY: Duration = Duration::from_secs(1);

/// Apply a burst that panicked, one change at a time. A change that panics
/// again is given up on: its files are removed from the index and reported
/// as unreadable (see [`ReadFailures`]) until they change again. A panic
/// used to drop the whole burst without a word, leaving its files out of
/// date until each changed again.
fn retry_after_panic(cfg: &WatchLoopConfig, read_failures: &ReadFailures, burst: &[FileChange]) {
    let applying = read_engine(&cfg.engine, |e| Arc::clone(&e.watch_backlog.applying));
    for (i, change) in burst.iter().enumerate() {
        if apply_burst(&cfg.engine, std::slice::from_ref(change), &cfg.indexer).is_none() {
            let paths = change_paths(change);
            error!(
                paths = ?paths,
                "File watcher failed applying this change twice; its files are left out of \
                 the index until they change again"
            );
            for path in &paths {
                read_failures.give_up(path);
            }
            forget_paths(&cfg.engine, &paths);
        }
        // apply_burst clears the count; the rest of the burst is still to do.
        if let Some(applying) = &applying {
            applying.store(burst.len() - i - 1, Ordering::Release);
        }
    }
}

/// The paths a change names.
fn change_paths(change: &FileChange) -> Vec<PathBuf> {
    match change {
        FileChange::Modified(p) | FileChange::Deleted(p) => vec![p.clone()],
        FileChange::Renamed { from, to } => vec![from.clone(), to.clone()],
    }
}

/// Remove `paths` (files, or every file under a directory) from the index.
fn forget_paths(engine: &Arc<RwLock<SearchEngine>>, paths: &[PathBuf]) {
    with_engine_write(engine, |engine| {
        for path in paths {
            if !engine.remove_file(path) {
                engine.remove_files_under(path);
            }
        }
    });
}

/// Run `f` under the engine read lock, recovering a poisoned lock.
fn read_engine<R>(
    engine: &Arc<RwLock<SearchEngine>>,
    f: impl FnOnce(&SearchEngine) -> R,
) -> Option<R> {
    let guard = engine
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guarded("read", || f(&guard))
}

/// What applying a burst did and how long it kept searches out.
#[derive(Debug, Clone, Default)]
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
    /// Files that could not be read; they are tried again later (see
    /// [`ReadFailures`]) and keep their previous index entry until then.
    pub unreadable: usize,
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
/// A file that cannot be read keeps its index entry and is recorded in the
/// engine's [`ReadFailures`] to be tried again; one given up on is removed.
///
/// `None` if a step panicked; the rest of the burst is then skipped (the
/// watch loop tries it again).
pub fn apply_burst(
    engine: &Arc<RwLock<SearchEngine>>,
    changes: &[FileChange],
    indexer: &IndexerConfig,
) -> Option<BurstReport> {
    let started = Instant::now();
    let pool = watch_pool();
    let (plan, parser, applying, read_failures) = guarded("planning", || {
        let engine = engine
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let applying = Arc::clone(&engine.watch_backlog.applying);
        let read_failures = Arc::clone(&engine.watch_backlog.read_failures);
        pool.install(|| {
            (
                plan_changes(&engine, changes, indexer),
                engine.file_parser(),
                applying,
                read_failures,
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
    let (mut parsed, unreadable) = guarded("parsing", || {
        pool.install(|| parse_planned_checked(parser, &plan))
    })?;

    // Whatever was read, or is gone, no longer needs a retry; what could
    // not be read is retried later and keeps its entry meanwhile, unless
    // it has failed too often.
    let mut report = BurstReport {
        unreadable: unreadable.len(),
        ..BurstReport::default()
    };
    if !read_failures.is_empty() {
        let failed: std::collections::HashSet<&PathBuf> =
            unreadable.iter().map(|(p, _)| p).collect();
        for change in changes {
            for path in change_paths(change) {
                if !failed.contains(&path) {
                    read_failures.cleared(&path);
                }
            }
        }
        for (path, _) in &parsed {
            read_failures.cleared(path);
        }
    }
    let now = Instant::now();
    for (path, e) in unreadable {
        let attempts_left = !read_failures.failed(&path, now);
        if attempts_left {
            debug!(path = %path.display(), error = %e, "Could not read changed file; will retry");
        } else {
            warn!(
                path = %path.display(),
                error = %e,
                "Could not read changed file after repeated tries; leaving it out of the index \
                 until it changes again"
            );
            parsed.push((path, None));
        }
    }

    // The removals go in one step and the files in steps, each under its
    // own write lock, so a big burst never holds searches back for longer
    // than one step.
    let removals = ChangePlan {
        remove: plan.remove,
        index: Vec::new(),
    };
    let removing = removals.remove.len();
    install(
        engine,
        removals,
        &mut Vec::new(),
        Duration::ZERO,
        &mut report,
        &applying.0,
    )?;
    applied(&applying.0, removing);
    while !parsed.is_empty() {
        // Searches that queued during the last step go first.
        crate::sim::sleep(STEP_GAP);
        install(
            engine,
            ChangePlan::default(),
            &mut parsed,
            MAX_HOLD,
            &mut report,
            &applying.0,
        )?;
    }
    report.elapsed = started.elapsed();
    Some(report)
}

/// Take `n` installed changes off the `applying` count.
fn applied(applying: &AtomicUsize, n: usize) {
    crate::diagnostics::capture::watcher_progress(n.max(1));
    let _ = applying.fetch_update(Ordering::AcqRel, Ordering::Acquire, |c| {
        Some(c.saturating_sub(n))
    });
}

/// Files installed at a time; a step installs one or more of these.
pub const INSTALL_CHUNK: usize = 512;

/// The longest a step holds the write lock past its first
/// [`INSTALL_CHUNK`] files.
///
/// A step waits for a moment when no search holds the lock, which while
/// people search is about as long as one search. Installing a fixed 512
/// files per step then made the watcher apply a few hundred files a second
/// on a tree whose searches take half a second: measured with
/// `examples/watch_load.rs --mixed` on 200,000 files, 100,000 rewritten
/// files were still 17,000 behind four minutes later, and the server looked
/// as if it was updating for ever. A step now stays in about as long as it
/// waited to get in, up to this cap, so a backlog drains at half speed or
/// better while a search waits at most this long for a step. That holds
/// only while the wait stays under the cap, which is why new searches hold
/// back about as long as a search runs (`engine_lock::DEFER`): with a
/// shorter hold-back, new searches keep the writer out past the running
/// ones and the drain fell to 8% speed in `tests/lock_sim.rs`.
pub const MAX_HOLD: Duration = Duration::from_secs(1);

/// The pause between install steps. Longer than a waiting search's
/// longest back-off (see [`crate::search::engine_lock`]), so the searches
/// that arrived during a step are answered before the next one starts.
pub const STEP_GAP: Duration = Duration::from_millis(20);

/// One install step of [`apply_burst`], added to `report`: `plan` and the
/// first [`INSTALL_CHUNK`] of `parsed` (taken off it), then more chunks
/// for as long as the lock took to get, up to `max_hold`. Each installed
/// chunk is taken off `applying`.
fn install(
    engine: &Arc<RwLock<SearchEngine>>,
    plan: ChangePlan,
    parsed: &mut Vec<(PathBuf, Option<crate::search::PreIndexedFile>)>,
    max_hold: Duration,
    report: &mut BurstReport,
    applying: &AtomicUsize,
) -> Option<()> {
    if plan.remove.is_empty() && parsed.is_empty() {
        return Some(());
    }
    let mut plan = Some(plan);
    let held = install_step(engine, max_hold, |engine| {
        let rest = parsed.split_off(parsed.len().min(INSTALL_CHUNK));
        let chunk = std::mem::replace(parsed, rest);
        let files = chunk.len();
        let plan = plan.take().unwrap_or_default();
        report.outcome += guarded("update", || {
            for (path, _) in &chunk {
                crate::faults::panic_point("watch.apply", path);
            }
            apply_plan(engine, plan, chunk)
        })?;
        applied(applying, files);
        Some(!parsed.is_empty())
    })?;
    report.steps += 1;
    report.longest_hold = report.longest_hold.max(held);
    Some(())
}

/// The step policy of `install`, apart from what a chunk is: take the
/// write lock, install a first chunk (`chunk` returns whether more are
/// left, `None` to stop), then more for as long as the lock took to get,
/// up to `max_hold`. Returns how long the lock was held.
///
/// Public so the lock simulation (`tests/lock_sim.rs`) runs this policy,
/// not a copy of it, with chunks that take a simulated time.
pub fn install_step<T>(
    lock: &RwLock<T>,
    max_hold: Duration,
    mut chunk: impl FnMut(&mut T) -> Option<bool>,
) -> Option<Duration> {
    let asked = crate::sim::now();
    let mut guard = engine_lock::write(lock);
    let held = crate::sim::now();
    let budget = (held - asked).min(max_hold);
    loop {
        let more = chunk(&mut guard)?;
        if !more || crate::sim::now() - held >= budget {
            break;
        }
    }
    Some(crate::sim::now() - held)
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
