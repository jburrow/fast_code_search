//! Simple vector search index
//!
//! Implements basic nearest neighbor search for semantic code search.
//! Uses linear search for now (can be upgraded to HNSW later).

use anyhow::Result;
use std::path::Path;

/// Simple vector search index
pub struct VectorIndex {
    embeddings: Vec<Vec<f32>>,
    chunk_ids: Vec<u32>,  // Maps index position to chunk ID
    embedding_dim: usize,
}

impl VectorIndex {
    /// Create new vector index
    pub fn new(embedding_dim: usize) -> Self {
        Self {
            embeddings: Vec::new(),
            chunk_ids: Vec::new(),
            embedding_dim,
        }
    }

    /// Add embedding to index
    pub fn add(&mut self, chunk_id: u32, embedding: Vec<f32>) -> Result<()> {
        if embedding.len() != self.embedding_dim {
            anyhow::bail!(
                "Embedding dimension mismatch: expected {}, got {}",
                self.embedding_dim,
                embedding.len()
            );
        }

        self.embeddings.push(embedding);
        self.chunk_ids.push(chunk_id);

        Ok(())
    }

    /// Search for k nearest neighbors using linear search
    pub fn search(&self, query_embedding: &[f32], k: usize) -> Vec<(u32, f32)> {
        if query_embedding.len() != self.embedding_dim {
            return Vec::new();
        }

        let mut results: Vec<(u32, f32)> = self
            .embeddings
            .iter()
            .zip(&self.chunk_ids)
            .map(|(emb, &chunk_id)| {
                let similarity = cosine_similarity(query_embedding, emb);
                (chunk_id, similarity)
            })
            .collect();

        // Sort by similarity (descending)
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top k
        results.truncate(k);

        results
    }

    /// Get number of indexed embeddings
    pub fn len(&self) -> usize {
        self.embeddings.len()
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.embeddings.is_empty()
    }

    /// Save index to disk
    pub fn save(&self, path: &Path) -> Result<()> {
        let data = bincode::serialize(&(&self.embeddings, &self.chunk_ids, self.embedding_dim))?;
        std::fs::write(path, data)?;
        Ok(())
    }

    /// Load index from disk
    pub fn load(path: &Path) -> Result<Self> {
        let data = std::fs::read(path)?;
        let (embeddings, chunk_ids, embedding_dim): (Vec<Vec<f32>>, Vec<u32>, usize) =
            bincode::deserialize(&data)?;

        Ok(Self {
            embeddings,
            chunk_ids,
            embedding_dim,
        })
    }
}

/// Compute cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_search() {
        let mut index = VectorIndex::new(3);

        // Add some vectors
        index.add(0, vec![1.0, 0.0, 0.0]).unwrap();
        index.add(1, vec![0.0, 1.0, 0.0]).unwrap();
        index.add(2, vec![0.0, 0.0, 1.0]).unwrap();

        assert_eq!(index.len(), 3);

        // Search for nearest to [1, 0, 0]
        let results = index.search(&[1.0, 0.0, 0.0], 2);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 0); // Should find exact match first
        assert!((results[0].1 - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_dimension_mismatch() {
        let mut index = VectorIndex::new(3);
        let result = index.add(0, vec![1.0, 0.0]); // Wrong dimension
        assert!(result.is_err());
    }
}
