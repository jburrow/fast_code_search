# Troubleshooting

| Symptom | Likely cause | What to do |
|---|---|---|
| `fcs: cannot reach the search server` | Server not running, or listening elsewhere | `fcs status`; start the server or pass `--server URL` / set `$FCS_SERVER`; see [Run at startup](../imported/run-at-startup.md) |
| `fcs` says it is searching the on-disk index | Server down, `index_path` known | Fine for a quick answer (results may be behind the tree); start the server for live results |
| Web UI shows "Index is currently being updated" briefly | An install step of the file watcher held the write lock for more than five seconds (rare: a step holds it for about a second at most) | Wait; the UI retries on its own. If constant, a build is running (see the Index page) |
| Results bar says "Index updating · N file changes to apply" | The file watcher has seen changes (a branch switch, a formatter run) and not applied them all yet; results come from the index as it was | Search again shortly; the count drops as the changes are installed. After a branch switch on a very large tree this can take minutes; a count that does not drop for minutes is a bug: with [diagnostic captures](diagnostic-captures.md) on, the server writes what it was doing itself (`fcs diagnose`); otherwise capture a thread dump (`gdb -p <pid> -batch -ex "thread apply all bt"`) and the log |
| A file you edited is not found | Watcher off, or the inotify limit was hit | Set `watch = true`; on Linux check the log for "Failed to watch" and raise `fs.inotify.max_user_watches` |
| Files from an excluded directory still appear | Rules changed while the server ran | Restart; reconciliation drops files the current rules exclude |
| Results stop at "50+ (scan budget reached)" | Broad query, budget hit | Narrow with `file:`, `lang:` or a longer literal; use Load more / `--offset` for the next page |
| `fn main` returns lines without `main` | Older version | Since 0.16 several words are one phrase; upgrade |
| `fn main` misses `fn  main` or `fn\tmain` | Plain text is a literal, spacing included | Use a regex: `fn\s+main` |
| "Searched N of M files" | The server sets `search.fast_rank_files`, and the search read only that many of the most likely files | Click SEARCH ALL FILES (`max_files` on the API and MCP, `fcs --max-files`); raise or unset `search.fast_rank_files` to change the default |
| A regex finds nothing that plain search finds | `{ ( .` are syntax; outside the web UI a regex is also case-sensitive | Escape special characters with `\`, use `\s*` for spacing, start with `(?i)` (or `fcs -e -i`); the web UI's suggestions under an empty result do this for you |
| A regex is slow | No required literal, so every file is scanned | Anchor it with a literal (`fn\s+main` rather than `\w+\s+\w+`) |
| `references` finds nothing, or misses a use, for a name | The use is in a position that language does not record (C and C++ record none), or the index predates extraction changes | [Reference support](../reference/reference-support.md) lists what each language finds; search the plain text for the rest. The index re-extracts symbols on the first start after an upgrade that changed extraction |
| `gRPC API NOT started` / `Web UI ... NOT started` at startup | Port already in use by another instance (perhaps started by hand) | The server keeps running on the other API; `ss -ltnp \| grep :PORT` finds the holder. Stop it, change `address` / `web_address`, or set `enable_grpc = false` |
| Nothing indexed, `paths_count=0` | No config file found: without `--config` only `$FCS_CONFIG`, `./fast_code_search.toml` and `~/.config/fast_code_search/config.toml` are read | Pass `--config FILE`; the `Configuration source` log line says which file was used. Add `--verbose` (or `RUST_LOG=debug`) to see each path and ignore file |
| `Index NOT writable: another fast_code_search server is already using this index` | Two servers share one `index_path` (often an old one still running) | The log names the other PID and ports. Stop it, or give each config its own `index_path`; until then this server loads the index read-only |
| "Server started with N problems" on the search page | A port was busy, the index belongs to another server, or storage is low | Open the Index page: its Service card lists each problem (`/api/diagnostics` has the same list) |
| `No space left on device` when saving, while `df -h` shows free space | The filesystem ran out of inodes | The log says whether bytes or inodes ran out; `df -i` confirms. Free inodes (often many small files) or move `index_path` elsewhere. The server also warns at startup when either is low |
| Index rebuilt on every start | `index_path` unset (startup warns) or unwritable | Set it to a writable location; the log names the path it tried |
| Windows: `C:\Users\NAME~1\…` paths | Short-name spelling of a root | Harmless; roots are compared in canonical form |
| 504 from the API | Request timeout | The search also stopped; narrow the query or pass `timeout_ms` for a partial answer |
| macOS: binary "cannot be opened" | Quarantine flag on a download | `xattr -d com.apple.quarantine fast_code_search_server` |

## Getting more information

- `fcs status` prints the server, version, readiness and index size.
- The **Index** page in the UI (`/diagnostics.html`) shows the service
  status (APIs, where the index is saved, startup and storage problems)
  and runs self-tests that search for sampled files.
- `RUST_LOG=debug fast_code_search_server` logs every batch, watch and save;
  `RUST_LOG=info` is the default.
- `/metrics` exposes request counters and a latency histogram for
  Prometheus.
- With `[diagnostics] capture = true` the server writes a file whenever it
  is slow or stuck, saying what it was doing; `fcs diagnose` summarizes
  them. See [Diagnostic captures](diagnostic-captures.md).

If none of that explains it, open a
[bug report](https://github.com/jburrow/fast_code_search/issues/new/choose)
with the version, the query and the log lines around the problem, and any
diagnostic captures.
