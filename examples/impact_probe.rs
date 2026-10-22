//! Evidence for diff impact analysis: given a commit's diff, which symbols
//! changed, and which other files use them, by several strategies built
//! from what fcs already has (the import graph, references, text search).
//!
//! Indexes ONE repository at its working tree (HEAD), then for each of the
//! last N commits that touch Python or TypeScript:
//!
//! 1. reads `git diff -U0 C^ C`, parses the old and new file with
//!    tree-sitter, and lists the innermost definitions (function, method,
//!    class, module-level constant) whose lines changed;
//! 2. finds each one in HEAD by qualified name (`Class.method`), so it can
//!    be compared with an oracle run on HEAD;
//! 3. for each strategy, lists the files (other than the changed ones)
//!    reported as users, and how long the engine took.
//!
//! ```text
//! cargo run --release --example impact_probe -- --repo DIR --commits 300 \
//!     --out cases.jsonl [--index FILE] [--level2]
//! ```

use anyhow::{Context, Result};
use clap::Parser;
use fast_code_search::config::IndexerConfig;
use fast_code_search::search::engine::{
    PartialIndexedFile, PreIndexedFile, RankMode, SearchEngine, SearchLimits, SearchMatch,
};
use fast_code_search::search::{parse_query, FileDiscoveryConfig, FileDiscoveryIterator};
use rayon::prelude::*;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    repo: PathBuf,
    /// Extra folders indexed alongside the repo (scale: more files, same graph).
    #[arg(long)]
    extra: Vec<PathBuf>,
    #[arg(long, default_value_t = 300)]
    commits: usize,
    #[arg(long)]
    out: PathBuf,
    /// Load this index if it exists, else build and save it here.
    #[arg(long)]
    index: Option<PathBuf>,
    /// Also follow one more level: the definitions enclosing each use.
    #[arg(long)]
    level2: bool,
    /// Skip commits whose diff changes more than this many symbols.
    #[arg(long, default_value_t = 40)]
    max_symbols: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Def {
    qualname: String,
    name: String,
    kind: &'static str,
    /// 0-based rows, inclusive; `start` includes decorators.
    start: usize,
    end: usize,
    /// Row of the name (what an oracle reports as the definition line).
    name_row: usize,
    /// Last row of the header (before the body).
    header_end: usize,
}

fn lang_for(path: &str) -> Option<tree_sitter::Language> {
    let ext = Path::new(path).extension()?.to_str()?;
    Some(match ext {
        "py" | "pyi" => tree_sitter_python::LANGUAGE.into(),
        "ts" | "mts" | "cts" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
        "js" | "jsx" | "mjs" | "cjs" => tree_sitter_javascript::LANGUAGE.into(),
        _ => return None,
    })
}

fn node_text<'a>(n: tree_sitter::Node, src: &'a str) -> &'a str {
    &src[n.byte_range()]
}

/// Every definition in `src`, with spans.
fn definitions(path: &str, src: &str) -> Vec<Def> {
    let Some(lang) = lang_for(path) else {
        return Vec::new();
    };
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang).unwrap();
    let Some(tree) = parser.parse(src, None) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    walk(tree.root_node(), src, &mut Vec::new(), 0, &mut out);
    out
}

fn walk(
    n: tree_sitter::Node,
    src: &str,
    scope: &mut Vec<String>,
    depth: usize,
    out: &mut Vec<Def>,
) {
    let kind = n.kind();
    let def_kind: Option<&'static str> = match kind {
        "function_definition" | "function_declaration" | "generator_function_declaration" => {
            Some(if scope.is_empty() {
                "function"
            } else {
                "method"
            })
        }
        "method_definition" | "abstract_method_signature" => Some("method"),
        "class_definition" | "class_declaration" | "abstract_class_declaration" => Some("class"),
        "interface_declaration" | "type_alias_declaration" | "enum_declaration" => Some("type"),
        _ => None,
    };
    if let Some(k) = def_kind {
        if let Some(name_node) = n.child_by_field_name("name") {
            let name = node_text(name_node, src).to_string();
            let start = match n.parent() {
                Some(p) if p.kind() == "decorated_definition" => p.start_position().row,
                _ => n.start_position().row,
            };
            let header_end = n
                .child_by_field_name("body")
                .map_or(n.end_position().row, |b| b.start_position().row);
            let mut q = scope.clone();
            q.push(name.clone());
            out.push(Def {
                qualname: q.join("."),
                name: name.clone(),
                kind: k,
                start,
                end: n.end_position().row,
                name_row: name_node.start_position().row,
                header_end,
            });
            // Interfaces and type aliases are one symbol; don't descend.
            if k == "type" {
                return;
            }
            scope.push(name);
            let mut c = n.walk();
            for ch in n.children(&mut c) {
                walk(ch, src, scope, depth + 1, out);
            }
            scope.pop();
            return;
        }
    }
    // Module-level constants: `X = ...` (Python), `const x = ...` (TS/JS).
    if scope.is_empty() {
        let decl = match kind {
            "assignment"
                if n.parent().is_some_and(|p| {
                    p.kind() == "expression_statement"
                        && p.parent().is_some_and(|g| g.kind() == "module")
                }) =>
            {
                n.child_by_field_name("left")
                    .filter(|l| l.kind() == "identifier")
            }
            "variable_declarator" => n
                .child_by_field_name("name")
                .filter(|l| l.kind() == "identifier"),
            _ => None,
        };
        if let Some(name_node) = decl {
            // Only top level: the declaration's statement is a child of the
            // program (or of an export statement at the top).
            let stmt = if kind == "assignment" {
                n.parent()
            } else {
                n.parent()
            };
            let top = stmt.and_then(|s| s.parent()).is_some_and(|p| {
                matches!(p.kind(), "module" | "program")
                    || (p.kind() == "export_statement"
                        && p.parent().is_some_and(|g| g.kind() == "program"))
            });
            if top {
                let span_node = stmt.unwrap();
                out.push(Def {
                    qualname: node_text(name_node, src).to_string(),
                    name: node_text(name_node, src).to_string(),
                    kind: "variable",
                    start: span_node.start_position().row,
                    end: span_node.end_position().row,
                    name_row: name_node.start_position().row,
                    header_end: name_node.start_position().row,
                });
                // A `const f = () => {...}` is a function whose body we
                // also want to attribute to `f`; don't descend.
                return;
            }
        }
    }
    let mut c = n.walk();
    for ch in n.children(&mut c) {
        walk(ch, src, scope, depth, out);
    }
}

/// The innermost definition containing `row`.
fn innermost(defs: &[Def], row: usize) -> Option<&Def> {
    defs.iter()
        .filter(|d| d.start <= row && row <= d.end)
        .min_by_key(|d| d.end - d.start)
}

fn git(repo: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Changed rows per file: (old rows, new rows), 0-based.
fn diff_rows(diff: &str) -> BTreeMap<(Option<String>, Option<String>), (Vec<usize>, Vec<usize>)> {
    let mut out: BTreeMap<_, (Vec<usize>, Vec<usize>)> = BTreeMap::new();
    let (mut old, mut new): (Option<String>, Option<String>) = (None, None);
    for line in diff.lines() {
        if let Some(p) = line.strip_prefix("--- ") {
            old = p.strip_prefix("a/").map(String::from);
        } else if let Some(p) = line.strip_prefix("+++ ") {
            new = p.strip_prefix("b/").map(String::from);
        } else if let Some(h) = line.strip_prefix("@@ ") {
            // @@ -a,b +c,d @@
            let mut parts = h.split_whitespace();
            let parse = |s: &str| -> (usize, usize) {
                let s = &s[1..];
                let mut it = s.split(',');
                let a: usize = it.next().unwrap_or("0").parse().unwrap_or(0);
                let b: usize = it.next().map_or(1, |x| x.parse().unwrap_or(1));
                (a, b)
            };
            let (os, oc) = parse(parts.next().unwrap_or("-0"));
            let (ns, nc) = parse(parts.next().unwrap_or("+0"));
            let e = out.entry((old.clone(), new.clone())).or_default();
            e.0.extend((0..oc).map(|i| os + i - 1));
            e.1.extend((0..nc).map(|i| ns + i - 1));
            // A pure insertion or deletion still touches the definition it
            // lands in: record the row it sits at on the other side.
            if oc == 0 && os > 0 {
                e.0.push(os - 1);
            }
            if nc == 0 && ns > 0 {
                e.1.push(ns - 1);
            }
        }
    }
    out
}

#[derive(Debug, Clone)]
struct Changed {
    file: String,
    def: Def,
    status: &'static str,
    signature: bool,
}

fn changed_symbols(repo: &Path, commit: &str) -> Option<Vec<Changed>> {
    let diff = git(
        repo,
        &[
            "diff",
            "-U0",
            "--no-renames",
            &format!("{commit}^"),
            commit,
            "--",
            "*.py",
            "*.ts",
            "*.tsx",
        ],
    )?;
    let mut out: BTreeMap<(String, String), Changed> = BTreeMap::new();
    for ((old_path, new_path), (old_rows, new_rows)) in diff_rows(&diff) {
        let file = new_path.clone().or(old_path.clone())?;
        let old_src = old_path
            .as_ref()
            .and_then(|p| git(repo, &["show", &format!("{commit}^:{p}")]));
        let new_src = new_path
            .as_ref()
            .and_then(|p| git(repo, &["show", &format!("{commit}:{p}")]));
        let old_defs = old_src
            .as_deref()
            .map_or(Vec::new(), |s| definitions(&file, s));
        let new_defs = new_src
            .as_deref()
            .map_or(Vec::new(), |s| definitions(&file, s));
        let new_names: BTreeSet<&str> = new_defs.iter().map(|d| d.qualname.as_str()).collect();
        let old_names: BTreeSet<&str> = old_defs.iter().map(|d| d.qualname.as_str()).collect();
        for &r in &new_rows {
            if let Some(d) = innermost(&new_defs, r) {
                let status = if old_names.contains(d.qualname.as_str()) {
                    "modified"
                } else {
                    "added"
                };
                let e = out
                    .entry((file.clone(), d.qualname.clone()))
                    .or_insert(Changed {
                        file: file.clone(),
                        def: d.clone(),
                        status,
                        signature: false,
                    });
                e.signature |= r <= d.header_end;
            }
        }
        for &r in &old_rows {
            if let Some(d) = innermost(&old_defs, r) {
                let removed = !new_names.contains(d.qualname.as_str());
                let e = out
                    .entry((file.clone(), d.qualname.clone()))
                    .or_insert(Changed {
                        file: file.clone(),
                        def: d.clone(),
                        status: if removed { "removed" } else { "modified" },
                        signature: false,
                    });
                e.signature |= r <= d.header_end || removed;
            }
        }
    }
    Some(out.into_values().collect())
}

fn build(engine: &mut SearchEngine, files: &[PathBuf], config: &IndexerConfig) {
    for chunk in files.chunks(config.batch_size.max(1)) {
        let pre: Vec<PreIndexedFile> = chunk
            .par_iter()
            .filter_map(|p| {
                let (partial, _) = PartialIndexedFile::process(
                    p,
                    config.transcode_non_utf8,
                    config.max_file_size,
                )?;
                Some(PreIndexedFile::from_partial(partial, config.enable_symbols))
            })
            .collect();
        engine.index_batch(pre);
        engine.resolve_imports_incremental();
    }
    engine.resolve_imports();
    engine.finalize();
}

struct Probe<'a> {
    engine: &'a SearchEngine,
    root: PathBuf,
}

impl Probe<'_> {
    fn rel(&self, m: &SearchMatch) -> String {
        rel_of(
            &self.root,
            self.engine.get_file_path(m.file_id).unwrap_or_default(),
        )
    }

    /// Files (and the hits) for one query; Err text when fcs refuses it.
    fn files(&self, q: &str, refs: bool) -> (Result<BTreeMap<String, Vec<usize>>, String>, f64) {
        let parsed = parse_query(q);
        let t = Instant::now();
        let r = if refs {
            self.engine
                .search_references_parsed(&parsed, "", "", SearchLimits::exhaustive())
        } else {
            self.engine
                .search_parsed(&parsed, "", "", SearchLimits::exhaustive(), RankMode::Full)
        };
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let r = r.map_err(|e| e.to_string()).map(|(ms, _)| {
            let mut files: BTreeMap<String, Vec<usize>> = BTreeMap::new();
            for m in &ms {
                files
                    .entry(self.rel(m))
                    .or_default()
                    .push(m.line_number - 1);
            }
            files
        });
        (r, ms)
    }
}

/// A display path (`<root name>/rel`) as a path relative to the repo.
fn rel_of(root: &Path, display: String) -> String {
    let label = root
        .file_name()
        .map(|n| format!("{}/", n.to_string_lossy()));
    match label.as_deref().and_then(|l| display.strip_prefix(l)) {
        Some(rest) => rest.to_string(),
        None => display,
    }
}

/// Files with a whole-word match / a reference of `name`, by id.
fn id_set(engine: &SearchEngine, q: &str, refs: bool) -> (BTreeSet<u32>, f64) {
    let parsed = parse_query(q);
    let t = Instant::now();
    let r = if refs {
        engine.search_references_parsed(&parsed, "", "", SearchLimits::exhaustive())
    } else {
        engine.search_parsed(&parsed, "", "", SearchLimits::exhaustive(), RankMode::Full)
    };
    let ms = t.elapsed().as_secs_f64() * 1e3;
    let ids = r
        .map(|(m, _)| m.iter().map(|m| m.file_id).collect())
        .unwrap_or_default();
    (ids, ms)
}

fn is_entry(p: &str) -> bool {
    let n = p.rsplit('/').next().unwrap_or(p);
    matches!(
        n,
        "__init__.py" | "index.ts" | "index.tsx" | "index.js" | "mod.rs" | "lib.rs"
    )
}

/// Name-guided walk: importers of the anchor, transitively, but only
/// through files that mention `name` as a word (or package entry files,
/// which re-export). Returns the files reached that mention it.
fn guided(engine: &SearchEngine, anchor: u32, words: &BTreeSet<u32>) -> BTreeSet<u32> {
    let mut seen = BTreeSet::from([anchor]);
    let mut queue = std::collections::VecDeque::from([anchor]);
    while let Some(cur) = queue.pop_front() {
        for n in engine.get_dependents(cur) {
            if seen.contains(&n) {
                continue;
            }
            let entry = engine.get_file_path(n).is_some_and(|p| is_entry(&p));
            if words.contains(&n) || entry {
                seen.insert(n);
                queue.push_back(n);
            }
        }
    }
    seen.retain(|id| words.contains(id) && *id != anchor);
    seen
}

/// [`guided`], also following renames: when a file on the walk says
/// `NAME as OTHER` (an import or re-export alias), OTHER is followed too.
fn guided_alias(
    engine: &SearchEngine,
    root: &Path,
    anchor: u32,
    name: &str,
) -> (BTreeSet<u32>, Vec<String>) {
    let mut names = vec![name.to_string()];
    let mut words = id_set(engine, &format!("word:yes case:yes {name}"), false).0;
    let mut seen = BTreeSet::from([anchor]);
    let mut queue = std::collections::VecDeque::from([anchor]);
    while let Some(cur) = queue.pop_front() {
        // Renames in this file of any name being followed.
        if cur != anchor {
            let content = engine
                .get_file_path(cur)
                .and_then(|p| std::fs::read_to_string(root.join(rel_of(root, p))).ok());
            if let Some(content) = content {
                for n in names.clone() {
                    let re = regex::Regex::new(&format!(
                        r"\b{}\s+as\s+([A-Za-z_$][\w$]*)",
                        regex::escape(&n)
                    ))
                    .unwrap();
                    for cap in re.captures_iter(&content) {
                        let alias = cap[1].to_string();
                        if !names.contains(&alias) {
                            words.extend(
                                id_set(engine, &format!("word:yes case:yes {alias}"), false).0,
                            );
                            names.push(alias);
                        }
                    }
                }
            }
        }
        for n in engine.get_dependents(cur) {
            if seen.contains(&n) {
                continue;
            }
            let entry = engine.get_file_path(n).is_some_and(|p| is_entry(&p));
            if words.contains(&n) || entry {
                seen.insert(n);
                queue.push_back(n);
            }
        }
    }
    seen.retain(|id| words.contains(id) && *id != anchor);
    (seen, names)
}

/// [`guided`] without a whole-repo search first: a neighbour is tested
/// only when the walk reaches it, against the trigram candidates (an
/// in-memory bitmap), then its recorded references (in memory), and only
/// then its text. Cost follows the files the walk touches, not the number
/// of files in the repo that contain the name.
fn guided_lazy(
    engine: &SearchEngine,
    root: &Path,
    anchor: u32,
    name: &str,
) -> (BTreeSet<u32>, usize) {
    let cands = engine.text_candidates(&name.to_lowercase());
    let name_id = engine.reference_name_id(name);
    let re = regex::Regex::new(&format!(r"\b{}\b", regex::escape(name))).unwrap();
    let mut read = 0usize;
    let mut mentions = |id: u32| -> bool {
        if !cands.contains(id) {
            return false;
        }
        if name_id.is_some_and(|n| engine.references_of(id).iter().any(|r| r.name == n)) {
            return true;
        }
        read += 1;
        engine
            .get_file_path(id)
            .and_then(|p| std::fs::read_to_string(root.join(rel_of(root, p))).ok())
            .is_some_and(|c| re.is_match(&c))
    };
    let mut hit = BTreeSet::new();
    let mut seen = BTreeSet::from([anchor]);
    let mut queue = std::collections::VecDeque::from([anchor]);
    while let Some(cur) = queue.pop_front() {
        for n in engine.get_dependents(cur) {
            if !seen.insert(n) {
                continue;
            }
            let m = mentions(n);
            if m {
                hit.insert(n);
            }
            if m || engine.get_file_path(n).is_some_and(|p| is_entry(&p)) {
                queue.push_back(n);
            }
        }
    }
    (hit, read)
}

/// The class a method belongs to.
fn owner(c: &Changed) -> Option<String> {
    if c.def.kind != "method" {
        return None;
    }
    let q = &c.def.qualname;
    let parent = &q[..q.len().checked_sub(c.def.name.len() + 1)?];
    Some(parent.rsplit('.').next().unwrap_or(parent).to_string())
}

/// The name to look for: a constructor is used through its class.
fn search_name(c: &Changed) -> String {
    let n = &c.def.name;
    if (n == "__init__" || n == "constructor" || n == "__new__") && c.def.qualname.contains('.') {
        let q = &c.def.qualname;
        let parent = &q[..q.len() - n.len() - 1];
        return parent.rsplit('.').next().unwrap_or(parent).to_string();
    }
    n.clone()
}

fn main() -> Result<()> {
    let args = Args::parse();
    let root = args.repo.canonicalize()?;
    let mut roots = vec![root.to_string_lossy().into_owned()];
    for e in &args.extra {
        roots.push(e.canonicalize()?.to_string_lossy().into_owned());
    }
    let config = IndexerConfig {
        paths: roots.clone(),
        ..Default::default()
    };
    let mut engine = SearchEngine::new();
    for r in &roots {
        engine.add_root_path(Path::new(r));
    }
    let t = Instant::now();
    let loaded = match &args.index {
        Some(p) if p.exists() => {
            engine.load_index_with_reconciliation(p, &config)?;
            engine.finalize();
            true
        }
        _ => false,
    };
    if !loaded {
        let discovery = FileDiscoveryConfig {
            paths: roots.clone(),
            exclude_patterns: config.exclude_patterns.clone(),
            include_extensions: config.include_extensions.clone(),
            max_file_size: Some(config.max_file_size),
            respect_gitignore: config.respect_gitignore,
            ..Default::default()
        };
        let files: Vec<PathBuf> = FileDiscoveryIterator::new(&discovery).collect();
        build(&mut engine, &files, &config);
        if let Some(p) = &args.index {
            engine.save_index(p, &config)?;
        }
    }
    let stats = engine.get_stats();
    eprintln!(
        "index: {} files ({}) in {:.1}s",
        stats.num_files,
        if loaded { "loaded" } else { "built" },
        t.elapsed().as_secs_f64()
    );
    let probe = Probe {
        engine: &engine,
        root: root.clone(),
    };

    let log = git(
        &root,
        &[
            "log",
            "--no-merges",
            "--format=%H",
            "-n",
            &args.commits.to_string(),
            "--",
            "*.py",
            "*.ts",
            "*.tsx",
        ],
    )
    .context("git log")?;
    let mut out = std::fs::File::create(&args.out)?;
    let mut head_defs_cache: BTreeMap<String, Vec<Def>> = BTreeMap::new();
    for commit in log.lines() {
        let Some(changed) = changed_symbols(&root, commit) else {
            continue;
        };
        if changed.is_empty() || changed.len() > args.max_symbols {
            writeln!(
                out,
                "{}",
                json!({"commit": commit, "skipped": changed.len()})
            )?;
            continue;
        }
        let changed_files: BTreeSet<String> = changed.iter().map(|c| c.file.clone()).collect();
        let mut syms = Vec::new();
        let case_t = Instant::now();
        for c in &changed {
            let head_defs = head_defs_cache.entry(c.file.clone()).or_insert_with(|| {
                std::fs::read_to_string(root.join(&c.file))
                    .map(|s| definitions(&c.file, &s))
                    .unwrap_or_default()
            });
            let head = head_defs.iter().find(|d| d.qualname == c.def.qualname);
            let name = search_name(c);
            let anchor = &c.file;
            let mut strat = serde_json::Map::new();
            let queries = [
                ("module1", format!("rdeps:{anchor}:1 {name}"), false, true),
                (
                    "refs_rdeps1",
                    format!("rdeps:{anchor}:1 {name}"),
                    true,
                    false,
                ),
                (
                    "refs_rdeps_all",
                    format!("rdeps:{anchor}:* {name}"),
                    true,
                    false,
                ),
                ("refs_global", name.clone(), true, false),
                (
                    "word_rdeps1",
                    format!("rdeps:{anchor}:1 word:yes case:yes {name}"),
                    false,
                    false,
                ),
                (
                    "word_global",
                    format!("word:yes case:yes {name}"),
                    false,
                    false,
                ),
                (
                    "word_rdeps_all",
                    format!("rdeps:{anchor}:* word:yes case:yes {name}"),
                    false,
                    false,
                ),
                // A method's class, mentioned by name, in the same scope.
                (
                    "owner_word_rdeps_all",
                    match owner(c) {
                        Some(o) => format!("rdeps:{anchor}:* word:yes case:yes {o}"),
                        None => String::new(),
                    },
                    false,
                    false,
                ),
            ];
            for (label, q, refs, module_only) in queries {
                if q.is_empty() {
                    continue;
                }
                let (r, ms) = if module_only {
                    // The scope itself: every importer, whatever it uses.
                    let parsed = parse_query(&q);
                    let t = Instant::now();
                    let scope = engine.graph_scope(&parsed.graph);
                    let ms = t.elapsed().as_secs_f64() * 1e3;
                    let r = scope.map(|s| {
                        let s = s.expect("filter");
                        let mut files = BTreeMap::new();
                        for id in 0..stats.num_files as u32 * 2 {
                            if s.hops(id).is_some() && s.allows(id) {
                                if let Some(p) = engine.get_file_path(id) {
                                    files.insert(rel_of(&root, p), Vec::new());
                                }
                            }
                        }
                        files
                    });
                    (r, ms)
                } else {
                    probe.files(&q, refs)
                };
                let v = match r {
                    Ok(f) => {
                        let hits: usize = f.values().map(Vec::len).sum();
                        let files: Vec<&String> =
                            f.keys().filter(|k| !changed_files.contains(*k)).collect();
                        json!({"ms": ms, "files": files, "hits": hits})
                    }
                    Err(e) => json!({"ms": ms, "error": e}),
                };
                strat.insert(label.to_string(), v);
            }
            {
                let (words, word_ms) = id_set(&engine, &format!("word:yes case:yes {name}"), false);
                let (refs, refs_ms) = id_set(&engine, &name, true);
                let t = Instant::now();
                let reached = engine
                    .find_file_id(&root.join(&c.file).to_string_lossy())
                    .map(|a| guided(&engine, a, &words))
                    .unwrap_or_default();
                let walk_ms = t.elapsed().as_secs_f64() * 1e3;
                let names = |ids: &mut dyn Iterator<Item = u32>| -> Vec<String> {
                    ids.filter_map(|id| engine.get_file_path(id))
                        .map(|p| rel_of(&root, p))
                        .filter(|p| !changed_files.contains(p))
                        .collect()
                };
                let word_files = names(&mut reached.iter().copied());
                let ref_files = names(&mut reached.iter().copied().filter(|id| refs.contains(id)));
                let t = Instant::now();
                let (alias_reached, alias_names) = engine
                    .find_file_id(&root.join(&c.file).to_string_lossy())
                    .map(|a| guided_alias(&engine, &root, a, &name))
                    .unwrap_or_default();
                let alias_ms = t.elapsed().as_secs_f64() * 1e3;
                strat.insert(
                    "guided_alias".into(),
                    json!({"ms": alias_ms, "names": alias_names,
                    "files": names(&mut alias_reached.iter().copied())}),
                );
                // Today's file-level change impact: every transitive importer.
                let t = Instant::now();
                let mut all = BTreeSet::new();
                if let Some(a) = engine.find_file_id(&root.join(&c.file).to_string_lossy()) {
                    let mut queue = std::collections::VecDeque::from([a]);
                    all.insert(a);
                    while let Some(cur) = queue.pop_front() {
                        for n in engine.get_dependents(cur) {
                            if all.insert(n) {
                                queue.push_back(n);
                            }
                        }
                    }
                }
                strat.insert(
                    "module_all".into(),
                    json!({"ms": t.elapsed().as_secs_f64() * 1e3,
                    "files": names(&mut all.iter().copied())}),
                );
                let t = Instant::now();
                let (lazy, reads) = engine
                    .find_file_id(&root.join(&c.file).to_string_lossy())
                    .map(|a| guided_lazy(&engine, &root, a, &name))
                    .unwrap_or_default();
                strat.insert(
                    "guided_lazy".into(),
                    json!({"ms": t.elapsed().as_secs_f64() * 1e3,
                    "reads": reads, "files": names(&mut lazy.iter().copied())}),
                );
                strat.insert(
                    "guided_word".into(),
                    json!({"ms": word_ms + walk_ms, "walk_ms": walk_ms, "files": word_files}),
                );
                strat.insert("guided_refs".into(), json!({"ms": word_ms + refs_ms + walk_ms, "walk_ms": walk_ms, "files": ref_files}));
            }
            if args.level2 {
                // Definitions that use the symbol (in its importers), then
                // their own users among their importers.
                let (r, _) = probe.files(&format!("rdeps:{anchor}:1 {name}"), true);
                let t = Instant::now();
                let mut users: BTreeSet<(String, String, String)> = BTreeSet::new();
                for (f, rows) in r.unwrap_or_default() {
                    if changed_files.contains(&f) {
                        continue;
                    }
                    let src = std::fs::read_to_string(root.join(&f)).unwrap_or_default();
                    let defs = definitions(&f, &src);
                    for row in rows {
                        if let Some(d) = innermost(&defs, row) {
                            let fake = Changed {
                                file: f.clone(),
                                def: d.clone(),
                                status: "user",
                                signature: false,
                            };
                            users.insert((f.clone(), d.qualname.clone(), search_name(&fake)));
                        }
                    }
                }
                let parse_ms = t.elapsed().as_secs_f64() * 1e3;
                let mut files2: BTreeSet<String> = BTreeSet::new();
                for (f, _, n) in &users {
                    if let (Ok(fs), _) = probe.files(&format!("rdeps:{f}:1 {n}"), true) {
                        files2.extend(fs.into_keys());
                    }
                }
                strat.insert(
                    "level2".into(),
                    json!({"ms": t.elapsed().as_secs_f64() * 1e3, "parse_ms": parse_ms,
                           "user_defs": users.len(), "files": files2}),
                );
            }
            syms.push(json!({
                "file": c.file, "qualname": c.def.qualname, "name": c.def.name,
                "search_name": name, "kind": c.def.kind, "status": c.status,
                "signature": c.signature, "head_line": head.map(|d| d.name_row),
                "strategies": strat,
            }));
        }
        writeln!(
            out,
            "{}",
            json!({"commit": commit, "changed_files": changed_files,
                   "ms": case_t.elapsed().as_secs_f64() * 1e3, "symbols": syms})
        )?;
    }
    Ok(())
}
