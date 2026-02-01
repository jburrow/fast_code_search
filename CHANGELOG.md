# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
- Extracts function definitions, class declarations, method signatures
- Import/dependency tracking for enhanced relevance

#### Intelligent Scoring System
- **Symbol definitions**: 3x boost for function/class definitions
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
