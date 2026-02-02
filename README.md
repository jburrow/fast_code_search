# fast_code_search

High-performance, in-memory code search service built in Rust. Designed to handle 10GB+ of text with blazing fast search capabilities.

## Features

- **Trigram-based inverted index** using Roaring bitmaps for efficient storage and fast lookup
- **Memory-mapped files** using `memmap2` for optimal memory usage
- **Symbol awareness** using `tree-sitter` for Rust, Python, JavaScript, and TypeScript
- **Parallel search** using `rayon` for maximum throughput
- **Weight-based scoring** that boosts:
  - Symbol definitions (3.0x)
  - Primary source directories (src/, lib/) (1.5x)
  - Exact case-sensitive matches (2.0x)
  - Matches at the start of lines (1.5x)
  - Shorter lines (inverse length factor)
  - **Heavily-imported files** — logarithmic boost based on dependent count (PageRank-style)
- **gRPC API** using `tonic` with streaming results
- **Supports 10GB+ codebases** efficiently

## Why an In-Memory Server?

Unlike command-line tools (ripgrep) or disk-based indexes (Zoekt), fast_code_search runs as an **always-on, in-memory server**. This architecture provides unique advantages:

| Advantage | Impact |
|-----------|--------|
| **Zero cold-start** | Index always hot in RAM — no disk I/O on first query |
| **Sub-millisecond search** | Memory access is 100-1000x faster than SSD |
| **Warm CPU caches** | Repeated queries hit L1/L2 cache |
| **Live dependency graph** | Track imports across the entire codebase |
| **Concurrent access** | RwLock allows many simultaneous searches |
| **Real-time streaming** | gRPC streams results to IDEs as they're found |

### Best For

- **IDE integration** — Sub-10ms latency enables "search as you type"
- **Repeated queries** — Same patterns searched throughout a coding session
- **Large codebases** — 10GB+ where per-query scanning takes seconds
- **Dependency queries** — "What files import this module?"
- **Team search** — Multiple developers querying the same codebase

### When to Use Other Tools

- **One-off searches**: ripgrep is instant for single searches, no server needed
- **Disk-constrained**: Zoekt's persistent index survives restarts without rebuild
- **Planet-scale**: GitHub Code Search scales horizontally across data centers

📖 **See [PRIOR_ART.md](PRIOR_ART.md) for a detailed comparison with ripgrep, Zoekt, GitHub Code Search, and improvement roadmap.**

## Architecture

### Core Components

1. **Trigram Index** (`src/index/trigram.rs`)
   - Extracts 3-character sequences from text
   - Uses Roaring bitmaps to efficiently store document sets
   - Performs fast intersection queries

2. **File Store** (`src/index/file_store.rs`)
   - Memory-maps files for efficient access
   - Supports large codebases without loading everything into RAM

3. **Symbol Extractor** (`src/symbols/extractor.rs`)
   - Uses tree-sitter parsers for multiple languages
   - Identifies function/class definitions
   - Enhances search results with semantic information

4. **Search Engine** (`src/search/engine.rs`)
   - Parallel search using rayon
   - Sophisticated scoring algorithm
   - Returns ranked results

5. **gRPC Server** (`src/server/service.rs`)
   - Streaming search results
   - Index management
   - Remote access via gRPC

## Building

### Prerequisites

- Rust 1.70 or later
- Protocol Buffers compiler (`protoc`)

On Debian/Ubuntu:
```bash
sudo apt-get install protobuf-compiler
```

### Build

```bash
cargo build --release
```

### Run Tests

```bash
cargo test
```

## Usage

### Starting the Server

```bash
cargo run --release --bin fast_code_search_server
```

The server will start on `0.0.0.0:50051`.

### Example Client

See `examples/client.rs` for a complete example. Run it with:

```bash
cargo run --example client
```

### gRPC API

The service provides two main operations:

#### Index

Index files from one or more directories:

```proto
message IndexRequest {
  repeated string paths = 1;
}

message IndexResponse {
  int32 files_indexed = 1;
  int64 total_size = 2;
  string message = 3;
}
```

#### Search

Search for a query and stream results:

```proto
message SearchRequest {
  string query = 1;
  int32 max_results = 2;
  repeated string include_paths = 3;  // Glob patterns for paths to include
  repeated string exclude_paths = 4;  // Glob patterns for paths to exclude
  bool is_regex = 5;                  // Treat query as regex pattern
  bool symbols_only = 6;              // Search only in discovered symbols
}

message SearchResult {
  string file_path = 1;
  string content = 2;
  int32 line_number = 3;
  double score = 4;
  MatchType match_type = 5;
}
```

### Search Modes

The search engine supports multiple modes:

| Mode | Flag | Description |
|------|------|-------------|
| **Text Search** | (default) | Full-text search across all file contents |
| **Regex Search** | `is_regex=true` | Regular expression pattern matching |
| **Symbols-Only** | `symbols_only=true` | Search only in function/class names |

**Symbols-only search** is ideal when you're looking for definitions rather than usages. It searches the symbol cache (extracted via tree-sitter) and returns only matches where the query appears in a symbol name. This is significantly faster than full-text search when you know you're looking for a function or class.

## Performance Characteristics

- **Indexing**: Parallel file processing, ~100MB/s on modern hardware
- **Search**: Sub-millisecond for most queries on 10GB+ codebases
- **Memory**: Uses memory mapping, so actual RAM usage is much lower than codebase size
- **Scalability**: Handles 10GB+ of text efficiently

## Benchmarks

Benchmarks run on synthetic corpus using Criterion. Run locally with `cargo bench`.

| Benchmark | Corpus Size | Time | Throughput |
|-----------|-------------|------|------------|
| text_search/common_query | 100 files | 1.4 ms | 7.0 Melem/s |
| text_search/common_query | 500 files | 8.5 ms | 5.9 Melem/s |
| text_search/common_query | 1000 files | 22 ms | 4.5 Melem/s |
| text_search/rare_query | 500 files | 0.3 ms | - |
| text_search/no_match | 500 files | 0.1 ms | - |
| regex_search/simple_literal | 500 files | 9 ms | - |
| regex_search/no_literal | 500 files | 45 ms | - |

*Last updated: v0.2.0*

## How It Works

1. **Indexing Phase**:
   - Files are memory-mapped using `memmap2`
   - Text is split into trigrams (3-character sequences)
   - Each trigram is mapped to document IDs using Roaring bitmaps
   - Symbols are extracted using tree-sitter parsers

2. **Search Phase**:
   - Query is split into trigrams
   - Roaring bitmap intersection finds candidate documents
   - Parallel search across candidates using `rayon`
   - Results are scored based on:
     - Exact case-sensitive match: **2.0x**
     - Symbol definitions: **3.0x**
     - Primary source directories (src/, lib/): **1.5x**
     - Shorter lines: `1.0 / (1.0 + line_len * 0.01)`
     - Matches at start of line: **1.5x**
     - Dependency boost: `1.0 + log10(import_count) * 0.5` — files imported by many others rank higher (PageRank-style)

3. **Result Streaming**:
   - Top results are streamed via gRPC
   - Allows incremental display of results
   - Efficient for large result sets

## Supported Languages

Tree-sitter symbol extraction is supported for:
- Rust (`.rs`)
- Python (`.py`)
- JavaScript (`.js`, `.jsx`)
- TypeScript (`.ts`, `.tsx`)

Other file types are still searchable, just without symbol-awareness.

## License

See LICENSE file.

