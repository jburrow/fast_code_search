//! Rank order, pinned.
//!
//! Every other suite checks *which* hits a search finds, and that a list is
//! well formed (each file's hits together, files by best hit). None notices
//! when a change quietly reorders the first page, which is the page people
//! read. This test indexes a fixed corpus (the backtest fixtures and a
//! generated Python tree), runs about a hundred searches the way the first
//! page of every surface runs them, and compares the first 20 hits of each,
//! plus the total, with `tests/snapshots/rank_order.txt`.
//!
//! A difference is not necessarily a bug: a ranking change is supposed to
//! move hits. It must be a change someone meant, so the diff of the
//! snapshot shows up in the pull request that caused it. To accept a new
//! order, run
//!
//! ```text
//! FCS_UPDATE_SNAPSHOTS=1 cargo test --test rank_snapshot
//! ```
//!
//! and commit the file. Before comparing, the corpus is indexed twice, at
//! two different places on disk; both must give the same answer, so the
//! order cannot depend on where a repository is checked out, on thread
//! timing or on hash order.

#[path = "support/pygen.rs"]
mod pygen;

use fast_code_search::config::IndexerConfig;
use fast_code_search::search::{
    create_progress_broadcaster, run_background_indexer, BackgroundIndexerConfig, IndexingProgress,
    SearchEngine, SearchMode, SearchRequest, SearchSettings,
};
use std::fmt::Write as _;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};

/// The indexed folder's name, which starts every displayed path.
const ROOT: &str = "snap";
/// Hits pinned per query: the first page most surfaces show.
const PAGE: usize = 20;
const SNAPSHOT: &str = "tests/snapshots/rank_order.txt";

fn python() -> pygen::Codebase {
    pygen::generate(&pygen::Options {
        packages: 16,
        ..pygen::Options::new(7)
    })
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    let mut entries: Vec<_> = std::fs::read_dir(from)
        .unwrap()
        .map(|e| e.unwrap())
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let target = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), &target).unwrap();
        }
    }
}

/// Write the corpus under `root` and index it.
fn build(root: &Path) -> SearchEngine {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/backtest");
    copy_dir(&fixtures, &root.join("fixtures"));
    python().write(&root.join("py"));

    let engine = Arc::new(RwLock::new(SearchEngine::new()));
    run_background_indexer(BackgroundIndexerConfig {
        indexer_config: IndexerConfig {
            paths: vec![root.to_string_lossy().into_owned()],
            watch: false,
            ..Default::default()
        },
        engine: engine.clone(),
        progress: Arc::new(RwLock::new(IndexingProgress::default())),
        progress_tx: create_progress_broadcaster(),
        shutdown: Arc::new(AtomicBool::new(false)),
    });
    Arc::try_unwrap(engine)
        .ok()
        .expect("the indexer let go of the engine")
        .into_inner()
        .unwrap()
}

/// The searches: common words and identifiers from the fixtures, generated
/// names (the most used, which have many hits in many files, and module
/// names, which also match file names), regexes, symbols and references.
fn queries() -> Vec<(SearchMode, String)> {
    let cb = python();
    let mut by_uses: Vec<(&String, usize)> = cb.refs.iter().map(|(n, r)| (n, r.len())).collect();
    by_uses.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let most_used: Vec<String> = by_uses.iter().take(12).map(|(n, _)| (*n).clone()).collect();

    let mut q: Vec<(SearchMode, String)> = Vec::new();
    for w in [
        "self",
        "return",
        "import",
        "def",
        "fn",
        "class",
        "pub",
        "let",
        "use",
        "if",
        "the",
        "new",
        "test",
        "error",
        "Error",
        "widget",
        "Widget",
        "engine",
        "search",
        "Search",
        "provider",
        "user",
        "main",
        "TODO",
        "foo",
        "bar",
        "match",
        "struct",
        "include",
        "line",
        "word",
        "case",
        "quote",
        "\"",
        "->",
        "::",
        "==",
        "async",
        "await",
        "super",
        "raise",
        "except",
        "from gen",
        "__init__",
        "m0",
        "m1",
        "p1",
        "gen",
        "Optional",
        "isinstance",
    ] {
        q.push((SearchMode::Text, w.to_string()));
    }
    for n in &most_used {
        q.push((SearchMode::Text, n.clone()));
    }
    for r in [
        r"fn_\d+",
        r"Cls\d+",
        r"class \w+",
        r"def \w+\(",
        r"^import \w+",
        r"^from \.",
        r"self\.\w+\(",
        r"Err\d+",
        r"\bfn\s+\w+",
        r"[A-Z][a-z]+[A-Z]\w*",
        r"\d{3,}",
        r"(?i)widget",
    ] {
        q.push((SearchMode::Regex, r.to_string()));
    }
    for s in [
        "fn_", "Cls", "Err", "meth_", "deco_", "Widget", "search", "main", "engine",
    ] {
        q.push((SearchMode::Symbols, s.to_string()));
    }
    for n in most_used.iter().take(8) {
        q.push((SearchMode::References, n.clone()));
    }
    q
}

/// The first page of every query, as text.
fn render(engine: &SearchEngine) -> String {
    let settings = SearchSettings::default();
    let mut out = String::new();
    for (mode, text) in queries() {
        let req = SearchRequest {
            query: text.clone(),
            mode,
            max_results: PAGE,
            ..Default::default()
        };
        let limits = req.limits(&settings, None).unwrap();
        let res = engine.run(&req, limits).unwrap();
        let total = match res.info.total_matches {
            Some(n) => n.to_string(),
            None => "more".to_string(),
        };
        writeln!(out, "## {mode:?} {text:?}: {total}").unwrap();
        for m in &res.matches {
            writeln!(out, "{}:{}", m.file_path.replace('\\', "/"), m.line_number).unwrap();
        }
        out.push('\n');
    }
    out
}

#[test]
fn first_pages_keep_their_order() {
    let temp = tempfile::tempdir().unwrap();
    let a = render(&build(&temp.path().join("a").join(ROOT)));
    let b = render(&build(
        &temp
            .path()
            .join("a much longer")
            .join("place")
            .join("on disk")
            .join(ROOT),
    ));
    if a != b {
        let line = a
            .lines()
            .zip(b.lines())
            .position(|(x, y)| x != y)
            .unwrap_or(0);
        panic!(
            "the same corpus at two places on disk ranks differently (line {}):\n  {:?}\n  {:?}",
            line + 1,
            a.lines().nth(line),
            b.lines().nth(line)
        );
    }

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(SNAPSHOT);
    let header = "# The first 20 hits and the total of each search in tests/rank_snapshot.rs.\n\
                  # Regenerate with FCS_UPDATE_SNAPSHOTS=1 cargo test --test rank_snapshot\n\n";
    let want = format!("{header}{a}");
    if std::env::var_os("FCS_UPDATE_SNAPSHOTS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &want).unwrap();
        return;
    }
    let have = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    if have != want {
        let mut diff = String::new();
        let (mut section, mut shown) = (String::new(), 0);
        let (h, w): (Vec<&str>, Vec<&str>) = (have.lines().collect(), want.lines().collect());
        for i in 0..h.len().max(w.len()) {
            let (x, y) = (h.get(i).copied(), w.get(i).copied());
            if let Some(s) = y.filter(|s| s.starts_with("## ")) {
                section = s.to_string();
            }
            if x != y && shown < 20 {
                writeln!(diff, "{section}\n  snapshot: {x:?}\n  now:      {y:?}").unwrap();
                shown += 1;
            }
        }
        panic!(
            "the first page of some searches changed. If the new order is meant, run\n\
             FCS_UPDATE_SNAPSHOTS=1 cargo test --test rank_snapshot and commit {SNAPSHOT}.\n{diff}"
        );
    }
}
