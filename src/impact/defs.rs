//! Definitions with their line spans, from tree-sitter, for the languages
//! impact analysis supports: Python, JavaScript/TypeScript and Rust (the
//! languages with an import graph).
//!
//! The index keeps where each definition starts but not where it ends, so
//! the changed files are parsed again here; a diff names few files.

use serde::Serialize;
use std::path::Path;

/// What kind of definition a changed symbol is. Members (methods and
/// fields) are found by name on any receiver; the rest through the files
/// that import theirs.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum DefKind {
    Function,
    Class,
    /// Interface, type alias, enum, trait, struct.
    Type,
    /// A module-level constant or variable.
    Constant,
    Method,
    /// A class attribute or field.
    Field,
}

impl DefKind {
    /// Used through a value of its type (`obj.name`), not by importing it.
    pub fn is_member(self) -> bool {
        matches!(self, Self::Method | Self::Field)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Class => "class",
            Self::Type => "type",
            Self::Constant => "constant",
            Self::Method => "method",
            Self::Field => "field",
        }
    }
}

/// One definition and the lines it covers (0-based, inclusive).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Def {
    /// Enclosing names joined with `.`: `Model.save`, `outer.inner`.
    pub qualname: String,
    pub name: String,
    pub kind: DefKind,
    /// First line, decorators and attributes included.
    pub start: usize,
    pub end: usize,
    /// The line of the name.
    pub name_row: usize,
    /// Last line of the header (the signature), before the body.
    pub header_end: usize,
}

impl Def {
    /// The class (or impl type) a member belongs to.
    pub fn owner(&self) -> Option<&str> {
        let parent = self.qualname.strip_suffix(&self.name)?.strip_suffix('.')?;
        Some(parent.rsplit('.').next().unwrap_or(parent))
    }
}

/// The tree-sitter language for `path`, when impact analysis supports it.
pub fn language(path: &str) -> Option<tree_sitter::Language> {
    let ext = Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "py" | "pyi" | "pyw" => tree_sitter_python::LANGUAGE.into(),
        "ts" | "mts" | "cts" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
        "js" | "jsx" | "mjs" | "cjs" => tree_sitter_javascript::LANGUAGE.into(),
        "rs" => tree_sitter_rust::LANGUAGE.into(),
        _ => return None,
    })
}

fn text<'a>(n: tree_sitter::Node, src: &'a str) -> &'a str {
    src.get(n.byte_range()).unwrap_or_default()
}

/// Every definition in `src`, outermost first. Empty when the language is
/// not supported or the file does not parse at all.
pub fn definitions(path: &str, src: &str) -> Vec<Def> {
    let Some(lang) = language(path) else {
        return Vec::new();
    };
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&lang).is_err() {
        return Vec::new();
    }
    let Some(tree) = parser.parse(src, None) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    walk(tree.root_node(), src, &mut Vec::new(), &mut out);
    out
}

/// What node `n` defines, if anything: its kind and the node holding its
/// name.
fn definition<'t>(
    n: tree_sitter::Node<'t>,
    in_class: bool,
) -> Option<(DefKind, tree_sitter::Node<'t>)> {
    let name = |field: &str| n.child_by_field_name(field);
    let member_or = |k: DefKind| if in_class { DefKind::Method } else { k };
    Some(match n.kind() {
        // Python, JavaScript/TypeScript
        "function_definition" | "function_declaration" | "generator_function_declaration" => {
            (member_or(DefKind::Function), name("name")?)
        }
        "method_definition" | "abstract_method_signature" => (DefKind::Method, name("name")?),
        "class_definition" | "class_declaration" | "abstract_class_declaration" | "class" => {
            (DefKind::Class, name("name")?)
        }
        "interface_declaration" | "type_alias_declaration" | "enum_declaration" => {
            (DefKind::Type, name("name")?)
        }
        "public_field_definition" | "field_definition" if in_class => {
            let n = name("name").or_else(|| name("property"))?;
            (DefKind::Field, n)
        }
        // Rust
        "function_item" | "function_signature_item" => {
            (member_or(DefKind::Function), name("name")?)
        }
        "struct_item" | "enum_item" | "union_item" | "trait_item" | "type_item" => {
            (DefKind::Type, name("name")?)
        }
        "const_item" | "static_item" => (
            if in_class {
                DefKind::Field
            } else {
                DefKind::Constant
            },
            name("name")?,
        ),
        "macro_definition" => (DefKind::Function, name("name")?),
        _ => return None,
    })
}

/// A Python class attribute or module constant (`X = …`, `x: int = …`),
/// or a top-level `const x = …` in JavaScript/TypeScript.
fn assigned_name<'t>(n: tree_sitter::Node<'t>) -> Option<tree_sitter::Node<'t>> {
    match n.kind() {
        // Python: expression_statement > assignment, directly in a module
        // or a class body.
        "assignment" => {
            let stmt = n.parent().filter(|p| p.kind() == "expression_statement")?;
            let holder = stmt.parent()?;
            let ok = holder.kind() == "module"
                || (holder.kind() == "block"
                    && holder
                        .parent()
                        .is_some_and(|c| c.kind() == "class_definition"));
            ok.then_some(())?;
            n.child_by_field_name("left")
                .filter(|l| l.kind() == "identifier")
        }
        // JavaScript/TypeScript: program > lexical_declaration >
        // variable_declarator, or the same under `export`.
        "variable_declarator" => {
            let decl = n.parent()?;
            let holder = decl.parent()?;
            let top = holder.kind() == "program"
                || (holder.kind() == "export_statement"
                    && holder.parent().is_some_and(|g| g.kind() == "program"));
            top.then_some(())?;
            n.child_by_field_name("name")
                .filter(|l| l.kind() == "identifier")
        }
        _ => None,
    }
}

fn walk(n: tree_sitter::Node, src: &str, scope: &mut Vec<(String, bool)>, out: &mut Vec<Def>) {
    let in_class = scope.last().is_some_and(|(_, class)| *class);
    if let Some((kind, name_node)) = definition(n, in_class) {
        let name = text(name_node, src).to_string();
        let start = match n.parent() {
            Some(p) if p.kind() == "decorated_definition" => p.start_position().row,
            _ => n.start_position().row,
        };
        // A Python `block` starts at its first statement, the line after
        // the header; a `{ … }` body starts on the header's last line.
        let header_end = n
            .child_by_field_name("body")
            .map_or(n.end_position().row, |b| {
                let row = b.start_position().row;
                if b.kind() == "block" {
                    row.saturating_sub(1).max(name_node.start_position().row)
                } else {
                    row
                }
            });
        out.push(Def {
            qualname: qualify(scope, &name),
            name: name.clone(),
            kind,
            start: leading_attributes(n, start),
            end: n.end_position().row,
            name_row: name_node.start_position().row,
            header_end,
        });
        // A type alias or interface is one symbol; its members are not
        // looked up on their own.
        if matches!(n.kind(), "interface_declaration" | "type_alias_declaration") {
            return;
        }
        let class = kind == DefKind::Class || matches!(n.kind(), "trait_item");
        scope.push((name, class));
        children(n, src, scope, out);
        scope.pop();
        return;
    }
    // `impl Type { … }`: its functions are methods of `Type`.
    if n.kind() == "impl_item" {
        if let Some(ty) = n.child_by_field_name("type") {
            let ty = text(ty, src);
            let ty = ty.split('<').next().unwrap_or(ty).trim().to_string();
            scope.push((ty, true));
            children(n, src, scope, out);
            scope.pop();
            return;
        }
    }
    if n.kind() == "mod_item" {
        if let Some(name) = n.child_by_field_name("name") {
            scope.push((text(name, src).to_string(), false));
            children(n, src, scope, out);
            scope.pop();
            return;
        }
    }
    if let Some(name_node) = assigned_name(n) {
        let stmt = n.parent().unwrap_or(n);
        let name = text(name_node, src).to_string();
        out.push(Def {
            qualname: qualify(scope, &name),
            name,
            kind: if in_class {
                DefKind::Field
            } else {
                DefKind::Constant
            },
            start: stmt.start_position().row,
            end: stmt.end_position().row,
            name_row: name_node.start_position().row,
            header_end: name_node.start_position().row,
        });
        // `const f = () => {…}`: a change in the body is a change to `f`.
        return;
    }
    children(n, src, scope, out);
}

fn children(n: tree_sitter::Node, src: &str, scope: &mut Vec<(String, bool)>, out: &mut Vec<Def>) {
    let mut c = n.walk();
    for ch in n.children(&mut c) {
        walk(ch, src, scope, out);
    }
}

fn qualify(scope: &[(String, bool)], name: &str) -> String {
    let mut q: Vec<&str> = scope.iter().map(|(s, _)| s.as_str()).collect();
    q.push(name);
    q.join(".")
}

/// Rust `#[…]` attributes directly above an item belong to it.
fn leading_attributes(n: tree_sitter::Node, start: usize) -> usize {
    let mut start = start;
    let mut prev = n.prev_sibling();
    while let Some(p) = prev.filter(|p| p.kind() == "attribute_item") {
        if p.end_position().row + 1 < start {
            break;
        }
        start = p.start_position().row;
        prev = p.prev_sibling();
    }
    start
}

/// The innermost definition covering `row`.
pub fn innermost(defs: &[Def], row: usize) -> Option<&Def> {
    defs.iter()
        .filter(|d| d.start <= row && row <= d.end)
        .min_by_key(|d| (d.end - d.start, std::cmp::Reverse(d.start)))
}

/// How a symbol changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Change {
    Added,
    Modified,
    Removed,
}

impl Change {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Modified => "modified",
            Self::Removed => "removed",
        }
    }
}

/// A definition the diff touches.
#[derive(Debug, Clone)]
pub struct ChangedSymbol {
    pub def: Def,
    pub change: Change,
    /// A changed line is in the header (name, parameters, decorators),
    /// not only in the body.
    pub signature: bool,
}

/// The definitions `old_rows` / `new_rows` fall in, and the rows that fall
/// in none (module-level code: imports, top-level statements).
pub fn changed_symbols(
    old_defs: &[Def],
    new_defs: &[Def],
    old_rows: &[usize],
    new_rows: &[usize],
) -> (Vec<ChangedSymbol>, usize) {
    let mut out: Vec<ChangedSymbol> = Vec::new();
    let mut outside = 0;
    let has = |defs: &[Def], q: &str| defs.iter().any(|d| d.qualname == q);
    let mut add = |d: &Def, change: Change, sig: bool| {
        let sig = sig || change != Change::Modified;
        match out.iter_mut().find(|c| c.def.qualname == d.qualname) {
            Some(c) => c.signature |= sig,
            None => out.push(ChangedSymbol {
                def: d.clone(),
                change,
                signature: sig,
            }),
        }
    };
    for &r in new_rows {
        match innermost(new_defs, r) {
            Some(d) => {
                let change = if has(old_defs, &d.qualname) {
                    Change::Modified
                } else {
                    Change::Added
                };
                add(d, change, r <= d.header_end);
            }
            None => outside += 1,
        }
    }
    for &r in old_rows {
        match innermost(old_defs, r) {
            Some(d) => {
                let change = if has(new_defs, &d.qualname) {
                    Change::Modified
                } else {
                    Change::Removed
                };
                let sig = r <= d.header_end;
                // The new side's span is the one to report for a symbol
                // that still exists.
                let d = new_defs
                    .iter()
                    .find(|n| n.qualname == d.qualname)
                    .unwrap_or(d);
                add(d, change, sig);
            }
            None => outside += 1,
        }
    }
    (out, outside)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(path: &str, src: &str) -> Vec<(String, DefKind, usize, usize)> {
        definitions(path, src)
            .into_iter()
            .map(|d| (d.qualname, d.kind, d.start, d.end))
            .collect()
    }

    #[test]
    fn python_definitions_and_spans() {
        let src = "import os\n\nLIMIT = 3\n\n\n@decorator\ndef helper(x):\n    return x\n\n\nclass Model(Base):\n    table = 'm'\n\n    def save(self):\n        pass\n";
        let got = names("a.py", src);
        assert_eq!(
            got,
            vec![
                ("LIMIT".into(), DefKind::Constant, 2, 2),
                ("helper".into(), DefKind::Function, 5, 7),
                ("Model".into(), DefKind::Class, 10, 14),
                ("Model.table".into(), DefKind::Field, 11, 11),
                ("Model.save".into(), DefKind::Method, 13, 14),
            ]
        );
        let defs = definitions("a.py", src);
        assert_eq!(innermost(&defs, 14).unwrap().qualname, "Model.save");
        assert_eq!(innermost(&defs, 12).unwrap().qualname, "Model");
        assert_eq!(innermost(&defs, 0), None);
        assert_eq!(defs[4].owner(), Some("Model"));
        assert_eq!(defs[1].owner(), None);
    }

    #[test]
    fn typescript_definitions() {
        let src = "export const LIMIT = 3;\nexport function run(x: number): number {\n  return x;\n}\nexport class Box {\n  size = 1;\n  open(): void {}\n}\nexport interface Shape {\n  area(): number;\n}\ntype Id = string;\nconst make = () => {\n  return 1;\n};\n";
        let got: Vec<(String, DefKind)> = names("a.ts", src)
            .into_iter()
            .map(|(q, k, _, _)| (q, k))
            .collect();
        assert_eq!(
            got,
            vec![
                ("LIMIT".into(), DefKind::Constant),
                ("run".into(), DefKind::Function),
                ("Box".into(), DefKind::Class),
                ("Box.size".into(), DefKind::Field),
                ("Box.open".into(), DefKind::Method),
                ("Shape".into(), DefKind::Type),
                ("Id".into(), DefKind::Type),
                ("make".into(), DefKind::Constant),
            ]
        );
        let defs = definitions("a.ts", src);
        assert_eq!(innermost(&defs, 13).unwrap().qualname, "make");
    }

    #[test]
    fn rust_definitions() {
        let src = "pub const MAX: usize = 3;\n\n#[derive(Debug)]\npub struct Engine {\n    n: u32,\n}\n\nimpl<T> Engine {\n    pub fn run(&self) -> u32 {\n        self.n\n    }\n}\n\npub fn helper() {}\n";
        let got: Vec<(String, DefKind, usize)> = names("a.rs", src)
            .into_iter()
            .map(|(q, k, s, _)| (q, k, s))
            .collect();
        assert_eq!(
            got,
            vec![
                ("MAX".into(), DefKind::Constant, 0),
                ("Engine".into(), DefKind::Type, 2),
                ("Engine.run".into(), DefKind::Method, 8),
                ("helper".into(), DefKind::Function, 13),
            ]
        );
    }

    #[test]
    fn changed_symbols_classify_added_modified_removed() {
        let old = definitions(
            "a.py",
            "def a():\n    return 1\n\n\ndef b():\n    return 2\n",
        );
        let new = definitions(
            "a.py",
            "def a():\n    return 3\n\n\ndef c():\n    return 2\n",
        );
        // a's body changed (row 1); b became c (rows 4 on both sides).
        let (syms, outside) = changed_symbols(&old, &new, &[1, 4], &[1, 4]);
        let got: Vec<(&str, Change, bool)> = syms
            .iter()
            .map(|s| (s.def.qualname.as_str(), s.change, s.signature))
            .collect();
        assert_eq!(
            got,
            vec![
                ("a", Change::Modified, false),
                ("c", Change::Added, true),
                ("b", Change::Removed, true),
            ]
        );
        assert_eq!(outside, 0);
        let (_, outside) = changed_symbols(&old, &new, &[], &[2]);
        assert_eq!(
            outside, 1,
            "a blank line between definitions is module code"
        );
    }
}
