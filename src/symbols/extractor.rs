use anyhow::Result;
use std::path::Path;
use tree_sitter::{Parser, Language};

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

pub struct SymbolExtractor {
    language: Option<Language>,
}

impl SymbolExtractor {
    pub fn new(file_path: &Path) -> Self {
        let language = Self::language_for_file(file_path);
        Self { language }
    }

    fn language_for_file(path: &Path) -> Option<Language> {
        let extension = path.extension()?.to_str()?;
        
        match extension {
            "rs" => Some(tree_sitter_rust::language()),
            "py" => Some(tree_sitter_python::language()),
            "js" | "jsx" => Some(tree_sitter_javascript::language()),
            "ts" | "tsx" => Some(tree_sitter_typescript::language_typescript()),
            _ => None,
        }
    }

    pub fn extract(&self, source: &str) -> Result<Vec<Symbol>> {
        let language = match self.language {
            Some(lang) => lang,
            None => return Ok(Vec::new()), // No symbols for unknown languages
        };

        let mut parser = Parser::new();
        parser.set_language(language)?;

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
