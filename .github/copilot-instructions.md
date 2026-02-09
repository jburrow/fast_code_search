# Copilot Instructions for fast_code_search

High-performance, in-memory code search service in Rust. Trigram indexing + symbol awareness for 10GB+ codebases.

## Quick Reference: File Map

| File | Responsibility |
|------|----------------|
| `src/index/trigram.rs` | Roaring bitmap trigram index (trigrams → doc IDs) |
| `src/index/file_store.rs` | Memory-mapped file storage via `memmap2` |
| `src/index/persistence.rs` | Save/load index to disk with file locking |
| `src/symbols/extractor.rs` | Tree-sitter parsing for 12+ languages (Rust, Python, JS/TS, Go, C/C++, Java, C#, Ruby, PHP, Bash) |
| `src/dependencies/mod.rs` | Import graph for PageRank-style scoring |
| `src/search/engine.rs` | Core parallel search with rayon + scoring |
| `src/search/regex_search.rs` | Regex pattern analysis for trigram acceleration |
| `src/search/path_filter.rs` | Glob-based include/exclude filtering |
| `src/server/service.rs` | gRPC streaming server (port 50051) |
| `src/web/api.rs` | REST/JSON API via axum (port 8080) |
| `src/semantic/engine.rs` | Semantic search coordinator |
| `src/semantic/embeddings.rs` | TF-IDF / ONNX ML embeddings |
| `src/semantic/chunking.rs` | Code chunking into functions/classes |
| `src/semantic/vector_index.rs` | HNSW vector index for fast similarity search |
| `src/semantic/config.rs` | Semantic search configuration (HNSW params) |
| `src/semantic_server/service.rs` | Semantic gRPC server (port 50052) |
| `src/semantic_web/api.rs` | Semantic REST API (port 8081) |
| `proto/search.proto` | gRPC service definitions |
| `tests/integration_tests.rs` | End-to-end integration tests |
| `static/` | Embedded web UI files |

## Common Tasks Cheatsheet

| Task | Steps |
|------|-------|
| **Add new language** | 1. Add tree-sitter dep to `Cargo.toml` 2. Update `language_for_file()` in `src/symbols/extractor.rs` 3. Add node type patterns in `extract_functions()` 4. Add tests |
| **Modify gRPC API** | 1. Edit `proto/search.proto` 2. `cargo build` 3. Update `src/server/service.rs` 4. Update `examples/client.rs` |
| **Add REST endpoint** | Edit `src/web/api.rs` — params: `q` (query), `max`, `include`, `exclude`, `regex`, `symbols` |
| **Add new scoring factor** | Edit `calculate_score()` in `src/search/engine.rs` |
| **Change config options** | Edit `src/config.rs` — TOML config loaded at startup |

## Architecture Summary

**Data Flow**: Query → trigram extraction → bitmap intersection → candidate docs → parallel search → scored results → streaming response

**Five Layers** (shared via `Arc<RwLock<SearchEngine>>`):
1. **Index** (`src/index/`) — trigram indexing, file storage, persistence
2. **Symbols** (`src/symbols/`) — tree-sitter extraction for function/class defs
3. **Dependencies** (`src/dependencies/`) — import graph for scoring boost
4. **Search** (`src/search/`) — parallel search, regex, path filtering
5. **Server** (`src/server/`, `src/web/`) — gRPC + REST APIs

**Scoring factors**: Symbol defs (3x), exact match (2x), src/lib dirs (1.5x), import count (log boost)

> See [README.md](../README.md#architecture) for detailed architecture docs.

## Key Patterns

### Error Handling
```rust
File::open(path).with_context(|| format!("Failed to open: {}", path.display()))?;
```
Use `anyhow::Result` with context for all error propagation.

### Threading Model
- **Indexing**: Single-threaded, I/O bound (memory-mapping)
- **Search**: Parallel via rayon (CPU bound)
- **Servers**: Tokio async runtime for gRPC/HTTP

### Project Conventions
- Default branch: `main`
- Shared state: `Arc<RwLock<SearchEngine>>`
- Document IDs: `u32` indices into `FileStore.files`
- Line numbers: 1-based in results, 0-based internally
- Trigrams: raw bytes, not Unicode graphemes

### Running Servers
- **NEVER** kill running server processes to "check if running" - use a separate terminal or API call
- Test API endpoints with `curl` or `Invoke-WebRequest` in a **new terminal**
- If you need to rebuild, ask the user first or use a separate terminal
- Server ports: gRPC 50051/50059, Web UI 8080, Semantic 8081

## Before Every Commit

```bash
cargo fmt                      # REQUIRED - CI rejects unformatted code
cargo clippy -- -D warnings    # REQUIRED - CI rejects clippy warnings
cargo test                     # Run full test suite
```

## Testing

**Integration tests are prioritized.** Always add integration tests for new features.

```bash
cargo test                           # All tests
cargo test --test integration_tests  # Integration only
cargo test -- --nocapture            # Show output
```

Test pattern: Use `setup_test_server()` helper, test both gRPC and HTTP, use `TempDir` for cleanup.

> See [docs/DEVELOPMENT.md](../docs/DEVELOPMENT.md#testing) for detailed testing guide.

## Validator Tool

Whitebox testing binary for validating search engines with synthetic corpus generation.

```bash
# Basic validation (generates 100 files, runs all tests)
cargo run --release --bin fast_code_search_validator

# Custom corpus size and seed for reproducibility
cargo run --release --bin fast_code_search_validator -- --corpus-size 200 --seed 12345

# With load testing (measure throughput and latency)
cargo run --release --bin fast_code_search_validator -- --load-test --duration 30

# JSON output for CI integration
cargo run --release --bin fast_code_search_validator -- --json
```

**CLI Options:**
- `--corpus-size N` — Number of files to generate (default: 100)
- `--seed N` — Random seed for reproducible corpus (default: 42)
- `--sample-count N` — Additional random samples for validation (default: 10)
- `--load-test` — Enable throughput/latency measurement
- `--concurrent N` — Parallel query threads for load test (default: 4)
- `--duration N` — Load test duration in seconds (default: 10)
- `--json` — Output results as JSON
- `--keep-corpus` — Don't delete temp directory after run

**What it tests:**
- Index completeness: All generated needles are findable
- Line number accuracy: Matches report correct line numbers
- Symbol extraction: Generated symbols are searchable
- Query options: `search()`, `search_with_filter()`, `search_regex()`, `search_symbols()`
- Path filtering: Include/exclude patterns work correctly

**CI Integration:**
```yaml
- run: cargo run --release --bin fast_code_search_validator -- --json
```

> Source: `src/bin/fast_code_search_validator.rs`, `src/bin/validator/corpus.rs`

## Semantic Search (Optional)

Requires `ml-models` feature + ONNX Runtime 1.18-1.22:

```bash
cargo build --release --bin fast_code_search_semantic --features ml-models
```

Windows: Use `scripts\run_semantic_server.ps1` (sets `ORT_DYLIB_PATH`).

> See [docs/semantic/SEMANTIC_SEARCH_README.md](../docs/semantic/SEMANTIC_SEARCH_README.md) for setup.

## Documentation Updates

When changing code, update:
1. This file — for architecture/pattern changes
2. `README.md` — for user-facing changes
3. `CHANGELOG.md` — for every user-visible change

## Benchmarks

```bash
cargo bench -- --save-baseline before  # Before changes
cargo bench -- --baseline before       # After changes, compare
```

Reports at `target/criterion/report/index.html`. Benchmark groups: `text_search`, `regex_search`, `filtered_search`, `indexing`.
