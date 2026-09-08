# API and configuration reference

Everything the server exposes: the REST API served on `web_address`
(default `127.0.0.1:8080`), the gRPC service on `address` (default
`127.0.0.1:50051`), the server's command line, and the configuration file.
The web UI's own `/docs.html` page carries the same REST reference with
live examples.

## REST API

Errors are JSON (`{"error": "…"}`) with the right status: 400 for bad
parameters or an invalid regex, 404 for an unknown file, 503 with
`Retry-After` while the index is being written, 504 with the same envelope
when the request timeout is hit.

| Endpoint | Description |
|----------|-------------|
| `GET /api/search` | Search (text, regex, symbols, references) |
| `GET /api/file?file=…` | Full content of an indexed file |
| `GET /api/context?file=…&line=N&context=K` | Lines around a match (K ≤ 200) |
| `GET /api/dependents?file=…` / `GET /api/dependencies?file=…` | Files that import this file / files it imports |
| `GET /api/stats` | Index statistics (files, trigrams, dependency edges, content bytes) |
| `GET /api/status` | Indexing progress |
| `GET /api/health` | Liveness (`{"status":"healthy","version":…}`) |
| `GET /api/ready` | Readiness: 200 once there is an index to search, 503 otherwise |
| `GET /api/diagnostics` | Index health, extension breakdown, self-tests |
| `GET /metrics` | Prometheus text format (request counters, latency histogram, index gauges) |
| `WS /ws/progress` | Live indexing progress frames |

### `GET /api/search`

| Parameter | Default | Description |
|-----------|---------|-------------|
| `q` | required | Query. Plain text understands the [query syntax](#query-syntax); with `regex=true` it is a regular expression. |
| `max` | 50 | Page size, 1–1000 (`0` = default). |
| `offset` | 0 | Hits to skip; ordering is deterministic, so `offset=max` is page two. At most 10,000. |
| `regex` | false | Treat `q` as a regex (Rust `regex` syntax, matched one line at a time unless the pattern mentions `\n` or sets `(?s)`). |
| `symbols` | false | Definitions only (functions, types, classes, …) plus filename matches. |
| `references` | false | Uses of the identifier in `q`: call sites and type mentions. Cannot be combined with `regex` or `symbols`. |
| `case` | — | `true` / `false` overrides `case:` in the query. |
| `word` | — | `true` for whole-word matching. |
| `include` / `exclude` | — | Semicolon-separated path globs (`src/**/*.rs;lib/**`). |
| `rank` | auto | `auto`, `fast` (metadata-ranked sample, used above 5,000 candidates) or `full`. |
| `context` | 0 | Context lines before and after each hit, 0–10. |
| `timeout_ms` | 0 | Stop scanning after this long and return the best hits so far (capped at 30 s; every search also runs under the server's request timeout). |

Response:

```json
{
  "results": [{
    "file_path": "proj/src/main.rs",
    "line_number": 5,
    "content": "fn helper_target(x: i32) -> i32 {",
    "match_start": 3, "match_end": 16,
    "line_match_start": 3, "line_match_end": 16, "match_column": 3,
    "content_truncated": false,
    "score": 12.5,
    "match_type": "SYMBOL_DEFINITION",
    "dependency_count": 2,
    "context_lines": ["…"], "context_start_line": 4
  }],
  "query": "helper_target",
  "total_results": 1, "offset": 0, "total_matches": 3,
  "has_more": true, "truncated_by_budget": false,
  "elapsed_ms": 0.7, "rank_mode": "full",
  "total_candidates": 12, "candidates_searched": 12
}
```

`match_start`/`match_end` are byte offsets into `content` (which may be a
truncated window of a long line, see `content_truncated`);
`line_match_start`/`line_match_end` are byte offsets into the full line and
`match_column` the 0-based character column, for placing an editor cursor.
`line_number` is 0 for a filename match. `match_type` is `TEXT`,
`SYMBOL_DEFINITION` or `SYMBOL_REFERENCE`. `total_matches` is present when
the search ran to completion; otherwise `truncated_by_budget` is true and
`has_more` means more may exist.

### Query syntax

Plain-text queries (not `regex=true`):

| Syntax | Meaning |
|--------|---------|
| `fn main` | Several terms: a file must contain every term. Lines holding the phrase rank first, then lines holding every term, then the rest. |
| `"exact phrase"` | One term containing spaces. |
| `-term` | Drop files that contain `term` (only before a letter, `_` or a quote, so `->` and `-1` are ordinary terms). |
| `file:PATTERN` / `-file:PATTERN` | Only / never paths matching the glob; a bare word matches anywhere in the path, `src/` means everything under `src`. |
| `lang:rust` / `-lang:py` | Only / never files of that language. |
| `case:yes` | Case-sensitive. |
| `word:yes` | Whole words only. |

### Ranking

Each hit's score combines the match with structural signals (weights in
`src/search/ranking.rs`):

| Signal | Effect |
|--------|--------|
| Line defines a symbol | 3× |
| Exact case | 2× |
| Match at the start of the line | 1.5× |
| File under `src/` or `lib/` | 1.5× |
| Files that import this file | `1 + 0.5·log10(dependents)` |
| Line holds the whole phrase / every term | tier above all other lines |
| Long line | inverse-length damping |
| Test or example path (fast mode) | 0.7× |

Within a tier, hits are interleaved by file (every file's best hit first)
so one file cannot fill a page.

## gRPC

The schema is [`proto/search.proto`](../proto/search.proto): `Search`
streams `SearchResult` messages with the same fields and modes as REST
(`is_regex`, `symbols_only`, `references`, `case_sensitive`,
`whole_word`, `include_paths`, `exclude_paths`, `rank`, `offset`,
`deadline_ms`), and `Index` adds paths under the configured roots. The
standard `grpc.health.v1` service is served too. A minimal client:
[`examples/client.rs`](../examples/client.rs) (`cargo run --example client`).

Limits match REST: `offset` ≤ 10,000, the search semaphore is shared with
the REST side, and every search runs under a deadline derived from the
request timeout.

## Server command line

```
fast_code_search_server [OPTIONS]

  -c, --config <FILE>       Configuration file
  -a, --address <ADDR>      gRPC listen address (overrides config)
      --web-address <ADDR>  Web UI / REST listen address (overrides config)
  -i, --index <PATH>        Additional path to index (repeatable)
      --no-auto-index       Skip indexing on startup
      --init <FILE>         Write a documented template configuration and exit
      --static-dir <DIR>    Serve the UI from disk (development)
  -v, --verbose             Debug logging
```

Configuration discovery: `--config`, then `$FCS_CONFIG`, then
`./fast_code_search.toml`, then `~/.config/fast_code_search/config.toml`.

## Configuration file

`fast_code_search_server --init config.toml` writes a fully commented
template. The keys that matter most:

```toml
[server]
address = "127.0.0.1:50051"       # gRPC
web_address = "127.0.0.1:8080"    # REST + web UI
enable_web_ui = true
max_concurrent_searches = 64      # shared by REST and gRPC; beyond it: 503 / RESOURCE_EXHAUSTED
request_timeout_secs = 30         # searches get an engine deadline just under this
cors_origins = []                 # e.g. ["https://ide.example.com"]; empty = same-origin only

[indexer]
paths = ["~/work", "/srv/repos/other"]     # ~ expands; relative paths resolve against this file
exclude_patterns = ["**/node_modules/**", "**/target/**", "**/.git/**"]
include_extensions = []           # empty = every text file; ["rs", "py"] to restrict
max_file_size = 10485760          # bytes; 0 = default
respect_gitignore = true

index_path = "~/.local/share/fast_code_search/index.fcsidx"   # persist across restarts
save_after_build = true
checkpoint_interval_files = 20000
watch = true                      # follow edits, renames, deletes
save_after_updates = 100          # save after this many watched file changes
```

Changing exclude patterns, extensions, the size cap or a `.gitignore` takes
effect on the next start: files that are no longer eligible are dropped
when the persisted index is reconciled.

## Exit codes and errors (CLI)

`fcs` exits 0 when it printed at least one match, 1 when the search found
nothing, and 2 on error (bad arguments, invalid regex, server error, no
server and no index). See [CLI.md](CLI.md).
