//! Semantic search module
//!
//! Provides natural language code search using embeddings and vector similarity.

pub mod cache;
pub mod chunking;
pub mod config;
pub mod embeddings;
pub mod engine;
#[cfg(feature = "ml-models")]
pub mod model_download;
pub mod vector_index;

pub use cache::QueryCache;
pub use chunking::{ChunkType, CodeChunk, CodeChunker};
pub use config::{HnswConfig, SemanticConfig};
pub use embeddings::EmbeddingModel;
pub use engine::{EngineStats, SemanticSearchEngine, SemanticSearchResult};
#[cfg(feature = "ml-models")]
pub use model_download::{default_cache_dir, ModelDownloader, ModelInfo};
pub use vector_index::{HnswParams, VectorIndex};
