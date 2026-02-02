# Copilot Instructions for fast_code_search

High-performance, in-memory code search service built in Rust. Handles 10GB+ codebases with trigram-based indexing, symbol awareness, and parallel search.

## Architecture Overview

The system has five layers that communicate through shared `Arc<RwLock<SearchEngine>>`:

1. **Index Layer** (`src/index/`) - File discovery, trigram indexing, and persistence
   - `file_store.rs`: Memory-mapped files via `memmap2` for zero-copy access
   - `trigram.rs`: Roaring bitmap-based inverted index mapping trigrams → document IDs (`u32`)
   - `persistence.rs`: Save/load index to disk with file locking and config fingerprinting

2. **Symbol Layer** (`src/symbols/extractor.rs`) - Tree-sitter parsing for Rust/Python/JS/TS
   - Extracts function/class definitions to boost search relevance
   - Also extracts import statements for dependency analysis

3. **Dependency Layer** (`src/dependencies/mod.rs`) - Import graph for PageRank-style scoring
   - `imports`: file_id → set of file_ids it imports
   - `imported_by`: file_id → set of file_ids that import it (reverse index)
   - `import_counts`: cached counts for fast scoring lookups
   - Files with more dependents get logarithmic boost: `1.0 + log10(count) * 0.5`

4. **Search Layer** (`src/search/`) - Parallel search with scoring
   - `engine.rs`: Core search using rayon for parallel line-by-line search
   - `regex_search.rs`: Regex pattern analysis for trigram acceleration
   - `path_filter.rs`: Glob-based include/exclude filtering
   - Multi-factor scoring: symbol defs (3x), exact match (2x), src/lib dirs (1.5x)
   - Four search methods:
     - `search()`: Basic text search
     - `search_with_filter()`: Text search with path filtering
     - `search_regex()`: Regex pattern matching with trigram acceleration
     - `search_symbols()`: Symbols-only search for function/class names

5. **Server Layer** - Dual API exposure (shared engine via `Arc<RwLock<>>`)
   - `src/server/service.rs`: gRPC streaming via tonic (port 50051)
   - `src/web/api.rs`: REST/JSON via axum (port 8080)
   - `static/`: Web UI files embedded via `rust-embed`

**Data Flow**: Query → trigram extraction → bitmap intersection → candidate docs → parallel search → scored results → streaming response

### Index Persistence

The index can be saved to disk and loaded on restart for faster startup:

- **Configuration**: Set `index_path` in config to enable persistence
- **Save triggers**: After initial build (`save_after_build = true`), or after N file updates (`save_after_updates`)
- **Load on startup**: If index file exists, loads it with reconciliation against current config
- **Staleness detection**: Checks mtime + size of each file; stale files are re-indexed
- **Config fingerprint**: Hash of paths/extensions/excludes stored in index; if config changes, affected paths are reconciled
- **File locking**: Exclusive lock for writes, shared lock for reads (multiple servers can share read-only access)
- **Reconciliation**: Background task walks filesystem to find new files, updates index incrementally

## Build & Development

```bash
# Build (requires protoc for gRPC codegen)
cargo build --release

# Run server with config
cargo run --release -- --config ./config.toml

# Generate template config file
cargo run --release -- --init fast_code_search.toml

# Run tests (unit + integration)
cargo test
```

The `build.rs` compiles `proto/search.proto` via tonic-build on each build.

### ML Models Feature (Semantic Search)

The `ml-models` feature enables ONNX-based embeddings for semantic search:

```bash
cargo build --release --bin fast_code_search_semantic --features ml-models
```

**Windows-specific**: Due to CRT linking conflicts between `ort` (dynamic /MD) and `tokenizers` (static /MT), the `ort` crate uses `load-dynamic` feature. This requires:

1. Download ONNX Runtime from https://github.com/microsoft/onnxruntime/releases
2. Set `ORT_DYLIB_PATH` environment variable to point to `onnxruntime.dll`

See `docs/semantic/SEMANTIC_SEARCH_README.md` for detailed setup instructions.

## Before Every Commit

**IMPORTANT**: Always run these commands before committing to avoid CI failures:

```bash
# Format code (REQUIRED - CI will reject unformatted code)
cargo fmt

# Run linter (REQUIRED - CI will reject code with clippy warnings)
cargo clippy -- -D warnings

# Run tests
cargo test
```

The CI pipeline runs `cargo fmt --all -- --check` and `cargo clippy -- -D warnings` on every PR. Commits that fail these checks will block the PR.

## Documentation Requirements

**IMPORTANT**: Documentation must be kept up-to-date with every change. This includes:

### Files to Update with Every Change

1. **This file** (`.github/copilot-instructions.md`):
   - Update architecture descriptions when adding/modifying components
   - Add new patterns or conventions when established
   - Update API documentation when endpoints change
   - Add new build/test commands when introduced

2. **README.md**:
   - Update feature descriptions for user-facing changes
   - Update usage examples when CLI or API changes
   - Update installation instructions when dependencies change
   - Update benchmark results table after performance changes

3. **CHANGELOG.md**:
   - Add entry for every user-facing change
   - Follow Keep a Changelog format (Added, Changed, Deprecated, Removed, Fixed, Security)

4. **Code comments and doc comments**:
   - Update function/struct doc comments when behavior changes
   - Keep inline comments accurate with code changes

### When Adding New Features

- Add new section to this instructions file explaining the feature
- Update README.md with user-facing documentation
- Add CHANGELOG.md entry
- Update any relevant design docs (e.g., `DESIGN-*.md` files)

### When Modifying Existing Features

- Review and update all documentation that references the changed behavior
- Search for outdated references in markdown files
- Update examples if API signatures change

### When Removing Features

- Remove or update references in all documentation files
- Add deprecation notice to CHANGELOG.md
- Update this instructions file to remove obsolete sections

## Handling Merge Conflicts

When there are merge conflicts with the main branch, **always resolve them automatically** by:

1. Fetching the latest main branch
2. Merging or rebasing to incorporate upstream changes
3. Resolving any conflicts by keeping both sets of changes when possible
4. Running `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` after resolution
5. Committing the resolved changes

This ensures the PR stays up-to-date with the latest main branch changes.

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

### REST API Endpoints (src/web/api.rs)
The REST API supports the following search parameters:
- `q`: Query string (required)
- `max`: Maximum results (default: 50)
- `include`: Semicolon-delimited glob patterns for paths to include
- `exclude`: Semicolon-delimited glob patterns for paths to exclude
- `regex`: Set to `true` for regex pattern matching
- `symbols`: Set to `true` for symbols-only search (functions/classes)

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

Unit tests live in same file using `#[cfg(test)]` modules. Integration tests in `tests/integration_tests.rs` spin up real gRPC/HTTP servers with temp directories.

```bash
cargo test index::trigram          # Specific module
cargo test -- --nocapture          # See output
cargo test integration             # Integration tests only
```

## Performance Optimization Workflow

When doing optimization work, **always use benchmarks to measure impact**:

### Before Making Changes
```bash
# Save current performance as baseline
cargo bench -- --save-baseline before
```

### After Making Changes
```bash
# Compare against baseline to quantify improvement
cargo bench -- --baseline before
```

### Benchmark Suite
Benchmarks are in `benches/search_benchmark.rs`. Key groups:
- `text_search` - Basic search with varying corpus sizes
- `regex_search` - Regex patterns with/without trigram acceleration
- `filtered_search` - Path filtering impact
- `case_sensitivity` - Case folding overhead
- `result_limits` - Impact of max_results parameter
- `indexing` - File indexing throughput

### Quick Benchmark Check
```bash
# Run specific benchmark with fewer samples for fast feedback
cargo bench -- "text_search/common_query/100" --sample-size 10
```

### View Detailed Reports
HTML reports with graphs are generated at `target/criterion/report/index.html`

### What NOT to Commit
- Benchmark baselines (`target/criterion/`) - machine-specific, already gitignored
- Only commit the benchmark code itself

## Release Checklist

Before preparing a release:

1. **Update benchmarks in README.md**:
   ```bash
   cargo bench
   ```
   Then update the Benchmarks table in `README.md` with latest results.

2. **Update version** in `Cargo.toml`

3. **Update CHANGELOG.md** with notable changes

4. **Run full test suite and formatting**:
   ```bash
   cargo fmt
   cargo clippy -- -D warnings
   cargo test
   ```

5. **Tag the release**: `git tag v0.x.x`

## Project-Specific Conventions

- Default branch is `main` (not `master`)
- Use `Arc<RwLock<SearchEngine>>` for shared engine state between gRPC and web servers
- Document IDs are `u32` indices into `FileStore.files` vector
- Line numbers in results are 1-based; internally 0-based
- Trigrams operate on raw bytes, not Unicode graphemes
