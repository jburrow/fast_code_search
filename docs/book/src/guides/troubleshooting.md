# Troubleshooting

| Symptom | Likely cause | What to do |
|---|---|---|
| `fcs: cannot reach the search server` | Server not running, or listening elsewhere | `fcs status`; start the server or pass `--server URL` / set `$FCS_SERVER`; see [Run at startup](../imported/run-at-startup.md) |
| `fcs` says it is searching the on-disk index | Server down, `index_path` known | Fine for a quick answer (results may be behind the tree); start the server for live results |
| Web UI shows "Index is currently being updated" briefly | A watcher batch or checkpoint holds the write lock | Wait; the UI retries on its own. If constant, a build is running (see the Index page) |
| A file you edited is not found | Watcher off, or the inotify limit was hit | Set `watch = true`; on Linux check the log for "Failed to watch" and raise `fs.inotify.max_user_watches` |
| Files from an excluded directory still appear | Rules changed while the server ran | Restart; reconciliation drops files the current rules exclude |
| Results stop at "50+ (scan budget reached)" | Broad query, budget hit | Narrow with `file:`, `lang:` or a longer literal; use Load more / `--offset` for the next page |
| `fn main` returns lines without `main` | Older version | Since 0.12 lines with the phrase rank first; upgrade |
| A regex is slow | No required literal, so every file is scanned | Anchor it with a literal (`fn\s+main` rather than `\w+\s+\w+`) |
| `references` finds nothing for a name | Language without call-site capture, or the index predates extraction changes | Rust, Python, JavaScript, TypeScript capture calls; the index re-extracts symbols on the first start after an upgrade that changed extraction |
| Port already in use | Another instance (perhaps started by hand) | Stop it, or change `address` / `web_address` |
| Index rebuilt on every start | `index_path` unset or unwritable | Set it to a writable location; the log names the path it tried |
| Windows: `C:\Users\NAME~1\…` paths | Short-name spelling of a root | Harmless; roots are compared in canonical form |
| 504 from the API | Request timeout | The search also stopped; narrow the query or pass `timeout_ms` for a partial answer |
| macOS: binary "cannot be opened" | Quarantine flag on a download | `xattr -d com.apple.quarantine fast_code_search_server` |

## Getting more information

- `fcs status` prints the server, version, readiness and index size.
- The **Index** page in the UI (`/diagnostics.html`) runs self-tests that
  search for sampled files.
- `RUST_LOG=debug fast_code_search_server` logs every batch, watch and save;
  `RUST_LOG=info` is the default.
- `/metrics` exposes request counters and a latency histogram for
  Prometheus.

If none of that explains it, open a
[bug report](https://github.com/jburrow/fast_code_search/issues/new/choose)
with the version, the query and the log lines around the problem.
