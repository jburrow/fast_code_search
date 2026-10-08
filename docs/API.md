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
| `POST /mcp` | Model Context Protocol for coding agents. See [MCP](#mcp-coding-agents) |
| `GET /api/graph/…` | The import graph: neighbourhoods, impact, import chains, the folder map. See [Import graph](#import-graph) |
| `GET /api/stats` | Index statistics (files, trigrams, dependency edges, content bytes) |
| `GET /api/status` | Indexing progress |
| `GET /api/health` | Liveness (`{"status":"healthy","version":…,"problems":N}`); `problems` counts startup and storage problems |
| `GET /api/ready` | Readiness: 200 once there is an index to search, 503 otherwise |
| `GET /api/diagnostics` | Index health, extension breakdown, self-tests, and `service`: web UI and gRPC status, where the index is saved, and the problems (a problem marks the status `degraded`) |
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
| `references` | false | Uses of the identifier in `q`: call sites and type mentions. Cannot be combined with `regex` or `symbols` (400). |
| `case` | — | `true` / `false` overrides `case:` in the query. A regex is case-sensitive by default; `case=false` adds `(?i)`. |
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
| `"exact phrase"` | One term containing spaces. Quotes that do not wrap a whole word are part of the term: `{ "success": True` searches for `"success":` with its quotes. |
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

### Import graph

The dependency explorer (`/graph.html`) is built on these. Every endpoint
answers from memory under the engine read lock. `file` accepts a display
path (`project/src/main.rs`) or any path `/api/file` accepts. Unknown files
are 404.

Rust `mod foo;` declarations are edges from a parent module to its own
child. They describe structure, not dependencies, and would put every parent
and child in a cycle, so they are left out unless `containment=true` (also
accepted by every endpoint below). A `pub use child::X` re-export is the same
edge, so it is left out too.

| Endpoint | Parameters | Returns |
|----------|------------|---------|
| `GET /api/graph/neighborhood` | `file`, `depth` (1–4, default 2), `limit` (files per level, default 12, max 500), `expand` (comma-separated levels to list up to 500 files, e.g. `-2,1`) | `nodes` (`path`, signed `depth`: negative for files it imports, positive for files importing it; `imports`, `imported_by`, `test`, `cycle`), `edges` among them (`from`, `to`, `cycle`, `containment`), `hidden` (per level: `count` and the shared `folder` or number of `folders`), and `upstream` / `downstream` totals |
| `GET /api/graph/impact` | `file`, `limit`, `expand` | Every file that transitively imports `file`, by distance: `affected`, `levels`, `tests_total`, `tests` (up to 200, nearest first), plus `nodes` / `edges` / `hidden` as above |
| `GET /api/graph/path` | `from`, `to` | The shortest import chain as `files` (each imports the next). When there is none from `from` to `to`, the reverse chain with `reversed: true`; `found: false` when neither exists |
| `GET /api/graph/modules` | — | The graph collapsed to folders: `folders` (`files`, `imports`, `imported_by`, `cycle`) and `edges` (`from`, `to`, `count` of file imports, `cycle`). Cached until the index changes |
| `GET /api/graph/imports` | `file` | The file's import statements: `line` (1-based), `spec` as written, the indexed `target` it resolves to (`null` for packages, the standard library and files outside the index), `containment` |
| `GET /api/graph/files` | `q` (case-insensitive substring), `limit` (default 100, max 1000) | `files` most connected first, with `imports` / `imported_by`, and the `total` that matched |

Imports are resolved for Rust (`crate::`, `self::`, `super::`, sibling
modules and the crate's own name), Python and JavaScript/TypeScript.

## MCP (coding agents)

The web server also speaks the [Model Context Protocol](https://modelcontextprotocol.io)
at `/mcp`, so a coding agent can use the index instead of grepping: search,
read files with line numbers, and walk the import graph. Point the agent at
the URL:

```bash
claude mcp add --transport http fast_code_search http://127.0.0.1:8080/mcp
```

For clients that only launch local commands, `fcs mcp` relays MCP over stdio
to the same endpoint (it finds the server like every `fcs` command:
`--server`, `$FCS_SERVER`, the configuration, then `http://127.0.0.1:8080`):

```json
{ "mcpServers": { "fast_code_search": { "command": "fcs", "args": ["mcp"] } } }
```

| Tool | Arguments | What it returns |
|------|-----------|-----------------|
| `search_code` | `query` (the [query syntax](#query-syntax)), `mode` (`text`, `regex`, `symbols`, `references`), `include` / `exclude` globs, `limit` (default 20, max 100), `context` (0–5) | `path:line:column: line` per hit, with the match count and whether more exist |
| `read_file` | `path`, `start_line`, `end_line` | Numbered lines, 200 by default and at most 1,000, with how to continue |
| `file_dependencies` | `path`, `direction` (`in`, `out`, `both`), `depth` (1–4), `limit` | Files it imports and files importing it, by distance, with the line of each direct import |
| `change_impact` | `paths` (one or more), `limit` | Every file that transitively imports any of them, nearest first, split into tests and other code |
| `import_path` | `from`, `to` | The shortest chain of imports between two files, either way round |
| `dependency_overview` | `directory` (optional), `limit` | The most connected folders, the heaviest folder-to-folder imports, folder cycles and hub files |

Every tool is read-only and answers from the same handlers as the REST API,
so it shares their search permits, deadlines and caps. Results use the
root-relative paths search results print, and lists are capped with a note
saying what was left out. The graph tools say which languages have import
edges (Rust, Python, JavaScript/TypeScript), so an empty answer for another
language is not mistaken for "unused".

Transport details: each JSON-RPC message is a `POST /mcp` answered with
`application/json` (notifications get `202`); there is no event stream
(`GET /mcp` is `405`) and no session. Requests carrying a browser `Origin`
are refused with `403` unless it is a loopback address, the server's own
address, or listed in `cors_origins`, so a web page cannot drive a local
server through the visitor's browser. Like the REST API, `/mcp` has no
authentication: anyone who can reach the web port can read the index.

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
      --no-grpc             Do not start the gRPC API (same as enable_grpc = false)
      --init <FILE>         Write a documented template configuration and exit
      --static-dir <DIR>    Serve the UI from disk (development)
  -v, --verbose             Debug logging
```

Configuration discovery: `--config`, then `$FCS_CONFIG`, then
`./fast_code_search.toml`, then `~/.config/fast_code_search/config.toml`.
The first log lines say which file was used (or that none was found and
the defaults apply) and any warnings about it.

If one of the ports cannot be bound, the server logs which port and keeps
running on the other API; it exits only when neither could start. The
diagnostics page and `/api/health` report the failure.

## Configuration file

`fast_code_search_server --init config.toml` writes a fully commented
template. The keys that matter most:

```toml
[server]
address = "127.0.0.1:50051"       # gRPC
enable_grpc = true                # false (or --no-grpc) for web UI / REST / fcs only
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

`--init` writes every key with its default, and sets `index_path` from the
template's own name (`--init work.toml` gives
`~/.local/share/fast_code_search/work.fcsidx`). Without `index_path` the
index lives only in memory and is rebuilt on every start; the startup log
warns about it.

Only one server writes an index. A server holds a lock on
`<index_path>.lock` and records its PID and addresses in
`<index_path>.lock.owner`; a second server given the same `index_path`
logs who owns it, loads the index read-only and never saves. Give each
configuration its own `index_path`.

At startup and before each save the server checks free space and free
inodes on the filesystem holding the index (and inodes where indexed
source lives), and warns when either is low.

Changing exclude patterns, extensions, the size cap or a `.gitignore` takes
effect on the next start: files that are no longer eligible are dropped
when the persisted index is reconciled.

## Exit codes and errors (CLI)

`fcs` exits 0 when it printed at least one match, 1 when the search found
nothing, and 2 on error (bad arguments, invalid regex, server error, no
server and no index). See [CLI.md](CLI.md).
