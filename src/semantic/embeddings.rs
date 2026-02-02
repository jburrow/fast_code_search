//! Simple embedding implementation for semantic search
//!
//! This is a simplified implementation using TF-IDF-like vectors
//! as a placeholder for full ML-based embeddings (CodeBERT, etc.)
//! 
//! Future enhancement: Replace with ONNX Runtime + CodeBERT

use anyhow::Result;
use rustc_hash::FxHashMap;

/// Simple embedding model using TF-IDF-style vectors
pub struct EmbeddingModel {
    vocabulary: FxHashMap<String, usize>,
    embedding_dim: usize,
}

impl EmbeddingModel {
    /// Create a new embedding model
    pub fn new() -> Self {
        Self {
            vocabulary: FxHashMap::default(),
            embedding_dim: 128, // Fixed dimension for now
        }
    }

    /// Get embedding dimension
    pub fn embedding_dim(&self) -> usize {
        self.embedding_dim
    }

    /// Encode text to embedding vector
    /// Uses simple word frequency based approach
    pub fn encode(&mut self, text: &str) -> Result<Vec<f32>> {
        let words = self.tokenize(text);
        let mut embedding = vec![0.0f32; self.embedding_dim];

        // Simple hash-based embedding
        for word in words {
            let idx = self.get_or_create_word_index(&word);
            if idx < self.embedding_dim {
                embedding[idx] += 1.0;
            }
        }

        // Normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in &mut embedding {
                *val /= norm;
            }
        }

        Ok(embedding)
    }

    /// Encode batch of texts
    pub fn encode_batch(&mut self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        texts.iter().map(|&text| self.encode(text)).collect()
    }

    /// Simple tokenization
    fn tokenize(&self, text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty() && s.len() > 2)
            .map(|s| s.to_string())
            .collect()
    }

    /// Get or create index for a word
    fn get_or_create_word_index(&mut self, word: &str) -> usize {
        if let Some(&idx) = self.vocabulary.get(word) {
            idx
        } else {
            // Simple hash to index mapping
            let idx = word.bytes().map(|b| b as usize).sum::<usize>() % self.embedding_dim;
            self.vocabulary.insert(word.to_string(), idx);
            idx
        }
    }
}

impl Default for EmbeddingModel {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute cosine similarity between two vectors
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
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
    fn test_encode() {
        let mut model = EmbeddingModel::new();
        let embedding = model.encode("function authenticate user").unwrap();
        assert_eq!(embedding.len(), 128);
        
        // Check normalization
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01 || norm == 0.0);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 0.01);

        let c = vec![0.0, 1.0, 0.0];
        let sim2 = cosine_similarity(&a, &c);
        assert!((sim2 - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_similar_texts() {
        let mut model = EmbeddingModel::new();
        let emb1 = model.encode("authenticate user login").unwrap();
        let emb2 = model.encode("user authentication login").unwrap();
        let emb3 = model.encode("database connection pool").unwrap();

        let sim_12 = cosine_similarity(&emb1, &emb2);
        let sim_13 = cosine_similarity(&emb1, &emb3);

        // Similar texts should have higher similarity
        assert!(sim_12 > sim_13);
    }
}
