# Copilot Instructions for fast_code_search

High-performance, in-memory code search service in Rust. Trigram indexing + symbol awareness for 10GB+ codebases.

## Quick Reference: File Map

| File | Responsibility |
|------|----------------|
| `src/index/trigram.rs` | Roaring bitmap trigram index (trigrams → doc IDs) |
| `src/index/file_store.rs` | Memory-mapped file storage via `memmap2` |
| `src/index/persistence.rs` | Save/load index to disk with file locking (v3: includes symbols & dependencies) |
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
| `docs/site/` | GitHub Pages static site (HTML/CSS/JS, deployed by `pages.yml`) |
| `docs/site/index.html` | Main site page (features, CI status, benchmarks, coverage, changelog) |
| `tests/integration_tests.rs` | End-to-end integration tests |
| `static/` | Embedded web UI files |

## Common Tasks Cheatsheet

| Task | Steps |
|------|-------|
| **Add new language** | 1. Add tree-sitter dep to `Cargo.toml` 2. Update `language_for_file()` in `src/symbols/extractor.rs` 3. Add node type patterns in `extract_functions()` 4. Add tests |
| **Modify gRPC API** | 1. Edit `proto/search.proto` and/or `proto/semantic_search.proto` 2. `cargo build` 3. Update `src/server/service.rs` and/or `src/semantic_server/service.rs` 4. Update corresponding client examples |
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

**Scoring factors**: Symbol defs (3x), exact match (2x), src/lib dirs (1.5x), import count (log boost), start-of-line (1.5x), shorter lines (log scale, min 0.3x)

> **Keep in sync**: When changing scoring factors in `src/search/engine.rs` (`calculate_score_inline` / `calculate_score_regex_inline`), update the `title` tooltip on the `result-score` span in `static/keyword.js` to match.

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
- Server ports: keyword gRPC 50051 + web 8080, semantic gRPC 50052 + web 8081

## Before Every Commit

```bash
cargo fmt                      # REQUIRED - CI rejects unformatted code
cargo clippy -- -D warnings    # REQUIRED - CI rejects clippy warnings
cargo test                     # Run full test suite
```

## Testing

**Integration tests are prioritized.** Add integration tests for user-visible behavior and targeted unit tests for module logic.

```bash
cargo test                           # All tests
cargo test --test integration_tests  # Integration only
cargo test -- --nocapture            # Show output
```

Test pattern: Use `setup_test_server()` helper, test both gRPC and HTTP, use `TempDir` for cleanup.

> See [docs/DEVELOPMENT.md](../docs/DEVELOPMENT.md#testing) for detailed testing guide.

## Code Coverage

Coverage reports are generated automatically on every CI run. Download the `coverage-report` artifact from the GitHub Actions run to view the HTML report.

To generate coverage locally, install [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) and run:

```bash
# Install (one-time)
cargo install cargo-llvm-cov
rustup component add llvm-tools-preview

# Generate and open HTML report
cargo llvm-cov --all-features --workspace --open

# Generate LCOV file (for IDE integration)
cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info
```

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

Instruction governance source: `docs/INSTRUCTION_FILES_BLUEPRINT.md`

## Web UI Architecture & Consistency

### Two Parallel Search Interfaces

| Aspect | Keyword Search (`index.html`) | Semantic Search (`semantic.html`) |
|--------|------|------|
| **File** | `static/index.html` + `static/keyword.js` | `static/semantic.html` + `static/semantic.js` |
| **Features** | Advanced: regex, filtering, symbols, ranking, context lines | Simplified: basic semantic matching only |
| **Result Interaction** | Hover tooltips + full-file modal viewer | Read-only card display (no tooltips/modal) |
| **UI Framework** | Tailwind CSS + custom layout | Tailwind CSS + custom layout |

### Consistent Behaviors (Must Stay in Sync)

Both UIs **must maintain consistency** in:
1. **Search History** — localStorage (`fcs_history` / `fcs_sem_history`) via `loadHistory()` / `saveToHistory()`
2. **Settings Persistence** — localStorage (`fcs_settings` / `fcs_sem_settings`) via `loadSettingsToStorage()`
3. **URL State** — query parameter sync via `syncUrlFromState()` and `loadStateFromUrl()`
4. **Index Readiness** — shared `searchReadiness` object from `common.js` (progress, WebSocket, readiness signals)
5. **Backend Health** — both call `checkBackendHealth()` and render status banners identically
6. **HTML Escaping** — both use `escapeHtml()` from `common.js` to prevent XSS
7. **Language Badges** — both use `langClassForPath()` from `common.js` for consistent language coloring
8. **Loading/Error States** — both use `showLoading()` / `showError()` from `common.js`

### UI-Specific Behaviors (Do NOT Transfer)

**Keyword Search Only:**
- Context tooltip system (`hideContextTooltipImmediately()`, `showContextTooltip()`, `_ctxTooltip`) — **Tooltip must dismiss immediately when viewing a file to prevent lingering underneath the modal**
- Full-file modal viewer (`showFileModal()`, `closeFileModal()`)
- Regex mode, symbol-only filtering, ranking mode selection
- Context lines display in results
- Path filtering (include/exclude patterns)

**Semantic Search Only:**
- Similarity score bar visualization
- Chunk type badges (function, class, module — not in keyword search)
- Simplified 1-step result display (no drill-down)

### When Updating Frontend Code

1. **Shared code** (in `common.js`): Update both UIs via the shared function
2. **Keyword-only features**: Update `keyword.js` only; never port to semantic unless UX parity is intentional
3. **Semantic-only features**: Update `semantic.js` only; never port to keyword (would overcomplicate the interface)
4. **New shared feature** (e.g., a new badge type, new localStorage key): Add to `common.js` first, then use in both UIs
5. **After any edit to common.js**: Test both `index.html` **and** `semantic.html` in the browser to confirm consistency

### Tooltip Cleanup (Keyword UI)

When clicking to view a file or performing any navigation that opens a modal/overlay:
- Always call `hideContextTooltipImmediately()` **before** rendering the modal to prevent tooltip artifacts
- This function clears pending timers, aborts in-flight fetches, and hides the element immediately (unlike the delayed `hideContextTooltip()`)

## Benchmarks

```bash
cargo bench -- --save-baseline before  # Before changes
cargo bench -- --baseline before       # After changes, compare
```

Reports at `target/criterion/report/index.html`. Benchmark groups: `text_search`, `regex_search`, `filtered_search`, `indexing`.
