use anyhow::Result;
use std::path::Path;
use tree_sitter::Parser;
use tree_sitter_language::LanguageFn;

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolType {
    Function,
    Class,
    Method,
    Variable,
    Constant,
    Interface,
    Type,
}

#[derive(Debug, Clone)]
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
            "rs" => Some(tree_sitter_rust::LANGUAGE),
            "py" => Some(tree_sitter_python::LANGUAGE),
            "js" | "jsx" => Some(tree_sitter_javascript::LANGUAGE),
            "ts" | "tsx" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT),
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
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "function_item" | "function_declaration" | "function_definition" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = &source[name_node.byte_range()];
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
                "impl_item" | "class_declaration" | "class_definition" => {
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
                _ => {}
            }

            // Recursively process children
            Self::extract_functions(&child, source, symbols);
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
            "py" => Self::extract_python_imports(&root_node, source, &mut imports),
            "js" | "jsx" | "ts" | "tsx" => {
                Self::extract_js_imports(&root_node, source, &mut imports)
            }
            _ => {}
        }

        imports.sort_by_key(|i| i.line);
        Ok(imports)
    }

    /// Extract Rust use statements and mod declarations
    fn extract_rust_imports(
        node: &tree_sitter::Node,
        source: &str,
        imports: &mut Vec<ImportStatement>,
    ) {
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
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

            // Recursively process children
            Self::extract_rust_imports(&child, source, imports);
        }
    }

    /// Extract Python import statements
    fn extract_python_imports(
        node: &tree_sitter::Node,
        source: &str,
        imports: &mut Vec<ImportStatement>,
    ) {
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
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

            Self::extract_python_imports(&child, source, imports);
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
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
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

            Self::extract_js_imports(&child, source, imports);
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
}
