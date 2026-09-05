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
tree returns in milliseconds. Results stream over gRPC, a JSON REST API, and an
embedded web UI.

It ships two engines:

| Engine | Query style | Backed by | Ports |
|--------|-------------|-----------|-------|
| Keyword (primary) | `fn main`, `class.*Handler`, symbol names | Trigram index, tree-sitter ranking | 50051 / 8080 |
| Semantic (optional) | "retry logic with exponential backoff" | TF-IDF or CodeBERT/UniXcoder embeddings | 50052 / 8081 |

## Quick start

Requires Rust 1.89+ and the Protocol Buffers compiler (`protoc`).

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
The semantic engine is behind the `semantic` Cargo feature (`ml-models` implies it), so the
default build is the keyword server only; see
[docs/semantic/SEMANTIC_SEARCH_README.md](docs/semantic/SEMANTIC_SEARCH_README.md).

## Features

- **Trigram inverted index** over Roaring bitmaps; candidate lookup is a bitmap
  intersection, independent of corpus size.
- **Symbol-aware ranking.** tree-sitter parses 12 programming languages using each
  grammar's own `tags.scm` definitions query; definitions outrank usages.
- **Dependency graph.** Imports are resolved across the codebase — query "what
  imports this file", and heavily-imported files rank higher.
- **Regex search** accelerated by literal pre-filtering: required literals are
  extracted from the pattern and intersected through the trigram index before the
  regex runs.
  Patterns are line-oriented unless they mention a newline or set the `s`
  flag (`(?s)begin.*?end`), which matches across lines.
- **Symbols-only mode** for finding definitions without wading through call sites,
  and **reference search** for the opposite: every call site or type mention of an
  identifier, as reported by the grammars' tags queries.
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

## Network exposure

The server has no authentication: the REST API returns full file contents and
the gRPC `Index` RPC indexes paths on request (restricted to the configured
`paths`). Both listeners therefore bind to loopback by default. Bind to
`0.0.0.0` only on a trusted network, and list explicit `cors_origins` if a page
on another origin needs to call the API (the embedded UI does not).

## Why a server instead of a CLI?

Tools like ripgrep re-scan files on every invocation. That cost is unbeatable for a
one-off search, and a poor fit when an IDE issues a query per keystroke:
[ripgrep's published benchmarks](https://burntsushi.net/ripgrep/) (Andrew Gallant,
2016; Linux kernel checkout, simple literal) measured roughly 80 ms per query, and
scans of multi-gigabyte corpora take seconds. fast_code_search pays the scan cost once, at
index build, and answers subsequent queries from memory in microseconds to
milliseconds (measured numbers below).

The trade is a one-time build plus resident memory in exchange for a per-query cost
two to three orders of magnitude lower — which is what makes search-as-you-type,
live dependency queries, and team-shared indexes practical.

Use ripgrep for one-off searches; use [Zoekt](https://github.com/sourcegraph/zoekt) if
you need a disk-resident index. See [docs/design/PRIOR_ART.md](docs/design/PRIOR_ART.md)
for a detailed comparison of the architectures, including published benchmark context.

## Benchmarks

Tracked in CI on every push to `main` ([workflow](../../actions/workflows/benchmark.yml));
historical trends are charted at
**[jburrow.github.io/fast_code_search/dev/bench](https://jburrow.github.io/fast_code_search/dev/bench/)**:

| Benchmark | Corpus | Time |
|-----------|--------|------|
| text search, common query | 200 files | 0.93 ms |
| text search, common query | 100 files | 0.51 ms |
| text search, rare query | 100 files | 21 µs |
| text search, no match | 100 files | 0.6 µs |
| regex, accelerated literal | 100 files | 0.34 ms |
| regex, alternation | 100 files | 0.60 ms |
| regex, no literal (full scan) | 100 files | 0.82 ms |
| index build | 100 files | 44 ms |
| index save / load | 1000 files | 6.4 ms / 16.5 ms |

**Source:** [Benchmarks run #27326492528](https://github.com/jburrow/fast_code_search/actions/runs/27326492528)
on commit `f25c92b` (v0.9.0, 2026-06-11), GitHub-hosted `ubuntu-latest` runner.
Values are Criterion means (`cargo bench -- --output-format bencher`, ns/iter),
rounded to two significant figures; raw output is in the run's `benchmark-results`
artifact. The corpus is synthetic Rust-like source, 50 lines (~3 KB) per file,
generated by [benches/search_benchmark.rs](benches/search_benchmark.rs) — searches
measure the query path only, against a pre-built in-memory index.

Absolute timings vary with hardware and corpus; the workflow tracks trends across
commits and flags regressions beyond a 2× threshold.

### Real corpus

The same workflow also builds an index over two pinned real repositories,
[tokio 1.45.0](https://github.com/tokio-rs/tokio/tree/tokio-1.45.0) and
[Django 5.2](https://github.com/django/django/tree/5.2), with
[examples/corpus_bench.rs](examples/corpus_bench.rs) and prints this table in the
job summary. Numbers below are from a local run on an 11th-gen Intel i5 laptop
(4 cores / 8 threads, 7 GB RAM) on 2026-09-05, commit `04e1a00`; the CI run for
each commit on `main` carries the authoritative values.

| Measure | Value |
|---------|-------|
| files indexed | 6,240 (38.9 MB of text) |
| full build (read, trigrams, symbols, imports, merge) | 3.1 s, ~2,000 files/s, ~12 MB/s |
| resident memory after build | 143 MB |
| index save / reconciling load | 0.11 s / 0.24 s (18.6 MB on disk) |
| text search, common word (`return`), p50 / p95 | 0.51 ms / 0.60 ms |
| text search, identifier, p50 | 0.6 ms |
| regex with literal / case-insensitive / no literal, p50 | 1.5 ms / 1.2 ms / 0.8 ms |
| symbol search, p50 | 1.7 ms |
| incremental update of one file, p50 / p95 | 4.3 ms / 8.0 ms |

Tree-sitter symbol extraction is about three quarters of build time (the same
build without symbols takes 1.5 s). A nightly job runs the same benchmark over
the `rust-lang/rust` 1.89.0 tree and records file count, throughput and memory in
its job summary.

```bash
cargo run --release --example corpus_bench -- path/to/repo [more paths] --label mine
```

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
address = "127.0.0.1:50051"    # gRPC
web_address = "127.0.0.1:8080" # REST + web UI
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
| `references` | false | Return the uses (call sites, type mentions) of the identifier in `q` |
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
