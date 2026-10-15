# Memory and limits

## What is resident

| Structure | Grows with | Notes |
|---|---|---|
| Trigram posting lists | distinct trigrams × files | Roaring bitmaps; run-optimised after a build |
| Symbol tables | definitions | per file, restored from the saved index |
| Reference tables | call sites and type mentions | names interned once; positions packed |
| Dependency graph | resolved imports | bidirectional, with cached dependent counts |
| File store | files | one small entry per file; content is **not** kept |
| Mapped content | files > 1 MiB read by searches | the "mapped" figure in the UI; released when a file is removed |

Small files are read into owned buffers per access and dropped
afterwards, so search traffic does not accumulate memory. The CI corpus
(6,237 files, 38.9 MB) sits at about 120 MB resident after a build.

## Limits that protect the server

| Limit | Value | Where |
|---|---|---|
| Page size | 1 to `search.max_page_size` (1,000); default `search.default_page_size` (50) | `max` |
| Offset | ≤ `search.max_page_offset` (100,000) | `offset`; deeper pages are refused |
| Files read | best first; `search.fast_rank_files` (no limit by default), up to `search.max_fast_rank_files` on request | `max_files` |
| Match budget | `search.min_match_budget` (32,768) to `search.max_match_budget` (100,000), `search.match_budget_multiplier` (8) × page end; about 230 bytes per match while a search runs | stops the scan; reported as `truncated_by_budget` and `limits_hit` |
| Deadline | `timeout_ms` or just under `request_timeout_secs` | the scan stops on its own even if the HTTP response was abandoned |
| Context lines | ≤ `search.max_context_lines` (10) per hit (`/api/context`: ≤ 200) | `context` |
| Concurrent searches | `max_concurrent_searches`, shared by REST and gRPC | beyond it: 503 / `RESOURCE_EXHAUSTED` immediately |
| Regex | compiled size and DFA size limits | absurd patterns are a 400 |
| Per-file hits | `search.per_file_matches` (100) per document per search, raised to the page end | keeps a broad regex bounded |
| File size | `max_file_size` (default 10 MB) | larger files are not indexed |
| WebSocket frames | 4 KiB inbound | `/ws/progress` ignores inbound data |

When a limit cuts a search short, the response names it in `limits_hit`
(`max_files`, `match_budget`, `per_file_cap` or `timeout`, each with a
sentence for people). gRPC sends the names in the `fcs-limits-hit`
response header, `fcs` prints them on stderr, and MCP adds a "Results
capped" line, so a capped result never passes for a complete one.

None of these need tuning for normal use; they exist so a bad query or an
abandoned client cannot pin the machine. The defaults are measured; see
[Search limits](../benchmarks/search-limits.md). MCP's own limits are in
`[mcp]` (`search_code` 50 hits by default, at most 1,000, paged with
`offset`).
