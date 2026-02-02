//! Semantic search engine
//!
//! Coordinates chunking, embedding, and vector search for semantic code search.

use super::{
    chunking::{CodeChunk, CodeChunker},
    embeddings::EmbeddingModel,
    vector_index::VectorIndex,
};
use anyhow::Result;
use rustc_hash::FxHashMap;
use std::path::Path;
use tracing::{debug, info};

/// Semantic search engine
pub struct SemanticSearchEngine {
    vector_index: VectorIndex,
    embedding_model: EmbeddingModel,
    chunker: CodeChunker,
    chunks: FxHashMap<u32, CodeChunk>,  // chunk_id -> chunk
    next_chunk_id: u32,
}

/// Search result
#[derive(Debug, Clone)]
pub struct SemanticSearchResult {
    pub chunk: CodeChunk,
    pub similarity_score: f32,
}

impl SemanticSearchEngine {
    /// Create new engine
    pub fn new(chunk_size: usize, chunk_overlap: usize) -> Self {
        let embedding_model = EmbeddingModel::new();
        let embedding_dim = embedding_model.embedding_dim();
        let vector_index = VectorIndex::new(embedding_dim);
        let chunker = CodeChunker::new(chunk_size, chunk_overlap);

        Self {
            vector_index,
            embedding_model,
            chunker,
            chunks: FxHashMap::default(),
            next_chunk_id: 0,
        }
    }

    /// Index a file
    pub fn index_file(&mut self, file_path: &Path, content: &str) -> Result<usize> {
        debug!(path = %file_path.display(), "Indexing file");

        // Chunk the file
        let file_chunks = self.chunker.chunk_file(content, file_path);
        let num_chunks = file_chunks.len();

        // Process each chunk
        for chunk in file_chunks {
            // Generate embedding
            let embedding = self.embedding_model.encode(&chunk.text)?;

            // Assign chunk ID
            let chunk_id = self.next_chunk_id;
            self.next_chunk_id += 1;

            // Add to vector index
            self.vector_index.add(chunk_id, embedding)?;

            // Store chunk metadata
            self.chunks.insert(chunk_id, chunk);
        }

        Ok(num_chunks)
    }

    /// Search with natural language query
    pub fn search(&mut self, query: &str, max_results: usize) -> Result<Vec<SemanticSearchResult>> {
        info!(query = query, max_results = max_results, "Semantic search");

        // Encode query
        let query_embedding = self.embedding_model.encode(query)?;

        // Search vector index
        let neighbors = self.vector_index.search(&query_embedding, max_results);

        // Build results
        let results: Vec<SemanticSearchResult> = neighbors
            .into_iter()
            .filter_map(|(chunk_id, similarity)| {
                self.chunks.get(&chunk_id).map(|chunk| SemanticSearchResult {
                    chunk: chunk.clone(),
                    similarity_score: similarity,
                })
            })
            .collect();

        debug!(results_count = results.len(), "Search completed");

        Ok(results)
    }

    /// Get statistics
    pub fn get_stats(&self) -> EngineStats {
        let unique_files: std::collections::HashSet<_> = 
            self.chunks.values().map(|c| &c.file_path).collect();

        EngineStats {
            num_chunks: self.chunks.len(),
            num_files: unique_files.len(),
            embedding_dim: self.embedding_model.embedding_dim(),
        }
    }
}

/// Engine statistics
#[derive(Debug, Clone)]
pub struct EngineStats {
    pub num_chunks: usize,
    pub num_files: usize,
    pub embedding_dim: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_and_search() {
        let mut engine = SemanticSearchEngine::new(10, 2);

        // Index a simple file
        let content = "fn authenticate_user() {\n    // Login logic\n}";
        let result = engine.index_file(Path::new("test.rs"), content);
        assert!(result.is_ok());

        // Search
        let results = engine.search("authentication", 5).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_stats() {
        let mut engine = SemanticSearchEngine::new(10, 2);
        engine.index_file(Path::new("test1.rs"), "fn main() {}").unwrap();
        engine.index_file(Path::new("test2.rs"), "fn test() {}").unwrap();

        let stats = engine.get_stats();
        assert_eq!(stats.num_files, 2);
        assert!(stats.num_chunks >= 2);
    }
}
