//! Self-checks against independent tools, shared by `fcs check` and the CI
//! harnesses (`examples/rg_backtest.rs`, `examples/graph_oracle.rs`).
//!
//! - [`search`]: queries made up from the corpus, each compared with what
//!   `rg -F` finds on the same files;
//! - [`graph`]: the import graph compared with the edges the language's
//!   own resolver draws (grimp for Python, TypeScript's
//!   `resolveModuleName` for JS and TS).
//!
//! Both build a fresh in-memory index of the trees ([`Corpus`]) so what is
//! checked is the code, not a stale index.

pub mod graph;
pub mod search;

use crate::config::IndexerConfig;
use crate::search::{FileDiscoveryIterator, PartialIndexedFile, PreIndexedFile, SearchEngine};
use anyhow::Result;
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::PathBuf;

/// An in-memory index of some trees, built the way the server builds one.
pub struct Corpus {
    pub engine: SearchEngine,
    pub roots: Vec<PathBuf>,
    /// Display paths of every indexed file.
    pub indexed: HashSet<String>,
}

impl Corpus {
    /// Index `config.paths` (which must exist; canonical paths are best)
    /// with `config`'s discovery settings, then resolve the import graph.
    pub fn build(config: &IndexerConfig) -> Result<Self> {
        let roots: Vec<PathBuf> = config.paths.iter().map(PathBuf::from).collect();
        let discovery = config.discovery_config(config.paths.clone());
        let files: Vec<PathBuf> = FileDiscoveryIterator::new(&discovery).collect();
        let mut engine = SearchEngine::new();
        for r in &roots {
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
        Ok(Self {
            engine,
            roots,
            indexed,
        })
    }

    /// Index `roots` with the default settings.
    pub fn build_default(roots: &[PathBuf]) -> Result<Self> {
        Self::build(&IndexerConfig {
            paths: roots
                .iter()
                .map(|r| r.to_string_lossy().into_owned())
                .collect(),
            ..Default::default()
        })
    }

    /// The content of the file shown as `file` (a display path).
    pub fn read_display(&self, file: &str) -> Option<String> {
        // Display paths start with the root's own name.
        let (root_name, rel) = file.split_once('/').unwrap_or(("", file));
        self.roots
            .iter()
            .find(|r| {
                r.file_name()
                    .is_some_and(|n| n.to_string_lossy() == root_name)
            })
            .and_then(|r| std::fs::read(r.join(rel)).ok())
            .map(|b| String::from_utf8_lossy(&b).into_owned())
    }
}

/// What `fcs check` runs.
#[derive(Debug, Clone)]
pub struct CheckOptions {
    /// Generated queries to compare with rg (0 = no search check).
    pub queries: usize,
    /// Seed for the queries (`None` = a fresh one, printed).
    pub seed: Option<u64>,
    /// The ripgrep binary.
    pub rg: String,
    /// Run the graph oracles that are installed.
    pub graph: bool,
    /// Examples shown per difference.
    pub samples: usize,
}

impl Default for CheckOptions {
    fn default() -> Self {
        Self {
            queries: 200,
            seed: None,
            rg: "rg".into(),
            graph: true,
            samples: 5,
        }
    }
}

/// `fcs check`: index `config.paths` in memory, compare generated queries
/// with `rg -F` and the import graph with the installed oracles, and write
/// a markdown report to `out`. Returns the exit status: 1 when a search
/// differs from rg (a bug: please report it), else 0. Graph differences
/// are reported but do not fail: the support matrix lists known gaps.
pub fn run_check(
    config: &IndexerConfig,
    opts: &CheckOptions,
    out: &mut dyn std::io::Write,
    note: &dyn Fn(&str),
) -> Result<i32> {
    let mut config = config.clone();
    config.paths = config
        .paths
        .iter()
        .filter_map(|p| {
            std::fs::canonicalize(crate::config::expand_path(p, None))
                .map_err(|_| note(&format!("skipping {p}: not found")))
                .ok()
        })
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    if config.paths.is_empty() {
        anyhow::bail!("nothing to check: no indexed paths in the configuration (or --path)");
    }
    let started = std::time::Instant::now();
    note(&format!("indexing {} in memory…", config.paths.join(", ")));
    let corpus = Corpus::build(&config)?;
    note(&format!(
        "indexed {} files in {:.1} s",
        corpus.indexed.len(),
        started.elapsed().as_secs_f64()
    ));
    let mut code = 0;
    let mut md = String::from("# fcs check\n\n");
    let invariants = corpus.engine.check_invariants();
    if !invariants.is_clean() {
        code = 1;
        md += &format!(
            "## Index self-check\n\nA fresh index of these files does not agree with itself; \
             please report this.\n\n```\n{invariants}```\n\n"
        );
    }

    if opts.queries > 0 {
        md += "## Search vs ripgrep\n\n";
        if !search::rg_available(&opts.rg) {
            md += "Skipped: ripgrep (`rg`) is not installed.\n\n";
        } else {
            let seed = opts.seed.unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            });
            let queries = search::generate(&corpus, opts.queries, seed);
            note(&format!(
                "comparing {} generated queries with rg -F (seed {seed})…",
                queries.len()
            ));
            let mut differing = Vec::new();
            for q in &queries {
                let cmp = search::compare(&corpus, &opts.rg, q)?;
                if cmp.differs() {
                    differing.push((q.clone(), cmp));
                }
            }
            md += &format!(
                "{} queries made up from the files (seed {seed}; repeat with `--seed {seed}`): **{} differ** from `rg -F`.\n",
                queries.len(),
                differing.len()
            );
            if !differing.is_empty() {
                code = 1;
                md += "\nfcs should find exactly the lines ripgrep finds. Each query below is a bug; please report it with the shrunk query.\n";
                let mut cache = std::collections::HashMap::new();
                for (q, cmp) in &differing {
                    let small = search::shrink(&corpus, &opts.rg, q)?;
                    let flags = small.flags();
                    md += &format!(
                        "\n### `{}`{}\n\nrg finds {} lines, fcs {}.\n",
                        small.q.replace('`', "'"),
                        if flags.is_empty() {
                            String::new()
                        } else {
                            format!(" ({flags})")
                        },
                        search::count(&cmp.rg),
                        search::count(&cmp.fcs)
                    );
                    for (label, set) in [
                        ("Missing (rg finds, fcs does not)", &cmp.missing),
                        ("Extra (fcs finds, rg does not)", &cmp.extra),
                    ] {
                        if set.is_empty() {
                            continue;
                        }
                        md += &format!("\n{label}: {}\n\n```\n", set.len());
                        for (f, l) in set.iter().take(opts.samples) {
                            md += &search::line_text(&corpus, &mut cache, f, *l);
                            md += "\n";
                        }
                        md += "```\n";
                    }
                }
            }
            md += "\n";
        }
    }

    if opts.graph {
        for lang in [graph::Lang::Python, graph::Lang::Ts] {
            let has_sources = corpus.engine.dependency_index.file_ids().any(|id| {
                corpus
                    .engine
                    .dependency_index
                    .path_of(id)
                    .is_some_and(|p| lang.is_source(p))
            });
            if !has_sources {
                continue;
            }
            let mut oracle = graph::Oracle::default();
            let mut skipped = None;
            for root in &corpus.roots {
                note(&format!(
                    "resolving {} imports in {} with the language's own resolver…",
                    lang.name(),
                    root.display()
                ));
                match graph::run_oracle(lang, root)? {
                    Ok(o) => {
                        oracle.files.extend(o.files);
                        oracle.edges.extend(o.edges);
                    }
                    Err(graph::Unavailable(why)) => {
                        skipped = Some(why);
                        break;
                    }
                }
            }
            if let Some(why) = skipped {
                md += &format!(
                    "## Dependency graph ({})\n\nSkipped: the check {why}.\n\n",
                    lang.name()
                );
                continue;
            }
            if oracle.files.is_empty() {
                md += &format!(
                    "## Dependency graph ({})\n\nSkipped: the resolver found no files to analyse{}.\n\n",
                    lang.name(),
                    if lang == graph::Lang::Python {
                        " (it looks for packages directly under each indexed folder or its src/)"
                    } else {
                        ""
                    }
                );
                continue;
            }
            let report = graph::compare(&corpus.engine, lang, &oracle);
            md += &report.markdown(lang.name(), opts.samples);
            md += "\nMissing or extra edges are either a gap listed in the graph support matrix (docs: Reference, Dependency graph support) or a bug worth reporting.\n\n";
        }
    }
    out.write_all(md.as_bytes())?;
    Ok(code)
}
