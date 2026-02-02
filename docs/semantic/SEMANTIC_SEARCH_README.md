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

**Embedding Model**: TF-IDF-based vector representation (default)
- Fast and lightweight
- No external dependencies
- Works out of the box

**ML Embeddings** (optional, via `--features ml-models`):
- CodeBERT/UniXcoder models via ONNX Runtime
- Higher quality semantic understanding
- Requires model download (~500MB)
- See "Building with ML Models" section for setup

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

### Building with ML Models (Optional)

To use ML-based embeddings (CodeBERT/UniXcoder) instead of TF-IDF, build with the `ml-models` feature:

```bash
cargo build --release --bin fast_code_search_semantic --features ml-models
```

#### Windows-Specific Setup

On Windows, the `ml-models` feature requires additional setup due to CRT (C Runtime) linking conflicts between the `ort` (ONNX Runtime) and `tokenizers` crates. The solution is to use dynamic loading.

**⚠️ Version Compatibility**: This project uses `ort 2.0.0-rc.10` which requires **ONNX Runtime 1.18.x - 1.22.x**. Do NOT use ONNX Runtime 1.23+ (incompatible).

1. **Build with the feature** (ort uses `load-dynamic` internally):
   ```powershell
   cargo build --release --bin fast_code_search_semantic --features ml-models
   ```

2. **Download ONNX Runtime 1.22.0** (recommended version):
   - Go to [ONNX Runtime v1.22.0 Release](https://github.com/microsoft/onnxruntime/releases/tag/v1.22.0)
   - Download `onnxruntime-win-x64-1.22.0.zip`
   - Extract to the project's `onnxruntime/` folder:
     ```
     fast_code_search/
     └── onnxruntime/
         └── onnxruntime-win-x64-1.22.0/
             └── lib/
                 └── onnxruntime.dll
     ```

3. **Run using the launcher script** (recommended - sets `ORT_DYLIB_PATH` automatically):
   ```powershell
   # PowerShell (recommended)
   .\scripts\run_semantic_server.ps1
   
   # Or with custom config
   .\scripts\run_semantic_server.ps1 -ConfigFile .\my_config.toml
   
   # Command Prompt alternative
   scripts\run_semantic_server.bat
   ```

4. **Or set the environment variable manually**:
   ```powershell
   # PowerShell (per-session)
   $env:ORT_DYLIB_PATH = "C:\path\to\onnxruntime.dll"
   .\target\release\fast_code_search_semantic.exe --config fast_code_search_semantic.toml
   
   # Or permanently via System Settings
   [Environment]::SetEnvironmentVariable("ORT_DYLIB_PATH", "C:\path\to\onnxruntime.dll", "User")
   ```

**Why is this needed?** The `ort` crate links against the dynamic C runtime (/MD), while `tokenizers` (via `esaxx-rs`) uses the static runtime (/MT). These are incompatible at link time on Windows. Using `load-dynamic` avoids static linking entirely by loading `onnxruntime.dll` at runtime.

#### Linux/macOS Setup

On Unix systems, the build typically works without additional setup:

```bash
cargo build --release --bin fast_code_search_semantic --features ml-models
```

If you encounter issues, you can still use the dynamic loading approach by setting:
```bash
export ORT_DYLIB_PATH=/path/to/libonnxruntime.so  # Linux
export ORT_DYLIB_PATH=/path/to/libonnxruntime.dylib  # macOS
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
- **ML Embeddings**: Available now via `--features ml-models` (CodeBERT/UniXcoder via ONNX Runtime)
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

### ML Models won't build (Windows)

**Symptom**: Linker errors mentioning `RuntimeLibrary mismatch` or `LIBCMT vs MSVCRT`

**Solution**: This is expected. The `ort` crate in this project uses `load-dynamic` to avoid CRT conflicts. Follow the "Windows-Specific Setup" section in Development > Building with ML Models.

### ML Models runtime error: "Cannot find ONNX Runtime library"

**Solution**: Set the `ORT_DYLIB_PATH` environment variable to point to the ONNX Runtime library:

```powershell
# Windows
$env:ORT_DYLIB_PATH = "C:\path\to\onnxruntime.dll"

# Linux
export ORT_DYLIB_PATH=/path/to/libonnxruntime.so

# macOS
export ORT_DYLIB_PATH=/path/to/libonnxruntime.dylib
```

### ML Models runtime error: "ort X.X is not compatible with ONNX Runtime binary"

**Symptom**: Error like `expected version >= '1.23.x', but got '1.22.0'` or similar version mismatch.

**Solution**: This project uses `ort 2.0.0-rc.10` which requires ONNX Runtime **1.18.x - 1.22.x**.

- If you have ONNX Runtime 1.23+, downgrade to [v1.22.0](https://github.com/microsoft/onnxruntime/releases/tag/v1.22.0)
- If you have ONNX Runtime older than 1.18, upgrade to v1.22.0

On Windows, use the bundled launcher scripts which point to the correct version:
```powershell
.\scripts\run_semantic_server.ps1
```

## License

Same as fast_code_search (see main README)
