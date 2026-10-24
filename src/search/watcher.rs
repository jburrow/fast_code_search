//! File watcher for incremental indexing
//!
//! Uses the notify crate to watch for file changes and trigger re-indexing.
//!
//! Events are treated as *hints*: each one only names paths that may have
//! changed, and the consumer ([`crate::search::apply_changes`]) looks at the
//! disk to decide what to do with each path (index it, drop it, or reconcile
//! a directory). Backends coalesce, reorder and lose events (a debouncer
//! used to turn "rename a file into a directory, then delete the directory"
//! into a lone delete of the directory, leaving the file's old name indexed
//! forever), so nothing here relies on event kinds or rename pairing. When
//! events are known to be lost (the OS queue overflowed, or our own queue
//! was full), [`FileWatcher::take_rescan`] says so and the consumer
//! reconciles the watched roots instead.

use crate::search::path_filter::PathFilter;
use anyhow::Result;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// File change event types for incremental indexing
#[derive(Debug, Clone)]
pub enum FileChange {
    /// A path that may have been created or modified (a file or a directory)
    Modified(PathBuf),
    /// A path that may have been deleted
    Deleted(PathBuf),
    /// A file was renamed from old path to new path
    Renamed { from: PathBuf, to: PathBuf },
}

impl FileChange {
    /// The paths this change names: one, or two for a rename.
    pub fn paths(&self) -> Vec<&PathBuf> {
        match self {
            Self::Modified(p) | Self::Deleted(p) => vec![p],
            Self::Renamed { from, to } => vec![from, to],
        }
    }
}

/// Configuration for the file watcher
#[derive(Debug, Clone)]
pub struct WatcherConfig {
    /// Paths to watch
    pub paths: Vec<PathBuf>,
    /// Glob patterns to exclude
    pub exclude_patterns: Vec<String>,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            exclude_patterns: vec![
                "**/node_modules/**".to_string(),
                "**/target/**".to_string(),
                "**/.git/**".to_string(),
            ],
        }
    }
}

/// File watcher handle
pub struct FileWatcher {
    /// Channel receiver for file change events
    pub rx: Receiver<FileChange>,
    /// Keep the watcher alive
    watcher: RecommendedWatcher,
    /// Exclude patterns, for deciding which directories get a watch.
    exclude_filter: Arc<PathFilter>,
    /// One non-recursive watch per kept directory (Linux/inotify, where a
    /// recursive watch would install a handle in every excluded directory
    /// too — node_modules, target, .git/objects — and exhaust
    /// `max_user_watches`). Other platforms watch each root recursively.
    per_directory: bool,
    /// The watched roots, for a rescan.
    roots: Vec<PathBuf>,
    /// Set when events were lost and the roots need reconciling.
    rescan: Arc<AtomicBool>,
    /// Files with an event queued and not received yet; see
    /// [`Self::queued_counter`].
    queued: Arc<AtomicUsize>,
    /// How many queued events name each of those files.
    in_queue: Arc<Mutex<QueuedPaths>>,
}

/// The paths named by queued events, with how many events name each.
/// Editors and build tools raise several events per file (a write, a
/// truncate, a rename into place), so the queue can hold far more events
/// than files; searches report the files.
type QueuedPaths = HashMap<PathBuf, u32>;

fn lock_queue(in_queue: &Mutex<QueuedPaths>) -> std::sync::MutexGuard<'_, QueuedPaths> {
    in_queue.lock().unwrap_or_else(|p| p.into_inner())
}

/// Bound on queued change events. The consumer applies batches under the
/// engine write lock, which can be held for a long time during the initial
/// load; an unbounded queue would grow without limit meanwhile. When the
/// queue is full the event is dropped and a rescan is requested instead.
const EVENT_QUEUE_CAPACITY: usize = 65_536;

/// Is `dir` excluded as a directory? Patterns are written for files
/// (`**/target/**`), so probe the directory both as itself and via a child.
fn dir_is_excluded(filter: &PathFilter, dir: &Path) -> bool {
    let normalized = dir.to_string_lossy().replace('\\', "/");
    filter.is_excluded(&normalized)
        || filter.is_excluded(&format!("{}/_", normalized.trim_end_matches('/')))
}

/// Add a non-recursive watch to every non-excluded directory under `root`
/// (`root` itself included). Returns `(watched, failed)`.
fn watch_tree(
    watcher: &mut RecommendedWatcher,
    root: &Path,
    filter: &PathFilter,
) -> (usize, usize) {
    let mut watched = 0usize;
    let mut failed = 0usize;
    let walker = walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            e.depth() == 0 || !e.file_type().is_dir() || !dir_is_excluded(filter, e.path())
        });
    for entry in walker.flatten() {
        if !entry.file_type().is_dir() {
            continue;
        }
        match watcher.watch(entry.path(), RecursiveMode::NonRecursive) {
            Ok(()) => watched += 1,
            Err(e) => {
                if failed == 0 {
                    warn!(path = %entry.path().display(), error = %e, "Failed to watch directory");
                }
                failed += 1;
            }
        }
    }
    (watched, failed)
}

impl FileWatcher {
    /// Create and start a new file watcher
    pub fn new(config: WatcherConfig) -> Result<Self> {
        let (tx, rx) = mpsc::sync_channel::<FileChange>(EVENT_QUEUE_CAPACITY);
        let per_directory = cfg!(target_os = "linux");
        let rescan = Arc::new(AtomicBool::new(false));

        // Compile exclude patterns into a real glob filter (same semantics as file
        // discovery). The previous approach trimmed `**/.git/**` down to `.git` and
        // substring-matched, which wrongly excluded `.github/` and `.gitignore` and
        // failed entirely on Windows backslash paths.
        let exclude_filter = Arc::new(
            PathFilter::exclude_only(&config.exclude_patterns).unwrap_or_else(|e| {
                warn!(
                    "Invalid watcher exclude pattern(s): {}; exclusions disabled",
                    e
                );
                PathFilter::default()
            }),
        );

        let handler_exclude = Arc::clone(&exclude_filter);
        let handler_rescan = Arc::clone(&rescan);
        let queued = Arc::new(AtomicUsize::new(0));
        let handler_queued = Arc::clone(&queued);
        let in_queue = Arc::new(Mutex::new(QueuedPaths::new()));
        let handler_in_queue = Arc::clone(&in_queue);
        let mut watcher = notify::recommended_watcher(move |result: notify::Result<Event>| {
            match result {
                Ok(event) => {
                    if cfg!(feature = "failpoints")
                        && event
                            .paths
                            .iter()
                            .any(|p| crate::faults::fires("watch.overflow", p))
                    {
                        // As if the OS queue overflowed: this event is lost.
                        warn!("File watcher lost events; the watched paths will be rescanned");
                        handler_rescan.store(true, Ordering::Release);
                        return;
                    }
                    if event.need_rescan() {
                        warn!("File watcher lost events; the watched paths will be rescanned");
                        handler_rescan.store(true, Ordering::Release);
                    }
                    for change in hints_for(&event, &handler_exclude) {
                        let paths: Vec<PathBuf> = change.paths().into_iter().cloned().collect();
                        // Sent and counted under the lock, so the receiver
                        // never takes it off the count before it is on it.
                        let mut in_queue = lock_queue(&handler_in_queue);
                        let sent = tx.try_send(change);
                        if sent.is_ok() {
                            for path in paths {
                                let events = in_queue.entry(path).or_insert(0);
                                if *events == 0 {
                                    handler_queued.fetch_add(1, Ordering::AcqRel);
                                }
                                *events += 1;
                            }
                        }
                        drop(in_queue);
                        match sent {
                            Ok(()) => {}
                            Err(mpsc::TrySendError::Full(_)) => {
                                if !handler_rescan.swap(true, Ordering::AcqRel) {
                                    warn!(
                                        capacity = EVENT_QUEUE_CAPACITY,
                                        "File change queue full; the watched paths will be rescanned"
                                    );
                                }
                            }
                            Err(mpsc::TrySendError::Disconnected(_)) => {
                                debug!("File watcher channel closed");
                                return;
                            }
                        }
                    }
                }
                Err(error) => {
                    error!("File watcher error: {:?}", error);
                }
            }
        })?;

        // Watch all configured paths; per-path failures are non-fatal so that a single
        // over-limit directory does not prevent watching the remaining paths.
        let mut watched = 0usize;
        let mut watch_errors = 0usize;
        let mut roots = Vec::new();
        for path in &config.paths {
            if path.exists() {
                if per_directory {
                    let (ok, failed) = watch_tree(&mut watcher, path, &exclude_filter);
                    if ok > 0 {
                        info!(
                            path = %path.display(),
                            directories = ok,
                            failed,
                            "Watching directory tree for changes"
                        );
                        watched += 1;
                        roots.push(path.clone());
                    } else {
                        warn!(path = %path.display(), failed, "Failed to watch path (skipping)");
                        watch_errors += 1;
                    }
                    continue;
                }
                match watcher.watch(path, RecursiveMode::Recursive) {
                    Ok(()) => {
                        info!(path = %path.display(), "Watching directory for changes");
                        watched += 1;
                        roots.push(path.clone());
                    }
                    Err(e) => {
                        warn!(
                            path = %path.display(),
                            error = %e,
                            "Failed to watch path (skipping)"
                        );
                        watch_errors += 1;
                    }
                }
            } else {
                warn!(path = %path.display(), "Watch path does not exist, skipping");
            }
        }

        if watched == 0 && watch_errors > 0 {
            anyhow::bail!(
                "OS file watch limit reached for all {} configured path(s). \
                On Linux, increase the limit with: \
                sudo sysctl -w fs.inotify.max_user_watches=524288 \
                (add to /etc/sysctl.conf to persist across reboots). \
                Alternatively, set `watch = false` in your config to disable file watching.",
                watch_errors
            );
        }

        if watch_errors > 0 {
            warn!(
                watched = watched,
                failed = watch_errors,
                "Some watch paths failed due to OS watch limit; \
                incremental indexing may miss changes in those directories. \
                On Linux: sudo sysctl -w fs.inotify.max_user_watches=524288"
            );
        }

        Ok(Self {
            rx,
            watcher,
            exclude_filter,
            per_directory,
            roots,
            rescan,
            queued,
            in_queue,
        })
    }

    /// Make sure `dir` (a directory that was just created or moved into a
    /// watched tree) and everything under it is watched. A no-op where roots
    /// are watched recursively, or when `dir` is excluded.
    pub fn ensure_watched(&mut self, dir: &Path) {
        if !self.per_directory || !dir.is_dir() || dir_is_excluded(&self.exclude_filter, dir) {
            return;
        }
        let (watched, failed) = watch_tree(&mut self.watcher, dir, &self.exclude_filter);
        debug!(path = %dir.display(), watched, failed, "Added watches for new directory");
    }

    /// If events were lost since the last call, the watched roots (to be
    /// reconciled with the disk); otherwise empty.
    pub fn take_rescan(&self) -> Vec<PathBuf> {
        if self.rescan.swap(false, Ordering::AcqRel) {
            self.roots.clone()
        } else {
            Vec::new()
        }
    }

    /// The count of files with an event queued and not received yet (not
    /// of events: one save can raise several), kept current by the watcher
    /// thread and the receive methods, so a search can report changes
    /// nobody has applied yet.
    pub fn queued_counter(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.queued)
    }

    /// One event taken off the queue.
    fn received(&self, change: Option<FileChange>) -> Option<FileChange> {
        if let Some(change) = &change {
            let mut in_queue = lock_queue(&self.in_queue);
            for path in change.paths() {
                if let Some(events) = in_queue.get_mut(path) {
                    *events -= 1;
                    if *events == 0 {
                        in_queue.remove(path);
                        self.queued.fetch_sub(1, Ordering::AcqRel);
                    }
                }
            }
        }
        change
    }

    /// Try to receive a file change event without blocking
    pub fn try_recv(&self) -> Option<FileChange> {
        self.received(self.rx.try_recv().ok())
    }

    /// Receive a file change event, blocking until one is available
    pub fn recv(&self) -> Option<FileChange> {
        self.received(self.rx.recv().ok())
    }

    /// Receive a file change event with a timeout
    pub fn recv_timeout(&self, timeout: Duration) -> Option<FileChange> {
        self.received(self.rx.recv_timeout(timeout).ok())
    }
}

/// The paths a notify event says may have changed, as hints.
///
/// Every non-excluded file path is reported: `Deleted` if it is gone now,
/// `Modified` otherwise (the consumer re-checks the disk either way, so a
/// stale guess is harmless). A directory is reported only when it appeared,
/// vanished or was renamed, since those change what is under it; metadata
/// and access events on a directory say nothing about its files.
fn hints_for(event: &Event, exclude_filter: &PathFilter) -> Vec<FileChange> {
    use notify::event::ModifyKind;
    if matches!(event.kind, EventKind::Access(_)) {
        return Vec::new();
    }
    let changes_tree = matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(_))
    );
    event
        .paths
        .iter()
        .filter(|p| !should_exclude(p, exclude_filter))
        .filter_map(|p| {
            if p.is_dir() {
                changes_tree.then(|| FileChange::Modified(p.clone()))
            } else if p.exists() {
                Some(FileChange::Modified(p.clone()))
            } else {
                Some(FileChange::Deleted(p.clone()))
            }
        })
        .collect()
}

/// Check if a path should be excluded using the compiled glob filter.
pub fn should_exclude(path: &Path, exclude_filter: &PathFilter) -> bool {
    exclude_filter.is_excluded(&path.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_exclude() {
        let filter = PathFilter::exclude_only(&[
            "**/node_modules/**".to_string(),
            "**/.git/**".to_string(),
            "**/target/**".to_string(),
        ])
        .unwrap();

        // Files inside excluded directories are excluded.
        assert!(should_exclude(
            Path::new("/project/node_modules/package/index.js"),
            &filter
        ));
        assert!(should_exclude(
            Path::new("/project/.git/objects/abc"),
            &filter
        ));

        // Regression: `.git` must NOT swallow `.github/` or `.gitignore`.
        assert!(!should_exclude(
            Path::new("/project/.github/workflows/ci.yml"),
            &filter
        ));
        assert!(!should_exclude(Path::new("/project/.gitignore"), &filter));
        // Regression: `target` must NOT swallow `targeted.rs`.
        assert!(!should_exclude(
            Path::new("/project/src/targeted.rs"),
            &filter
        ));
        assert!(!should_exclude(Path::new("/project/src/main.rs"), &filter));
    }

    #[test]
    fn test_should_exclude_windows_paths() {
        let filter = PathFilter::exclude_only(&["**/target/**".to_string()]).unwrap();
        // Backslash paths must match too (previous substring approach failed here).
        assert!(should_exclude(
            Path::new(r"C:\proj\target\debug\app.exe"),
            &filter
        ));
        assert!(!should_exclude(Path::new(r"C:\proj\src\main.rs"), &filter));
    }

    #[test]
    fn test_bare_name_exclude_matches_contents() {
        // A bare directory name (no glob meta) must exclude its contents too.
        let filter = PathFilter::exclude_only(&["node_modules".to_string()]).unwrap();
        assert!(filter.is_excluded("/p/node_modules/pkg/index.js"));
        assert!(!filter.is_excluded("/p/src/main.rs"));
    }

    #[test]
    fn test_watcher_config_default() {
        let config = WatcherConfig::default();
        assert!(config.paths.is_empty());
        assert!(!config.exclude_patterns.is_empty());
    }

    /// One file saved many times is one change still to apply, not one per
    /// event (the scoreboard saw 372,534 pending for 250,000 changed files).
    #[test]
    fn queued_count_is_files_not_events() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let watcher = FileWatcher::new(WatcherConfig {
            paths: vec![root.clone()],
            exclude_patterns: Vec::new(),
        })
        .unwrap();
        let queued = watcher.queued_counter();
        let file = root.join("a.txt");
        let other = root.join("b.txt");
        for i in 0..20 {
            std::fs::write(&file, format!("version {i}")).unwrap();
        }
        std::fs::write(&other, "x").unwrap();

        // Let the events arrive, without taking any off the queue, until
        // the count has settled.
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut last = usize::MAX;
        while std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(300));
            let now = queued.load(Ordering::Acquire);
            if now == last && now >= 2 {
                break;
            }
            last = now;
        }
        let pending = queued.load(Ordering::Acquire);
        let mut events = 0;
        let mut paths = std::collections::HashSet::new();
        while let Some(change) = watcher.recv_timeout(Duration::from_millis(500)) {
            events += 1;
            paths.extend(change.paths().into_iter().cloned());
        }
        // Some backends also name the folder (FSEvents on macOS), so the
        // paths are counted rather than assumed to be the two files.
        assert!(paths.contains(&file) && paths.contains(&other), "{paths:?}");
        assert_eq!(pending, paths.len(), "one per path: {paths:?}");
        // inotify raises an event per write; FSEvents may coalesce them.
        if cfg!(target_os = "linux") {
            assert!(
                events > paths.len(),
                "several events per file, got {events}"
            );
        }
        assert_eq!(queued.load(Ordering::Acquire), 0, "all received");
    }
}
