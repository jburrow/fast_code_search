//! Semantic search engine
//!
//! Coordinates chunking, embedding, and vector search for semantic code search.

use super::{
    cache::QueryCache,
    chunking::{CodeChunk, CodeChunker},
    config::HnswConfig,
    embeddings::EmbeddingModel,
    vector_index::{HnswParams, VectorIndex},
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
    chunks: FxHashMap<u32, CodeChunk>, // chunk_id -> chunk
    next_chunk_id: u32,
    query_cache: QueryCache, // Cache for query embeddings
}

/// Search result
#[derive(Debug, Clone)]
pub struct SemanticSearchResult {
    pub chunk: CodeChunk,
    pub similarity_score: f32,
}

impl SemanticSearchEngine {
    /// Create new engine with default HNSW parameters
    pub fn new(chunk_size: usize, chunk_overlap: usize) -> Self {
        Self::with_hnsw_config(chunk_size, chunk_overlap, HnswConfig::default())
    }

    /// Create new engine with custom HNSW configuration
    pub fn with_hnsw_config(
        chunk_size: usize,
        chunk_overlap: usize,
        hnsw_config: HnswConfig,
    ) -> Self {
        let embedding_model = EmbeddingModel::new();
        let embedding_dim = embedding_model.embedding_dim();

        // Convert config to HNSW params
        let hnsw_params = HnswParams {
            m: hnsw_config.m,
            ef_construction: hnsw_config.ef_construction,
            ef_search: hnsw_config.ef_search,
        };

        let vector_index = VectorIndex::with_params(embedding_dim, hnsw_params);
        let chunker = CodeChunker::new(chunk_size, chunk_overlap);

        Self {
            vector_index,
            embedding_model,
            chunker,
            chunks: FxHashMap::default(),
            next_chunk_id: 0,
            query_cache: QueryCache::new(100), // Cache up to 100 queries
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
    #[tracing::instrument(skip(self))]
    pub fn search(&mut self, query: &str, max_results: usize) -> Result<Vec<SemanticSearchResult>> {
        info!(query = query, max_results = max_results, "Semantic search");

        // Check cache first
        let query_embedding = if let Some(cached) = self.query_cache.get(query) {
            debug!("Using cached query embedding");
            cached.clone()
        } else {
            // Encode query
            let embedding = self.embedding_model.encode(query)?;
            // Cache for future use
            self.query_cache
                .insert(query.to_string(), embedding.clone());
            embedding
        };

        // Search vector index
        let neighbors = self.vector_index.search(&query_embedding, max_results);

        // Build results
        let results: Vec<SemanticSearchResult> = neighbors
            .into_iter()
            .filter_map(|(chunk_id, similarity)| {
                self.chunks
                    .get(&chunk_id)
                    .map(|chunk| SemanticSearchResult {
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
            cache_size: self.query_cache.len(),
        }
    }

    /// Save index to disk
    pub fn save_index(&self, path: &Path) -> Result<()> {
        info!(path = %path.display(), "Saving semantic index");

        // Save vector index
        let index_path = path.with_extension("index");
        self.vector_index.save(&index_path)?;

        // Save chunks metadata
        let chunks_path = path.with_extension("chunks");
        let chunks_data = bincode::serialize(&(&self.chunks, self.next_chunk_id))?;
        std::fs::write(chunks_path, chunks_data)?;

        info!("Index saved successfully");
        Ok(())
    }

    /// Load index from disk
    pub fn load_index(&mut self, path: &Path) -> Result<()> {
        info!(path = %path.display(), "Loading semantic index");

        // Load vector index
        let index_path = path.with_extension("index");
        self.vector_index = VectorIndex::load(&index_path)?;

        // Load chunks metadata
        let chunks_path = path.with_extension("chunks");
        let chunks_data = std::fs::read(chunks_path)?;
        let (chunks, next_id): (FxHashMap<u32, CodeChunk>, u32) =
            bincode::deserialize(&chunks_data)?;

        self.chunks = chunks;
        self.next_chunk_id = next_id;

        info!(num_chunks = self.chunks.len(), "Index loaded successfully");
        Ok(())
    }
}

/// Engine statistics
#[derive(Debug, Clone)]
pub struct EngineStats {
    pub num_chunks: usize,
    pub num_files: usize,
    pub embedding_dim: usize,
    pub cache_size: usize,
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
        engine
            .index_file(Path::new("test1.rs"), "fn main() {}")
            .unwrap();
        engine
            .index_file(Path::new("test2.rs"), "fn test() {}")
            .unwrap();

        let stats = engine.get_stats();
        assert_eq!(stats.num_files, 2);
        assert!(stats.num_chunks >= 2);
    }

    #[test]
    fn test_query_cache() {
        let mut engine = SemanticSearchEngine::new(10, 2);
        engine
            .index_file(Path::new("test.rs"), "fn main() {}")
            .unwrap();

        // First search - not cached
        let _ = engine.search("test query", 5).unwrap();

        // Second search - should use cache
        let _ = engine.search("test query", 5).unwrap();

        let stats = engine.get_stats();
        assert_eq!(stats.cache_size, 1);
    }

    #[test]
    fn test_save_and_load() {
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let index_path = dir.path().join("test_index");

        // Create and save engine
        {
            let mut engine = SemanticSearchEngine::new(10, 2);
            engine
                .index_file(Path::new("test.rs"), "fn main() {}")
                .unwrap();
            engine.save_index(&index_path).unwrap();
        }

        // Load engine
        {
            let mut engine = SemanticSearchEngine::new(10, 2);
            engine.load_index(&index_path).unwrap();

            let stats = engine.get_stats();
            assert!(stats.num_chunks > 0);
        }
    }
}
