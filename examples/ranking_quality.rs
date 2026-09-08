//! Ranking quality: does the first result answer the query?
//!
//! Runs a labelled set of queries (`benches/ranking_quality.toml`) against
//! an index built over the pinned benchmark corpora and reports
//! precision@1 and precision@5: the share of queries whose expected file
//! (and, when given, expected line content) appears as the first hit, or
//! within the first five. Latency numbers say how fast the engine is;
//! this says whether what it puts first is what a reader wanted.
//!
//!     cargo run --release --example ranking_quality -- bench-corpus/tokio bench-corpus/django \
//!         [--set benches/ranking_quality.toml] [--markdown FILE] [--json FILE] [--min-p1 0.9] [--min-p5 1.0]
//!
//! With `--min-p1` / `--min-p5` the process exits 1 when the suite scores
//! below the threshold, so CI can gate a ranking regression.

use anyhow::{bail, Context, Result};
use clap::Parser;
use fast_code_search::config::IndexerConfig;
use fast_code_search::search::engine::{RankMode, SearchLimits, SearchMatch};
use fast_code_search::search::PartialIndexedFile;
use fast_code_search::search::{
    parse_query, FileDiscoveryConfig, FileDiscoveryIterator, PreIndexedFile, SearchEngine,
};
use rayon::prelude::*;
use serde::Deserialize;
use std::fmt::Write as _;
use std::path::PathBuf;

#[derive(Parser, Debug)]
struct Args {
    /// Directories to index (the pinned corpora the query set was written against).
    #[arg(required = true)]
    paths: Vec<PathBuf>,
    /// The labelled query set.
    #[arg(long, default_value = "benches/ranking_quality.toml")]
    set: PathBuf,
    /// Write the markdown report here as well as to stderr.
    #[arg(long)]
    markdown: Option<PathBuf>,
    /// Write the per-query results and scores as JSON.
    #[arg(long)]
    json: Option<PathBuf>,
    /// Fail (exit 1) when precision@1 is below this.
    #[arg(long)]
    min_p1: Option<f64>,
    /// Fail (exit 1) when precision@5 is below this.
    #[arg(long)]
    min_p5: Option<f64>,
    /// Instead of the suite, run this one query (mode via --explain-mode)
    /// and print the ranking info and the top ten hits.
    #[arg(long)]
    explain: Option<String>,
    #[arg(long, default_value = "text")]
    explain_mode: String,
}

#[derive(Deserialize, Debug)]
struct QuerySet {
    #[serde(default)]
    description: String,
    query: Vec<LabelledQuery>,
}

#[derive(Deserialize, Debug, Clone)]
struct LabelledQuery {
    /// What a user would type.
    q: String,
    /// text (default), regex, symbols, references.
    #[serde(default = "default_mode")]
    mode: String,
    /// The expected file: a path suffix (forward slashes) of the display path.
    #[serde(default)]
    expect_file: String,
    /// Alternatively, any of these files is acceptable (equally canonical
    /// definitions, e.g. a mirrored method on two types).
    #[serde(default)]
    expect_file_any: Vec<String>,
    /// When given, the expected line must contain this text.
    #[serde(default)]
    expect_line_contains: Option<String>,
    /// Why this is the right answer (kept in the report).
    #[serde(default)]
    why: String,
}

fn default_mode() -> String {
    "text".to_string()
}

fn build(engine: &mut SearchEngine, roots: &[String]) -> Result<usize> {
    let config = IndexerConfig {
        paths: roots.to_vec(),
        ..Default::default()
    };
    let discovery = FileDiscoveryConfig {
        paths: roots.to_vec(),
        exclude_patterns: config.exclude_patterns.clone(),
        include_extensions: config.include_extensions.clone(),
        max_file_size: Some(config.max_file_size),
        respect_gitignore: config.respect_gitignore,
        ..Default::default()
    };
    let files: Vec<PathBuf> = FileDiscoveryIterator::new(&discovery).collect();
    for r in roots {
        engine.add_root_path(std::path::Path::new(r));
    }
    for chunk in files.chunks(500) {
        let pre: Vec<PreIndexedFile> = chunk
            .par_iter()
            .filter_map(|p| {
                PartialIndexedFile::process(p, config.transcode_non_utf8, config.max_file_size)
                    .map(|(partial, _)| PreIndexedFile::from_partial(partial, true))
            })
            .collect();
        engine.index_batch(pre);
        engine.resolve_imports_incremental();
    }
    engine.resolve_imports();
    engine.finalize();
    Ok(files.len())
}

fn run_query(engine: &SearchEngine, q: &LabelledQuery) -> Result<Vec<SearchMatch>> {
    let limits = SearchLimits::new(5);
    let parsed = parse_query(&q.q);
    let (hits, _) = match q.mode.as_str() {
        "text" => engine.search_parsed(&parsed, "", "", limits, RankMode::Auto)?,
        "regex" => engine.search_regex_with_limits(&q.q, "", "", limits, RankMode::Auto)?,
        "symbols" => engine.search_symbols_parsed(&parsed, "", "", limits)?,
        "references" => engine.search_references_parsed(&parsed, "", "", limits)?,
        other => bail!("unknown mode {other:?} for query {:?}", q.q),
    };
    Ok(hits)
}

fn matches(hit: &SearchMatch, q: &LabelledQuery) -> bool {
    let path = hit.file_path.replace('\\', "/");
    let file_ok = if q.expect_file_any.is_empty() {
        path.ends_with(&q.expect_file)
    } else {
        q.expect_file_any.iter().any(|f| path.ends_with(f))
    };
    file_ok
        && q.expect_line_contains
            .as_ref()
            .map(|needle| hit.content.contains(needle))
            .unwrap_or(true)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let set: QuerySet = toml::from_str(
        &std::fs::read_to_string(&args.set)
            .with_context(|| format!("reading {}", args.set.display()))?,
    )
    .with_context(|| format!("parsing {}", args.set.display()))?;
    let roots: Vec<String> = args
        .paths
        .iter()
        .map(|p| {
            p.canonicalize()
                .with_context(|| format!("{} does not exist", p.display()))
                .map(|c| c.to_string_lossy().into_owned())
        })
        .collect::<Result<_>>()?;

    let mut engine = SearchEngine::new();
    let files = build(&mut engine, &roots)?;

    if let Some(q) = &args.explain {
        let limits = SearchLimits::new(10);
        let parsed = parse_query(q);
        let (hits, info) = match args.explain_mode.as_str() {
            "regex" => engine.search_regex_with_limits(q, "", "", limits, RankMode::Auto)?,
            "symbols" => engine.search_symbols_parsed(&parsed, "", "", limits)?,
            "references" => engine.search_references_parsed(&parsed, "", "", limits)?,
            _ => engine.search_parsed(&parsed, "", "", limits, RankMode::Auto)?,
        };
        eprintln!(
            "{files} files; mode {:?}, candidates {}, searched {}, truncated {}, total {:?}",
            info.mode,
            info.total_candidates,
            info.candidates_searched,
            info.truncated_by_budget,
            info.total_matches
        );
        for h in &hits {
            eprintln!(
                "  {:8.2}  {}:{}  {}",
                h.score,
                h.file_path,
                h.line_number,
                h.content.trim()
            );
        }
        return Ok(());
    }

    let mut rows = Vec::new();
    let mut p1 = 0usize;
    let mut p5 = 0usize;
    for q in &set.query {
        let hits = run_query(&engine, q)?;
        let rank = hits.iter().position(|h| matches(h, q));
        if rank == Some(0) {
            p1 += 1;
        }
        if rank.is_some() {
            p5 += 1;
        }
        let first = hits
            .first()
            .map(|h| format!("{}:{} {}", h.file_path, h.line_number, h.content.trim()))
            .unwrap_or_else(|| "(no results)".to_string());
        let top: Vec<String> = hits
            .iter()
            .map(|h| format!("{}:{} {}", h.file_path, h.line_number, h.content.trim()))
            .collect();
        rows.push((q.clone(), rank, first, top));
    }
    let n = set.query.len().max(1);
    let precision_1 = p1 as f64 / n as f64;
    let precision_5 = p5 as f64 / n as f64;

    let mut md = String::new();
    let _ = writeln!(md, "### Ranking quality: {}", set.description);
    let _ = writeln!(md);
    let _ = writeln!(
        md,
        "{} files indexed, {} labelled queries. **precision@1 = {:.2}**, **precision@5 = {:.2}**.",
        files,
        set.query.len(),
        precision_1,
        precision_5
    );
    let _ = writeln!(md);
    let _ = writeln!(md, "| Query | Mode | Rank of expected | First result |");
    let _ = writeln!(md, "|---|---|---|---|");
    for (q, rank, first, _) in &rows {
        let r = match rank {
            Some(0) => "1 ✓".to_string(),
            Some(i) => format!("{}", i + 1),
            None => "miss".to_string(),
        };
        let _ = writeln!(
            md,
            "| `{}` | {} | {} | `{}` |",
            q.q.replace('|', "\\|"),
            q.mode,
            r,
            first
                .replace('|', "\\|")
                .chars()
                .take(90)
                .collect::<String>()
        );
    }
    eprint!("{md}");
    if let Some(path) = &args.markdown {
        std::fs::write(path, &md)?;
    }
    if let Some(path) = &args.json {
        let per_query: Vec<serde_json::Value> = rows
            .iter()
            .map(|(q, rank, first, top)| {
                serde_json::json!({
                    "q": q.q, "mode": q.mode, "expect_file": q.expect_file,
                    "expect_file_any": q.expect_file_any,
                    "expect_line_contains": q.expect_line_contains, "why": q.why,
                    "rank": rank.map(|r| r + 1), "first": first, "top": top,
                })
            })
            .collect();
        std::fs::write(
            path,
            serde_json::to_string_pretty(&serde_json::json!({
                "schema": 1,
                "description": set.description,
                "files": files,
                "queries": set.query.len(),
                "precision_at_1": precision_1,
                "precision_at_5": precision_5,
                "results": per_query,
            }))?,
        )?;
    }
    if let Some(min) = args.min_p1 {
        if precision_1 < min {
            eprintln!("precision@1 {precision_1:.2} is below the required {min:.2}");
            std::process::exit(1);
        }
    }
    if let Some(min) = args.min_p5 {
        if precision_5 < min {
            eprintln!("precision@5 {precision_5:.2} is below the required {min:.2}");
            std::process::exit(1);
        }
    }
    Ok(())
}
