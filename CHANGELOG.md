# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.4] - 2026-02-25

### Added
- **`exclude_files` config option**: Explicitly exclude specific file paths from indexing.
  Use this when a file causes a crash — identify it via `fcs_last_processed.txt` then add it:
  ```toml
  [indexer]
  exclude_files = ["/repo/src/generated/bad_file.rs"]
  ```
- **Crash probe file `fcs_last_processed.txt`**: Written to the working directory before each
  tree-sitter symbol extraction call. After a SIGABRT crash, read this file to identify the
  exact file being processed at the time of the crash.
- **Per-file DEBUG logging**: Both indexing phases now emit `tracing::debug!` with the file path.
  Run with `RUST_LOG=fast_code_search=debug` to see a rolling log of files being processed,
  making the crashing file identifiable from the last log entry.

## [0.5.3] - 2026-02-25

### Added
- **Version logged on startup**: The binary version is now emitted as a structured log field
  (`version=X.Y.Z`) alongside the startup address line, making it easy to confirm which build
  is running.
- **Debug builds in release artifacts**: Each GitHub release now also ships a debug build
  (with full symbols) per platform, archived as `fast_code_search-VERSION-TARGET-debug.{tar.gz,zip}`.
  Use these builds when diagnosing crashes — `RUST_BACKTRACE=full` will show real function names.

## [0.5.2] - 2026-02-25

### Fixed
- **SIGABRT / heap corruption during parallel batch indexing**: Tree-sitter C parsers were being
  called concurrently from rayon's thread pool (`par_iter` over a 500-file batch). Tree-sitter's
  internal C allocator is not thread-safe under concurrent use, causing intermittent heap corruption
  manifesting as `memory allocation of N bytes failed` + `Aborted (core dumped)` after ~110 batches
  (~55K files). Fixed by splitting `PreIndexedFile::process` into two phases:
  1. `PartialIndexedFile::process` — pure-Rust trigram extraction, runs in parallel safely.
  2. `PreIndexedFile::from_partial` — tree-sitter symbol/import extraction, runs sequentially.
  Trigram extraction (the dominant CPU cost) remains parallel; only the tree-sitter FFI calls
  are serialised.

## [0.5.1] - 2026-02-25

### Fixed
- **OOM crash during background indexing**: `PreIndexedFile` was retaining a full copy of each file's content (`String`, up to 10 MB each) in memory while rayon processed batches in parallel. With a 500-file batch size this could accumulate up to 5 GB of redundant heap on top of the trigram index, causing an allocation failure on large codebases (~60K files). The `content` field has been removed from `PreIndexedFile` — it was only needed transiently inside `process()` to compute trigrams, symbols, and imports, all of which are now the only data carried into `index_batch`.

## [0.5.0] - 2026-02-25

### Added
- **HNSW vector index for semantic search**: Replaced linear O(n) search with HNSW O(log n) algorithm
  - New `hnsw_rs` dependency for fast approximate nearest neighbor search
  - Configurable HNSW parameters in `[indexer.hnsw]` section: `m`, `ef_construction`, `ef_search`
  - Default values optimized for moderate codebases (m=16, ef_construction=200, ef_search=100)
  - Embeddings now persisted alongside HNSW index for proper save/load functionality
  - 2-10x faster query performance for large codebases (>10K chunks)

### Changed
- Semantic search vector index now uses HNSW instead of linear search
- Updated documentation with HNSW configuration and tuning guide
- Memory overhead increased to ~3x indexed code size (from ~2x) due to HNSW graph structure

## [0.4.0] - 2026-02-09

### Added
- **OpenTelemetry distributed tracing**: Both server binaries now export traces via OTLP/gRPC
  - New `[telemetry]` TOML config section with `enabled`, `otlp_endpoint`, and `service_name`
  - Enabled by default — set `enabled = false` or `OTEL_SDK_DISABLED=true` to disable
  - Standard env var overrides: `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_SERVICE_NAME`, `FCS_TRACING_ENABLED`
  - New `src/telemetry.rs` module with `init_telemetry()` / `shutdown_telemetry()` lifecycle
  - Graceful shutdown flushes all pending spans before process exit

- **Tracing instrumentation across the stack**:
  - `#[tracing::instrument]` on all public search methods: `search`, `search_ranked`, `search_with_filter`, `search_with_filter_ranked`, `search_regex`, `search_symbols`
  - `#[tracing::instrument]` on semantic engine `search` method
  - `#[tracing::instrument]` on all gRPC handlers (keyword + semantic)
  - `tower_http::trace::TraceLayer` on both HTTP/REST routers
  - `Server::builder().trace_fn()` on both gRPC servers

- **Semantic model download verification**: Optional SHA256 verification via `FCS_MODEL_SHA256` or `FCS_MODEL_SHA256_<MODEL>`

### Changed
- Tracing subscriber upgraded from simple `FmtSubscriber` to layered `Registry` (fmt + OTel)
- `tracing-subscriber` now uses `registry` feature alongside `env-filter`
- `tower-http` now enables `trace` feature alongside `cors`
- Binary startup reordered: config loads before tracing init so TOML telemetry values are available
- `--init` flag exits before tracing init (no subscriber needed for template generation)

### Fixed
- **Persisted index loads now rebuild symbol and dependency caches** to restore symbol scoring boosts and import-based ranking
- **gRPC keyword service lock handling** now reports lock errors instead of panicking on poisoned locks

### Dependencies
- Added `opentelemetry` 0.27, `opentelemetry_sdk` 0.27 (rt-tokio), `opentelemetry-otlp` 0.27 (tonic), `tracing-opentelemetry` 0.28

## [0.3.0] - 2026-02-06

### Added
- **Expanded language support**: Tree-sitter symbol extraction now supports 18 languages (up from 4)
  - **Programming languages**: Rust, Python, JavaScript, TypeScript, Go, C, C++, Java, C#, Ruby, PHP, Bash
  - **Config & markup**: JSON, TOML, YAML, HTML, CSS, Markdown
  - Additional file extensions: `.pyi`, `.pyw`, `.mjs`, `.cjs`, `.mts`, `.cts`, `.cc`, `.cxx`, `.hpp`, `.hxx`, `.hh`, `.rake`, `.gemspec`, `.jsonc`, `.scss`

- **New symbol types extracted per language**:
  - Go: functions, methods, structs, interfaces, type aliases, constants, variables
  - C/C++: functions, structs, unions, enums, classes, namespaces (C++ templates traversed)
  - Java: classes, interfaces, enums, records, methods, constructors
  - C#: classes, structs, interfaces, enums, records, methods, properties, constructors
  - Ruby: classes, modules, methods (including singleton methods)
  - PHP: classes, interfaces, traits, functions, methods
  - Bash: functions

### Changed
- Symbol extractor now handles C/C++ function declarators correctly (name is in `declarator` field, not `name`)

## [0.2.4] - 2026-02-05

### Added
- **Two-phase ranking system for large-scale search**: Dramatically improves search performance on codebases with 100k+ files
  - Fast mode: Ranks candidates by pre-computed file metadata (symbols, imports, path), reads only top 2000 files
  - Full mode: Reads all candidate files for complete scoring
  - Auto mode (default): Automatically switches to Fast when >5,000 candidates
  - File-level scoring factors: symbol density (+4 max), src/lib location (+2), import count (+5 max), test/example penalty (0.7×)
  - New `rank` API parameter: `auto`, `fast`, or `full`
  - Response includes `rank_mode`, `total_candidates`, and `candidates_searched` metadata

- **Ranking mode UI toggle**: New dropdown in Advanced Options to select ranking mode
  - Results header shows actual mode used and files searched (e.g., "⚡ Fast (2000/100,000 files)")

- **Documentation page**: New `/docs.html` page in Web UI explaining the ranking system, API reference, and path filter patterns

- **Lazy file store**: Memory-mapped file content is now loaded on-demand rather than all at once
  - Reduces memory usage during index loading
  - ~8x faster index loading via parallel file mapping

### Changed
- Search methods now use file metadata ranking instead of early termination for large candidate sets
- `search_regex()` and `search_symbols()` also use fast ranking for consistency

## [0.2.3] - 2026-02-04

### Added
- **Terminal progress bars for index loading**: Added animated progress bars when loading a persisted index from disk
  - Shows phase-specific indicators: 📖 Reading file, 🔄 Deserializing, 🔍 Checking files, 🧠 Restoring index, 📁 Loading files
  - Progress bar shows elapsed time, visual progress, position/total count, and ETA
  - Displays summary with file count when loading completes

- **Whitebox validator binary**: New `fast_code_search_validator` binary for comprehensive search engine validation
  - Generates deterministic synthetic corpus with seeded randomness for reproducible tests
  - Multi-language corpus: Rust (40%), Python (25%), TypeScript (20%), JavaScript (15%)
  - Varied file sizes and complexity (functions, classes, nested structures)
  - Validates index completeness via embedded "needle" markers at known locations
  - Tests all query options: `search()`, `search_with_filter()`, `search_regex()`, `search_symbols()`
  - Verifies line number accuracy and symbol extraction
  - Optional load testing mode with throughput (queries/sec) and latency percentiles (p50/p95/p99)
  - JSON output mode for CI integration
  - Run with: `cargo run --release --bin fast_code_search_validator`

- **Index persistence**: The trigram index can now be saved to disk and loaded on restart for faster startup times
  - Configure `index_path` in config to enable persistence
  - `save_after_build = true` (default) saves after initial indexing completes
  - `save_after_updates = N` saves after N file updates via watcher (disabled by default)
  - Stores config fingerprint for detecting configuration changes
  - File locking ensures safe concurrent access (exclusive writes, shared reads)
  - Multiple read-only servers can share the same index file

- **Incremental reconciliation**: When loading a persisted index, the system reconciles against the current filesystem
  - Detects stale files via mtime + size checks and re-indexes them
  - Detects new files not in index and indexes them
  - Detects removed files and excludes them from results
  - Detects config changes (paths, extensions, excludes) and incrementally updates
  - Background reconciliation allows searches during the update process

- **New indexing status states**: UI shows `loading_index` when loading from disk and `reconciling` during background reconciliation

- **Symbols-only search mode**: New search mode that searches only in discovered symbol names (functions, classes, methods, types, etc.) plus filename matches. Enable via `symbols=true` query parameter in REST API or `symbols_only=true` in gRPC SearchRequest. This provides faster, more targeted results when looking for definitions.

### Changed
- **Updated performance comparison docs**: Added comprehensive benchmark comparison with traditional search tools (ripgrep, ag, git grep, grep) in README.md and PRIOR_ART.md. Includes published benchmark data from ripgrep's official benchmarks, break-even analysis for when indexing pays off, and detailed feature comparison tables.

### Fixed
- **ML models build on Windows**: Fixed ort 2.0 API compatibility issues and CRT linking conflicts
  - Updated `ort` dependency to use `load-dynamic` feature to avoid RuntimeLibrary mismatch with `tokenizers`
  - Fixed import paths for `Session`, `GraphOptimizationLevel`, and `Tensor` in ort 2.0 API
  - Added comprehensive Windows setup documentation in `docs/semantic/SEMANTIC_SEARCH_README.md`
  - Windows users must download ONNX Runtime DLL and set `ORT_DYLIB_PATH` environment variable
- **ONNX Runtime version compatibility**: Downgraded `ort` from 2.0.0-rc.11 to 2.0.0-rc.10
  - rc.11 requires ONNX Runtime >= 1.23.x, but bundled version is 1.22.0
  - rc.10 supports ONNX Runtime 1.18.x - 1.22.x
  - Added Windows launcher scripts (`scripts/run_semantic_server.ps1` and `.bat`) that automatically set `ORT_DYLIB_PATH`

## [0.2.2] - 2026-02-01

### Changed
- **Improved indexing log readability**: The background indexing completion message now formats large numbers with underscore separators (e.g., `89_210`), rounds `files_per_sec` to whole numbers, and removes redundant raw byte values.

## [0.2.1] - 2026-02-01

### Added
- **Indexing completion stats**: When background indexing completes, the log now reports the total size of indexed text and the current process memory usage in human-readable format (e.g., "150.00 MB"). Uses the `sysinfo` crate for cross-platform memory reporting.

## [0.2.0] - 2026-02-01

### Fixed
- **Duplicate search results**: Fixed issue where the same file could appear multiple times in search results when discovered via symlinks or different path representations. The `FileStore` now deduplicates files by canonical path.

### Added
- **Server-side search timing**: Search API response now includes `elapsed_ms` field showing the actual server-side query time in milliseconds. The Web UI displays this instead of client-side round-trip time.

## [0.1.0] - 2026-02-01

Initial release of fast_code_search — a high-performance, in-memory code search service built in Rust, designed to handle 10GB+ codebases with sub-millisecond query times.

### Core Features

#### Trigram-Based Indexing
- Roaring bitmap inverted index maps 3-character sequences to document IDs
- O(n) intersection for candidate document selection
- Efficient storage with compressed bitmaps

#### Memory-Mapped File Storage
- Zero-copy file access via `memmap2` crate
- Handles 10GB+ codebases without loading everything into RAM
- Files indexed in parallel for fast startup

#### Symbol-Aware Search
- Tree-sitter parsing for **Rust**, **Python**, **JavaScript**, and **TypeScript**
- Extracts symbol definitions (functions, classes, methods, types, etc.)
- Import/dependency tracking for enhanced relevance

#### Intelligent Scoring System
- **Symbol definitions**: 3x boost for symbol definitions
- **Exact matches**: 2x boost for case-sensitive matches
- **Source directories**: 1.5x boost for `src/` and `lib/` paths
- **Line position**: Boost for matches at start of lines
- **Dependency awareness**: Factors in file connectivity

#### Parallel Search Engine
- Rayon-powered parallel line-by-line search
- Concurrent processing across all CPU cores
- Sub-millisecond search on large codebases

### Server & API

#### gRPC Streaming API (Port 50051)
- `Search` RPC with streaming results for real-time UI updates
- `Index` RPC for adding directories to the search index
- Protocol Buffers schema in `proto/search.proto`
- Match types: `TEXT`, `SYMBOL_DEFINITION`, `SYMBOL_REFERENCE`

#### REST/JSON API (Port 8080)
- `GET /api/search?q=query&max=50` — Search with JSON response
- `GET /api/stats` — Index statistics (files, size, trigrams)
- `GET /api/status` — Indexing progress and status
- `GET /api/health` — Health check endpoint
- CORS-enabled for browser clients

#### Embedded Web UI
- Static HTML/CSS/JS served from embedded files
- Browser-based search interface at `http://localhost:8080`
- Real-time search results display

### Configuration

#### TOML Configuration Files
- Auto-discovers `fast_code_search.toml` or `config.toml`
- XDG/platform config directories supported
- CLI argument overrides for all settings

#### CLI Options
```
fast_code_search_server [OPTIONS]

Options:
  -c, --config <FILE>       Path to configuration file
  -a, --address <ADDR>      Server listen address
  -i, --index <PATH>        Additional paths to index (repeatable)
      --no-auto-index       Skip automatic indexing on startup
  -v, --verbose             Enable verbose logging
      --init <FILE>         Generate template configuration file
  -h, --help                Print help
  -V, --version             Print version
```

#### Exclude Patterns
- Glob-based exclusion: `node_modules/`, `target/`, `.git/`, `*.min.js`
- Configurable via `indexer.exclude_patterns` in TOML

### Developer Tools

#### Example Clients
- `examples/client.rs` — Basic gRPC client demonstrating index and search
- `examples/benchmark_client.rs` — Performance benchmarking tool

#### Build System
- `build.rs` compiles Protocol Buffers on each build
- Cross-platform support: Linux, macOS, Windows

### Performance Characteristics

| Metric | Value |
|--------|-------|
| Indexing speed | ~100MB/s on modern hardware |
| Search latency | Sub-millisecond for most queries |
| Memory usage | Fraction of codebase size (memory-mapped) |
| Max file size | 10MB (larger files skipped) |

### Requirements

- **Rust**: 1.70 or later
- **protoc**: Protocol Buffers compiler

### Installation

```bash
# Linux/Debian
sudo apt-get install protobuf-compiler

# macOS
brew install protobuf

# Build
cargo build --release
```

[Unreleased]: https://github.com/jburrow/fast_code_search/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/jburrow/fast_code_search/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jburrow/fast_code_search/releases/tag/v0.1.0
