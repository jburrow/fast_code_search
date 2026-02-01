# Copilot Instructions for fast_code_search

High-performance, in-memory code search service built in Rust. Handles 10GB+ codebases with trigram-based indexing, symbol awareness, and parallel search.

## Architecture Overview

The system has four layers that communicate through shared data structures:

1. **Index Layer** (`src/index/`) - File discovery and trigram indexing
   - `file_store.rs`: Memory-mapped files via `memmap2` for zero-copy access
   - `trigram.rs`: Roaring bitmap-based inverted index mapping trigrams → document IDs

2. **Symbol Layer** (`src/symbols/extractor.rs`) - Tree-sitter parsing for Rust/Python/JS/TS
   - Extracts function/class definitions to boost search relevance

3. **Search Layer** (`src/search/engine.rs`) - Parallel search with scoring
   - Uses rayon for parallel line-by-line search across candidate documents
   - Multi-factor scoring: symbol defs (3x), exact match (2x), src/lib dirs (1.5x)

4. **Server Layer** - Dual API exposure
   - `src/server/service.rs`: gRPC streaming via tonic (port 50051)
   - `src/web/api.rs`: REST/JSON via axum (port 8080)

**Data Flow**: Query → trigram extraction → bitmap intersection → candidate docs → parallel search → scored results → streaming response

## Build & Development

```bash
# Build (requires protoc for gRPC codegen)
cargo build --release

# Run server with config
cargo run --release -- --config ./config.toml

# Run tests
cargo test

# Lint before committing
cargo clippy -- -D warnings && cargo fmt --check
```

The `build.rs` compiles `proto/search.proto` via tonic-build on each build.

## Key Patterns

### Error Handling
Use `anyhow::Result` for propagation with context:
```rust
File::open(path).with_context(|| format!("Failed to open: {}", path.display()))?;
```

### Adding New Language Support
1. Add tree-sitter dependency in `Cargo.toml`
2. Update `language_for_file()` match in `src/symbols/extractor.rs`
3. Add extraction tests

### Modifying gRPC API
1. Edit `proto/search.proto`
2. Run `cargo build` to regenerate bindings
3. Update `src/server/service.rs` implementation
4. Update `examples/client.rs`

### Configuration
TOML config in `config.toml` or `fast_code_search.toml`. Key settings:
- `server.address`: gRPC bind address
- `indexer.paths`: Directories to auto-index
- `indexer.exclude_patterns`: Glob patterns to skip (node_modules, target, .git)

## Threading Model

- **Indexing**: Single-threaded, I/O bound (memory-mapping)
- **Search**: Parallel via rayon (CPU bound)
- **Servers**: Tokio async runtime for gRPC/HTTP

## Testing

Unit tests live in same file using `#[cfg(test)]` modules. Run specific module tests:
```bash
cargo test index::trigram
cargo test -- --nocapture  # See output
```

## Project-Specific Conventions

- Use `Arc<Mutex<SearchEngine>>` for shared engine state between gRPC and web servers
- Document IDs are `u32` indices into `FileStore.files` vector
- Line numbers in results are 1-based; internally 0-based
- Trigrams operate on raw bytes, not Unicode graphemes
