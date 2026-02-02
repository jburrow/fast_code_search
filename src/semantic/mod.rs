//! Semantic search module
//!
//! Provides natural language code search using embeddings and vector similarity.

pub mod cache;
pub mod chunking;
pub mod config;
pub mod embeddings;
pub mod engine;
pub mod model_download;
pub mod vector_index;

pub use cache::QueryCache;
pub use chunking::{ChunkType, CodeChunk, CodeChunker};
pub use config::SemanticConfig;
pub use embeddings::EmbeddingModel;
pub use engine::{EngineStats, SemanticSearchEngine, SemanticSearchResult};
pub use model_download::{ModelDownloader, ModelInfo, default_cache_dir};
pub use vector_index::VectorIndex;
