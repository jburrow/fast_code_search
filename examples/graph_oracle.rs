//! Dependency-graph oracle: does fcs draw the import edges the language's
//! own resolver draws?
//!
//! Indexes a tree in-process and compares the graph's edges with an edge
//! list from an independent resolver, written by one of the scripts in
//! `src/check/oracle/` (the same comparison `fcs check` runs):
//!
//! - `ts.mjs`: TypeScript's `ts.resolveModuleName` under each file's
//!   `tsconfig.json` (`paths`, `baseUrl`, `extends`), for JS and TS;
//! - `py.py`: `grimp`, which resolves Python imports statically as Python
//!   would with the tree on `sys.path`.
//!
//! ```sh
//! (cd scripts/graph-oracle && npm ci)   # pins TypeScript
//! (cd scripts/graph-oracle && node ../../src/check/oracle/ts.mjs ../../TREE) > ts.json
//! cargo run --release --example graph_oracle -- TREE --oracle ts.json --lang ts
//! ```
//!
//! An edge the oracle draws and fcs does not is a **miss**; one fcs draws
//! and the oracle does not is an **extra** (a wrong edge). Only files fcs
//! indexed count, on both sides. `--max-missing` and `--max-extra` fail the
//! run above a count, so a baseline can only go down. The report groups
//! misses by the shape of the import, which says which gap in
//! `docs/book/src/reference/graph-support.md` each one is.

use anyhow::{Context, Result};
use clap::Parser;
use fast_code_search::check::graph::{compare, Lang, Oracle};
use fast_code_search::check::Corpus;
use std::path::PathBuf;

#[derive(Parser, Debug)]
struct Args {
    /// The tree to index (the one the oracle ran on).
    tree: PathBuf,
    /// The oracle's edge list (JSON from src/check/oracle/).
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

fn main() -> Result<()> {
    let args = Args::parse();
    let lang = Lang::parse(&args.lang)?;
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
    let corpus = Corpus::build_default(std::slice::from_ref(&tree))?;
    eprintln!(
        "indexed {} files in {:.1} s",
        corpus.indexed.len(),
        started.elapsed().as_secs_f64()
    );
    let report = compare(&corpus.engine, lang, &oracle);
    let md = report.markdown(
        &tree.file_name().unwrap_or_default().to_string_lossy(),
        args.samples,
    );
    eprint!("{md}");
    if let Some(p) = &args.markdown {
        std::fs::write(p, &md)?;
    }

    let mut failed = Vec::new();
    if let Some(m) = args.max_missing.filter(|&m| report.missing.len() > m) {
        failed.push(format!(
            "{} missing edges, more than --max-missing {m}",
            report.missing.len()
        ));
    }
    if let Some(m) = args.max_extra.filter(|&m| report.extra.len() > m) {
        failed.push(format!(
            "{} extra edges, more than --max-extra {m}",
            report.extra.len()
        ));
    }
    if !failed.is_empty() {
        eprintln!("\n{}", failed.join("; "));
        std::process::exit(1);
    }
    Ok(())
}
