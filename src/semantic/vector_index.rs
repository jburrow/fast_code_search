//! Vector search index with HNSW (Hierarchical Navigable Small World)
//!
//! Implements fast approximate nearest neighbor search for semantic code search.
//! Uses HNSW algorithm for O(log n) search complexity instead of O(n) linear search.

use anyhow::Result;
use hnsw_rs::prelude::*;
use std::path::Path;

/// Maximum number of layers in the HNSW graph
/// Using 16 provides good balance between depth and construction time
const HNSW_MAX_LAYER: usize = 16;

/// Configuration for HNSW index
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HnswParams {
    /// Number of bi-directional links per element (M parameter)
    pub m: usize,
    /// Size of dynamic candidate list during construction
    pub ef_construction: usize,
    /// Size of dynamic candidate list during search
    pub ef_search: usize,
}

impl Default for HnswParams {
    fn default() -> Self {
        Self {
            m: 16,
            ef_construction: 200,
            ef_search: 100,
        }
    }
}

/// Vector search index using HNSW for fast approximate nearest neighbor search
pub struct VectorIndex {
    hnsw: Hnsw<'static, f32, DistCosine>,
    embeddings: Vec<Vec<f32>>, // Store embeddings for persistence
    chunk_ids: Vec<u32>,       // Maps HNSW index to chunk ID
    embedding_dim: usize,
    ef_search: usize,        // Search parameter (can be adjusted at query time)
    next_id: usize,          // Next ID to assign
    hnsw_params: HnswParams, // Store params for rebuilding
}

impl VectorIndex {
    /// Create new vector index with default HNSW parameters
    pub fn new(embedding_dim: usize) -> Self {
        Self::with_params(embedding_dim, HnswParams::default())
    }

    /// Create new vector index with custom HNSW parameters
    pub fn with_params(embedding_dim: usize, params: HnswParams) -> Self {
        // Initial capacity estimate
        let initial_capacity = 1000;

        // Create HNSW index
        // Parameters: max_nb_connection (M), max_elements, max_layer, ef_construction, distance
        let hnsw = Hnsw::<f32, DistCosine>::new(
            params.m,
            initial_capacity,
            HNSW_MAX_LAYER,
            params.ef_construction,
            DistCosine {},
        );

        Self {
            hnsw,
            embeddings: Vec::new(),
            chunk_ids: Vec::new(),
            embedding_dim,
            ef_search: params.ef_search,
            next_id: 0,
            hnsw_params: params,
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

        // Insert into HNSW with internal ID
        self.hnsw.insert((&embedding, self.next_id));

        // Store embedding for persistence
        self.embeddings.push(embedding);
        self.chunk_ids.push(chunk_id);
        self.next_id += 1;

        Ok(())
    }

    /// Search for k nearest neighbors using HNSW
    pub fn search(&self, query_embedding: &[f32], k: usize) -> Vec<(u32, f32)> {
        if query_embedding.len() != self.embedding_dim {
            return Vec::new();
        }

        if self.is_empty() {
            return Vec::new();
        }

        // Search using HNSW
        let neighbors = self.hnsw.search(query_embedding, k, self.ef_search);

        // Convert HNSW results to (chunk_id, similarity) pairs
        // HNSW returns Vec<Neighbour> with (DataId, distance)
        // We need to convert distance to similarity and map to chunk_id
        neighbors
            .iter()
            .filter_map(|n| {
                let internal_id = n.d_id;
                if internal_id < self.chunk_ids.len() {
                    let chunk_id = self.chunk_ids[internal_id];
                    // Convert cosine distance to similarity: similarity = 1 - distance
                    let similarity = 1.0 - n.distance;
                    Some((chunk_id, similarity))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get number of indexed embeddings
    pub fn len(&self) -> usize {
        self.chunk_ids.len()
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.chunk_ids.is_empty()
    }

    /// Save index to disk
    pub fn save(&self, path: &Path) -> Result<()> {
        // Save embeddings, chunk_ids, and all metadata
        let data = bincode::serialize(&(
            &self.embeddings,
            &self.chunk_ids,
            self.embedding_dim,
            self.ef_search,
            self.next_id,
            &self.hnsw_params,
        ))?;

        std::fs::write(path, data)?;
        Ok(())
    }

    /// Load index from disk
    pub fn load(path: &Path) -> Result<Self> {
        // Load all data
        let data = std::fs::read(path)?;
        let (embeddings, chunk_ids, embedding_dim, ef_search, next_id, hnsw_params): (
            Vec<Vec<f32>>,
            Vec<u32>,
            usize,
            usize,
            usize,
            HnswParams,
        ) = bincode::deserialize(&data)?;

        // Rebuild HNSW index from embeddings
        let initial_capacity = embeddings.len().max(1000);
        let hnsw = Hnsw::<f32, DistCosine>::new(
            hnsw_params.m,
            initial_capacity,
            HNSW_MAX_LAYER,
            hnsw_params.ef_construction,
            DistCosine {},
        );

        let mut index = Self {
            hnsw,
            embeddings: Vec::new(),
            chunk_ids: Vec::new(),
            embedding_dim,
            ef_search,
            next_id: 0,
            hnsw_params,
        };

        // Re-insert all embeddings
        for (i, embedding) in embeddings.into_iter().enumerate() {
            index.hnsw.insert((&embedding, i));
            index.embeddings.push(embedding);
            if i < chunk_ids.len() {
                index.chunk_ids.push(chunk_ids[i]);
            }
        }

        index.next_id = next_id;

        Ok(index)
    }
}

/// Compute cosine similarity between two vectors
/// Kept for backward compatibility and testing
#[allow(dead_code)]
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
    use tempfile::NamedTempFile;

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

        // First result should be chunk_id 0 with high similarity
        assert_eq!(results[0].0, 0);
        assert!(results[0].1 > 0.9); // Should be very similar to [1,0,0]
    }

    #[test]
    fn test_dimension_mismatch() {
        let mut index = VectorIndex::new(3);
        let result = index.add(0, vec![1.0, 0.0]); // Wrong dimension
        assert!(result.is_err());
    }

    #[test]
    fn test_save_and_load() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Create and populate index
        let mut index = VectorIndex::new(3);
        index.add(0, vec![1.0, 0.0, 0.0]).unwrap();
        index.add(1, vec![0.0, 1.0, 0.0]).unwrap();
        index.add(2, vec![0.0, 0.0, 1.0]).unwrap();

        // Save
        index.save(path).unwrap();

        // Load
        let loaded = VectorIndex::load(path).unwrap();
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded.embedding_dim, 3);

        // Verify search works on loaded index
        let results = loaded.search(&[1.0, 0.0, 0.0], 2);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 0);
    }

    #[test]
    fn test_custom_params() {
        let params = HnswParams {
            m: 8,
            ef_construction: 100,
            ef_search: 50,
        };
        let mut index = VectorIndex::with_params(3, params);

        index.add(0, vec![1.0, 0.0, 0.0]).unwrap();
        index.add(1, vec![0.0, 1.0, 0.0]).unwrap();

        let results = index.search(&[1.0, 0.0, 0.0], 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 0);
    }
}
