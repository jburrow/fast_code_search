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
use fast_code_search::index::PersistedIndex;
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
    /// Outside the watched tree.
    index_dir: TempDir,
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
            index_dir: TempDir::new().unwrap(),
            root,
            engine,
            shutdown: Arc::new(AtomicBool::new(false)),
            watch: None,
        }
    }

    /// Save the index to (and after watcher updates, every
    /// `save_after_updates`) a file of this rig's own.
    fn saving(mut self, save_after_updates: usize) -> Self {
        self.config.index_path = Some(self.index_path().to_string_lossy().to_string());
        self.config.save_after_updates = save_after_updates;
        self
    }

    fn index_path(&self) -> PathBuf {
        self.index_dir.path().join("index.bin")
    }

    /// Files next to the index other than the index itself.
    fn index_leftovers(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.index_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n != "index.bin")
            .collect();
        names.sort();
        names
    }

    /// Stop the watcher (it saves on the way out) and wait for it.
    fn stop_watching(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Some(h) = self.watch.take() {
            h.join().unwrap();
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

    /// Arm `point` for this rig's index file.
    fn arm_save(&self, point: &str, action: Action, times: Option<usize>) {
        faults::set(
            point,
            Fault {
                action,
                path_contains: Some(self.index_path().to_string_lossy().to_string()),
                times,
            },
        );
    }

    fn disarm_save(&self, point: &str) {
        faults::clear(point, Some(&self.index_path().to_string_lossy()));
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
        if !std::thread::panicking() {
            let report = self.engine.read().unwrap().check_invariants();
            assert!(report.is_clean(), "{report}");
        }
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

/// A change the watcher never hears about (a backend that drops events)
/// is found by the background scrub, counted, and indexed again.
#[test]
fn scrub_finds_and_fixes_a_lost_change() {
    use fast_code_search::search::integrity::{run_scrub, ScrubConfig};
    let mut rig = Rig::new(&[("a.py", "x = 'oldword'\n"), ("b.py", "y = 1\n")], true);
    rig.build();
    rig.start_watching();
    let scrub = std::thread::spawn({
        let (engine, shutdown) = (rig.engine.clone(), rig.shutdown.clone());
        move || {
            run_scrub(
                engine,
                ScrubConfig {
                    files_per_minute: 6_000,
                    invariants_every: Some(Duration::from_millis(500)),
                    suspect_for: Duration::from_millis(300),
                },
                shutdown,
            )
        }
    });
    rig.arm("watch.lose", "a.py", Action::Panic, None);
    // Not the same second as the build, so the change shows in the mtime.
    std::thread::sleep(Duration::from_millis(1100));
    rig.write("a.py", "x = 'newword'\n");
    std::thread::sleep(Duration::from_millis(500));
    assert!(rig.found_in("newword").is_empty(), "the change was lost");
    rig.disarm("watch.lose", "a.py");
    rig.wait_for("the scrub to index the file again", |r| {
        r.found_in("newword") == ["a.py"]
    });
    let stats = rig
        .engine
        .read()
        .unwrap()
        .watch_backlog
        .scrub
        .lock()
        .unwrap()
        .clone();
    assert_eq!(stats.drift.get("changed"), Some(&1), "{stats:?}");
    assert_eq!(stats.requeued, 1);
    rig.wait_for("a consistency check", |r| {
        r.engine
            .read()
            .unwrap()
            .watch_backlog
            .scrub
            .lock()
            .unwrap()
            .invariant_checks
            > 0
    });
    rig.shutdown.store(true, Ordering::Release);
    scrub.join().unwrap();
}

/// A file that vanishes between being listed and being read is simply
/// not indexed: it is not reported as unreadable.
#[test]
fn file_that_vanishes_before_it_is_read_is_not_reported() {
    let rig = Rig::new(&[("a.py", "x = 'buildword'\n"), ("b.py", "y = 1\n")], false);
    rig.arm(
        "parse.read",
        "a.py",
        Action::IoError(std::io::ErrorKind::NotFound),
        None,
    );
    rig.build();
    rig.disarm("parse.read", "a.py");
    assert_eq!(rig.indexed(), ["b.py"]);
    assert_eq!(rig.unreadable(), 0);
    assert!(!rig.note().contains("could not be read"), "{}", rig.note());
}

/// Out of file handles (`EMFILE`, which has no `ErrorKind` of its own):
/// the build and the watcher both try again.
#[test]
fn out_of_file_handles_is_retried() {
    // EMFILE on Linux and macOS (on Windows 24 is some other error, which
    // is retried the same way).
    const EMFILE: i32 = 24;
    let mut rig = Rig::new(&[("a.py", "x = 'buildword'\n"), ("b.py", "y = 1\n")], true);
    rig.arm("parse.read", "a.py", Action::OsError(EMFILE), Some(1));
    rig.build();
    assert_eq!(rig.found_in("buildword"), ["a.py"]);
    rig.start_watching();
    rig.arm("parse.read", "b.py", Action::OsError(EMFILE), Some(2));
    rig.write("b.py", "y = 'newword'\n");
    rig.wait_for("the change to be applied", |r| {
        r.found_in("newword") == ["b.py"]
    });
    rig.wait_for("nothing pending", |r| r.pending() == 0);
    assert_eq!(rig.unreadable(), 0);
}

/// A panic while reading one file during the build is tried again; it
/// used to leave the file out of the index without a word.
#[test]
fn build_retries_a_file_whose_read_panicked() {
    let rig = Rig::new(&[("a.py", "x = 'buildword'\n"), ("b.py", "y = 1\n")], false);
    rig.arm("parse.panic", "a.py", Action::Panic, Some(1));
    rig.build();
    assert_eq!(rig.found_in("buildword"), ["a.py"]);
    assert_eq!(rig.unreadable(), 0);
}

/// A file whose read panics every time is reported, like one that cannot
/// be read.
#[test]
fn build_reports_a_file_whose_read_always_panics() {
    let rig = Rig::new(&[("a.py", "x = 'buildword'\n"), ("b.py", "y = 1\n")], false);
    rig.arm("parse.panic", "a.py", Action::Panic, None);
    rig.build();
    rig.disarm("parse.panic", "a.py");
    assert_eq!(rig.indexed(), ["b.py"]);
    assert_eq!(rig.unreadable(), 1);
    assert!(
        rig.note().contains("1 file could not be read"),
        "{}",
        rig.note()
    );
}

/// A change whose read panics once is applied on the next try.
#[test]
fn watcher_retries_a_change_whose_read_panicked() {
    let mut rig = Rig::new(&[("a.py", "x = 'oldword'\n"), ("b.py", "y = 1\n")], true);
    rig.build();
    rig.start_watching();
    rig.arm("parse.panic", "a.py", Action::Panic, Some(1));
    rig.write("a.py", "x = 'newword'\n");
    rig.write("b.py", "y = 'newword'\n");
    rig.wait_for("both changes to be applied", |r| {
        r.found_in("newword") == ["a.py", "b.py"]
    });
    rig.wait_for("nothing pending", |r| r.pending() == 0);
    assert_eq!(rig.unreadable(), 0);
}

/// Symbol extraction that panics (tree-sitter on a pathological file)
/// leaves the file searchable as text.
#[test]
fn file_whose_parser_panics_stays_searchable() {
    let rig = Rig::new(
        &[
            ("a.py", "def parsed_fn():\n    return 'buildword'\n"),
            ("b.py", "y = 1\n"),
        ],
        false,
    );
    rig.arm("parse.symbols", "a.py", Action::Panic, None);
    rig.build();
    rig.disarm("parse.symbols", "a.py");
    assert_eq!(rig.found_in("buildword"), ["a.py"]);
    assert_eq!(rig.unreadable(), 0);
}

/// Events the OS lost (its queue overflowed) are made up by a rescan of
/// the watched paths: an edit and a removal it never reported are both in
/// the index afterwards.
#[test]
fn lost_events_are_made_up_by_a_rescan() {
    let mut rig = Rig::new(
        &[
            ("a.py", "x = 'oldword'\n"),
            ("b.py", "y = 1\n"),
            ("c.py", "z = 'goneword'\n"),
        ],
        true,
    );
    rig.build();
    rig.start_watching();
    rig.arm("watch.overflow", "a.py", Action::Panic, None);
    rig.arm("watch.overflow", "c.py", Action::Panic, None);
    // Not the same second as the build, so the change shows in the mtime.
    std::thread::sleep(Duration::from_millis(1100));
    rig.write("a.py", "x = 'newword'\n");
    std::fs::remove_file(rig.root.join("c.py")).unwrap();
    rig.wait_for("the rescan to apply both changes", |r| {
        r.found_in("newword") == ["a.py"] && r.indexed() == ["a.py", "b.py"]
    });
    rig.disarm("watch.overflow", "a.py");
    rig.disarm("watch.overflow", "c.py");
    assert!(rig.found_in("oldword").is_empty());
    rig.wait_for("nothing pending", |r| r.pending() == 0);
}

/// A slow disk slows the watcher, not searches: the watcher reads files
/// without holding the index lock, and the change counts as pending while
/// it waits.
#[test]
fn slow_disk_does_not_hold_up_searches() {
    let mut rig = Rig::new(&[("a.py", "x = 'oldword'\n"), ("b.py", "y = 1\n")], true);
    rig.build();
    rig.start_watching();
    rig.arm(
        "parse.read",
        "a.py",
        Action::Delay(Duration::from_secs(2)),
        Some(1),
    );
    rig.write("a.py", "x = 'newword'\n");
    rig.wait_for("the change to count as pending", |r| r.pending() > 0);
    let started = Instant::now();
    let mut slowest = Duration::ZERO;
    while started.elapsed() < Duration::from_millis(1000) {
        let t = Instant::now();
        let _ = rig.found_in("oldword");
        slowest = slowest.max(t.elapsed());
    }
    assert!(
        slowest < Duration::from_millis(500),
        "a search waited {slowest:?} for a slow read"
    );
    rig.wait_for("the change to be applied", |r| {
        r.found_in("newword") == ["a.py"]
    });
    rig.wait_for("nothing pending", |r| r.pending() == 0);
}

/// Saving onto a full disk, or failing to move the new index into place,
/// leaves the previous index as it was and no temp file behind.
#[test]
fn failed_save_keeps_the_previous_index() {
    let rig = Rig::new(&[("a.py", "x = 'buildword'\n")], false).saving(0);
    rig.build();
    let saved = std::fs::read(rig.index_path()).expect("the build saved the index");
    let engine = rig.engine.read().unwrap();
    for (point, action) in [
        (
            "save.write",
            Action::IoError(std::io::ErrorKind::StorageFull),
        ),
        (
            "save.rename",
            Action::IoError(std::io::ErrorKind::PermissionDenied),
        ),
    ] {
        rig.arm_save(point, action, None);
        let result = engine.save_index(&rig.index_path(), &rig.config);
        rig.disarm_save(point);
        assert!(result.is_err(), "{point}: the save should fail");
        assert!(
            std::fs::read(rig.index_path()).unwrap() == saved,
            "{point}: the previous index changed"
        );
        assert_eq!(rig.index_leftovers(), Vec::<String>::new(), "{point}");
    }
    // A rename that fails once is tried again with the old index moved
    // aside, which is put back if that fails too (above).
    rig.arm_save(
        "save.rename",
        Action::IoError(std::io::ErrorKind::PermissionDenied),
        Some(1),
    );
    engine
        .save_index(&rig.index_path(), &rig.config)
        .expect("the second rename works");
    rig.disarm_save("save.rename");
    assert_eq!(rig.index_leftovers(), Vec::<String>::new());
    assert!(PersistedIndex::load(&rig.index_path()).is_ok());
}

/// Updates whose save failed still count: the next save (here, at
/// shutdown) writes them. They used to be forgotten, so the shutdown
/// skipped its save and the saved index lacked them.
#[test]
fn updates_whose_save_failed_are_saved_at_shutdown() {
    let mut rig = Rig::new(&[("a.py", "x = 'oldword'\n"), ("b.py", "y = 1\n")], true).saving(1);
    rig.build();
    rig.start_watching();
    rig.arm_save(
        "save.write",
        Action::IoError(std::io::ErrorKind::StorageFull),
        None,
    );
    std::thread::sleep(Duration::from_millis(1100));
    // A new size, so the saved index shows which version it holds.
    let new_text = "x = 'a longer newword'\n";
    rig.write("a.py", new_text);
    rig.wait_for("the change to be applied", |r| {
        r.found_in("newword") == ["a.py"]
    });
    // The save after the update has failed by the time nothing is pending.
    rig.wait_for("nothing pending", |r| r.pending() == 0);
    std::thread::sleep(Duration::from_millis(300));
    rig.disarm_save("save.write");
    rig.stop_watching();

    let saved = PersistedIndex::load(&rig.index_path()).unwrap();
    let a = saved
        .files
        .iter()
        .find(|f| f.path.ends_with("a.py"))
        .expect("a.py in the saved index");
    assert_eq!(
        a.size,
        new_text.len() as u64,
        "the saved index is out of date"
    );
}

/// A folder that can't be read is counted at every surface's freshness
/// note, by the build and by the watcher, instead of its files going
/// missing without a word; once it can be read, a rescan of its parent
/// indexes it and the count goes away. (Real permissions, not a fault
/// point: skipped where they are not enforced, as for root.)
#[cfg(unix)]
#[test]
fn unreadable_folder_is_reported_until_it_can_be_read() {
    use std::os::unix::fs::PermissionsExt;
    let mut rig = Rig::new(&[("a.py", "x = 'visible'\n")], true);
    let locked = rig.root.join("locked");
    std::fs::create_dir_all(&locked).unwrap();
    std::fs::write(locked.join("b.py"), "y = 'hidden'\n").unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read_dir(&locked).is_ok() {
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        eprintln!("skipped: permissions are not enforced for this user");
        return;
    }
    rig.build();
    rig.start_watching();
    let f = rig.engine.read().unwrap().freshness();
    assert_eq!((f.unreadable_folders, f.unwatched_folders), (1, 1));
    assert!(
        rig.note().contains("1 folder could not be read"),
        "{}",
        rig.note()
    );
    assert_eq!(rig.found_in("hidden"), Vec::<String>::new());

    // Readable again: touching the parent makes the watcher reconcile it.
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(rig.root.join("c.py"), "z = 1\n").unwrap();
    std::fs::rename(&locked, rig.root.join("unlocked")).unwrap();
    rig.wait_for("the folder to be indexed", |r| {
        r.found_in("hidden") == ["b.py"]
    });
    rig.wait_for("the count to clear", |r| {
        r.engine.read().unwrap().freshness().unreadable_folders == 0
    });
}
