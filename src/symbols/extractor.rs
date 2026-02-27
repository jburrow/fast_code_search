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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub symbol_type: SymbolType,
    pub line: usize,
    pub column: usize,
    pub is_definition: bool,
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

impl SymbolExtractor {
    pub fn new(file_path: &Path) -> Self {
        let language = Self::language_for_file(file_path);
        let extension = file_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        Self {
            language,
            extension,
        }
    }

    fn language_for_file(path: &Path) -> Option<LanguageFn> {
        let extension = path.extension()?.to_str()?;

        match extension {
            // Core programming languages
            "rs" => Some(tree_sitter_rust::LANGUAGE),
            "py" | "pyi" | "pyw" => Some(tree_sitter_python::LANGUAGE),
            "js" | "jsx" | "mjs" | "cjs" => Some(tree_sitter_javascript::LANGUAGE),
            "ts" | "tsx" | "mts" | "cts" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT),
            "go" => Some(tree_sitter_go::LANGUAGE),
            "c" | "h" => Some(tree_sitter_c::LANGUAGE),
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => Some(tree_sitter_cpp::LANGUAGE),
            "java" => Some(tree_sitter_java::LANGUAGE),
            "cs" => Some(tree_sitter_c_sharp::LANGUAGE),
            "rb" | "rake" | "gemspec" => Some(tree_sitter_ruby::LANGUAGE),
            "php" => Some(tree_sitter_php::LANGUAGE_PHP),
            "sh" | "bash" | "zsh" => Some(tree_sitter_bash::LANGUAGE),
            // Config and markup languages
            "json" | "jsonc" => Some(tree_sitter_json::LANGUAGE),
            "toml" => Some(tree_sitter_toml_ng::LANGUAGE),
            "yaml" | "yml" => Some(tree_sitter_yaml::LANGUAGE),
            "html" | "htm" => Some(tree_sitter_html::LANGUAGE),
            "css" | "scss" => Some(tree_sitter_css::LANGUAGE),
            "md" | "markdown" => Some(tree_sitter_md::LANGUAGE),
            _ => None,
        }
    }

    pub fn extract(&self, source: &str) -> Result<Vec<Symbol>> {
        let language = match self.language {
            Some(lang) => lang,
            None => return Ok(Vec::new()), // No symbols for unknown languages
        };

        let mut parser = Parser::new();
        parser.set_language(&language.into())?;

        let tree = match parser.parse(source, None) {
            Some(tree) => tree,
            None => return Ok(Vec::new()),
        };

        let mut symbols = Vec::new();
        let root_node = tree.root_node();

        // Extract function definitions
        Self::extract_functions(&root_node, source, &mut symbols);

        symbols.sort_by_key(|s| s.line);
        Ok(symbols)
    }

    fn extract_functions(node: &tree_sitter::Node, source: &str, symbols: &mut Vec<Symbol>) {
        // Use iterative traversal with explicit stack to avoid stack overflow
        // on deeply nested code
        let mut stack = vec![*node];

        while let Some(current) = stack.pop() {
            let mut cursor = current.walk();

            for child in current.children(&mut cursor) {
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
                            let name_text = if name_node.kind() == "function_declarator" {
                                name_node
                                    .child_by_field_name("declarator")
                                    .map(|n| &source[n.byte_range()])
                            } else {
                                Some(&source[name_node.byte_range()])
                            };
                            if let Some(name) = name_text {
                                let start = child.start_position();
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
                            let start = child.start_position();
                            symbols.push(Symbol {
                                name: name.to_string(),
                                symbol_type: SymbolType::Method,
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
                            let start = child.start_position();
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
                            let start = child.start_position();
                            symbols.push(Symbol {
                                name: name.to_string(),
                                symbol_type: SymbolType::Method,
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
                            let start = child.start_position();
                            symbols.push(Symbol {
                                name: name.to_string(),
                                symbol_type: SymbolType::Class,
                                line: start.row,
                                column: start.column,
                                is_definition: true,
                            });
                        }
                    }
                    // Ruby modules (similar to classes)
                    "module" => {
                        if let Some(name_node) = child.child_by_field_name("name") {
                            let name = &source[name_node.byte_range()];
                            let start = child.start_position();
                            symbols.push(Symbol {
                                name: name.to_string(),
                                symbol_type: SymbolType::Class,
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
                            let start = child.start_position();
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
                            let start = child.start_position();
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
                            let start = child.start_position();
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
                            let start = child.start_position();
                            symbols.push(Symbol {
                                name: name.to_string(),
                                symbol_type: SymbolType::Enum,
                                line: start.row,
                                column: start.column,
                                is_definition: true,
                            });
                        }
                    }
                    // Records: Java, C#
                    "record_declaration" | "record_struct_declaration" => {
                        if let Some(name_node) = child.child_by_field_name("name") {
                            let name = &source[name_node.byte_range()];
                            let start = child.start_position();
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
                            let start = child.start_position();
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
                            let start = child.start_position();
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
                            let start = child.start_position();
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
                            let start = child.start_position();
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
                            if type_child.kind() == "type_spec" {
                                if let Some(name_node) = type_child.child_by_field_name("name") {
                                    let name = &source[name_node.byte_range()];
                                    let start = type_child.start_position();
                                    let symbol_type = if let Some(type_node) =
                                        type_child.child_by_field_name("type")
                                    {
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
                                    let start = spec.start_position();
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
                            let start = child.start_position();
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
                            let start = child.start_position();
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
                            let start = child.start_position();
                            symbols.push(Symbol {
                                name: name.to_string(),
                                symbol_type: SymbolType::Class,
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

                // Add child to stack for iterative processing
                stack.push(child);
            }
        }
    }

    /// Extract import statements from source code
    pub fn extract_imports(&self, source: &str) -> Result<Vec<ImportStatement>> {
        let language = match self.language {
            Some(lang) => lang,
            None => return Ok(Vec::new()),
        };

        let mut parser = Parser::new();
        parser.set_language(&language.into())?;

        let tree = match parser.parse(source, None) {
            Some(tree) => tree,
            None => return Ok(Vec::new()),
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
        let language = match self.language {
            Some(lang) => lang,
            None => return Ok((Vec::new(), Vec::new())),
        };

        let mut parser = Parser::new();
        parser.set_language(&language.into())?;

        let tree = match parser.parse(source, None) {
            Some(tree) => tree,
            None => return Ok((Vec::new(), Vec::new())),
        };

        let root_node = tree.root_node();

        let mut symbols = Vec::new();
        Self::extract_functions(&root_node, source, &mut symbols);
        symbols.sort_by_key(|s| s.line);

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

        Ok((symbols, imports))
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
                    "import_statement" | "import_from_statement" => {
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
                    "import_statement" => {
                        // import { foo } from './bar'
                        if let Some(source_node) = child.child_by_field_name("source") {
                            let path = &source[source_node.byte_range()];
                            // Remove quotes
                            let path = path.trim_matches(|c| c == '"' || c == '\'');
                            imports.push(ImportStatement {
                                path: path.to_string(),
                                line: child.start_position().row,
                                import_type: ImportType::JavaScript,
                            });
                        }
                    }
                    "call_expression" => {
                        // require('./foo')
                        if let Some(func_node) = child.child_by_field_name("function") {
                            let func_name = &source[func_node.byte_range()];
                            if func_name == "require" {
                                if let Some(args_node) = child.child_by_field_name("arguments") {
                                    let args_text = &source[args_node.byte_range()];
                                    let path = args_text
                                        .trim_matches(|c| c == '(' || c == ')')
                                        .trim_matches(|c| c == '"' || c == '\'');
                                    if !path.is_empty() {
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
                .any(|s| s.name == "MyNamespace" && s.symbol_type == SymbolType::Class),
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
                .any(|s| s.name == "Helpers" && s.symbol_type == SymbolType::Class),
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
