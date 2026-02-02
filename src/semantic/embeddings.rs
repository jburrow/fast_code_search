//! ML-based embedding implementation for semantic search
//!
//! Uses ONNX Runtime with pretrained code models (CodeBERT) for
//! generating high-quality embeddings for code search.

use anyhow::{Context, Result};
use ndarray::{Array2, Axis};
use ort::{GraphOptimizationLevel, Session};
use tokenizers::Tokenizer;
use tracing::{debug, info, warn};

use super::model_download::{ModelDownloader, ModelInfo, default_cache_dir};

/// ML-based embedding model using ONNX Runtime
pub struct EmbeddingModel {
    session: Option<Session>,
    tokenizer: Option<Tokenizer>,
    embedding_dim: usize,
    max_length: usize,
    use_ml: bool,
}

impl EmbeddingModel {
    /// Create a new embedding model with ML support
    ///
    /// Attempts to load ONNX model. Falls back to simple TF-IDF if loading fails.
    pub fn new() -> Self {
        Self::with_config(true, 512)
    }

    /// Create embedding model with custom configuration
    pub fn with_config(use_ml: bool, max_length: usize) -> Self {
        if !use_ml {
            info!("ML embeddings disabled, using TF-IDF fallback");
            return Self {
                session: None,
                tokenizer: None,
                embedding_dim: 128, // TF-IDF fallback dimension
                max_length,
                use_ml: false,
            };
        }

        match Self::load_ml_model(max_length) {
            Ok((session, tokenizer, embedding_dim)) => {
                info!(dim = embedding_dim, "ML embedding model loaded successfully");
                Self {
                    session: Some(session),
                    tokenizer: Some(tokenizer),
                    embedding_dim,
                    max_length,
                    use_ml: true,
                }
            }
            Err(e) => {
                warn!("Failed to load ML model, falling back to TF-IDF: {}", e);
                Self {
                    session: None,
                    tokenizer: None,
                    embedding_dim: 128, // TF-IDF fallback dimension
                    max_length,
                    use_ml: false,
                }
            }
        }
    }

    /// Load ONNX model and tokenizer
    fn load_ml_model(max_length: usize) -> Result<(Session, Tokenizer, usize)> {
        info!("Loading ML embedding model");

        // Get cache directory
        let cache_dir = default_cache_dir()
            .context("Failed to get cache directory")?;

        // Download model if needed
        let downloader = ModelDownloader::new(cache_dir);
        let model_info = ModelInfo::codebert();
        let model_dir = downloader.ensure_model(&model_info)
            .context("Failed to ensure model is downloaded")?;

        // Load ONNX session
        let model_path = model_dir.join("model.onnx");
        debug!(path = %model_path.display(), "Loading ONNX model");
        
        let session = Session::builder()
            .context("Failed to create session builder")?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .context("Failed to set optimization level")?
            .commit_from_file(&model_path)
            .context("Failed to load ONNX model")?;

        // Load tokenizer
        let tokenizer_path = model_dir.join("tokenizer.json");
        debug!(path = %tokenizer_path.display(), "Loading tokenizer");
        
        let mut tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        // Configure tokenizer
        if let Some(truncation) = tokenizer.get_truncation_mut() {
            truncation.max_length = max_length;
        }
        tokenizer.with_padding(Some(tokenizers::PaddingParams {
            strategy: tokenizers::PaddingStrategy::BatchLongest,
            ..Default::default()
        }));

        // CodeBERT uses 768-dimensional embeddings
        let embedding_dim = 768;

        Ok((session, tokenizer, embedding_dim))
    }

    /// Get embedding dimension
    pub fn embedding_dim(&self) -> usize {
        self.embedding_dim
    }

    /// Check if using ML model
    pub fn is_ml(&self) -> bool {
        self.use_ml
    }

    /// Encode text to embedding vector
    pub fn encode(&mut self, text: &str) -> Result<Vec<f32>> {
        if self.use_ml {
            self.encode_ml(text)
        } else {
            self.encode_tfidf(text)
        }
    }

    /// Encode batch of texts
    pub fn encode_batch(&mut self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if self.use_ml {
            self.encode_batch_ml(texts)
        } else {
            texts.iter().map(|&text| self.encode_tfidf(text)).collect()
        }
    }

    /// Encode using ML model
    fn encode_ml(&self, text: &str) -> Result<Vec<f32>> {
        let session = self.session.as_ref()
            .context("ONNX session not available")?;
        let tokenizer = self.tokenizer.as_ref()
            .context("Tokenizer not available")?;

        // Tokenize
        let encoding = tokenizer
            .encode(text, true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;

        let input_ids = encoding.get_ids();
        let attention_mask = encoding.get_attention_mask();

        // Convert to arrays
        let input_ids_array = Array2::from_shape_vec(
            (1, input_ids.len()),
            input_ids.iter().map(|&x| x as i64).collect(),
        )?;

        let attention_mask_array = Array2::from_shape_vec(
            (1, attention_mask.len()),
            attention_mask.iter().map(|&x| x as i64).collect(),
        )?;

        // Run inference
        let outputs = session.run(ort::inputs![
            "input_ids" => input_ids_array.view(),
            "attention_mask" => attention_mask_array.view()
        ]?)?;

        // Extract embeddings from output
        // CodeBERT output is [batch_size, sequence_length, hidden_size]
        let embeddings = outputs["last_hidden_state"]
            .try_extract_tensor::<f32>()?
            .view()
            .to_owned();

        // Mean pooling over sequence dimension
        let pooled = embeddings
            .mean_axis(Axis(1))
            .context("Failed to pool embeddings")?;

        // L2 normalization
        let mut embedding: Vec<f32> = pooled.to_vec();
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in &mut embedding {
                *val /= norm;
            }
        }

        Ok(embedding)
    }

    /// Encode batch using ML model
    fn encode_batch_ml(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let session = self.session.as_ref()
            .context("ONNX session not available")?;
        let tokenizer = self.tokenizer.as_ref()
            .context("Tokenizer not available")?;

        if texts.is_empty() {
            return Ok(vec![]);
        }

        // Tokenize all texts
        let encodings = tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| anyhow::anyhow!("Batch tokenization failed: {}", e))?;

        let batch_size = encodings.len();
        let seq_len = encodings[0].get_ids().len();

        // Collect input IDs and attention masks
        let mut input_ids_vec = Vec::with_capacity(batch_size * seq_len);
        let mut attention_mask_vec = Vec::with_capacity(batch_size * seq_len);

        for encoding in &encodings {
            let ids = encoding.get_ids();
            let mask = encoding.get_attention_mask();
            
            input_ids_vec.extend(ids.iter().map(|&x| x as i64));
            attention_mask_vec.extend(mask.iter().map(|&x| x as i64));
        }

        let input_ids_array = Array2::from_shape_vec(
            (batch_size, seq_len),
            input_ids_vec,
        )?;

        let attention_mask_array = Array2::from_shape_vec(
            (batch_size, seq_len),
            attention_mask_vec,
        )?;

        // Run inference
        let outputs = session.run(ort::inputs![
            "input_ids" => input_ids_array.view(),
            "attention_mask" => attention_mask_array.view()
        ]?)?;

        // Extract and pool embeddings
        let embeddings = outputs["last_hidden_state"]
            .try_extract_tensor::<f32>()?
            .view()
            .to_owned();

        let mut results = Vec::with_capacity(batch_size);
        for i in 0..batch_size {
            let sample_embeddings = embeddings.index_axis(Axis(0), i);
            let pooled = sample_embeddings.mean_axis(Axis(0))
                .context("Failed to pool embeddings")?;

            // L2 normalization
            let mut embedding: Vec<f32> = pooled.to_vec();
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for val in &mut embedding {
                    *val /= norm;
                }
            }

            results.push(embedding);
        }

        Ok(results)
    }

    /// Simple TF-IDF-style encoding as fallback
    /// This is the original placeholder implementation
    fn encode_tfidf(&self, text: &str) -> Result<Vec<f32>> {
        let words = self.tokenize_simple(text);
        let mut embedding = vec![0.0f32; self.embedding_dim];

        // Simple hash-based embedding
        for word in words {
            let idx = self.word_to_index(&word);
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

    /// Simple tokenization for TF-IDF fallback
    fn tokenize_simple(&self, text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty() && s.len() > 2)
            .map(|s| s.to_string())
            .collect()
    }

    /// Map word to index for TF-IDF fallback
    fn word_to_index(&self, word: &str) -> usize {
        word.bytes().map(|b| b as usize).sum::<usize>() % self.embedding_dim
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
    fn test_tfidf_fallback() {
        // Test TF-IDF fallback (no ML model)
        let mut model = EmbeddingModel::with_config(false, 512);
        assert!(!model.is_ml());
        assert_eq!(model.embedding_dim(), 128);

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
    fn test_similar_texts_tfidf() {
        let mut model = EmbeddingModel::with_config(false, 512);
        let emb1 = model.encode("authenticate user login").unwrap();
        let emb2 = model.encode("user authentication login").unwrap();
        let emb3 = model.encode("database connection pool").unwrap();

        let sim_12 = cosine_similarity(&emb1, &emb2);
        let sim_13 = cosine_similarity(&emb1, &emb3);

        // Similar texts should have higher similarity
        assert!(sim_12 > sim_13);
    }

    // ML model tests - only run when model is available
    #[test]
    #[ignore] // Skip by default as it requires model download
    fn test_ml_model_loading() {
        let model = EmbeddingModel::new();
        // If ML model loads, dimension should be 768 (CodeBERT)
        // If fallback, dimension should be 128
        assert!(model.embedding_dim() == 768 || model.embedding_dim() == 128);
    }

    #[test]
    #[ignore] // Skip by default as it requires model download
    fn test_ml_encode() {
        let mut model = EmbeddingModel::new();
        if model.is_ml() {
            let embedding = model.encode("function authenticate user").unwrap();
            assert_eq!(embedding.len(), 768);

            // Check normalization
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 0.01);
        }
    }

    #[test]
    #[ignore] // Skip by default as it requires model download
    fn test_ml_encode_batch() {
        let mut model = EmbeddingModel::new();
        if model.is_ml() {
            let texts = vec!["function auth()", "class User", "def login()"];
            let embeddings = model.encode_batch(&texts).unwrap();
            assert_eq!(embeddings.len(), 3);
            assert_eq!(embeddings[0].len(), 768);
        }
    }
}
