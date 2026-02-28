# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Automated performance benchmarks in CI**: A new `benchmark` workflow runs on every push to `main`. It executes `cargo bench` for both `search_benchmark` and `persistence_benchmark` suites using the Criterion bencher output format, runs the validator binary in JSON mode for functional performance validation, and uploads results as a `benchmark-results` artifact. Historical trends are tracked in the `gh-pages` branch via `benchmark-action/github-action-benchmark`, with automatic alerts on >200% regressions.
- **Registered `persistence_benchmark` in `Cargo.toml`**: The `benches/persistence_benchmark.rs` suite (save/load index, trigram deserialization, file staleness checks) was already present but not declared as a `[[bench]]` target. It is now runnable via `cargo bench --bench persistence_benchmark`.

## [0.6.7] - 2026-02-27

### Added
- **Code coverage reports in CI**: A new `coverage` job runs on every push and pull request using `cargo-llvm-cov`. The HTML coverage report is uploaded as a `coverage-report` GitHub Actions artifact and LCOV data is uploaded to Codecov. See the [Code Coverage](#code-coverage) section in the copilot instructions for how to generate reports locally.

### Fixed
- **Website becomes unresponsive during index building**: API handlers (`search`, `stats`, `dependents`, `dependencies`, `diagnostics`) previously called `engine.read()` which blocks until any pending write lock (held during batch merges) is released. Under concurrent load, this caused blocking threads to pile up and exhaust the Tokio thread pool, leading to memory pressure and system unresponsiveness. All handlers now use `engine.try_read()` and immediately return `503 Service Unavailable` with the message _"Index is currently being updated, please try again shortly"_ when the write lock is held. The `status` endpoint was already safe and is unchanged.

## [0.6.6] - 2026-02-27

### Fixed
- **OOM during indexing of large codebases (Vec/HashMap over-allocation)**: After each doubling growth, Rust's `Vec` and `FxHashMap` may carry up to 2× excess capacity. On a 62 000-file corpus this contributes tens to hundreds of MB of wasted heap. Three changes address this:
  1. `SearchEngine::compact_memory()` — a new method that calls `shrink_to_fit()` on `symbol_cache` (both outer Vec and every inner `Vec<Symbol>`), `pending_imports`, and the trigram HashMap. Called automatically every 50 batches during the initial index build, so memory is reclaimed without waiting for `finalize()`.
  2. `SearchEngine::finalize()` — additionally shrinks `file_metadata` and runs a full `compact_memory()` pass once indexing completes.
  3. `TrigramIndex::finalize()` — now includes `shrink_to_fit()` on the `FxHashMap<Trigram, RoaringBitmap>` which can hold ~50% empty bucket slots after bulk insertion.

## [0.6.5] - 2026-02-27

### Fixed
- **`content_fallback` bytes permanently retained in heap (regression since v0.6.2)**: Files whose mmap failed fell back to a `OnceLock<Vec<u8>>` cache, which is write-once — once populated, the bytes could never be freed. Replaced with `Mutex<Option<Vec<u8>>>` so the cached bytes are evictable. Resolves the memory-growth issue tracked in v0.6.4 for Linux deployments approaching the mmap limit.

### Improved
- **Persist symbol caches and import graph in saved index**: When saving the trigram index to disk, the per-file symbol caches and resolved dependency edges are now also persisted. On the next startup, symbols and imports are restored directly from the index file instead of being re-extracted from file contents, eliminating the `rebuild_symbols_and_dependencies` pass for unchanged files and significantly reducing startup latency for large codebases.
  - Symbols are only re-extracted for files whose mtime has changed (stale files), which are queued for re-indexing as before.
  - Persisted indexes from older versions (format < 3) are automatically rebuilt once and saved in the new format.
  - Index persistence format bumped to version 3.

## [0.6.4] - 2026-02-27

### Fixed
- **OOM / memory errors during indexing on large codebases (regression since v0.6.0)**: `BATCH_SIZE` was silently raised from 500 to 2000 in v0.6.0 as part of the Phase-2 parallelisation change (#24). During Phase 1, all `BATCH_SIZE` files are read into memory simultaneously, so peak RAM scales as `batch_size × average_file_size × ~4`. On repos with many sizable files (generated code, minified JS, large headers, etc.) this caused 4× higher indexing-time memory consumption and triggered out-of-memory crashes. The default has been restored to **500**. A new `batch_size` key under `[indexer]` lets operators tune the trade-off between throughput and RAM:
  ```toml
  [indexer]
  batch_size = 1000   # raise on high-RAM machines; lower if still OOM
  ```
- **`content_fallback` Vec permanently retains file bytes in heap (Linux, search-time; tracked)**: Files that fail mmap (Linux: past `vm.max_map_count`; any OS: individual mmap failure) have their content cached in a `OnceLock<Vec<u8>>` on first search access. Because `OnceLock` is write-once, those bytes are never freed. On large repos where most files exceed the mmap soft-limit, every search that returns such a file permanently pins its content in heap memory; after enough searches the process exhausts RAM. A full fix requires refactoring the `&[u8]` lifetime API in `LazyMappedFile`; a TODO comment has been added at the relevant site and the fix is tracked. The `batch_size` fix above is sufficient for Windows users (where the mmap limit is not enforced).

## [0.6.3] - 2026-02-27

### Improved
- **Mmap-limit warning emitted only once**: Previously, every file registered without mmap (after the OS `vm.max_map_count` safe limit was reached) emitted its own `WARN` log line, flooding logs on large codebases. A single warning is now issued when the limit is first hit; all subsequent files are silently registered under the direct-read fallback.
- **Mmap-limit error message clarified**: The internal error text now states that remaining files will be indexed via direct-read fallback (slower retrieval, but search still works) instead of the previous misleading "Cannot index more files" wording.
- **Background indexer logs persistence settings at startup**: When `index_path` is configured, the indexer now emits an `INFO` log with the active `save_after_build`, `save_after_updates`, and `checkpoint_interval_files` values so operators can confirm persistence is active without reading config files.
- **README and startup warning updated**: Messaging around `vm.max_map_count` now accurately describes the graceful fallback behaviour introduced in 0.6.2 rather than implying hard failures.

## [0.6.2] - 2026-02-27

### Fixed
- **Files beyond the OS mmap limit were silently dropped from search results (P0 — Correctness)**: When the Linux `vm.max_map_count` safe limit (85% of system max, typically 55,700 on default kernels) was exceeded during indexing, `add_file()` bailed out and the remaining files were neither registered in the store nor added to the trigram index. On large codebases (e.g. 629k files) only the first ~55,700 files were searchable. Files are now always registered. For files that cannot be memory-mapped (limit exhausted), content is read via `std::fs::read()` at result-retrieval time and the result is cached, so there is no repeated I/O cost.
- **File count in UI showed mmap-capped number instead of true indexed count**: Because the above fix ensures all files are registered, `file_store.len()` — which drives the FILES counter in the web UI and `/api/status` — now reflects the true number of indexed files rather than the OS mmap limit.

## [0.6.1] - 2026-02-27

### Added
- **Checkpoint saves during initial indexing**: New `checkpoint_interval_files` config option under `[indexer]` (default: `0` = disabled). When set (e.g. `20000`), the index is written to `index_path` every N files during the initial build. If the process is killed before completion, the next run loads the checkpoint and re-indexes only the missing files — no compute is lost. Has no effect if `index_path` is not configured.

## [0.6.0] - 2026-02-27

### Added
- **Non-UTF-8 encoding support**: Files in legacy encodings (Latin-1/ISO-8859-1, Windows-1252, Shift-JIS, UTF-16 LE/BE, etc.) are now automatically detected and transcoded to UTF-8 during indexing, making them fully searchable. Zero performance impact on UTF-8 files (fast path via `std::str::from_utf8`).
  - New `transcode_non_utf8` config option under `[indexer]` (default: `true`) to disable transcoding if only UTF-8 codebases are indexed.
  - Transcoding uses `chardetng` (Mozilla's Firefox charset detector) + `encoding_rs` for accurate, battle-tested detection.
  - `files_transcoded` counter exposed in `/api/status`, `/api/diagnostics`, and gRPC `IndexResponse` for visibility.
  - Transcoded files emit an `INFO`-level log entry with the detected encoding name.
- **Filename-only matches now returned**: Searching for a filename that does not appear in the file's content previously returned zero results (the file was shortlisted by the trigram index but then silently dropped). All three search paths now synthesize a `SearchMatch` at line 0 when the query matches the filename stem but no content lines match.

### Fixed
- **Regex trigram lookup not lowercased (P0 — Correctness)**: `search_regex()` passed raw literal text (e.g. `"MyClass"`) to the trigram index which stores only lowercased content. Uppercase literals ≥3 chars returned zero trigram hits, causing missed results or a full scan fallback. The literal is now lowercased before the trigram lookup.
- **Exact-match score boost used case-insensitive comparison (P0 — Correctness)**: `calculate_score_inline()` applied the 2× exact-match boost using the already-lowercased query, so every match received the boost regardless of case. The boost now uses the original (un-lowered) query for a true case-sensitive comparison.
- **C++ template declarations produced duplicate symbols (P1 — Correctness)**: The `template_declaration` tree-sitter handler both pushed template children onto the stack and fell through to the generic push, causing inner nodes to be visited twice and symbols to appear twice in results.
- **Symbol search scanned all documents (P1 — Performance)**: `search_symbols()` called `trigram_index.all_documents()` regardless of query length, checking every indexed file. For queries ≥3 characters the trigram index is now used to narrow candidates first.
- **Line length penalty was too harsh (P1 — Quality)**: The previous `1.0 / (1.0 + len * 0.01)` factor dropped to 0.50 at 100 chars and 0.09 at 1000 chars, severely penalizing function signatures and long lines. Replaced with a gentler logarithmic curve `(1.0 / (1.0 + (len / 100.0).ln_1p())).max(0.3)`.
- **`FAST_RANKING_TOP_N` too low (P2 — Quality)**: Raised from 500 to 2000. The previous limit could cause relevant files with low base scores to be missed in large codebases.
- **Empty search results allocated unnecessary `Vec` (P2 — Performance)**: `search_in_document_scored()`, `search_in_document()`, and `search_in_document_regex()` returned `Some(Vec::new())` on no match. They now return `None`, eliminating spurious allocations.
- **Trigram bleed at filename/content boundary (P2 — Correctness)**: `format!("{}\n{}", filename_stem, content)` generated spurious trigrams spanning the boundary (e.g. the last chars of the filename joined with the first chars of the content). The separator is now three newlines (`"\n\n\n"`) so no meaningful trigram crosses the boundary.
- **Redundant trigram deduplication (P3 — Code quality)**: `TrigramIndex::search()` ran a three-step dedup (extract → `FxHashSet` filter → collect). Replaced with a single call to `extract_unique_trigrams()`.
- **Per-query filename stem allocation (P2 — Performance)**: `filename_matches_query()` re-computed `file_stem().to_lowercase()` on every file for every query. A pre-computed `lowercase_stem: String` field is now stored in `FileMetadata` at index time.
- **`FileName` symbol search returned wrong content**: `search_symbols_in_document()` fetched line 0 of the file content for `FileName` symbols, returning the first line of the file instead of the file path. It now uses the file path as the match content.

## [0.5.8] - 2026-02-26

### Fixed
- **Fix panic in `content_safety_check` on multi-byte UTF-8 content**: `is_binary_content()` sliced a `&str` at a raw byte offset (8192) which panics when that position falls inside a multi-byte UTF-8 character (e.g. CJK text, emoji). The function now operates on `as_bytes()` directly, avoiding the invalid slice.

## [0.5.7] - 2026-02-26

### Added
- **Automatic mmap limit detection**: On Linux the server now reads `vm.max_map_count` at startup, computes a safe 85 % ceiling, and refuses to map additional files once that ceiling is reached — preventing "cannot allocate memory" crashes on RHEL 7 / CentOS 7 and similar systems with low default limits.
- **Startup system-limits logging**: `SystemLimits::collect()` gathers `max_map_count`, current map count, open file descriptors, and `file-max`, then logs them at `INFO` level on every launch.
- **Startup warning for low limits**: When `vm.max_map_count < 131072` the server prints an actionable warning to stderr with both "with sudo" and "without sudo" remediation steps, then pauses 3 seconds before continuing.
- **`diagnose_mmap_error()` helper**: Mmap failures that look like resource-limit errors now produce a rich diagnostic message including current system limits and concrete fix instructions.
- **Deployment troubleshooting docs**: New "Memory Allocation Errors on RHEL7/CentOS7" section in `DEPLOYMENT.md` covering symptoms, root cause, automatic detection behaviour, and two solution paths (with/without sudo).

### Fixed
- **`include_extensions` config now honoured during indexing**: The field was parsed but never forwarded to file discovery; it now filters files correctly.
- **`max_file_size` config now honoured during indexing**: The configured value is forwarded to `FileDiscoveryConfig` instead of hardcoding 10 MB.
- **`watch = true` in config now starts the file watcher**: A background thread is spawned to detect and apply file-system changes.
- **`save_after_updates` now triggers periodic index saves**: The config field was previously ignored; it now saves every N files.
- **Import extraction extended to all supported variants**: `pyi`, `pyw`, `mjs`, `cjs`, `mts`, `cts` now handled.
- **Uniform `/health` status string**: Both keyword and semantic REST APIs now return `"healthy"`.
- **Config template corrected**: `index_path`, `save_after_build`, `save_after_updates`, `watch` placed under `[indexer]`; `telemetry.enabled` default fixed.

## [0.5.6] - 2026-02-26

### Fixed
- **Binary-detection tests use genuine binary bytes**: Test helpers in `file_store`, `lazy_file_store`, `engine`, and `integration_tests` now construct binary payloads from real non-UTF-8 byte sequences instead of synthetic strings, making the tests more robust and accurately representative of real-world binary files.

## [0.5.5] - 2026-02-26

### Changed
- Maintenance release: version/changelog update only; no functional code changes.

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

[Unreleased]: https://github.com/jburrow/fast_code_search/compare/v0.6.7...HEAD
[0.6.7]: https://github.com/jburrow/fast_code_search/compare/v0.6.6...v0.6.7
[0.6.6]: https://github.com/jburrow/fast_code_search/compare/v0.6.5...v0.6.6
[0.6.5]: https://github.com/jburrow/fast_code_search/compare/v0.6.4...v0.6.5
[0.6.4]: https://github.com/jburrow/fast_code_search/compare/v0.6.3...v0.6.4
[0.6.3]: https://github.com/jburrow/fast_code_search/compare/v0.6.2...v0.6.3
[0.6.2]: https://github.com/jburrow/fast_code_search/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/jburrow/fast_code_search/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/jburrow/fast_code_search/compare/v0.5.8...v0.6.0
[0.5.8]: https://github.com/jburrow/fast_code_search/compare/v0.5.7...v0.5.8
[0.5.7]: https://github.com/jburrow/fast_code_search/compare/v0.5.6...v0.5.7
[0.5.6]: https://github.com/jburrow/fast_code_search/compare/v0.5.5...v0.5.6
[0.5.5]: https://github.com/jburrow/fast_code_search/compare/v0.5.4...v0.5.5
[0.5.4]: https://github.com/jburrow/fast_code_search/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/jburrow/fast_code_search/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/jburrow/fast_code_search/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/jburrow/fast_code_search/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/jburrow/fast_code_search/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/jburrow/fast_code_search/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/jburrow/fast_code_search/compare/v0.2.4...v0.3.0
[0.2.4]: https://github.com/jburrow/fast_code_search/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/jburrow/fast_code_search/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/jburrow/fast_code_search/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/jburrow/fast_code_search/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/jburrow/fast_code_search/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jburrow/fast_code_search/releases/tag/v0.1.0
