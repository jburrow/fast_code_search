//! The import graph against the language's own resolver: does fcs draw the
//! edges grimp (Python) or TypeScript's `resolveModuleName` (JS and TS)
//! draws?
//!
//! The oracles are the scripts in `src/check/oracle/`, embedded here so
//! `fcs check` can run them; each prints `{"files": [...], "edges": [{from,
//! line, spec, to}]}` with absolute paths. An edge the oracle draws and fcs
//! does not is a **miss**; one fcs draws and the oracle does not is an
//! **extra** (a wrong edge). Only the files the oracle analysed count, on
//! both sides. Misses are grouped by the shape of the import, which says
//! which gap in `docs/book/src/reference/graph-support.md` each one is.

use crate::search::SearchEngine;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// grimp's edges for every package directly under ROOT or ROOT/src.
pub const PY_ORACLE: &str = include_str!("oracle/py.py");
/// TypeScript's `resolveModuleName` for every JS/TS file under ROOT.
pub const TS_ORACLE: &str = include_str!("oracle/ts.mjs");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// JavaScript and TypeScript.
    Ts,
    Python,
}

impl Lang {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "ts" => Ok(Self::Ts),
            "py" => Ok(Self::Python),
            _ => bail!("the language must be ts or py"),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Ts => "JavaScript and TypeScript",
            Self::Python => "Python",
        }
    }

    fn short(self) -> &'static str {
        match self {
            Self::Ts => "ts",
            Self::Python => "py",
        }
    }

    /// Whether fcs indexes source files of this language at `p`.
    pub fn is_source(self, p: &Path) -> bool {
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        match self {
            Self::Ts => matches!(
                ext,
                "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts"
            ),
            Self::Python => matches!(ext, "py" | "pyi" | "pyw"),
        }
    }

    /// Whether the oracle can judge an edge to `to`. TypeScript resolves
    /// only script and JSON modules; an import of a stylesheet, a `.vue` or
    /// `.svelte` component or an asset is a real dependency that fcs draws
    /// and a bundler resolves, but TypeScript cannot, so those edges are
    /// counted apart.
    fn judged(self, to: &Path) -> bool {
        self == Self::Python
            || to.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                matches!(
                    e,
                    "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts" | "json"
                )
            })
    }

    /// The shape of an import, for grouping misses.
    fn shape(self, spec: &str) -> &'static str {
        let s = spec.trim();
        if self == Self::Python {
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
}

/// An oracle's answer.
#[derive(Deserialize, Default)]
pub struct Oracle {
    /// The files the oracle analysed; only their imports are compared.
    pub files: Vec<PathBuf>,
    pub edges: Vec<OracleEdge>,
}

#[derive(Deserialize)]
pub struct OracleEdge {
    pub from: PathBuf,
    pub line: usize,
    pub spec: String,
    pub to: Option<PathBuf>,
}

/// Why an oracle could not run (not an error: the tool is optional).
#[derive(Debug)]
pub struct Unavailable(pub String);

/// Run the embedded oracle for `lang` on `root`. `Ok(Err(_))` when the
/// oracle's tool (python3 with grimp, node with typescript) is missing.
pub fn run_oracle(lang: Lang, root: &Path) -> Result<std::result::Result<Oracle, Unavailable>> {
    let (programs, args, script): (&[&str], &[&str], &str) = match lang {
        Lang::Python => (&["python3", "python"], &["-"], PY_ORACLE),
        Lang::Ts => (&["node"], &["--input-type=module", "-"], TS_ORACLE),
    };
    // The script's way of saying its library is not installed.
    let missing = |err: &str, code: Option<i32>| match lang {
        Lang::Python => err.contains("No module named 'grimp'"),
        Lang::Ts => code == Some(3),
    };
    let tool = match lang {
        Lang::Python => "python3 with grimp (pip install grimp)",
        Lang::Ts => "node with typescript in the repository's node_modules",
    };
    for program in programs {
        let child = Command::new(program)
            .args(args)
            .arg(root)
            .current_dir(root)
            .env("FCS_ORACLE_ROOT", root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let Ok(mut child) = child else {
            continue; // not installed under this name
        };
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(script.as_bytes())?;
        }
        let out = child.wait_with_output()?;
        let err = String::from_utf8_lossy(&out.stderr);
        if !out.status.success() {
            if missing(&err, out.status.code()) {
                return Ok(Err(Unavailable(format!("needs {tool}"))));
            }
            bail!(
                "the {} oracle failed on {}: {}",
                lang.name(),
                root.display(),
                err.trim()
            );
        }
        let oracle = serde_json::from_slice(&out.stdout)
            .with_context(|| format!("parsing the {} oracle's output", lang.name()))?;
        return Ok(Ok(oracle));
    }
    Ok(Err(Unavailable(format!("needs {tool}"))))
}

/// A missed edge: importer, line, import text, target (display paths).
pub type Miss = (String, usize, String, String);

/// fcs's graph against an oracle's.
pub struct GraphReport {
    pub lang: Lang,
    /// Files compared (analysed by the oracle and indexed by fcs).
    pub files: usize,
    /// Edges the oracle draws between indexed files.
    pub expected: usize,
    /// Edges fcs draws from the compared files (judged ones).
    pub actual: usize,
    pub missing: Vec<Miss>,
    /// fcs draws, the oracle does not (importer, target).
    pub extra: Vec<(String, String)>,
    pub oracle_unresolved: usize,
    pub fcs_unresolved: usize,
    /// fcs edges to stylesheets, components and assets, not compared.
    pub unjudged: usize,
}

/// Compare `engine`'s import graph with `oracle`'s edges.
pub fn compare(engine: &SearchEngine, lang: Lang, oracle: &Oracle) -> GraphReport {
    let index = &engine.dependency_index;
    let id_of = |p: &Path| p.canonicalize().ok().and_then(|c| index.get_file_id(&c));
    let shown = |id: u32| {
        engine
            .display_path(id)
            .map(|p| p.into_owned())
            .unwrap_or_default()
    };

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
            let ok = index.path_of(to).is_some_and(|p| lang.judged(p));
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

    let missing = expected
        .iter()
        .filter(|(k, _)| !actual.contains(k))
        .map(|((from, to), (line, spec))| (shown(*from), *line, spec.clone(), shown(*to)))
        .collect();
    let extra = actual
        .iter()
        .filter(|k| !expected.contains_key(k))
        .map(|(from, to)| (shown(*from), shown(*to)))
        .collect();
    GraphReport {
        lang,
        files: scope.len(),
        expected: expected.len(),
        actual: actual.len(),
        missing,
        extra,
        oracle_unresolved,
        fcs_unresolved,
        unjudged,
    }
}

impl GraphReport {
    /// Edges both draw.
    pub fn found(&self) -> usize {
        self.expected - self.missing.len()
    }

    /// The report as markdown, with up to `samples` examples per group.
    pub fn markdown(&self, title: &str, samples: usize) -> String {
        let mut md = String::new();
        let _ = self.write(&mut md, title, samples);
        md
    }

    fn write(&self, md: &mut String, title: &str, samples: usize) -> std::fmt::Result {
        writeln!(md, "## Dependency graph oracle ({})", self.lang.short())?;
        writeln!(md)?;
        writeln!(
            md,
            "{title}: the oracle draws {} edges, fcs draws {}. **{} missing, {} extra**; fcs finds {:.1}% of the oracle's edges and {:.1}% of its edges are the oracle's.",
            self.expected,
            self.actual,
            self.missing.len(),
            self.extra.len(),
            100.0 * self.found() as f64 / self.expected.max(1) as f64,
            100.0 * self.found() as f64 / self.actual.max(1) as f64,
        )?;
        writeln!(
            md,
            "{} files compared. Imports left unresolved (external packages included): {} by the oracle, {} by fcs.",
            self.files, self.oracle_unresolved, self.fcs_unresolved
        )?;
        if self.unjudged > 0 {
            writeln!(
                md,
                "{} fcs edges to stylesheets, components and assets are not compared (the oracle cannot resolve them).",
                self.unjudged
            )?;
        }
        let mut by_shape: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for (from, line, spec, to) in &self.missing {
            by_shape
                .entry(self.lang.shape(spec))
                .or_default()
                .push(format!("{from}:{line}: {spec}  ->  {to}"));
        }
        if !self.missing.is_empty() {
            writeln!(md, "\n### Missing, by kind of import\n")?;
            writeln!(md, "| Kind | Edges |\n|---|---:|")?;
            for (k, v) in &by_shape {
                writeln!(md, "| {k} | {} |", v.len())?;
            }
            for (k, v) in &by_shape {
                writeln!(md, "\n{k}:\n\n```")?;
                for l in v.iter().take(samples) {
                    writeln!(md, "{l}")?;
                }
                writeln!(md, "```")?;
            }
        }
        if !self.extra.is_empty() {
            writeln!(md, "\n### Extra (fcs draws, the oracle does not)\n\n```")?;
            for (from, to) in self.extra.iter().take(samples * 4) {
                writeln!(md, "{from}  ->  {to}")?;
            }
            writeln!(md, "```")?;
        }
        Ok(())
    }
}
