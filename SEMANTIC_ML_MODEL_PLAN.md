# Semantic Search ML Model Implementation

## Overview

This document details the implementation of real ML-based embeddings for semantic code search, replacing the current TF-IDF placeholder with ONNX Runtime and a pretrained code model.

## Model Selection

### Chosen Model: Microsoft CodeBERT-base

**Why CodeBERT:**
- Well-established and widely used in code search
- Pre-trained on 6 programming languages (Python, Java, JavaScript, PHP, Ruby, Go)
- 768-dimensional embeddings (standard BERT size)
- Available in ONNX format or can be converted
- Good balance of quality and performance
- ~500MB download size

**Alternatives Considered:**
- GraphCodeBERT: Better for structure but larger
- UniXcoder: Newer but less battle-tested
- Custom models: Would require training infrastructure

## Architecture

### Components

1. **Model Download & Caching**
   - Download ONNX model from HuggingFace on first run
   - Cache in `~/.cache/fast_code_search_semantic/models/`
   - Support multiple model versions
   - Verify checksums

2. **ONNX Runtime Integration**
   - Use `ort` crate (ONNX Runtime bindings)
   - Support CPU and GPU execution providers
   - Batch inference for efficiency
   - Handle session management

3. **Tokenization**
   - Use `tokenizers` crate (HuggingFace tokenizers)
   - Download tokenizer config with model
   - Handle max sequence length (512 tokens)
   - Proper padding and truncation

4. **Embedding Generation**
   - Forward pass through ONNX model
   - Mean pooling over token embeddings
   - L2 normalization for cosine similarity
   - Return 768-dim f32 vectors

## Implementation Plan

### Phase 1: Dependencies

Add to `Cargo.toml`:
```toml
[dependencies]
ort = "2.0"                   # ONNX Runtime
tokenizers = "0.19"           # HuggingFace tokenizers
reqwest = { version = "0.12", features = ["blocking", "json"] }  # For downloads
sha2 = "0.10"                 # Checksum verification
```

### Phase 2: Model Infrastructure

Files to create/modify:
- `src/semantic/model_download.rs` - Model downloading and caching
- `src/semantic/embeddings.rs` - Replace with ONNX implementation
- `src/semantic/config.rs` - Add model config options

### Phase 3: API Compatibility

Maintain existing API:
```rust
impl EmbeddingModel {
    pub fn new() -> Self;  // Load with defaults
    pub fn embedding_dim(&self) -> usize;
    pub fn encode(&mut self, text: &str) -> Result<Vec<f32>>;
    pub fn encode_batch(&mut self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
}
```

### Phase 4: Configuration

Add to `semantic_config.toml`:
```toml
[model]
name = "microsoft/codebert-base"
cache_dir = "~/.cache/fast_code_search_semantic"
use_gpu = false
max_sequence_length = 512
```

### Phase 5: Testing

Tests to add:
- Model download and caching
- Tokenization correctness
- Embedding generation
- Batch processing
- CPU vs GPU execution
- Fallback to TF-IDF on error

## Model Download Strategy

### HuggingFace Hub Integration

Download from:
- Model: `https://huggingface.co/Xenova/codebert-base/resolve/main/onnx/model.onnx` (ONNX-optimized)
- Tokenizer: `https://huggingface.co/microsoft/codebert-base/resolve/main/tokenizer.json`
- Config: `https://huggingface.co/microsoft/codebert-base/resolve/main/config.json`

**Note**: The original `microsoft/codebert-base` repository doesn't include ONNX models. We use `Xenova/codebert-base` which provides properly ONNX-optimized versions of the model exported using Optimum.

Cache structure:
```
~/.cache/fast_code_search_semantic/
└── models/
    └── microsoft-codebert-base/
        ├── model.onnx
        ├── tokenizer.json
        ├── config.json
        └── .checksums
```

## Performance Considerations

### Expected Performance

- **CPU Encoding**: ~50-100ms per query
- **GPU Encoding**: ~5-10ms per query
- **Batch Encoding**: 10-20x faster for batches of 32+
- **Memory**: ~500MB for model + ~100MB per 10k embeddings

### Optimization Strategies

1. Query caching (already implemented)
2. Batch indexing during file scan
3. Lazy model loading
4. GPU acceleration when available
5. Quantized models (future)

## Backward Compatibility

### Fallback Strategy

If ONNX model fails to load:
1. Log warning
2. Fall back to TF-IDF implementation
3. Continue working with degraded quality
4. Suggest troubleshooting to user

### Migration Path

For existing users:
1. First run: Download model automatically
2. Show progress during download
3. Save index with model version
4. Re-index if model changes

## Testing Strategy

### Unit Tests

- `test_model_download()` - Download and cache
- `test_tokenization()` - Tokenizer correctness
- `test_encode_single()` - Single text encoding
- `test_encode_batch()` - Batch encoding
- `test_embedding_normalization()` - L2 norm = 1

### Integration Tests

- `test_semantic_search_accuracy()` - Quality comparison
- `test_cpu_gpu_equivalence()` - Same results
- `test_fallback_to_tfidf()` - Error handling

### Benchmarks

- `bench_encode_query()` - Query encoding latency
- `bench_encode_batch()` - Batch throughput
- `bench_search_end_to_end()` - Full search latency

## Documentation Updates

Files to update:
- `SEMANTIC_SEARCH_README.md` - Add ML model section
- `SEMANTIC_SEARCH_IMPLEMENTATION_PLAN.md` - Update Phase 1
- `.github/copilot-instructions.md` - Add ML model info
- `CHANGELOG.md` - Add entry for ML model feature

## Success Criteria

- [ ] ONNX model downloads and caches correctly
- [ ] Tokenization works for code snippets
- [ ] Embeddings are 768-dimensional and normalized
- [ ] Search quality improves over TF-IDF baseline
- [ ] CPU performance is acceptable (<500ms per query)
- [ ] GPU acceleration works when available
- [ ] All tests pass
- [ ] Documentation is complete

## Risks and Mitigations

### Risk: Model download fails

**Mitigation**: Fallback to TF-IDF, retry logic, mirror URLs

### Risk: ONNX Runtime installation issues

**Mitigation**: Clear installation docs, Docker images, troubleshooting guide

### Risk: Poor search quality

**Mitigation**: Benchmarks, user feedback, model tuning

### Risk: High memory usage

**Mitigation**: Lazy loading, model quantization, batch size limits

## Timeline

- **Day 1-2**: Dependencies and model download
- **Day 3-4**: ONNX integration and embedding
- **Day 5**: Testing and benchmarking
- **Day 6**: Documentation and polish

Total: ~6 days for one developer

## Future Enhancements

- Support for multiple models (GraphCodeBERT, UniXcoder)
- Model quantization (INT8) for smaller size
- GPU batching for faster indexing
- Fine-tuning on specific codebases
- Model version management and updates
