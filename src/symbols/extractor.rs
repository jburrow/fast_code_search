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
    Enum,
    Trait,
    Struct,
    /// File name - indexed for path-based searches
    FileName,
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
        // Use iterative traversal with explicit stack to avoid stack overflow
        // on deeply nested code
        let mut stack = vec![*node];

        while let Some(current) = stack.pop() {
            let mut cursor = current.walk();

            for child in current.children(&mut cursor) {
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
                    "class_declaration" | "class_definition" => {
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
                    // Enums: TypeScript enum_declaration, Rust enum_item
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
                    // Rust structs
                    "struct_item" => {
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
}
