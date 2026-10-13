//! Dependency-graph oracle: does fcs draw the import edges the language's
//! own resolver draws?
//!
//! Indexes a tree in-process and compares the graph's edges with an edge
//! list from an independent resolver, written by one of the scripts in
//! `scripts/graph-oracle/`:
//!
//! - `ts.mjs`: TypeScript's `ts.resolveModuleName` under each file's
//!   `tsconfig.json` (`paths`, `baseUrl`, `extends`), for JS and TS;
//! - `py.py`: `grimp`, which resolves Python imports statically as Python
//!   would with the tree on `sys.path`.
//!
//! ```sh
//! node scripts/graph-oracle/ts.mjs TREE > ts.json
//! cargo run --release --example graph_oracle -- TREE --oracle ts.json --lang ts
//! ```
//!
//! An edge the oracle draws and fcs does not is a **miss**; one fcs draws
//! and the oracle does not is an **extra** (a wrong edge). Only files fcs
//! indexed count, on both sides. `--max-missing` and `--max-extra` fail the
//! run above a count, so a baseline can only go down. The report groups
//! misses by the shape of the import, which says which gap in
//! `docs/book/src/reference/graph-support.md` each one is.

use anyhow::{bail, Context, Result};
use clap::Parser;
use fast_code_search::config::IndexerConfig;
use fast_code_search::search::{
    FileDiscoveryConfig, FileDiscoveryIterator, PartialIndexedFile, PreIndexedFile, SearchEngine,
};
use rayon::prelude::*;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
struct Args {
    /// The tree to index (the one the oracle ran on).
    tree: PathBuf,
    /// The oracle's edge list (JSON from scripts/graph-oracle/).
    #[arg(long)]
    oracle: PathBuf,
    /// The oracle's language, `ts` (JS and TS) or `py`, for grouping.
    #[arg(long)]
    lang: String,
    /// Fail when more edges than this are missing.
    #[arg(long)]
    max_missing: Option<usize>,
    /// Fail when more edges than this are extra.
    #[arg(long)]
    max_extra: Option<usize>,
    /// Example edges shown per group.
    #[arg(long, default_value_t = 5)]
    samples: usize,
    /// Write the markdown report here as well as to stderr.
    #[arg(long)]
    markdown: Option<PathBuf>,
}

#[derive(Deserialize)]
struct Oracle {
    /// The files the oracle analysed; only their imports are compared.
    files: Vec<PathBuf>,
    edges: Vec<OracleEdge>,
}

#[derive(Deserialize)]
struct OracleEdge {
    from: PathBuf,
    line: usize,
    spec: String,
    to: Option<PathBuf>,
}

/// Whether the oracle can judge an edge to `to`. TypeScript resolves only
/// script and JSON modules; an import of a stylesheet, a `.vue` or `.svelte`
/// component or an asset is a real dependency that fcs draws and a bundler
/// resolves, but TypeScript cannot, so those edges are counted apart.
fn judged(lang: &str, to: &Path) -> bool {
    lang != "ts"
        || to.extension().and_then(|e| e.to_str()).is_some_and(|e| {
            matches!(
                e,
                "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts" | "json"
            )
        })
}

/// The shape of an import, for grouping misses.
fn shape(lang: &str, spec: &str) -> &'static str {
    let s = spec.trim();
    if lang == "py" {
        return if s.starts_with("from .") {
            "relative `from .x import y`"
        } else if s.starts_with("from ") {
            "`from pkg import name`"
        } else {
            "`import pkg.mod`"
        };
    }
    if s.starts_with("./") || s.starts_with("../") || s == "." || s == ".." {
        "relative path"
    } else if s.starts_with("@/") || s.starts_with("~/") {
        "`@/` or `~/` alias"
    } else if s.starts_with('@') {
        "scoped package or alias (`@x/y`)"
    } else {
        "bare name (package, workspace or tsconfig `paths`)"
    }
}

fn build(tree: &Path) -> Result<SearchEngine> {
    let root = tree.to_string_lossy().into_owned();
    let config = IndexerConfig {
        paths: vec![root.clone()],
        ..Default::default()
    };
    let discovery = FileDiscoveryConfig {
        paths: vec![root],
        exclude_patterns: config.exclude_patterns.clone(),
        include_extensions: config.include_extensions.clone(),
        max_file_size: Some(config.max_file_size),
        respect_gitignore: config.respect_gitignore,
        ..Default::default()
    };
    let files: Vec<PathBuf> = FileDiscoveryIterator::new(&discovery).collect();
    let mut engine = SearchEngine::new();
    engine.add_root_path(tree);
    for chunk in files.chunks(500) {
        let pre: Vec<PreIndexedFile> = chunk
            .par_iter()
            .filter_map(|p| {
                PartialIndexedFile::process(p, config.transcode_non_utf8, config.max_file_size)
                    .map(|(partial, _)| PreIndexedFile::from_partial(partial, true))
            })
            .collect();
        engine.index_batch(pre);
    }
    engine.resolve_imports();
    engine.finalize();
    Ok(engine)
}

fn main() -> Result<()> {
    let args = Args::parse();
    if !matches!(args.lang.as_str(), "ts" | "py") {
        bail!("--lang must be ts or py");
    }
    let tree = args
        .tree
        .canonicalize()
        .with_context(|| format!("{} does not exist", args.tree.display()))?;
    let oracle: Oracle = serde_json::from_str(
        &std::fs::read_to_string(&args.oracle)
            .with_context(|| format!("reading {}", args.oracle.display()))?,
    )
    .context("parsing the oracle's edge list")?;

    let started = std::time::Instant::now();
    let engine = build(&tree)?;
    let index = &engine.dependency_index;
    let id_of = |p: &Path| p.canonicalize().ok().and_then(|c| index.get_file_id(&c));
    let shown = |id: u32| {
        engine
            .display_path(id)
            .map(|p| p.into_owned())
            .unwrap_or_default()
    };
    eprintln!(
        "indexed {} files in {:.1} s",
        index.file_ids().count(),
        started.elapsed().as_secs_f64()
    );

    // The oracle's edges between indexed files, with an example import.
    let mut expected: BTreeMap<(u32, u32), (usize, String)> = BTreeMap::new();
    let mut oracle_unresolved = 0usize;
    let scope: BTreeSet<u32> = oracle.files.iter().filter_map(|p| id_of(p)).collect();
    for e in &oracle.edges {
        let Some(from) = id_of(&e.from) else {
            continue;
        };
        match e.to.as_deref().and_then(id_of) {
            Some(to) if to != from => {
                expected
                    .entry((from, to))
                    .or_insert_with(|| (e.line, e.spec.clone()));
            }
            Some(_) => {}
            None => oracle_unresolved += 1,
        }
    }
    let mut unjudged = 0usize;
    let actual: BTreeSet<(u32, u32)> = index
        .get_all_edges()
        .into_iter()
        .filter(|&(from, to)| {
            from != to && scope.contains(&from) && !index.is_containment(from, to)
        })
        .filter(|&(_, to)| {
            let ok = index.path_of(to).is_some_and(|p| judged(&args.lang, p));
            unjudged += usize::from(!ok);
            ok
        })
        .collect();
    let fcs_unresolved: usize = engine
        .waiting_imports_by_file()
        .iter()
        .filter(|(id, _, _)| scope.contains(id))
        .map(|(_, _, imports)| imports.len())
        .sum();

    let missing: Vec<_> = expected
        .iter()
        .filter(|(k, _)| !actual.contains(k))
        .collect();
    let extra: Vec<_> = actual
        .iter()
        .filter(|k| !expected.contains_key(k))
        .collect();
    let found = expected.len() - missing.len();

    let mut md = String::new();
    writeln!(md, "## Dependency graph oracle ({})", args.lang)?;
    writeln!(md)?;
    writeln!(
        md,
        "{}: the oracle draws {} edges, fcs draws {}. **{} missing, {} extra**; fcs finds {:.1}% of the oracle's edges and {:.1}% of its edges are the oracle's.",
        tree.file_name().unwrap_or_default().to_string_lossy(),
        expected.len(),
        actual.len(),
        missing.len(),
        extra.len(),
        100.0 * found as f64 / expected.len().max(1) as f64,
        100.0 * found as f64 / actual.len().max(1) as f64,
    )?;
    writeln!(
        md,
        "{} files compared. Imports left unresolved (external packages included): {oracle_unresolved} by the oracle, {fcs_unresolved} by fcs.",
        scope.len()
    )?;
    if unjudged > 0 {
        writeln!(
            md,
            "{unjudged} fcs edges to stylesheets, components and assets are not compared (the oracle cannot resolve them)."
        )?;
    }

    let mut by_shape: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for ((from, to), (line, spec)) in &missing {
        by_shape
            .entry(shape(&args.lang, spec))
            .or_default()
            .push(format!(
                "{}:{line}: {spec}  ->  {}",
                shown(*from),
                shown(*to)
            ));
    }
    if !missing.is_empty() {
        writeln!(md, "\n### Missing, by kind of import\n")?;
        writeln!(md, "| Kind | Edges |\n|---|---:|")?;
        for (k, v) in &by_shape {
            writeln!(md, "| {k} | {} |", v.len())?;
        }
        for (k, v) in &by_shape {
            writeln!(md, "\n{k}:\n\n```")?;
            for l in v.iter().take(args.samples) {
                writeln!(md, "{l}")?;
            }
            writeln!(md, "```")?;
        }
    }
    if !extra.is_empty() {
        writeln!(md, "\n### Extra (fcs draws, the oracle does not)\n\n```")?;
        for (from, to) in extra.iter().take(args.samples * 4) {
            writeln!(md, "{}  ->  {}", shown(*from), shown(*to))?;
        }
        writeln!(md, "```")?;
    }
    eprint!("{md}");
    if let Some(p) = &args.markdown {
        std::fs::write(p, &md)?;
    }

    let mut failed = Vec::new();
    if args.max_missing.is_some_and(|m| missing.len() > m) {
        failed.push(format!(
            "{} missing edges, more than --max-missing {}",
            missing.len(),
            args.max_missing.unwrap_or_default()
        ));
    }
    if args.max_extra.is_some_and(|m| extra.len() > m) {
        failed.push(format!(
            "{} extra edges, more than --max-extra {}",
            extra.len(),
            args.max_extra.unwrap_or_default()
        ));
    }
    if !failed.is_empty() {
        eprintln!("\n{}", failed.join("; "));
        std::process::exit(1);
    }
    Ok(())
}
