//! Fault injection: the index stays honest when reads fail and code panics.
//!
//! Each test arms a fault point (`src/faults.rs`) for files in its own temp
//! directory, runs the real background indexer and watch loop, and checks
//! that a failure is retried, reported while it lasts, and never leaves the
//! index silently wrong. Run with:
//!
//! ```text
//! cargo test --features failpoints --test fault_injection
//! ```
#![cfg(feature = "failpoints")]

use fast_code_search::config::IndexerConfig;
use fast_code_search::faults::{self, Action, Fault};
use fast_code_search::search::{
    create_progress_broadcaster, run_background_indexer, watch_loop, BackgroundIndexerConfig,
    IndexingProgress, ReadFailures, SearchEngine, WatchLoopConfig,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// How long a change may take to show up (the macOS backend can be slow).
const TIMEOUT: Duration = Duration::from_secs(30);

/// A short first retry, so giving up (six tries) takes about 3 s, not 31 s.
const FIRST_RETRY: Duration = Duration::from_millis(50);

struct Rig {
    _temp: TempDir,
    root: PathBuf,
    config: IndexerConfig,
    engine: Arc<RwLock<SearchEngine>>,
    shutdown: Arc<AtomicBool>,
    watch: Option<JoinHandle<()>>,
}

impl Rig {
    /// A tree with `files` (name, content) under a fresh temp directory.
    fn new(files: &[(&str, &str)], watch: bool) -> Self {
        let temp = TempDir::new().unwrap();
        let root = temp.path().canonicalize().unwrap();
        for (name, text) in files {
            std::fs::write(root.join(name), text).unwrap();
        }
        let engine = SearchEngine::new();
        let engine = Arc::new(RwLock::new(engine));
        engine.write().unwrap().watch_backlog.read_failures =
            Arc::new(ReadFailures::with_first_retry(FIRST_RETRY));
        Self {
            config: IndexerConfig {
                paths: vec![root.to_string_lossy().to_string()],
                watch,
                ..Default::default()
            },
            _temp: temp,
            root,
            engine,
            shutdown: Arc::new(AtomicBool::new(false)),
            watch: None,
        }
    }

    /// Text that names one file of this rig only, for arming a fault.
    fn key(&self, name: &str) -> String {
        self.root.join(name).to_string_lossy().to_string()
    }

    fn arm(&self, point: &str, name: &str, action: Action, times: Option<usize>) {
        faults::set(
            point,
            Fault {
                action,
                path_contains: Some(self.key(name)),
                times,
            },
        );
    }

    fn disarm(&self, point: &str, name: &str) {
        faults::clear(point, Some(&self.key(name)));
    }

    fn build(&self) {
        run_background_indexer(BackgroundIndexerConfig {
            indexer_config: self.config.clone(),
            engine: self.engine.clone(),
            progress: Arc::new(RwLock::new(IndexingProgress::default())),
            progress_tx: create_progress_broadcaster(),
            shutdown: Arc::new(AtomicBool::new(false)),
        });
    }

    fn start_watching(&mut self) {
        let watcher = watch_loop::start_watcher(&self.config).expect("watcher");
        let cfg = WatchLoopConfig {
            indexer: self.config.clone(),
            engine: self.engine.clone(),
            shutdown: self.shutdown.clone(),
            batch_window: Duration::from_millis(50),
        };
        self.watch = Some(std::thread::spawn(move || watch_loop::run(watcher, cfg)));
        // Let the watcher subscribe before anything changes.
        std::thread::sleep(Duration::from_millis(300));
    }

    fn write(&self, name: &str, text: &str) {
        std::fs::write(self.root.join(name), text).unwrap();
    }

    /// The files (names) a word is found in.
    fn found_in(&self, word: &str) -> Vec<String> {
        let engine = self.engine.read().unwrap();
        let mut files: Vec<String> = engine
            .search(word, 1000)
            .into_iter()
            .map(|m| {
                Path::new(&m.file_path)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        files.sort();
        files.dedup();
        files
    }

    /// The files in the index (names).
    fn indexed(&self) -> Vec<String> {
        let engine = self.engine.read().unwrap();
        let mut files: Vec<String> = engine
            .file_ids_under(&self.root)
            .into_iter()
            .filter_map(|id| engine.get_file_path(id))
            .map(|p| {
                Path::new(&p)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        files.sort();
        files
    }

    fn pending(&self) -> usize {
        self.engine.read().unwrap().freshness().pending_changes
    }

    fn unreadable(&self) -> usize {
        self.engine.read().unwrap().freshness().unreadable_files
    }

    fn note(&self) -> String {
        self.engine
            .read()
            .unwrap()
            .freshness()
            .note(fast_code_search::search::unix_now_ms())
            .unwrap_or_default()
    }

    /// Wait until `cond` holds, or fail with `what`.
    fn wait_for(&self, what: &str, mut cond: impl FnMut(&Self) -> bool) {
        let start = Instant::now();
        while !cond(self) {
            assert!(
                start.elapsed() < TIMEOUT,
                "timed out waiting for {what}; pending {}, unreadable {}",
                self.pending(),
                self.unreadable()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Some(h) = self.watch.take() {
            let _ = h.join();
        }
    }
}

/// A change that can't be read at first is tried again and applied, and
/// the file keeps its old entry meanwhile (it used to be removed from the
/// index until it next changed).
#[test]
fn unreadable_change_is_retried_then_applied() {
    let mut rig = Rig::new(&[("a.py", "x = 'oldword'\n")], true);
    rig.build();
    rig.start_watching();
    rig.arm(
        "parse.read",
        "a.py",
        Action::IoError(std::io::ErrorKind::PermissionDenied),
        Some(2),
    );
    rig.write("a.py", "x = 'newword'\n");
    rig.wait_for("the change to be applied", |r| {
        r.found_in("newword") == ["a.py"]
    });
    assert!(rig.found_in("oldword").is_empty());
    rig.wait_for("nothing pending", |r| r.pending() == 0);
    assert_eq!(rig.unreadable(), 0);
}

/// While the retries last the file keeps its old entry and the change
/// counts as pending; after the last try it is left out of the index and
/// reported, and a later change that can be read brings it back.
#[test]
fn file_that_stays_unreadable_is_left_out_and_reported() {
    let mut rig = Rig::new(&[("a.py", "x = 'oldword'\n"), ("b.py", "y = 1\n")], true);
    rig.build();
    rig.start_watching();
    rig.arm(
        "parse.read",
        "a.py",
        Action::IoError(std::io::ErrorKind::PermissionDenied),
        None,
    );
    rig.write("a.py", "x = 'newword'\n");
    rig.wait_for("the failed read to count as pending", |r| r.pending() > 0);
    // Until it is given up on (about 1.5 s with the short retry), the old
    // entry is kept. (Searches read the file itself, so they see the
    // new text where its old trigrams still lead them.)
    assert_eq!(rig.indexed(), ["a.py", "b.py"]);
    rig.wait_for("the file to be given up on", |r| r.unreadable() == 1);
    rig.wait_for("the file to be left out", |r| r.indexed() == ["b.py"]);
    assert!(rig.found_in("newword").is_empty());
    assert_eq!(rig.pending(), 0);
    let note = rig.note();
    assert!(
        note.contains("1 file could not be read"),
        "searches should say a file is left out: {note:?}"
    );

    rig.disarm("parse.read", "a.py");
    rig.write("a.py", "x = 'thirdword'\n");
    rig.wait_for("the readable change to be applied", |r| {
        r.found_in("thirdword") == ["a.py"]
    });
    rig.wait_for("the report to clear", |r| r.unreadable() == 0);
}

/// A burst whose apply panics once is applied on the second try (a panic
/// used to drop the whole burst without a word).
#[test]
fn burst_that_panics_once_is_applied() {
    let mut rig = Rig::new(&[("a.py", "x = 1\n"), ("b.py", "y = 1\n")], true);
    rig.build();
    rig.start_watching();
    rig.arm("watch.apply", "a.py", Action::Panic, Some(1));
    rig.write("a.py", "x = 'panicword'\n");
    rig.write("b.py", "y = 'panicword'\n");
    rig.wait_for("both changes to be applied", |r| {
        r.found_in("panicword") == ["a.py", "b.py"]
    });
    rig.wait_for("nothing pending", |r| r.pending() == 0);
    assert_eq!(rig.unreadable(), 0);
}

/// A change that panics every time is left out and reported; the rest of
/// its burst is applied.
#[test]
fn change_that_always_panics_is_left_out_and_reported() {
    let mut rig = Rig::new(&[("a.py", "x = 'oldword'\n"), ("b.py", "y = 1\n")], true);
    rig.build();
    rig.start_watching();
    rig.arm("watch.apply", "a.py", Action::Panic, None);
    rig.write("a.py", "x = 'panicword'\n");
    rig.write("b.py", "y = 'panicword'\n");
    rig.wait_for("the other change to be applied", |r| {
        r.found_in("panicword") == ["b.py"]
    });
    rig.wait_for("the failing file to be reported", |r| r.unreadable() == 1);
    assert!(
        rig.found_in("oldword").is_empty(),
        "the failing file is left out"
    );
    rig.disarm("watch.apply", "a.py");
}

/// The initial build reads a file that failed once again before it ends.
#[test]
fn build_retries_a_file_it_could_not_read() {
    let rig = Rig::new(&[("a.py", "x = 'buildword'\n"), ("b.py", "y = 1\n")], false);
    rig.arm(
        "parse.read",
        "a.py",
        Action::IoError(std::io::ErrorKind::PermissionDenied),
        Some(1),
    );
    rig.build();
    assert_eq!(rig.found_in("buildword"), ["a.py"]);
    assert_eq!(rig.unreadable(), 0);
}

/// With watching off, a file the build can't read is reported, not
/// silently missing.
#[test]
fn build_reports_a_file_it_can_never_read() {
    let rig = Rig::new(&[("a.py", "x = 'buildword'\n"), ("b.py", "y = 1\n")], false);
    rig.arm(
        "parse.read",
        "a.py",
        Action::IoError(std::io::ErrorKind::PermissionDenied),
        None,
    );
    rig.build();
    rig.disarm("parse.read", "a.py");
    assert_eq!(rig.indexed(), ["b.py"]);
    assert_eq!(rig.unreadable(), 1);
    assert!(
        rig.note().contains("1 file could not be read"),
        "{}",
        rig.note()
    );
}

/// With watching on, the watcher keeps trying a file the build could not
/// read, and indexes it once it can.
#[test]
fn watcher_picks_up_a_file_the_build_could_not_read() {
    let mut rig = Rig::new(&[("a.py", "x = 'buildword'\n"), ("b.py", "y = 1\n")], true);
    rig.arm(
        "parse.read",
        "a.py",
        Action::IoError(std::io::ErrorKind::PermissionDenied),
        Some(3),
    );
    rig.build();
    rig.start_watching();
    rig.wait_for("the file to be indexed", |r| {
        r.found_in("buildword") == ["a.py"]
    });
    rig.wait_for("nothing pending", |r| r.pending() == 0);
    assert_eq!(rig.unreadable(), 0);
}
