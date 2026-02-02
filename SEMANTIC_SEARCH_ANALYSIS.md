# Semantic Search Integration Analysis

## Executive Summary

This document analyzes how **semantic search** could be integrated into `fast_code_search`, considering performance implications, codebase impact, and implementation approaches. Semantic search would enable developers to find code based on meaning and intent rather than just exact text matches, significantly improving code discovery in large codebases.

**Key Findings:**
- Semantic search would complement (not replace) the existing trigram-based search
- Hybrid approach offers best balance of speed and accuracy
- Implementation complexity: Medium to High
- Performance impact: Manageable with proper architecture
- Memory overhead: 2-10x depending on model choice
- Development effort: 2-4 weeks for MVP

---

## Table of Contents

1. [What is Semantic Search?](#what-is-semantic-search)
2. [Current Architecture Analysis](#current-architecture-analysis)
3. [Semantic Search Technologies](#semantic-search-technologies)
4. [Integration Approaches](#integration-approaches)
5. [Performance Analysis](#performance-analysis)
6. [Codebase Impact](#codebase-impact)
7. [Implementation Plan](#implementation-plan)
8. [Trade-offs and Recommendations](#trade-offs-and-recommendations)

---

## What is Semantic Search?

### Traditional (Current) Search
```
Query: "read file"
Matches: Exact text "read file" or "read_file"
Misses: "load document", "parse input", "fetch data"
```

### Semantic Search
```
Query: "read file"
Matches: All of the above plus:
  - "load document from disk"
  - "parse configuration"
  - "fetch data from storage"
  - Functions that read files even if named differently
```

### Benefits for Code Search

1. **Intent-Based Discovery**
   - Find functions by what they do, not just what they're named
   - "authentication logic" → finds login, verify_token, check_credentials

2. **Cross-Language Understanding**
   - Query in natural language, find code in any language
   - "sort array" → finds Array.sort(), std::sort, sorted()

3. **Conceptual Similarity**
   - Find related code without exact keywords
   - "database connection" → finds ORM setup, query builders, connection pools

4. **Reduced Keyword Dependency**
   - Less reliance on naming conventions
   - Works across different coding styles and domains

---

## Current Architecture Analysis

### Existing Search Mechanism

`fast_code_search` uses a **trigram-based inverted index**:

```
Text: "function process_data()"
Trigrams: ["fun", "unc", "nct", "cti", "tio", "ion", ...]
Index: trigram → [document IDs]
Search: Extract query trigrams → Intersect bitmaps → Rank results
```

**Strengths:**
- ✅ Very fast: O(log n) index lookup + O(k) bitmap intersection
- ✅ Low memory: Roaring bitmaps are space-efficient
- ✅ Exact matching: Perfect for known symbols/patterns
- ✅ No false negatives: If text exists, it will be found

**Limitations:**
- ❌ Exact text match only: Misses semantic similarity
- ❌ No understanding: "authenticate" ≠ "verify_credentials"
- ❌ Keyword-dependent: Must know exact terms
- ❌ No conceptual search: Can't find "what handles user login?"

### Search Flow Architecture

```
Current:
┌─────────┐     ┌──────────┐     ┌─────────┐     ┌─────────┐
│ Query   │────▶│ Trigram  │────▶│ Bitmap  │────▶│ Ranking │
│         │     │ Extract  │     │ Intersect│     │ Scoring │
└─────────┘     └──────────┘     └─────────┘     └─────────┘
   Fast ✓         Fast ✓           Fast ✓          Fast ✓
```

---

## Semantic Search Technologies

### Option 1: Embedding Models (Recommended)

**Concept:** Convert code to dense vectors; similar code has similar vectors

**Models:**
1. **CodeBERT** (Microsoft)
   - Specialized for code understanding
   - Pre-trained on 6M GitHub repos
   - 768-dimensional embeddings
   - Size: ~500MB model

2. **GraphCodeBERT** (Microsoft)  
   - Enhanced with data flow graphs
   - Better semantic understanding
   - 768-dimensional embeddings
   - Size: ~500MB model

3. **UniXcoder** (Microsoft)
   - Unified cross-lingual model
   - Supports 9 languages
   - 768-dimensional embeddings
   - Size: ~500MB model

4. **StarCoder** (Hugging Face/BigCode)
   - State-of-the-art code model
   - 15B parameters (too large for embedding)
   - Can use smaller variants

5. **OpenAI Embeddings** (API-based)
   - text-embedding-3-small (1536 dims)
   - High quality, but requires API calls
   - Cost: ~$0.02 per 1M tokens

**How It Works:**
```rust
// Indexing phase
for each code_snippet {
    embedding = model.encode(code_snippet)  // [768 floats]
    vector_index.add(doc_id, embedding)
}

// Search phase
query_embedding = model.encode(query)
similar_docs = vector_index.search(query_embedding, k=100)
```

**Pros:**
- ✅ True semantic understanding
- ✅ Language-agnostic queries
- ✅ Handles synonyms and paraphrasing
- ✅ Mature tooling (ONNX, Candle, etc.)

**Cons:**
- ❌ Slower inference (10-100ms per encoding)
- ❌ Memory overhead (768 × 4 bytes × num_chunks)
- ❌ Requires GPU/CPU inference
- ❌ Indexing more complex

### Option 2: BM25 with Semantic Expansion

**Concept:** Expand query with synonyms before traditional search

```
Query: "authenticate user"
Expanded: "authenticate OR login OR verify OR authorize user"
Traditional search: Use trigram index
```

**Pros:**
- ✅ Leverages existing trigram infrastructure
- ✅ Minimal performance impact
- ✅ Easy to implement

**Cons:**
- ❌ Limited semantic understanding
- ❌ Requires manual synonym dictionaries
- ❌ Misses subtle semantic relationships

### Option 3: Hybrid (Recommended Approach)

**Concept:** Combine trigram search with semantic ranking

```
Phase 1: Trigram search (fast, recall-focused)
  └─▶ Get top 1000 candidate documents

Phase 2: Semantic re-ranking (slow, precision-focused)
  └─▶ Re-rank top 100 with embedding similarity
  
Return: Top 20 results
```

**Pros:**
- ✅ Best of both worlds
- ✅ Fast common-case (trigram cache hit)
- ✅ Accurate for semantic queries
- ✅ Gradual rollout possible

**Cons:**
- ❌ More complex architecture
- ❌ Two search systems to maintain

---

## Integration Approaches

### Approach 1: Separate Semantic Index (Low Risk)

Add a parallel vector search engine alongside trigram index:

```
Architecture:
┌────────────────────────────────────────┐
│         SearchEngine                   │
├────────────────────────────────────────┤
│  TrigramIndex    │  VectorIndex       │
│  (existing)      │  (new)              │
│                  │                      │
│  Fast exact      │  Semantic search    │
│  matching        │  with embeddings    │
└────────────────────────────────────────┘
```

**Implementation:**
```rust
// New module: src/semantic/mod.rs
pub struct VectorIndex {
    embeddings: Vec<Vec<f32>>,  // One per code chunk
    doc_ids: Vec<u32>,           // Map embedding idx → doc_id
    model: EmbeddingModel,       // CodeBERT/UniXcoder
}

impl VectorIndex {
    pub fn search(&self, query: &str, k: usize) -> Vec<(u32, f32)> {
        let query_emb = self.model.encode(query);
        self.find_nearest_neighbors(query_emb, k)
    }
}

// In SearchEngine
pub fn search_semantic(&self, query: &str, max_results: usize) -> Vec<SearchResult> {
    self.vector_index.search(query, max_results)
}
```

**Pros:**
- ✅ No changes to existing search
- ✅ Can be feature-flagged
- ✅ Easy to A/B test
- ✅ Gradual rollout

**Cons:**
- ❌ Doubles index memory
- ❌ Two separate search paths
- ❌ No synergy between indexes

### Approach 2: Hybrid Pipeline (Recommended)

Trigram search filters candidates, then semantic re-ranking:

```rust
pub fn search_hybrid(&self, query: &str, max_results: usize) -> Vec<SearchResult> {
    // Phase 1: Fast trigram filtering (existing)
    let candidates = self.trigram_search(query, max_results * 10)?;
    
    // Phase 2: Semantic re-ranking (new)
    if should_use_semantic_rerank(query) {
        return self.semantic_rerank(candidates, query, max_results);
    }
    
    candidates.truncate(max_results);
    candidates
}

fn should_use_semantic_rerank(query: &str) -> bool {
    // Use semantic for natural language queries
    query.split_whitespace().count() > 2 || 
    query.contains("how") || 
    query.contains("what") ||
    // etc.
}

fn semantic_rerank(&self, candidates: Vec<SearchResult>, query: &str, k: usize) 
    -> Vec<SearchResult> 
{
    let query_emb = self.embedding_model.encode(query);
    
    let mut scored: Vec<_> = candidates.into_iter()
        .map(|result| {
            let code_emb = self.get_embedding(result.doc_id);
            let similarity = cosine_similarity(query_emb, code_emb);
            
            // Blend trigram score + semantic score
            let final_score = 0.3 * result.score + 0.7 * similarity;
            (result, final_score)
        })
        .collect();
    
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    scored.truncate(k);
    scored.into_iter().map(|(r, _)| r).collect()
}
```

**Pros:**
- ✅ Leverages existing trigram speed
- ✅ Semantic only when needed
- ✅ Better resource usage
- ✅ Single search API

**Cons:**
- ❌ More complex than separate indexes
- ❌ Requires heuristics for when to re-rank

### Approach 3: Embedding-Only Search (Not Recommended)

Replace trigram index entirely with vector search.

**Pros:**
- ✅ Simplest architecture
- ✅ Pure semantic search

**Cons:**
- ❌ Much slower for exact matches
- ❌ Loses existing performance characteristics
- ❌ No backward compatibility
- ❌ Breaking change for users

**Verdict:** ❌ Not recommended for `fast_code_search`

---

## Performance Analysis

### Memory Overhead

**Current Index:**
```
Trigram Index:   ~10-20% of source code size
Symbol Cache:    ~1-5% of source code size
File Mappings:   0 (memory-mapped)
Total:           ~15-25% overhead
```

**With Semantic Search (Hybrid):**
```
Assumptions:
- 10GB codebase
- Average chunk size: 500 tokens (functions/classes)
- Chunks per file: ~10
- Total chunks: ~200,000 chunks
- Embedding dimensions: 768 (CodeBERT)
- Float32: 4 bytes per dimension

Embedding storage:
  200,000 chunks × 768 dims × 4 bytes = 614 MB

Vector index (HNSW):
  ~1.5x embeddings = 920 MB

Total semantic overhead: ~1.5 GB
Combined total: 1.5-2.5 GB (for 10GB codebase)
```

**Memory Scaling:**
- 1GB codebase → ~200MB semantic index
- 10GB codebase → ~1.5GB semantic index  
- 100GB codebase → ~15GB semantic index

### Indexing Time

**Current Indexing:**
```
10GB codebase: 2-5 minutes
Throughput: 30-50 MB/s
```

**With Semantic Indexing:**
```
Embedding inference:
  CodeBERT: ~100 tokens/sec/CPU core (no GPU)
  200,000 chunks × 500 tokens = 100M tokens
  Single-threaded: ~11 days
  16 cores: ~16 hours
  With GPU (T4): ~30-60 minutes

Realistic with optimization:
  - Batch inference (32 chunks)
  - Multi-GPU (2x T4)
  - ONNX Runtime optimizations
  Result: 10-30 minutes for 10GB codebase
```

**Indexing Performance:**
| Stage | Current | With Semantic | Slowdown |
|-------|---------|---------------|----------|
| Trigram extraction | 2 min | 2 min | 1x |
| Symbol parsing | 1 min | 1 min | 1x |
| Embedding inference | 0 | 15 min | ∞ |
| **Total** | **3 min** | **18 min** | **6x** |

**Mitigation:**
- Incremental indexing (only changed files)
- Background re-indexing
- Persistent embedding cache
- GPU acceleration

### Query Time

**Current Search:**
```
Simple query:  1-5ms
Complex query: 5-20ms
Regex query:   20-100ms
```

**With Semantic Search:**
```
Trigram filtering: 1-5ms (unchanged)
Embedding query:   10-50ms (CPU inference)
Similarity search: 1-5ms (HNSW index)
Re-ranking:        2-10ms (100 candidates)

Total hybrid: 14-70ms
Pure semantic: 11-55ms
```

**Performance Profile:**
| Query Type | Current | Hybrid | Slowdown |
|------------|---------|--------|----------|
| Exact match | 2ms | 2ms | 1x |
| Keyword search | 5ms | 5ms | 1x |
| Natural language | N/A | 30ms | N/A |
| Semantic search | N/A | 25ms | N/A |

**Mitigation:**
- Cache query embeddings (common queries)
- Lazy semantic re-ranking (only if requested)
- GPU inference (5-10x faster)
- ONNX quantization (2-3x faster)

### Throughput Impact

**Current:**
```
Concurrent queries: 1000/sec (8 cores, memory-bound)
```

**With Semantic (CPU-only):**
```
Embedding bottleneck: ~10 queries/sec/core
16 cores: ~160 queries/sec maximum
```

**With Semantic (GPU):**
```
Batch inference: ~500 queries/sec (T4 GPU)
Multi-GPU: ~2000 queries/sec (4x T4)
```

**Recommendation:** GPU acceleration essential for production use

---

## Codebase Impact

### New Dependencies

**Required Crates:**
```toml
[dependencies]
# Option A: Pure Rust (slower, no external deps)
candle-core = "0.4"           # Hugging Face ML framework in Rust
candle-nn = "0.4"
candle-transformers = "0.4"   # Pre-built transformers
tokenizers = "0.15"           # Tokenization

# Option B: ONNX Runtime (faster, production-ready)
ort = "2.0"                   # ONNX Runtime bindings
ndarray = "0.15"              # Multi-dim arrays

# Vector search
hnsw = "0.11"                 # Hierarchical NSW for ANN search
# OR
usearch = "2.0"               # Faster alternative

# Optional: GPU acceleration
cuda = "0.3"                  # If using CUDA
```

**Model Files:**
- CodeBERT: ~500MB download
- Vocabulary: ~1MB
- Config: ~1KB

**Total new code:** ~2000-3000 lines

### Module Structure

```
src/
├── semantic/              (NEW)
│   ├── mod.rs            - Public API
│   ├── embeddings.rs     - Model inference
│   ├── vector_index.rs   - HNSW index
│   ├── chunking.rs       - Code chunking logic
│   └── models.rs         - Model loading/caching
│
├── search/
│   ├── engine.rs         - Add hybrid search (MODIFIED)
│   └── ranking.rs        - Add semantic scoring (MODIFIED)
│
├── index/
│   ├── persistence.rs    - Add embedding storage (MODIFIED)
│   └── mod.rs            - Export semantic index (MODIFIED)
│
└── server/
    └── service.rs        - Add semantic search RPC (MODIFIED)
```

### API Changes

**New gRPC Method:**
```proto
service CodeSearch {
  // Existing
  rpc Search(SearchRequest) returns (stream SearchResult);
  
  // New
  rpc SemanticSearch(SemanticSearchRequest) returns (stream SearchResult);
  rpc HybridSearch(HybridSearchRequest) returns (stream SearchResult);
}

message SemanticSearchRequest {
  string query = 1;                // Natural language query
  int32 max_results = 2;
  bool use_hybrid = 3;             // Combine with trigram search
  repeated string languages = 4;    // Filter by language
}
```

**REST API:**
```rust
// New endpoint
POST /api/search/semantic
{
  "query": "function that authenticates users",
  "max_results": 20,
  "use_hybrid": true
}
```

### Configuration

**New Settings:**
```toml
[semantic]
enabled = true
model = "codebert"                    # or "graphcodebert", "unixcoder"
model_path = "~/.cache/fcs/models"    # Where to download models
device = "cuda:0"                      # "cpu", "cuda:0", "cuda:1", etc.
chunk_size = 512                       # Tokens per embedding
chunk_overlap = 50                     # Overlap between chunks
batch_size = 32                        # Inference batch size
use_quantization = true                # INT8 quantization for speed
cache_embeddings = true                # Persist embeddings to disk

[semantic.hybrid]
enabled = true
trigram_weight = 0.3                   # Weight for trigram score
semantic_weight = 0.7                  # Weight for semantic score
rerank_threshold = 3                   # Min query words to trigger rerank
max_rerank_candidates = 100            # How many to rerank
```

### Testing Requirements

**New Test Categories:**
1. **Unit Tests**
   - Embedding model loading
   - Code chunking logic
   - Vector similarity computation
   - Index serialization

2. **Integration Tests**
   - End-to-end semantic search
   - Hybrid search pipeline
   - Model caching
   - Multi-language queries

3. **Performance Tests**
   - Embedding inference throughput
   - Vector search latency
   - Memory usage scaling
   - Index size vs. corpus size

4. **Benchmark Suite**
   - Compare semantic vs. trigram
   - Measure quality (precision@k, recall@k)
   - Stress test (1M+ embeddings)

**Estimated Test Code:** ~1000-1500 lines

---

## Implementation Plan

### Phase 1: Foundation (Week 1)

**Goal:** Basic embedding infrastructure

**Tasks:**
1. Add ONNX Runtime dependency
2. Download and load CodeBERT model
3. Implement embedding inference
4. Create code chunking logic
5. Write unit tests

**Deliverables:**
```rust
pub struct EmbeddingModel {
    session: ort::Session,
    tokenizer: Tokenizer,
}

impl EmbeddingModel {
    pub fn encode(&self, text: &str) -> Vec<f32> { ... }
    pub fn encode_batch(&self, texts: &[&str]) -> Vec<Vec<f32>> { ... }
}
```

### Phase 2: Vector Index (Week 2)

**Goal:** Build and query vector index

**Tasks:**
1. Integrate HNSW library
2. Implement vector index operations (add, search)
3. Add persistence (save/load embeddings)
4. Benchmark vector search performance
5. Write integration tests

**Deliverables:**
```rust
pub struct VectorIndex {
    hnsw: Hnsw<f32, SquaredEuclidean>,
    embeddings: Vec<Vec<f32>>,
    doc_ids: Vec<u32>,
}

impl VectorIndex {
    pub fn add(&mut self, doc_id: u32, embedding: Vec<f32>) { ... }
    pub fn search(&self, query_emb: &[f32], k: usize) -> Vec<(u32, f32)> { ... }
    pub fn save(&self, path: &Path) -> Result<()> { ... }
    pub fn load(path: &Path) -> Result<Self> { ... }
}
```

### Phase 3: Integration (Week 3)

**Goal:** Integrate with existing search engine

**Tasks:**
1. Add semantic search to SearchEngine
2. Implement hybrid search pipeline
3. Add query embedding cache
4. Update gRPC/REST APIs
5. Add configuration options
6. Write end-to-end tests

**Deliverables:**
```rust
impl SearchEngine {
    pub fn search_semantic(&self, query: &str, opts: SearchOptions) 
        -> Result<Vec<SearchResult>> 
    {
        let query_emb = self.embedding_cache
            .get_or_insert(query, || self.embedding_model.encode(query));
        
        let candidates = if opts.use_hybrid {
            self.trigram_search(query, opts.max_results * 10)?
        } else {
            self.all_documents()
        };
        
        self.vector_index.search_and_rank(query_emb, candidates, opts.max_results)
    }
}
```

### Phase 4: Optimization & Polish (Week 4)

**Goal:** Production-ready performance

**Tasks:**
1. GPU acceleration (if available)
2. Query result caching
3. Incremental embedding updates
4. Model quantization (INT8)
5. Benchmark against real codebases
6. Documentation
7. Examples

**Deliverables:**
- Performance benchmarks (vs. baseline)
- User documentation
- Example client code
- Configuration guide

---

## Trade-offs and Recommendations

### Should We Implement Semantic Search?

**Arguments FOR:**

1. ✅ **Improved Developer Experience**
   - Natural language queries → less friction
   - Find code by intent, not just keywords
   - Better onboarding for new team members

2. ✅ **Competitive Advantage**
   - GitHub Code Search doesn't have semantic search (yet)
   - Sourcegraph has basic semantic, could improve
   - Differentiation from ripgrep/Zoekt

3. ✅ **Future-Proofing**
   - AI-assisted coding is the future
   - Semantic search enables:
     - Code explanation ("what does this do?")
     - Example finding ("show me similar code")
     - Refactoring suggestions

4. ✅ **Research Opportunity**
   - Cutting-edge application of ML to code search
   - Potential for papers/talks
   - Community interest

**Arguments AGAINST:**

1. ❌ **Complexity**
   - 2-3x codebase size increase
   - New dependencies (ONNX, models)
   - GPU infrastructure for production
   - More moving parts = more bugs

2. ❌ **Performance Overhead**
   - 6x slower indexing (without GPU)
   - 10-30x slower queries (CPU-only semantic)
   - Higher memory usage (2-10x)

3. ❌ **Maintenance Burden**
   - Keep up with new embedding models
   - Handle model updates/versioning
   - More complex debugging
   - User support for semantic queries

4. ❌ **Unclear ROI**
   - No proven user demand
   - Existing trigram search works well
   - May not justify the complexity

### Recommendations

#### Option 1: Full Implementation (High Ambition)

**Recommendation:** Implement hybrid semantic search with the following:

**Architecture:**
- Hybrid approach (trigram + semantic reranking)
- CodeBERT or UniXcoder for embeddings
- ONNX Runtime for inference (CPU/GPU)
- HNSW for vector search
- Feature flag for gradual rollout

**Minimum Requirements:**
- GPU support (CUDA/ROCm) for production
- Persistent embedding cache
- Incremental indexing
- Query result caching

**Timeline:** 4-6 weeks
**Risk:** Medium-High
**Effort:** High
**Value:** High (if successful)

#### Option 2: Experimental Branch (Conservative)

**Recommendation:** Build semantic search as an experimental feature:

**Approach:**
- Separate binary (`fast_code_search_semantic`)
- No changes to main codebase
- Evaluate with real users
- Decide on integration later

**Timeline:** 2-3 weeks
**Risk:** Low
**Effort:** Medium
**Value:** Medium (validation)

#### Option 3: External Service (Pragmatic)

**Recommendation:** Implement as a separate microservice:

**Architecture:**
```
┌─────────────────┐      ┌──────────────────┐
│ fast_code_search│      │ semantic_service │
│ (existing)      │◄────►│ (new)            │
│                 │      │                   │
│ Trigram search  │      │ Embedding search  │
└─────────────────┘      └──────────────────┘
         ▲                        ▲
         └────────────┬───────────┘
                      │
                 ┌────▼─────┐
                 │  Client  │
                 └──────────┘
```

**Benefits:**
- ✅ Independent deployment
- ✅ Separate scaling (GPU for semantic, CPU for trigram)
- ✅ No risk to existing service
- ✅ Language-agnostic (could be Python)

**Timeline:** 2-3 weeks
**Risk:** Low
**Effort:** Medium
**Value:** Medium-High

#### Option 4: Defer (Most Conservative)

**Recommendation:** Wait for:
- User demand to materialize
- Embedding models to improve (smaller, faster)
- Infrastructure to support GPU easily
- Proven value from competitors

**Timeline:** N/A
**Risk:** None
**Effort:** None (document for future)
**Value:** Avoids premature optimization

### Final Recommendation

**Recommended Path: Option 2 + Option 3 Hybrid**

1. **Phase 1 (Now):** Build experimental implementation
   - Validate approach with real codebases
   - Measure actual performance
   - Gather user feedback
   - Timeline: 2-3 weeks

2. **Phase 2 (After Validation):** If successful, choose:
   - **A)** Integrate into main codebase (Option 1)
   - **B)** Keep as separate service (Option 3)
   - **C)** Defer/abandon if value is unclear

**Rationale:**
- ✅ Low risk (experimental only)
- ✅ Real validation before commitment
- ✅ Demonstrates innovation
- ✅ Can pivot based on results

---

## Appendix: Code Examples

### A. Embedding Model Wrapper

```rust
use ort::{Session, Value};
use tokenizers::Tokenizer;
use anyhow::Result;

pub struct EmbeddingModel {
    session: Session,
    tokenizer: Tokenizer,
}

impl EmbeddingModel {
    pub fn load(model_path: &Path) -> Result<Self> {
        let session = Session::builder()?
            .with_model_from_file(model_path)?;
        
        let tokenizer_path = model_path.parent().unwrap().join("tokenizer.json");
        let tokenizer = Tokenizer::from_file(tokenizer_path)?;
        
        Ok(Self { session, tokenizer })
    }
    
    pub fn encode(&self, text: &str) -> Result<Vec<f32>> {
        // Tokenize
        let encoding = self.tokenizer.encode(text, false)?;
        let input_ids = encoding.get_ids();
        
        // Prepare ONNX input
        let input_tensor = Value::from_array(
            self.session.allocator(),
            &[input_ids]
        )?;
        
        // Run inference
        let outputs = self.session.run(&[input_tensor])?;
        let embedding = outputs[0].extract_tensor::<f32>()?;
        
        Ok(embedding.to_vec())
    }
    
    pub fn encode_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        texts.iter()
            .map(|text| self.encode(text))
            .collect()
    }
}
```

### B. Code Chunking Strategy

```rust
pub struct CodeChunker {
    chunk_size: usize,      // Max tokens per chunk
    chunk_overlap: usize,   // Overlap between chunks
}

impl CodeChunker {
    pub fn chunk_file(&self, content: &str, file_path: &Path) -> Vec<Chunk> {
        // Strategy 1: Function/class level (preferred)
        if let Some(chunks) = self.chunk_by_symbols(content, file_path) {
            return chunks;
        }
        
        // Strategy 2: Fixed-size with overlap (fallback)
        self.chunk_by_size(content)
    }
    
    fn chunk_by_symbols(&self, content: &str, file_path: &Path) -> Option<Vec<Chunk>> {
        let extractor = SymbolExtractor::new();
        let symbols = extractor.extract_symbols(content, file_path).ok()?;
        
        let mut chunks = Vec::new();
        for symbol in symbols {
            // Extract function/class with context
            let chunk_text = self.extract_symbol_with_context(content, &symbol);
            
            chunks.push(Chunk {
                text: chunk_text,
                start_line: symbol.line,
                end_line: symbol.line + symbol.length,
                chunk_type: ChunkType::Symbol(symbol.name),
            });
        }
        
        Some(chunks)
    }
    
    fn chunk_by_size(&self, content: &str) -> Vec<Chunk> {
        let lines: Vec<&str> = content.lines().collect();
        let mut chunks = Vec::new();
        let mut i = 0;
        
        while i < lines.len() {
            let end = (i + self.chunk_size).min(lines.len());
            let chunk_lines = &lines[i..end];
            
            chunks.push(Chunk {
                text: chunk_lines.join("\n"),
                start_line: i,
                end_line: end,
                chunk_type: ChunkType::Fixed,
            });
            
            i += self.chunk_size - self.chunk_overlap;
        }
        
        chunks
    }
}

pub struct Chunk {
    pub text: String,
    pub start_line: usize,
    pub end_line: usize,
    pub chunk_type: ChunkType,
}

pub enum ChunkType {
    Symbol(String),  // Function/class name
    Fixed,           // Fixed-size chunk
}
```

### C. Hybrid Search Implementation

```rust
impl SearchEngine {
    pub fn search_hybrid(
        &self,
        query: &str,
        max_results: usize,
    ) -> Result<Vec<SearchResult>> {
        // Step 1: Trigram search (fast, broad)
        let trigram_candidates = self.trigram_index
            .search(query, max_results * 10)?;
        
        // Step 2: Decide if semantic reranking is beneficial
        if !self.should_use_semantic(query) {
            trigram_candidates.truncate(max_results);
            return Ok(trigram_candidates);
        }
        
        // Step 3: Get or compute query embedding
        let query_emb = self.embedding_cache
            .get_or_compute(query, || {
                self.embedding_model.encode(query)
            })?;
        
        // Step 4: Semantic reranking
        let reranked = self.semantic_rerank(
            trigram_candidates,
            &query_emb,
            max_results,
        )?;
        
        Ok(reranked)
    }
    
    fn should_use_semantic(&self, query: &str) -> bool {
        // Heuristics for when semantic search helps
        let word_count = query.split_whitespace().count();
        
        // Use semantic for natural language queries
        if word_count > 3 { return true; }
        
        // Use semantic for question-like queries
        if query.starts_with("how") || 
           query.starts_with("what") ||
           query.starts_with("find") {
            return true;
        }
        
        // Use semantic for descriptive queries
        if query.contains("function that") ||
           query.contains("code for") {
            return true;
        }
        
        // Don't use for exact symbol searches
        if query.contains("::") || query.contains(".") {
            return false;
        }
        
        false
    }
    
    fn semantic_rerank(
        &self,
        candidates: Vec<SearchResult>,
        query_emb: &[f32],
        k: usize,
    ) -> Result<Vec<SearchResult>> {
        let mut scored: Vec<_> = candidates
            .into_iter()
            .filter_map(|result| {
                // Get embedding for this code chunk
                let code_emb = self.vector_index.get_embedding(result.doc_id)?;
                
                // Compute similarity
                let semantic_score = cosine_similarity(query_emb, code_emb);
                
                // Blend scores (configurable weights)
                let final_score = 
                    0.3 * result.score +      // Trigram score
                    0.7 * semantic_score;     // Semantic score
                
                Some((result, final_score))
            })
            .collect();
        
        // Sort by final score
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scored.truncate(k);
        
        Ok(scored.into_iter()
            .map(|(mut result, score)| {
                result.score = score;
                result
            })
            .collect())
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());
    
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    
    dot / (norm_a * norm_b)
}
```

---

## Conclusion

Semantic search represents a significant evolution for `fast_code_search`, enabling natural language code discovery beyond keyword matching. While the implementation involves notable complexity and performance trade-offs, a hybrid approach that combines the existing trigram index with selective semantic re-ranking offers the best balance.

**Key Takeaways:**

1. **Hybrid approach is optimal** - Leverages existing speed while adding semantic understanding
2. **GPU acceleration is critical** - CPU-only semantic search is too slow for production
3. **Start with experiment** - Validate approach before full integration
4. **Consider microservice** - Separate deployment may simplify architecture
5. **4-6 week effort** - Realistic timeline for production-ready implementation

**Next Steps:**

1. Gather stakeholder feedback on this analysis
2. Decide on implementation approach (Options 1-4)
3. If approved, start with Phase 1 (foundation)
4. Measure performance on real codebases
5. Iterate based on results

This analysis provides a comprehensive foundation for deciding whether and how to integrate semantic search into `fast_code_search`.
