//! Watch harness: does the live index converge to what a fresh index of the
//! same disk would hold?
//!
//! Each test builds a small Python + TypeScript tree in a temp directory,
//! indexes it with the real background indexer, runs the real watch loop
//! (`search::watch_loop::run`, the code the server runs), changes files the
//! way people and tools do (edits, atomic saves, renames, directory moves,
//! deletes, branch-switch bursts), and then compares the live engine with an
//! oracle: a fresh engine built from the final disk by the same indexer.
//!
//! The comparison covers what users see: the indexed file set, every hit for
//! every vocabulary word, symbol definitions and dependency-graph edges. A
//! failure prints the difference and the operations that produced it, and
//! random sequences print their seed so they can be replayed with
//! `FCS_WATCH_SEED=<seed>`.

use fast_code_search::config::IndexerConfig;
use fast_code_search::search::{
    create_progress_broadcaster, engine_lock, run_background_indexer, watch_loop,
    BackgroundIndexerConfig, FileChange, IndexingProgress, SearchEngine, WatchLoopConfig,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// Words the generated files are made of; every one is searched for.
const VOCAB: &[&str] = &[
    "amber", "basalt", "cobalt", "dune", "ember", "fjord", "garnet", "harbor", "indigo", "jasper",
    "kelp", "lagoon", "marble", "nectar", "onyx", "pebble", "quartz", "russet", "sienna", "tundra",
];

/// How long a change may take to show up in the live index. Generous: the
/// macOS backend can deliver events seconds late.
const CONVERGE_TIMEOUT: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// Deterministic randomness (no extra dependency).

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    fn next(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn word(&mut self) -> &'static str {
        VOCAB[self.below(VOCAB.len())]
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        (!items.is_empty()).then(|| &items[self.below(items.len())])
    }
}

// ---------------------------------------------------------------------------
// File contents. Every file defines a symbol, holds a few vocabulary words
// and imports a sibling, so text, symbols and the graph are all exercised.

fn python_source(rng: &mut Rng, stem: &str, sibling: Option<&str>) -> String {
    let mut s = String::new();
    if let Some(sib) = sibling {
        s.push_str(&format!("from .{sib} import thing\n"));
    }
    s.push_str(&format!(
        "\ndef fn_{stem}_{}():\n    return \"{} {}\"\n",
        rng.word(),
        rng.word(),
        rng.word()
    ));
    s
}

fn ts_source(rng: &mut Rng, stem: &str, sibling: Option<&str>) -> String {
    let mut s = String::new();
    if let Some(sib) = sibling {
        s.push_str(&format!("import {{ thing }} from \"./{sib}\";\n"));
    }
    s.push_str(&format!(
        "\nexport function fn_{stem}_{}() {{\n  return \"{} {}\";\n}}\n",
        rng.word(),
        rng.word(),
        rng.word()
    ));
    s
}

/// Source for `path`, by extension, importing `sibling` (a stem in the same
/// directory) when given.
fn source_for(rng: &mut Rng, path: &Path, sibling: Option<&str>) -> String {
    let stem = path
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .replace('.', "_");
    match path.extension().and_then(|e| e.to_str()) {
        Some("py") => python_source(rng, &stem, sibling),
        _ => ts_source(rng, &stem, sibling),
    }
}

// ---------------------------------------------------------------------------
// Snapshot: what a user can observe about an index.

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    files: BTreeSet<String>,
    hits: BTreeSet<(String, String, usize)>,
    symbols: BTreeSet<(String, String, usize)>,
    edges: BTreeSet<(String, String)>,
}

fn snapshot(engine: &SearchEngine, root: &Path) -> Snapshot {
    let ids = engine.file_ids_under(root);
    let path_of = |id: u32| engine.get_file_path(id).unwrap_or_else(|| format!("#{id}"));
    let files = ids.iter().map(|&id| path_of(id)).collect();
    let mut hits = BTreeSet::new();
    let mut symbols = BTreeSet::new();
    for word in VOCAB {
        for m in engine.search(word, 1_000_000) {
            hits.insert((word.to_string(), m.file_path, m.line_number));
        }
        for m in engine
            .search_symbols(word, "", "", 1_000_000)
            .expect("symbol search")
        {
            symbols.insert((word.to_string(), m.file_path, m.line_number));
        }
    }
    let mut edges = BTreeSet::new();
    for &id in &ids {
        for dep in engine.get_dependencies(id) {
            edges.insert((path_of(id), path_of(dep)));
        }
    }
    Snapshot {
        files,
        hits,
        symbols,
        edges,
    }
}

/// A readable account of how `live` differs from `want`.
fn describe_diff(live: &Snapshot, want: &Snapshot) -> String {
    fn side<T: std::fmt::Debug + Ord>(
        name: &str,
        live: &BTreeSet<T>,
        want: &BTreeSet<T>,
    ) -> String {
        let extra: Vec<_> = live.difference(want).take(10).collect();
        let missing: Vec<_> = want.difference(live).take(10).collect();
        if extra.is_empty() && missing.is_empty() {
            return String::new();
        }
        format!("  {name}: live has extra {extra:?}\n  {name}: live is missing {missing:?}\n")
    }
    side("files", &live.files, &want.files)
        + &side("hits", &live.hits, &want.hits)
        + &side("symbols", &live.symbols, &want.symbols)
        + &side("edges", &live.edges, &want.edges)
}

// ---------------------------------------------------------------------------
// The harness.

/// Run a directory operation, retrying briefly when Windows refuses it
/// because the indexer or watcher has a file under it open (a sharing
/// violation, "Access is denied"), as editors and git do.
fn retry_sharing(mut op: impl FnMut() -> std::io::Result<()>) {
    for attempt in 0.. {
        match op() {
            Ok(()) => return,
            Err(e)
                if cfg!(windows)
                    && e.kind() == std::io::ErrorKind::PermissionDenied
                    && attempt < 50 =>
            {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err(e) => panic!("{e:?}"),
        }
    }
}

fn indexer_config(root: &Path) -> IndexerConfig {
    IndexerConfig {
        paths: vec![root.to_string_lossy().to_string()],
        // Small batches so a modest tree still spans many batches, which is
        // where changes made during the initial build can interleave.
        batch_size: 16,
        ..Default::default()
    }
}

fn build_index(config: &IndexerConfig, engine: Arc<RwLock<SearchEngine>>) {
    run_background_indexer(BackgroundIndexerConfig {
        indexer_config: config.clone(),
        engine,
        progress: Arc::new(RwLock::new(IndexingProgress::default())),
        progress_tx: create_progress_broadcaster(),
        shutdown: Arc::new(AtomicBool::new(false)),
    });
}

struct Harness {
    _temp: TempDir,
    root: PathBuf,
    config: IndexerConfig,
    engine: Arc<RwLock<SearchEngine>>,
    shutdown: Arc<AtomicBool>,
    watch: Option<JoinHandle<()>>,
    indexer: Option<JoinHandle<()>>,
    rng: Rng,
    log: Vec<String>,
}

impl Harness {
    /// A tree of `files` generated files under a few directories.
    fn new(seed: u64, files: usize) -> Self {
        let temp = TempDir::new().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut h = Self {
            config: indexer_config(&root),
            _temp: temp,
            root,
            engine: Arc::new(RwLock::new(SearchEngine::new())),
            shutdown: Arc::new(AtomicBool::new(false)),
            watch: None,
            indexer: None,
            rng: Rng::new(seed),
            log: Vec::new(),
        };
        for i in 0..files {
            let dir = ["app", "app/core", "web/src", "web/src/ui"][i % 4];
            let ext = if dir.starts_with("app") { "py" } else { "ts" };
            let rel = format!("{dir}/m{i}.{ext}");
            let sibling = (i >= 4).then(|| format!("m{}", i - 4));
            h.write_quiet(&rel, sibling.as_deref());
        }
        for pkg in ["app", "app/core"] {
            std::fs::write(h.root.join(pkg).join("__init__.py"), "").unwrap();
        }
        h
    }

    /// Start the watch loop; with `index_concurrently` the initial build runs
    /// at the same time (as in the server) instead of before it.
    fn start(&mut self, index_concurrently: bool) {
        if !index_concurrently {
            build_index(&self.config, self.engine.clone());
        }
        let watcher = watch_loop::start_watcher(&self.config).expect("watcher");
        let cfg = WatchLoopConfig {
            indexer: self.config.clone(),
            engine: self.engine.clone(),
            shutdown: self.shutdown.clone(),
            batch_window: watch_loop::DEFAULT_BATCH_WINDOW,
        };
        self.watch = Some(std::thread::spawn(move || watch_loop::run(watcher, cfg)));
        if index_concurrently {
            let (config, engine) = (self.config.clone(), self.engine.clone());
            self.indexer = Some(std::thread::spawn(move || build_index(&config, engine)));
        }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    /// Write generated content to `rel`, importing a sibling file when one
    /// exists, without logging (used for the initial tree).
    fn write_quiet(&mut self, rel: &str, sibling: Option<&str>) {
        let path = self.path(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let text = source_for(&mut self.rng, &path, sibling);
        std::fs::write(&path, text).unwrap();
    }

    fn note(&mut self, what: String) {
        self.log.push(what);
    }

    /// Indexable files currently on disk, relative to the root.
    fn files(&self) -> Vec<String> {
        let mut out: Vec<String> = walk(&self.root)
            .into_iter()
            .filter(|p| p.ends_with(".py") || p.ends_with(".ts"))
            .filter(|p| !p.contains("node_modules/") && !p.ends_with("__init__.py"))
            .collect();
        out.sort();
        out
    }

    fn dirs(&self) -> Vec<String> {
        let mut out: Vec<String> = walk_dirs(&self.root)
            .into_iter()
            .filter(|d| !d.contains("node_modules"))
            .collect();
        out.sort();
        out
    }

    /// A sibling stem for an import from `rel`, if any file sits next to it.
    fn sibling_of(&self, rel: &str) -> Option<String> {
        let dir = Path::new(rel).parent()?.to_path_buf();
        self.files()
            .into_iter()
            .map(PathBuf::from)
            .find(|p| p.parent() == Some(dir.as_path()) && p.to_string_lossy() != rel)
            .map(|p| p.file_stem().unwrap().to_string_lossy().to_string())
    }

    // --- operations people and tools perform ---

    fn edit(&mut self, rel: &str) {
        let sib = self.sibling_of(rel);
        self.write_quiet(rel, sib.as_deref());
        self.note(format!("edit {rel}"));
    }

    fn create(&mut self, rel: &str) {
        let sib = self.sibling_of(rel);
        self.write_quiet(rel, sib.as_deref());
        self.note(format!("create {rel}"));
    }

    /// Editor-style save: write a temp file next to it, rename over it.
    fn atomic_save(&mut self, rel: &str) {
        let path = self.path(rel);
        let tmp = path.with_file_name(format!(
            ".{}.swp",
            path.file_name().unwrap().to_string_lossy()
        ));
        let sib = self.sibling_of(rel);
        let text = source_for(&mut self.rng, &path, sib.as_deref());
        std::fs::write(&tmp, text).unwrap();
        std::fs::rename(&tmp, &path).unwrap();
        self.note(format!("atomic save {rel}"));
    }

    fn delete(&mut self, rel: &str) {
        std::fs::remove_file(self.path(rel)).unwrap();
        self.note(format!("delete {rel}"));
    }

    fn rename(&mut self, from: &str, to: &str) {
        std::fs::create_dir_all(self.path(to).parent().unwrap()).unwrap();
        std::fs::rename(self.path(from), self.path(to)).unwrap();
        self.note(format!("rename {from} -> {to}"));
    }

    fn rename_dir(&mut self, from: &str, to: &str) {
        let (from_path, to_path) = (self.path(from), self.path(to));
        retry_sharing(|| std::fs::rename(&from_path, &to_path));
        self.note(format!("rename dir {from} -> {to}"));
    }

    fn delete_dir(&mut self, rel: &str) {
        let path = self.path(rel);
        retry_sharing(|| std::fs::remove_dir_all(&path));
        self.note(format!("delete dir {rel}"));
    }

    /// Write (or, with `None`, delete) the ignore file `name` in directory
    /// `dir`, as a branch switch or a hand edit does.
    fn set_ignore(&mut self, dir: &str, name: &str, rules: Option<&str>) {
        let path = self.path(&join(dir, name));
        match rules {
            Some(r) => {
                std::fs::write(&path, r).unwrap();
                self.note(format!("write {dir}/{name}: {r:?}"));
            }
            None => {
                let _ = std::fs::remove_file(&path);
                self.note(format!("delete {dir}/{name}"));
            }
        }
    }

    fn mkdir_with_files(&mut self, rel: &str, n: usize, ext: &str) {
        for i in 0..n {
            let file = format!("{rel}/n{i}.{ext}");
            let sib = (i > 0).then(|| "n0".to_string());
            self.write_quiet(&file, sib.as_deref());
        }
        self.note(format!("mkdir {rel} with {n} .{ext} files"));
    }

    /// One random operation; returns false if nothing applicable existed.
    fn random_op(&mut self, counter: &mut usize) -> bool {
        *counter += 1;
        let n = *counter;
        let files = self.files();
        let dirs = self.dirs();
        match self.rng.below(11) {
            0 | 1 => match self.rng.pick(&files).cloned() {
                Some(f) => self.edit(&f),
                None => return false,
            },
            2 => match self.rng.pick(&files).cloned() {
                Some(f) => self.atomic_save(&f),
                None => return false,
            },
            3 => {
                let dir = self.rng.pick(&dirs).cloned().unwrap_or_default();
                let ext = if self.rng.below(2) == 0 { "py" } else { "ts" };
                let rel = join(&dir, &format!("c{n}.{ext}"));
                self.create(&rel);
            }
            4 => match self.rng.pick(&files).cloned() {
                Some(f) => self.delete(&f),
                None => return false,
            },
            5 => {
                let (Some(f), Some(d)) = (
                    self.rng.pick(&files).cloned(),
                    self.rng.pick(&dirs).cloned(),
                ) else {
                    return false;
                };
                let ext = Path::new(&f)
                    .extension()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                self.rename(&f, &join(&d, &format!("r{n}.{ext}")));
            }
            6 => {
                // Out of the index: a node_modules directory is excluded.
                let Some(f) = self.rng.pick(&files).cloned() else {
                    return false;
                };
                let name = Path::new(&f)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                self.rename(&f, &format!("node_modules/pkg{n}/{name}"));
            }
            7 => {
                let parent = self.rng.pick(&dirs).cloned().unwrap_or_default();
                let ext = if self.rng.below(2) == 0 { "py" } else { "ts" };
                self.mkdir_with_files(&join(&parent, &format!("d{n}")), 3, ext);
            }
            8 => {
                let candidates: Vec<String> =
                    dirs.iter().filter(|d| !d.is_empty()).cloned().collect();
                let Some(d) = self.rng.pick(&candidates).cloned() else {
                    return false;
                };
                let to = join(
                    Path::new(&d)
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .as_deref()
                        .unwrap_or(""),
                    &format!("moved{n}"),
                );
                self.rename_dir(&d, &to);
            }
            10 => {
                // Ignore a file or a whole kind of file in a directory, or
                // stop ignoring them.
                let dir = self.rng.pick(&dirs).cloned().unwrap_or_default();
                let name = [".gitignore", ".ignore", ".rgignore"][self.rng.below(3)];
                if self.path(&join(&dir, name)).exists() {
                    self.set_ignore(&dir, name, None);
                } else {
                    let in_dir: Vec<String> = files
                        .iter()
                        .filter(|f| Path::new(f).parent() == Some(Path::new(&dir)))
                        .map(|f| {
                            Path::new(f)
                                .file_name()
                                .unwrap()
                                .to_string_lossy()
                                .to_string()
                        })
                        .collect();
                    let rule = match self.rng.pick(&in_dir) {
                        Some(f) if self.rng.below(2) == 0 => format!("{f}\n"),
                        _ => "*.ts\n".to_string(),
                    };
                    self.set_ignore(&dir, name, Some(&rule));
                }
            }
            _ => {
                let candidates: Vec<String> = dirs
                    .iter()
                    .filter(|d| d.matches('/').count() >= 1)
                    .cloned()
                    .collect();
                let Some(d) = self.rng.pick(&candidates).cloned() else {
                    return false;
                };
                self.delete_dir(&d);
            }
        }
        true
    }

    /// Wait until the live index matches a fresh index of the disk.
    fn assert_converges(&mut self, context: &str) {
        if let Some(h) = self.indexer.take() {
            h.join().unwrap();
        }
        let oracle = Arc::new(RwLock::new(SearchEngine::new()));
        build_index(&self.config, oracle.clone());
        let want = snapshot(&oracle.read().unwrap(), &self.root);
        assert!(!want.files.is_empty(), "{context}: oracle indexed nothing");

        let deadline = Instant::now() + CONVERGE_TIMEOUT;
        loop {
            let live = snapshot(&self.engine.read().unwrap(), &self.root);
            if live == want {
                return;
            }
            if Instant::now() > deadline {
                panic!(
                    "{context}: live index did not converge to a fresh index within {:?}\n{}\noperations:\n  {}",
                    CONVERGE_TIMEOUT,
                    describe_diff(&live, &want),
                    self.log.join("\n  ")
                );
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Some(h) = self.indexer.take() {
            let _ = h.join();
        }
        if let Some(h) = self.watch.take() {
            let _ = h.join();
        }
    }
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

/// Every file under `root`, relative and with `/` separators.
fn walk(root: &Path) -> Vec<String> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .map(|e| rel_path(root, e.path()))
        .collect()
}

/// Every directory under `root` (root itself as ""), relative.
fn walk_dirs(root: &Path) -> Vec<String> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_dir())
        .map(|e| rel_path(root, e.path()))
        .collect()
}

fn rel_path(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

fn seed_from_env(default: u64) -> u64 {
    std::env::var("FCS_WATCH_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

// ---------------------------------------------------------------------------
// Scenarios.

/// Each everyday operation on its own, checked after every step.
#[test]
fn watch_everyday_operations_converge() {
    let mut h = Harness::new(1, 12);
    h.start(false);
    h.assert_converges("initial build");

    h.edit("app/m0.py");
    h.assert_converges("edit");
    h.atomic_save("web/src/m2.ts");
    h.assert_converges("atomic save");
    h.create("app/core/fresh.py");
    h.assert_converges("create");
    h.rename("app/m4.py", "app/core/m4_moved.py");
    h.assert_converges("rename across directories");
    h.delete("web/src/ui/m3.ts");
    h.assert_converges("delete");
    h.mkdir_with_files("web/src/feature", 3, "ts");
    h.assert_converges("new directory with files");
    h.rename_dir("web/src/feature", "web/src/feature2");
    h.assert_converges("rename directory");
    h.rename("app/m8.py", "node_modules/pkg/m8.py");
    h.assert_converges("move into excluded directory");
    h.rename("app/core/m5.py", "app/core/m5.py.bak");
    h.assert_converges("rename to an extension that is not indexed");
    h.delete_dir("web/src/ui");
    h.assert_converges("delete directory");
    // Moving a file into a directory and deleting the directory straight
    // away used to reach the index as a lone delete of the directory: the
    // file's old name stayed indexed forever.
    h.rename("app/core/m9.py", "web/src/m6_dir/m9.py");
    h.delete_dir("web/src/m6_dir");
    h.assert_converges("move a file into a directory, then delete the directory");
    // Ignore files apply as soon as they change, as they would after a
    // restart: newly ignored files leave the index, allowed ones return.
    h.set_ignore("app", ".gitignore", Some("m1.py\n"));
    h.assert_converges("ignore a file in .gitignore");
    h.set_ignore("web", ".rgignore", Some("src/ui/\n"));
    h.assert_converges("ignore a directory in .rgignore");
    h.set_ignore("app", ".gitignore", Some("m3.py\n"));
    h.assert_converges("edit .gitignore to ignore another file");
    h.set_ignore("web", ".rgignore", None);
    h.set_ignore("app", ".gitignore", None);
    h.assert_converges("delete the ignore files");
    // app/m4.py imports m0: the edge must go with the file and come back
    // with it.
    h.delete("app/m0.py");
    h.assert_converges("delete an imported file");
    h.create("app/m0.py");
    h.assert_converges("recreate an imported file");
}

/// A burst of changes with no pause between them, like a branch switch or
/// a formatter run.
#[test]
fn watch_burst_converges() {
    let seed = seed_from_env(7);
    let mut h = Harness::new(seed, 40);
    h.start(false);
    h.assert_converges("initial build");
    let mut n = 0;
    for _ in 0..60 {
        h.random_op(&mut n);
    }
    h.assert_converges(&format!("burst of 60 random operations (seed {seed})"));
}

/// Several rounds of random operations with time to settle in between.
#[test]
fn watch_random_sequences_converge() {
    let base = seed_from_env(1234);
    for round in 0..3u64 {
        let seed = base + round;
        let mut h = Harness::new(seed, 24);
        h.start(false);
        let mut n = 0;
        for step in 0..4 {
            for _ in 0..5 {
                h.random_op(&mut n);
            }
            std::thread::sleep(Duration::from_millis(300));
            h.assert_converges(&format!("seed {seed}, step {step}"));
        }
    }
}

/// Files change while the initial index is still being built, as happens
/// when the server starts on a tree someone is working in.
#[test]
fn watch_changes_during_initial_build_converge() {
    let seed = seed_from_env(99);
    let mut h = Harness::new(seed, 400);
    h.start(true);
    let mut n = 0;
    for _ in 0..80 {
        h.random_op(&mut n);
    }
    h.assert_converges(&format!("changes during the initial build (seed {seed})"));
}

// ---------------------------------------------------------------------------
// Searches during updates. A batch used to be applied under one write lock,
// file reads and directory scans included, and a search that met the lock
// was turned away ("index is being updated"); on a large tree with changes
// streaming in, that was most searches.

/// Rewrite every file of a [`Harness::new`] tree of `files` files, keeping
/// each file's import, and return their paths.
fn rewrite_all(h: &mut Harness, files: usize) -> Vec<PathBuf> {
    (0..files)
        .map(|i| {
            let dir = ["app", "app/core", "web/src", "web/src/ui"][i % 4];
            let ext = if dir.starts_with("app") { "py" } else { "ts" };
            let rel = format!("{dir}/m{i}.{ext}");
            let sibling = (i >= 4).then(|| format!("m{}", i - 4));
            h.write_quiet(&rel, sibling.as_deref());
            h.path(&rel)
        })
        .collect()
}

/// A burst far bigger than one install step is installed in steps, and no
/// step holds the write lock for more than a fraction of the burst: the
/// planning and the file reads happen outside it. The result still matches
/// a fresh index.
#[test]
fn burst_holds_the_write_lock_in_short_steps() {
    const FILES: usize = 2400;
    let mut h = Harness::new(seed_from_env(31), FILES);
    build_index(&h.config, h.engine.clone());
    let changed = rewrite_all(&mut h, FILES);
    let changes: Vec<FileChange> = changed.into_iter().map(FileChange::Modified).collect();

    let report = watch_loop::apply_burst(&h.engine, &changes, &h.config).expect("burst applied");

    assert_eq!(report.outcome.indexed, FILES);
    assert!(
        report.steps >= FILES.div_ceil(watch_loop::INSTALL_CHUNK),
        "expected the burst in steps of at most {} files: {report:?}",
        watch_loop::INSTALL_CHUNK
    );
    assert!(
        report.longest_hold * 3 < report.elapsed,
        "one write-lock hold took most of the burst, so searches were held \
         back for all of it: {report:?}"
    );
    h.note(format!("rewrite all {FILES} files in one burst"));
    h.assert_converges("burst applied in steps");
}

/// A step that had to wait for searches to finish stays in longer, so a
/// backlog drains while people search. With one chunk per step the watcher
/// paid a whole search's wait for every 512 files and fell minutes behind
/// a large change on a big tree (see `watch_loop::MAX_HOLD`).
#[test]
fn a_step_that_waited_for_searches_installs_more() {
    const FILES: usize = 2400;
    let mut h = Harness::new(seed_from_env(43), FILES);
    build_index(&h.config, h.engine.clone());
    let changed = rewrite_all(&mut h, FILES);
    let changes: Vec<FileChange> = changed.into_iter().map(FileChange::Modified).collect();

    // A long search holds the lock until the burst is planned, and for a
    // while after, so the first step waits well past MAX_HOLD.
    let search = h.engine.read().unwrap();
    let burst = {
        let (engine, config) = (h.engine.clone(), h.config.clone());
        std::thread::spawn(move || watch_loop::apply_burst(&engine, &changes, &config))
    };
    let deadline = Instant::now() + Duration::from_secs(60);
    while search.watch_backlog.pending() < FILES {
        assert!(Instant::now() < deadline, "the burst was never planned");
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(watch_loop::MAX_HOLD * 3);
    drop(search);
    let report = burst.join().unwrap().expect("burst applied");

    assert_eq!(report.outcome.indexed, FILES);
    assert!(
        report.steps < FILES.div_ceil(watch_loop::INSTALL_CHUNK),
        "the step that waited for the search installed one chunk only: {report:?}"
    );
    assert!(
        report.longest_hold < watch_loop::MAX_HOLD * 2,
        "a step held the lock far past MAX_HOLD: {report:?}"
    );
    h.note(format!("rewrite all {FILES} files behind a long search"));
    h.assert_converges("burst applied behind a long search");
}

/// Searches running while the watch loop applies a burst are all answered,
/// none turned away, and the index still converges.
#[test]
fn searches_are_answered_while_a_burst_applies() {
    const FILES: usize = 1200;
    let mut h = Harness::new(seed_from_env(57), FILES);
    h.start(false);
    h.assert_converges("initial build");

    let stop = Arc::new(AtomicBool::new(false));
    let searchers: Vec<_> = (0..2)
        .map(|t| {
            let (engine, stop) = (h.engine.clone(), stop.clone());
            std::thread::spawn(move || {
                let (mut served, mut turned_away) = (0usize, 0usize);
                let mut slowest = Duration::ZERO;
                let mut n = t;
                while !stop.load(Ordering::Relaxed) {
                    n += 1;
                    let started = Instant::now();
                    match engine_lock::read_within(&engine, engine_lock::SEARCH_READ_WAIT) {
                        Some(engine) => {
                            std::hint::black_box(engine.search(VOCAB[n % VOCAB.len()], 20));
                            served += 1;
                            slowest = slowest.max(started.elapsed());
                        }
                        None => turned_away += 1,
                    }
                }
                (served, turned_away, slowest)
            })
        })
        .collect();

    rewrite_all(&mut h, FILES);
    let mut n = 0;
    for _ in 0..40 {
        h.random_op(&mut n);
    }
    h.note(format!(
        "rewrite all {FILES} files, then 40 random operations"
    ));
    h.assert_converges("burst while searching");

    stop.store(true, Ordering::Relaxed);
    for s in searchers {
        let (served, turned_away, slowest) = s.join().unwrap();
        assert!(served > 0, "a searcher never got the lock");
        assert_eq!(
            turned_away, 0,
            "{turned_away} searches were turned away during the burst (slowest answered in {slowest:?})"
        );
    }
}

/// While a burst waits to be installed (here: behind a long search),
/// other searches are still answered, and they report the changes not
/// applied yet instead of passing off the old index as current.
#[test]
fn searches_report_changes_still_to_apply() {
    const FILES: usize = 600;
    let mut h = Harness::new(seed_from_env(73), FILES);
    build_index(&h.config, h.engine.clone());
    h.engine.write().unwrap().watching = true;
    let changed = rewrite_all(&mut h, FILES);
    let changes: Vec<FileChange> = changed.into_iter().map(FileChange::Modified).collect();

    // A long search holds the lock, so no install step can start.
    let slow = h.engine.read().unwrap();
    let burst = {
        let (engine, config) = (h.engine.clone(), h.config.clone());
        std::thread::spawn(move || watch_loop::apply_burst(&engine, &changes, &config))
    };
    let deadline = Instant::now() + Duration::from_secs(60);
    let pending = loop {
        let started = Instant::now();
        let engine = engine_lock::read_within(&h.engine, engine_lock::SEARCH_READ_WAIT)
            .expect("a search is answered while the burst waits");
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "a search waited {:?} behind the waiting burst",
            started.elapsed()
        );
        let pending = engine.freshness().pending_changes;
        if pending == FILES {
            break pending;
        }
        drop(engine);
        assert!(
            Instant::now() < deadline,
            "the burst never got to installing"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    let note = h.engine.read().unwrap().freshness().note(0).unwrap();
    assert!(note.contains(&format!("{pending} file changes")), "{note}");
    drop(slow);

    let report = burst.join().unwrap().expect("burst applied");
    assert_eq!(report.outcome.indexed, FILES);
    let freshness = h.engine.read().unwrap().freshness();
    assert_eq!(freshness.pending_changes, 0);
    assert_eq!(freshness.note(0), None);
    h.note(format!("rewrite all {FILES} files behind a long search"));
    h.assert_converges("burst applied after the long search");
}
