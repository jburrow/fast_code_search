# Semantic Search Implementation Plan

## Overview

This document provides a detailed implementation plan for adding semantic search to `fast_code_search` as a **separate binary** (`fast_code_search_semantic`) following the same architectural patterns as the existing traditional search server (`fast_code_search_server`).

The implementation will mirror all features of traditional search:
- **Configuration** - TOML-based config with CLI overrides
- **gRPC API** - Streaming search results with proto definitions
- **REST API** - HTTP/JSON endpoints for web clients
- **Web UI** - Browser-based interface for queries
- **CLI** - Command-line server management
- **Indexing** - Background and on-demand indexing
- **Monitoring** - Stats, progress tracking, health checks

---

## Project Structure

### New Files to Create

```
fast_code_search/
├── src/
│   ├── bin/
│   │   └── fast_code_search_semantic.rs    # NEW: Semantic search server binary
│   ├── semantic/                             # NEW: Semantic search module
│   │   ├── mod.rs                           # Module exports
│   │   ├── embeddings.rs                    # Embedding model management
│   │   ├── vector_index.rs                  # HNSW vector search index
│   │   ├── chunking.rs                      # Code chunking strategies
│   │   ├── engine.rs                        # SemanticSearchEngine
│   │   └── config.rs                        # Semantic-specific config
│   ├── semantic_server/                      # NEW: gRPC service for semantic
│   │   ├── mod.rs                           # Module exports
│   │   └── service.rs                       # gRPC service implementation
│   └── semantic_web/                         # NEW: REST API for semantic
│       ├── mod.rs                           # Module exports
│       └── api.rs                           # REST handlers
├── proto/
│   └── semantic_search.proto                # NEW: Protobuf definitions
├── static/
│   └── semantic/                            # NEW: Semantic UI (or extend existing)
│       ├── index.html                       # Semantic search interface
│       ├── app.js                           # JavaScript logic
│       └── style.css                        # Styling
├── examples/
│   └── semantic_client.rs                   # NEW: Example gRPC client
└── fast_code_search_semantic.toml.example   # NEW: Config template
```

### Files to Modify

```
Cargo.toml                    # Add dependencies (ort, ndarray, hnsw, tokenizers)
src/lib.rs                    # Export semantic module
build.rs                      # Add semantic_search.proto compilation
```

---

## Phase 1: Foundation (Week 1)

### 1.1 Dependencies and Build Setup

**File: `Cargo.toml`**

Add new dependencies:

```toml
[dependencies]
# Existing dependencies...

# Semantic search dependencies
ort = "2.0"                   # ONNX Runtime for model inference
ndarray = "0.15"              # Multi-dimensional arrays
hnsw = "0.11"                 # Hierarchical Navigable Small World index
tokenizers = "0.15"           # Tokenization for embeddings
reqwest = { version = "0.12", features = ["blocking"] }  # Download models

[build-dependencies]
# Existing...
```

**File: `build.rs`**

Update to compile semantic search proto:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Existing proto compilation
    tonic_prost_build::compile_protos("proto/search.proto")?;
    
    // NEW: Compile semantic search proto
    tonic_prost_build::compile_protos("proto/semantic_search.proto")?;
    
    Ok(())
}
```

**File: `proto/semantic_search.proto`**

Define semantic search protocol:

```protobuf
syntax = "proto3";

package semantic_search;

service SemanticCodeSearch {
  rpc SemanticSearch(SemanticSearchRequest) returns (stream SemanticSearchResult);
  rpc Index(SemanticIndexRequest) returns (SemanticIndexResponse);
  rpc GetStats(StatsRequest) returns (StatsResponse);
}

message SemanticSearchRequest {
  string query = 1;                    // Natural language query
  int32 max_results = 2;               // Maximum results to return
  repeated string include_paths = 3;    // Glob patterns for paths to include
  repeated string exclude_paths = 4;    // Glob patterns for paths to exclude
  repeated string languages = 5;        // Filter by programming language
  float similarity_threshold = 6;       // Minimum similarity score (0.0-1.0)
}

message SemanticSearchResult {
  string file_path = 1;
  string content = 2;                  // Code chunk content
  int32 start_line = 3;                // Starting line of chunk
  int32 end_line = 4;                  // Ending line of chunk
  float similarity_score = 5;           // Cosine similarity (0.0-1.0)
  ChunkType chunk_type = 6;
  string symbol_name = 7;              // Function/class name if chunk_type is SYMBOL
}

enum ChunkType {
  FIXED = 0;          // Fixed-size chunk
  FUNCTION = 1;       // Function definition
  CLASS = 2;          // Class definition
  MODULE = 3;         // Module/file level
}

message SemanticIndexRequest {
  repeated string paths = 1;
}

message SemanticIndexResponse {
  int32 files_indexed = 1;
  int32 chunks_created = 2;
  int64 total_size = 3;
  string message = 4;
}

message StatsRequest {}

message StatsResponse {
  int32 num_files = 1;
  int32 num_chunks = 2;
  int32 embedding_dimensions = 3;
  int64 index_memory_bytes = 4;
  string model_name = 5;
}
```

### 1.2 Configuration Module

**File: `src/semantic/config.rs`**

```rust
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Semantic search configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SemanticConfig {
    #[serde(default)]
    pub server: SemanticServerConfig,

    #[serde(default)]
    pub indexer: SemanticIndexerConfig,

    #[serde(default)]
    pub model: ModelConfig,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticServerConfig {
    /// gRPC server address
    #[serde(default = "default_grpc_address")]
    pub address: String,

    /// Web UI/REST API address
    #[serde(default = "default_web_address")]
    pub web_address: String,

    /// Enable web UI
    #[serde(default = "default_enable_web_ui")]
    pub enable_web_ui: bool,
}

/// Indexer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticIndexerConfig {
    /// Paths to index
    #[serde(default)]
    pub paths: Vec<String>,

    /// Exclude patterns
    #[serde(default = "default_exclude_patterns")]
    pub exclude_patterns: Vec<String>,

    /// Chunk size in tokens
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,

    /// Overlap between chunks in tokens
    #[serde(default = "default_chunk_overlap")]
    pub chunk_overlap: usize,

    /// Index persistence path
    #[serde(default)]
    pub index_path: Option<String>,

    /// Enable file watcher
    #[serde(default)]
    pub watch: bool,
}

/// Model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Model name (codebert, graphcodebert, unixcoder)
    #[serde(default = "default_model_name")]
    pub name: String,

    /// Model cache directory
    #[serde(default = "default_model_path")]
    pub model_path: String,

    /// Device (cpu, cuda:0, cuda:1, etc.)
    #[serde(default = "default_device")]
    pub device: String,

    /// Batch size for inference
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Use INT8 quantization
    #[serde(default)]
    pub use_quantization: bool,

    /// Cache embeddings to disk
    #[serde(default = "default_cache_embeddings")]
    pub cache_embeddings: bool,
}

// Default functions
fn default_grpc_address() -> String {
    "0.0.0.0:50052".to_string()
}

fn default_web_address() -> String {
    "0.0.0.0:8081".to_string()
}

fn default_enable_web_ui() -> bool {
    true
}

fn default_exclude_patterns() -> Vec<String> {
    vec![
        "**/node_modules/**".to_string(),
        "**/target/**".to_string(),
        "**/.git/**".to_string(),
        "**/build/**".to_string(),
        "**/dist/**".to_string(),
        "**/__pycache__/**".to_string(),
        "**/venv/**".to_string(),
        "**/.venv/**".to_string(),
    ]
}

fn default_chunk_size() -> usize {
    512
}

fn default_chunk_overlap() -> usize {
    50
}

fn default_model_name() -> String {
    "codebert".to_string()
}

fn default_model_path() -> String {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("fast_code_search_semantic")
        .join("models")
        .to_string_lossy()
        .to_string()
}

fn default_device() -> String {
    "cpu".to_string()
}

fn default_batch_size() -> usize {
    32
}

fn default_cache_embeddings() -> bool {
    true
}

impl Default for SemanticServerConfig {
    fn default() -> Self {
        Self {
            address: default_grpc_address(),
            web_address: default_web_address(),
            enable_web_ui: default_enable_web_ui(),
        }
    }
}

impl Default for SemanticIndexerConfig {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            exclude_patterns: default_exclude_patterns(),
            chunk_size: default_chunk_size(),
            chunk_overlap: default_chunk_overlap(),
            index_path: None,
            watch: false,
        }
    }
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            name: default_model_name(),
            model_path: default_model_path(),
            device: default_device(),
            batch_size: default_batch_size(),
            use_quantization: false,
            cache_embeddings: default_cache_embeddings(),
        }
    }
}

impl SemanticConfig {
    /// Load from file
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config: {}", path.display()))?;
        
        toml::from_str(&content)
            .with_context(|| format!("Failed to parse config: {}", path.display()))
    }

    /// Try default locations
    pub fn from_default_locations() -> Result<Option<(Self, PathBuf)>> {
        // Check environment variable
        if let Ok(env_path) = std::env::var("FCS_SEMANTIC_CONFIG") {
            let path = PathBuf::from(&env_path);
            if path.exists() {
                return Ok(Some((Self::from_file(&path)?, path)));
            }
        }

        // Check current directory
        let local_path = PathBuf::from("fast_code_search_semantic.toml");
        if local_path.exists() {
            return Ok(Some((Self::from_file(&local_path)?, local_path)));
        }

        // Check user config directory
        if let Some(config_dir) = dirs::config_dir() {
            let user_path = config_dir
                .join("fast_code_search")
                .join("semantic.toml");
            if user_path.exists() {
                return Ok(Some((Self::from_file(&user_path)?, user_path)));
            }
        }

        Ok(None)
    }

    /// Generate template config
    pub fn generate_template() -> String {
        r#"# Fast Code Search - Semantic Search Configuration

[server]
# gRPC server address (default: 0.0.0.0:50052)
address = "0.0.0.0:50052"

# Web UI/REST API address (default: 0.0.0.0:8081)
web_address = "0.0.0.0:8081"

# Enable web UI (default: true)
enable_web_ui = true

[indexer]
# Paths to index (add your project directories)
paths = [
    # "/path/to/your/project",
]

# Patterns to exclude
exclude_patterns = [
    "**/node_modules/**",
    "**/target/**",
    "**/.git/**",
    "**/build/**",
    "**/dist/**",
    "**/__pycache__/**",
    "**/venv/**",
    "**/.venv/**",
]

# Chunk size in tokens (default: 512)
# Larger chunks provide more context but slower search
chunk_size = 512

# Overlap between chunks in tokens (default: 50)
# Helps prevent splitting related code
chunk_overlap = 50

# Path to save/load index (optional)
# index_path = "/var/lib/fast_code_search_semantic/index.bin"

# Enable file watcher for incremental indexing (default: false)
# watch = false

[model]
# Model name: codebert, graphcodebert, unixcoder (default: codebert)
name = "codebert"

# Model cache directory (default: ~/.cache/fast_code_search_semantic/models)
# model_path = "/path/to/models"

# Device: cpu, cuda:0, cuda:1, etc. (default: cpu)
# For production, use GPU for better performance
device = "cpu"

# Batch size for embedding inference (default: 32)
batch_size = 32

# Use INT8 quantization for faster inference (default: false)
# use_quantization = false

# Cache embeddings to disk (default: true)
cache_embeddings = true
"#.to_string()
    }

    /// Write template to file
    pub fn write_template(path: &Path) -> Result<()> {
        let template = Self::generate_template();
        
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        std::fs::write(path, template)
            .with_context(|| format!("Failed to write config: {}", path.display()))?;
        
        Ok(())
    }

    /// Apply CLI overrides
    pub fn with_overrides(
        mut self,
        address: Option<String>,
        extra_paths: Vec<String>,
    ) -> Self {
        if let Some(addr) = address {
            self.server.address = addr;
        }
        self.indexer.paths.extend(extra_paths);
        self
    }
}
```

### 1.3 Embedding Model Module

**File: `src/semantic/embeddings.rs`**

```rust
use anyhow::{Context, Result};
use ort::{Session, Value};
use std::path::Path;
use tokenizers::Tokenizer;
use tracing::{debug, info};

/// Embedding model for code
pub struct EmbeddingModel {
    session: Session,
    tokenizer: Tokenizer,
    embedding_dim: usize,
}

impl EmbeddingModel {
    /// Load model from path
    pub fn load(model_dir: &Path) -> Result<Self> {
        info!(path = %model_dir.display(), "Loading embedding model");
        
        let model_path = model_dir.join("model.onnx");
        let tokenizer_path = model_dir.join("tokenizer.json");
        
        // Check if model exists, download if not
        if !model_path.exists() {
            info!("Model not found, downloading...");
            Self::download_model(model_dir)?;
        }
        
        // Load ONNX model
        let session = Session::builder()?
            .with_model_from_file(&model_path)
            .context("Failed to load ONNX model")?;
        
        // Load tokenizer
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .context("Failed to load tokenizer")?;
        
        // Determine embedding dimension from model metadata
        let embedding_dim = 768; // CodeBERT/GraphCodeBERT standard
        
        info!(
            embedding_dim = embedding_dim,
            "Embedding model loaded successfully"
        );
        
        Ok(Self {
            session,
            tokenizer,
            embedding_dim,
        })
    }

    /// Download model files
    fn download_model(_model_dir: &Path) -> Result<()> {
        // TODO: Implement model download from Hugging Face
        // For now, user must manually download model
        anyhow::bail!("Model download not yet implemented. Please download manually:\n\
            1. Download CodeBERT ONNX model\n\
            2. Place model.onnx and tokenizer.json in model directory");
    }

    /// Encode text to embedding vector
    pub fn encode(&self, text: &str) -> Result<Vec<f32>> {
        debug!(text_len = text.len(), "Encoding text");
        
        // Tokenize
        let encoding = self.tokenizer
            .encode(text, false)
            .context("Failed to tokenize text")?;
        
        let input_ids: Vec<i64> = encoding
            .get_ids()
            .iter()
            .map(|&id| id as i64)
            .collect();
        
        // Prepare ONNX input tensor
        let input_shape = vec![1, input_ids.len()];
        let input_tensor = Value::from_array(
            self.session.allocator(),
            ndarray::Array::from_shape_vec(input_shape, input_ids)?
        )?;
        
        // Run inference
        let outputs = self.session.run(vec![input_tensor])?;
        
        // Extract embedding (pooled output)
        let embedding: Vec<f32> = outputs[0]
            .extract_tensor::<f32>()?
            .iter()
            .copied()
            .collect();
        
        debug!(embedding_len = embedding.len(), "Text encoded");
        
        Ok(embedding)
    }

    /// Encode batch of texts
    pub fn encode_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        texts.iter().map(|&text| self.encode(text)).collect()
    }

    /// Get embedding dimension
    pub fn embedding_dim(&self) -> usize {
        self.embedding_dim
    }
}
```

### 1.4 Code Chunking Module

**File: `src/semantic/chunking.rs`**

```rust
use crate::symbols::{Symbol, SymbolExtractor};
use std::path::Path;

/// Type of code chunk
#[derive(Debug, Clone, PartialEq)]
pub enum ChunkType {
    Fixed,                  // Fixed-size chunk
    Function(String),       // Function with name
    Class(String),          // Class with name
    Module,                 // Module/file level
}

/// A chunk of code with metadata
#[derive(Debug, Clone)]
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
        let extractor = SymbolExtractor::new();
        let symbols = extractor
            .extract_symbols(content, Path::new(file_path))
            .ok()?;
        
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
            
            let chunk_type = match symbol.kind.as_str() {
                "function" | "method" => ChunkType::Function(symbol.name.clone()),
                "class" | "struct" | "impl" => ChunkType::Class(symbol.name.clone()),
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
```

---

## Phase 2: Vector Index (Week 2)

### 2.1 Vector Index Module

**File: `src/semantic/vector_index.rs`**

```rust
use anyhow::Result;
use hnsw::{Hnsw, Params};
use std::path::Path;

/// Vector search index using HNSW
pub struct VectorIndex {
    hnsw: Hnsw<f32, SquaredEuclidean>,
    embeddings: Vec<Vec<f32>>,
    chunk_ids: Vec<u32>,  // Maps HNSW index to chunk ID
    embedding_dim: usize,
}

/// Distance metric for HNSW
#[derive(Clone)]
pub struct SquaredEuclidean;

impl hnsw::DistanceMetric<f32> for SquaredEuclidean {
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| {
                let diff = x - y;
                diff * diff
            })
            .sum()
    }
}

impl VectorIndex {
    /// Create new vector index
    pub fn new(embedding_dim: usize) -> Self {
        let params = Params::new()
            .m(16)               // Number of connections per layer
            .ef_construction(200) // Quality of construction
            .max_elements(100_000); // Initial capacity
        
        let hnsw = Hnsw::new(params, SquaredEuclidean);
        
        Self {
            hnsw,
            embeddings: Vec::new(),
            chunk_ids: Vec::new(),
            embedding_dim,
        }
    }

    /// Add embedding to index
    pub fn add(&mut self, chunk_id: u32, embedding: Vec<f32>) -> Result<()> {
        if embedding.len() != self.embedding_dim {
            anyhow::bail!("Embedding dimension mismatch");
        }
        
        let idx = self.embeddings.len();
        self.embeddings.push(embedding.clone());
        self.chunk_ids.push(chunk_id);
        
        self.hnsw.insert(&embedding, idx);
        
        Ok(())
    }

    /// Search for k nearest neighbors
    pub fn search(&self, query_embedding: &[f32], k: usize) -> Vec<(u32, f32)> {
        if query_embedding.len() != self.embedding_dim {
            return Vec::new();
        }
        
        // Search HNSW index
        let neighbors = self.hnsw.search(query_embedding, k);
        
        // Map HNSW indices to chunk IDs with similarity scores
        neighbors
            .into_iter()
            .map(|(idx, distance)| {
                let chunk_id = self.chunk_ids[idx];
                // Convert squared Euclidean distance to cosine similarity
                let similarity = 1.0 / (1.0 + distance.sqrt());
                (chunk_id, similarity)
            })
            .collect()
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
        let data = bincode::serialize(&(
            &self.embeddings,
            &self.chunk_ids,
            self.embedding_dim,
        ))?;
        std::fs::write(path, data)?;
        Ok(())
    }

    /// Load index from disk
    pub fn load(path: &Path) -> Result<Self> {
        let data = std::fs::read(path)?;
        let (embeddings, chunk_ids, embedding_dim): (Vec<Vec<f32>>, Vec<u32>, usize) =
            bincode::deserialize(&data)?;
        
        let params = Params::new()
            .m(16)
            .ef_construction(200)
            .max_elements(embeddings.len());
        
        let mut hnsw = Hnsw::new(params, SquaredEuclidean);
        
        // Rebuild HNSW index
        for (idx, embedding) in embeddings.iter().enumerate() {
            hnsw.insert(embedding, idx);
        }
        
        Ok(Self {
            hnsw,
            embeddings,
            chunk_ids,
            embedding_dim,
        })
    }
}
```

---

## Phase 3: Semantic Search Engine (Week 3)

### 3.1 Main Engine Module

**File: `src/semantic/engine.rs`**

```rust
use super::{
    chunking::{CodeChunk, CodeChunker},
    embeddings::EmbeddingModel,
    vector_index::VectorIndex,
};
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info};

/// Semantic search engine
pub struct SemanticSearchEngine {
    vector_index: VectorIndex,
    embedding_model: EmbeddingModel,
    chunker: CodeChunker,
    chunks: HashMap<u32, CodeChunk>,  // chunk_id -> chunk
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
    pub fn new(model: EmbeddingModel, chunk_size: usize, chunk_overlap: usize) -> Self {
        let embedding_dim = model.embedding_dim();
        let vector_index = VectorIndex::new(embedding_dim);
        let chunker = CodeChunker::new(chunk_size, chunk_overlap);
        
        Self {
            vector_index,
            embedding_model,
            chunker,
            chunks: HashMap::new(),
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
    pub fn search(&self, query: &str, max_results: usize) -> Result<Vec<SemanticSearchResult>> {
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
        EngineStats {
            num_chunks: self.chunks.len(),
            num_files: self.chunks
                .values()
                .map(|c| &c.file_path)
                .collect::<std::collections::HashSet<_>>()
                .len(),
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
```

### 3.2 Module Exports

**File: `src/semantic/mod.rs`**

```rust
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
```

---

## Phase 4: gRPC Service (Week 3)

### 4.1 gRPC Service Implementation

**File: `src/semantic_server/service.rs`**

```rust
use crate::semantic::{SemanticSearchEngine, SemanticSearchResult};
use anyhow::Result;
use std::sync::{Arc, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{debug, info};

// Include generated proto code
pub mod semantic_search_proto {
    tonic::include_proto!("semantic_search");
}

use semantic_search_proto::{
    semantic_code_search_server::{SemanticCodeSearch, SemanticCodeSearchServer},
    ChunkType, SemanticIndexRequest, SemanticIndexResponse, SemanticSearchRequest,
    SemanticSearchResult as ProtoResult, StatsRequest, StatsResponse,
};

pub struct SemanticCodeSearchService {
    engine: Arc<RwLock<SemanticSearchEngine>>,
}

impl SemanticCodeSearchService {
    pub fn new(engine: Arc<RwLock<SemanticSearchEngine>>) -> Self {
        Self { engine }
    }
}

#[tonic::async_trait]
impl SemanticCodeSearch for SemanticCodeSearchService {
    type SemanticSearchStream = ReceiverStream<Result<ProtoResult, Status>>;

    async fn semantic_search(
        &self,
        request: Request<SemanticSearchRequest>,
    ) -> Result<Response<Self::SemanticSearchStream>, Status> {
        let req = request.into_inner();
        info!(query = %req.query, "Semantic search request");

        let (tx, rx) = tokio::sync::mpsc::channel(100);

        let engine = Arc::clone(&self.engine);
        let query = req.query.clone();
        let max_results = req.max_results as usize;

        tokio::spawn(async move {
            let results = {
                let engine_guard = engine.read().unwrap();
                engine_guard.search(&query, max_results)
            };

            match results {
                Ok(results) => {
                    for result in results {
                        let proto_result = convert_to_proto_result(result);
                        if tx.send(Ok(proto_result)).await.is_err() {
                            break;
                        }
                    }
                }
                Err(e) => {
                    let _ = tx
                        .send(Err(Status::internal(format!("Search error: {}", e))))
                        .await;
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn index(
        &self,
        request: Request<SemanticIndexRequest>,
    ) -> Result<Response<SemanticIndexResponse>, Status> {
        let req = request.into_inner();
        info!(paths_count = req.paths.len(), "Index request");

        // TODO: Implement indexing logic
        let response = SemanticIndexResponse {
            files_indexed: 0,
            chunks_created: 0,
            total_size: 0,
            message: "Indexing not yet implemented".to_string(),
        };

        Ok(Response::new(response))
    }

    async fn get_stats(
        &self,
        _request: Request<StatsRequest>,
    ) -> Result<Response<StatsResponse>, Status> {
        let stats = {
            let engine = self.engine.read().unwrap();
            engine.get_stats()
        };

        let response = StatsResponse {
            num_files: stats.num_files as i32,
            num_chunks: stats.num_chunks as i32,
            embedding_dimensions: stats.embedding_dim as i32,
            index_memory_bytes: 0, // TODO: Calculate
            model_name: "codebert".to_string(),
        };

        Ok(Response::new(response))
    }
}

fn convert_to_proto_result(result: SemanticSearchResult) -> ProtoResult {
    let chunk_type = match result.chunk.chunk_type {
        crate::semantic::ChunkType::Fixed => ChunkType::Fixed as i32,
        crate::semantic::ChunkType::Function(_) => ChunkType::Function as i32,
        crate::semantic::ChunkType::Class(_) => ChunkType::Class as i32,
        crate::semantic::ChunkType::Module => ChunkType::Module as i32,
    };

    let symbol_name = match &result.chunk.chunk_type {
        crate::semantic::ChunkType::Function(name) | crate::semantic::ChunkType::Class(name) => {
            name.clone()
        }
        _ => String::new(),
    };

    ProtoResult {
        file_path: result.chunk.file_path,
        content: result.chunk.text,
        start_line: result.chunk.start_line as i32,
        end_line: result.chunk.end_line as i32,
        similarity_score: result.similarity_score,
        chunk_type,
        symbol_name,
    }
}

pub fn create_server(
    engine: Arc<RwLock<SemanticSearchEngine>>,
) -> SemanticCodeSearchServer<SemanticCodeSearchService> {
    SemanticCodeSearchServer::new(SemanticCodeSearchService::new(engine))
}
```

**File: `src/semantic_server/mod.rs`**

```rust
pub mod service;

pub use service::{create_server, SemanticCodeSearchService};
```

---

## Phase 5: REST API and Web UI (Week 4)

### 5.1 REST API

**File: `src/semantic_web/api.rs`**

```rust
use crate::semantic::SemanticSearchEngine;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::time::Instant;

pub type WebState = Arc<RwLock<SemanticSearchEngine>>;

/// Search query parameters
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    q: String,
    #[serde(default = "default_max_results")]
    max: usize,
}

fn default_max_results() -> usize {
    20
}

/// Search result for JSON
#[derive(Debug, Serialize)]
pub struct SearchResultJson {
    pub file_path: String,
    pub content: String,
    pub start_line: usize,
    pub end_line: usize,
    pub similarity_score: f32,
    pub chunk_type: String,
    pub symbol_name: Option<String>,
}

/// Search response
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResultJson>,
    pub query: String,
    pub total_results: usize,
    pub elapsed_ms: f64,
}

/// Stats response
#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub num_files: usize,
    pub num_chunks: usize,
    pub embedding_dim: usize,
}

/// Health response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}

pub async fn search_handler(
    State(state): State<WebState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, (StatusCode, String)> {
    let start = Instant::now();
    
    let results = {
        let engine = state.read().unwrap();
        engine
            .search(&params.q, params.max)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    let json_results: Vec<SearchResultJson> = results
        .into_iter()
        .map(|r| {
            let (chunk_type, symbol_name) = match &r.chunk.chunk_type {
                crate::semantic::ChunkType::Fixed => ("fixed".to_string(), None),
                crate::semantic::ChunkType::Function(name) => {
                    ("function".to_string(), Some(name.clone()))
                }
                crate::semantic::ChunkType::Class(name) => {
                    ("class".to_string(), Some(name.clone()))
                }
                crate::semantic::ChunkType::Module => ("module".to_string(), None),
            };

            SearchResultJson {
                file_path: r.chunk.file_path,
                content: r.chunk.text,
                start_line: r.chunk.start_line,
                end_line: r.chunk.end_line,
                similarity_score: r.similarity_score,
                chunk_type,
                symbol_name,
            }
        })
        .collect();

    let response = SearchResponse {
        total_results: json_results.len(),
        results: json_results,
        query: params.q,
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
    };

    Ok(Json(response))
}

pub async fn stats_handler(
    State(state): State<WebState>,
) -> Result<Json<StatsResponse>, (StatusCode, String)> {
    let stats = {
        let engine = state.read().unwrap();
        engine.get_stats()
    };

    Ok(Json(StatsResponse {
        num_files: stats.num_files,
        num_chunks: stats.num_chunks,
        embedding_dim: stats.embedding_dim,
    }))
}

pub async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}
```

**File: `src/semantic_web/mod.rs`**

```rust
pub mod api;

use api::{health_handler, search_handler, stats_handler, WebState};
use axum::{
    routing::{get, post},
    Router,
};
use tower_http::cors::CorsLayer;

pub fn create_router(state: WebState) -> Router {
    Router::new()
        .route("/api/search", get(search_handler))
        .route("/api/stats", get(stats_handler))
        .route("/api/health", get(health_handler))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
```

### 5.2 Web UI

**File: `static/semantic/index.html`**

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Semantic Code Search</title>
    <link rel="stylesheet" href="/semantic/style.css">
</head>
<body>
    <div class="container">
        <header>
            <h1>🧠 Semantic Code Search</h1>
            <p class="subtitle">Natural language code discovery with AI embeddings</p>
        </header>

        <div class="search-box">
            <input type="text" id="query" 
                   placeholder='Ask in natural language: "how do we authenticate users?"' 
                   autocomplete="off" autofocus>
            <div class="search-options">
                <label>
                    Max results:
                    <select id="max-results">
                        <option value="10">10</option>
                        <option value="20" selected>20</option>
                        <option value="50">50</option>
                    </select>
                </label>
            </div>
        </div>

        <div class="stats-panel" id="stats">
            <div class="stat">
                <span class="stat-value" id="stat-files">-</span>
                <span class="stat-label">Files Indexed</span>
            </div>
            <div class="stat">
                <span class="stat-value" id="stat-chunks">-</span>
                <span class="stat-label">Code Chunks</span>
            </div>
            <div class="stat">
                <span class="stat-value" id="stat-dim">-</span>
                <span class="stat-label">Embedding Dim</span>
            </div>
        </div>

        <div class="results-header" id="results-header" style="display: none;">
            <span id="results-count">0 results</span>
            <span id="search-time"></span>
        </div>

        <div class="results" id="results">
            <div class="empty-state">
                <p>🔍 Enter a natural language query to discover code</p>
                <p class="examples">Examples:</p>
                <ul class="examples">
                    <li>"how do we handle user authentication?"</li>
                    <li>"database connection setup"</li>
                    <li>"functions that parse JSON"</li>
                </ul>
            </div>
        </div>
    </div>

    <script src="/semantic/app.js"></script>
</body>
</html>
```

---

## Phase 6: Main Binary (Week 4)

### 6.1 Semantic Search Server Binary

**File: `src/bin/fast_code_search_semantic.rs`**

```rust
use anyhow::Result;
use clap::Parser;
use fast_code_search::semantic::{EmbeddingModel, SemanticConfig, SemanticSearchEngine};
use fast_code_search::semantic_server;
use fast_code_search::semantic_web;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tonic::transport::Server;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

/// Fast Code Search Semantic Server
#[derive(Parser, Debug)]
#[command(name = "fast_code_search_semantic")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Server listen address (overrides config)
    #[arg(short, long, value_name = "ADDR")]
    address: Option<String>,

    /// Additional paths to index
    #[arg(short, long = "index", value_name = "PATH")]
    index_paths: Vec<String>,

    /// Skip automatic indexing
    #[arg(long)]
    no_auto_index: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Generate template config and exit
    #[arg(long, value_name = "FILE")]
    init: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.verbose {
        Level::DEBUG
    } else {
        Level::INFO
    };
    
    FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();

    // Handle --init flag
    if let Some(init_path) = args.init {
        let path = if init_path.as_os_str().is_empty() {
            PathBuf::from("fast_code_search_semantic.toml")
        } else {
            init_path
        };

        if path.exists() {
            eprintln!("Error: Config file already exists: {}", path.display());
            std::process::exit(1);
        }

        SemanticConfig::write_template(&path)?;
        println!("✓ Generated config file: {}", path.display());
        println!("\nEdit the file to configure your semantic search, then start:");
        println!("  cargo run --release --bin fast_code_search_semantic -- --config {}", path.display());
        return Ok(());
    }

    // Load configuration
    let config = load_config(&args)?;

    info!(
        server_address = %config.server.address,
        model = %config.model.name,
        "Configuration loaded"
    );

    // Load embedding model
    info!("Loading embedding model (this may take a moment)...");
    let model_path = PathBuf::from(&config.model.model_path)
        .join(&config.model.name);
    
    let embedding_model = EmbeddingModel::load(&model_path)?;
    info!("Embedding model loaded successfully");

    // Create semantic search engine
    let engine = SemanticSearchEngine::new(
        embedding_model,
        config.indexer.chunk_size,
        config.indexer.chunk_overlap,
    );

    let shared_engine = Arc::new(RwLock::new(engine));

    // Start web server if enabled
    if config.server.enable_web_ui {
        let web_addr = config.server.web_address.clone();
        let web_engine = Arc::clone(&shared_engine);
        
        info!(web_address = %web_addr, "Starting Web UI server");

        tokio::spawn(async move {
            let router = semantic_web::create_router(web_engine);
            let listener = tokio::net::TcpListener::bind(&web_addr)
                .await
                .expect("Failed to bind Web UI");
            
            info!(address = %web_addr, "Web UI available at http://{}", web_addr);
            
            axum::serve(listener, router)
                .await
                .expect("Web UI server failed");
        });
    }

    // TODO: Background indexing
    if !args.no_auto_index && !config.indexer.paths.is_empty() {
        info!("Background indexing not yet implemented");
    }

    // Start gRPC server
    let addr = config.server.address.parse()?;
    let service = semantic_server::create_server(Arc::clone(&shared_engine));

    info!(address = %addr, "Semantic Search Server starting");
    info!(grpc_endpoint = %format!("grpc://{}", addr), "gRPC endpoint");
    info!("Ready to accept connections");

    Server::builder()
        .add_service(service)
        .serve(addr)
        .await?;

    Ok(())
}

fn load_config(args: &Args) -> Result<SemanticConfig> {
    let base_config = if let Some(ref config_path) = args.config {
        if !config_path.exists() {
            anyhow::bail!(
                "Config file not found: {}\nUse --init {} to generate.",
                config_path.display(),
                config_path.display()
            );
        }
        info!(path = %config_path.display(), "Loading config");
        SemanticConfig::from_file(config_path)?
    } else {
        match SemanticConfig::from_default_locations()? {
            Some((config, path)) => {
                info!(path = %path.display(), "Loading config from default location");
                config
            }
            None => {
                info!("No config file found, using defaults");
                SemanticConfig::default()
            }
        }
    };

    Ok(base_config.with_overrides(args.address.clone(), args.index_paths.clone()))
}
```

### 6.2 Update lib.rs

**File: `src/lib.rs` (additions)**

```rust
// Existing modules...

#[cfg(feature = "semantic")]
pub mod semantic;
#[cfg(feature = "semantic")]
pub mod semantic_server;
#[cfg(feature = "semantic")]
pub mod semantic_web;
```

---

## Testing Strategy

### Unit Tests

Create tests for each module:

```rust
// tests/semantic_tests.rs

#[cfg(test)]
mod tests {
    use fast_code_search::semantic::*;

    #[test]
    fn test_code_chunking() {
        let chunker = CodeChunker::new(100, 10);
        let content = "fn main() {\n    println!(\"Hello\");\n}";
        let chunks = chunker.chunk_file(content, Path::new("test.rs"));
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_config_parsing() {
        let toml = r#"
[server]
address = "127.0.0.1:50052"

[model]
name = "codebert"
"#;
        let config: SemanticConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.server.address, "127.0.0.1:50052");
        assert_eq!(config.model.name, "codebert");
    }
}
```

### Integration Tests

```rust
// tests/semantic_integration_tests.rs

#[tokio::test]
async fn test_semantic_search_e2e() {
    // 1. Load model
    // 2. Create engine
    // 3. Index sample files
    // 4. Perform search
    // 5. Verify results
}
```

---

## Documentation

### README Updates

Add section to main README:

```markdown
## Semantic Search

Fast Code Search now includes **semantic search** capabilities for natural language code discovery.

### Two Search Modes

- **Traditional Search** (`fast_code_search_server`) - Fast, exact matching with regex/keywords
  - Port: 50051 (gRPC), 8080 (Web)
  - Use for: exact symbols, regex patterns, known terms

- **Semantic Search** (`fast_code_search_semantic`) - AI-powered natural language queries
  - Port: 50052 (gRPC), 8081 (Web)
  - Use for: "how do we authenticate?", "database setup", exploratory queries

### Quick Start (Semantic)

```bash
# Generate config
cargo run --release --bin fast_code_search_semantic -- --init

# Edit fast_code_search_semantic.toml to add your project paths

# Start server
cargo run --release --bin fast_code_search_semantic -- --config fast_code_search_semantic.toml

# Access Web UI
open http://localhost:8081
```

### Requirements

- **CPU**: Works on CPU but slow (10-30ms per query)
- **GPU**: Recommended for production (CUDA/ROCm)
- **Memory**: ~1.5GB for 10GB codebase
- **Model**: Downloaded automatically on first run (~500MB)
```

---

## Deployment Guide

### Docker Support

Create `Dockerfile.semantic`:

```dockerfile
FROM rust:1.70 as builder

WORKDIR /app
COPY . .

RUN cargo build --release --bin fast_code_search_semantic

FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/fast_code_search_semantic /usr/local/bin/

EXPOSE 50052 8081

CMD ["fast_code_search_semantic"]
```

### Docker Compose

```yaml
version: '3.8'

services:
  traditional:
    build:
      context: .
      dockerfile: Dockerfile
    ports:
      - "50051:50051"
      - "8080:8080"
    volumes:
      - ./config.toml:/etc/fast_code_search/config.toml
      - /path/to/code:/code:ro

  semantic:
    build:
      context: .
      dockerfile: Dockerfile.semantic
    ports:
      - "50052:50052"
      - "8081:8081"
    volumes:
      - ./semantic_config.toml:/etc/fast_code_search/semantic.toml
      - /path/to/code:/code:ro
      - model_cache:/root/.cache/fast_code_search_semantic
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: 1
              capabilities: [gpu]

volumes:
  model_cache:
```

---

## Timeline Summary

| Phase | Duration | Deliverables |
|-------|----------|--------------|
| Phase 1: Foundation | Week 1 | Config, embeddings, chunking modules |
| Phase 2: Vector Index | Week 2 | HNSW index, persistence |
| Phase 3: Search Engine | Week 3 | Core search engine, indexing |
| Phase 4: gRPC Service | Week 3 | Proto definitions, service |
| Phase 5: REST & UI | Week 4 | Web API, browser interface |
| Phase 6: Integration | Week 4 | Main binary, docs, testing |

**Total: 4 weeks (one developer)**

---

## Success Metrics

### Functionality
- ✅ Natural language queries work
- ✅ Results are relevant (top-5 precision > 70%)
- ✅ All features match traditional search (config, UI, APIs)

### Performance
- ✅ Query latency < 50ms (GPU) or < 500ms (CPU)
- ✅ Indexing speed > 100 files/min (GPU)
- ✅ Memory usage < 2x indexed codebase size

### Quality
- ✅ All tests pass
- ✅ Documentation complete
- ✅ Example client works
- ✅ Docker deployment functional

---

## Next Steps After Implementation

1. **Benchmark** against traditional search
2. **User feedback** from early adopters
3. **Optimize** based on real-world usage
4. **GPU support** for production deployment
5. **Model updates** as better embeddings release
6. **Advanced features**:
   - Query suggestions
   - Saved searches
   - Code explanations
   - Similar code detection

---

## Appendix: Command Reference

### Building

```bash
# Build both binaries
cargo build --release

# Build only semantic
cargo build --release --bin fast_code_search_semantic

# Run tests
cargo test

# Run benchmarks
cargo bench
```

### Running

```bash
# Traditional search
./target/release/fast_code_search_server --config config.toml

# Semantic search
./target/release/fast_code_search_semantic --config semantic.toml

# Both on different ports
./target/release/fast_code_search_server &
./target/release/fast_code_search_semantic &
```

### Configuration

```bash
# Generate configs
./target/release/fast_code_search_server --init config.toml
./target/release/fast_code_search_semantic --init semantic.toml
```

---

This implementation plan provides a complete roadmap for adding semantic search to fast_code_search while maintaining separation from the traditional search and following all existing architectural patterns.
