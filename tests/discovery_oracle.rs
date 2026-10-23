//! Which files fcs indexes, against `rg --files`.
//!
//! fcs promises to search the files ripgrep searches: it reads `.gitignore`,
//! `.ignore`, `.rgignore` and `.git/info/exclude` with ripgrep's rules and
//! precedence. proptest makes small trees (hidden files and folders, names
//! with spaces, folders the default excludes name) with random ignore files
//! at random levels, written from a palette of rules: globs, anchored
//! paths, folder rules, `**`, negations that can and cannot bring a file
//! back, comments, escapes. Then three answers must agree:
//!
//! - `rg --files --hidden --no-require-git` on the tree, minus the folders
//!   fcs excludes by default (`exclude_patterns`: `.git`, `node_modules`,
//!   `target`, `build`, `dist`, `__pycache__`, `venv`, `.venv`);
//! - the files a full build indexes (the walk);
//! - the files the watcher's per-path check (`EligibilityProbe`, used when
//!   a file changes or a folder is reconciled) accepts, asked about every
//!   file on disk.
//!
//! The documented differences from rg are the default excludes and hidden
//! files: rg skips hidden files unless `--hidden`, fcs indexes them (see
//! "How matching compares with grep, ripgrep and git grep" in the book). Binary and
//! oversized files are not generated here; the backtest and unit tests
//! cover those.
//!
//! A second property checks the FILES box with a regex (`regex=true`,
//! the path matches beside the content hits): the paths it reports must be
//! the indexed paths, as shown (`tree/a/f.py`), whose text matches the
//! pattern, as `rg --files | rg -e PATTERN` decides, with `-i` when match
//! case is off.
//!
//! Needs `rg` on PATH. Without it the test prints a note and passes, unless
//! `FCS_REQUIRE_RG=1` is set (the "fcs vs ripgrep" CI job sets it).

use fast_code_search::config::IndexerConfig;
use fast_code_search::search::{
    create_progress_broadcaster, run_background_indexer, BackgroundIndexerConfig, EligibilityProbe,
    IndexingProgress, SearchEngine, SearchMode, SearchRequest, SearchSettings,
};
use proptest::prelude::*;
use std::collections::BTreeSet;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};

/// Folders a tree can have (`""` is the root).
const DIRS: &[&str] = &[
    "",
    "a",
    "a/b",
    "a/b/c",
    "c",
    ".hid",
    "a/.hid",
    "x y",
    "node_modules",
    "a/build",
    "a/b/dist",
];
const NAMES: &[&str] = &["f.py", "g.txt", "h.log", ".dot", "k.md", "c", "y z.txt"];
const IGNORE_FILES: &[&str] = &[".gitignore", ".ignore", ".rgignore", ".git/info/exclude"];
const RULES: &[&str] = &[
    "*.log",
    "!h.log",
    "/f.py",
    "f.py",
    "!f.py",
    "b/",
    "/a",
    "a/",
    "!a/",
    "*.md",
    "!k.md",
    "**/g.txt",
    "g.txt",
    "!g.txt",
    ".dot",
    ".hid/",
    "!.hid/",
    "!a/b/",
    "b/**",
    "a/**/f.py",
    "[fg].*",
    "*",
    "!*/",
    "!*.py",
    "# comment",
    "",
    "\\#x",
    "c",
    "!c",
    "/c/",
    "x y/",
    "y z.txt",
    "*.txt",
    "c/*",
    "!c/f.py",
    "a/b/c/",
    "**/.dot",
    ".*",
];
/// Path segments `exclude_patterns` names by default.
const DEFAULT_EXCLUDED: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "build",
    "dist",
    "__pycache__",
    "venv",
    ".venv",
];

#[derive(Debug, Clone)]
struct Tree {
    files: Vec<String>,
    /// (folder, ignore file name, rules)
    ignores: Vec<(String, String, Vec<String>)>,
}

fn tree() -> impl Strategy<Value = Tree> {
    let file = (prop::sample::select(DIRS), prop::sample::select(NAMES)).prop_map(|(d, n)| {
        if d.is_empty() {
            n.to_string()
        } else {
            format!("{d}/{n}")
        }
    });
    let ignore = (
        prop::sample::select(DIRS),
        prop::sample::select(IGNORE_FILES),
        prop::collection::vec(prop::sample::select(RULES).prop_map(String::from), 1..5),
    )
        .prop_map(|(d, f, r)| (d.to_string(), f.to_string(), r));
    (
        prop::collection::btree_set(file, 1..18),
        prop::collection::vec(ignore, 0..5),
    )
        .prop_map(|(files, ignores)| Tree {
            files: files.into_iter().collect(),
            ignores,
        })
}

impl Tree {
    fn write(&self, root: &Path) {
        for f in &self.files {
            let p = root.join(f);
            // A file named like a folder ("c") makes room for the folder.
            if p.parent().is_some_and(|d| d.is_file()) {
                continue;
            }
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            if !p.is_dir() {
                std::fs::write(&p, format!("text of {f}\n")).unwrap();
            }
        }
        for (dir, name, rules) in &self.ignores {
            let p = root.join(dir).join(name);
            if p.ancestors().skip(1).any(|a| a.is_file()) {
                continue;
            }
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            let mut text = std::fs::read_to_string(&p).unwrap_or_default();
            for r in rules {
                text.push_str(r);
                text.push('\n');
            }
            std::fs::write(&p, text).unwrap();
        }
    }
}

fn relative(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

fn default_excluded(rel: &str) -> bool {
    rel.split('/').any(|s| DEFAULT_EXCLUDED.contains(&s))
}

fn rg_files(root: &Path) -> BTreeSet<String> {
    let out = Command::new("rg")
        .args([
            "--no-config",
            "--files",
            "--hidden",
            "--no-require-git",
            "--null",
        ])
        .current_dir(root)
        .output()
        .expect("running rg");
    assert!(
        matches!(out.status.code(), Some(0) | Some(1)),
        "rg failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .unwrap()
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(|s| s.trim_start_matches("./").replace('\\', "/"))
        .filter(|s| !default_excluded(s))
        .collect()
}

fn indexed_files(root: &Path, config: &IndexerConfig) -> BTreeSet<String> {
    let engine = index(config);
    let engine = engine.read().unwrap();
    engine
        .file_ids_under(root)
        .into_iter()
        .filter_map(|id| engine.get_file_path(id))
        // Display paths start with the root folder's name.
        .map(|p| {
            p.replace('\\', "/")
                .strip_prefix("tree/")
                .expect("a display path under the root")
                .to_string()
        })
        .collect()
}

fn index(config: &IndexerConfig) -> Arc<RwLock<SearchEngine>> {
    let engine = Arc::new(RwLock::new(SearchEngine::new()));
    run_background_indexer(BackgroundIndexerConfig {
        indexer_config: config.clone(),
        engine: engine.clone(),
        progress: Arc::new(RwLock::new(IndexingProgress::default())),
        progress_tx: create_progress_broadcaster(),
        shutdown: Arc::new(AtomicBool::new(false)),
    });
    engine
}

fn probe_files(root: &Path, config: &IndexerConfig) -> BTreeSet<String> {
    let probe = EligibilityProbe::new(&config.discovery_config(config.paths.clone()));
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| probe.is_eligible_path(e.path(), None))
        .map(|e| relative(root, e.path()))
        .collect()
}

fn diff(name: &str, got: &BTreeSet<String>, want: &BTreeSet<String>) -> String {
    let extra: Vec<_> = got.difference(want).collect();
    let missing: Vec<_> = want.difference(got).collect();
    format!("{name}: also has {extra:?}, misses {missing:?}")
}

fn rg_available() -> bool {
    let found = Command::new("rg").arg("--version").output().is_ok();
    if !found {
        assert!(
            std::env::var_os("FCS_REQUIRE_RG").is_none(),
            "FCS_REQUIRE_RG is set but rg is not on PATH"
        );
        eprintln!("rg is not on PATH; skipping the discovery oracle");
    }
    found
}

fn check(t: &Tree) -> Result<(), TestCaseError> {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("tree");
    std::fs::create_dir_all(&root).unwrap();
    t.write(&root);
    let config = IndexerConfig {
        paths: vec![root.to_string_lossy().into_owned()],
        watch: false,
        ..Default::default()
    };
    let rg = rg_files(&root);
    let walk = indexed_files(&root, &config);
    let probe = probe_files(&root, &config);
    prop_assert!(walk == rg, "{}", diff("the build", &walk, &rg));
    prop_assert!(probe == rg, "{}", diff("the watcher's check", &probe, &rg));
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn indexed_files_are_rg_files(t in tree()) {
        if !rg_available() {
            return Ok(());
        }
        check(&t)?;
    }
}

/// Pieces path regexes are made of.
const REGEX_PIECES: &[&str] = &[
    "a", "b/", "c", "/", r"\.py", r"\.txt", r"\w+", "[a-c]", "^", "$", r"\.", ".", "f", "y z",
    r"\b", "F", "K", r"\.hid", "x y/", "[^/]+", "/.*", r"\d", "tree/", "(?:a|c)/", "md$", "_?",
    "[.]", r"\s",
];

fn path_regex() -> impl Strategy<Value = (String, bool)> {
    (
        prop::collection::vec(prop::sample::select(REGEX_PIECES), 1..5),
        any::<bool>(),
    )
        .prop_map(|(p, case)| (p.concat(), case))
}

/// The lines of `list` that `rg -e pattern` matches.
fn rg_filter(list: &BTreeSet<String>, pattern: &str, case: bool) -> BTreeSet<String> {
    let mut child = Command::new("rg")
        .args(["--no-config", "--no-line-number", "--color", "never"])
        .args(if case { None } else { Some("-i") })
        .arg("-e")
        .arg(pattern)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("running rg");
    let mut input = String::new();
    for line in list {
        input.push_str(line);
        input.push('\n');
    }
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        matches!(out.status.code(), Some(0) | Some(1)),
        "rg failed on {pattern:?}"
    );
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(String::from)
        .collect()
}

fn check_file_box(t: &Tree, pattern: &str, case: bool) -> Result<(), TestCaseError> {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("tree");
    std::fs::create_dir_all(&root).unwrap();
    t.write(&root);
    let config = IndexerConfig {
        paths: vec![root.to_string_lossy().into_owned()],
        watch: false,
        ..Default::default()
    };
    let engine = index(&config);
    let engine = engine.read().unwrap();
    let shown: BTreeSet<String> = engine
        .file_ids_under(&root)
        .into_iter()
        .filter_map(|id| engine.get_file_path(id))
        .map(|p| p.replace('\\', "/"))
        .collect();
    let want = rg_filter(&shown, pattern, case);

    let req = SearchRequest {
        query: pattern.to_string(),
        mode: SearchMode::Regex,
        case_sensitive: Some(case),
        path_matches: 1000,
        ..Default::default()
    };
    let limits = req.limits(&SearchSettings::default(), None).unwrap();
    let out = engine.run(&req, limits).unwrap();
    let paths = out.paths.expect("a regex search reports path matches");
    let got: BTreeSet<String> = paths
        .matches
        .iter()
        .map(|m| m.file_path.replace('\\', "/"))
        .collect();
    prop_assert_eq!(paths.total, got.len(), "total vs listed for {:?}", pattern);
    prop_assert!(
        got == want,
        "{}",
        diff(
            &format!("FILES box for {pattern:?} (case {case})"),
            &got,
            &want
        )
    );
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn regex_file_box_matches_rg_on_the_path_list(t in tree(), (pattern, case) in path_regex()) {
        if !rg_available() {
            return Ok(());
        }
        // Patterns rg would reject are not the point here.
        prop_assume!(regex::Regex::new(&pattern).is_ok());
        check_file_box(&t, &pattern, case)?;
    }
}
