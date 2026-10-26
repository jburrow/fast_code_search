# Memory and limits

## What is resident

| Structure | Grows with | Notes |
|---|---|---|
| Trigram posting lists | distinct trigrams × files | Roaring bitmaps; run-optimised after a build |
| Symbol tables | definitions | per file, restored from the saved index; plus a dictionary of distinct names, built after a build and saved with the index |
| Reference tables | call sites and type mentions | names interned once; positions packed |
| Dependency graph | resolved imports | bidirectional, with cached dependent counts |
| File store | files | one small entry per file; content is **not** kept |
| Mapped content | files > 1 MiB read by searches | the "mapped" figure in the UI; released when a file is removed |

Small files are read into owned buffers per access and dropped
afterwards, so search traffic does not accumulate memory. The CI corpus
(6,237 files, 38.9 MB) sits at about 120 MB resident after a build.

## Where it goes

`/api/diagnostics` reports the index's estimated heap per structure under
`index.memory`, largest first, next to the process's resident memory
(`index.resident_bytes`, Linux only). `corpus_bench` prints the same
breakdown after a build, and the [scale scoreboard](../benchmarks/scoreboard.md)
records it per scenario.

| Part | What it holds |
|---|---|
| `postings` | trigram posting lists (Roaring bitmaps) and their table |
| `references` | symbol references and their interned names |
| `symbols` | symbol definitions and the name dictionary |
| `imports` | imports as written, and those still waiting for a file to resolve to |
| `graph_paths` | path tables the import resolver looks paths up in |
| `graph_edges` | dependency edges and dependent counts |
| `file_table` | indexed paths and the path lookup |
| `file_metadata` | ranking metadata and display paths |
| `dir_index` | files by folder, for the watcher |

The figures are estimates from lengths and capacities, not allocator
measurements (`src/heap_size.rs`). `tests/heap_estimates.rs` keeps them
honest: it counts every allocation and fails if the estimates for a loaded
index fall outside 75% to 105% of the heap it really holds.
Checked the same way on public code (commit after `d62cc56`, with one
path per file):

| Corpus | Index | Live heap | Estimated |
|---|---|---|---|
| tokio + Django (6,200 files) | just built | 76.3 MB | 48.3 MB (63%) |
| tokio + Django | loaded from disk | 43.7 MB | 39.9 MB (91%) |
| Linux kernel (85,700 files, 1.23 GB) | just built | 575.1 MB | 533.2 MB (93%) |
| Linux kernel | loaded from disk | 479.7 MB | 470.7 MB (98%) |

The Linux kernel, loaded from disk:

| Part | Estimated |
|---|---|
| symbols (46.5 MB of it the name dictionary) | 253.8 MB |
| postings | 172.3 MB |
| graph_paths | 17.2 MB |
| file_table | 14.7 MB |
| file_metadata | 7.8 MB |
| references | 3.2 MB |
| everything else | 1.8 MB |

**Each path is held once.** The file store interns every file's canonical
path; the ranking metadata (its root-qualified path is an offset into that
path), the import resolver's path tables and the import lists all share
that one allocation, so `file_table` counts the paths and the others count
only their own tables. On the 302,000-file real corpus this took resident
memory after a build from 1,665 MB to 1,504 MB (same machine, 3 rounds).
The lowercase file-name stem is the exception: it is kept inside the
metadata (up to 23 bytes; 85% of stems fit), because every candidate file
of a search reads it and reading it through the path cost a cache miss per
file.

Known limitations: on Windows the metadata keeps its own copy of the
root-qualified path, since it is spelled with `/` and the stored path with
`\`. A search with a path filter reads every candidate's path from the
file store: on an index just built (paths spread through the heap among
the build's other allocations) that measured 19% slower on 302,000 files
(95 to 113 ms); on an index loaded from disk, where the paths are
allocated in order, it is as fast as before.

**A just-built index holds more than the same index loaded.** On tokio +
Django it is 76 MB against 44 MB. Posting lists built one insert at a time
keep spare capacity (a list in one container costs 168 bytes instead of 42),
and the estimates cannot see that spare capacity, which is why they cover
only 63% there. A restart gives the memory back.

Resident memory is higher again. A build allocates and frees far more
than the index it leaves behind (file contents, per-batch trigram sets,
merge buffers), and the allocator keeps the freed pages. On Linux (glibc)
the server hands them back to the system once a build or a load is done
(`malloc_trim`), outside the write lock:

| Corpus | Resident before | After | Took |
|---|---|---|---|
| tokio + Django | 135 MB | 107 MB | 8 ms |
| Linux kernel | 793 MB | 709 MB | 36 ms |
| Linux + LLVM + Rust + Kubernetes (302,000 files) | 1,718 MB | 1,613 MB | not timed |

What is left above the live heap is fragmentation: freed memory that
shares pages with live data. Compacting the posting lists after a build
(reallocating each at its exact size) was tried. It did not lower resident
memory and made finalize about 200 ms slower on tokio + Django, so it is
not done. `corpus_bench` shows everything outside the estimates as "not
estimated".

**Known limitation:** on macOS, Windows and musl builds the freed memory
is not handed back, so resident memory after a build stays higher there
than after a restart. The breakdown leaves out the allocator, thread stacks,
mapped content, the regex cache and spare capacity inside posting lists, so
it reads low, most of all just after a build. Use it to see which part grew
between two runs, not as an exact total.

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
| Per-file hits | `search.per_file_matches` (1,000) per document per search, its best by score; the budget from result 1000 on | keeps a broad regex bounded |
| File size | `max_file_size` (default 10 MB) | larger files are not indexed |
| WebSocket frames | 4 KiB inbound | `/ws/progress` ignores inbound data |

When a limit cuts a search short, the response names it in `limits_hit`
(`max_files`, `match_budget`, `per_file_cap`, `timeout` or `indexing`, each with a
sentence for people). gRPC sends the names in the `fcs-limits-hit`
response header, `fcs` prints them on stderr, and MCP adds a "Results
capped" line, so a capped result never passes for a complete one.

None of these need tuning for normal use; they exist so a bad query or an
abandoned client cannot pin the machine. The defaults are measured; see
[Search limits](../benchmarks/search-limits.md). MCP's own limits are in
`[mcp]` (`search_code` 50 hits by default, at most 1,000, paged with
`offset`).
