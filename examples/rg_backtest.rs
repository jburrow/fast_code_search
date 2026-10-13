//! ripgrep backtest: does fcs find exactly what ripgrep finds?
//!
//! Indexes the given trees in-process, then runs every query in a query
//! set (`benches/rg_backtest.toml`) through the engine with no result
//! limits and through `rg` with the equivalent flags, and compares the
//! hits line by line. ripgrep is the oracle: a line it reports that fcs
//! does not is a **miss**, a line fcs reports that it does not is an
//! **extra**. Any difference fails the run unless the query is marked
//! `known` with the reason.
//!
//! With `--generate N`, N more queries are made up from the corpus itself
//! (identifiers, substrings, whole lines, punctuation and non-ASCII text,
//! in random case, some with MATCH CASE and whole word set), seeded by
//! `--seed` so a run can be repeated. Each is compared with `rg -F` the
//! same way; a query that differs is shrunk (characters dropped from either
//! end while it still differs) and the report gives the smallest form as a
//! `[[query]]` block to add to the query set.
//!
//!     cargo run --release --example rg_backtest -- bench-corpus/tokio bench-corpus/django \
//!         tests/fixtures/backtest [--set benches/rg_backtest.toml] [--markdown FILE] \
//!         [--json FILE] [--only SUBSTRING] [--samples 5]
//!
//! How each query is translated:
//!
//! - `mode = "regex"`: `rg -e PATTERN` (plus `-U` when the pattern asks for
//!   whole-file matching, see `regex_search::needs_multiline`). fcs reports
//!   one hit per line where a match *starts*; rg's JSON gives each
//!   submatch's start, so both sides are compared on start lines.
//! - `mode = "text"` (default): the query is parsed with fcs's own parser,
//!   and its text (everything but the `file:`, `lang:`, `case:` and `word:`
//!   operators, spaces, quotes and dashes included) becomes
//!   `rg -F -e TEXT`, with `-i` unless `case:yes` and `-w` for `word:yes`.
//!   The line sets must be equal.
//! - `file:` / `lang:` operators are fcs syntax with no rg equivalent;
//!   such a query must say what it means in rg terms with `globs`
//!   (rg `-g` globs, relative to each tree, `!` to exclude), which keeps
//!   the oracle independent of the code under test.
//!
//! rg runs with `-uu --crlf` per tree, and only files fcs indexed are
//! compared, so discovery differences (gitignore, binary files, size caps)
//! never show up as search differences. Filename hits (line 0, a file whose
//! name matches) have no rg counterpart and are left out.
//!
//! The first page (50 results) of every query is run three times and must
//! be identical: a search that stops at its match budget must still be
//! repeatable. With `--check-paging`, each query with at most 1000 hits
//! is also loaded the way the web UI's LOAD MORE does (the whole list again,
//! 50 longer each time, while `has_more` as `/api/search` computes it), and
//! every hit must end up shown exactly once.
//!
//! With `--check-defaults`, each query is also walked the way a web UI user
//! sees it with the server's default settings, once with auto ranking and
//! once with fast ranking: the first page, SEARCH ALL FILES when fast
//! ranking skipped files, then LOAD MORE until `has_more` is false (the
//! whole list again up to 1000, offset pages past that). The walk must end
//! with every hit exactly once, and an exact total on a search that opened
//! every candidate file must be the real count. Everything else in the
//! backtest runs without limits, which is how the defaults' own gaps (a
//! file cap, an offset ceiling) went unnoticed.

use anyhow::{bail, Context, Result};
use clap::Parser;
use fast_code_search::config::IndexerConfig;
use fast_code_search::search::engine::{
    RankMode, SearchLimits, SearchMatch, SearchRankingInfo, SearchSettings,
};
use fast_code_search::search::regex_search::needs_multiline;
use fast_code_search::search::{
    parse_query, FileDiscoveryConfig, FileDiscoveryIterator, ParsedQuery, PartialIndexedFile,
    PreIndexedFile, SearchEngine,
};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser, Debug)]
struct Args {
    /// Trees to index and search (each is also searched by rg).
    #[arg(required = true)]
    paths: Vec<PathBuf>,
    /// The query set.
    #[arg(long, default_value = "benches/rg_backtest.toml")]
    set: PathBuf,
    /// Write the markdown report here as well as to stderr.
    #[arg(long)]
    markdown: Option<PathBuf>,
    /// Write per-query results as JSON.
    #[arg(long)]
    json: Option<PathBuf>,
    /// Run only queries whose text contains this.
    #[arg(long)]
    only: Option<String>,
    /// Example lines shown per difference.
    #[arg(long, default_value_t = 5)]
    samples: usize,
    /// Also load each query the way LOAD MORE does (the list again, 50
    /// longer each time) and fail when a hit is repeated or skipped.
    #[arg(long)]
    check_paging: bool,
    /// Also walk each query as the web UI does under the server's default
    /// settings (auto and fast ranking): SEARCH ALL FILES, then LOAD MORE to
    /// the end. Fails when a hit is repeated, skipped or out of reach, or an
    /// exact total is wrong.
    #[arg(long)]
    check_defaults: bool,
    /// `--check-defaults` walks only queries with at most this many hits
    /// (deep offset pages each re-run the search).
    #[arg(long, default_value_t = 20_000)]
    defaults_max_hits: usize,
    /// The ripgrep binary.
    #[arg(long, default_value = "rg")]
    rg: String,
    /// Also make up this many text queries from the corpus and check them
    /// against `rg -F` (no paging checks).
    #[arg(long, default_value_t = 0)]
    generate: usize,
    /// Seed for `--generate`.
    #[arg(long, default_value_t = 1)]
    seed: u64,
}

#[derive(Deserialize, Debug)]
struct QuerySet {
    #[serde(default)]
    description: String,
    query: Vec<Query>,
}

#[derive(Deserialize, Debug, Clone)]
struct Query {
    /// What a user would type.
    q: String,
    /// text (default) or regex.
    #[serde(default = "default_mode")]
    mode: String,
    /// rg `-g` globs equivalent to the query's `file:` / `lang:` operators.
    #[serde(default)]
    globs: Vec<String>,
    /// A difference that is understood and accepted, and why. The query is
    /// still run and its difference reported, but does not fail the run.
    #[serde(default)]
    known: Option<String>,
    /// What the query exercises (kept in the report).
    #[serde(default)]
    why: String,
    /// Match case, as the API's `case` parameter sets it (overrides
    /// `case:` in `q`). A text query only.
    #[serde(default)]
    case: Option<bool>,
    /// Whole word, as the API's `word` parameter sets it. A text query only.
    #[serde(default)]
    word: Option<bool>,
    /// Made up by `--generate`.
    #[serde(skip)]
    generated: bool,
}

impl Query {
    /// The query as the engine sees it: parsed, with `case` / `word` on top
    /// (as `SearchRequest` applies the API's parameters).
    fn parsed(&self) -> ParsedQuery {
        let mut p = parse_query(&self.q);
        if let Some(c) = self.case {
            p.options.case_sensitive = c;
        }
        if let Some(w) = self.word {
            p.options.whole_word = w;
        }
        p
    }

    /// `case` / `word` for the report.
    fn flags(&self) -> String {
        let mut f = Vec::new();
        if self.case == Some(true) {
            f.push("case");
        }
        if self.word == Some(true) {
            f.push("word");
        }
        f.join(", ")
    }

    /// A `[[query]]` block for the query set.
    fn toml(&self) -> String {
        let mut t = format!("[[query]]\nq = {}\n", toml_str(&self.q));
        if let Some(c) = self.case {
            t += &format!("case = {c}\n");
        }
        if let Some(w) = self.word {
            t += &format!("word = {w}\n");
        }
        t += "why = \"found by --generate\"\n";
        t
    }
}

fn toml_str(s: &str) -> String {
    toml::Value::String(s.to_string()).to_string()
}

fn default_mode() -> String {
    "text".to_string()
}

/// display path -> 1-based line numbers.
type Hits = BTreeMap<String, BTreeSet<usize>>;

#[derive(Serialize)]
struct Row {
    q: String,
    mode: String,
    status: &'static str,
    fcs_lines: usize,
    rg_lines: usize,
    missing: Vec<String>,
    extra: Vec<String>,
    missing_count: usize,
    extra_count: usize,
    filename_hits: usize,
    known: Option<String>,
    note: String,
}

struct Corpus {
    engine: SearchEngine,
    roots: Vec<PathBuf>,
    /// Display paths of every indexed file.
    indexed: HashSet<String>,
}

fn build(roots: &[PathBuf]) -> Result<Corpus> {
    let root_strs: Vec<String> = roots
        .iter()
        .map(|r| r.to_string_lossy().into_owned())
        .collect();
    let config = IndexerConfig {
        paths: root_strs.clone(),
        ..Default::default()
    };
    let discovery = FileDiscoveryConfig {
        paths: root_strs,
        exclude_patterns: config.exclude_patterns.clone(),
        include_extensions: config.include_extensions.clone(),
        max_file_size: Some(config.max_file_size),
        respect_gitignore: config.respect_gitignore,
        ..Default::default()
    };
    let files: Vec<PathBuf> = FileDiscoveryIterator::new(&discovery).collect();
    let mut engine = SearchEngine::new();
    for r in roots {
        engine.add_root_path(r);
    }
    let mut indexed_paths = Vec::new();
    for chunk in files.chunks(500) {
        let pre: Vec<PreIndexedFile> = chunk
            .par_iter()
            .filter_map(|p| {
                PartialIndexedFile::process(p, config.transcode_non_utf8, config.max_file_size)
                    .map(|(partial, _)| PreIndexedFile::from_partial(partial, true))
            })
            .collect();
        indexed_paths.extend(pre.iter().map(|p| p.path.clone()));
        engine.index_batch(pre);
    }
    engine.resolve_imports();
    engine.finalize();
    let indexed = indexed_paths
        .iter()
        .map(|p| engine.make_display_path(&p.canonicalize().unwrap_or_else(|_| p.clone())))
        .collect();
    Ok(Corpus {
        engine,
        roots: roots.to_vec(),
        indexed,
    })
}

/// fcs hits for `q`, every one of them; filename hits counted apart.
fn fcs_hits(engine: &SearchEngine, q: &Query) -> Result<(Hits, usize)> {
    let (matches, _) = run(engine, q, SearchLimits::exhaustive())?;
    let mut hits = Hits::new();
    let mut filename = 0;
    for m in matches {
        if m.line_number == 0 {
            filename += 1;
        } else {
            hits.entry(m.file_path).or_default().insert(m.line_number);
        }
    }
    Ok((hits, filename))
}

/// The web UI's page size, for the repeat and LOAD MORE checks.
const PAGE: usize = 50;
/// LOAD MORE is checked only for queries with at most this many hits: the
/// web API's `max` cap, past which the UI falls back to offset pages.
const PAGING_MAX_HITS: usize = 1000;

fn run(
    engine: &SearchEngine,
    q: &Query,
    limits: SearchLimits,
) -> Result<(Vec<SearchMatch>, Option<usize>)> {
    let (m, info) = run_ranked(engine, q, limits, RankMode::Full)?;
    Ok((m, info.total_matches))
}

fn run_ranked(
    engine: &SearchEngine,
    q: &Query,
    limits: SearchLimits,
    rank: RankMode,
) -> Result<(Vec<SearchMatch>, SearchRankingInfo)> {
    Ok(match q.mode.as_str() {
        "text" => engine.search_parsed(&q.parsed(), "", "", limits, rank)?,
        "regex" => engine.search_regex_with_limits(&q.q, "", "", limits, rank)?,
        other => bail!("unknown mode {other:?}"),
    })
}

/// `None` when the first page is identical over three runs. A search that
/// stops at its match budget must still be repeatable.
fn repeat_problem(engine: &SearchEngine, q: &Query) -> Result<Option<String>> {
    let page = || -> Result<Vec<(String, usize)>> {
        Ok(run(engine, q, SearchLimits::new(PAGE))?
            .0
            .iter()
            .map(|m| (m.file_path.clone(), m.line_number))
            .collect())
    };
    let first = page()?;
    for _ in 0..2 {
        if page()? != first {
            return Ok(Some(
                "repeat: the first page changed between identical runs".into(),
            ));
        }
    }
    Ok(None)
}

/// `None` when LOAD MORE ends with every hit exactly once, else what went
/// wrong. Emulates the web UI (`loadMoreRequest` in keyword-helpers.js):
/// each click fetches the whole list again, `PAGE` longer, from offset 0 and
/// replaces what is shown, while the response says more exist (`has_more`
/// as `/api/search` computes it). Every list along the way must be free of
/// repeats and hold only real hits, and the last one must hold them all.
fn paging_problem(engine: &SearchEngine, q: &Query) -> Result<Option<String>> {
    let key = |m: &SearchMatch| (m.file_path.clone(), m.line_number);
    let (all, _) = run(engine, q, SearchLimits::exhaustive())?;
    if all.len() > PAGING_MAX_HITS {
        return Ok(None);
    }
    let want: BTreeSet<(String, usize)> = all.iter().map(key).collect();
    let mut shown: Vec<(String, usize)> = Vec::new();
    let mut max = PAGE;
    let (mut dups, mut invented) = (0, 0);
    loop {
        let (list, total) = run(engine, q, SearchLimits::new(max))?;
        let grew = list.len() > shown.len();
        shown = list.iter().map(key).collect();
        let distinct: BTreeSet<_> = shown.iter().collect();
        dups = dups.max(shown.len() - distinct.len());
        invented = invented.max(distinct.iter().filter(|k| !want.contains(**k)).count());
        let has_more = match total {
            Some(t) => shown.len() < t,
            None => true,
        };
        if !has_more || !grew || max >= PAGING_MAX_HITS {
            break;
        }
        max = (shown.len() + PAGE).min(PAGING_MAX_HITS);
    }
    let shown: BTreeSet<_> = shown.into_iter().collect();
    let lost: Vec<_> = want.iter().filter(|k| !shown.contains(*k)).collect();
    if dups == 0 && lost.is_empty() && invented == 0 {
        return Ok(None);
    }
    Ok(Some(format!(
        "paging: {} lost, {dups} repeated, {invented} not in the full result{}",
        lost.len(),
        lost.first()
            .map(|(f, l)| format!(" (first lost: {f}:{l})"))
            .unwrap_or_default()
    )))
}

/// `None` when a web UI user with the server's default settings can reach
/// every hit exactly once, else what went wrong. Emulates keyword.js: the
/// first page; SEARCH ALL FILES (`moreFilesRequest`) when fast ranking
/// skipped candidate files; then LOAD MORE (`loadMoreRequest`) while
/// `has_more`: below 1000 shown, the whole list again `PAGE` longer from
/// offset 0, past it the next `PAGE` at `offset`. Also checks that an exact
/// total from a search that opened every candidate is the real count.
fn defaults_problem(
    engine: &SearchEngine,
    q: &Query,
    rank: RankMode,
    max_hits: usize,
) -> Result<Option<String>> {
    let key = |m: &SearchMatch| (m.file_path.clone(), m.line_number);
    let settings = SearchSettings::default();
    let (all, _) = run(engine, q, SearchLimits::exhaustive())?;
    if all.len() > max_hits {
        return Ok(None);
    }
    let want: BTreeSet<(String, usize)> = all.iter().map(key).collect();
    let page = |offset: usize, max: usize, files: Option<usize>| {
        let limits = settings.apply(SearchLimits::new(max).with_offset(offset), files);
        run_ranked(engine, q, limits, rank)
    };
    let label = format!("{rank:?}").to_lowercase();
    let mut problems = Vec::new();
    let honest_total = |info: &SearchRankingInfo, problems: &mut Vec<String>| {
        if let Some(t) = info.total_matches {
            if info.candidates_searched >= info.total_candidates && t != want.len() {
                problems.push(format!("total says {t}, there are {}", want.len()));
            }
        }
    };

    let (_, first) = page(0, PAGE, None)?;
    honest_total(&first, &mut problems);
    let files =
        (first.candidates_searched < first.total_candidates).then_some(first.total_candidates);

    let mut shown: Vec<(String, usize)> = Vec::new();
    let mut stuck = None;
    for _ in 0..=(want.len() / PAGE + 2) {
        let (offset, max) = if shown.len() < PAGING_MAX_HITS {
            (0, (shown.len() + PAGE).min(PAGING_MAX_HITS))
        } else {
            (shown.len(), PAGE)
        };
        if settings.check_offset(offset).is_err() {
            stuck = Some(format!("offset {offset} is past server.max_page_offset"));
            break;
        }
        let (list, info) = page(offset, max, files)?;
        if offset == 0 && shown.is_empty() {
            honest_total(&info, &mut problems);
        }
        let before = shown.len();
        if offset == 0 {
            shown = list.iter().map(key).collect();
        } else {
            shown.extend(list.iter().map(key));
        }
        let has_more = match info.total_matches {
            Some(t) => offset + list.len() < t,
            None => true,
        };
        if !has_more {
            break;
        }
        if shown.len() <= before {
            stuck = Some(format!("LOAD MORE stopped growing at {}", shown.len()));
            break;
        }
    }
    let distinct: BTreeSet<_> = shown.iter().cloned().collect();
    let dups = shown.len() - distinct.len();
    let invented = distinct.iter().filter(|k| !want.contains(*k)).count();
    let lost: Vec<_> = want.iter().filter(|k| !distinct.contains(*k)).collect();
    if dups > 0 || invented > 0 || !lost.is_empty() {
        problems.push(format!(
            "{} lost, {dups} repeated, {invented} not in the full result{}",
            lost.len(),
            lost.first()
                .map(|(f, l)| format!(" (first lost: {f}:{l})"))
                .unwrap_or_default()
        ));
    }
    problems.extend(stuck);
    if problems.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!("defaults ({label}): {}", problems.join(", "))))
}

#[derive(Default, Clone, Copy)]
struct RgFlags {
    fixed: bool,
    ignore_case: bool,
    word: bool,
    multiline: bool,
}

/// rg hits for one pattern: the start line of every match, per file,
/// restricted to the files fcs indexed.
fn rg_hits(c: &Corpus, rg: &str, pattern: &str, f: RgFlags, globs: &[String]) -> Result<Hits> {
    let mut hits = Hits::new();
    for root in &c.roots {
        let mut cmd = Command::new(rg);
        cmd.current_dir(root)
            .args(["--json", "--no-config", "-uu", "--crlf", "--no-messages"]);
        if f.fixed {
            cmd.arg("-F");
        }
        if f.ignore_case {
            cmd.arg("-i");
        }
        if f.word {
            cmd.arg("-w");
        }
        if f.multiline {
            cmd.arg("-U");
        }
        for g in globs {
            cmd.args(["-g", g]);
        }
        cmd.args(["-e", pattern, "--", "."]);
        let out = cmd
            .output()
            .with_context(|| format!("running {rg} (is ripgrep installed?)"))?;
        // 0 = matches, 1 = none, 2 = error (unreadable files are fine:
        // --no-messages, and only indexed files count anyway).
        if out.status.code() == Some(2) && out.stdout.is_empty() {
            bail!(
                "rg failed for {pattern:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        for line in out.stdout.split(|&b| b == b'\n') {
            if line.is_empty() {
                continue;
            }
            let v: serde_json::Value = serde_json::from_slice(line)?;
            if v["type"] != "match" {
                continue;
            }
            let d = &v["data"];
            let Some(rel) = d["path"]["text"].as_str() else {
                continue; // non-UTF-8 path
            };
            let rel = rel.strip_prefix("./").unwrap_or(rel);
            let path = root.join(rel);
            let display = c.engine.make_display_path(&path);
            if !c.indexed.contains(&display) {
                continue;
            }
            let first = d["line_number"].as_u64().unwrap_or(0) as usize;
            let text = d["lines"]["text"].as_str().unwrap_or("");
            let set = hits.entry(display).or_default();
            for sm in d["submatches"].as_array().into_iter().flatten() {
                let start = sm["start"].as_u64().unwrap_or(0) as usize;
                let before = text.get(..start).unwrap_or("");
                set.insert(first + before.bytes().filter(|&b| b == b'\n').count());
            }
        }
    }
    hits.retain(|_, v| !v.is_empty());
    Ok(hits)
}

fn flatten(h: &Hits) -> BTreeSet<(String, usize)> {
    h.iter()
        .flat_map(|(f, ls)| ls.iter().map(move |l| (f.clone(), *l)))
        .collect()
}

fn count(h: &Hits) -> usize {
    h.values().map(BTreeSet::len).sum()
}

/// The content of the file shown as `file`.
fn read_display(c: &Corpus, file: &str) -> Option<String> {
    // Display paths start with the root's own name.
    let (root_name, rel) = file.split_once('/').unwrap_or(("", file));
    c.roots
        .iter()
        .find(|r| {
            r.file_name()
                .is_some_and(|n| n.to_string_lossy() == root_name)
        })
        .and_then(|r| std::fs::read(r.join(rel)).ok())
        .map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// The text of `file:line`, for the report.
fn line_text(
    c: &Corpus,
    cache: &mut HashMap<String, Vec<String>>,
    file: &str,
    line: usize,
) -> String {
    let lines = cache.entry(file.to_string()).or_insert_with(|| {
        read_display(c, file)
            .map(|t| t.lines().map(str::to_string).collect())
            .unwrap_or_default()
    });
    let t = lines
        .get(line.wrapping_sub(1))
        .map(|s| s.trim())
        .unwrap_or("");
    let t: String = t.chars().take(120).collect();
    format!("{file}:{line}: {t}")
}

struct Outcome {
    expected: Hits,
    missing: BTreeSet<(String, usize)>,
    extra: BTreeSet<(String, usize)>,
    note: String,
}

fn compare_regex(c: &Corpus, rg: &str, q: &Query, fcs: &Hits) -> Result<Outcome> {
    let flags = RgFlags {
        multiline: needs_multiline(&q.q),
        ..Default::default()
    };
    let expected = rg_hits(c, rg, &q.q, flags, &q.globs)?;
    let (e, f) = (flatten(&expected), flatten(fcs));
    Ok(Outcome {
        missing: e.difference(&f).cloned().collect(),
        extra: f.difference(&e).cloned().collect(),
        expected,
        note: if flags.multiline {
            "rg -U".into()
        } else {
            String::new()
        },
    })
}

fn compare_text(c: &Corpus, rg: &str, q: &Query, fcs: &Hits) -> Result<Outcome> {
    let parsed = q.parsed();
    if (!parsed.include_globs.is_empty() || !parsed.exclude_globs.is_empty()) && q.globs.is_empty()
    {
        bail!("query {:?} uses file:/lang: but gives no rg `globs`", q.q);
    }
    let flags = RgFlags {
        fixed: true,
        ignore_case: !parsed.options.case_sensitive,
        word: parsed.options.whole_word,
        multiline: false,
    };
    if parsed.text.trim().is_empty() {
        let f = flatten(fcs);
        return Ok(Outcome {
            expected: Hits::new(),
            missing: BTreeSet::new(),
            extra: f,
            note: "no text: fcs should return nothing".into(),
        });
    }
    let expected = rg_hits(c, rg, &parsed.text, flags, &q.globs)?;
    let (e, f) = (flatten(&expected), flatten(fcs));
    Ok(Outcome {
        missing: e.difference(&f).cloned().collect(),
        extra: f.difference(&e).cloned().collect(),
        expected,
        note: String::new(),
    })
}

/// Whether `needle` is searched for exactly as written: no operators, no
/// trimming, one line.
fn usable_needle(needle: &str) -> bool {
    if needle.chars().count() < 2
        || needle.trim() != needle
        || needle.contains(['\n', '\r'])
        || needle.contains('\u{FFFD}')
    {
        return false;
    }
    let p = parse_query(needle);
    p.text == needle
        && p.include_globs.is_empty()
        && p.exclude_globs.is_empty()
        && !p.options.case_sensitive
        && !p.options.whole_word
}

/// `n` text queries made up from the corpus, the same for the same seed.
fn generate(c: &Corpus, n: usize, seed: u64) -> Vec<Query> {
    use rand::{rngs::StdRng, Rng, SeedableRng};
    let mut rng = StdRng::seed_from_u64(seed);
    let mut files: Vec<&String> = c.indexed.iter().collect();
    files.sort();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut attempts = 0;
    while out.len() < n && attempts < n * 100 && !files.is_empty() {
        attempts += 1;
        let file = files[rng.random_range(0..files.len())];
        let Some(text) = read_display(c, file) else {
            continue;
        };
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        if lines.is_empty() {
            continue;
        }
        let line = lines[rng.random_range(0..lines.len())].trim();
        let chars: Vec<char> = line.chars().collect();
        let window = |rng: &mut StdRng, at: usize, max: usize| -> String {
            let len = rng.random_range(2..=max.max(2));
            chars[at.min(chars.len())..(at + len).min(chars.len())]
                .iter()
                .collect()
        };
        let (kind, needle) = match rng.random_range(0..5) {
            0 => {
                let words: Vec<&str> = line
                    .split(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
                    .filter(|w| !w.is_empty())
                    .collect();
                if words.is_empty() {
                    continue;
                }
                (
                    "identifier",
                    words[rng.random_range(0..words.len())].to_string(),
                )
            }
            1 => {
                let at = rng.random_range(0..chars.len());
                ("substring", window(&mut rng, at, 16))
            }
            2 => (
                "line",
                chars.iter().take(60).collect::<String>().trim().to_string(),
            ),
            3 => {
                let punct: Vec<usize> = (0..chars.len())
                    .filter(|&i| chars[i].is_ascii_punctuation())
                    .collect();
                if punct.is_empty() {
                    continue;
                }
                let at = punct[rng.random_range(0..punct.len())];
                ("punctuation", window(&mut rng, at.saturating_sub(1), 5))
            }
            _ => {
                let wide: Vec<usize> = (0..chars.len()).filter(|&i| !chars[i].is_ascii()).collect();
                if wide.is_empty() {
                    continue;
                }
                let at = wide[rng.random_range(0..wide.len())];
                let back = rng.random_range(0..3);
                ("non-ASCII", window(&mut rng, at.saturating_sub(back), 6))
            }
        };
        let needle = match rng.random_range(0..4) {
            0 => needle.to_uppercase(),
            1 => needle.to_lowercase(),
            2 => needle
                .chars()
                .map(|ch| {
                    if ch.is_lowercase() {
                        ch.to_uppercase().next().unwrap_or(ch)
                    } else {
                        ch.to_lowercase().next().unwrap_or(ch)
                    }
                })
                .collect(),
            _ => needle,
        };
        if !usable_needle(&needle) {
            continue;
        }
        let case = rng.random_bool(0.3).then_some(true);
        let word = rng.random_bool(0.3).then_some(true);
        if !seen.insert((needle.clone(), case, word)) {
            continue;
        }
        out.push(Query {
            q: needle,
            mode: default_mode(),
            globs: Vec::new(),
            known: None,
            why: format!("generated {kind} (seed {seed})"),
            case,
            word,
            generated: true,
        });
    }
    out
}

/// Whether fcs and rg disagree on `q`.
fn differs(c: &Corpus, rg: &str, q: &Query) -> Result<bool> {
    let (fcs, _) = fcs_hits(&c.engine, q)?;
    let out = compare_text(c, rg, q, &fcs)?;
    Ok(!out.missing.is_empty() || !out.extra.is_empty())
}

/// The shortest form of a differing generated query that still differs:
/// characters dropped from either end, one at a time.
fn shrink(c: &Corpus, rg: &str, q: &Query) -> Result<Query> {
    let mut best = q.clone();
    for _ in 0..64 {
        let chars: Vec<char> = best.q.chars().collect();
        let mut next = None;
        for cand in [
            chars[1..].iter().collect::<String>(),
            chars[..chars.len() - 1].iter().collect::<String>(),
        ] {
            if !usable_needle(&cand) {
                continue;
            }
            let trial = Query {
                q: cand,
                ..best.clone()
            };
            if differs(c, rg, &trial)? {
                next = Some(trial);
                break;
            }
        }
        match next {
            Some(n) => best = n,
            None => break,
        }
    }
    Ok(best)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let set: QuerySet = toml::from_str(
        &std::fs::read_to_string(&args.set)
            .with_context(|| format!("reading {}", args.set.display()))?,
    )
    .with_context(|| format!("parsing {}", args.set.display()))?;
    let roots: Vec<PathBuf> = args
        .paths
        .iter()
        .map(|p| {
            p.canonicalize()
                .with_context(|| format!("{} does not exist", p.display()))
        })
        .collect::<Result<_>>()?;
    let started = std::time::Instant::now();
    let corpus = build(&roots)?;
    eprintln!(
        "indexed {} files from {} trees in {:.1} s",
        corpus.indexed.len(),
        roots.len(),
        started.elapsed().as_secs_f64()
    );

    let mut queries = set.query.clone();
    if args.generate > 0 {
        let made = generate(&corpus, args.generate, args.seed);
        eprintln!("generated {} queries (seed {})", made.len(), args.seed);
        queries.extend(made);
    }
    let mut rows = Vec::new();
    let mut cache = HashMap::new();
    let mut shrunk = Vec::new();
    for q in &queries {
        if !q.generated
            && args
                .only
                .as_ref()
                .is_some_and(|o| !q.q.contains(o.as_str()))
        {
            continue;
        }
        let (fcs, filename_hits) = fcs_hits(&corpus.engine, q)?;
        let out = match q.mode.as_str() {
            "regex" => compare_regex(&corpus, &args.rg, q, &fcs)?,
            _ => compare_text(&corpus, &args.rg, q, &fcs)?,
        };
        // Generated queries check the line sets only.
        let paging = if q.generated {
            None
        } else {
            match repeat_problem(&corpus.engine, q)? {
                Some(p) => Some(p),
                None if args.check_paging => paging_problem(&corpus.engine, q)?,
                None => None,
            }
        };
        let paging = match paging {
            Some(p) => Some(p),
            None if args.check_defaults && !q.generated => {
                let mut found = None;
                for rank in [RankMode::Auto, RankMode::Fast] {
                    found = defaults_problem(&corpus.engine, q, rank, args.defaults_max_hits)?;
                    if found.is_some() {
                        break;
                    }
                }
                found
            }
            None => None,
        };
        let differs = !out.missing.is_empty() || !out.extra.is_empty() || paging.is_some();
        if differs && q.generated {
            shrunk.push(shrink(&corpus, &args.rg, q)?);
        }
        let status = match (differs, q.known.is_some()) {
            (false, false) => "pass",
            (false, true) => "pass (known no longer differs)",
            (true, true) => "known",
            (true, false) => "DIFF",
        };
        let sample = |s: &BTreeSet<(String, usize)>, cache: &mut HashMap<_, _>| -> Vec<String> {
            s.iter()
                .take(args.samples)
                .map(|(f, l)| line_text(&corpus, cache, f, *l))
                .collect()
        };
        rows.push(Row {
            q: q.q.clone(),
            mode: q.mode.clone(),
            status,
            fcs_lines: count(&fcs),
            rg_lines: count(&out.expected),
            missing: sample(&out.missing, &mut cache),
            extra: sample(&out.extra, &mut cache),
            missing_count: out.missing.len(),
            extra_count: out.extra.len(),
            filename_hits,
            known: q.known.clone(),
            note: [
                paging.as_deref().unwrap_or(""),
                &q.flags(),
                &q.why,
                &out.note,
            ]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("; "),
        });
    }

    let failed = rows.iter().filter(|r| r.status == "DIFF").count();
    let known = rows.iter().filter(|r| r.status == "known").count();
    let mut md = String::new();
    writeln!(md, "## ripgrep backtest")?;
    writeln!(md)?;
    writeln!(
        md,
        "{} · {} files · {} queries: **{} differ**, {} known, {} match ripgrep exactly",
        if set.description.is_empty() {
            "corpus"
        } else {
            &set.description
        },
        corpus.indexed.len(),
        rows.len(),
        failed,
        known,
        rows.len() - failed - known
    )?;
    writeln!(md)?;
    writeln!(
        md,
        "| Status | Mode | Query | rg lines | fcs lines | Missing | Extra |"
    )?;
    writeln!(md, "|---|---|---|---:|---:|---:|---:|")?;
    for r in &rows {
        writeln!(
            md,
            "| {} | {} | `{}` | {} | {} | {} | {} |",
            r.status,
            r.mode,
            r.q.replace('|', "\\|").replace('`', "'"),
            r.rg_lines,
            r.fcs_lines,
            r.missing_count,
            r.extra_count
        )?;
    }
    for r in rows.iter().filter(|r| {
        r.missing_count + r.extra_count > 0
            || r.note.starts_with("paging:")
            || r.note.starts_with("repeat:")
            || r.note.starts_with("defaults")
    }) {
        writeln!(md)?;
        writeln!(
            md,
            "### `{}` ({}, {})",
            r.q.replace('`', "'"),
            r.mode,
            r.status
        )?;
        if let Some(k) = &r.known {
            writeln!(md, "Known: {k}")?;
        }
        if !r.note.is_empty() {
            writeln!(md, "Note: {}", r.note)?;
        }
        for (label, n, lines) in [
            (
                "Missing (rg finds, fcs does not)",
                r.missing_count,
                &r.missing,
            ),
            ("Extra (fcs finds, rg does not)", r.extra_count, &r.extra),
        ] {
            if n > 0 {
                writeln!(md, "\n{label}: {n}\n\n```")?;
                for l in lines {
                    writeln!(md, "{l}")?;
                }
                writeln!(md, "```")?;
            }
        }
    }
    if !shrunk.is_empty() {
        writeln!(md)?;
        writeln!(
            md,
            "### Generated queries that differ, shrunk\n\nAdd these to the query set once fixed:\n\n```toml"
        )?;
        for q in &shrunk {
            writeln!(md, "{}", q.toml())?;
        }
        writeln!(md, "```")?;
    }
    eprint!("{md}");
    if let Some(p) = &args.markdown {
        std::fs::write(p, &md)?;
    }
    if let Some(p) = &args.json {
        std::fs::write(
            p,
            serde_json::to_string_pretty(&serde_json::json!({
                "files": corpus.indexed.len(),
                "queries": rows.len(),
                "differ": failed,
                "known": known,
                "rows": rows,
            }))?,
        )?;
    }
    if failed > 0 {
        eprintln!("\n{failed} queries differ from ripgrep");
        std::process::exit(1);
    }
    Ok(())
}
