//! Text search against ripgrep: does fcs find exactly the lines `rg -F`
//! finds?
//!
//! [`generate`] makes queries up from the corpus itself (identifiers,
//! substrings, whole lines, punctuation and non-ASCII text, in random case,
//! some with match case and whole word set), seeded so a run can be
//! repeated. [`compare`] runs one through the engine with no result limits
//! and through `rg -F` with the same flags, on the same files, and returns
//! the lines only one of them found. [`shrink`] cuts a query that differs
//! down to the shortest form that still differs.
//!
//! rg runs with `-uu --crlf` per root and only files fcs indexed are
//! compared, so discovery differences (gitignore, binary files, size caps)
//! never show up as search differences. Filename hits (line 0) have no rg
//! counterpart and are left out.

use super::Corpus;
use crate::search::engine::{RankMode, SearchLimits};
use crate::search::{parse_query, ParsedQuery, SearchEngine};
use anyhow::{bail, Context, Result};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::process::Command;

/// display path -> 1-based line numbers.
pub type Hits = BTreeMap<String, BTreeSet<usize>>;

/// One (file, line) per hit.
pub fn flatten(h: &Hits) -> BTreeSet<(String, usize)> {
    h.iter()
        .flat_map(|(f, ls)| ls.iter().map(move |l| (f.clone(), *l)))
        .collect()
}

/// How many lines `h` holds.
pub fn count(h: &Hits) -> usize {
    h.values().map(BTreeSet::len).sum()
}

/// rg's flags for one run.
#[derive(Default, Clone, Copy, Debug)]
pub struct RgFlags {
    pub fixed: bool,
    pub ignore_case: bool,
    pub word: bool,
    pub multiline: bool,
}

/// rg hits for one pattern: the start line of every match, per file,
/// restricted to the files `c` indexed. `globs` are rg `-g` globs.
pub fn rg_hits(c: &Corpus, rg: &str, pattern: &str, f: RgFlags, globs: &[String]) -> Result<Hits> {
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
            let display = c.engine.make_display_path(&root.join(rel));
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

/// Whether `rg` runs.
pub fn rg_available(rg: &str) -> bool {
    Command::new(rg)
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// A plain text query, as the web UI's MATCH CASE and WHOLE WORD set it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextQuery {
    /// What a user would type.
    pub q: String,
    /// Match case (the API's `case`; overrides `case:` in `q`).
    pub case: Option<bool>,
    /// Whole word (the API's `word`).
    pub word: Option<bool>,
    /// Where it came from, for the report.
    pub why: String,
}

impl TextQuery {
    /// The query as the engine sees it: parsed, with `case` / `word` on top
    /// (as `SearchRequest` applies the API's parameters).
    pub fn parsed(&self) -> ParsedQuery {
        let mut p = parse_query(&self.q);
        if let Some(c) = self.case {
            p.options.case_sensitive = c;
        }
        if let Some(w) = self.word {
            p.options.whole_word = w;
        }
        p
    }

    /// `case` / `word` for a report.
    pub fn flags(&self) -> String {
        let mut f = Vec::new();
        if self.case == Some(true) {
            f.push("case");
        }
        if self.word == Some(true) {
            f.push("word");
        }
        f.join(", ")
    }

    /// A `[[query]]` block for `benches/rg_backtest.toml`.
    pub fn toml(&self) -> String {
        let mut t = format!("[[query]]\nq = {}\n", toml::Value::String(self.q.clone()));
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

/// Every line fcs finds for `q` with no limits; filename hits counted apart.
pub fn fcs_hits(engine: &SearchEngine, q: &ParsedQuery) -> Result<(Hits, usize)> {
    let (matches, _) =
        engine.search_parsed(q, "", "", SearchLimits::exhaustive(), RankMode::Full)?;
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

/// What fcs and rg found for one query.
pub struct Comparison {
    pub fcs: Hits,
    pub rg: Hits,
    /// rg finds, fcs does not.
    pub missing: BTreeSet<(String, usize)>,
    /// fcs finds, rg does not.
    pub extra: BTreeSet<(String, usize)>,
}

impl Comparison {
    pub fn differs(&self) -> bool {
        !self.missing.is_empty() || !self.extra.is_empty()
    }
}

/// `q` through fcs and through `rg -F` (`-i` unless match case, `-w` for
/// whole word). `q` must have no `file:` / `lang:` operators.
pub fn compare(c: &Corpus, rg: &str, q: &TextQuery) -> Result<Comparison> {
    let parsed = q.parsed();
    let (fcs, _) = fcs_hits(&c.engine, &parsed)?;
    let rg = if parsed.text.trim().is_empty() {
        Hits::new()
    } else {
        let flags = RgFlags {
            fixed: true,
            ignore_case: !parsed.options.case_sensitive,
            word: parsed.options.whole_word,
            multiline: false,
        };
        rg_hits(c, rg, &parsed.text, flags, &[])?
    };
    let (e, f) = (flatten(&rg), flatten(&fcs));
    Ok(Comparison {
        missing: e.difference(&f).cloned().collect(),
        extra: f.difference(&e).cloned().collect(),
        fcs,
        rg,
    })
}

/// Whether `needle` is searched for exactly as written: no operators, no
/// trimming, one line.
pub fn usable_needle(needle: &str) -> bool {
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
pub fn generate(c: &Corpus, n: usize, seed: u64) -> Vec<TextQuery> {
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
        let Some(text) = c.read_display(file) else {
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
        out.push(TextQuery {
            q: needle,
            case,
            word,
            why: format!("generated {kind} (seed {seed})"),
        });
    }
    out
}

/// The shortest form of a differing query that still differs: characters
/// dropped from either end, one at a time.
pub fn shrink(c: &Corpus, rg: &str, q: &TextQuery) -> Result<TextQuery> {
    let mut best = q.clone();
    for _ in 0..64 {
        let chars: Vec<char> = best.q.chars().collect();
        if chars.len() < 2 {
            break;
        }
        let mut next = None;
        for cand in [
            chars[1..].iter().collect::<String>(),
            chars[..chars.len() - 1].iter().collect::<String>(),
        ] {
            if !usable_needle(&cand) {
                continue;
            }
            let trial = TextQuery {
                q: cand,
                ..best.clone()
            };
            if compare(c, rg, &trial)?.differs() {
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

/// The text of `file:line`, trimmed to 120 characters, for a report.
pub fn line_text(
    c: &Corpus,
    cache: &mut std::collections::HashMap<String, Vec<String>>,
    file: &str,
    line: usize,
) -> String {
    let lines = cache.entry(file.to_string()).or_insert_with(|| {
        c.read_display(file)
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
