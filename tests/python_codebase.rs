//! Generated Python codebases checked against their answer keys.
//!
//! `support/pygen.rs` writes a seeded tree of packages and modules that
//! import and use each other in every form fcs documents, and records what
//! the right answers are. This test indexes each tree the way the server
//! does and checks, for every name and every module:
//!
//! - the import graph has exactly the generator's edges (the comparison
//!   `fcs check` runs against grimp);
//! - a symbol search for each name finds its one definition;
//! - a reference search for each name finds exactly the lines that use it
//!   in the positions reference-support.md lists as found;
//! - `-rdeps:FILE:*` and `-deps:FILE:*` (which walk import edges strictly)
//!   leave out exactly the files that reach, or are reached from, FILE.
//!
//! Then it rewrites the tree as another seed's (files change, appear and
//! go), hands the changes to the watcher's `apply_burst` in a few bursts,
//! and checks the index against the new key: incremental import
//! resolution must end where a fresh build does.
//!
//! The key itself is checked where the tools exist: Python compiles every
//! file, and grimp (`pip install grimp`) draws the same edges. Set
//! `FCS_REQUIRE_PYTHON_ORACLES=1` to fail instead of skipping those checks
//! (CI's graph-oracle job does).
//!
//! Seeds: `FCS_PYGEN_SEEDS=1,2,3` (default 1 to 6); size:
//! `FCS_PYGEN_PACKAGES` (default 12).

#[path = "support/pygen.rs"]
mod pygen;

use fast_code_search::check::graph::{compare, run_oracle, Lang, Oracle, OracleEdge};
use fast_code_search::check::Corpus;
use fast_code_search::config::IndexerConfig;
use fast_code_search::search::{
    watch_loop, FileChange, SearchEngine, SearchMode, SearchRequest, SearchSettings,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, RwLock};

/// The name of the folder each tree is written to; display paths start with
/// it.
const ROOT_NAME: &str = "tree";

fn env_list(name: &str) -> Option<Vec<u64>> {
    std::env::var(name)
        .ok()
        .map(|v| v.split(',').filter_map(|s| s.trim().parse().ok()).collect())
}

fn require_oracles() -> bool {
    std::env::var_os("FCS_REQUIRE_PYTHON_ORACLES").is_some()
}

/// A display path (`tree/gen/m0.py`) relative to the tree.
fn rel(display: &str) -> String {
    display
        .strip_prefix(&format!("{ROOT_NAME}/"))
        .unwrap_or(display)
        .to_string()
}

/// Every (file, line) a search reports, page by page.
fn search(engine: &SearchEngine, req: SearchRequest) -> Result<BTreeSet<(String, usize)>, String> {
    let settings = SearchSettings::default();
    let mut out = BTreeSet::new();
    let mut offset = 0;
    loop {
        let req = SearchRequest {
            offset,
            max_results: 500,
            ..req.clone()
        };
        let limits = req.limits(&settings, None)?;
        let res = engine.run(&req, limits)?;
        if !res.info.limits_hit.is_empty() {
            return Err(format!("limits hit: {:?}", res.info.limits_hit));
        }
        for m in &res.matches {
            out.insert((rel(&m.file_path), m.line_number));
        }
        let n = res.matches.len();
        if n == 0 || !res.has_more(offset) {
            return Ok(out);
        }
        offset += n;
    }
}

/// Files reachable from `start` along `graph` (including `start`).
fn closure<'a>(graph: &BTreeMap<&'a str, BTreeSet<&'a str>>, start: &'a str) -> BTreeSet<&'a str> {
    let mut seen = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(f) = queue.pop_front() {
        for &next in graph.get(f).into_iter().flatten() {
            if seen.insert(next) {
                queue.push_back(next);
            }
        }
    }
    seen
}

/// Does the generator's key say what Python and grimp say? `Err` names
/// the difference; `Ok(false)` when the tools are missing.
fn check_key(cb: &pygen::Codebase, root: &Path) -> Result<bool, String> {
    let compiled = Command::new("python3")
        .args(["-m", "compileall", "-q"])
        .arg(root)
        .output();
    match compiled {
        Ok(out) if out.status.success() => {}
        Ok(out) => {
            return Err(format!(
                "the generated code does not compile:\n{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ))
        }
        Err(_) => return Ok(false),
    }
    // compileall leaves __pycache__ folders; fcs would index nothing in
    // them, but keep the tree as generated.
    let _ = std::fs::remove_dir_all(root.join("gen").join("__pycache__"));
    for dir in walk_dirs(root) {
        if dir.ends_with("__pycache__") {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
    let grimp = match run_oracle(Lang::Python, root) {
        Ok(Ok(oracle)) => oracle,
        Ok(Err(_)) => return Ok(false),
        Err(e) => return Err(format!("grimp failed: {e:#}")),
    };
    let relative = |p: &Path| {
        p.strip_prefix(root)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/")
    };
    let theirs: BTreeSet<(String, usize, String)> = grimp
        .edges
        .iter()
        .filter_map(|e| Some((relative(&e.from), e.line, relative(e.to.as_deref()?))))
        .collect();
    let ours: BTreeSet<(String, usize, String)> = cb
        .edges
        .iter()
        .map(|e| (e.from.clone(), e.line, e.to.clone()))
        .collect();
    if theirs != ours {
        return Err(format!(
            "the answer key and grimp disagree\n  key only: {:?}\n  grimp only: {:?}",
            ours.difference(&theirs).collect::<Vec<_>>(),
            theirs.difference(&ours).collect::<Vec<_>>()
        ));
    }
    Ok(true)
}

fn walk_dirs(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            if e.path().is_dir() {
                stack.push(e.path());
                out.push(e.path());
            }
        }
    }
    out
}

/// Check one seed; every difference is listed.
fn check_seed(opts: &pygen::Options) -> Vec<String> {
    let cb = pygen::generate(opts);
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join(ROOT_NAME);
    cb.write(&root);
    if let Some(keep) = std::env::var_os("FCS_PYGEN_KEEP") {
        let keep = PathBuf::from(keep).join(format!("seed-{}", opts.seed));
        cb.write(&keep);
    }
    eprintln!(
        "seed {}: {} files, {} import edges, {} names, {} references",
        opts.seed,
        cb.files.len(),
        cb.edges.len(),
        cb.defs.len(),
        cb.refs.values().map(BTreeSet::len).sum::<usize>()
    );
    let mut problems = Vec::new();

    match check_key(&cb, &root) {
        Ok(true) => {}
        Ok(false) => {
            assert!(
                !require_oracles(),
                "FCS_REQUIRE_PYTHON_ORACLES is set but python3 with grimp is missing"
            );
        }
        Err(e) => {
            // A wrong key would make every other check meaningless.
            return vec![e];
        }
    }

    let corpus = Corpus::build_default(std::slice::from_ref(&root)).unwrap();
    for p in check_engine(&corpus.engine, &cb, &root) {
        problems.push(format!("built: {p}"));
    }

    // The watcher's path: rewrite the tree as another seed's (files
    // change, appear and go), report the changes in a few bursts as the
    // watcher would, and check the index against the new key.
    let next = pygen::generate(&pygen::Options {
        seed: opts.seed + 1_000_000,
        ..opts.clone()
    });
    let mut changes: Vec<FileChange> = Vec::new();
    for rel in cb.files.keys().filter(|f| !next.files.contains_key(*f)) {
        std::fs::remove_file(root.join(rel)).unwrap();
        changes.push(FileChange::Deleted(root.join(rel)));
    }
    next.write(&root);
    changes.extend(
        next.files
            .iter()
            .filter(|(f, text)| cb.files.get(*f) != Some(*text))
            .map(|(f, _)| FileChange::Modified(root.join(f))),
    );
    // Leave the folders that lost every file as they are: a deleted file
    // leaves its folder behind on disk too.
    let config = IndexerConfig {
        paths: vec![root.to_string_lossy().into_owned()],
        ..Default::default()
    };
    let engine = Arc::new(RwLock::new(corpus.engine));
    let mut rng = pygen::Rng::new(opts.seed);
    while !changes.is_empty() {
        let n = rng.range(1, changes.len());
        let burst: Vec<FileChange> = changes.drain(..n).collect();
        watch_loop::apply_burst(&engine, &burst, &config).expect("applying a burst");
    }
    let engine = engine.read().unwrap();
    for p in check_engine(&engine, &next, &root) {
        problems.push(format!("after the watcher rewrote it: {p}"));
    }
    let report = engine.check_invariants();
    if !report.is_clean() {
        problems.push(format!("after the watcher rewrote it: {report}"));
    }
    problems
}

/// Check an index of `root` against `cb`'s key.
fn check_engine(engine: &SearchEngine, cb: &pygen::Codebase, root: &Path) -> Vec<String> {
    let mut problems = Vec::new();

    // The import graph.
    let oracle = Oracle {
        files: cb.files.keys().map(|f| root.join(f)).collect(),
        edges: cb
            .edges
            .iter()
            .map(|e| OracleEdge {
                from: root.join(&e.from),
                line: e.line,
                spec: e.spec.clone(),
                to: Some(root.join(&e.to)),
            })
            .collect(),
    };
    let report = compare(engine, Lang::Python, &oracle);
    for (from, line, spec, to) in &report.missing {
        problems.push(format!("missing edge {from}:{line} `{spec}` -> {to}"));
    }
    for (from, to) in &report.extra {
        problems.push(format!("extra edge {from} -> {to}"));
    }

    // Definitions and references, name by name.
    for (name, (path, line, kind)) in &cb.defs {
        let req = SearchRequest {
            query: name.clone(),
            mode: SearchMode::Symbols,
            ..Default::default()
        };
        match search(engine, req) {
            Ok(found) => {
                let want = BTreeSet::from([(path.clone(), *line)]);
                if found != want {
                    problems.push(format!(
                        "symbol {name} ({kind:?}) defined at {path}:{line}; symbol search found {found:?}"
                    ));
                }
            }
            Err(e) => problems.push(format!("symbol search {name}: {e}")),
        }
        let req = SearchRequest {
            query: name.clone(),
            mode: SearchMode::References,
            ..Default::default()
        };
        let want = cb.refs.get(name).cloned().unwrap_or_default();
        match search(engine, req) {
            Ok(found) if found == want => {}
            Ok(found) => problems.push(format!(
                "references of {name}: missing {:?}, extra {:?}",
                want.difference(&found).collect::<Vec<_>>(),
                found.difference(&want).collect::<Vec<_>>()
            )),
            Err(e) => problems.push(format!("reference search {name}: {e}")),
        }
    }

    // Strict graph scopes: `-rdeps:F:*` leaves out F and everything that
    // reaches it; `-deps:F:*` F and everything it reaches.
    let imports = cb.imports();
    let mut imported_by: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (from, tos) in &imports {
        for to in tos {
            imported_by.entry(to).or_default().insert(from);
        }
    }
    let all: BTreeSet<&str> = cb.files.keys().map(String::as_str).collect();
    let marker_files = |found: &BTreeSet<(String, usize)>| -> BTreeSet<String> {
        found.iter().map(|(f, _)| f.clone()).collect()
    };
    for (i, file) in cb.files.keys().enumerate() {
        if i % 3 != 0 {
            continue; // a third of the files is plenty
        }
        for (negated_rdeps, graph) in [(true, &imported_by), (false, &imports)] {
            let reached = closure(graph, file);
            let want: BTreeSet<String> = all.difference(&reached).map(|s| s.to_string()).collect();
            let filter = format!("-{ROOT_NAME}/{file}:*");
            let mut req = SearchRequest {
                query: "MARKER = ".to_string(),
                mode: SearchMode::Text,
                case_sensitive: Some(true),
                ..Default::default()
            };
            if negated_rdeps {
                req.rdeps = vec![filter.clone()];
            } else {
                req.deps = vec![filter.clone()];
            }
            let op = if negated_rdeps { "rdeps" } else { "deps" };
            match search(engine, req) {
                Ok(found) => {
                    let got = marker_files(&found);
                    if got != want {
                        problems.push(format!(
                            "-{op}:{file}:* searched {:?} too many, {:?} too few",
                            got.difference(&want).collect::<Vec<_>>(),
                            want.difference(&got).collect::<Vec<_>>()
                        ));
                    }
                }
                Err(e) => problems.push(format!("-{op}:{file}:*: {e}")),
            }
        }
    }
    problems
}

#[test]
fn generated_python_codebases_match_their_answer_keys() {
    let seeds = env_list("FCS_PYGEN_SEEDS").unwrap_or_else(|| (1..=6).collect());
    let packages = env_list("FCS_PYGEN_PACKAGES")
        .and_then(|v| v.first().copied())
        .unwrap_or(12) as usize;
    let mut failures = Vec::new();
    for seed in seeds {
        let opts = pygen::Options {
            packages,
            ..pygen::Options::new(seed)
        };
        let problems = check_seed(&opts);
        if !problems.is_empty() {
            failures.push(format!(
                "seed {seed} ({} problems; reproduce with FCS_PYGEN_SEEDS={seed}):\n  {}",
                problems.len(),
                problems
                    .iter()
                    .take(25)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n  ")
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
