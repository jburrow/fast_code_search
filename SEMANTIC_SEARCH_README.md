# Semantic Code Search

Natural language code discovery for fast_code_search, enabling developers to find code using conceptual queries instead of exact keywords.

## Quick Start

### 1. Generate Configuration

```bash
cargo run --bin fast_code_search_semantic -- --init
```

This creates `fast_code_search_semantic.toml` with default settings.

### 2. Configure Paths to Index

Edit `fast_code_search_semantic.toml`:

```toml
[indexer]
paths = [
    "/path/to/your/codebase",  # Unix/Linux/Mac
    # "C:\\code\\my-project",  # Windows example
]

# Optional: Enable persistent index
# Unix/Linux/Mac:
index_path = "/var/lib/fast_code_search_semantic/index"
# Windows:
# index_path = "C:\\ProgramData\\fast_code_search_semantic\\index"
```

### 3. Start the Server

```bash
cargo run --bin fast_code_search_semantic -- --config fast_code_search_semantic.toml
```

The server starts on:
- **gRPC**: port 50052
- **Web UI**: http://localhost:8081

### 4. Search Your Code

**Web UI** (easiest):
- Open http://localhost:8081 in your browser
- Enter natural language queries like "authentication logic" or "database connection"

**REST API**:
```bash
curl "http://localhost:8081/api/search?q=authentication&max=10"
```

**gRPC** (using example client):
```bash
cargo run --example semantic_grpc_client "authentication logic"
```

## Features

### Natural Language Queries

Ask questions in plain English:
- "authentication and login flow"
- "error handling patterns"
- "database connection setup"
- "API endpoint definitions"
- "configuration file loading"

### Symbol-Aware Chunking

Automatically identifies and indexes:
- Functions and methods
- Classes and structs
- Modules and namespaces

### Query Caching

- LRU cache for query embeddings
- Near-instant responses for repeated queries
- 100 query capacity (configurable)

### Index Persistence

- Save index to disk after indexing
- Load pre-built index on startup
- No re-indexing needed on restart

### Dual API Access

**REST API** (`/api/...`):
- `/api/search?q={query}&max={limit}` - Search with natural language
- `/api/stats` - Get server statistics
- `/api/health` - Health check

**gRPC** (port 50052):
- `Search` - Streaming search results
- `GetStats` - Server statistics
- `ReloadIndex` - Dynamic index reloading

### Web Interface

Modern, responsive UI with:
- Natural language query input
- Example queries for quick start
- Syntax-highlighted results
- Relevance scoring visualization
- Real-time server statistics

## Architecture

### Separate from Traditional Search

Semantic search runs as a completely independent binary:
- Traditional: `fast_code_search_server` (exact/regex search)
- Semantic: `fast_code_search_semantic` (natural language)

Both share:
- Symbol extraction (tree-sitter)
- Infrastructure (logging, config)
- Code organization patterns

### Current Implementation

**Embedding Model**: TF-IDF-based vector representation
- Fast and lightweight
- No external dependencies
- Ready to upgrade to ML models (CodeBERT, UniXcoder)

**Vector Index**: Linear similarity search
- Cosine similarity scoring
- Efficient for moderate codebases
- Upgradeable to HNSW for larger scales

### Performance

Current performance (TF-IDF model):
- Query latency: 20-100ms (depending on index size)
- Index loading: ~1s for 10,000 chunks
- Memory overhead: ~2x indexed code size

## Configuration Reference

### Server Settings

```toml
[server]
# gRPC server address
address = "0.0.0.0:50052"

# Web UI server address
web_address = "0.0.0.0:8081"

# Enable/disable Web UI
enable_web_ui = true
```

### Indexer Settings

```toml
[indexer]
# Paths to index
paths = [
    "/path/to/code",
]

# Patterns to exclude
exclude_patterns = [
    "**/node_modules/**",
    "**/target/**",
    "**/.git/**",
    "**/dist/**",
]

# Code chunking parameters
chunk_size = 50          # Lines per chunk
chunk_overlap = 5        # Overlap between chunks

# Optional: Persistent index
index_path = "/var/lib/fast_code_search_semantic/index"
```

## Examples

### Web UI

Visit http://localhost:8081 and try:
- "authentication and login"
- "error handling"
- "database queries"

### REST API

```bash
# Search
curl "http://localhost:8081/api/search?q=authentication&max=5" | jq

# Stats
curl "http://localhost:8081/api/stats" | jq

# Health check
curl "http://localhost:8081/api/health"
```

### gRPC Client

```bash
# Use the example client
cargo run --example semantic_grpc_client "login logic"

# Or use grpcurl
grpcurl -plaintext -d '{"query":"authentication","max_results":10}' \
  localhost:50052 semantic_search.SemanticCodeSearch/Search
```

## Development

### Running Tests

```bash
# All semantic search tests
cargo test --lib semantic

# Specific module
cargo test --lib semantic::cache
```

### Building

```bash
# Debug build
cargo build --bin fast_code_search_semantic

# Release build
cargo build --release --bin fast_code_search_semantic
```

### Linting

```bash
cargo clippy --bin fast_code_search_semantic
cargo fmt --check
```

## Roadmap

### Phase 4 (Complete) ✅
- Enhanced Web UI with natural language interface
- Example queries and suggestions
- Improved result visualization

### Future Enhancements
- **ML Embeddings**: Upgrade to CodeBERT/UniXcoder via ONNX Runtime
- **HNSW Index**: Sub-linear similarity search for large codebases
- **GPU Support**: Faster embedding generation
- **Multi-language**: Support for more programming languages

## Troubleshooting

### Server doesn't start

Check if ports are available:
```bash
lsof -i :50052  # gRPC
lsof -i :8081   # Web UI
```

### No results found

1. Check if files are indexed:
   ```bash
   curl http://localhost:8081/api/stats
   ```

2. Verify paths in config are correct

3. Check exclude patterns aren't too broad

### Slow queries

1. Enable index persistence to avoid re-indexing
2. Reduce `max_results` parameter
3. Consider upgrading to ML embeddings for better accuracy

## License

Same as fast_code_search (see main README)
