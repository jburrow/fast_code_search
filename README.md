<div align="center">

# fast_code_search

**An in-memory code search server. Millisecond queries over multi-gigabyte codebases.**

[![CI](https://github.com/jburrow/fast_code_search/actions/workflows/ci.yml/badge.svg)](https://github.com/jburrow/fast_code_search/actions/workflows/ci.yml)
[![Benchmarks](https://github.com/jburrow/fast_code_search/actions/workflows/benchmark.yml/badge.svg)](https://github.com/jburrow/fast_code_search/actions/workflows/benchmark.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

<img src="docs/images/web-ui.png" alt="The embedded web UI searching a live index" width="820"/>

</div>

---

fast_code_search is an always-on search server written in Rust. It builds a trigram
inverted index over your code, enriches it with symbols parsed by tree-sitter, and
keeps the whole thing hot in memory — so a query that takes `grep` seconds on a large
tree returns in one to five milliseconds. Results stream over gRPC, a JSON REST API,
and an embedded web UI.

It ships two engines:

| Engine | Query style | Backed by | Ports |
|--------|-------------|-----------|-------|
| Keyword (primary) | `fn main`, `class.*Handler`, symbol names | Trigram index, tree-sitter ranking | 50051 / 8080 |
| Semantic (optional) | "retry logic with exponential backoff" | TF-IDF or CodeBERT/UniXcoder embeddings | 50052 / 8081 |

## Quick start

Requires Rust 1.70+ and the Protocol Buffers compiler (`protoc`).

```bash
cargo build --release

# Generate a config, then add your project paths to it
cargo run --release --bin fast_code_search_server -- --init .keyword_config.toml

# Start the server and open http://localhost:8080
cargo keyword
```

Or query the API directly:

```bash
curl "http://localhost:8080/api/search?q=fn%20main&max=10"
```

`cargo keyword` and `cargo semantic` are aliases defined in [.cargo/config.toml](.cargo/config.toml).
For the optional semantic engine, see [docs/semantic/SEMANTIC_SEARCH_README.md](docs/semantic/SEMANTIC_SEARCH_README.md).

## Features

- **Trigram inverted index** over Roaring bitmaps; candidate lookup is a bitmap
  intersection, independent of corpus size.
- **Symbol-aware ranking.** tree-sitter parses 12 programming languages (plus JSON,
  TOML, YAML, HTML, CSS, Markdown); definitions outrank usages.
- **Dependency graph.** Imports are resolved across the codebase — query "what
  imports this file", and heavily-imported files rank higher.
- **Regex search** accelerated by literal pre-filtering: required literals are
  extracted from the pattern and intersected through the trigram index before the
  regex runs.
- **Symbols-only mode** for finding definitions without wading through call sites.
- **Incremental indexing.** A file watcher applies edits, deletes, and renames to the
  live index; no rebuild, no restart.
- **Persistent index.** Atomic save/load with integrity checks — restarts skip
  re-indexing entirely.
- **Parallel everything.** Indexing and search fan out across cores via rayon;
  searches run concurrently under a read lock.

### Ranking

Scores combine a content match with structural signals:

| Signal | Boost |
|--------|------|
| Symbol definition | 3.0× |
| Exact case-sensitive match | 2.0× |
| Match at start of line | 1.5× |
| File in `src/` or `lib/` | 1.5× |
| Heavily-imported file | `1 + log10(importers) × 0.5` |
| Long lines | inverse-length penalty |

## Why a server instead of a CLI?

Tools like ripgrep re-scan files on every invocation; that cost is unbeatable for a
one-off search and unacceptable when an IDE issues a query per keystroke. fast_code_search
pays the scan cost once at startup, then answers from memory:

| Scenario | ripgrep | fast_code_search |
|----------|---------|------------------|
| 1 query, Linux kernel (~1 GB) | 80 ms | 3 s index + 5 ms |
| 100 queries | 8 s | 3.5 s total |
| 100 queries, 10 GB corpus | ~500 s | ~61 s total |

The crossover arrives around 50 queries — roughly one coding session. The always-on
design is what makes search-as-you-type, live dependency queries, and team-shared
indexes practical.

Use ripgrep for one-off searches; use [Zoekt](https://github.com/sourcegraph/zoekt) if
you need a disk-resident index. See [docs/design/PRIOR_ART.md](docs/design/PRIOR_ART.md)
for a detailed comparison of the architectures, including published benchmark context.

## Benchmarks

Tracked in CI on every push to `main` ([workflow](../../actions/workflows/benchmark.yml),
history on `gh-pages`). Representative timings on the synthetic corpus:

| Benchmark | Corpus | Time |
|-----------|--------|------|
| text search, common query | 200 files | ~3.5 ms |
| text search, rare query | 100 files | ~0.3 ms |
| text search, no match | 100 files | ~0.1 ms |
| regex, with literal | 100 files | ~9 ms |
| regex, no literal (full scan) | 100 files | ~45 ms |
| indexing | 100 files | ~25 ms |

```bash
cargo bench                                # all benchmarks
cargo bench --bench search_benchmark       # search only
cargo bench --bench persistence_benchmark  # persistence only
```

## Usage

### Configuration

Generate a documented template with `--init`:

```bash
cargo run --release --bin fast_code_search_server -- --init config.toml
```

```toml
[server]
address = "0.0.0.0:50051"      # gRPC
web_address = "0.0.0.0:8080"   # REST + web UI
enable_web_ui = true

[indexer]
paths = ["/path/to/codebase"]
exclude_patterns = ["**/node_modules/**", "**/target/**", "**/.git/**"]  # globs
max_file_size = 10485760

# Optional: persist the index across restarts
index_path = "/var/lib/fast_code_search/index.bin"
save_after_build = true
checkpoint_interval_files = 20000   # crash recovery on very large builds

# Optional: watch the filesystem and update the index incrementally
watch = true
```

On hosts with a low `vm.max_map_count` (e.g. RHEL 7), the server detects the limit
and degrades gracefully to direct reads; see
[docs/DEPLOYMENT.md](docs/DEPLOYMENT.md#memory-allocation-errors-on-rhel7centos7).

### CLI

```
fast_code_search_server [OPTIONS]

  -c, --config <FILE>       Path to configuration file
  -a, --address <ADDR>      gRPC listen address (overrides config)
  -i, --index <PATH>        Additional paths to index (repeatable)
      --no-auto-index       Skip automatic indexing on startup
      --init <FILE>         Generate a template configuration file
  -v, --verbose             Verbose logging
```

### REST API

Served from the web address (default `:8080`). Errors are JSON (`{"error": …}`);
`503` during index updates carries a `Retry-After` header.

| Endpoint | Description |
|----------|-------------|
| `GET /api/search` | Search the index |
| `GET /api/stats` | Index statistics |
| `GET /api/status` | Indexing progress |
| `GET /api/health` | Health check |
| `GET /api/file` | Full file content |
| `GET /api/context` | Lines around a match (`?file=…&line=N&context=K`) |
| `GET /api/dependents` / `GET /api/dependencies` | Import graph queries |
| `GET /api/diagnostics` | Index health and self-tests |
| `WS /ws/progress` | Live indexing progress |

Search parameters:

| Parameter | Default | Description |
|-----------|---------|-------------|
| `q` | required | Query string |
| `max` | 50 | Result cap, 1–1000; response sets `has_more` when hit |
| `regex` | false | Treat the query as a regex |
| `symbols` | false | Match symbol names only |
| `include` / `exclude` | — | Semicolon-delimited path globs |
| `rank` | auto | `auto`, `fast`, or `full` ranking |
| `context` | 0 | Context lines per match, 0–10 |

```bash
curl "http://localhost:8080/api/search?q=fn%20main&regex=true&include=src/**"
```

The full parameter reference is on the server's own `/docs.html` page.

### gRPC API

Streaming search and index management on the gRPC port; the schema lives in
[proto/](proto/). A minimal client is in [examples/client.rs](examples/client.rs):

```bash
cargo run --example client
```

## Architecture

| Component | Source | Role |
|-----------|--------|------|
| Trigram index | `src/index/trigram.rs` | Roaring-bitmap postings, intersection queries |
| File store | `src/index/lazy_file_store.rs` | Lazy memory-mapped file access |
| Persistence | `src/index/persistence.rs` | Atomic save/load with integrity header |
| Symbol extractor | `src/symbols/extractor.rs` | tree-sitter parsing across languages |
| Search engine | `src/search/engine.rs` | Candidate selection, ranking, parallel scan |
| Background indexer | `src/search/background_indexer.rs` | Two-phase parallel index build |
| File watcher | `src/search/watcher.rs` | Debounced incremental updates |
| gRPC server | `src/server/` | Streaming results |
| REST + web UI | `src/web/` | JSON API, embedded UI, progress WebSocket |
| Semantic engine | `src/semantic/` | Chunking, embeddings, vector search |

Indexing runs in two phases: a parallel pure-Rust pass (read, transcode, trigram
extraction) followed by tree-sitter symbol extraction, merged into the engine in
batches so searches stay responsive during a build. Queries intersect trigram
bitmaps to find candidates, scan candidates in parallel, then rank.

## Supported languages

Symbol extraction: Rust, Python, JavaScript, TypeScript, Go, C, C++, Java, C#, Ruby,
PHP, Bash — plus JSON, TOML, YAML, HTML, CSS, and Markdown. Everything else is still
indexed and searchable, just without symbol-aware ranking.

## Development

```bash
cargo test                                          # unit + integration tests
cargo run --release --bin fast_code_search_validator  # synthetic-corpus validation
cargo run --release --bin fast_code_search_validator -- --load-test --duration 30
```

The validator generates a corpus, verifies index completeness, line numbers, symbol
extraction, and every query option, and can measure throughput under load (`--json`
for CI). Contributor workflow: [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md). Instruction-file
policy: [docs/INSTRUCTION_FILES_BLUEPRINT.md](docs/INSTRUCTION_FILES_BLUEPRINT.md).

## Documentation

- [CHANGELOG.md](CHANGELOG.md) — release notes
- [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) — development guide
- [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) — deployment guide
- [docs/design/PRIOR_ART.md](docs/design/PRIOR_ART.md) — ripgrep / Zoekt / GitHub Code Search comparison
- [docs/semantic/SEMANTIC_SEARCH_README.md](docs/semantic/SEMANTIC_SEARCH_README.md) — semantic engine setup
- [docs/GLOSSARY.md](docs/GLOSSARY.md) — terminology

## License

MIT — see [LICENSE](LICENSE).
