# fast_code_search

High-performance, in-memory code search service built in Rust. Designed to handle 10GB+ of text with blazing fast search capabilities.

## Two Search Engines, One Platform

fast_code_search provides **two complementary search engines** optimized for different use cases:

| Engine | Best For | How It Works | Port |
|--------|----------|--------------|------|
| **🔍 Keyword Search** | Finding exact code patterns, function calls, variable names | Trigram index + AST-based ranking | 8080 |
| **🧠 Semantic Search** | Natural language queries like "authentication logic" | TF-IDF or ML embeddings (CodeBERT) | 8081 |

### Keyword Search Engine

The **primary search engine** uses trigram-based indexing with **AST-aware ranking**:

- **Trigram Index**: Splits code into 3-character sequences for O(1) candidate lookup
- **Tree-sitter Parsing**: Extracts function/class definitions from Rust, Python, JS, TS
- **Smart Scoring**: Boosts symbol definitions (3x), exact matches (2x), heavily-imported files (PageRank-style)
- **Regex Support**: Full regex with trigram acceleration for literal prefixes
- **Symbols-Only Mode**: Search only function/class names for targeted results

**Example queries**: `fn main`, `class.*Handler`, `import useState`

### Semantic Search Engine

The **optional semantic engine** understands code meaning, not just text patterns:

- **Natural Language**: Query with phrases like "database connection pooling" or "error handling middleware"
- **Code Chunking**: Splits files into semantic units (functions, classes, modules)
- **Embedding Models**: TF-IDF (fast, no GPU) or CodeBERT/UniXcoder (better quality)
- **Similarity Search**: Finds conceptually related code even without keyword matches

**Example queries**: "retry logic with exponential backoff", "validate user input", "websocket message handler"

> 📖 See [Semantic Search README](docs/semantic/SEMANTIC_SEARCH_README.md) for setup instructions.

## Features

### Keyword Search Engine Features

- **Trigram-based inverted index** using Roaring bitmaps for efficient storage and fast lookup
- **Memory-mapped files** using `memmap2` for optimal memory usage
- **Symbol awareness** using `tree-sitter` for Rust, Python, JavaScript, and TypeScript
- **Parallel search** using `rayon` for maximum throughput
- **Index persistence** — save index to disk and reload on restart for faster startup times
- **File watcher** — incremental indexing monitors filesystem changes in real-time
- **Symbols-only search** — search only in function/class names for faster, targeted results
- **AST-based scoring** that boosts:
  - Symbol definitions (3.0x)
  - Primary source directories (src/, lib/) (1.5x)
  - Exact case-sensitive matches (2.0x)
  - Matches at the start of lines (1.5x)
  - Shorter lines (inverse length factor)
  - **Heavily-imported files** — logarithmic boost based on dependent count (PageRank-style)

### Semantic Search Engine Features

- **Natural language queries** — search by concept, not just keywords
- **Code-aware chunking** — splits files into functions, classes, and modules
- **Dual embedding support**:
  - TF-IDF (default): Fast, no dependencies, good for exact terms
  - ML models (optional): CodeBERT/UniXcoder for deeper semantic understanding
- **Similarity scoring** — ranks results by conceptual relevance

### Shared Infrastructure

- **Dual API access**:
  - **gRPC API** using `tonic` with streaming results (port 50051/50052)
  - **REST API** using `axum` with JSON responses (port 8080/8081)
- **Embedded Web UI** — browser-based search interface with real-time results
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

📖 **See [PRIOR_ART.md](docs/design/PRIOR_ART.md) for a detailed comparison with ripgrep, Zoekt, GitHub Code Search, and improvement roadmap.**

## Architecture

### Keyword Search Engine Components

1. **Trigram Index** (`src/index/trigram.rs`)
   - Extracts 3-character sequences from text
   - Uses Roaring bitmaps to efficiently store document sets
   - Performs fast intersection queries

2. **File Store** (`src/index/file_store.rs`)
   - Memory-maps files for efficient access
   - Deduplicates files by canonical path
   - Supports large codebases without loading everything into RAM

3. **Index Persistence** (`src/index/persistence.rs`)
   - Save/load index to disk with file locking
   - Stores config fingerprint for detecting configuration changes
   - Incremental reconciliation against filesystem on load
   - Multiple read-only servers can share the same index file

4. **Symbol Extractor** (`src/symbols/extractor.rs`)
   - Uses tree-sitter parsers for multiple languages
   - Identifies function/class definitions
   - Enhances search results with semantic information

5. **Search Engine** (`src/search/engine.rs`)
   - Parallel search using rayon
   - Sophisticated AST-based scoring algorithm
   - Returns ranked results
   - Four search methods: text, filtered, regex, and symbols-only

6. **File Watcher** (`src/search/watcher.rs`)
   - Monitors filesystem for changes using `notify-debouncer-full`
   - Incrementally updates index on file add/modify/delete
   - Background processing without blocking searches

7. **gRPC Server** (`src/server/service.rs`)
   - Streaming search results
   - Index management
   - Remote access via gRPC (port 50051)

8. **REST API & Web UI** (`src/web/`)
   - JSON REST API on port 8080
   - Embedded browser-based search interface
   - WebSocket for real-time progress updates

### Semantic Search Engine Components

1. **Semantic Engine** (`src/semantic/engine.rs`)
   - Manages code chunking and embedding generation
   - Coordinates indexing and search operations

2. **Code Chunker** (`src/semantic/chunking.rs`)
   - Splits files into semantic units (functions, classes, modules)
   - Preserves context for better embeddings

3. **Embeddings** (`src/semantic/embeddings.rs`)
   - TF-IDF vectorization for fast, dependency-free operation
   - Optional ML model support via ONNX Runtime

4. **Vector Index** (`src/semantic/vector_index.rs`)
   - Stores and searches embedding vectors
   - Cosine similarity scoring

5. **Semantic gRPC Server** (`src/semantic_server/service.rs`)
   - Streaming semantic search results (port 50052)

6. **Semantic Web UI** (`src/semantic_web/`)
   - JSON REST API on port 8081
   - Natural language search interface

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

### Semantic Search (Optional)

The semantic search binary enables natural language queries:

```bash
# Basic build (TF-IDF embeddings)
cargo build --release --bin fast_code_search_semantic

# With ML models (CodeBERT/UniXcoder - better quality)
cargo build --release --bin fast_code_search_semantic --features ml-models
```

Semantic search runs as a separate server on port 50052 (gRPC) and 8081 (Web UI).

> **Windows users**: The `ml-models` feature requires ONNX Runtime DLL and `ORT_DYLIB_PATH` environment variable. See [Semantic Search README](docs/semantic/SEMANTIC_SEARCH_README.md#building-with-ml-models-optional) for detailed setup instructions.

### Run Tests

```bash
cargo test
```

## Usage

### Starting the Keyword Search Server

```bash
# Start with default settings
cargo run --release --bin fast_code_search_server

# Start with a config file
cargo run --release --bin fast_code_search_server -- --config config.toml

# Generate a template config file
cargo run --release --bin fast_code_search_server -- --init fast_code_search.toml
```

The keyword search server starts on:
- **gRPC**: `0.0.0.0:50051`
- **Web UI**: `http://localhost:8080`

### Starting the Semantic Search Server

```bash
# Generate a config file
cargo run --release --bin fast_code_search_semantic -- --init fast_code_search_semantic.toml

# Start the semantic server
cargo run --release --bin fast_code_search_semantic -- --config fast_code_search_semantic.toml
```

The semantic search server starts on:
- **gRPC**: `0.0.0.0:50052`
- **Web UI**: `http://localhost:8081`

> 💡 **Tip**: You can run both servers simultaneously for combined keyword + semantic search capabilities.

### CLI Options

```
fast_code_search_server [OPTIONS]

Options:
  -c, --config <FILE>       Path to configuration file
  -a, --address <ADDR>      Server listen address (overrides config)
  -i, --index <PATH>        Additional paths to index (repeatable)
      --no-auto-index       Skip automatic indexing on startup
  -v, --verbose             Enable verbose logging
      --init <FILE>         Generate template configuration file
  -h, --help                Print help
  -V, --version             Print version
```

### Configuration File

Create a TOML configuration file (see `--init` to generate a template):

```toml
[server]
address = "0.0.0.0:50051"      # gRPC server address
web_address = "0.0.0.0:8080"   # REST API / Web UI address
enable_web_ui = true           # Enable embedded Web UI

[indexer]
paths = [                      # Directories to index
    "/path/to/codebase",
]
exclude_patterns = [           # Glob patterns to exclude
    "**/node_modules/**",
    "**/target/**",
    "**/.git/**",
]
max_file_size = 10485760       # Skip files larger than 10MB

# Index persistence (optional)
index_path = "/var/lib/fast_code_search/index"
save_after_build = true        # Save after initial indexing
save_after_updates = 0         # Save after N file updates (0 = disabled)

# File watcher (optional)
watch = true                   # Monitor filesystem for changes
```

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

### REST API

The REST API is available at `http://localhost:8080` when `enable_web_ui` is true.

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/search` | GET | Search the index |
| `/api/stats` | GET | Get index statistics |
| `/api/status` | GET | Get indexing progress and status |
| `/api/health` | GET | Health check |
| `/api/dependents` | GET | Get files that import a given file |
| `/api/dependencies` | GET | Get files imported by a given file |
| `/ws/progress` | WS | WebSocket for real-time indexing progress |

#### Search Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `q` | string | required | Query string |
| `max` | int | 50 | Maximum results (1-1000) |
| `include` | string | - | Semicolon-delimited glob patterns to include |
| `exclude` | string | - | Semicolon-delimited glob patterns to exclude |
| `regex` | bool | false | Treat query as regex pattern |
| `symbols` | bool | false | Search only in symbol names |

**Example:**
```bash
curl "http://localhost:8080/api/search?q=fn%20main&max=10&regex=true"
```

### Search Modes (Keyword Engine)

The keyword search engine supports multiple modes:

| Mode | REST Flag | gRPC Flag | Description |
|------|-----------|-----------|-------------|
| **Text Search** | (default) | (default) | Full-text search across all file contents |
| **Regex Search** | `regex=true` | `is_regex=true` | Regular expression pattern matching |
| **Symbols-Only** | `symbols=true` | `symbols_only=true` | Search only in function/class names |

**Symbols-only search** is ideal when you're looking for definitions rather than usages. It searches the symbol cache (extracted via tree-sitter) and returns only matches where the query appears in a symbol name. This is significantly faster than full-text search when you know you're looking for a function or class.

### Semantic Search Mode

For natural language queries like "authentication logic" or "database connection handling", use the semantic search server on port 8081.

The semantic engine excels at:
- **Conceptual queries**: "error handling with retry" finds related code even without exact matches
- **Exploratory search**: "how does authentication work" across unfamiliar codebases
- **Finding related code**: Discover similar implementations across the project

See [docs/semantic/SEMANTIC_SEARCH_README.md](docs/semantic/SEMANTIC_SEARCH_README.md) for detailed documentation.

## Performance Characteristics

- **Indexing**: Parallel file processing, ~100MB/s on modern hardware
- **Search**: Sub-millisecond for most queries on 10GB+ codebases
- **Memory**: Uses memory mapping, so actual RAM usage is much lower than codebase size
- **Scalability**: Handles 10GB+ of text efficiently
- **Persistence**: Load pre-built index on restart (no re-indexing needed)

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

*Last updated: v0.2.2*

### Comparison with Traditional Search Tools

How does fast_code_search compare to industry-standard search tools? Here's a summary based on published benchmarks and architecture analysis:

#### Benchmark Context: Linux Kernel Source (~1GB, ~70,000 files)

| Tool | Query Type | Time | Notes |
|------|------------|------|-------|
| **ripgrep** | Simple literal | ~80ms | Stateless, rescans files each query |
| **ripgrep** | Regex `[A-Z]+_SUSPEND` | ~80ms | SIMD-accelerated literal extraction |
| **The Silver Searcher (ag)** | Simple literal | ~400-1600ms | Memory-mapped, PCRE-based |
| **git grep** | Simple literal | ~340ms | Uses git index, avoids directory walk |
| **GNU grep** | Simple literal | ~500ms | Single-threaded, no filtering |
| **fast_code_search** | Simple literal | **~1-5ms** | Pre-indexed, in-memory |
| **fast_code_search** | Regex | **~5-20ms** | Trigram pre-filtering |

#### Benchmark Context: Large Single File (~9-13GB, subtitle corpus)

| Tool | Query | Time | Notes |
|------|-------|------|-------|
| **ripgrep** | `Sherlock Holmes` | ~270ms | SIMD memchr, rare byte selection |
| **ripgrep** | `Sherlock Holmes` (lines) | ~600ms | Line counting adds overhead |
| **GNU grep** | `Sherlock Holmes` | ~500ms | Boyer-Moore with memchr |
| **GNU grep** (Unicode) | Case-insensitive | ~4s+ | Unicode handling is expensive |
| **The Silver Searcher** | With line numbers | ~2.7s | PCRE + memory mapping |
| **UCG** | With line numbers | ~750ms | PCRE2 JIT compilation |

*Source: [ripgrep benchmark blog](https://burntsushi.net/ripgrep/) by Andrew Gallant*

#### Why fast_code_search Excels at Repeated Queries

The key insight: **amortized cost**. Traditional tools pay per-query costs, while fast_code_search pays once during indexing:

| Scenario | ripgrep (10 queries) | fast_code_search (10 queries) |
|----------|---------------------|------------------------------|
| Linux kernel | 10 × 80ms = **800ms** | 1 × 3000ms + 10 × 5ms = **3050ms** |
| Linux kernel | 100 × 80ms = **8s** | 1 × 3000ms + 100 × 5ms = **3.5s** ✓ |
| 10GB codebase | 10 × 5s = **50s** | 1 × 60s + 10 × 10ms = **60.1s** |
| 10GB codebase | 100 × 5s = **500s** | 1 × 60s + 100 × 10ms = **61s** ✓ |

**Crossover point**: fast_code_search becomes faster after ~50 queries on a typical codebase.

#### Feature Comparison

| Feature | ripgrep | ag | git grep | GNU grep | fast_code_search |
|---------|---------|----|-----------|---------|--------------------|
| Parallel search | ✓ | ✓ | ✓ | ✗ | ✓ |
| Pre-built index | ✗ | ✗ | ✗ | ✗ | ✓ |
| .gitignore support | ✓ | ✓ | ✓ | ✗ | ✓ |
| Regex support | ✓ | ✓ | ✓ | ✓ | ✓ |
| Unicode-aware | ✓ | Partial | Partial | Slow | ✓ |
| Symbol search | ✗ | ✗ | ✗ | ✗ | ✓ |
| Dependency graph | ✗ | ✗ | ✗ | ✗ | ✓ |
| Streaming results | Pipe | Pipe | Pipe | Pipe | gRPC |
| IDE integration | Editor plugins | Editor plugins | Git | Limited | Native API |
| Cross-platform | ✓ | ✓ | ✓ | ✓ | ✓ |

📖 **See [PRIOR_ART.md](docs/design/PRIOR_ART.md) for detailed architectural analysis and improvement roadmap.**

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

## Documentation

- [CHANGELOG.md](CHANGELOG.md) — Version history and release notes
- [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) — Development guide
- [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) — Deployment guide
- [docs/design/PRIOR_ART.md](docs/design/PRIOR_ART.md) — Comparison with ripgrep, Zoekt, etc.
- [docs/semantic/SEMANTIC_SEARCH_README.md](docs/semantic/SEMANTIC_SEARCH_README.md) — Semantic search setup

## License

MIT — See [LICENSE](LICENSE) file.

