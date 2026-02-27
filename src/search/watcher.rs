//! File watcher for incremental indexing
//!
//! Uses the notify crate to watch for file changes and trigger re-indexing.

use anyhow::Result;
use notify_debouncer_full::{
    new_debouncer, notify::RecursiveMode, DebouncedEvent, Debouncer, RecommendedCache,
};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
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
    _watcher: Debouncer<notify_debouncer_full::notify::RecommendedWatcher, RecommendedCache>,
}

impl FileWatcher {
    /// Create and start a new file watcher
    pub fn new(config: WatcherConfig) -> Result<Self> {
        let (tx, rx) = mpsc::channel::<FileChange>();

        // Pre-compile exclude patterns
        let exclude_patterns: Vec<String> = config
            .exclude_patterns
            .iter()
            .map(|p| p.trim_matches('*').trim_matches('/').to_string())
            .filter(|p| !p.is_empty())
            .collect();

        // Create the debouncer with event handler
        let handler_tx = tx.clone();
        let handler_exclude = exclude_patterns.clone();

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
                                if handler_tx.send(change).is_err() {
                                    debug!("File watcher channel closed");
                                    return;
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
            _watcher: debouncer,
        })
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
fn process_event(event: &DebouncedEvent, exclude_patterns: &[String]) -> Option<FileChange> {
    use notify_debouncer_full::notify::EventKind;

    let paths = &event.paths;

    // Skip if all paths match exclude patterns
    let should_process = paths.iter().any(|path| {
        let path_str = path.to_string_lossy();
        !exclude_patterns
            .iter()
            .any(|pattern| path_str.contains(pattern))
    });

    if !should_process {
        return None;
    }

    match &event.kind {
        EventKind::Create(_) | EventKind::Modify(_) => {
            // Only process regular files
            if let Some(path) = paths.first() {
                if path.is_file() {
                    return Some(FileChange::Modified(path.clone()));
                }
            }
            None
        }
        EventKind::Remove(_) => {
            if let Some(path) = paths.first() {
                return Some(FileChange::Deleted(path.clone()));
            }
            None
        }
        EventKind::Any | EventKind::Access(_) | EventKind::Other => None,
    }
}

/// Check if a path should be excluded based on patterns
pub fn should_exclude(path: &Path, exclude_patterns: &[String]) -> bool {
    let path_str = path.to_string_lossy();
    exclude_patterns
        .iter()
        .any(|pattern| path_str.contains(pattern))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_exclude() {
        let patterns = vec!["node_modules".to_string(), ".git".to_string()];

        assert!(should_exclude(
            Path::new("/project/node_modules/package/index.js"),
            &patterns
        ));
        assert!(should_exclude(
            Path::new("/project/.git/objects/abc"),
            &patterns
        ));
        assert!(!should_exclude(
            Path::new("/project/src/main.rs"),
            &patterns
        ));
    }

    #[test]
    fn test_watcher_config_default() {
        let config = WatcherConfig::default();
        assert!(config.paths.is_empty());
        assert_eq!(config.debounce_duration, Duration::from_secs(2));
        assert!(!config.exclude_patterns.is_empty());
    }
}
