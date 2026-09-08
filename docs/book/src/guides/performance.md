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
- **Fast versus full ranking.** Above 5,000 candidates the engine reads only
  the top 2,000 by file metadata (`rank=auto`); `rank=full` reads them all.
  Files containing a multi-term query's phrase are always among those read.
- **Budget and deadline.** Every search stops at a match budget and at a
  deadline derived from `request_timeout_secs` (or a smaller `timeout_ms`),
  reporting `truncated_by_budget` when it did. Narrow the query (`file:`,
  `lang:`, a longer literal) rather than raising limits.
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
