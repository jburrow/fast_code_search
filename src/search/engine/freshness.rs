//! How current the index is compared with the files on disk.
//!
//! With the file watcher off, nothing written after the last scan reaches
//! the index, while `read_file` / the file view read the disk directly. The
//! types here let every client (web UI, `fcs`, MCP) say so instead of
//! silently missing a fresh edit.

use super::*;
use crate::index::persistence::mtime_secs_of;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// When the index last matched the disk, and whether a watcher keeps it so.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IndexFreshness {
    /// A file watcher applies changes as they happen.
    pub watching: bool,
    /// When the last full scan of the indexed folders started (Unix millis);
    /// absent while the first build or reconcile is still running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synced_at_ms: Option<u64>,
    /// When the watcher last changed the index (Unix millis).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at_ms: Option<u64>,
    /// File changes the watcher has seen and not applied yet: searches do
    /// not see them until it has (a branch switch can leave thousands).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub pending_changes: usize,
    /// Files that could not be read and are left out of the index (see
    /// [`ReadFailures`]); the server log names them.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub unreadable_files: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// The changes a file watcher has seen and not applied yet. Counters, not
/// engine state: the watcher updates them without the engine lock, so a
/// search reports the backlog as it is while the watcher waits to install.
#[derive(Debug, Clone, Default)]
pub struct WatchBacklog {
    /// Events queued by the watcher and not picked up yet (shared with
    /// [`crate::search::FileWatcher`]).
    pub queued: Arc<AtomicUsize>,
    /// Changes picked up and not installed yet.
    pub applying: Arc<AtomicUsize>,
    /// Files that could not be read when indexed, being retried or given
    /// up on (shared by the initial build and the watcher).
    pub read_failures: Arc<ReadFailures>,
    /// Paths the background scrub found out of step with the disk, for the
    /// watcher to index again.
    pub requeue: Arc<std::sync::Mutex<Vec<PathBuf>>>,
    /// What the background scrub has checked and found so far.
    pub scrub: Arc<std::sync::Mutex<crate::search::integrity::ScrubStats>>,
}

impl WatchBacklog {
    /// Changes seen and not applied yet, files waiting for a read retry
    /// included.
    pub fn pending(&self) -> usize {
        self.queued.load(Ordering::Acquire)
            + self.applying.load(Ordering::Acquire)
            + self.read_failures.retrying()
    }
}

/// How many times a file that cannot be read is tried before it is left
/// out of the index (and reported).
pub const MAX_READ_ATTEMPTS: u32 = 6;

/// The wait before the first retry; it doubles each time (1, 2, 4, 8 and
/// 16 s: a file is given up on about half a minute after the first
/// failure). Long enough for the usual short-lived lock (an editor's or a
/// virus scanner's on Windows, a build tool rewriting the file), short
/// enough that the index catches up soon after.
pub const FIRST_READ_RETRY: Duration = Duration::from_secs(1);

/// Files that could not be read (permission denied, locked by another
/// program, an I/O error) when the build or the watcher went to index them.
///
/// A file that fails is retried with a growing wait, up to
/// [`MAX_READ_ATTEMPTS`] tries; until then it counts as a change still to
/// apply. After that it is given up on: it is left out of the index, as
/// ripgrep leaves out a file it cannot open, and counted in
/// [`IndexFreshness::unreadable_files`] so searches say results may miss
/// it. A later change to the file tries again. (A failed read used to
/// remove the file from the index without a word, until it next changed.)
#[derive(Debug)]
pub struct ReadFailures {
    inner: std::sync::Mutex<ReadFailuresInner>,
    first_retry: Duration,
}

impl Default for ReadFailures {
    fn default() -> Self {
        Self::with_first_retry(FIRST_READ_RETRY)
    }
}

#[derive(Debug, Default)]
struct ReadFailuresInner {
    files: HashMap<PathBuf, ReadFailure>,
    given_up: usize,
}

#[derive(Debug, Clone, Copy)]
struct ReadFailure {
    attempts: u32,
    /// When to try again; `None` while a retry is under way, or once given up.
    next_try: Option<Instant>,
    given_up: bool,
}

impl ReadFailures {
    /// Retry after `first_retry`, doubling each time (tests use a short one).
    pub fn with_first_retry(first_retry: Duration) -> Self {
        Self {
            inner: Default::default(),
            first_retry,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ReadFailuresInner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Record a failed read of `path`. True if it is now given up on.
    pub fn failed(&self, path: &Path, now: Instant) -> bool {
        let mut inner = self.lock();
        let inner = &mut *inner;
        let entry = inner
            .files
            .entry(path.to_path_buf())
            .or_insert(ReadFailure {
                attempts: 0,
                next_try: None,
                given_up: false,
            });
        if entry.given_up {
            // A later change could not be read either: start over.
            entry.given_up = false;
            entry.attempts = 0;
            inner.given_up -= 1;
        }
        entry.attempts += 1;
        if entry.attempts >= MAX_READ_ATTEMPTS {
            entry.given_up = true;
            entry.next_try = None;
            inner.given_up += 1;
            true
        } else {
            entry.next_try = Some(now + self.first_retry * 2u32.pow(entry.attempts - 1));
            false
        }
    }

    /// `path` was read (or is gone, or no longer indexed): forget it.
    pub fn cleared(&self, path: &Path) {
        let mut inner = self.lock();
        if inner.files.is_empty() {
            return;
        }
        if let Some(f) = inner.files.remove(path) {
            if f.given_up {
                inner.given_up -= 1;
            }
        }
    }

    /// Files due for another try at `now`, marked as under way.
    pub fn take_due(&self, now: Instant) -> Vec<PathBuf> {
        let mut inner = self.lock();
        let mut due = Vec::new();
        for (path, f) in inner.files.iter_mut() {
            if f.next_try.is_some_and(|t| t <= now) {
                f.next_try = None;
                due.push(path.clone());
            }
        }
        due.sort();
        due
    }

    /// When the next retry is due, if any is waiting.
    pub fn next_due(&self) -> Option<Instant> {
        self.lock().files.values().filter_map(|f| f.next_try).min()
    }

    /// Give up on every file still being retried (nothing will retry them:
    /// the build is over and no watcher runs). Returns them.
    pub fn give_up_all(&self) -> Vec<PathBuf> {
        let mut inner = self.lock();
        let mut gave_up = Vec::new();
        for (path, f) in inner.files.iter_mut() {
            if !f.given_up {
                f.given_up = true;
                f.next_try = None;
                gave_up.push(path.clone());
            }
        }
        inner.given_up += gave_up.len();
        gave_up.sort();
        gave_up
    }

    /// Give up on `path` straight away (applying its change failed twice).
    pub fn give_up(&self, path: &Path) {
        let mut inner = self.lock();
        let entry = inner
            .files
            .entry(path.to_path_buf())
            .or_insert(ReadFailure {
                attempts: 0,
                next_try: None,
                given_up: false,
            });
        if !entry.given_up {
            entry.given_up = true;
            entry.next_try = None;
            inner.given_up += 1;
        }
    }

    /// Files still being retried.
    pub fn retrying(&self) -> usize {
        let inner = self.lock();
        inner.files.len() - inner.given_up
    }

    /// Files given up on: left out of the index because they can't be read.
    pub fn given_up(&self) -> usize {
        self.lock().given_up
    }

    /// Is anything recorded at all? (A cheap check before clearing paths.)
    pub fn is_empty(&self) -> bool {
        self.lock().files.is_empty()
    }
}

impl IndexFreshness {
    /// One line warning that results may miss recent writes, or `None` while
    /// a watcher keeps the index current or no scan has completed (an engine
    /// built in-process, or the first build, which progress already reports).
    /// With a watcher, it names the changes the watcher has not applied yet.
    pub fn note(&self, now_ms: u64) -> Option<String> {
        let note = if self.watching {
            (self.pending_changes > 0).then(|| {
                format!(
                    "The file watcher is still applying {}; results may not include them yet.",
                    changes(self.pending_changes)
                )
            })
        } else {
            self.synced_at_ms.map(|at| {
                format!(
                    "Index last synced with disk {} ({}); file watching is off, so files \
                     written since then may be missing or out of date.",
                    format_age(now_ms.saturating_sub(at)),
                    format_utc(at)
                )
            })
        };
        let unreadable =
            (self.unreadable_files > 0).then(|| {
                format!(
                "{} could not be read and {} left out of the index (the server log names them).",
                files(self.unreadable_files),
                if self.unreadable_files == 1 { "is" } else { "are" }
            )
            });
        match (note, unreadable) {
            (Some(a), Some(b)) => Some(format!("{a} {b}")),
            (a, b) => a.or(b),
        }
    }

    /// One line for status output: watching state plus the last sync.
    pub fn summary(&self, now_ms: u64) -> String {
        let synced = match self.synced_at_ms {
            Some(at) => format!(
                "last synced with disk {} ({})",
                format_age(now_ms.saturating_sub(at)),
                format_utc(at)
            ),
            None => "first build still running".to_string(),
        };
        let updated = match self.updated_at_ms {
            Some(at) if self.watching => format!(
                ", last change applied {}",
                format_age(now_ms.saturating_sub(at))
            ),
            _ => String::new(),
        };
        let pending = if self.watching && self.pending_changes > 0 {
            format!(", {} still to apply", changes(self.pending_changes))
        } else {
            String::new()
        };
        let unreadable = if self.unreadable_files > 0 {
            format!(", {} could not be read", files(self.unreadable_files))
        } else {
            String::new()
        };
        format!(
            "index {synced}, file watching {}{updated}{pending}{unreadable}",
            if self.watching { "on" } else { "off" }
        )
    }
}

/// "1 file change", "2000 file changes".
fn changes(n: usize) -> String {
    format!("{n} file change{}", if n == 1 { "" } else { "s" })
}

/// "1 file", "3 files".
fn files(n: usize) -> String {
    format!("{n} file{}", if n == 1 { "" } else { "s" })
}

/// Whether one path's indexed content still matches the file on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathFreshness {
    /// Indexed, and the file's modification time and size are unchanged.
    Current,
    /// Indexed, but the file changed on disk after it was indexed.
    Stale,
    /// Indexed, but the file no longer exists.
    Deleted,
    /// On disk but not in the index: written after the last sync, or
    /// excluded by the configuration.
    NotIndexed,
    /// Neither in the index nor on disk.
    NotFound,
    /// Indexed, but what was indexed was not recorded, so it can't be compared.
    Unknown,
}

impl PathFreshness {
    /// Short explanation for people and agents.
    pub fn describe(self) -> &'static str {
        match self {
            Self::Current => "current: unchanged since it was indexed",
            Self::Stale => "stale: changed on disk after it was indexed",
            Self::Deleted => "deleted: still in the index but gone from disk",
            Self::NotIndexed => {
                "not indexed: exists on disk but is not in the index (written after the last \
                 sync, or excluded by the configuration)"
            }
            Self::NotFound => "not found: neither in the index nor under an indexed folder",
            Self::Unknown => "indexed, but its indexed version was not recorded",
        }
    }
}

impl SearchEngine {
    /// When the index last matched the disk.
    pub fn freshness(&self) -> IndexFreshness {
        IndexFreshness {
            watching: self.watching,
            synced_at_ms: self.synced_at_ms,
            updated_at_ms: self.updated_at_ms,
            pending_changes: if self.watching {
                self.watch_backlog.pending()
            } else {
                0
            },
            unreadable_files: self.watch_backlog.read_failures.given_up(),
        }
    }

    /// Compare one path's indexed (mtime, size) with the file on disk.
    ///
    /// `path` is absolute, a display path as results print it, or relative
    /// to an indexed folder. Unlike [`Self::find_file_id`] there is no
    /// suffix fallback: a new `src/x.rs` must not match some other indexed
    /// `.../src/x.rs` and be reported current.
    pub fn path_freshness(&self, path: &str) -> PathFreshness {
        let candidates = self.disk_candidates(path);
        let id = candidates
            .iter()
            .find_map(|p| self.find_file_id_exact(p))
            .or_else(|| self.find_by_display_path(path));
        let Some(id) = id else {
            return if candidates.iter().any(|p| p.is_file()) {
                PathFreshness::NotIndexed
            } else {
                PathFreshness::NotFound
            };
        };
        let Some(file) = self.file_store.get(id) else {
            return PathFreshness::NotFound;
        };
        let (mtime, size) = self
            .indexed_meta
            .get(id as usize)
            .copied()
            .unwrap_or((0, 0));
        match std::fs::metadata(&*file.path) {
            Err(_) => PathFreshness::Deleted,
            Ok(_) if (mtime, size) == (0, 0) => PathFreshness::Unknown,
            Ok(m) if mtime_secs_of(&m) == mtime && m.len() == size => PathFreshness::Current,
            Ok(_) => PathFreshness::Stale,
        }
    }

    /// Where `path` would be on disk: itself when absolute, otherwise under
    /// each indexed folder, both as written and with a leading root-folder
    /// name removed (the display form).
    fn disk_candidates(&self, path: &str) -> Vec<PathBuf> {
        let p = Path::new(path);
        if p.is_absolute() {
            return vec![p.to_path_buf()];
        }
        let display = path.replace('\\', "/");
        let mut out = Vec::new();
        for (root, label) in self.roots_with_labels() {
            out.push(root.join(&display));
            if let Some(rest) = display
                .strip_prefix(label)
                .filter(|_| !label.is_empty())
                .and_then(|r| r.strip_prefix('/'))
            {
                out.push(root.join(rest));
            }
        }
        out
    }
}

/// Milliseconds since the Unix epoch.
pub fn unix_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn format_age(ms: u64) -> String {
    let secs = ms / 1000;
    match secs {
        0..=59 => "just now".to_string(),
        60..=3599 => format!("{} min ago", secs / 60),
        3600..=86_399 => format!("{} h {} min ago", secs / 3600, secs % 3600 / 60),
        _ => format!("{} days ago", secs / 86_400),
    }
}

fn format_utc(ms: u64) -> String {
    chrono::DateTime::from_timestamp_millis(ms as i64)
        .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| "unknown time".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_is_silent_while_watching() {
        let f = IndexFreshness {
            watching: true,
            synced_at_ms: Some(0),
            updated_at_ms: None,
            pending_changes: 0,
            unreadable_files: 0,
        };
        assert_eq!(f.note(1_000_000), None);
    }

    #[test]
    fn note_reports_changes_the_watcher_has_not_applied() {
        let f = IndexFreshness {
            watching: true,
            synced_at_ms: Some(0),
            updated_at_ms: None,
            pending_changes: 2_500,
            unreadable_files: 0,
        };
        let note = f.note(1_000_000).unwrap();
        assert!(note.contains("2500 file changes"), "{note}");
        assert!(f
            .summary(1_000_000)
            .contains("2500 file changes still to apply"));
    }

    #[test]
    fn freshness_counts_queued_and_applying_changes_while_watching() {
        let mut engine = SearchEngine::new();
        engine.watch_backlog.queued.store(3, Ordering::Release);
        engine.watch_backlog.applying.store(4, Ordering::Release);
        assert_eq!(engine.freshness().pending_changes, 0, "not watching");
        engine.watching = true;
        assert_eq!(engine.freshness().pending_changes, 7);
    }

    #[test]
    fn note_reports_age_and_time_when_not_watching() {
        let at = 1_760_000_000_000; // 2025-10-09 08:53 UTC
        let f = IndexFreshness {
            watching: false,
            synced_at_ms: Some(at),
            updated_at_ms: None,
            pending_changes: 0,
            unreadable_files: 0,
        };
        let note = f.note(at + 6 * 60_000).unwrap();
        assert!(note.contains("6 min ago"), "{note}");
        assert!(note.contains("2025-10-09 08:53 UTC"), "{note}");
        assert!(note.contains("watching is off"), "{note}");
        assert_eq!(IndexFreshness::default().note(at), None);
    }

    #[test]
    fn path_freshness_tracks_edits_new_files_and_deletes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let a = root.join("a.rs");
        std::fs::write(&a, "fn a() {}\n").unwrap();
        let mut engine = SearchEngine::new();
        engine.add_root_path(&root);
        engine.update_files(std::slice::from_ref(&a));
        let root_name = root.file_name().unwrap().to_str().unwrap().to_string();

        assert_eq!(engine.path_freshness("a.rs"), PathFreshness::Current);
        assert_eq!(
            engine.path_freshness(&format!("{root_name}/a.rs")),
            PathFreshness::Current
        );
        assert_eq!(
            engine.path_freshness(a.to_str().unwrap()),
            PathFreshness::Current
        );

        std::fs::write(&a, "fn a() { changed(); }\n").unwrap();
        assert_eq!(engine.path_freshness("a.rs"), PathFreshness::Stale);

        std::fs::write(root.join("b.rs"), "fn b() {}\n").unwrap();
        assert_eq!(engine.path_freshness("b.rs"), PathFreshness::NotIndexed);
        // No suffix fallback: a new file must not match an indexed one.
        assert_eq!(engine.path_freshness("x/a.rs"), PathFreshness::NotFound);

        std::fs::remove_file(&a).unwrap();
        assert_eq!(engine.path_freshness("a.rs"), PathFreshness::Deleted);
    }

    #[test]
    fn read_failures_back_off_then_give_up() {
        let failures = ReadFailures::default();
        let path = Path::new("/r/a.py");
        let t0 = Instant::now();
        let mut now = t0;
        for attempt in 1..MAX_READ_ATTEMPTS {
            assert!(!failures.failed(path, now), "attempt {attempt}");
            assert_eq!(failures.retrying(), 1);
            let due = failures.next_due().expect("a retry is due");
            assert_eq!(due - now, FIRST_READ_RETRY * 2u32.pow(attempt - 1));
            assert!(failures.take_due(due - Duration::from_millis(1)).is_empty());
            assert_eq!(failures.take_due(due), vec![path.to_path_buf()]);
            // Under way: not handed out again.
            assert!(failures.take_due(due + Duration::from_secs(60)).is_empty());
            now = due;
        }
        assert!(failures.failed(path, now), "the last try gives up");
        assert_eq!((failures.retrying(), failures.given_up()), (0, 1));
        assert_eq!(failures.next_due(), None);
        // About half a minute from the first failure to giving up.
        assert_eq!(now - t0, Duration::from_secs(31));

        // A later change that fails starts over; one that reads clears it.
        assert!(!failures.failed(path, now));
        assert_eq!((failures.retrying(), failures.given_up()), (1, 0));
        failures.cleared(path);
        assert!(failures.is_empty());
        assert_eq!((failures.retrying(), failures.given_up()), (0, 0));
    }

    #[test]
    fn read_failures_give_up_all_and_count_as_pending() {
        let mut engine = SearchEngine::new();
        engine.watching = true;
        let failures = &engine.watch_backlog.read_failures;
        failures.failed(Path::new("/r/a.py"), Instant::now());
        failures.failed(Path::new("/r/b.py"), Instant::now());
        failures.give_up(Path::new("/r/c.py"));
        let f = engine.freshness();
        assert_eq!((f.pending_changes, f.unreadable_files), (2, 1));
        assert_eq!(
            failures.give_up_all(),
            vec![PathBuf::from("/r/a.py"), PathBuf::from("/r/b.py")]
        );
        let f = engine.freshness();
        assert_eq!((f.pending_changes, f.unreadable_files), (0, 3));
        assert_eq!(
            f.note(0).as_deref(),
            Some("3 files could not be read and are left out of the index (the server log names them).")
        );
    }

    #[test]
    fn note_reports_unreadable_files_with_or_without_watching() {
        let watching = IndexFreshness {
            watching: true,
            pending_changes: 2,
            unreadable_files: 1,
            ..Default::default()
        };
        assert_eq!(
            watching.note(0).as_deref(),
            Some(
                "The file watcher is still applying 2 file changes; results may not include \
                 them yet. 1 file could not be read and is left out of the index (the server \
                 log names them)."
            )
        );
        assert!(watching.summary(0).ends_with(", 1 file could not be read"));
        let not_watching = IndexFreshness {
            unreadable_files: 4,
            ..Default::default()
        };
        assert_eq!(
            not_watching.note(0).as_deref(),
            Some("4 files could not be read and are left out of the index (the server log names them).")
        );
    }
}
