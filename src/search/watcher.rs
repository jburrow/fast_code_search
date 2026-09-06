//! File watcher for incremental indexing
//!
//! Uses the notify crate to watch for file changes and trigger re-indexing.

use crate::search::path_filter::PathFilter;
use anyhow::Result;
use notify_debouncer_full::{
    new_debouncer, notify::RecursiveMode, DebouncedEvent, Debouncer, RecommendedCache,
};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// File change event types for incremental indexing
#[derive(Debug, Clone)]
pub enum FileChange {
    /// A file was created or modified
    Modified(PathBuf),
    /// A file was deleted
    Deleted(PathBuf),
    /// A file was renamed from old path to new path
    Renamed { from: PathBuf, to: PathBuf },
}

/// Configuration for the file watcher
#[derive(Debug, Clone)]
pub struct WatcherConfig {
    /// Paths to watch
    pub paths: Vec<PathBuf>,
    /// Debounce duration for file change events
    pub debounce_duration: Duration,
    /// Glob patterns to exclude
    pub exclude_patterns: Vec<String>,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            debounce_duration: Duration::from_secs(2),
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
    watcher: Debouncer<notify_debouncer_full::notify::RecommendedWatcher, RecommendedCache>,
    /// Exclude patterns, for deciding which directories get a watch.
    exclude_filter: Arc<PathFilter>,
    /// One non-recursive watch per kept directory (Linux/inotify, where a
    /// recursive watch would install a handle in every excluded directory
    /// too — node_modules, target, .git/objects — and exhaust
    /// `max_user_watches`). Other platforms watch each root recursively.
    per_directory: bool,
}

/// Bound on queued change events. The consumer applies batches under the
/// engine write lock, which can be held for a long time during the initial
/// load; an unbounded queue would grow without limit meanwhile. When the
/// queue is full the event is dropped and the vanished-sibling check in
/// `apply_changes` heals what it can.
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
    debouncer: &mut Debouncer<notify_debouncer_full::notify::RecommendedWatcher, RecommendedCache>,
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
        match debouncer.watch(entry.path(), RecursiveMode::NonRecursive) {
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

        // Create the debouncer with event handler
        let handler_tx = tx;
        let handler_exclude = Arc::clone(&exclude_filter);
        let mut dropped_since_warn = 0usize;

        let mut debouncer = new_debouncer(
            config.debounce_duration,
            None,
            move |result: Result<
                Vec<DebouncedEvent>,
                Vec<notify_debouncer_full::notify::Error>,
            >| {
                match result {
                    Ok(events) => {
                        for event in events {
                            if let Some(change) = process_event(&event, &handler_exclude) {
                                match handler_tx.try_send(change) {
                                    Ok(()) => dropped_since_warn = 0,
                                    Err(mpsc::TrySendError::Full(_)) => {
                                        dropped_since_warn += 1;
                                        if dropped_since_warn == 1 {
                                            warn!(
                                                capacity = EVENT_QUEUE_CAPACITY,
                                                "File change queue full; dropping events until it drains"
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
                    }
                    Err(errors) => {
                        for error in errors {
                            error!("File watcher error: {:?}", error);
                        }
                    }
                }
            },
        )?;

        // Watch all configured paths; per-path failures are non-fatal so that a single
        // over-limit directory does not prevent watching the remaining paths.
        let mut watched = 0usize;
        let mut watch_errors = 0usize;
        for path in &config.paths {
            if path.exists() {
                if per_directory {
                    let (ok, failed) = watch_tree(&mut debouncer, path, &exclude_filter);
                    if ok > 0 {
                        info!(
                            path = %path.display(),
                            directories = ok,
                            failed,
                            "Watching directory tree for changes"
                        );
                        watched += 1;
                    } else {
                        warn!(path = %path.display(), failed, "Failed to watch path (skipping)");
                        watch_errors += 1;
                    }
                    continue;
                }
                match debouncer.watch(path, RecursiveMode::Recursive) {
                    Ok(()) => {
                        info!(path = %path.display(), "Watching directory for changes");
                        watched += 1;
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
            watcher: debouncer,
            exclude_filter,
            per_directory,
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

    /// Try to receive a file change event without blocking
    pub fn try_recv(&self) -> Option<FileChange> {
        self.rx.try_recv().ok()
    }

    /// Receive a file change event, blocking until one is available
    pub fn recv(&self) -> Option<FileChange> {
        self.rx.recv().ok()
    }

    /// Receive a file change event with a timeout
    pub fn recv_timeout(&self, timeout: Duration) -> Option<FileChange> {
        self.rx.recv_timeout(timeout).ok()
    }
}

/// Process a notify event and convert to FileChange
fn process_event(event: &DebouncedEvent, exclude_filter: &PathFilter) -> Option<FileChange> {
    use notify_debouncer_full::notify::event::ModifyKind;
    use notify_debouncer_full::notify::EventKind;

    let paths = &event.paths;

    // Skip if all paths match exclude patterns
    let should_process = paths
        .iter()
        .any(|path| !should_exclude(path, exclude_filter));

    if !should_process {
        return None;
    }

    match &event.kind {
        // Renames arrive as Modify(Name) — on Windows/Linux typically with two
        // paths [from, to]. The old code took paths.first() (the OLD path),
        // failed is_file(), and dropped the event entirely, leaving the old path
        // in the index forever and never indexing the new one.
        EventKind::Modify(ModifyKind::Name(_)) => {
            match (paths.first(), paths.get(1)) {
                (Some(from), Some(to)) => Some(FileChange::Renamed {
                    from: from.clone(),
                    to: to.clone(),
                }),
                (Some(p), None) => {
                    // Single-path name event: deleted if it no longer exists,
                    // otherwise treat as a modification of the (new) path.
                    if p.exists() {
                        Some(FileChange::Modified(p.clone()))
                    } else {
                        Some(FileChange::Deleted(p.clone()))
                    }
                }
                _ => None,
            }
        }
        EventKind::Create(_) | EventKind::Modify(_) => {
            // Use the first non-excluded path that is a regular file. A
            // newly created directory is reported too: its contents need
            // discovering, and on platforms that watch per directory it
            // needs a watch of its own.
            let is_create = matches!(event.kind, EventKind::Create(_));
            paths
                .iter()
                .find(|p| {
                    !should_exclude(p, exclude_filter) && (p.is_file() || (is_create && p.is_dir()))
                })
                .map(|p| FileChange::Modified(p.clone()))
        }
        EventKind::Remove(_) => paths
            .iter()
            .find(|p| !should_exclude(p, exclude_filter))
            .map(|p| FileChange::Deleted(p.clone())),
        EventKind::Any | EventKind::Access(_) | EventKind::Other => None,
    }
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
        assert_eq!(config.debounce_duration, Duration::from_secs(2));
        assert!(!config.exclude_patterns.is_empty());
    }
}
