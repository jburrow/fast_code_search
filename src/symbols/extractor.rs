use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tree_sitter::Parser;
use tree_sitter_language::LanguageFn;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SymbolType {
    Function,
    Class,
    Method,
    Variable,
    Constant,
    Interface,
    Type,
    Enum,
    Trait,
    Struct,
    /// File name - indexed for path-based searches
    FileName,
    /// Module / namespace (`mod`, `namespace`, `package`, Ruby `module`)
    Module,
    /// Macro definition (`macro_rules!`, C `#define` is not parsed)
    Macro,
    /// Field / property of a type
    Field,
    /// C# / TS property accessor pair
    Property,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub symbol_type: SymbolType,
    pub line: usize,
    pub column: usize,
    pub is_definition: bool,
}

/// A use of a name that the grammar's tags query marks as a reference
/// (`@reference.call`, `@reference.class`, `@reference.implementation`, ...):
/// a call site, a type mention, an implemented interface. Positions are the
/// name node's, 0-based.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolRef {
    pub name: String,
    pub line: u32,
    pub column: u32,
}

/// A reference as the engine stores it: the name interned to an id, so a
/// call site costs 12 bytes rather than a `String`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackedRef {
    pub name: u32,
    pub line: u32,
    pub column: u32,
}

/// Represents an import statement found in source code
#[derive(Debug, Clone)]
pub struct ImportStatement {
    /// The raw import path/module name as written in source
    pub path: String,
    /// Line number where the import appears (0-based)
    pub line: usize,
    /// The type of import for context
    pub import_type: ImportType,
}

/// Type of import statement
#[derive(Debug, Clone, PartialEq)]
pub enum ImportType {
    /// Rust: `use crate::foo`, `use super::bar`, `mod foo`
    Rust,
    /// Python: `import foo`, `from foo import bar`
    Python,
    /// JavaScript/TypeScript: `import`, `require()`
    JavaScript,
}

pub struct SymbolExtractor {
    language: Option<LanguageFn>,
    extension: String,
}

/// Wall-clock cap for a single tree-sitter parse; pathological inputs that
/// pass the structural pre-check can still be quadratic.
const PARSE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

thread_local! {
    /// One parser per worker thread, re-targeted with `set_language` instead
    /// of being allocated per file.
    static PARSER: std::cell::RefCell<Parser> = std::cell::RefCell::new(Parser::new());
}

/// Parse `source` with the thread's parser under [`PARSE_TIMEOUT`].
/// `Ok(None)` means the parse was cancelled (timeout).
fn parse_with_reused_parser(
    language: LanguageFn,
    source: &str,
) -> Result<Option<tree_sitter::Tree>> {
    PARSER.with(|cell| {
        let mut parser = cell.borrow_mut();
        parser.set_language(&language.into())?;
        let started = std::time::Instant::now();
        let mut progress = |_: &tree_sitter::ParseState| {
            if started.elapsed() > PARSE_TIMEOUT {
                std::ops::ControlFlow::Break(())
            } else {
                std::ops::ControlFlow::Continue(())
            }
        };
        let options = tree_sitter::ParseOptions::new().progress_callback(&mut progress);
        let bytes = source.as_bytes();
        let tree = parser.parse_with_options(
            &mut |offset, _| {
                if offset < bytes.len() {
                    &bytes[offset..]
                } else {
                    &[]
                }
            },
            None,
            Some(options),
        );
        if tree.is_none() {
            tracing::warn!(
                elapsed_ms = started.elapsed().as_millis() as u64,
                "tree-sitter parse cancelled (timeout); file indexed without symbols"
            );
        }
        parser.reset();
        Ok(tree)
    })
}

/// Cheap C++-ness test for `.h` headers.
fn looks_like_cpp(source: &str) -> bool {
    const HINTS: [&str; 7] = [
        "class ",
        "namespace ",
        "template<",
        "template <",
        "public:",
        "private:",
        "::",
    ];
    HINTS.iter().any(|h| source.contains(h))
}

/// Innermost identifier of a (possibly pointer/reference/function) declarator.
fn innermost_declarator(mut node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    loop {
        match node.kind() {
            "identifier"
            | "field_identifier"
            | "type_identifier"
            | "destructor_name"
            | "operator_name"
            | "qualified_identifier" => return Some(node),
            "function_declarator"
            | "pointer_declarator"
            | "array_declarator"
            | "parenthesized_declarator"
            | "init_declarator" => {
                node = node.child_by_field_name("declarator")?;
            }
            // reference_declarator has no fields: the declarator is its last named child
            "reference_declarator" => {
                let mut cursor = node.walk();
                let last = node.named_children(&mut cursor).last()?;
                node = last;
            }
            _ => return None,
        }
    }
}

/// Map a `@definition.<kind>` capture (plus the captured node's kind, for
/// grammars whose query lumps several constructs together) to a SymbolType.
fn symbol_type_for(capture_kind: &str, node: tree_sitter::Node) -> SymbolType {
    let node_kind = node.kind();
    // Go: `type X struct{}` / `type X interface{}` / `type X = Y` all land on
    // a type_spec; the `type` child says which.
    if node_kind == "type_spec" {
        return match node.child_by_field_name("type").map(|t| t.kind()) {
            Some("struct_type") => SymbolType::Struct,
            Some("interface_type") => SymbolType::Interface,
            _ => SymbolType::Type,
        };
    }
    match node_kind {
        "struct_item"
        | "struct_specifier"
        | "struct_declaration"
        | "union_item"
        | "union_specifier"
        | "record_struct_declaration" => return SymbolType::Struct,
        "enum_item" | "enum_specifier" | "enum_declaration" => return SymbolType::Enum,
        "type_item" | "type_alias_declaration" | "type_definition" | "type_alias" => {
            return SymbolType::Type
        }
        "trait_item" | "trait_declaration" => return SymbolType::Trait,
        "interface_declaration" | "interface_type" => return SymbolType::Interface,
        "namespace_definition" | "namespace_declaration" | "internal_module" => {
            return SymbolType::Module
        }
        _ => {}
    }
    // C/C++ tags mark every function_declarator as a function; one that is
    // a class member declaration (`field_declaration`) is a method.
    if capture_kind == "function" {
        let mut up = node.parent();
        for _ in 0..3 {
            match up {
                Some(p) if p.kind() == "field_declaration" => return SymbolType::Method,
                Some(p) => up = p.parent(),
                None => break,
            }
        }
    }
    match capture_kind {
        "function" => SymbolType::Function,
        "method" => SymbolType::Method,
        "class" => SymbolType::Class,
        "interface" => SymbolType::Interface,
        "module" => SymbolType::Module,
        "macro" => SymbolType::Macro,
        "constant" => SymbolType::Constant,
        "type" => SymbolType::Type,
        "field" | "property" => SymbolType::Field,
        _ => SymbolType::Function,
    }
}

/// Tags queries describe definitions (`@definition.*`) and references
/// (`@reference.call` on every call expression, `@reference.class`, ...);
/// some also carry `@doc` patterns we never use. Switch off, once at compile
/// time, every pattern that captures neither a definition nor a reference.
fn disable_unused_patterns(query: &mut tree_sitter::Query) {
    let names = query.capture_names().to_vec();
    let wanted: Vec<usize> = names
        .iter()
        .enumerate()
        .filter(|(_, n)| n.starts_with("definition.") || n.starts_with("reference."))
        .map(|(i, _)| i)
        .collect();
    for pattern in 0..query.pattern_count() {
        let quantifiers = query.capture_quantifiers(pattern);
        let used = wanted.iter().any(|&c| {
            quantifiers
                .get(c)
                .is_some_and(|q| *q != tree_sitter::CaptureQuantifier::Zero)
        });
        if !used {
            query.disable_pattern(pattern);
        }
    }
}

/// The Rust grammar's `tags.scm` plus references it leaves out: calls
/// through a path (`crate::run()`, `Type::new()`), generic calls, and type
/// mentions (`fn f(e: &SearchEngine)`, `Vec<Item>`, `impl Trait for T`).
/// A type's own definition is also a `type_identifier`; those positions are
/// dropped again in `extract_with_tags`.
fn rust_tags_source() -> &'static str {
    use std::sync::OnceLock;
    static SRC: OnceLock<String> = OnceLock::new();
    SRC.get_or_init(|| {
        format!(
            "{}\n\n; fast_code_search supplement: scoped and generic calls\n\
             (call_expression\n    function: (scoped_identifier\n        name: (identifier) @name)) @reference.call\n\n\
             (call_expression\n    function: (generic_function\n        function: (identifier) @name)) @reference.call\n\n\
             (call_expression\n    function: (generic_function\n        function: (scoped_identifier\n            name: (identifier) @name))) @reference.call\n\n\
             ; fast_code_search supplement: type mentions\n\
             (type_identifier) @name @reference.type\n",
            tree_sitter_rust::TAGS_QUERY
        )
    })
}

/// Compile-once registry of the grammars' `tags.scm` queries.
fn tags_query_for(language: LanguageFn, extension: &str) -> Option<&'static tree_sitter::Query> {
    use std::sync::OnceLock;
    macro_rules! once {
        ($cell:ident, $lang:expr, $src:expr) => {{
            static $cell: OnceLock<Option<tree_sitter::Query>> = OnceLock::new();
            $cell
                .get_or_init(|| match tree_sitter::Query::new(&$lang.into(), $src) {
                    Ok(mut q) => {
                        disable_unused_patterns(&mut q);
                        Some(q)
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "tags query failed to compile; using walker only");
                        None
                    }
                })
                .as_ref()
        }};
    }
    let _ = language;
    match extension {
        "rs" => once!(RUST, tree_sitter_rust::LANGUAGE, rust_tags_source()),
        "py" | "pyi" | "pyw" => once!(
            PY,
            tree_sitter_python::LANGUAGE,
            tree_sitter_python::TAGS_QUERY
        ),
        "js" | "jsx" | "mjs" | "cjs" => once!(
            JS,
            tree_sitter_javascript::LANGUAGE,
            tree_sitter_javascript::TAGS_QUERY
        ),
        "ts" | "mts" | "cts" => once!(
            TS,
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
            tree_sitter_typescript::TAGS_QUERY
        ),
        "tsx" => once!(
            TSX,
            tree_sitter_typescript::LANGUAGE_TSX,
            tree_sitter_typescript::TAGS_QUERY
        ),
        "go" => once!(GO, tree_sitter_go::LANGUAGE, tree_sitter_go::TAGS_QUERY),
        "c" | "h" => once!(C, tree_sitter_c::LANGUAGE, tree_sitter_c::TAGS_QUERY),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" | "inl" => {
            once!(CPP, tree_sitter_cpp::LANGUAGE, tree_sitter_cpp::TAGS_QUERY)
        }
        "java" => once!(
            JAVA,
            tree_sitter_java::LANGUAGE,
            tree_sitter_java::TAGS_QUERY
        ),
        "cs" => once!(
            CS,
            tree_sitter_c_sharp::LANGUAGE,
            include_str!("queries/c_sharp_tags.scm")
        ),
        "rb" | "rake" | "gemspec" => {
            once!(RB, tree_sitter_ruby::LANGUAGE, tree_sitter_ruby::TAGS_QUERY)
        }
        "php" | "phtml" => once!(
            PHP,
            tree_sitter_php::LANGUAGE_PHP,
            tree_sitter_php::TAGS_QUERY
        ),
        _ => None,
    }
}

impl SymbolExtractor {
    pub fn new(file_path: &Path) -> Self {
        Self::new_for_source(file_path, None)
    }

    /// Like [`Self::new`], but may look at the content to disambiguate
    /// (`.h` headers that are really C++).
    pub fn new_for_source(file_path: &Path, source: Option<&str>) -> Self {
        let extension = file_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        // A `.h` header that is unmistakably C++ is treated as `.hpp` so the
        // grammar, the tags query and import extraction all agree.
        let extension = if extension == "h" && source.is_some_and(looks_like_cpp) {
            "hpp".to_string()
        } else {
            extension
        };
        let language = Self::language_for_extension(&extension, source);
        Self {
            language,
            extension,
        }
    }

    fn language_for_extension(extension: &str, _source: Option<&str>) -> Option<LanguageFn> {
        match extension {
            // Core programming languages
            "rs" => Some(tree_sitter_rust::LANGUAGE),
            "py" | "pyi" | "pyw" => Some(tree_sitter_python::LANGUAGE),
            "js" | "jsx" | "mjs" | "cjs" => Some(tree_sitter_javascript::LANGUAGE),
            "ts" | "mts" | "cts" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT),
            // JSX is not valid TypeScript; .tsx needs the dedicated grammar.
            "tsx" => Some(tree_sitter_typescript::LANGUAGE_TSX),
            "go" => Some(tree_sitter_go::LANGUAGE),
            // `.h` is C unless `new_for_source` already promoted it to `hpp`.
            "c" | "h" => Some(tree_sitter_c::LANGUAGE),
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" | "inl" => Some(tree_sitter_cpp::LANGUAGE),
            "java" => Some(tree_sitter_java::LANGUAGE),
            "cs" => Some(tree_sitter_c_sharp::LANGUAGE),
            "rb" | "rake" | "gemspec" => Some(tree_sitter_ruby::LANGUAGE),
            "php" | "phtml" => Some(tree_sitter_php::LANGUAGE_PHP),
            "sh" | "bash" | "zsh" => Some(tree_sitter_bash::LANGUAGE),
            // Config and markup formats are deliberately NOT parsed: no symbol
            // or import captures exist for them, so parsing (e.g. a multi-MB
            // package-lock.json) was pure cost.
            _ => None,
        }
    }

    pub fn extract(&self, source: &str) -> Result<Vec<Symbol>> {
        let language = match self.language {
            Some(lang) => lang,
            None => return Ok(Vec::new()), // No symbols for unknown languages
        };
        let Some(tree) = parse_with_reused_parser(language, source)? else {
            return Ok(Vec::new());
        };
        Ok(self.symbols_from_tree(&tree, source))
    }

    /// Symbols from a parsed tree: the grammar's own `tags.scm` query first
    /// (correct name positions, per-language maintained upstream), then the
    /// hand-written walker for the node kinds the query does not cover
    /// (Rust consts, C# properties, trait signatures, ...). Deduplicated on
    /// (name, line), sorted by line.
    fn symbols_from_tree(&self, tree: &tree_sitter::Tree, source: &str) -> Vec<Symbol> {
        self.symbols_and_refs_from_tree(tree, source).0
    }

    /// Definitions (tags query + walker, merged on (line, name)) and the
    /// references the tags query reports.
    fn symbols_and_refs_from_tree(
        &self,
        tree: &tree_sitter::Tree,
        source: &str,
    ) -> (Vec<Symbol>, Vec<SymbolRef>) {
        let root_node = tree.root_node();
        let mut symbols = Vec::new();
        let mut refs = Vec::new();
        if let Some(query) = self.tags_query() {
            Self::extract_with_tags(query, &root_node, source, &mut symbols, &mut refs);
        }
        let mut walker_symbols = Vec::new();
        Self::extract_functions(&root_node, source, &mut walker_symbols);
        let mut seen: std::collections::HashSet<(usize, String)> =
            symbols.iter().map(|s| (s.line, s.name.clone())).collect();
        for s in walker_symbols {
            if seen.insert((s.line, s.name.clone())) {
                symbols.push(s);
            }
        }
        symbols.sort_by_key(|s| s.line);
        refs.sort_by_key(|r| (r.line, r.column));
        (symbols, refs)
    }

    /// The compiled `tags.scm` query for this file's grammar, if it has one.
    fn tags_query(&self) -> Option<&'static tree_sitter::Query> {
        let language = self.language?;
        tags_query_for(language, &self.extension)
    }

    /// Run the tags query and collect every `@definition.*` capture with its
    /// `@name` as a symbol, and every `@reference.*` capture as a reference.
    /// Several patterns may match one node (Rust methods match both the
    /// method and the function pattern); the first pattern wins.
    fn extract_with_tags(
        query: &tree_sitter::Query,
        root: &tree_sitter::Node,
        source: &str,
        symbols: &mut Vec<Symbol>,
        refs: &mut Vec<SymbolRef>,
    ) {
        use tree_sitter::StreamingIterator;
        let names = query.capture_names();
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(query, *root, source.as_bytes());
        let mut seen_defs: std::collections::HashSet<(usize, usize)> =
            std::collections::HashSet::new();
        let mut seen_refs: std::collections::HashSet<(usize, usize)> =
            std::collections::HashSet::new();
        // Byte range of each reference pushed below, so references that
        // coincide with a definition's name (a bare `(type_identifier)`
        // pattern also matches `struct Foo`) can be dropped afterwards,
        // whatever order the two patterns matched in.
        let mut ref_ranges: Vec<(usize, usize)> = Vec::new();
        let first_ref = refs.len();
        while let Some(m) = matches.next() {
            let mut name_node: Option<tree_sitter::Node> = None;
            let mut def: Option<(&str, tree_sitter::Node)> = None;
            let mut is_ref = false;
            for cap in m.captures {
                let cap_name = names[cap.index as usize];
                if cap_name == "name" {
                    name_node = Some(cap.node);
                } else if let Some(kind) = cap_name.strip_prefix("definition.") {
                    def = Some((kind, cap.node));
                } else if cap_name.starts_with("reference.") {
                    is_ref = true;
                }
            }
            let Some(name_node) = name_node else {
                continue;
            };
            let range = name_node.byte_range();
            let Some(name) = source.get(range.clone()) else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            let start = name_node.start_position();
            if let Some((kind, def_node)) = def {
                if !seen_defs.insert((range.start, range.end)) {
                    continue;
                }
                symbols.push(Symbol {
                    name: name.to_string(),
                    symbol_type: symbol_type_for(kind, def_node),
                    line: start.row,
                    column: start.column,
                    is_definition: true,
                });
            } else if is_ref {
                if !seen_refs.insert((range.start, range.end)) {
                    continue;
                }
                ref_ranges.push((range.start, range.end));
                refs.push(SymbolRef {
                    name: name.to_string(),
                    line: start.row as u32,
                    column: start.column as u32,
                });
            }
        }
        // A definition's name is never a reference to itself.
        if ref_ranges.iter().any(|r| seen_defs.contains(r)) {
            let tail: Vec<SymbolRef> = refs.drain(first_ref..).collect();
            refs.extend(
                tail.into_iter()
                    .zip(ref_ranges)
                    .filter(|(_, r)| !seen_defs.contains(r))
                    .map(|(r, _)| r),
            );
        }
    }

    fn extract_functions(node: &tree_sitter::Node, source: &str, symbols: &mut Vec<Symbol>) {
        // Single-cursor depth-first walk: no per-node cursor allocation and
        // no explicit stack, which halves the walker's cost on large trees.
        let mut cursor = node.walk();
        if !cursor.goto_first_child() {
            return;
        }
        loop {
            Self::visit_definition_node(cursor.node(), source, symbols);
            if cursor.goto_first_child() {
                continue;
            }
            loop {
                if cursor.goto_next_sibling() {
                    break;
                }
                if !cursor.goto_parent() {
                    return;
                }
            }
        }
    }

    /// The hand-written per-node-kind extraction (supplements the tags query).
    fn visit_definition_node(child: tree_sitter::Node, source: &str, symbols: &mut Vec<Symbol>) {
        match child.kind() {
            // Functions: Rust, Python, JS/TS, PHP, Bash
            // Note: C/C++ function_definition has "declarator" not "name"
            "function_item" | "function_declaration" | "function_definition" => {
                // Try "name" first (most languages), then "declarator" (C/C++)
                let name_opt = child.child_by_field_name("name").or_else(|| {
                    // C/C++: name is inside declarator -> function_declarator -> identifier
                    child
                        .child_by_field_name("declarator")
                        .and_then(|d| d.child_by_field_name("declarator"))
                        .or_else(|| child.child_by_field_name("declarator"))
                });
                if let Some(name_node) = name_opt {
                    // For C/C++, the declarator might be a function_declarator
                    // We need to find the actual identifier
                    let ident_node = if matches!(
                        name_node.kind(),
                        "function_declarator" | "pointer_declarator" | "reference_declarator"
                    ) {
                        innermost_declarator(name_node)
                    } else {
                        Some(name_node)
                    };
                    if let Some(ident) = ident_node {
                        let name = &source[ident.byte_range()];
                        let start = ident.start_position();
                        symbols.push(Symbol {
                            name: name.to_string(),
                            symbol_type: SymbolType::Function,
                            line: start.row,
                            column: start.column,
                            is_definition: true,
                        });
                    }
                }
            }
            // Methods: Go, Java, C#, Ruby, PHP
            "method_declaration" | "method" | "singleton_method" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = &source[name_node.byte_range()];
                    let start = name_node.start_position();
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: SymbolType::Method,
                        line: start.row,
                        column: start.column,
                        is_definition: true,
                    });
                }
            }
            // JS/TS class members: methods, getters/setters, constructors
            "method_definition" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = &source[name_node.byte_range()];
                    let start = name_node.start_position();
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: SymbolType::Method,
                        line: start.row,
                        column: start.column,
                        is_definition: true,
                    });
                }
            }
            // JS/TS: `const Foo = () => …` / `const foo = function () {}` —
            // the dominant modern function form (React components, handlers).
            // Only plain identifier names (not destructuring patterns).
            "variable_declarator" => {
                let is_fn_value = child
                    .child_by_field_name("value")
                    .map(|v| {
                        matches!(
                            v.kind(),
                            "arrow_function"
                                | "function_expression"
                                | "function"
                                | "generator_function"
                        )
                    })
                    .unwrap_or(false);
                if is_fn_value {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        if name_node.kind() == "identifier" {
                            let name = &source[name_node.byte_range()];
                            let start = name_node.start_position();
                            symbols.push(Symbol {
                                name: name.to_string(),
                                symbol_type: SymbolType::Function,
                                line: start.row,
                                column: start.column,
                                is_definition: true,
                            });
                        }
                    }
                }
            }
            // JS: `function* gen() {}`
            "generator_function_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = &source[name_node.byte_range()];
                    let start = name_node.start_position();
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: SymbolType::Function,
                        line: start.row,
                        column: start.column,
                        is_definition: true,
                    });
                }
            }
            // TS: `abstract class X {}` and `namespace X {}` / `module X {}`
            "abstract_class_declaration" | "internal_module" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = &source[name_node.byte_range()];
                    let start = name_node.start_position();
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: SymbolType::Class,
                        line: start.row,
                        column: start.column,
                        is_definition: true,
                    });
                }
            }
            // Constructors: Java, C#
            "constructor_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = &source[name_node.byte_range()];
                    let start = name_node.start_position();
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: SymbolType::Method,
                        line: start.row,
                        column: start.column,
                        is_definition: true,
                    });
                }
            }
            // C# property declarations
            "property_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = &source[name_node.byte_range()];
                    let start = name_node.start_position();
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: SymbolType::Property,
                        line: start.row,
                        column: start.column,
                        is_definition: true,
                    });
                }
            }
            // Classes: JS/TS, Python, Java, C#, PHP, Ruby
            "class_declaration" | "class_definition" | "class" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = &source[name_node.byte_range()];
                    let start = name_node.start_position();
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: SymbolType::Class,
                        line: start.row,
                        column: start.column,
                        is_definition: true,
                    });
                }
            }
            // Ruby modules
            "module" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = &source[name_node.byte_range()];
                    let start = name_node.start_position();
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: SymbolType::Module,
                        line: start.row,
                        column: start.column,
                        is_definition: true,
                    });
                }
            }
            // Rust impl blocks use "type" field, not "name"
            "impl_item" => {
                if let Some(type_node) = child.child_by_field_name("type") {
                    let name = &source[type_node.byte_range()];
                    let start = type_node.start_position();
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: SymbolType::Class,
                        line: start.row,
                        column: start.column,
                        is_definition: true,
                    });
                }
            }
            // Interfaces: TS, Java, C#, PHP
            "interface_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = &source[name_node.byte_range()];
                    let start = name_node.start_position();
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: SymbolType::Interface,
                        line: start.row,
                        column: start.column,
                        is_definition: true,
                    });
                }
            }
            // Type aliases: TS, Rust
            "type_alias_declaration" | "type_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = &source[name_node.byte_range()];
                    let start = name_node.start_position();
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: SymbolType::Type,
                        line: start.row,
                        column: start.column,
                        is_definition: true,
                    });
                }
            }
            // Enums: TS, Rust, Java, C#
            "enum_declaration" | "enum_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = &source[name_node.byte_range()];
                    let start = name_node.start_position();
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: SymbolType::Enum,
                        line: start.row,
                        column: start.column,
                        is_definition: true,
                    });
                }
            }
            // Rust trait method signatures (`fn area(&self) -> f64;`)
            "function_signature_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = &source[name_node.byte_range()];
                    let start = name_node.start_position();
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: SymbolType::Method,
                        line: start.row,
                        column: start.column,
                        is_definition: true,
                    });
                }
            }
            // C# delegates are named types
            "delegate_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = &source[name_node.byte_range()];
                    let start = name_node.start_position();
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: SymbolType::Type,
                        line: start.row,
                        column: start.column,
                        is_definition: true,
                    });
                }
            }
            // C++ in-class method declarations: `int area() const;`
            "field_declaration" => {
                if let Some(decl) = child.child_by_field_name("declarator") {
                    if decl.kind() == "function_declarator" {
                        if let Some(ident) = innermost_declarator(decl) {
                            let name = &source[ident.byte_range()];
                            let start = ident.start_position();
                            symbols.push(Symbol {
                                name: name.to_string(),
                                symbol_type: SymbolType::Method,
                                line: start.row,
                                column: start.column,
                                is_definition: true,
                            });
                        }
                    }
                }
            }
            // Records: Java, C#
            "record_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = &source[name_node.byte_range()];
                    let start = name_node.start_position();
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: SymbolType::Class,
                        line: start.row,
                        column: start.column,
                        is_definition: true,
                    });
                }
            }
            // Rust traits (similar to interfaces)
            "trait_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = &source[name_node.byte_range()];
                    let start = name_node.start_position();
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: SymbolType::Trait,
                        line: start.row,
                        column: start.column,
                        is_definition: true,
                    });
                }
            }
            // PHP traits
            "trait_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = &source[name_node.byte_range()];
                    let start = name_node.start_position();
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: SymbolType::Trait,
                        line: start.row,
                        column: start.column,
                        is_definition: true,
                    });
                }
            }
            // Structs: Rust, C#
            "struct_item" | "struct_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = &source[name_node.byte_range()];
                    let start = name_node.start_position();
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: SymbolType::Struct,
                        line: start.row,
                        column: start.column,
                        is_definition: true,
                    });
                }
            }
            // Rust constants and statics
            "const_item" | "static_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = &source[name_node.byte_range()];
                    let start = name_node.start_position();
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: SymbolType::Constant,
                        line: start.row,
                        column: start.column,
                        is_definition: true,
                    });
                }
            }
            // Go: type declarations (struct, interface, type alias)
            "type_declaration" => {
                let mut type_cursor = child.walk();
                for type_child in child.children(&mut type_cursor) {
                    if type_child.kind() == "type_alias" {
                        if let Some(name_node) = type_child.child_by_field_name("name") {
                            let start = name_node.start_position();
                            symbols.push(Symbol {
                                name: source[name_node.byte_range()].to_string(),
                                symbol_type: SymbolType::Type,
                                line: start.row,
                                column: start.column,
                                is_definition: true,
                            });
                        }
                        continue;
                    }
                    if type_child.kind() == "type_spec" {
                        if let Some(name_node) = type_child.child_by_field_name("name") {
                            let name = &source[name_node.byte_range()];
                            let start = name_node.start_position();
                            let symbol_type =
                                if let Some(type_node) = type_child.child_by_field_name("type") {
                                    match type_node.kind() {
                                        "struct_type" => SymbolType::Struct,
                                        "interface_type" => SymbolType::Interface,
                                        _ => SymbolType::Type,
                                    }
                                } else {
                                    SymbolType::Type
                                };
                            symbols.push(Symbol {
                                name: name.to_string(),
                                symbol_type,
                                line: start.row,
                                column: start.column,
                                is_definition: true,
                            });
                        }
                    }
                }
            }
            // Go: const and var declarations
            "const_declaration" | "var_declaration" => {
                let mut const_cursor = child.walk();
                for spec in child.children(&mut const_cursor) {
                    if spec.kind() == "const_spec" || spec.kind() == "var_spec" {
                        if let Some(name_node) = spec.child_by_field_name("name") {
                            let name = &source[name_node.byte_range()];
                            let start = name_node.start_position();
                            let symbol_type = if child.kind() == "const_declaration" {
                                SymbolType::Constant
                            } else {
                                SymbolType::Variable
                            };
                            symbols.push(Symbol {
                                name: name.to_string(),
                                symbol_type,
                                line: start.row,
                                column: start.column,
                                is_definition: true,
                            });
                        }
                    }
                }
            }
            // C/C++: struct, union declarations
            "struct_specifier" | "union_specifier" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = &source[name_node.byte_range()];
                    let start = name_node.start_position();
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: SymbolType::Struct,
                        line: start.row,
                        column: start.column,
                        is_definition: true,
                    });
                }
            }
            // C/C++: enum specifier
            "enum_specifier" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = &source[name_node.byte_range()];
                    let start = name_node.start_position();
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: SymbolType::Enum,
                        line: start.row,
                        column: start.column,
                        is_definition: true,
                    });
                }
            }
            // C++: class specifier and namespace
            "class_specifier" | "namespace_definition" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = &source[name_node.byte_range()];
                    let start = name_node.start_position();
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: if child.kind() == "namespace_definition" {
                            SymbolType::Module
                        } else {
                            SymbolType::Class
                        },
                        line: start.row,
                        column: start.column,
                        is_definition: true,
                    });
                }
            }
            // C++: template declarations - traverse into them
            // C++: template declarations — just let the default stack.push(child)
            // below handle traversal. The template_declaration node will be pushed
            // to the stack, and when it becomes `current`, its children
            // (function_definition, class_specifier, etc.) will be matched naturally.
            // No special handling needed.
            "template_declaration" => {}
            _ => {}
        }
    }

    /// Extract import statements from source code
    pub fn extract_imports(&self, source: &str) -> Result<Vec<ImportStatement>> {
        let language = match self.language {
            Some(lang) => lang,
            None => return Ok(Vec::new()),
        };
        let Some(tree) = parse_with_reused_parser(language, source)? else {
            return Ok(Vec::new());
        };

        let mut imports = Vec::new();
        let root_node = tree.root_node();

        match self.extension.as_str() {
            "rs" => Self::extract_rust_imports(&root_node, source, &mut imports),
            // Include stub files (.pyi) and Windows-specific (.pyw) as Python
            "py" | "pyi" | "pyw" => Self::extract_python_imports(&root_node, source, &mut imports),
            // Include ESM/CJS variants and TypeScript module variants
            "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" => {
                Self::extract_js_imports(&root_node, source, &mut imports)
            }
            _ => {}
        }

        imports.sort_by_key(|i| i.line);
        Ok(imports)
    }

    /// Extract both symbols and imports in a single parse pass.
    ///
    /// Equivalent to calling `extract` and `extract_imports` separately, but
    /// only parses the source file once, making it roughly 2× faster when both
    /// are needed.
    pub fn extract_all(&self, source: &str) -> Result<(Vec<Symbol>, Vec<ImportStatement>)> {
        let (symbols, imports, _) = self.extract_all_with_refs(source)?;
        Ok((symbols, imports))
    }

    /// [`Self::extract_all`] plus the references (call sites, type mentions)
    /// the grammar's tags query reports, from the same single parse.
    #[allow(clippy::type_complexity)]
    pub fn extract_all_with_refs(
        &self,
        source: &str,
    ) -> Result<(Vec<Symbol>, Vec<ImportStatement>, Vec<SymbolRef>)> {
        let language = match self.language {
            Some(lang) => lang,
            None => return Ok((Vec::new(), Vec::new(), Vec::new())),
        };
        let Some(tree) = parse_with_reused_parser(language, source)? else {
            return Ok((Vec::new(), Vec::new(), Vec::new()));
        };
        let root_node = tree.root_node();
        let (symbols, refs) = self.symbols_and_refs_from_tree(&tree, source);

        let mut imports = Vec::new();
        match self.extension.as_str() {
            "rs" => Self::extract_rust_imports(&root_node, source, &mut imports),
            "py" | "pyi" | "pyw" => Self::extract_python_imports(&root_node, source, &mut imports),
            "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" => {
                Self::extract_js_imports(&root_node, source, &mut imports)
            }
            _ => {}
        }
        imports.sort_by_key(|i| i.line);

        Ok((symbols, imports, refs))
    }

    /// Extract Rust use statements and mod declarations
    fn extract_rust_imports(
        node: &tree_sitter::Node,
        source: &str,
        imports: &mut Vec<ImportStatement>,
    ) {
        // Use iterative traversal with explicit stack to avoid stack overflow
        let mut stack = vec![*node];

        while let Some(current) = stack.pop() {
            let mut cursor = current.walk();

            for child in current.children(&mut cursor) {
                match child.kind() {
                    "use_declaration" => {
                        // Extract the use path
                        if let Some(path_node) = child.child_by_field_name("argument") {
                            let path = &source[path_node.byte_range()];
                            imports.push(ImportStatement {
                                path: path.to_string(),
                                line: child.start_position().row,
                                import_type: ImportType::Rust,
                            });
                        } else {
                            // Fallback: get full text minus 'use' and ';'
                            let text = &source[child.byte_range()];
                            let path = text.trim_start_matches("use").trim_end_matches(';').trim();
                            if !path.is_empty() {
                                imports.push(ImportStatement {
                                    path: path.to_string(),
                                    line: child.start_position().row,
                                    import_type: ImportType::Rust,
                                });
                            }
                        }
                    }
                    "mod_item" => {
                        // mod foo; declarations
                        if let Some(name_node) = child.child_by_field_name("name") {
                            let name = &source[name_node.byte_range()];
                            imports.push(ImportStatement {
                                path: name.to_string(),
                                line: child.start_position().row,
                                import_type: ImportType::Rust,
                            });
                        }
                    }
                    _ => {}
                }

                // Add child to stack for iterative processing
                stack.push(child);
            }
        }
    }

    /// Extract Python import statements
    fn extract_python_imports(
        node: &tree_sitter::Node,
        source: &str,
        imports: &mut Vec<ImportStatement>,
    ) {
        // Use iterative traversal with explicit stack to avoid stack overflow
        let mut stack = vec![*node];

        while let Some(current) = stack.pop() {
            let mut cursor = current.walk();

            for child in current.children(&mut cursor) {
                match child.kind() {
                    // `import a, b as c` — one `name` field per imported module
                    // (a `dotted_name` or an `aliased_import` wrapping one).
                    "import_statement" => {
                        let mut import_cursor = child.walk();
                        let mut found = false;
                        for part in child.children_by_field_name("name", &mut import_cursor) {
                            let module_node = if part.kind() == "aliased_import" {
                                part.child_by_field_name("name")
                            } else {
                                Some(part)
                            };
                            if let Some(module_node) = module_node {
                                found = true;
                                imports.push(ImportStatement {
                                    path: source[module_node.byte_range()].to_string(),
                                    line: child.start_position().row,
                                    import_type: ImportType::Python,
                                });
                            }
                        }
                        if !found {
                            let text = &source[child.byte_range()];
                            if let Some(path) = Self::parse_python_import_text(text) {
                                imports.push(ImportStatement {
                                    path,
                                    line: child.start_position().row,
                                    import_type: ImportType::Python,
                                });
                            }
                        }
                    }
                    "import_from_statement" => {
                        // Get the module name from the import
                        if let Some(module_node) = child.child_by_field_name("module_name") {
                            let module = &source[module_node.byte_range()];
                            imports.push(ImportStatement {
                                path: module.to_string(),
                                line: child.start_position().row,
                                import_type: ImportType::Python,
                            });
                        } else {
                            // Fallback: extract from full text
                            let text = &source[child.byte_range()];
                            if let Some(path) = Self::parse_python_import_text(text) {
                                imports.push(ImportStatement {
                                    path,
                                    line: child.start_position().row,
                                    import_type: ImportType::Python,
                                });
                            }
                        }
                    }
                    _ => {}
                }

                // Add child to stack for iterative processing
                stack.push(child);
            }
        }
    }

    /// Parse Python import text to extract module path
    fn parse_python_import_text(text: &str) -> Option<String> {
        let text = text.trim();
        if text.starts_with("from ") {
            // from foo.bar import baz
            let rest = text.strip_prefix("from ")?.trim();
            let module = rest.split_whitespace().next()?;
            Some(module.to_string())
        } else if text.starts_with("import ") {
            // import foo.bar
            let rest = text.strip_prefix("import ")?.trim();
            let module = rest.split([',', ' ']).next()?;
            Some(module.to_string())
        } else {
            None
        }
    }

    /// Extract JavaScript/TypeScript import statements
    fn extract_js_imports(
        node: &tree_sitter::Node,
        source: &str,
        imports: &mut Vec<ImportStatement>,
    ) {
        // Use iterative traversal with explicit stack to avoid stack overflow
        let mut stack = vec![*node];

        while let Some(current) = stack.pop() {
            let mut cursor = current.walk();

            for child in current.children(&mut cursor) {
                match child.kind() {
                    // `import x from './bar'` and re-exports `export { x } from './bar'`
                    // (barrel index files) both carry a `source` field.
                    "import_statement" | "export_statement" => {
                        if let Some(source_node) = child.child_by_field_name("source") {
                            let path = &source[source_node.byte_range()];
                            let path = path.trim_matches(|c| c == '"' || c == '\'' || c == '`');
                            imports.push(ImportStatement {
                                path: path.to_string(),
                                line: child.start_position().row,
                                import_type: ImportType::JavaScript,
                            });
                        }
                    }
                    "call_expression" => {
                        // require('./foo') and dynamic import('./foo')
                        if let Some(func_node) = child.child_by_field_name("function") {
                            let func_name = &source[func_node.byte_range()];
                            if func_name == "require" || func_name == "import" {
                                if let Some(args_node) = child.child_by_field_name("arguments") {
                                    let args_text = &source[args_node.byte_range()];
                                    let path = args_text
                                        .trim_matches(|c| c == '(' || c == ')')
                                        .trim()
                                        .trim_matches(|c| c == '"' || c == '\'' || c == '`');
                                    if !path.is_empty() && !path.contains("${") {
                                        imports.push(ImportStatement {
                                            path: path.to_string(),
                                            line: child.start_position().row,
                                            import_type: ImportType::JavaScript,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }

                // Add child to stack for iterative processing
                stack.push(child);
            }
        }
    }

    pub fn is_supported(&self) -> bool {
        self.language.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_function_extraction() {
        let source = r#"
fn hello_world() {
    println!("Hello");
}

fn another_function() {
    // code
}
"#;
        let extractor = SymbolExtractor::new(Path::new("test.rs"));
        let symbols = extractor.extract(source).unwrap();

        assert!(symbols.len() >= 2);
        assert!(symbols.iter().any(|s| s.name == "hello_world"));
        assert!(symbols.iter().any(|s| s.name == "another_function"));
    }

    #[test]
    fn test_typescript_interface_extraction() {
        let source = r#"
interface User {
    id: number;
    name: string;
}

interface Config {
    debug: boolean;
}

type UserId = string | number;

type Handler = (event: Event) => void;

function processUser(user: User): void {
    console.log(user.name);
}
"#;
        let extractor = SymbolExtractor::new(Path::new("test.ts"));
        let symbols = extractor.extract(source).unwrap();

        // Should find interfaces
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "User" && s.symbol_type == SymbolType::Interface),
            "Should find User interface"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Config" && s.symbol_type == SymbolType::Interface),
            "Should find Config interface"
        );

        // Should find type aliases
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "UserId" && s.symbol_type == SymbolType::Type),
            "Should find UserId type alias"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Handler" && s.symbol_type == SymbolType::Type),
            "Should find Handler type alias"
        );

        // Should find function
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "processUser" && s.symbol_type == SymbolType::Function),
            "Should find processUser function"
        );
    }

    /// Roadmap 1.5: `.tsx` must be parsed with the TSX grammar (JSX is not
    /// valid TypeScript), and modern JS/TS function forms must be captured:
    /// class methods, arrow-function consts, abstract classes, namespaces.
    #[test]
    fn test_tsx_react_component_extraction() {
        let source = r#"
import React from "react";

export const Button = ({ label }: { label: string }) => {
    return <button className="btn">{label}</button>;
};

export function Card(props: { title: string }) {
    return <div><h1>{props.title}</h1></div>;
}

class Widget extends React.Component {
    render() {
        return <span />;
    }
    handleClick = () => {};
}

abstract class Shape {
    abstract area(): number;
}

namespace Util {
    export const helper = function () { return 1; };
}
"#;
        let extractor = SymbolExtractor::new(Path::new("App.tsx"));
        let symbols = extractor.extract(source).unwrap();
        let has =
            |n: &str, t: SymbolType| symbols.iter().any(|s| s.name == n && s.symbol_type == t);

        assert!(
            has("Button", SymbolType::Function),
            "arrow const: {symbols:?}"
        );
        assert!(has("Card", SymbolType::Function), "function with JSX body");
        assert!(has("Widget", SymbolType::Class), "class");
        assert!(has("render", SymbolType::Method), "method_definition");
        assert!(has("Shape", SymbolType::Class), "abstract class");
        assert!(has("Util", SymbolType::Class), "namespace");
        assert!(
            has("helper", SymbolType::Function),
            "function expression const"
        );
        // A component body with JSX would have become ERROR nodes under the
        // TypeScript grammar; `Card` being found with its correct line proves
        // the TSX grammar parsed it.
        let card = symbols.iter().find(|s| s.name == "Card").unwrap();
        assert_eq!(card.line, 7, "Card is on (0-based) line 7");
    }

    /// Roadmap 1.5: plain JavaScript method and arrow-function coverage.
    #[test]
    fn test_javascript_methods_and_arrows() {
        let source = r#"
class Store {
    constructor() { this.items = []; }
    add(item) { this.items.push(item); }
    get size() { return this.items.length; }
    static create() { return new Store(); }
}
const onClick = (e) => e.preventDefault();
const legacy = function (x) { return x; };
function* ids() { yield 1; }
const { a, b } = obj;
"#;
        let extractor = SymbolExtractor::new(Path::new("store.js"));
        let symbols = extractor.extract(source).unwrap();
        let has =
            |n: &str, t: SymbolType| symbols.iter().any(|s| s.name == n && s.symbol_type == t);
        assert!(has("Store", SymbolType::Class));
        assert!(has("constructor", SymbolType::Method));
        assert!(has("add", SymbolType::Method));
        assert!(has("size", SymbolType::Method), "getter");
        assert!(has("create", SymbolType::Method), "static method");
        assert!(has("onClick", SymbolType::Function), "arrow const");
        assert!(
            has("legacy", SymbolType::Function),
            "function-expression const"
        );
        assert!(has("ids", SymbolType::Function), "generator");
        assert!(
            !symbols.iter().any(|s| s.name == "a" || s.name == "b"),
            "destructuring patterns are not symbols"
        );
    }

    /// Roadmap 1.5: `line`/`column` must come from the *name* node. Annotations,
    /// decorators, modifiers and multi-line return types precede the name and
    /// previously shifted every such symbol to the wrong line (and the
    /// definition boost with it).
    #[test]
    fn test_symbol_line_is_name_line() {
        // Java: @Override on its own line above the method name.
        let java = "class A {\n    @Override\n    public String toString() {\n        return \"\";\n    }\n}\n";
        let syms = SymbolExtractor::new(Path::new("A.java"))
            .extract(java)
            .unwrap();
        let m = syms
            .iter()
            .find(|s| s.name == "toString")
            .expect("toString");
        assert_eq!(
            (m.line, m.column),
            (2, 18),
            "Java method line/col from name node: {m:?}"
        );

        // TypeScript: decorator line above the class name.
        let ts = "@Component({})\nexport class AppRoot {\n}\n";
        let syms = SymbolExtractor::new(Path::new("app.ts"))
            .extract(ts)
            .unwrap();
        let c = syms.iter().find(|s| s.name == "AppRoot").expect("AppRoot");
        assert_eq!(c.line, 1, "TS decorated class line from name node: {c:?}");

        // C: return type on its own line above the function name.
        let c_src = "static int\nadd(int a, int b)\n{\n    return a + b;\n}\n";
        let syms = SymbolExtractor::new(Path::new("m.c"))
            .extract(c_src)
            .unwrap();
        let f = syms.iter().find(|s| s.name == "add").expect("add");
        assert_eq!(
            (f.line, f.column),
            (1, 0),
            "C function line/col from name node: {f:?}"
        );

        // Rust: attribute above a function.
        let rs = "#[inline]\npub fn fast() {}\n";
        let syms = SymbolExtractor::new(Path::new("x.rs")).extract(rs).unwrap();
        let f = syms.iter().find(|s| s.name == "fast").expect("fast");
        assert_eq!(
            (f.line, f.column),
            (1, 7),
            "Rust fn line/col from name node: {f:?}"
        );
    }

    /// Roadmap 2.5: import extraction covers `import a, b as c`, re-exports,
    /// dynamic `import()` and backtick `require`.
    #[test]
    fn test_python_and_js_import_extraction() {
        let py = "import os, sys as system\nfrom .rel import thing\nfrom pkg.mod import x\n";
        let imports = SymbolExtractor::new(Path::new("a.py"))
            .extract_imports(py)
            .unwrap();
        let paths: Vec<&str> = imports.iter().map(|i| i.path.as_str()).collect();
        assert_eq!(paths, vec!["os", "sys", ".rel", "pkg.mod"], "{paths:?}");

        let js = concat!(
            "import a from './a';\n",
            "export { b } from './b';\n",
            "export * from './c';\n",
            "const d = require(`./d`);\n",
            "const e = await import('./e');\n",
            "const bad = require(`./${name}`);\n",
        );
        let imports = SymbolExtractor::new(Path::new("index.js"))
            .extract_imports(js)
            .unwrap();
        let mut paths: Vec<&str> = imports.iter().map(|i| i.path.as_str()).collect();
        paths.sort();
        assert_eq!(paths, vec!["./a", "./b", "./c", "./d", "./e"], "{paths:?}");
    }

    /// Roadmap 5.6: Python — classes, functions, methods, module constants,
    /// with line numbers.
    #[test]
    fn test_python_extraction_with_lines() {
        let src = "MAX_RETRIES = 3\n\nclass Client:\n    def __init__(self):\n        pass\n\n    @property\n    def name(self):\n        return 'x'\n\ndef helper(x):\n    return x\n";
        let syms = SymbolExtractor::new(Path::new("client.py"))
            .extract(src)
            .unwrap();
        let find = |n: &str| {
            syms.iter()
                .find(|s| s.name == n)
                .unwrap_or_else(|| panic!("{n}: {syms:?}"))
        };
        assert_eq!(find("MAX_RETRIES").symbol_type, SymbolType::Constant);
        assert_eq!(find("MAX_RETRIES").line, 0);
        assert_eq!(find("Client").symbol_type, SymbolType::Class);
        assert_eq!(find("Client").line, 2);
        assert_eq!(find("__init__").line, 3);
        assert_eq!(
            find("name").line,
            7,
            "decorated method: name line, not decorator line"
        );
        assert_eq!((find("helper").line, find("helper").column), (10, 4));
    }

    /// Roadmap 5.6: C and a C++ header named `.h`.
    #[test]
    fn test_c_and_cpp_header_extraction() {
        let c_src = "typedef struct point { int x; } point_t;\nstatic int *make_ptr(void) { return 0; }\nint &ref_fn();\nenum color { RED };\nint add(int a, int b) {\n    return a + b;\n}\n";
        let syms = SymbolExtractor::new(Path::new("m.c"))
            .extract(c_src)
            .unwrap();
        let has = |n: &str, t: SymbolType| syms.iter().any(|s| s.name == n && s.symbol_type == t);
        assert!(has("point", SymbolType::Struct), "{syms:?}");
        assert!(has("point_t", SymbolType::Type), "typedef: {syms:?}");
        assert!(
            has("make_ptr", SymbolType::Function),
            "pointer declarator unwrapped: {syms:?}"
        );
        assert!(has("color", SymbolType::Enum), "{syms:?}");
        assert!(has("add", SymbolType::Function));
        assert!(
            !syms
                .iter()
                .any(|s| s.name.contains('*') || s.name.contains('&')),
            "{syms:?}"
        );

        // A .h that is really C++: class/namespace parse (they would be ERROR
        // nodes under the C grammar).
        let h_src = "namespace geo {\nclass Shape {\npublic:\n    virtual double area() const;\n    Shape();\n};\n}\n";
        let syms = SymbolExtractor::new_for_source(Path::new("shape.h"), Some(h_src))
            .extract(h_src)
            .unwrap();
        let has = |n: &str, t: SymbolType| syms.iter().any(|s| s.name == n && s.symbol_type == t);
        assert!(has("geo", SymbolType::Module), "{syms:?}");
        assert!(has("Shape", SymbolType::Class), "{syms:?}");
        assert!(
            has("area", SymbolType::Method),
            "in-class declaration: {syms:?}"
        );
        // Plain C header stays C.
        let plain = "int add(int a, int b);\n";
        let syms = SymbolExtractor::new_for_source(Path::new("add.h"), Some(plain))
            .extract(plain)
            .unwrap();
        assert!(syms.iter().any(|s| s.name == "add"), "{syms:?}");
    }

    /// Roadmap 5.2/5.3: kinds and coverage added on top of the tags queries.
    #[test]
    fn test_extra_kinds_and_coverage() {
        let rs = "pub trait Shape {\n    fn area(&self) -> f64;\n}\nmacro_rules! sq { ($x:expr) => { $x * $x }; }\npub union U { a: u32 }\nmod inner {}\nimpl Shape for U {\n    fn area(&self) -> f64 { 0.0 }\n}\n";
        let syms = SymbolExtractor::new(Path::new("s.rs")).extract(rs).unwrap();
        let has = |n: &str, t: SymbolType| syms.iter().any(|s| s.name == n && s.symbol_type == t);
        assert!(has("Shape", SymbolType::Trait), "{syms:?}");
        assert!(
            has("area", SymbolType::Method),
            "trait signature + impl method: {syms:?}"
        );
        assert!(has("sq", SymbolType::Macro), "{syms:?}");
        assert!(has("U", SymbolType::Struct), "union: {syms:?}");
        assert!(has("inner", SymbolType::Module), "{syms:?}");
        assert_eq!(
            syms.iter().filter(|s| s.name == "area").count(),
            2,
            "one per line"
        );

        let cs = "namespace App {\n  public delegate void Handler(int x);\n  public class Svc {\n    public int Count { get; set; }\n    public void Run() {}\n  }\n}\n";
        let syms = SymbolExtractor::new(Path::new("a.cs")).extract(cs).unwrap();
        let has = |n: &str, t: SymbolType| syms.iter().any(|s| s.name == n && s.symbol_type == t);
        assert!(has("App", SymbolType::Module), "{syms:?}");
        assert!(has("Handler", SymbolType::Type), "delegate: {syms:?}");
        assert!(has("Count", SymbolType::Property), "{syms:?}");
        assert!(has("Run", SymbolType::Method), "{syms:?}");

        let go = "package p\ntype ID = string\ntype Reader interface{ Read() }\n";
        let syms = SymbolExtractor::new(Path::new("p.go")).extract(go).unwrap();
        let has = |n: &str, t: SymbolType| syms.iter().any(|s| s.name == n && s.symbol_type == t);
        assert!(has("ID", SymbolType::Type), "type alias: {syms:?}");
        assert!(has("Reader", SymbolType::Interface), "{syms:?}");

        // Case-insensitive extension.
        let syms = SymbolExtractor::new(Path::new("X.RS"))
            .extract("fn upper_ext() {}\n")
            .unwrap();
        assert!(syms.iter().any(|s| s.name == "upper_ext"), "{syms:?}");
        // Markup is not parsed at all.
        assert!(!SymbolExtractor::new(Path::new("package-lock.json")).is_supported());
    }

    #[test]
    fn test_typescript_enum_extraction() {
        let source = r#"
enum Status {
    Active,
    Inactive,
    Pending
}

enum Color {
    Red = "red",
    Green = "green",
    Blue = "blue"
}
"#;
        let extractor = SymbolExtractor::new(Path::new("test.ts"));
        let symbols = extractor.extract(source).unwrap();

        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Status" && s.symbol_type == SymbolType::Enum),
            "Should find Status enum"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Color" && s.symbol_type == SymbolType::Enum),
            "Should find Color enum"
        );
    }

    #[test]
    fn test_rust_comprehensive_extraction() {
        let source = r#"
struct Point {
    x: i32,
    y: i32,
}

enum Direction {
    North,
    South,
    East,
    West,
}

trait Drawable {
    fn draw(&self);
}

type Coordinate = (i32, i32);

const MAX_SIZE: usize = 100;

static GLOBAL_COUNT: u32 = 0;

fn process() {}

impl Point {
    fn new() -> Self {
        Point { x: 0, y: 0 }
    }
}
"#;
        let extractor = SymbolExtractor::new(Path::new("test.rs"));
        let symbols = extractor.extract(source).unwrap();

        // Should find struct
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Point" && s.symbol_type == SymbolType::Struct),
            "Should find Point struct"
        );

        // Should find enum
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Direction" && s.symbol_type == SymbolType::Enum),
            "Should find Direction enum"
        );

        // Should find trait
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Drawable" && s.symbol_type == SymbolType::Trait),
            "Should find Drawable trait"
        );

        // Should find type alias
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Coordinate" && s.symbol_type == SymbolType::Type),
            "Should find Coordinate type alias"
        );

        // Should find const
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "MAX_SIZE" && s.symbol_type == SymbolType::Constant),
            "Should find MAX_SIZE constant"
        );

        // Should find static
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "GLOBAL_COUNT" && s.symbol_type == SymbolType::Constant),
            "Should find GLOBAL_COUNT static"
        );

        // Should find function
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "process" && s.symbol_type == SymbolType::Function),
            "Should find process function"
        );
    }

    #[test]
    fn test_go_extraction() {
        let source = r#"
package main

type User struct {
    Name string
    Age  int
}

type Reader interface {
    Read(p []byte) (n int, err error)
}

type UserID = string

const MaxSize = 100

var GlobalCount = 0

func main() {
    fmt.Println("Hello")
}

func (u *User) GetName() string {
    return u.Name
}
"#;
        let extractor = SymbolExtractor::new(Path::new("test.go"));
        let symbols = extractor.extract(source).unwrap();

        assert!(
            symbols
                .iter()
                .any(|s| s.name == "User" && s.symbol_type == SymbolType::Struct),
            "Should find User struct"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Reader" && s.symbol_type == SymbolType::Interface),
            "Should find Reader interface"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "main" && s.symbol_type == SymbolType::Function),
            "Should find main function"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "GetName" && s.symbol_type == SymbolType::Method),
            "Should find GetName method"
        );
    }

    #[test]
    fn test_java_extraction() {
        let source = r#"
public class User {
    private String name;
    
    public User(String name) {
        this.name = name;
    }
    
    public String getName() {
        return name;
    }
}

interface Readable {
    void read();
}

enum Status {
    ACTIVE, INACTIVE
}
"#;
        let extractor = SymbolExtractor::new(Path::new("Test.java"));
        let symbols = extractor.extract(source).unwrap();

        assert!(
            symbols
                .iter()
                .any(|s| s.name == "User" && s.symbol_type == SymbolType::Class),
            "Should find User class"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Readable" && s.symbol_type == SymbolType::Interface),
            "Should find Readable interface"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Status" && s.symbol_type == SymbolType::Enum),
            "Should find Status enum"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "getName" && s.symbol_type == SymbolType::Method),
            "Should find getName method"
        );
    }

    #[test]
    fn test_csharp_extraction() {
        let source = r#"
public class User {
    public string Name { get; set; }
    
    public User(string name) {
        Name = name;
    }
    
    public void Greet() {
        Console.WriteLine("Hello");
    }
}

public struct Point {
    public int X;
    public int Y;
}

public interface IReadable {
    void Read();
}

public enum Status {
    Active,
    Inactive
}
"#;
        let extractor = SymbolExtractor::new(Path::new("Test.cs"));
        let symbols = extractor.extract(source).unwrap();

        assert!(
            symbols
                .iter()
                .any(|s| s.name == "User" && s.symbol_type == SymbolType::Class),
            "Should find User class"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Point" && s.symbol_type == SymbolType::Struct),
            "Should find Point struct"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "IReadable" && s.symbol_type == SymbolType::Interface),
            "Should find IReadable interface"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Status" && s.symbol_type == SymbolType::Enum),
            "Should find Status enum"
        );
    }

    #[test]
    fn test_cpp_extraction() {
        let source = r#"
class MyClass {
public:
    void doSomething();
};

struct Point {
    int x;
    int y;
};

enum Color {
    RED,
    GREEN,
    BLUE
};

namespace MyNamespace {
    void helper() {}
}

void globalFunction() {
}
"#;
        let extractor = SymbolExtractor::new(Path::new("test.cpp"));
        let symbols = extractor.extract(source).unwrap();

        assert!(
            symbols
                .iter()
                .any(|s| s.name == "MyClass" && s.symbol_type == SymbolType::Class),
            "Should find MyClass class"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Point" && s.symbol_type == SymbolType::Struct),
            "Should find Point struct"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Color" && s.symbol_type == SymbolType::Enum),
            "Should find Color enum"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "MyNamespace" && s.symbol_type == SymbolType::Module),
            "Should find MyNamespace namespace"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "globalFunction" && s.symbol_type == SymbolType::Function),
            "Should find globalFunction function"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "helper" && s.symbol_type == SymbolType::Function),
            "Should find helper function inside namespace"
        );
    }

    #[test]
    fn test_ruby_extraction() {
        let source = r#"
class User
  def initialize(name)
    @name = name
  end
  
  def greet
    puts "Hello"
  end
  
  def self.create
    new("default")
  end
end

module Helpers
  def format
  end
end
"#;
        let extractor = SymbolExtractor::new(Path::new("test.rb"));
        let symbols = extractor.extract(source).unwrap();

        assert!(
            symbols
                .iter()
                .any(|s| s.name == "User" && s.symbol_type == SymbolType::Class),
            "Should find User class"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Helpers" && s.symbol_type == SymbolType::Module),
            "Should find Helpers module"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "greet" && s.symbol_type == SymbolType::Method),
            "Should find greet method"
        );
    }

    #[test]
    fn test_php_extraction() {
        let source = r#"<?php
class User {
    public function __construct($name) {
        $this->name = $name;
    }
    
    public function greet() {
        echo "Hello";
    }
}

interface Readable {
    public function read();
}

trait Logger {
    public function log($message) {}
}

function helper() {
    return true;
}
"#;
        let extractor = SymbolExtractor::new(Path::new("test.php"));
        let symbols = extractor.extract(source).unwrap();

        assert!(
            symbols
                .iter()
                .any(|s| s.name == "User" && s.symbol_type == SymbolType::Class),
            "Should find User class"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Readable" && s.symbol_type == SymbolType::Interface),
            "Should find Readable interface"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Logger" && s.symbol_type == SymbolType::Trait),
            "Should find Logger trait"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "helper" && s.symbol_type == SymbolType::Function),
            "Should find helper function"
        );
    }

    #[test]
    fn test_bash_extraction() {
        let source = r#"#!/bin/bash

function greet() {
    echo "Hello, $1"
}

helper() {
    return 0
}
"#;
        let extractor = SymbolExtractor::new(Path::new("test.sh"));
        let symbols = extractor.extract(source).unwrap();

        assert!(
            symbols
                .iter()
                .any(|s| s.name == "greet" && s.symbol_type == SymbolType::Function),
            "Should find greet function"
        );
    }

    /// Fix #3: C++ template declarations should NOT produce duplicate symbols.
    /// A templated function like `template<class T> T max_val(T a, T b)` should
    /// yield exactly one symbol "max_val", not two.
    #[test]
    fn test_cpp_template_no_duplicate_symbols() {
        let source = r#"
template<class T>
T max_val(T a, T b) {
    return a > b ? a : b;
}

template<typename T>
class Container {
public:
    void add(T item);
};

void regular_function() {
}
"#;
        let extractor = SymbolExtractor::new(Path::new("test.cpp"));
        let symbols = extractor.extract(source).unwrap();

        // Count how many times max_val appears
        let max_val_count = symbols.iter().filter(|s| s.name == "max_val").count();
        assert_eq!(
            max_val_count, 1,
            "Templated function 'max_val' should appear exactly once, got {}",
            max_val_count
        );

        // Count how many times Container appears
        let container_count = symbols.iter().filter(|s| s.name == "Container").count();
        assert_eq!(
            container_count, 1,
            "Templated class 'Container' should appear exactly once, got {}",
            container_count
        );

        // Verify all expected symbols are present
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "max_val" && s.symbol_type == SymbolType::Function),
            "Should find max_val function"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Container" && s.symbol_type == SymbolType::Class),
            "Should find Container class"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "regular_function" && s.symbol_type == SymbolType::Function),
            "Should find regular_function"
        );
    }

    /// Roadmap 7: the tags queries' `@reference.*` captures are kept: call
    /// sites, type mentions and implemented traits come back as references
    /// with the name node's position, never as definitions.
    #[test]
    fn test_references_are_captured() {
        let source = "fn helper() {}\nfn main() {\n    helper();\n    other::helper();\n}\nimpl Display for Foo {}\n";
        let extractor = SymbolExtractor::new(Path::new("refs.rs"));
        let (symbols, _imports, refs) = extractor.extract_all_with_refs(source).unwrap();
        assert!(symbols
            .iter()
            .any(|s| s.name == "helper" && s.line == 0 && s.is_definition));
        let calls: Vec<(u32, u32)> = refs
            .iter()
            .filter(|r| r.name == "helper")
            .map(|r| (r.line, r.column))
            .collect();
        assert_eq!(calls, vec![(2, 4), (3, 11)], "{refs:?}");
        assert!(
            refs.iter().any(|r| r.name == "Display" && r.line == 5),
            "{refs:?}"
        );
        // The definition line is not a reference.
        assert!(!refs.iter().any(|r| r.name == "helper" && r.line == 0));

        let py = "def f():\n    pass\nf()\nobj.method(f)\n";
        let (_, _, refs) = SymbolExtractor::new(Path::new("a.py"))
            .extract_all_with_refs(py)
            .unwrap();
        let names: Vec<(&str, u32)> = refs.iter().map(|r| (r.name.as_str(), r.line)).collect();
        assert!(names.contains(&("f", 2)), "{names:?}");
        assert!(names.contains(&("method", 3)), "{names:?}");
    }

    /// Review 1.4: Rust type mentions (parameters, fields, generics, paths,
    /// impl targets) are references; a type's own definition is not.
    #[test]
    fn test_rust_type_mentions_are_references() {
        let source = "pub struct SearchEngine {\n    items: Vec<Item>,\n}\n\
                      fn run(e: &SearchEngine) -> Option<Item> { None }\n\
                      impl SearchEngine {}\n\
                      impl fmt::Display for Item {}\n\
                      enum Item { A }\n\
                      type Alias = crate::search::SearchEngine;\n";
        let extractor = SymbolExtractor::new(Path::new("types.rs"));
        let (symbols, _imports, refs) = extractor.extract_all_with_refs(source).unwrap();
        let mentions = |name: &str| -> Vec<(u32, u32)> {
            refs.iter()
                .filter(|r| r.name == name)
                .map(|r| (r.line, r.column))
                .collect()
        };
        // Definitions stay definitions and are not their own references.
        assert!(symbols
            .iter()
            .any(|s| s.name == "SearchEngine" && s.line == 0 && s.is_definition));
        assert!(!mentions("SearchEngine").contains(&(0, 11)), "{refs:?}");
        assert!(!mentions("Item").contains(&(6, 5)), "{refs:?}");
        assert!(!mentions("Alias").contains(&(7, 5)), "{refs:?}");
        // Parameter, impl target and scoped path.
        assert_eq!(
            mentions("SearchEngine"),
            vec![(3, 11), (4, 5), (7, 28)],
            "{refs:?}"
        );
        // Field generic argument, return generic argument, impl-for target.
        assert_eq!(
            mentions("Item"),
            vec![(1, 15), (3, 35), (5, 22)],
            "{refs:?}"
        );
        assert_eq!(mentions("Vec"), vec![(1, 11)]);
        assert_eq!(mentions("Display"), vec![(5, 10)]);
    }

    #[test]
    fn test_extract_all_matches_individual_methods() {
        let source = r#"
use std::collections::HashMap;
use crate::utils::helper;

fn process(data: &str) -> Vec<String> {
    vec![]
}

struct Config {
    debug: bool,
}
"#;
        let path = Path::new("test.rs");
        let extractor = SymbolExtractor::new(path);

        let symbols_only = extractor.extract(source).unwrap();
        let imports_only = extractor.extract_imports(source).unwrap();
        let (all_symbols, all_imports) = extractor.extract_all(source).unwrap();

        // extract_all must return the same symbols and imports as calling each separately
        assert_eq!(
            symbols_only.len(),
            all_symbols.len(),
            "extract_all symbol count should match extract"
        );
        assert_eq!(
            imports_only.len(),
            all_imports.len(),
            "extract_all import count should match extract_imports"
        );
        for (a, b) in symbols_only.iter().zip(all_symbols.iter()) {
            assert_eq!(a.name, b.name);
            assert_eq!(a.symbol_type, b.symbol_type);
            assert_eq!(a.line, b.line);
        }
        for (a, b) in imports_only.iter().zip(all_imports.iter()) {
            assert_eq!(a.path, b.path);
            assert_eq!(a.line, b.line);
        }
    }
}
