//! A seeded generator of Python codebases that come with an answer key.
//!
//! The generator writes packages nested a few levels deep, each with an
//! `__init__.py` and some modules. Modules define functions, classes and
//! methods with names unique in the whole tree (`fn_0042`, `Cls0043`,
//! `meth_0044`, `deco_0045`), import each other in every form fcs claims to
//! handle (`import a.b`, `import a.b as x`, `from a import b`, `from a.b
//! import c, d`, parenthesised lists over several lines, `from .x import y`,
//! `from .. import y`, `from ...x import y`, `from . import m`, several
//! modules on one line, imports inside functions and under `if
//! TYPE_CHECKING:`), and use what they import as calls, attribute calls,
//! `await`, instantiations, method calls (`obj.m()`, `Cls().m()`,
//! `Cls.m(x)`, `super().m()`), decorators on functions and classes, base
//! classes, annotations (`C`, `list[C]`, `C | None`, `dict[str, C]`),
//! `raise` and `except`, and values (lists, dicts, keyword arguments,
//! comparisons, `isinstance`). Names also appear where they are *not*
//! references (strings, comments, docstrings, string annotations), which
//! the key leaves out. Import cycles happen.
//!
//! Because the generator writes every line, it knows the answer to each
//! question fcs answers about the tree: the import edges (with their
//! lines), where each name is defined, and every line that uses a name in
//! a position fcs documents as a reference. The same seed gives the same
//! tree, so a failure names the seed that reproduces it.
//!
//! Used by `tests/python_codebase.rs`; scale tests can include it with
//! `#[path = "../tests/support/pygen.rs"] mod pygen;`.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// SplitMix64: small, seedable, and the same on every platform.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `0..n` (`n` > 0).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    /// A number in `lo..=hi`.
    pub fn range(&mut self, lo: usize, hi: usize) -> usize {
        lo + self.below(hi - lo + 1)
    }

    pub fn chance(&mut self, p: f64) -> bool {
        ((self.next_u64() >> 11) as f64 / (1u64 << 53) as f64) < p
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// How big a tree to make.
#[derive(Debug, Clone)]
pub struct Options {
    pub seed: u64,
    /// Packages, including the top one (`gen`).
    pub packages: usize,
    /// Modules per package besides `__init__.py`.
    pub modules_per_package: (usize, usize),
    /// Deepest package nesting (`gen` is depth 1).
    pub max_depth: usize,
    /// Imports per module at module level.
    pub imports_per_module: (usize, usize),
}

impl Options {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            packages: 12,
            modules_per_package: (1, 4),
            max_depth: 4,
            imports_per_module: (0, 4),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefKind {
    Function,
    Class,
    Method,
}

/// An import edge: `from` imports `to` at `line` (paths relative to the
/// tree's root, with `/`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Edge {
    pub from: String,
    pub line: usize,
    pub spec: String,
    pub to: String,
}

/// A generated tree and its answer key.
#[derive(Debug, Default)]
pub struct Codebase {
    /// Relative path -> content.
    pub files: BTreeMap<String, String>,
    pub edges: Vec<Edge>,
    /// Name -> where it is defined (path, line) and what it is.
    pub defs: BTreeMap<String, (String, usize, DefKind)>,
    /// Name -> every (path, line) using it in a reference position.
    pub refs: BTreeMap<String, BTreeSet<(String, usize)>>,
}

impl Codebase {
    /// Write the files under `root`.
    pub fn write(&self, root: &Path) {
        for (rel, text) in &self.files {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
    }

    /// The modules each module imports, directly (relative paths).
    pub fn imports(&self) -> BTreeMap<&str, BTreeSet<&str>> {
        let mut out: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for e in &self.edges {
            if e.from != e.to {
                out.entry(&e.from).or_default().insert(&e.to);
            }
        }
        out
    }
}

/// What a module defines.
#[derive(Debug, Clone)]
enum Item {
    Function {
        name: String,
        is_async: bool,
    },
    Decorator {
        name: String,
    },
    /// `error`: derives from `Exception`, so it can be raised and caught.
    Class {
        name: String,
        methods: Vec<String>,
        error: bool,
    },
}

impl Item {
    fn name(&self) -> &str {
        match self {
            Item::Function { name, .. } | Item::Decorator { name } | Item::Class { name, .. } => {
                name
            }
        }
    }
    fn is_class(&self) -> bool {
        matches!(self, Item::Class { .. })
    }
}

/// A module being generated.
#[derive(Debug, Clone)]
struct Module {
    /// `gen.p1.m0`, or `gen.p1` for its `__init__.py`.
    dotted: String,
    path: String,
    /// The package the module's relative imports start from: the module's
    /// own package (an `__init__.py` is its package).
    package: String,
    is_init: bool,
    /// Decorators first (they are defined before they are used), then
    /// functions and classes in the order they are written.
    items: Vec<Item>,
}

impl Module {
    fn leaf(&self) -> &str {
        self.dotted.rsplit('.').next().unwrap()
    }
}

/// Something a module can use, reached through `prefix` (`""`, `m0.`,
/// `gen.p1.m0.`, `alias.`).
#[derive(Debug, Clone)]
struct Handle {
    prefix: String,
    item: Item,
}

impl Handle {
    fn expr(&self) -> String {
        format!("{}{}", self.prefix, self.item.name())
    }
}

/// Lines of one file, numbered as they are pushed.
struct Lines {
    path: String,
    lines: Vec<String>,
}

impl Lines {
    fn push(&mut self, line: impl Into<String>) -> usize {
        self.lines.push(line.into());
        self.lines.len()
    }
}

pub fn generate(opts: &Options) -> Codebase {
    let mut rng = Rng::new(opts.seed);

    // Packages: `gen`, then each new one under a random shallower one.
    let mut packages: Vec<(String, usize)> = vec![("gen".to_string(), 1)];
    for i in 1..opts.packages {
        let parents: Vec<&(String, usize)> = packages
            .iter()
            .filter(|(_, d)| *d < opts.max_depth)
            .collect();
        let (parent, depth) = (*rng.pick(&parents)).clone();
        packages.push((format!("{parent}.p{i}"), depth + 1));
    }

    // Modules, with their definitions decided up front so importers know
    // what each one offers.
    let mut uid = 0usize;
    let mut next = |prefix: &str| {
        uid += 1;
        format!("{prefix}{uid:04}")
    };
    let mut modules: Vec<Module> = Vec::new();
    for (pkg, _) in &packages {
        let dir = pkg.replace('.', "/");
        let count = rng.range(opts.modules_per_package.0, opts.modules_per_package.1);
        let mut names = vec![(pkg.clone(), format!("{dir}/__init__.py"), true)];
        for j in 0..count {
            names.push((format!("{pkg}.m{j}"), format!("{dir}/m{j}.py"), false));
        }
        for (dotted, path, is_init) in names {
            let mut items = Vec::new();
            if rng.chance(0.3) {
                items.push(Item::Decorator {
                    name: next("deco_"),
                });
            }
            let mut rest = Vec::new();
            for _ in 0..rng.range(1, 3) {
                rest.push(Item::Function {
                    name: next("fn_"),
                    is_async: rng.chance(0.2),
                });
            }
            for _ in 0..rng.range(0, 2) {
                let error = rng.chance(0.25);
                let name = next(if error { "Err" } else { "Cls" });
                let methods = (0..rng.range(1, 2)).map(|_| next("meth_")).collect();
                rest.push(Item::Class {
                    name,
                    methods,
                    error,
                });
            }
            while !rest.is_empty() {
                let i = rng.below(rest.len());
                items.push(rest.remove(i));
            }
            modules.push(Module {
                dotted,
                path,
                package: pkg.clone(),
                is_init,
                items,
            });
        }
    }

    let mut cb = Codebase::default();
    for i in 0..modules.len() {
        let file = write_module(&modules, i, &mut rng, opts, &mut cb);
        cb.files.insert(modules[i].path.clone(), file);
    }
    cb
}

/// Everything `target` offers, through `prefix`.
fn offered(target: &Module, prefix: &str) -> Vec<Handle> {
    target
        .items
        .iter()
        .map(|item| Handle {
            prefix: prefix.to_string(),
            item: item.clone(),
        })
        .collect()
}

/// Pick 1-2 names `target` defines.
fn some_names(rng: &mut Rng, target: &Module) -> Vec<Handle> {
    let all = offered(target, "");
    let mut picked: Vec<Handle> = Vec::new();
    for _ in 0..rng.range(1, 2) {
        let h = rng.pick(&all).clone();
        if !picked.iter().any(|p| p.item.name() == h.item.name()) {
            picked.push(h);
        }
    }
    picked
}

/// The relative spelling of `target` from `from`'s package, when it is in
/// that package, its parent or its grandparent: (dots, rest), e.g.
/// (".", "m0"), ("..", ""), ("...", "m1").
fn relative(from: &Module, target: &Module) -> Option<(String, String)> {
    let (t_pkg, t_leaf) = if target.is_init {
        (target.dotted.as_str(), "")
    } else {
        (target.package.as_str(), target.leaf())
    };
    let mut pkg = from.package.as_str();
    let mut dots = ".".to_string();
    for _ in 0..3 {
        if t_pkg == pkg {
            return Some((dots, t_leaf.to_string()));
        }
        pkg = pkg.rsplit_once('.')?.0;
        dots.push('.');
    }
    None
}

/// The generator's answer key, filled in as lines are written.
struct Key<'a> {
    cb: &'a mut Codebase,
    path: String,
}

impl Key<'_> {
    fn edge(&mut self, line: usize, spec: impl Into<String>, to: &Module) {
        self.cb.edges.push(Edge {
            from: self.path.clone(),
            line,
            spec: spec.into(),
            to: to.path.clone(),
        });
    }
    fn uses(&mut self, name: &str, line: usize) {
        self.cb
            .refs
            .entry(name.to_string())
            .or_default()
            .insert((self.path.clone(), line));
    }
    fn defines(&mut self, name: &str, line: usize, kind: DefKind) {
        self.cb
            .defs
            .insert(name.to_string(), (self.path.clone(), line, kind));
    }
}

fn write_module(
    modules: &[Module],
    me: usize,
    rng: &mut Rng,
    opts: &Options,
    cb: &mut Codebase,
) -> String {
    let m = &modules[me];
    let mut out = Lines {
        path: m.path.clone(),
        lines: Vec::new(),
    };
    let mut key = Key {
        cb,
        path: m.path.clone(),
    };
    out.push(format!(
        "# Generated module {} (seed {}).",
        m.dotted, opts.seed
    ));
    out.push("from __future__ import annotations");
    out.push("");
    out.push("from typing import TYPE_CHECKING, Optional");
    out.push("");
    out.push("MARKER = \"marker\"");

    let mut handles: Vec<Handle> = Vec::new();
    // Classes imported for annotations only (under TYPE_CHECKING).
    let mut type_only: Vec<Handle> = Vec::new();
    let mut local_names: BTreeSet<String> = BTreeSet::new();
    let mut aliases = 0usize;
    let mut alias = || {
        aliases += 1;
        format!("mod_{aliases}")
    };

    // Module-level imports.
    let n_imports = rng.range(opts.imports_per_module.0, opts.imports_per_module.1);
    for _ in 0..n_imports {
        let t = rng.below(modules.len());
        if t == me {
            continue;
        }
        let target = &modules[t];
        match rng.below(10) {
            0 => {
                let spec = format!("import {}", target.dotted);
                let line = out.push(&spec);
                key.edge(line, spec, target);
                handles.extend(offered(target, &format!("{}.", target.dotted)));
            }
            1 => {
                let a = alias();
                let spec = format!("import {} as {a}", target.dotted);
                let line = out.push(&spec);
                key.edge(line, spec, target);
                handles.extend(offered(target, &format!("{a}.")));
            }
            2 => {
                // Two modules on one line: `import a.b, c.d as x`.
                let u = rng.below(modules.len());
                if u == me || u == t {
                    continue;
                }
                let other = &modules[u];
                let a = alias();
                let spec = format!("import {}, {} as {a}", target.dotted, other.dotted);
                let line = out.push(&spec);
                key.edge(line, spec.clone(), target);
                key.edge(line, spec, other);
                handles.extend(offered(target, &format!("{}.", target.dotted)));
                handles.extend(offered(other, &format!("{a}.")));
            }
            3 => {
                // `from parent import leaf`: a module, or a package's
                // `__init__.py` (`from gen import p1`).
                let Some((parent, leaf)) = target.dotted.rsplit_once('.') else {
                    continue;
                };
                if !local_names.insert(leaf.to_string()) {
                    continue;
                }
                let spec = format!("from {parent} import {leaf}");
                let line = out.push(&spec);
                key.edge(line, spec, target);
                handles.extend(offered(target, &format!("{leaf}.")));
            }
            4 | 5 => {
                let names = some_names(rng, target);
                let list: Vec<&str> = names.iter().map(|h| h.item.name()).collect();
                let line = if rng.chance(0.4) {
                    let line = out.push(format!("from {} import (", target.dotted));
                    for n in &list {
                        out.push(format!("    {n},"));
                    }
                    out.push(")");
                    line
                } else {
                    out.push(format!("from {} import {}", target.dotted, list.join(", ")))
                };
                key.edge(line, format!("from {} import …", target.dotted), target);
                handles.extend(names);
            }
            6 | 7 => {
                let Some((dots, rest)) = relative(m, target) else {
                    continue;
                };
                if rest.is_empty() {
                    // `from . import name` / `from .. import name`: names
                    // the package's `__init__.py` defines.
                    let names = some_names(rng, target);
                    let list: Vec<&str> = names.iter().map(|h| h.item.name()).collect();
                    let spec = format!("from {dots} import {}", list.join(", "));
                    let line = out.push(&spec);
                    key.edge(line, spec, target);
                    handles.extend(names);
                } else if rng.chance(0.5) && local_names.insert(rest.clone()) {
                    // `from . import m0`: the module itself.
                    let spec = format!("from {dots} import {rest}");
                    let line = out.push(&spec);
                    key.edge(line, spec, target);
                    handles.extend(offered(target, &format!("{rest}.")));
                } else {
                    let names = some_names(rng, target);
                    let list: Vec<&str> = names.iter().map(|h| h.item.name()).collect();
                    let spec = format!("from {dots}{rest} import {}", list.join(", "));
                    let line = out.push(&spec);
                    key.edge(line, spec, target);
                    handles.extend(names);
                }
            }
            _ => {
                // Under `if TYPE_CHECKING:`: a class for annotations only.
                let classes: Vec<Handle> = offered(target, "")
                    .into_iter()
                    .filter(|h| h.item.is_class())
                    .collect();
                if classes.is_empty() {
                    continue;
                }
                let class = rng.pick(&classes).clone();
                out.push("if TYPE_CHECKING:");
                let spec = format!("from {} import {}", target.dotted, class.item.name());
                let line = out.push(format!("    {spec}"));
                key.edge(line, spec, target);
                type_only.push(class);
            }
        }
    }
    out.push("");
    out.push(format!("__all__ = [{}]", {
        let names: Vec<String> = m
            .items
            .iter()
            .map(|i| format!("\"{}\"", i.name()))
            .collect();
        names.join(", ")
    }));

    for item in &m.items {
        out.push("");
        out.push("");
        match item {
            Item::Decorator { name } => {
                let line = out.push(format!("def {name}(func):"));
                key.defines(name, line, DefKind::Function);
                out.push("    return func");
            }
            Item::Function { name, is_async } => {
                decorate(rng, &handles, &mut out, &mut key);
                let classes: Vec<Handle> = handles
                    .iter()
                    .filter(|h| h.item.is_class())
                    .chain(type_only.iter())
                    .cloned()
                    .collect();
                let mut used: Vec<String> = Vec::new();
                let annotation = |rng: &mut Rng, used: &mut Vec<String>| {
                    let c = rng.pick(&classes);
                    let e = c.expr();
                    // A string annotation is a known gap: not a reference.
                    let (text, counts) = match rng.below(6) {
                        0 => (format!("list[{e}]"), true),
                        1 => (format!("{e} | None"), true),
                        2 => (format!("dict[str, {e}]"), true),
                        3 => (format!("Optional[{e}]"), true),
                        4 => (format!("\"{e}\""), false),
                        _ => (e, true),
                    };
                    if counts {
                        used.push(c.item.name().to_string());
                    }
                    text
                };
                let arg = if !classes.is_empty() && rng.chance(0.5) {
                    format!("arg: {} = None", annotation(rng, &mut used))
                } else {
                    "arg=None".to_string()
                };
                let ret = if !classes.is_empty() && rng.chance(0.3) {
                    annotation(rng, &mut used)
                } else {
                    "object".to_string()
                };
                let def = if *is_async { "async def" } else { "def" };
                let line = out.push(format!("{def} {name}({arg}) -> {ret}:"));
                key.defines(name, line, DefKind::Function);
                for u in used {
                    key.uses(&u, line);
                }
                if rng.chance(0.3) {
                    // A docstring naming something is not a use of it.
                    if let Some(h) = handles.first() {
                        out.push(format!(
                            "    \"\"\"Calls {} now and then.\"\"\"",
                            h.item.name()
                        ));
                    }
                }
                function_body(modules, me, rng, &handles, *is_async, &mut out, &mut key);
            }
            Item::Class {
                name,
                methods,
                error,
            } => {
                if rng.chance(0.2) {
                    decorate(rng, &handles, &mut out, &mut key);
                }
                let bases: Vec<Handle> = handles
                    .iter()
                    .filter(|h| matches!(&h.item, Item::Class { error: e, .. } if e == error))
                    .cloned()
                    .collect();
                let mut base_methods: Vec<String> = Vec::new();
                let line = if !bases.is_empty() && rng.chance(0.4) {
                    let b = rng.pick(&bases);
                    if let Item::Class { methods, .. } = &b.item {
                        base_methods = methods.clone();
                    }
                    let line = out.push(format!("class {name}({}):", b.expr()));
                    key.uses(b.item.name(), line);
                    line
                } else if *error {
                    out.push(format!("class {name}(Exception):"))
                } else {
                    out.push(format!("class {name}:"))
                };
                key.defines(name, line, DefKind::Class);
                let mut earlier: Vec<String> = Vec::new();
                for meth in methods {
                    let line = out.push(format!("    def {meth}(self, n=0):"));
                    key.defines(meth, line, DefKind::Method);
                    let funcs: Vec<&Handle> = handles
                        .iter()
                        .filter(|h| {
                            matches!(
                                h.item,
                                Item::Function {
                                    is_async: false,
                                    ..
                                }
                            )
                        })
                        .collect();
                    match rng.below(4) {
                        0 if !earlier.is_empty() => {
                            let e = rng.pick(&earlier).clone();
                            let line = out.push(format!("        return self.{e}(n)"));
                            key.uses(&e, line);
                        }
                        1 if !funcs.is_empty() => {
                            let f = (*rng.pick(&funcs)).clone();
                            let line = out.push(format!("        return {}(n)", f.expr()));
                            key.uses(f.item.name(), line);
                        }
                        2 if !base_methods.is_empty() => {
                            let b = rng.pick(&base_methods).clone();
                            let line = out.push(format!("        return super().{b}(n)"));
                            key.uses(&b, line);
                        }
                        _ => {
                            out.push("        return n  # not a call of anything");
                        }
                    }
                    earlier.push(meth.clone());
                }
            }
        }
        // What this module defined is usable below it.
        handles.push(Handle {
            prefix: String::new(),
            item: item.clone(),
        });
    }
    out.push("");
    out.lines.join("\n")
}

/// Now and then, a decorator line.
fn decorate(rng: &mut Rng, handles: &[Handle], out: &mut Lines, key: &mut Key) {
    let decorators: Vec<&Handle> = handles
        .iter()
        .filter(|h| matches!(h.item, Item::Decorator { .. }))
        .collect();
    if !decorators.is_empty() && rng.chance(0.4) {
        let d = rng.pick(&decorators);
        let line = out.push(format!("@{}", d.expr()));
        key.uses(d.item.name(), line);
    }
}

/// The statements of a function: uses of the handles in every reference
/// position, and now and then an import inside the function.
fn function_body(
    modules: &[Module],
    me: usize,
    rng: &mut Rng,
    handles: &[Handle],
    is_async: bool,
    out: &mut Lines,
    key: &mut Key,
) {
    let mut wrote = false;
    for n in 0..rng.range(1, 5) {
        if rng.chance(0.1) {
            // An import inside the function, then a call of what it
            // imported.
            let t = rng.below(modules.len());
            let target = &modules[t];
            let function = target.items.iter().find_map(|i| match i {
                Item::Function {
                    name,
                    is_async: false,
                } => Some(name.clone()),
                _ => None,
            });
            if let (true, Some(name)) = (t != me, function) {
                let spec = format!("from {} import {name}", target.dotted);
                let line = out.push(format!("    {spec}"));
                key.edge(line, spec, target);
                let line = out.push(format!("    v{n} = {name}(arg)"));
                key.uses(&name, line);
                wrote = true;
                continue;
            }
        }
        if handles.is_empty() {
            continue;
        }
        let h = rng.pick(handles).clone();
        let e = h.expr();
        let name = h.item.name().to_string();
        let pick = rng.below(8);
        match &h.item {
            Item::Class { methods, error, .. } => {
                let meth = methods[0].clone();
                match pick {
                    0 if *error => {
                        out.push("    try:");
                        let line = out.push(format!("        raise {e}()"));
                        key.uses(&name, line);
                        let line = out.push(format!("    except {e} as err{n}:"));
                        key.uses(&name, line);
                        out.push(format!("        arg = err{n}"));
                    }
                    0 | 1 => {
                        let line = out.push(format!("    o{n} = {e}()"));
                        key.uses(&name, line);
                        let line = out.push(format!("    o{n}.{meth}(1)"));
                        key.uses(&meth, line);
                    }
                    2 => {
                        let line = out.push(format!("    r{n} = {e}().{meth}()"));
                        key.uses(&name, line);
                        key.uses(&meth, line);
                    }
                    3 => {
                        // The class through an attribute: `Cls.meth(x)`.
                        let line = out.push(format!("    r{n} = {e}.{meth}(arg)"));
                        key.uses(&name, line);
                        key.uses(&meth, line);
                    }
                    4 => {
                        let line = out.push(format!("    ok{n} = isinstance(arg, {e})"));
                        key.uses(&name, line);
                    }
                    5 => {
                        // A method read, not called.
                        let line = out.push(format!("    bound{n} = {e}().{meth}"));
                        key.uses(&name, line);
                        key.uses(&meth, line);
                    }
                    _ => {
                        let line = out.push(format!("    h{n} = {e}"));
                        key.uses(&name, line);
                    }
                }
            }
            Item::Function {
                is_async: callee_async,
                ..
            } => match pick {
                0 => {
                    let line = out.push(format!("    handlers{n} = [{e}]"));
                    key.uses(&name, line);
                }
                1 => {
                    let line = out.push(format!("    table{n} = {{\"k\": {e}}}"));
                    key.uses(&name, line);
                }
                2 => {
                    let line = out.push(format!("    kw{n} = dict(key={e})"));
                    key.uses(&name, line);
                }
                3 => {
                    let line = out.push(format!("    same{n} = arg == {e}"));
                    key.uses(&name, line);
                }
                4 => {
                    // In a string and a comment: not uses.
                    out.push(format!(
                        "    s{n} = \"{name}\"  # {name} is not called here"
                    ));
                }
                _ if *callee_async && is_async => {
                    let line = out.push(format!("    v{n} = await {e}(arg)"));
                    key.uses(&name, line);
                }
                _ => {
                    let line = out.push(format!("    v{n} = {e}(arg)"));
                    key.uses(&name, line);
                }
            },
            Item::Decorator { .. } => {
                let line = out.push(format!("    v{n} = {e}(arg)"));
                key.uses(&name, line);
            }
        }
        wrote = true;
    }
    if !wrote {
        out.push("    pass");
    }
    out.push("    return arg");
}
