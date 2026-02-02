//! Code chunking strategies for semantic search
//!
//! Chunks code into manageable pieces for embedding and search.
//! Supports both symbol-aware chunking (using tree-sitter) and
//! fixed-size chunking with overlap.

use crate::symbols::SymbolExtractor;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Type of code chunk
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ChunkType {
    Fixed,            // Fixed-size chunk
    Function(String), // Function with name
    Class(String),    // Class with name
    Module,           // Module/file level
}

/// A chunk of code with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeChunk {
    pub text: String,
    pub start_line: usize,
    pub end_line: usize,
    pub chunk_type: ChunkType,
    pub file_path: String,
}

/// Code chunking strategy
pub struct CodeChunker {
    chunk_size: usize,
    chunk_overlap: usize,
}

impl CodeChunker {
    pub fn new(chunk_size: usize, chunk_overlap: usize) -> Self {
        Self {
            chunk_size,
            chunk_overlap,
        }
    }

    /// Chunk a file into code chunks
    pub fn chunk_file(&self, content: &str, file_path: &Path) -> Vec<CodeChunk> {
        let file_path_str = file_path.to_string_lossy().to_string();

        // Try symbol-based chunking first
        if let Some(chunks) = self.chunk_by_symbols(content, &file_path_str) {
            return chunks;
        }

        // Fallback to fixed-size chunking
        self.chunk_by_size(content, &file_path_str)
    }

    /// Chunk by symbols (functions, classes)
    fn chunk_by_symbols(&self, content: &str, file_path: &str) -> Option<Vec<CodeChunk>> {
        let extractor = SymbolExtractor::new(Path::new(file_path));
        let symbols = extractor.extract(content).ok()?;

        if symbols.is_empty() {
            return None;
        }

        let lines: Vec<&str> = content.lines().collect();
        let mut chunks = Vec::new();

        for symbol in symbols {
            let start_line = symbol.line.saturating_sub(1);
            let end_line = (start_line + 50).min(lines.len()); // Max 50 lines per symbol

            let chunk_lines = &lines[start_line..end_line];
            let chunk_text = chunk_lines.join("\n");

            let chunk_type = match symbol.symbol_type {
                crate::symbols::SymbolType::Function | crate::symbols::SymbolType::Method => {
                    ChunkType::Function(symbol.name.clone())
                }
                crate::symbols::SymbolType::Class | crate::symbols::SymbolType::Type => {
                    ChunkType::Class(symbol.name.clone())
                }
                _ => ChunkType::Module,
            };

            chunks.push(CodeChunk {
                text: chunk_text,
                start_line: start_line + 1,
                end_line,
                chunk_type,
                file_path: file_path.to_string(),
            });
        }

        Some(chunks)
    }

    /// Chunk by fixed size with overlap
    fn chunk_by_size(&self, content: &str, file_path: &str) -> Vec<CodeChunk> {
        let lines: Vec<&str> = content.lines().collect();
        let mut chunks = Vec::new();
        let mut i = 0;

        while i < lines.len() {
            let end = (i + self.chunk_size).min(lines.len());
            let chunk_lines = &lines[i..end];

            chunks.push(CodeChunk {
                text: chunk_lines.join("\n"),
                start_line: i + 1,
                end_line: end,
                chunk_type: ChunkType::Fixed,
                file_path: file_path.to_string(),
            });

            i += self.chunk_size - self.chunk_overlap;
        }

        chunks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_by_size() {
        let chunker = CodeChunker::new(10, 2);
        let content = (0..25)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = chunker.chunk_by_size(&content, "test.rs");

        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].start_line, 1);
        assert!(chunks[0].end_line <= 11);
    }

    #[test]
    fn test_chunk_file() {
        let chunker = CodeChunker::new(10, 2);
        let content = "fn main() {\n    println!(\"Hello\");\n}";
        let chunks = chunker.chunk_file(content, Path::new("test.rs"));

        assert!(!chunks.is_empty());
    }
}
