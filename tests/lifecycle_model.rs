//! Model-based test of the index lifecycle.
//!
//! proptest generates sequences of operations: file writes, appends,
//! truncations, deletes, renames, directory renames and deletes, ignore-file
//! edits, files turning binary or too large, changes whose events are lost
//! (followed by the rescan the watcher does when events are lost), saves,
//! restarts and crashes. Each runs against a real directory and the real
//! engine: changes go through the watcher's own `apply_burst` (planning,
//! parsing, installing), restarts through the background indexer's load and
//! reconcile. Alongside, a deliberately naive model (a map from path to
//! bytes, a hand-written ignore matcher, a line-by-line substring search)
//! says what the index should hold.
//!
//! After every operation:
//! - the indexed file set equals the model's;
//! - every word's hits, read through `SearchEngine::run` page by page,
//!   equal the model's, with no hit repeated and each file's hits together
//!   (rank order: files by best hit, a file's hits side by side);
//! - `check_invariants` is clean and every file's content check is current.
//!
//! A failure is shrunk to the shortest sequence that still fails. Replay a
//! run with `PROPTEST_CASES` / the seed proptest prints; run more cases
//! with `PROPTEST_CASES=2000 cargo test --release --test lifecycle_model`.

use fast_code_search::config::IndexerConfig;
use fast_code_search::search::request::{SearchMode, SearchRequest};
use fast_code_search::search::{
    create_progress_broadcaster, run_background_indexer, watch_loop, BackgroundIndexerConfig,
    FileChange, FileCheck, IndexingProgress, SearchEngine, SearchSettings,
};
use proptest::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};

/// The words files are made of; each is searched for after every step.
const WORDS: [&str; 8] = [
    "amber", "basalt", "cobalt", "dune", "ember", "fjord", "garnet", "harbor",
];
/// Folders operations pick from ("" is the root).
const DIRS: [&str; 4] = ["", "pkg", "pkg/sub", "web"];
/// File names operations pick from.
const NAMES: [&str; 6] = ["a.py", "b.py", "c.py", "notes.txt", "d.ts", "bin.py"];
/// Ignore files and the patterns they may hold.
const IGNORE_FILES: [&str; 3] = [".gitignore", ".ignore", ".rgignore"];
const PATTERNS: [&str; 4] = ["*.txt", "b*.py", "sub/", "c.py"];
/// The size cap the tests index with, so "too large" is cheap to reach.
const MAX_FILE_SIZE: u64 = 4096;

#[derive(Debug, Clone)]
enum Op {
    /// Write a file: one line per entry, each a few words.
    Write {
        file: usize,
        lines: Vec<Vec<usize>>,
    },
    Append {
        file: usize,
        word: usize,
    },
    Truncate {
        file: usize,
    },
    Delete {
        file: usize,
    },
    Rename {
        from: usize,
        to: usize,
    },
    /// Rename a folder (not the root) to `<name>_moved`, or back.
    RenameDir {
        dir: usize,
    },
    DeleteDir {
        dir: usize,
    },
    /// Write an ignore file with one pattern, or delete it (`None`).
    Ignore {
        dir: usize,
        kind: usize,
        pattern: Option<usize>,
    },
    /// Overwrite a file with NUL bytes.
    Binary {
        file: usize,
    },
    /// Grow a file past the size cap.
    Oversize {
        file: usize,
    },
    /// Do `op` without telling the index, then rescan the root, as the
    /// watcher does when the OS reports lost events.
    Lost(Box<Op>),
    Save,
    /// Save, then start again from the saved index.
    Restart,
    /// Start again from the last save (or from scratch), losing whatever
    /// happened since.
    Crash,
}

fn file_op() -> impl Strategy<Value = Op> {
    let file = 0..DIRS.len() * NAMES.len();
    let line = prop::collection::vec(0..WORDS.len(), 1..4);
    prop_oneof![
        6 => (file.clone(), prop::collection::vec(line, 1..5))
            .prop_map(|(file, lines)| Op::Write { file, lines }),
        2 => (file.clone(), 0..WORDS.len()).prop_map(|(file, word)| Op::Append { file, word }),
        1 => file.clone().prop_map(|file| Op::Truncate { file }),
        2 => file.clone().prop_map(|file| Op::Delete { file }),
        2 => (file.clone(), file.clone()).prop_map(|(from, to)| Op::Rename { from, to }),
        1 => (1..DIRS.len()).prop_map(|dir| Op::RenameDir { dir }),
        1 => (1..DIRS.len()).prop_map(|dir| Op::DeleteDir { dir }),
        2 => (0..DIRS.len(), 0..IGNORE_FILES.len(), prop::option::of(0..PATTERNS.len()))
            .prop_map(|(dir, kind, pattern)| Op::Ignore { dir, kind, pattern }),
        1 => file.clone().prop_map(|file| Op::Binary { file }),
        1 => file.prop_map(|file| Op::Oversize { file }),
    ]
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        20 => file_op(),
        3 => file_op().prop_map(|o| Op::Lost(Box::new(o))),
        1 => Just(Op::Save),
        1 => Just(Op::Restart),
        1 => Just(Op::Crash),
    ]
}

/// The naive model: what is on disk, and what the index should make of it.
#[derive(Default, Debug)]
struct Model {
    /// Relative path -> content.
    files: BTreeMap<String, Vec<u8>>,
    /// Folders (from [`DIRS`]) renamed to `<name>_moved`.
    moved: BTreeSet<&'static str>,
}

impl Model {
    /// Where folder `i` of [`DIRS`] is now, after its own renames and its
    /// parents'.
    fn dir(&self, i: usize) -> String {
        let d = DIRS[i];
        if d.is_empty() {
            return String::new();
        }
        let (parent, leaf) = split(d);
        let parent = DIRS
            .iter()
            .position(|p| *p == parent)
            .map(|p| self.dir(p))
            .unwrap_or_default();
        if self.moved.contains(d) {
            join(&parent, &format!("{leaf}_moved"))
        } else {
            join(&parent, leaf)
        }
    }

    fn file(&self, i: usize) -> String {
        join(&self.dir(i / NAMES.len()), NAMES[i % NAMES.len()])
    }

    /// Is `path` ignored by an ignore file in a folder above it? Only the
    /// patterns in [`PATTERNS`] occur: a basename glob, or a folder name.
    fn ignored(&self, path: &str) -> bool {
        self.files.iter().any(|(ignore, content)| {
            let (dir, name) = split(ignore);
            if !IGNORE_FILES.contains(&name) {
                return false;
            }
            let Some(rest) = strip_dir(path, dir) else {
                return false;
            };
            let parts: Vec<&str> = rest.split('/').collect();
            let (folders, base) = parts.split_at(parts.len() - 1);
            String::from_utf8_lossy(content)
                .lines()
                .any(|p| match p.trim() {
                    "*.txt" => base[0].ends_with(".txt"),
                    "b*.py" => base[0].starts_with('b') && base[0].ends_with(".py"),
                    "c.py" => base[0] == "c.py",
                    "sub/" => folders.contains(&"sub"),
                    _ => false,
                })
        })
    }

    /// The files the index should hold.
    fn indexed(&self) -> BTreeSet<String> {
        self.files
            .iter()
            .filter(|(path, content)| {
                content.len() as u64 <= MAX_FILE_SIZE
                    && !content.contains(&0)
                    && !self.ignored(path)
            })
            .map(|(path, _)| path.clone())
            .collect()
    }

    /// Every (file, line) holding `word`, in indexed files.
    fn hits(&self, word: &str) -> BTreeSet<(String, usize)> {
        let indexed = self.indexed();
        let mut out = BTreeSet::new();
        for path in &indexed {
            let text = String::from_utf8_lossy(&self.files[path]);
            for (i, line) in text.split('\n').enumerate() {
                if line.contains(word) {
                    out.insert((path.clone(), i + 1));
                }
            }
        }
        out
    }
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

fn split(path: &str) -> (&str, &str) {
    path.rsplit_once('/').unwrap_or(("", path))
}

/// `path` relative to folder `dir` ("" is the root), if it is under it.
fn strip_dir<'a>(path: &'a str, dir: &str) -> Option<&'a str> {
    if dir.is_empty() {
        Some(path)
    } else {
        path.strip_prefix(dir)?.strip_prefix('/')
    }
}

struct World {
    _temp: tempfile::TempDir,
    root: PathBuf,
    index_path: PathBuf,
    saved: bool,
    config: IndexerConfig,
    engine: Arc<RwLock<SearchEngine>>,
    model: Model,
}

impl World {
    fn new() -> Self {
        // Folder stamps are trusted at once, so restarts read only the
        // folders that changed (the path a real restart takes); `start`
        // waits out the file system's clock tick instead.
        fast_code_search::search::TEST_WALK_DELAY_NS
            .store(10_000_000_000, std::sync::atomic::Ordering::Relaxed);
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().canonicalize().unwrap();
        let root = base.join("tree");
        std::fs::create_dir_all(&root).unwrap();
        let index_path = base.join("index.fcsidx");
        let config = IndexerConfig {
            paths: vec![root.to_string_lossy().to_string()],
            max_file_size: MAX_FILE_SIZE,
            show_root_name: false,
            watch: false,
            ..Default::default()
        };
        let mut world = Self {
            _temp: temp,
            root,
            index_path,
            saved: false,
            config,
            engine: Arc::new(RwLock::new(SearchEngine::new())),
            model: Model::default(),
        };
        world.start(false);
        world
    }

    /// Build the index, from the saved one when `load` (load + reconcile).
    fn start(&mut self, load: bool) {
        let mut config = self.config.clone();
        if load {
            config.index_path = Some(self.index_path.to_string_lossy().to_string());
            config.save_after_build = false;
        }
        self.engine = Arc::new(RwLock::new(SearchEngine::new()));
        run_background_indexer(BackgroundIndexerConfig {
            indexer_config: config,
            engine: self.engine.clone(),
            progress: Arc::new(RwLock::new(IndexingProgress::default())),
            progress_tx: create_progress_broadcaster(),
            shutdown: Arc::new(AtomicBool::new(false)),
        });
        // A folder changed in the same clock tick as the walk would keep
        // its time (see `TEST_WALK_DELAY_NS` above).
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    fn abs(&self, rel: &str) -> PathBuf {
        if rel.is_empty() {
            self.root.clone()
        } else {
            self.root.join(rel)
        }
    }

    fn write(&mut self, rel: &str, content: Vec<u8>) {
        let path = self.abs(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &content).unwrap();
        self.model.files.insert(rel.to_string(), content);
    }

    /// Apply `op` to the disk and the model; the events a watcher would
    /// report.
    fn perform(&mut self, op: &Op) -> Vec<FileChange> {
        match op {
            Op::Write { file, lines } => {
                let rel = self.model.file(*file);
                let text: String = lines
                    .iter()
                    .map(|l| l.iter().map(|&w| WORDS[w]).collect::<Vec<_>>().join(" ") + "\n")
                    .collect();
                self.write(&rel, text.into_bytes());
                vec![FileChange::Modified(self.abs(&rel))]
            }
            Op::Append { file, word } => {
                let rel = self.model.file(*file);
                let mut content = self.model.files.get(&rel).cloned().unwrap_or_default();
                content.extend_from_slice(format!("{}\n", WORDS[*word]).as_bytes());
                self.write(&rel, content);
                vec![FileChange::Modified(self.abs(&rel))]
            }
            Op::Truncate { file } => {
                let rel = self.model.file(*file);
                if !self.model.files.contains_key(&rel) {
                    return Vec::new();
                }
                self.write(&rel, Vec::new());
                vec![FileChange::Modified(self.abs(&rel))]
            }
            Op::Binary { file } => {
                let rel = self.model.file(*file);
                self.write(&rel, vec![0u8, 1, 2, 0, b'a', b'm', b'b', b'e', b'r']);
                vec![FileChange::Modified(self.abs(&rel))]
            }
            Op::Oversize { file } => {
                let rel = self.model.file(*file);
                let line = "amber basalt cobalt\n";
                let text = line.repeat(MAX_FILE_SIZE as usize / line.len() + 2);
                self.write(&rel, text.into_bytes());
                vec![FileChange::Modified(self.abs(&rel))]
            }
            Op::Delete { file } => {
                let rel = self.model.file(*file);
                if self.model.files.remove(&rel).is_none() {
                    return Vec::new();
                }
                std::fs::remove_file(self.abs(&rel)).unwrap();
                vec![FileChange::Deleted(self.abs(&rel))]
            }
            Op::Rename { from, to } => {
                let (from, to) = (self.model.file(*from), self.model.file(*to));
                if from == to || !self.model.files.contains_key(&from) {
                    return Vec::new();
                }
                let content = self.model.files.remove(&from).unwrap();
                std::fs::create_dir_all(self.abs(&to).parent().unwrap()).unwrap();
                std::fs::rename(self.abs(&from), self.abs(&to)).unwrap();
                self.model.files.insert(to.clone(), content);
                vec![FileChange::Renamed {
                    from: self.abs(&from),
                    to: self.abs(&to),
                }]
            }
            Op::RenameDir { dir } => {
                let name = DIRS[*dir];
                let current = self.model.dir(*dir);
                let renamed_back = self.model.moved.contains(name);
                if renamed_back {
                    self.model.moved.remove(name);
                } else {
                    self.model.moved.insert(name);
                }
                let target = self.model.dir(*dir);
                if !self.abs(&current).is_dir() || self.abs(&target).exists() {
                    // Nothing to move (undo the bookkeeping).
                    if renamed_back {
                        self.model.moved.insert(name);
                    } else {
                        self.model.moved.remove(name);
                    }
                    return Vec::new();
                }
                std::fs::rename(self.abs(&current), self.abs(&target)).unwrap();
                let moved: Vec<(String, Vec<u8>)> = self
                    .model
                    .files
                    .iter()
                    .filter_map(|(p, c)| {
                        strip_dir(p, &current).map(|rest| (join(&target, rest), c.clone()))
                    })
                    .collect();
                self.model
                    .files
                    .retain(|p, _| strip_dir(p, &current).is_none());
                self.model.files.extend(moved);
                vec![FileChange::Renamed {
                    from: self.abs(&current),
                    to: self.abs(&target),
                }]
            }
            Op::DeleteDir { dir } => {
                let current = self.model.dir(*dir);
                let path = self.abs(&current);
                if !path.exists() {
                    return Vec::new();
                }
                std::fs::remove_dir_all(&path).unwrap();
                self.model
                    .files
                    .retain(|p, _| strip_dir(p, &current).is_none());
                vec![FileChange::Deleted(path)]
            }
            Op::Ignore { dir, kind, pattern } => {
                let rel = join(&self.model.dir(*dir), IGNORE_FILES[*kind]);
                match pattern {
                    Some(p) => {
                        self.write(&rel, format!("{}\n", PATTERNS[*p]).into_bytes());
                        vec![FileChange::Modified(self.abs(&rel))]
                    }
                    None => {
                        if self.model.files.remove(&rel).is_none() {
                            return Vec::new();
                        }
                        std::fs::remove_file(self.abs(&rel)).unwrap();
                        vec![FileChange::Deleted(self.abs(&rel))]
                    }
                }
            }
            Op::Lost(inner) => {
                self.perform(inner);
                vec![FileChange::Modified(self.root.clone())]
            }
            Op::Save => {
                let engine = self.engine.read().unwrap();
                engine
                    .save_index(&self.index_path, &self.config)
                    .expect("save");
                self.saved = true;
                Vec::new()
            }
            Op::Restart => {
                self.perform(&Op::Save);
                self.start(true);
                Vec::new()
            }
            Op::Crash => {
                let saved = self.saved;
                self.start(saved);
                Vec::new()
            }
        }
    }

    fn step(&mut self, op: &Op) {
        let changes = self.perform(op);
        if !changes.is_empty() {
            watch_loop::apply_burst(&self.engine, &changes, &self.config)
                .expect("applying the changes panicked");
        }
    }

    /// The index's files, relative to the root.
    fn indexed(&self) -> BTreeSet<String> {
        let engine = self.engine.read().unwrap();
        engine
            .file_ids_under(&self.root)
            .into_iter()
            .filter_map(|id| engine.get_file_path(id))
            .collect()
    }

    /// Every hit for `word` through the front ends' search path, page by
    /// page, checking the pages as it goes.
    fn hits(&self, word: &str) -> Result<BTreeSet<(String, usize)>, String> {
        let engine = self.engine.read().unwrap();
        let settings = SearchSettings::default();
        let mut all: Vec<(String, usize)> = Vec::new();
        let mut offset = 0;
        loop {
            let req = SearchRequest {
                query: word.to_string(),
                mode: SearchMode::Text,
                max_results: 7,
                offset,
                ..Default::default()
            };
            let limits = req.limits(&settings, None)?;
            let out = engine.run(&req, limits)?;
            if !out.info.limits_hit.is_empty() {
                return Err(format!(
                    "limits hit on a tiny tree: {:?}",
                    out.info.limits_hit
                ));
            }
            let n = out.matches.len();
            all.extend(
                out.matches
                    .iter()
                    .filter(|m| m.line_number > 0)
                    .map(|m| (m.file_path.clone(), m.line_number)),
            );
            if !out.has_more(offset) || n == 0 {
                if let Some(total) = out.info.total_matches {
                    if total != offset + n {
                        return Err(format!("total {total} but pages held {}", offset + n));
                    }
                }
                break;
            }
            offset += n;
        }
        // Each file's hits side by side, and no hit twice.
        let mut seen_files: Vec<&str> = Vec::new();
        for (file, _) in &all {
            if seen_files.last() != Some(&file.as_str()) {
                if seen_files.contains(&file.as_str()) {
                    return Err(format!("hits of {file} are not together: {all:?}"));
                }
                seen_files.push(file);
            }
        }
        let set: BTreeSet<_> = all.iter().cloned().collect();
        if set.len() != all.len() {
            return Err(format!("a hit came twice: {all:?}"));
        }
        Ok(set)
    }

    fn check(&self) -> Result<(), String> {
        let want = self.model.indexed();
        let got = self.indexed();
        if want != got {
            return Err(format!(
                "indexed files differ\n  missing: {:?}\n  extra: {:?}",
                want.difference(&got).collect::<Vec<_>>(),
                got.difference(&want).collect::<Vec<_>>()
            ));
        }
        for word in WORDS {
            let want = self.model.hits(word);
            let got = self.hits(word)?;
            if want != got {
                return Err(format!(
                    "hits for {word:?} differ\n  missing: {:?}\n  extra: {:?}",
                    want.difference(&got).collect::<Vec<_>>(),
                    got.difference(&want).collect::<Vec<_>>()
                ));
            }
        }
        let engine = self.engine.read().unwrap();
        let report = engine.check_invariants();
        if !report.is_clean() {
            return Err(report.to_string());
        }
        for id in engine.live_file_ids() {
            let check = engine.check_file(id);
            if check != Some(FileCheck::Current) {
                return Err(format!("{:?}: {check:?}", engine.get_file_path(id)));
            }
        }
        Ok(())
    }
}

/// Apply `ops` one by one, checking the index against the model after
/// each.
fn replay(ops: &[Op]) -> Result<(), String> {
    let mut world = World::new();
    for (i, op) in ops.iter().enumerate() {
        world.step(op);
        world
            .check()
            .map_err(|e| format!("after step {} ({:?}):\n{}", i + 1, op, e))?;
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: std::env::var("PROPTEST_CASES").ok().and_then(|c| c.parse().ok()).unwrap_or(48),
        max_shrink_iters: 2_000,
        // Failures are kept as the named tests below, not in a seed file.
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    #[test]
    fn index_follows_the_disk(ops in prop::collection::vec(op(), 1..40)) {
        if let Err(e) = replay(&ops) {
            prop_assert!(false, "{}", e);
        }
    }
}

// Sequences this test found, kept as they were shrunk.

/// `sub/` in the root `.gitignore`, then a `.gitignore` written inside
/// `pkg/sub`: the watcher reconciled the folder and indexed it (the walk
/// never checked whether the folder it starts from is ignored).
#[test]
fn folder_ignored_by_its_parent_stays_out_when_an_ignore_file_is_written_in_it() {
    replay(&[
        Op::Ignore {
            dir: 0,
            kind: 0,
            pattern: Some(2),
        },
        Op::Ignore {
            dir: 2,
            kind: 0,
            pattern: Some(0),
        },
    ])
    .unwrap();
}

/// A file saved in the index, then ignored and rewritten before a restart:
/// the restart re-indexed it as a changed file without checking the
/// ignore rules.
#[test]
fn changed_file_that_is_now_ignored_stays_out_after_a_restart() {
    replay(&[
        Op::Write {
            file: 14,
            lines: vec![vec![0]],
        },
        Op::Save,
        Op::Ignore {
            dir: 0,
            kind: 0,
            pattern: Some(2),
        },
        Op::Write {
            file: 14,
            lines: vec![vec![0]],
        },
        Op::Crash,
    ])
    .unwrap();
}

/// A file skipped for being too large is not in the load's stat pass, and
/// cutting it down does not change its folder's time: the restart walk must
/// still read the folder (it once skipped it, so the file never came back).
#[test]
fn file_cut_down_from_too_large_is_indexed_after_a_restart() {
    replay(&[
        Op::Lost(Box::new(Op::Oversize { file: 19 })),
        Op::Restart,
        Op::Save,
        Op::Write {
            file: 19,
            lines: vec![vec![0]],
        },
        Op::Crash,
    ])
    .unwrap();
}
