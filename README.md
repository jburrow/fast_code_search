# fast_code_search

High-performance, in-memory code search service built in Rust. Designed to handle 10GB+ of text with blazing fast search capabilities.

## Features

- **Trigram-based inverted index** using Roaring bitmaps for efficient storage and fast lookup
- **Memory-mapped files** using `memmap2` for optimal memory usage
- **Symbol awareness** using `tree-sitter` for Rust, Python, JavaScript, and TypeScript
- **Parallel search** using `rayon` for maximum throughput
- **Weight-based scoring** that boosts:
  - Symbol definitions
  - Primary source directories (src/, lib/)
  - Exact matches
  - Matches at the start of lines
- **gRPC API** using `tonic` with streaming results
- **Supports 10GB+ codebases** efficiently

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
}

message SearchResult {
  string file_path = 1;
  string content = 2;
  int32 line_number = 3;
  double score = 4;
  MatchType match_type = 5;
}
```

## Performance Characteristics

- **Indexing**: Parallel file processing, ~100MB/s on modern hardware
- **Search**: Sub-millisecond for most queries on 10GB+ codebases
- **Memory**: Uses memory mapping, so actual RAM usage is much lower than codebase size
- **Scalability**: Handles 10GB+ of text efficiently

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
     - Exact vs. case-insensitive match
     - Symbol definitions get 3x boost
     - Primary source directories get 1.5x boost
     - Shorter lines preferred
     - Matches at start of line get 1.5x boost

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

