# Memory and limits

## What is resident

| Structure | Grows with | Notes |
|---|---|---|
| Trigram posting lists | distinct trigrams × files | Roaring bitmaps; run-optimised after a build; after a load read in place from the mapped index file (page cache, not heap), except lists indexed since |
| Symbol tables | definitions | per file, restored from the saved index; plus a dictionary of distinct names, built after a build and saved with the index |
| Reference tables | call sites and type mentions | names interned once; positions packed |
| Dependency graph | resolved imports | bidirectional, with cached dependent counts |
| File store | files | one small entry per file; content is **not** kept |
| Mapped content | files > 1 MiB read by searches | the "mapped" figure in the UI; released when a file is removed |

Small files are read into owned buffers per access and dropped
afterwards, so search traffic does not accumulate memory. The
[latest results](../benchmarks/latest.md) give the CI corpus's resident
memory after a build.

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
Checked the same way on public code (tokio + Django at `b1f9287`, the
Linux kernel re-measured after symbols were packed, C1.6). A loaded index reads
its posting lists in place from the mapped index file, so they are not
heap; the mapped bytes are reported apart (`mapped_postings_bytes`) and
are page cache the system can evict and share:

| Corpus | Index | Live heap | Estimated | Posting lists mapped |
|---|---|---|---|---|
| tokio + Django (6,200 files) | just built | 75.2 MB | 46.4 MB (62%) | none |
| tokio + Django | loaded from disk | 19.4 MB | 18.7 MB (96%) | 12.6 MB |
| Linux kernel (85,700 files, 1.23 GB) | just built | 422.6 MB | 379.4 MB (90%) | none |
| Linux kernel | loaded from disk | 161.9 MB | 157.2 MB (97%) | 149.3 MB |

The Linux kernel, loaded from disk:

| Part | Estimated |
|---|---|
| symbols | 113.9 MB |
| graph_paths | 14.0 MB |
| file_table | 12.7 MB |
| file_metadata | 7.8 MB |
| postings (on the heap: the table and lists indexed since the load) | 4.0 MB |
| references | 3.2 MB |
| everything else | 1.7 MB |

**Each symbol name is held once.** A symbol is 16 bytes (its name's
number, line, column, kind and whether it defines the name), and the
distinct names sit back to back in one string with a hash table over them.
Before, a symbol was 48 bytes plus its own copy of its name: the Linux
kernel's loaded symbols took 239.0 MB, and on the 302,000-file corpus
symbols took 405 MB and resident memory after a build was 1,490 MB, now
212 MB and 1,152 MB (same machine, 3 rounds). Names no symbol uses any
more (after edits) are kept until they outnumber the used ones, then the
table is rebuilt without them.

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
Django it is 75 MB against 19 MB. Most of the gap is the posting lists,
which a loaded index reads from the mapped file. The rest is spare
capacity: posting lists built one insert at a time keep it (a list in one
container costs 168 bytes instead of 42), and the estimates cannot see it,
which is why they cover only 62% there. A restart gives the memory back.

Resident memory is higher again. A build allocates and frees far more
than the index it leaves behind (file contents, per-batch trigram sets,
merge buffers), and the allocator keeps some of the freed pages.

**The allocator.** The server, `fcs` and the benchmarks use
[mimalloc](https://github.com/microsoft/mimalloc) (its v2 line) instead of
the system allocator, on every platform. Queries allocate a lot (result
lists, decoded posting lists, file contents), and with glibc's allocator a
server that had just built its index answered about 2x slower than the
same server after a restart: on the 302,000-file corpus a common word took
161 ms in the building process and 77 ms in a fresh one, for the same
instructions. It was the allocator's state after the build, not the
index's layout: a copy of the index loaded into the building process was
just as slow. With mimalloc, same machine, base `a708ca2`:

| Corpus | Common word | Identifier | Regex with literal | Build | Resident after build |
|---|---|---|---|---|---|
| tokio + Django (5 rounds) | 47.4 → 38.2 ms | 15.2 → 10.7 ms | 40.1 → 36.6 ms | 5.30 → 4.79 s | 100 → 117 MB |
| 302,000 files (1 round) | 181 → 87 ms | 78 → 44 ms | 110 → 75 ms | 189 → 176 s | 1,534 → 1,487 MB |

jemalloc was as fast but does not build for Windows; mimalloc's v3 line
held 50% more memory on tokio + Django. Once a build or a load is done the
server has the allocator hand unused memory back (mimalloc's `mi_collect`,
and `malloc_trim` for glibc), outside the write lock. Before the switch,
`malloc_trim` alone measured:

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

**Known limitations:** on a small index mimalloc holds a little more
resident memory than glibc did (17 MB more on tokio + Django); on a large
one it holds less. Resident memory after a build stays higher than after a
restart. The breakdown leaves out the allocator, thread stacks,
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
