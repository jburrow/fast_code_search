# Performance and memory

What the numbers depend on, and the knobs that move them. Measured values
are in [Latest results](../benchmarks/latest.md).

## Build time

Symbol extraction with tree-sitter is about three quarters of a full
build; trigram extraction and reading are the rest. The build uses every
core (rayon); `RAYON_NUM_THREADS=4` caps it if the machine must stay
responsive. Searches are served between batches while a build runs.

A persisted index (`index_path`) turns a restart into a load of a few
seconds: the file is memory-mapped, checked, and reconciled against the
tree by one `stat` per file plus an eligibility pass. Only files that
changed are re-read.

## Memory

Resident memory after a build is roughly: the trigram posting lists
(Roaring bitmaps, a few dozen bytes per file per distinct trigram), the
per-file symbol and reference tables, the dependency graph, and whatever
file content searches have recently read. Small files (≤ 1 MiB) are read
into owned buffers per access, so they do not stay resident; larger files
are memory-mapped and counted in the "mapped" figure the UI shows. The
CI corpus (tokio + Django, 38.9 MB of text) sits at about 120 MB resident;
a 60k-file, 545 MB workspace at about 500 MB.

To reduce it: exclude generated and vendored trees, cap `max_file_size`,
and restrict `include_extensions`.

## Search latency

- **Candidates.** A query's trigrams intersect posting lists; the fewer
  and rarer the trigrams, the smaller the candidate set. Short queries
  (under three characters) have no trigrams and scan everything.
- **Reading order.** Candidates are read best first by file metadata until
  the match budget is spent. A server can cap the files read
  (`search.fast_rank_files`; none by default); the web UI's SEARCH ALL FILES
  (`max_files` on the API, `fcs --max-files`) opens more for one search, up
  to `search.max_fast_rank_files`.
- **Budget and deadline.** Every search stops at a match budget (32,768
  matches for a first page by default, about 230 bytes each while the search
  runs) and at a deadline derived from `request_timeout_secs` (or a smaller
  `timeout_ms`), reporting `truncated_by_budget` when it did. On a 372k-file
  corpus a first page took about 80 ms (median) for words with over 100k
  hits; [Search limits](../benchmarks/search-limits.md) has the
  measurements behind each default. On a small machine serving many people,
  `search.min_match_budget` trades page-1 quality for memory and time.
- **Regex.** Literals extracted from the pattern pre-filter through the
  index; a pattern with no required literal scans every file. Line-mode
  patterns run in one pass per file.
- **Concurrency.** `max_concurrent_searches` (shared by REST and gRPC) bounds
  the blocking threads; beyond it the server answers 503 with `Retry-After`
  rather than queueing.

## Incremental updates

The watcher coalesces events for two seconds and applies a batch under one
write lock: deletes in one pass over the posting lists, modified files
stripped in one pass and re-read in parallel. On Linux one inotify watch is
installed per kept directory; excluded trees get none. If the limit is
hit the log says so; raise it with
`sysctl fs.inotify.max_user_watches=524288`.

## Disk

The index file is roughly half the size of the text it covers (21.5 MB
for 38.9 MB in CI). Saves are atomic (unique temp file, fsync, rename) and
serialized, so a crash mid-save leaves the previous file intact.
