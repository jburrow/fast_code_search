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
│   │   ├── query_syntax.rs     # literal text + file:/lang:/case:/word: parsing
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
│   ├── cli/                    # the `fcs` command-line client (docs/CLI.md)
│   └── bin/                    # fcs, fast_code_search_semantic (feature), fast_code_search_validator
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
5. **Checkpoints and final save**: atomic temp+fsync+rename, format v7
   (`src/index/persistence.rs`): a 36-byte header (magic `FCSIDX05`, version,
   CRC-32, section lengths), then a bincode metadata section (file table,
   symbols, edges, parked imports, reference names and per-file references),
   a fixed-width trigram directory sorted by trigram, and the roaring bitmaps
   back to back. Save streams bitmaps from the live map; load memory-maps the
   file, checks bounds and checksum, and deserializes bitmaps in parallel from
   the mapping. `tests/fixtures/index-v8.fcsidx` pins the format byte for
   byte; regenerate it with `FCS_WRITE_GOLDEN=1 cargo test golden` after a
   deliberate format change (and bump the version).

### Search Pipeline

1. **Parse** (`query_syntax.rs`): the literal text, `file:`/`lang:` globs, `case:`/`word:`.
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

### Changing the web UI

The pages in `static/` are embedded in the server binary, so rebuild the
server to see a change.

- **Header and navigation:** `static/shell.js` draws the header, the icon
  sprite and the phone tab bar on every page. A page passes its own header
  extras (stats, the compact search box) in `<template id="app-header-extra">`.
- **Colours, shadows and fonts** are design tokens (`var(--accent)`,
  `var(--shadow)`, …) defined once in `static/app.css`. Never write a colour
  anywhere else: `static/tests/tokens.test.js` fails on a hex or `rgb()`
  value outside `app.css`, and on a `var(--…)` that isn't defined.
- **Themes:** the `:root, [data-theme="bold"]` block in `app.css` is the
  default theme; `[data-theme="corporate"]` and `[data-theme="corporate-dark"]`
  redefine every token (colours, radius, fonts, label case) so a preview of
  one theme renders correctly inside a page in another. `static/theme.js`
  applies the saved theme before the page paints and draws the header menu
  and first-run picker. Adding a theme means a block in `app.css`, an entry
  in `theme.js` and a `UiTheme` variant in `src/config.rs`; a test checks
  all three agree and that every block sets every token.
- **Labels and buttons:** write labels in sentence case ("Match case") and
  give them the `label-case` class (or `text-transform: var(--label-case)`):
  Bold shows them in capitals, the corporate themes as written. Never put a
  file name, path or query inside one. Buttons use `.btn` / `.btn-primary`.
  The token test fails on a literal `uppercase` or a hard-coded font name.
- **Tailwind** colour names in `tailwind.config.js` point at the same tokens.
  After changing classes or the config, run `npm run build:css` and commit
  `static/tailwind.css` (CI checks it is up to date).
- **Code:** `static/code-view.js` is the one code viewer (the file viewer,
  the hover preview and the dependency explorer's source pane): line
  numbers, highlighting, a window of lines with "Show more", and the ruler
  of hits beside the scrollbar. Its pure parts are in
  `static/lib/keyword-helpers.js` with tests.
- Run the JavaScript tests with `node --test static/tests/*.test.js`.

### Performance Optimization Checklist

- [ ] Profile with `cargo flamegraph`
- [ ] Check allocations with `valgrind --tool=massif`
- [ ] Benchmark with `cargo bench` (if benchmarks exist)
- [ ] Test with large codebases (10GB+)
- [ ] Measure memory usage under load

### Re-recording the demos

The animated demos in the README and the documentation
(`docs/images/demos/*.gif`) and the `docs/images/web-ui.png` screenshot are
recorded from a server indexing a clean export of this repository, so they
only ever show this project's own code. After a UI change, re-record them:

```bash
NODE_PATH=$(npm root -g) scripts/docs/record-demos.sh          # all of them
NODE_PATH=$(npm root -g) scripts/docs/record-demos.sh graph    # one: search, references, regex, graph, cli or stills
```

It needs `ffmpeg`, Node with the `playwright` package and a Chromium it can
launch. Each demo is a short scripted session in
`scripts/docs/record-demos.mjs`; keep the GIFs under about 3 MB each.

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
`persistence_benchmark.rs`) for local use. They use a small synthetic corpus,
so they measure fixed overhead well and large-corpus behaviour poorly
(roadmap 6.5). CI no longer runs them: on every push to `main` the Benchmarks
workflow counts instructions per operation instead (`scripts/bench/icount.sh`,
needs valgrind) and fails on 3% more than the last passing push. If a change
adds instructions on purpose, say so in the squash commit message with a line
`Accept-instructions: <operation>, ...` (or `all`); see the book's
benchmark methodology page.

```bash
cargo bench --bench search_benchmark -- indexing
```

Use `std::hint::black_box`, not `criterion::black_box`.

### Same-machine A/B before merging

Numbers from different machines, or from GitHub runners on different days,
vary too much to compare. Before merging a change to indexing, search,
persistence or the watcher, compare it with `main` on one machine:

```bash
scripts/bench/ab.sh                      # base origin/main, 5 rounds
scripts/bench/ab.sh --base v0.18.0 --rounds 5
scripts/bench/ab.sh --corpus small,real --rounds 3   # also the 302k-file corpus
```

The script builds `examples/corpus_bench` at the base (in a worktree under
`target/ab/`, cached per commit) and at the working tree, fetches tokio 1.45.0
and Django 5.2 into `bench-corpus/` if they are missing, runs the two
alternately and prints a markdown table of medians for the PR's
**Performance** section. It exits 1 when a metric's median is more than
`--threshold` percent (default 10) slower and every round of the branch was
slower than every round of the base. Both conditions matter: run against
itself on a 4-core VM, main moved single metrics by up to 18% over 5 rounds,
but never with all rounds on one side. A slowdown can be the right trade; the
PR and the CHANGELOG entry then say what it buys.

`--corpus real` adds a pinned corpus the size of a large monorepo: Linux
v6.11, LLVM 19.1.0, Rust 1.82.0 and Kubernetes v1.31.0, about 302,000
files and 3 GB (about 4 GB of disk once cloned). Costs that grow with the
tree, such as the batch merge, a load or memory, show there and
not in tokio + Django. A build takes about 7 minutes, so a round takes about
17 minutes on 4 cores. Use it for changes to indexing, persistence or memory,
and say in the PR how many rounds the table had.

To find where time goes, profile the same benchmark with callgrind and
collect every thread (a toggle on the main thread misses rayon's workers):

```bash
cargo build --release --example corpus_bench
valgrind --tool=callgrind --callgrind-out-file=cg.out \
  target/release/examples/corpus_bench bench-corpus/tokio bench-corpus/django --iterations 3
callgrind_annotate --inclusive=yes cg.out | head -60
```

## Docs that repeat numbers

Some facts are stated in more than one place. When one changes, update
every place in the same PR, and say in the PR which you updated:

| Fact | Where it appears |
|---|---|
| Benchmark numbers (build, memory, query p50/p95, Criterion) | `README.md` (Benchmarks), `docs/book/src/benchmarks/latest.md` (Last hand-checked run), `docs/book/src/benchmarks/comparison.md` (Results), `docs/site/index.html` (bench-stats) |
| Number of backtest queries | `README.md`, `docs/book/src/benchmarks/methodology.md` (count `[[query]]` in `benches/rg_backtest.toml`) |
| Search limits and defaults | `docs/book/src/benchmarks/search-limits.md`, `docs/book/src/reference/config-template.md`, the server's `--init` template |
| Divergences from grep, ripgrep and git grep | `docs/book/src/reference/matching.md` |
| Released version | `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `docs/book/src/start/install.md` |
| Ports, CLI flags, config keys | `README.md`, `docs/CLI.md`, the book's reference pages |

Numbers always name the Benchmarks run and commit they come from, so a
reader can tell how old they are.

## Release Process

Releases are automated via GitHub Actions. The maintainer cuts every
release: a release PR bumps the version, then the maintainer pushes the
version tag, and the tag builds and publishes everything. Releases are
immutable, so a mistake means a new patch version, not an edit.

### Before you start

- `main` is green: no open issue labelled `main-red` and the latest CI,
  Benchmarks and Pages runs on `main` passed. A red workflow is fixed or
  explained in the release PR first.
- The published benchmark numbers are current. The README, the book's
  [latest results](book/src/benchmarks/latest.md) and
  [comparison](book/src/benchmarks/comparison.md) pages and the project site
  (`docs/site/index.html`) repeat numbers from one named Benchmarks run. If
  that run predates a change that moved them (any metric the A/B marked
  slower or faster), refresh all four from the latest run on `main` in the
  release PR. See [Docs that repeat numbers](#docs-that-repeat-numbers).

### Steps to Release

1. **Open a release PR** from a branch off `main` that changes only:
   - `Cargo.toml`: `version = "X.Y.Z"`.
   - `Cargo.lock`: the `fast_code_search` package's version line (`cargo check` rewrites
     it; no other entry should change).
   - `CHANGELOG.md`: move `[Unreleased]` items into a dated `## [X.Y.Z] -
     YYYY-MM-DD` section with a short summary and an **Upgrading** note,
     leave an empty `[Unreleased]`, and update the compare links at the
     bottom (`[Unreleased]` from `vX.Y.Z...HEAD`, plus a new `[X.Y.Z]` line).
     Any slowdown the A/B found since the last release is named here with
     what it buys.
   - `docs/book/src/start/install.md`: the version in the checksum and
     cosign examples and the container tags.

   Fill in the [PR template](../.github/PULL_REQUEST_TEMPLATE.md) and title
   it `chore: release vX.Y.Z`.

2. **Run quality checks** locally before pushing:
   ```bash
   cargo fmt --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test
   ```

3. **Merge** (squash) once CI is green. The Windows semantic test
   `test_add_and_search` is a known flake; it is the only check that may be
   red at merge, and only if it is the only one.

4. **Tag the merge commit and push the tag** (maintainer; tokens used by
   automation cannot push tags):
   ```bash
   git fetch origin
   git tag vX.Y.Z <merge-commit-sha>
   git push origin vX.Y.Z
   ```

5. **Check the release is healthy.** The tag starts the
   [release workflow](../.github/workflows/release.yml) and the
   [container workflow](../.github/workflows/container.yml). When both pass:
   - The GitHub Release has 15 assets: 5 archives, each with a `.sha256`
     and a `.sigstore.json`.
   - One archive verifies and reports the right version:
     ```bash
     sha256sum -c fast_code_search-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz.sha256
     ./fcs --version                     # fcs X.Y.Z
     ./fast_code_search_server --version # fast_code_search X.Y.Z
     ```
   - A smoke run indexes a public tree (for example tokio), `GET
     /api/check` reports no problems, and a plain search returns the same
     count as `rg -F` on that tree.
   - `ghcr.io/jburrow/fast_code_search` has `X.Y.Z`, `X.Y` and (for a
     non-pre-release) `latest`.

### Release Artifacts

Each release includes platform-specific archives, each with a `.sha256`
checksum and a Sigstore bundle (`.sigstore.json`):

| Platform | Archive |
|----------|---------|
| Linux x86_64 (glibc) | `fast_code_search-v{VERSION}-x86_64-unknown-linux-gnu.tar.gz` |
| Linux x86_64 (musl, static) | `fast_code_search-v{VERSION}-x86_64-unknown-linux-musl.tar.gz` |
| macOS x86_64 | `fast_code_search-v{VERSION}-x86_64-apple-darwin.tar.gz` |
| macOS ARM64 | `fast_code_search-v{VERSION}-aarch64-apple-darwin.tar.gz` |
| Windows x86_64 | `fast_code_search-v{VERSION}-x86_64-pc-windows-msvc.zip` |

Each archive contains:
- `fast_code_search_server` and `fcs` (or `.exe` on Windows)
- `config.toml.example`
- `deploy/` (startup unit files), `RUN-AT-STARTUP.md`, `CLI.md`
- `proto/`
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

- Read the [Contributing Guide](../CONTRIBUTING.md)
- Check existing GitHub issues
- Review the [README](../README.md) for architecture overview
- Ask questions in pull requests
