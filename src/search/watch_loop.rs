//! The file-watch loop: receive change events, gather each burst, apply it to
//! the engine under one write lock, and checkpoint the index.
//!
//! This used to live inline in `main.rs`, where no test could reach it; the
//! tests re-implemented their own copy of the loop instead. The server and
//! the watch tests (`tests/watch_harness.rs`) now run this same code.

use crate::config::IndexerConfig;
use crate::search::background_indexer::{save_after_watcher_shutdown, save_on_watcher_update};
use crate::search::incremental::apply_changes;
use crate::search::watcher::{FileChange, FileWatcher, WatcherConfig};
use crate::search::SearchEngine;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
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
    with_engine_write(&cfg.engine, |engine| engine.watching = true);
    let mut updates_since_save: usize = 0;
    loop {
        if cfg.shutdown.load(Ordering::Acquire) {
            info!("File watcher stopping for shutdown");
            with_engine_write(&cfg.engine, |engine| engine.watching = false);
            save_after_watcher_shutdown(&cfg.indexer, &cfg.engine, updates_since_save);
            return;
        }
        let mut changes = match watcher.recv_timeout(Duration::from_secs(1)) {
            Some(first) => gather_burst(&watcher, first, cfg.batch_window),
            None => Vec::new(),
        };
        // Events were lost: reconcile the watched roots with the disk.
        changes.extend(watcher.take_rescan().into_iter().map(FileChange::Modified));
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
        let outcome = with_engine_write(&cfg.engine, |engine| {
            apply_changes(engine, &changes, &cfg.indexer)
        });
        if let Some(outcome) = outcome.filter(|o| o.changed()) {
            debug!(
                events = changes.len(),
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

/// `first` plus every event that follows it with gaps shorter than `quiet`,
/// up to [`MAX_BURST`].
fn gather_burst(watcher: &FileWatcher, first: FileChange, quiet: Duration) -> Vec<FileChange> {
    let mut changes = vec![first];
    let deadline = Instant::now() + MAX_BURST;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return changes;
        }
        match watcher.recv_timeout(quiet.min(remaining)) {
            Some(c) => changes.push(c),
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
    let mut guard = engine.write().unwrap_or_else(|poisoned| {
        error!("Search engine lock was poisoned; recovering in file watcher");
        poisoned.into_inner()
    });
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&mut guard))) {
        Ok(result) => Some(result),
        Err(_) => {
            error!("File watcher update panicked; the change was skipped");
            None
        }
    }
}
