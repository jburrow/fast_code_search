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

1. Update version in `Cargo.toml`
2. Update CHANGELOG.md
3. Run full test suite: `cargo test`
4. Run clippy: `cargo clippy -- -D warnings`
5. Build release binary: `cargo build --release`
6. Test release binary manually
7. Create git tag: `git tag v0.2.0`
8. Push tag: `git push origin v0.2.0`

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
