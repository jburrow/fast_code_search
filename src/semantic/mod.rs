//! Semantic search module
//!
//! Provides natural language code search using embeddings and vector similarity.

pub mod chunking;
pub mod config;
pub mod embeddings;
pub mod engine;
pub mod vector_index;

pub use chunking::{ChunkType, CodeChunk, CodeChunker};
pub use config::SemanticConfig;
pub use embeddings::EmbeddingModel;
pub use engine::{EngineStats, SemanticSearchEngine, SemanticSearchResult};
pub use vector_index::VectorIndex;
