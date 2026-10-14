//! Reference oracle: does fcs record the references a language's own
//! analyser finds?
//!
//! Extracts references from every file the oracle analysed and compares
//! them with the oracle's list, written by a script in `src/check/oracle/`:
//!
//! - `py_refs.py`: Jedi, which resolves each name in a Python file to its
//!   definition; a use counts when it resolves to a function or class
//!   defined in the tree (import lines excluded).
//!
//! ```sh
//! pip install jedi==0.20.1
//! python3 src/check/oracle/py_refs.py TREE > py-refs.json
//! cargo run --release --example ref_oracle -- --oracle py-refs.json
//! ```
//!
//! A reference the oracle lists and fcs does not is a **miss**. A
//! reference fcs records, under the name of a function or class the oracle
//! saw, at a position the oracle does not list, is an **extra**: either a
//! use of something else with the same name (references are names, not
//! symbols) or a position that is no use at all. A reference at a use the
//! oracle could not resolve (an untyped receiver) counts as neither. Misses and extras are
//! grouped by the syntax around the name, which says which form of
//! `docs/book/src/reference/reference-support.md` each one is.
//! `--max-missing` and `--max-extra` fail the run above a count, so a
//! baseline can only go down.

use anyhow::{bail, Context, Result};
use clap::Parser;
use fast_code_search::symbols::SymbolExtractor;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

#[derive(Parser, Debug)]
struct Args {
    /// The oracle's reference list (JSON from src/check/oracle/).
    #[arg(long)]
    oracle: PathBuf,
    /// Fail when more references than this are missing.
    #[arg(long)]
    max_missing: Option<usize>,
    /// Fail when more references than this are extra.
    #[arg(long)]
    max_extra: Option<usize>,
    /// Example lines shown per group.
    #[arg(long, default_value_t = 3)]
    samples: usize,
    /// Groups shown for misses and for extras.
    #[arg(long, default_value_t = 12)]
    groups: usize,
    /// Write the markdown report here as well as to stderr.
    #[arg(long)]
    markdown: Option<PathBuf>,
}

#[derive(Deserialize)]
struct Oracle {
    files: Vec<String>,
    refs: Vec<OracleRef>,
    /// Uses the oracle could not resolve: not counted either way.
    #[serde(default)]
    unresolved: Vec<(String, u32, u32)>,
}

#[derive(Deserialize)]
struct OracleRef {
    file: String,
    line: u32,
    column: u32,
    name: String,
}

#[derive(Default)]
struct Group {
    count: usize,
    samples: Vec<String>,
}

/// The syntax around a name: its node's kind and its parent's and
/// grandparent's, e.g. `identifier < argument_list < call`.
fn shape(tree: &tree_sitter::Tree, line: u32, column: u32) -> String {
    let point = tree_sitter::Point::new(line as usize, column as usize);
    let Some(node) = tree.root_node().descendant_for_point_range(point, point) else {
        return "?".to_string();
    };
    let mut kinds = vec![node.kind().to_string()];
    let mut cur = node.parent();
    while let (Some(n), true) = (cur, kinds.len() < 3) {
        kinds.push(n.kind().to_string());
        cur = n.parent();
    }
    kinds.join(" < ")
}

fn add(groups: &mut BTreeMap<String, Group>, key: String, sample: String, max: usize) {
    let g = groups.entry(key).or_default();
    g.count += 1;
    if g.samples.len() < max {
        g.samples.push(sample);
    }
}

fn report(out: &mut String, title: &str, groups: &BTreeMap<String, Group>, args: &Args) {
    let mut v: Vec<_> = groups.iter().collect();
    v.sort_by(|a, b| b.1.count.cmp(&a.1.count).then(a.0.cmp(b.0)));
    out.push_str(&format!(
        "\n### {title}\n\n| Count | Shape | Examples |\n|---|---|---|\n"
    ));
    for (k, g) in v.into_iter().take(args.groups) {
        let ex = g.samples.join("<br>").replace('|', "\\|");
        out.push_str(&format!("| {} | `{}` | {} |\n", g.count, k, ex));
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let oracle: Oracle = serde_json::from_slice(
        &std::fs::read(&args.oracle).with_context(|| format!("reading {:?}", args.oracle))?,
    )?;
    let names: HashSet<&str> = oracle.refs.iter().map(|r| r.name.as_str()).collect();
    let mut want: HashMap<&str, HashSet<(u32, u32)>> = HashMap::new();
    for r in &oracle.refs {
        want.entry(r.file.as_str())
            .or_default()
            .insert((r.line, r.column));
    }

    let mut unknown: HashMap<&str, HashSet<(u32, u32)>> = HashMap::new();
    for (file, line, column) in &oracle.unresolved {
        unknown
            .entry(file.as_str())
            .or_default()
            .insert((*line, *column));
    }

    let (mut found, mut missing, mut extra, mut unsure) = (0usize, 0usize, 0usize, 0usize);
    let mut stored = 0usize;
    let mut missing_groups = BTreeMap::new();
    let mut extra_groups = BTreeMap::new();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_python::LANGUAGE.into())?;
    for file in &oracle.files {
        let Ok(source) = std::fs::read_to_string(file) else {
            continue;
        };
        let (_, _, refs) =
            SymbolExtractor::new(std::path::Path::new(file)).extract_all_with_refs(&source)?;
        stored += refs.len();
        let have: HashSet<(u32, u32)> = refs.iter().map(|r| (r.line, r.column)).collect();
        let lines: Vec<&str> = source.lines().collect();
        let Some(tree) = parser.parse(&source, None) else {
            continue;
        };
        let sample = |line: u32| {
            let short = file.rsplit('/').take(2).collect::<Vec<_>>();
            format!(
                "{}/{}:{} `{}`",
                short.get(1).unwrap_or(&""),
                short[0],
                line + 1,
                lines.get(line as usize).unwrap_or(&"").trim()
            )
        };
        let want_here = want.get(file.as_str());
        for &(line, column) in want_here.into_iter().flatten() {
            if have.contains(&(line, column)) {
                found += 1;
            } else {
                missing += 1;
                add(
                    &mut missing_groups,
                    shape(&tree, line, column),
                    sample(line),
                    args.samples,
                );
            }
        }
        for r in &refs {
            if !names.contains(r.name.as_str()) {
                continue;
            }
            if want_here.is_some_and(|w| w.contains(&(r.line, r.column))) {
                continue;
            }
            if unknown
                .get(file.as_str())
                .is_some_and(|u| u.contains(&(r.line, r.column)))
            {
                unsure += 1;
                continue;
            }
            extra += 1;
            add(
                &mut extra_groups,
                shape(&tree, r.line, r.column),
                sample(r.line),
                args.samples,
            );
        }
    }

    let total = found + missing;
    let pct = |n: usize, d: usize| {
        if d == 0 {
            100.0
        } else {
            100.0 * n as f64 / d as f64
        }
    };
    let mut out = format!(
        "## Reference oracle: {}\n\n{} files. The oracle lists {} references; fcs records {} of them ({:.1}%), misses {}, and records {} extra under the same names (precision {:.1}%). {} more fcs references are uses the oracle could not resolve. fcs stores {} references in these files.\n",
        args.oracle.display(),
        oracle.files.len(),
        total,
        found,
        pct(found, total),
        missing,
        extra,
        pct(found, found + extra),
        unsure,
        stored,
    );
    report(&mut out, "Missing", &missing_groups, &args);
    report(&mut out, "Extra", &extra_groups, &args);
    eprintln!("{out}");
    if let Some(p) = &args.markdown {
        std::fs::write(p, &out)?;
    }
    if args.max_missing.is_some_and(|m| missing > m) {
        bail!(
            "{missing} references missing, more than the baseline {}",
            args.max_missing.unwrap()
        );
    }
    if args.max_extra.is_some_and(|m| extra > m) {
        bail!(
            "{extra} extra references, more than the baseline {}",
            args.max_extra.unwrap()
        );
    }
    Ok(())
}
