# Development Guide

This guide provides detailed information for developers working on fast_code_search.

## Table of Contents

- [Development Environment Setup](#development-environment-setup)
- [Project Structure](#project-structure)
- [Building and Testing](#building-and-testing)
- [Architecture Deep Dive](#architecture-deep-dive)
- [Development Workflow](#development-workflow)
- [Debugging](#debugging)
- [Performance Profiling](#performance-profiling)

## Development Environment Setup

### System Requirements

- **OS**: Linux, macOS, or Windows (native or WSL2)
- **Rust**: 1.89 or later (see `rust-toolchain.toml`)
- **Memory**: 4GB minimum, 8GB recommended
- **Disk**: 2GB for dependencies and build artifacts

### Required Tools

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install protobuf compiler
# Ubuntu/Debian
sudo apt-get update
sudo apt-get install protobuf-compiler

# macOS
brew install protobuf

# Verify installation
protoc --version  # Should be 3.x or later
rustc --version   # Should be 1.89 or later
```

### Recommended Development Tools

```bash
# Install clippy and rustfmt
rustup component add clippy rustfmt

# Install cargo-watch for auto-rebuild on file changes
cargo install cargo-watch

# cargo-deny checks advisories/licenses (CI runs it too)
cargo install cargo-deny
```

### IDE Setup

#### VS Code
Install these extensions:
- `rust-analyzer`: Rust language support
- `CodeLLDB`: Debugging support
- `Even Better TOML`: TOML file support

#### IntelliJ IDEA / CLion
- Install the Rust plugin from JetBrains Marketplace

## Project Structure

```
fast_code_search/
├── proto/
│   ├── search.proto            # keyword gRPC service (Search, Index) + grpc.health.v1
│   └── semantic_search.proto   # semantic engine service (feature "semantic")
├── src/
│   ├── lib.rs                  # module tree; semantic* modules are cfg(feature = "semantic")
│   ├── main.rs                 # keyword server binary: config, servers, indexer, watcher, shutdown
│   ├── config.rs               # TOML config (deny_unknown_fields), validation, CLI overrides
│   ├── telemetry.rs            # tracing subscriber + optional OTLP export
│   ├── index/
│   │   ├── trigram.rs          # trigram extraction (bitset dedupe, per-byte fold) + Roaring index
│   │   ├── lazy_file_store.rs  # file table: small files by owned reads, large files mmapped
│   │   └── persistence.rs      # atomic save/load (magic + version), staleness checks
│   ├── search/
│   │   ├── engine/
│   │   │   ├── mod.rs          # SearchEngine: indexing, incremental update, import resolution
│   │   │   ├── query.rs        # run_candidates (budget/deadline/paging), per-document scans
│   │   │   ├── text.rs         # matching/scoring helpers (line hits, word bounds, truncation)
│   │   │   ├── persist.rs      # save_index, load_index*, reconciliation, symbol rebuild
│   │   │   ├── progress.rs     # IndexingProgress / status types, broadcaster
│   │   │   └── tests.rs
│   │   ├── background_indexer.rs # two-phase batch build, checkpoints, shutdown flag
│   │   ├── incremental.rs      # apply watcher changes (files and directories) to the engine
│   │   ├── watcher.rs          # notify-based file watcher
│   │   ├── file_discovery.rs   # walk (walkdir / ignore crate), eligibility rules
│   │   ├── path_filter.rs      # include/exclude globs
│   │   ├── query_syntax.rs     # file:/lang:/-term/case:/word: parsing
│   │   ├── ranking.rs          # RankingWeights / FileScoreWeights
│   │   └── regex_search.rs     # regex -> trigram constraints
│   ├── symbols/
│   │   ├── extractor.rs        # tree-sitter: tags.scm queries + supplementary walker, imports
│   │   └── queries/            # vendored tags queries (C#)
│   ├── dependencies/mod.rs     # import graph, per-language import resolution
│   ├── server/service.rs       # gRPC CodeSearch service
│   ├── web/                    # axum REST API, web UI assets, metrics
│   ├── diagnostics/            # /api/diagnostics types and self-tests
│   ├── utils.rs                # transcoding, binary detection, system limits
│   └── bin/                    # fast_code_search_semantic (feature), fast_code_search_validator
├── static/                     # embedded web UI (Tailwind build in tailwind.css)
├── tests/integration_tests.rs  # end-to-end gRPC + HTTP tests on ephemeral ports
├── benches/                    # criterion benchmarks (search, persistence)
├── docs/                       # this guide, deployment, design notes, plans, archive
├── build.rs                    # protobuf compilation
└── Cargo.toml
```

## Building and Testing

### Build Commands

```bash
# Debug build (faster compilation, slower runtime)
cargo build

# Release build (slower compilation, optimized runtime)
cargo build --release

# Build specific binary
cargo build --bin fast_code_search_server

# Build with verbose output
cargo build -v
```

### Testing

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_trigram_extraction

# Run tests in a specific module
cargo test index::trigram

# Run tests with multiple threads
cargo test -- --test-threads=4
```

### Continuous Development

```bash
# Auto-rebuild on file changes
cargo watch -x build

# Auto-test on file changes
cargo watch -x test

# Auto-run server on file changes
cargo watch -x 'run --bin fast_code_search_server'
```

### Code Quality

```bash
# Format code
cargo fmt

# Check formatting without modifying files
cargo fmt -- --check

# Run clippy linter
cargo clippy

# Run clippy with strict warnings
cargo clippy -- -D warnings

# Check for unused dependencies
cargo +nightly udeps
```

## Architecture Deep Dive

### Indexing Pipeline

1. **Discovery** (`file_discovery.rs`): a `walkdir` or, with `respect_gitignore`,
   an `ignore` walk applies exclude patterns, include extensions, binary and size
   rules. The same rules (`is_eligible`) gate watcher events.
2. **Phase 1, parallel (rayon)**: `PartialIndexedFile::process` reads each file
   into an owned buffer (never through a live mmap), records its mtime/size,
   transcodes non-UTF-8, runs the structural safety check (which only disables
   symbol extraction) and extracts trigrams.
3. **Phase 2, parallel**: `PreIndexedFile::from_partial` runs tree-sitter (tags
   query + walker) under `catch_unwind`.
4. **Merge, under the engine write lock**: `index_batch` registers files,
   inserts postings, stores symbols, queues imports; unresolved imports are
   parked and retried only when a file with a matching name appears.
5. **Checkpoints and final save**: atomic temp+fsync+rename, format v5.

### Search Pipeline

1. **Parse** (`query_syntax.rs`): terms, `-term`, `file:`/`lang:` globs, `case:`/`word:`.
2. **Candidates**: intersection of each term's trigram bitmaps (index is lowercase),
   then the path filter (precomputed display paths, no allocation).
3. **`run_candidates`** (`engine/query.rs`): fast vs full mode, fast-mode ordering by
   file metadata, parallel per-document scans under a `QueryRun` (match budget +
   deadline), deterministic sort (score, file id, line) and offset paging.
4. **Per document** (`engine/text.rs`): whole-buffer scan (memmem / memchr2 /
   Unicode fold) resolving line bounds only at hits, lazy symbol maps, scoring.
5. **Serve**: REST (`/api/search`) and gRPC report ranking info, totals and
   truncation; every result carries full-line offsets and a character column.

### Key Data Structures

- `TrigramIndex`: `FxHashMap<Trigram, RoaringBitmap>` (run-optimised in `finalize`),
  plus an all-documents cache kept warm across incremental updates.
- `LazyFileStore`: `Vec<LazyMappedFile>` addressed by file id; files ≤ 1 MiB are
  read into owned buffers per access, larger ones are memory-mapped; removed ids
  are tombstoned so ids stay stable.
- `DependencyIndex`: bidirectional import graph with cached dependent counts and
  per-language resolution (Rust module paths, Python packages, JS/TS extensions
  and `index.*`).
- `FileMetadata`: per-file fast-ranking score, lowercase stem, display path.

### Scoring

All weights live in `src/search/ranking.rs` (`RankingWeights`, `FileScoreWeights`).
Line-level score:

```
score = exact_case(2.0) * symbol_definition(3.0) * src_lib_dir(1.5)
      * line_start(1.5) * line_length_factor * dependency_boost
line_length_factor = max(1 / (1 + ln(1 + len/100)), 0.3)
dependency_boost   = 1 + log10(dependents) * 0.5
```

File-level (fast-mode ordering only, never in a result score): additive
`base + src/lib + extension + log2(symbols) + log2(dependents)`, ×0.7 for
test/example paths, ×5 when the query matches the file stem. Symbol search adds
exact (2.0) > prefix (1.5) > substring and a small penalty for variables.

### Threading Model

- **Indexing**: discovery on its own thread; phases 1–2 on the global rayon pool
  (8 MB stacks for tree-sitter, built in `main`); merges under the engine
  `RwLock` write lock.
- **Search**: REST/gRPC handlers `spawn_blocking`, take `try_read` (503 +
  `Retry-After` while a writer holds the lock), and scan candidates with rayon.
- **Watcher**: one thread (8 MB stack), gathers events for 200 ms and applies
  them under one write lock via `incremental::apply_changes`.
- **Servers**: tokio; graceful shutdown on SIGINT/SIGTERM saves the index.

## Development Workflow

### Adding a New Language for Symbol Extraction

1. Add the grammar crate to `Cargo.toml` (e.g. `tree-sitter-kotlin = "0.x"`).
2. Map its extensions in `SymbolExtractor::language_for_extension`
   (`src/symbols/extractor.rs`) and, if the crate exports a `TAGS_QUERY`, add it to
   `tags_query_for`. If it does not, vendor the grammar's `queries/tags.scm` under
   `src/symbols/queries/` (see the C# entry) or rely on the supplementary walker
   (`visit_definition_node`) for its node kinds.
3. Add the extension to `lang_globs` in `src/search/query_syntax.rs`.
4. Add a test in `extractor.rs` that asserts names **and** line numbers, and an
   import-resolution rule in `src/dependencies/mod.rs` if the language has imports.

### Modifying the gRPC API

1. Update `proto/search.proto`
2. Build project to regenerate bindings:
```bash
cargo build
```
3. Update `src/server/service.rs` to implement new endpoints
4. Update example client in `examples/client.rs`

### Performance Optimization Checklist

- [ ] Profile with `cargo flamegraph`
- [ ] Check allocations with `valgrind --tool=massif`
- [ ] Benchmark with `cargo bench` (if benchmarks exist)
- [ ] Test with large codebases (10GB+)
- [ ] Measure memory usage under load

## Debugging

### Debug Build

```bash
# Build with debug symbols
cargo build

# Run with debugging
rust-gdb target/debug/fast_code_search_server
# or
rust-lldb target/debug/fast_code_search_server
```

### Logging

The crate uses `tracing` (not `log`/`env_logger`):

```rust
use tracing::{debug, info, warn};

debug!(doc_id, "Processing document");
info!(count, "Indexed files");
```

`RUST_LOG` controls the filter and takes precedence over `--verbose`; the server
warns when both are set. Optional OTLP export is configured in `[telemetry]`
(`OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_SERVICE_NAME`; `OTEL_SDK_DISABLED=true` is final).

```bash
RUST_LOG=debug cargo run --bin fast_code_search_server
```

### Common Issues

**Issue**: Protobuf compilation fails
```bash
# Solution: Install protoc
sudo apt-get install protobuf-compiler
```

**Issue**: Tree-sitter linking errors
```bash
# Solution: Clean and rebuild
cargo clean
cargo build
```

**Issue**: Out of memory during indexing
```bash
# Solution: Process files in batches or increase system memory
```

## Performance Profiling

### CPU Profiling

```bash
# Install flamegraph
cargo install flamegraph

# Profile the server
cargo flamegraph --bin fast_code_search_server

# View generated flamegraph.svg in browser
```

### Memory Profiling

```bash
# Use Valgrind
valgrind --tool=massif target/debug/fast_code_search_server

# Analyze results
ms_print massif.out.<pid>
```

### Benchmarking

Criterion benchmarks live in `benches/` (`search_benchmark.rs`,
`persistence_benchmark.rs`) and run in CI on every push to `main`, with trends
published to GitHub Pages. They use a small synthetic corpus, so they measure
fixed overhead well and large-corpus behaviour poorly (roadmap 6.5).

```bash
cargo bench --bench search_benchmark -- indexing
```

Use `std::hint::black_box`, not `criterion::black_box`.

## Release Process

Releases are automated via GitHub Actions. When you push a version tag, the release workflow builds binaries for all platforms and creates a GitHub Release.

### Steps to Release

1. **Update version** in `Cargo.toml`:
   ```toml
   version = "0.2.0"
   ```

2. **Update CHANGELOG.md**:
   - Move items from `[Unreleased]` to new version section
   - Add release date
   - Update comparison links at bottom of file

3. **Run quality checks**:
   ```bash
   cargo test
   cargo clippy -- -D warnings
   cargo fmt --check
   ```

4. **Commit the version bump**:
   ```bash
   git add Cargo.toml CHANGELOG.md
   git commit -m "chore: release v0.2.0"
   ```

5. **Create and push the tag**:
   ```bash
   git tag v0.2.0
   git push origin main
   git push origin v0.2.0
   ```

6. **Monitor the release workflow**: The [release workflow](/.github/workflows/release.yml) will:
   - Build release binaries for Linux (x86_64, ARM64), macOS (x86_64, ARM64), and Windows (x86_64)
   - Package each with config template, proto files, README, and LICENSE
   - Generate SHA256 checksums
   - Create a GitHub Release with changelog notes

### Release Artifacts

Each release includes platform-specific archives:

| Platform | Archive |
|----------|---------|
| Linux x86_64 (glibc) | `fast_code_search-v{VERSION}-x86_64-unknown-linux-gnu.tar.gz` |
| Linux x86_64 (musl, static) | `fast_code_search-v{VERSION}-x86_64-unknown-linux-musl.tar.gz` |
| macOS x86_64 | `fast_code_search-v{VERSION}-x86_64-apple-darwin.tar.gz` |
| macOS ARM64 | `fast_code_search-v{VERSION}-aarch64-apple-darwin.tar.gz` |
| Windows x86_64 | `fast_code_search-v{VERSION}-x86_64-pc-windows-msvc.zip` |

Each archive contains:
- `fast_code_search_server` (or `.exe` on Windows)
- `config.toml.example`
- `proto/search.proto`
- `README.md`
- `LICENSE`

### Pre-release Versions

For pre-release versions (alpha, beta, rc), use a hyphen in the version:
```bash
git tag v0.2.0-beta.1
```

These are automatically marked as pre-releases on GitHub.

## Useful Commands Reference

```bash
# View dependency tree
cargo tree

# Check for outdated dependencies
cargo outdated

# Update dependencies
cargo update

# Generate documentation
cargo doc --open

# Check compilation without building
cargo check

# Clean build artifacts
cargo clean

# Show what cargo would compile
cargo build --dry-run
```

## Getting Help

- Read the [Contributing Guide](CONTRIBUTING.md)
- Check existing GitHub issues
- Review the [README](README.md) for architecture overview
- Ask questions in pull requests
