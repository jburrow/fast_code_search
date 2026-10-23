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
//! indexed count, on both sides. The report groups misses by the shape of
//! the import.
//!
//! `--gaps benches/graph/known_gaps.toml --corpus NAME` holds the run to
//! the known gaps listed for that corpus, each tied to a row of
//! `docs/book/src/reference/graph-support.md`: every miss and extra must
//! match a listed gap, and each gap must match exactly its `count` edges.
//! A new miss fails the run (a bug, or a gap to document), and so does a
//! gap that shrank (a fix: lower its count or remove it). `--max-missing`
//! and `--max-extra` are the older, count-only check.

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
    /// Known gaps (TOML); with `--corpus`, every miss and extra must be one.
    #[arg(long, requires = "corpus")]
    gaps: Option<PathBuf>,
    /// This tree's name in `--gaps`.
    #[arg(long)]
    corpus: Option<String>,
}

#[derive(serde::Deserialize)]
struct GapList {
    gap: Vec<Gap>,
}

/// A known gap: edges of one corpus fcs misses (or draws wrongly), all
/// for one documented reason.
#[derive(serde::Deserialize)]
struct Gap {
    corpus: String,
    /// `missing` or `extra`.
    kind: String,
    /// Regexes on the import text (misses only), the importer and the
    /// target, as the report shows them; all given must match.
    spec: Option<String>,
    from: Option<String>,
    to: Option<String>,
    /// Exactly this many edges.
    count: usize,
    /// Why: the row of the graph-support page this is.
    doc: String,
}

struct GapMatcher {
    gap: Gap,
    spec: Option<regex::Regex>,
    from: Option<regex::Regex>,
    to: Option<regex::Regex>,
    found: usize,
}

impl GapMatcher {
    fn new(gap: Gap) -> Result<Self> {
        let re = |r: &Option<String>| -> Result<Option<regex::Regex>> {
            r.as_deref()
                .map(regex::Regex::new)
                .transpose()
                .context("a regex in --gaps")
        };
        Ok(Self {
            spec: re(&gap.spec)?,
            from: re(&gap.from)?,
            to: re(&gap.to)?,
            gap,
            found: 0,
        })
    }

    fn matches(&self, kind: &str, from: &str, spec: Option<&str>, to: &str) -> bool {
        let ok = |re: &Option<regex::Regex>, text: Option<&str>| {
            re.as_ref()
                .is_none_or(|re| text.is_some_and(|t| re.is_match(t)))
        };
        self.gap.kind == kind
            && ok(&self.spec, spec)
            && ok(&self.from, Some(from))
            && ok(&self.to, Some(to))
    }
}

/// Sort `report`'s misses and extras into `gaps`; returns what fits none,
/// and the markdown table of the gaps.
fn check_gaps(
    report: &fast_code_search::check::graph::GraphReport,
    gaps: &mut [GapMatcher],
) -> (Vec<String>, String) {
    let mut unlisted = Vec::new();
    for (from, line, spec, to) in &report.missing {
        match gaps
            .iter_mut()
            .find(|g| g.matches("missing", from, Some(spec), to))
        {
            Some(g) => g.found += 1,
            None => unlisted.push(format!("missing {from}:{line}: {spec}  ->  {to}")),
        }
    }
    for (from, to) in &report.extra {
        match gaps.iter_mut().find(|g| g.matches("extra", from, None, to)) {
            Some(g) => g.found += 1,
            None => unlisted.push(format!("extra {from}  ->  {to}")),
        }
    }
    let mut md =
        String::from("\n### Known gaps\n\n| Gap | Kind | Listed | Found |\n|---|---|---:|---:|\n");
    for g in gaps.iter() {
        md.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            g.gap.doc, g.gap.kind, g.gap.count, g.found
        ));
    }
    (unlisted, md)
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
    let mut md = report.markdown(
        &tree.file_name().unwrap_or_default().to_string_lossy(),
        args.samples,
    );

    let mut failed = Vec::new();
    if let (Some(path), Some(corpus_name)) = (&args.gaps, &args.corpus) {
        let list: GapList = toml::from_str(
            &std::fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))?,
        )
        .with_context(|| format!("parsing {}", path.display()))?;
        let mut gaps = list
            .gap
            .into_iter()
            .filter(|g| &g.corpus == corpus_name)
            .map(GapMatcher::new)
            .collect::<Result<Vec<_>>>()?;
        let (unlisted, table) = check_gaps(&report, &mut gaps);
        md.push_str(&table);
        if !unlisted.is_empty() {
            md.push_str("\n### Not a known gap\n\n```\n");
            for u in &unlisted {
                md.push_str(u);
                md.push('\n');
            }
            md.push_str("```\n");
            failed.push(format!(
                "{} edges are not a known gap (a bug, or a gap to list in {})",
                unlisted.len(),
                path.display()
            ));
        }
        for g in &gaps {
            if g.found != g.gap.count {
                failed.push(format!(
                    "{:?} ({}): {} edges, listed as {}{}",
                    g.gap.doc,
                    g.gap.kind,
                    g.found,
                    g.gap.count,
                    if g.found < g.gap.count {
                        " (fixed? lower the count, or remove the gap)"
                    } else {
                        ""
                    }
                ));
            }
        }
    }
    eprint!("{md}");
    if let Some(p) = &args.markdown {
        std::fs::write(p, &md)?;
    }

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
