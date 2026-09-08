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
| Page size | 1–1,000 | `max` |
| Offset | ≤ 10,000 | `offset`; deeper pages are refused |
| Match budget | derived from page size and offset, capped | stops the scan; reported as `truncated_by_budget` |
| Deadline | `timeout_ms` or just under `request_timeout_secs` | the scan stops on its own even if the HTTP response was abandoned |
| Context lines | ≤ 10 per hit (`/api/context`: ≤ 200) | `context` |
| Concurrent searches | `max_concurrent_searches`, shared by REST and gRPC | beyond it: 503 / `RESOURCE_EXHAUSTED` immediately |
| Regex | compiled size and DFA size limits | absurd patterns are a 400 |
| Per-file hits | 100 per document per search | keeps a broad regex bounded |
| File size | `max_file_size` (default 10 MB) | larger files are not indexed |
| WebSocket frames | 4 KiB inbound | `/ws/progress` ignores inbound data |

None of these need tuning for normal use; they exist so a bad query or an
abandoned client cannot pin the machine.
