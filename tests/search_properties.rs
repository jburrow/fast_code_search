//! Property tests of search answers against ripgrep, on random text.
//!
//! proptest makes small trees of random lines: ASCII plus the characters
//! whose case folding is unusual (`ß`/`ẞ`, the long s `ſ`, the Kelvin sign,
//! the three sigmas, dotted and dotless i, title case `ǅ`, the micro sign)
//! and multi-byte text (CJK, emoji). It makes queries too: a piece of some
//! line with its case flipped at random, or random text, and random regexes
//! from a small grammar (classes, negated classes, `\s`, anchors, word
//! boundaries, repetition, alternation, `(?i)` groups).
//!
//! Each query runs through `SearchEngine::run` (the path every front end
//! uses) and through `rg` on the same files with the flags the matching
//! docs say are equivalent (`rg -F -i`, `-s`, `-w`; a regex as is). The
//! (file, line) sets must be equal. This checks trigram extraction, case
//! folding, regex candidate planning and line verification together,
//! against an oracle that shares no code with fcs.
//!
//! Needs `rg` on PATH. Without it the tests print a note and pass, unless
//! `FCS_REQUIRE_RG=1` is set (the "fcs vs ripgrep" CI job sets it). More
//! cases: `PROPTEST_CASES=5000 cargo test --release --test search_properties`.

use fast_code_search::search::{SearchEngine, SearchMode, SearchRequest, SearchSettings};
use proptest::prelude::*;
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

/// The characters text and queries are made of.
const CHARS: &[char] = &[
    'a', 'b', 'c', 'A', 'B', 'C', 's', 'S', 'k', 'K', 'i', 'I', 'x', '_', '1', ' ', '.', '(', 'é',
    'É', 'ß', 'ẞ', 'ſ', '\u{212A}', 'Σ', 'σ', 'ς', 'İ', 'ı', 'ǅ', 'ǆ', 'Ǆ', 'µ', 'μ', '日', '本',
    '🦀',
];

fn line() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(CHARS), 0..14).prop_map(String::from_iter)
}

/// Up to five files of up to six lines, each ending in a newline or not.
fn files() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(
        (prop::collection::vec(line(), 1..7), any::<bool>()).prop_map(|(lines, last_newline)| {
            let mut text = lines.join("\n");
            if last_newline {
                text.push('\n');
            }
            text
        }),
        1..6,
    )
}

/// Where a literal query comes from.
#[derive(Debug, Clone)]
enum QuerySource {
    /// Characters `start..start+len` of a line (indices wrap), with the
    /// case of the characters picked by `flips` changed.
    Piece {
        file: usize,
        line: usize,
        start: usize,
        len: usize,
        flips: Vec<bool>,
    },
    Random(String),
}

fn query_source() -> impl Strategy<Value = QuerySource> {
    prop_oneof![
        3 => (any::<usize>(), any::<usize>(), any::<usize>(), 1..6usize,
              prop::collection::vec(any::<bool>(), 6))
            .prop_map(|(file, line, start, len, flips)| QuerySource::Piece {
                file, line, start, len, flips
            }),
        1 => prop::collection::vec(prop::sample::select(CHARS), 1..5)
            .prop_map(|c| QuerySource::Random(String::from_iter(c))),
    ]
}

/// Flip the case of `c` when it has a single-character other case.
fn flip(c: char) -> char {
    let other: Vec<char> = if c.is_uppercase() {
        c.to_lowercase().collect()
    } else {
        c.to_uppercase().collect()
    };
    match (other.first().copied(), other.get(1)) {
        (Some(o), None) => o,
        _ => c,
    }
}

impl QuerySource {
    fn resolve(&self, files: &[String]) -> String {
        match self {
            QuerySource::Random(q) => q.clone(),
            QuerySource::Piece {
                file,
                line,
                start,
                len,
                flips,
            } => {
                let text = &files[file % files.len()];
                let lines: Vec<&str> = text.split('\n').collect();
                let chars: Vec<char> = lines[line % lines.len()].chars().collect();
                if chars.is_empty() {
                    return "ab".to_string();
                }
                let start = start % chars.len();
                let end = (start + len).min(chars.len());
                chars[start..end]
                    .iter()
                    .zip(flips.iter().cycle())
                    .map(|(&c, &f)| if f { flip(c) } else { c })
                    .collect()
            }
        }
    }
}

/// A random regex from a small grammar.
fn regex() -> impl Strategy<Value = String> {
    let leaf = prop_oneof![
        4 => prop::sample::select(CHARS).prop_map(|c| regex_syntax::escape(&c.to_string())),
        1 => Just(".".to_string()),
        1 => Just("[a-c]".to_string()),
        1 => Just("[^a ]".to_string()),
        1 => Just(r"\w".to_string()),
        1 => Just(r"\d".to_string()),
        1 => Just(r"\s".to_string()),
        1 => Just(r"\S".to_string()),
        1 => Just("[[:upper:]]".to_string()),
        1 => Just(r"\b".to_string()),
        1 => Just("^".to_string()),
        1 => Just("$".to_string()),
    ];
    leaf.prop_recursive(3, 12, 4, |inner| {
        prop_oneof![
            3 => prop::collection::vec(inner.clone(), 1..4).prop_map(|v| v.concat()),
            1 => (inner.clone(), inner.clone()).prop_map(|(a, b)| format!("(?:{a}|{b})")),
            1 => (inner.clone(), prop::sample::select(&["*", "+", "?", "{2}", "{1,3}"][..]))
                .prop_map(|(a, r)| format!("(?:{a}){r}")),
            1 => inner.prop_map(|a| format!("(?i:{a})")),
        ]
    })
}

/// The files of one case on disk, indexed.
struct Tree {
    _temp: tempfile::TempDir,
    dir: std::path::PathBuf,
    engine: SearchEngine,
}

impl Tree {
    fn new(files: &[String]) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().canonicalize().unwrap();
        let mut engine = SearchEngine::new();
        for (i, text) in files.iter().enumerate() {
            let path = dir.join(format!("f{i}.txt"));
            std::fs::write(&path, text).unwrap();
            engine.index_file(&path).unwrap();
        }
        engine.finalize();
        Self {
            _temp: temp,
            dir,
            engine,
        }
    }

    /// Every (file name, line) fcs reports, read page by page.
    fn fcs(&self, req: &SearchRequest) -> Result<BTreeSet<(String, usize)>, String> {
        let settings = SearchSettings::default();
        let mut hits = BTreeSet::new();
        let mut offset = 0;
        loop {
            let req = SearchRequest {
                offset,
                max_results: 500,
                ..req.clone()
            };
            let limits = req.limits(&settings, None)?;
            let out = self.engine.run(&req, limits)?;
            if !out.info.limits_hit.is_empty() {
                return Err(format!("limits hit: {:?}", out.info.limits_hit));
            }
            for m in out.matches.iter().filter(|m| m.line_number > 0) {
                let name = Path::new(&m.file_path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                hits.insert((name, m.line_number));
            }
            let n = out.matches.len();
            if n == 0 || !out.has_more(offset) {
                return Ok(hits);
            }
            offset += n;
        }
    }

    /// Every (file name, line) rg reports; `None` when rg rejects the
    /// pattern.
    fn rg(&self, flags: &[&str], pattern: &str) -> Option<BTreeSet<(String, usize)>> {
        let out = Command::new("rg")
            .args([
                "--no-config",
                "--line-number",
                "--with-filename",
                "--no-heading",
                "--null",
            ])
            .args(flags)
            .arg("-e")
            .arg(pattern)
            .arg(&self.dir)
            .output()
            .expect("running rg");
        match out.status.code() {
            Some(0) | Some(1) => {}
            _ => return None,
        }
        let stdout = String::from_utf8(out.stdout).expect("rg printed UTF-8");
        Some(
            stdout
                .lines()
                .map(|l| {
                    let (path, rest) = l.split_once('\0').expect("rg --null output");
                    let line = rest.split(':').next().unwrap().parse().unwrap();
                    let name = Path::new(path).file_name().unwrap().to_string_lossy();
                    (name.to_string(), line)
                })
                .collect(),
        )
    }
}

/// Is rg available? Panics when it is required and missing.
fn have_rg() -> bool {
    let found = Command::new("rg").arg("--version").output().is_ok();
    if !found {
        assert!(
            std::env::var_os("FCS_REQUIRE_RG").is_none(),
            "FCS_REQUIRE_RG is set but rg is not on PATH"
        );
        eprintln!("rg is not on PATH; skipping the ripgrep property tests");
    }
    found
}

fn diff(fcs: &BTreeSet<(String, usize)>, rg: &BTreeSet<(String, usize)>) -> String {
    format!(
        "fcs only: {:?}\nrg only: {:?}",
        fcs.difference(rg).collect::<Vec<_>>(),
        rg.difference(fcs).collect::<Vec<_>>()
    )
}

fn cases() -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|c| c.parse().ok())
        .unwrap_or(128)
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: cases(),
        max_shrink_iters: 4_000,
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// A literal query finds exactly the lines `rg -F` finds, with case
    /// folding (`-i`), exact case (`-s`) and whole words (`-w`).
    #[test]
    fn literal_search_matches_ripgrep(
        files in files(),
        source in query_source(),
        case_sensitive in any::<bool>(),
        whole_word in any::<bool>(),
    ) {
        if !have_rg() {
            return Ok(());
        }
        let query = source.resolve(&files);
        // The text is trimmed and an operator-looking token is not text;
        // CHARS has no ':', so only the trimming applies.
        let query = query.trim().to_string();
        prop_assume!(!query.is_empty());
        let tree = Tree::new(&files);
        let req = SearchRequest {
            query: query.clone(),
            mode: SearchMode::Text,
            case_sensitive: Some(case_sensitive),
            whole_word: Some(whole_word),
            ..Default::default()
        };
        let fcs = tree.fcs(&req).map_err(TestCaseError::fail)?;
        let mut flags = vec!["-F", if case_sensitive { "-s" } else { "-i" }];
        if whole_word {
            flags.push("-w");
        }
        let rg = tree.rg(&flags, &query).expect("rg rejected a literal");
        prop_assert!(fcs == rg, "query {:?} {:?}:\n{}", query, flags, diff(&fcs, &rg));
    }

    /// A regex finds exactly the lines rg finds with the same pattern.
    #[test]
    fn regex_search_matches_ripgrep(
        files in files(),
        pattern in regex(),
        case_sensitive in any::<bool>(),
    ) {
        if !have_rg() {
            return Ok(());
        }
        let tree = Tree::new(&files);
        let flag = if case_sensitive { "-s" } else { "-i" };
        // A pattern rg refuses (it can match a line break) is not compared.
        let Some(rg) = tree.rg(&[flag], &pattern) else {
            return Ok(());
        };
        let req = SearchRequest {
            query: pattern.clone(),
            mode: SearchMode::Regex,
            case_sensitive: Some(case_sensitive),
            ..Default::default()
        };
        let fcs = tree.fcs(&req).map_err(|e| TestCaseError::fail(format!("{pattern:?}: {e}")))?;
        prop_assert!(fcs == rg, "regex {:?} {}:\n{}", pattern, flag, diff(&fcs, &rg));
    }
}
