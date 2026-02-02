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

- **OS**: Linux, macOS, or Windows with WSL2
- **Rust**: 1.70 or later
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
rustc --version   # Should be 1.70 or later
```

### Recommended Development Tools

```bash
# Install clippy and rustfmt
rustup component add clippy rustfmt

# Install cargo-watch for auto-rebuild on file changes
cargo install cargo-watch

# Install cargo-tree to visualize dependencies
cargo install cargo-tree
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
├── proto/                      # Protocol buffer definitions
│   └── search.proto           # gRPC service definition
├── src/
│   ├── lib.rs                 # Library entry point
│   ├── main.rs                # Server binary entry point
│   ├── index/                 # Indexing layer
│   │   ├── mod.rs
│   │   ├── trigram.rs         # Trigram extraction and bitmap index
│   │   └── file_store.rs      # Memory-mapped file storage
│   ├── search/                # Search engine
│   │   ├── mod.rs
│   │   └── engine.rs          # Parallel search with scoring
│   ├── symbols/               # Symbol extraction
│   │   ├── mod.rs
│   │   └── extractor.rs       # Tree-sitter integration
│   └── server/                # gRPC server
│       ├── mod.rs
│       └── service.rs         # Service implementation
├── examples/
│   └── client.rs              # Example gRPC client
├── build.rs                   # Build script for protobuf compilation
├── Cargo.toml                 # Dependencies and project metadata
└── README.md                  # User-facing documentation
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

1. **File Discovery**: `walkdir` traverses directory trees
2. **Memory Mapping**: `memmap2` creates memory-mapped views of files
3. **Trigram Extraction**: Text split into overlapping 3-character sequences
4. **Bitmap Indexing**: Roaring bitmaps store document sets for each trigram
5. **Symbol Parsing**: Tree-sitter extracts function/class definitions

### Search Pipeline

1. **Query Processing**: Query split into trigrams
2. **Candidate Selection**: Bitmap intersection finds matching documents
3. **Parallel Search**: Rayon distributes line-by-line search across threads
4. **Scoring**: Multi-factor scoring ranks results
5. **Result Streaming**: Top results streamed via gRPC

### Key Data Structures

#### Trigram Index
```rust
HashMap<Trigram, RoaringBitmap>
```
- Maps each trigram to a bitmap of document IDs
- Roaring bitmaps provide O(n) intersection where n = # of matching docs

#### File Store
```rust
Vec<MappedFile>
```
- Array-indexed storage for O(1) document lookup
- Memory-mapped files avoid loading entire files into RAM

#### Dependency Index
```rust
struct DependencyIndex {
    imports: HashMap<u32, HashSet<u32>>,      // file → files it imports
    imported_by: HashMap<u32, HashSet<u32>>,  // file → files that import it
    import_counts: HashMap<u32, u32>,         // cached dependent counts
}
```
- Bidirectional import graph built from tree-sitter extracted imports
- Enables PageRank-style scoring: heavily-imported files rank higher

### Scoring Algorithm

Search results are ranked using a multiplicative scoring formula:

```
score = base_score * case_boost * symbol_boost * path_boost * line_len_factor * position_boost * dependency_boost
```

| Factor | Condition | Multiplier |
|--------|-----------|------------|
| `case_boost` | Exact case-sensitive match | 2.0x |
| `symbol_boost` | Line contains a symbol definition | 3.0x |
| `path_boost` | File in `/src/` or `/lib/` directory | 1.5x |
| `line_len_factor` | Shorter lines preferred | `1.0 / (1.0 + len * 0.01)` |
| `position_boost` | Match at start of line | 1.5x |
| `dependency_boost` | File imported by N other files | `1.0 + log10(N) * 0.5` |

**Example**: A class definition in `src/models.py` imported by 100 files:
- symbol_boost: 3.0x
- path_boost: 1.5x  
- dependency_boost: `1.0 + log10(100) * 0.5 = 2.0x`
- Combined: **9.0x** base score (before other factors)

This ensures that **definitions rank above usages** and **core modules rank above consumers**.

### Threading Model

- **Indexing**: Single-threaded (I/O bound)
- **Search**: Parallel via rayon (CPU bound)
- **gRPC Server**: Tokio async runtime (I/O bound)

## Development Workflow

### Adding a New Language for Symbol Extraction

1. Add dependency to `Cargo.toml`:
```toml
tree-sitter-<language> = "0.20"
```

2. Update `src/symbols/extractor.rs`:
```rust
fn language_for_file(path: &Path) -> Option<Language> {
    match extension {
        "cpp" | "cc" => Some(tree_sitter_cpp::language()),
        // ... existing cases
    }
}
```

3. Add test:
```rust
#[test]
fn test_cpp_function_extraction() {
    // ...
}
```

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

Add logging to your code:

```rust
// In Cargo.toml dependencies
env_logger = "0.10"
log = "0.4"

// In code
use log::{info, debug, error};

debug!("Processing document {}", doc_id);
info!("Indexed {} files", count);
```

Run with logging:
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

Create benchmarks in `benches/` directory:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn trigram_benchmark(c: &mut Criterion) {
    c.bench_function("trigram extraction", |b| {
        b.iter(|| extract_trigrams(black_box("some text")))
    });
}

criterion_group!(benches, trigram_benchmark);
criterion_main!(benches);
```

Run benchmarks:
```bash
cargo bench
```

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
| Linux x86_64 | `fast_code_search-v{VERSION}-x86_64-unknown-linux-gnu.tar.gz` |
| Linux ARM64 | `fast_code_search-v{VERSION}-aarch64-unknown-linux-gnu.tar.gz` |
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
