# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- **A loaded index keeps its posting lists in the index file.** The
  server maps the file and a query decodes only the lists for its own
  trigrams, so the lists sit in the page cache, which the OS can evict and
  share, instead of on the heap: at 302,000 files a loaded index holds
  6 MB of posting data on the heap instead of about 479 MB (350 MB of the
  file is mapped instead), and loads 17% faster (4.1 s to 3.4 s, same
  machine, median of 3). Query times are unchanged. Removing or changing a file
  costs one bit for the mapped lists instead of a pass over every list.
  Saves while the server runs (checkpoints) now keep every file's id,
  writing a removed file as an empty slot, copy unchanged lists from the
  mapped file as they are, and leave the server reading the file they
  wrote; the save at shutdown compacts the slots away. `index.memory` in
  `/api/diagnostics` reports the mapped bytes as `mapped_postings_bytes`.
  Index format v11 (the first start after upgrading rebuilds the index
  once). Known limitation: on Windows, which cannot replace a mapped file,
  the lists are still decoded onto the heap at load.
- **Loading a saved index is about 2.5 times faster.** With one file
  changed since the save, a load of the 302,000-file real corpus took
  10.3 s and now takes 4.1 s (same machine, median of 5). A load used to
  renumber the files that survived, which rebuilt every posting list when
  even one file had changed; each saved file now keeps its id and the
  changed and deleted ones are tombstoned, as a removal at runtime is.
  From the same profile: the checksum is computed eight bytes at a time,
  path tables use a faster hash, directory counts are added once per
  directory, symbols are moved rather than copied, and the ranking
  metadata pass runs on every core.
- **Restarts read only the folders that changed.** After loading a saved
  index, the server looked for new files by walking every folder under the
  roots: at 302,000 files, 2.6 s with the file system cache warm and 9.2 s
  cold, while answers said the index was incomplete. The last complete walk
  now saves each folder's modification time and a fingerprint of its
  ignore files, and a restart reads only the folders whose time or ignore
  files changed: 0.18 s warm, 1.3 s cold. A full walk still happens after
  a configuration change, a change to the global excludes file, or when
  the index has no stamps yet. Index format v10 (now v11, above): the
  first start after upgrading rebuilds the index once. Known limitation: on a file system
  that does not update a folder's time when its entries change, a file
  added while the server was stopped is found only once it or its folder
  changes again.
- **About 10% less memory on large trees: each file's path is held once.**
  The ranking metadata, the import resolver's path tables and the import
  lists kept their own copies of every path (five to eight per file); they
  now share the file store's one copy. On 4 cores the 302,000-file real
  corpus (Linux, LLVM, Rust, Kubernetes) sits at 1,504 MB resident after a
  build instead of 1,665 MB, and a loaded Linux kernel index holds 35 MB
  less heap (468 to 433 MB, counted by allocation). Results are identical. One slowdown, named:
  a search with a path filter (`file:`, include/exclude globs) on an index
  that was just built measured 19% slower on the large corpus (95 to
  113 ms), because the paths it reads are spread through the heap; on an
  index loaded from disk it is as fast as before. Other searches measured
  2 to 12% faster.
- **Searches with many candidate files sort only what they read.** Every
  search reads its candidates best first and usually stops at its match
  budget a thousand or two files in, but it sorted all of them first: at
  302,000 candidates, scoring and sorting took 31 to 34 ms. The order is
  now settled lazily (a selection of the next part, then a sort of just
  that part), with the same order. Same-machine runs at 302,000 files:
  short-text search -15%, regex with a literal -10%; other queries within
  noise.
- **Symbol search about 12 times faster on large trees.** It tested every
  symbol of every file whose text contained the query: for `Error` on the
  302,000-file real corpus (Linux, LLVM, Rust, Kubernetes) that was 5.3
  million symbols in 85,000 files, to find the 7,000 files defining a
  matching name. A dictionary of the distinct case-folded symbol names
  (1.6 million, one string searched with `memmem`) now lists the files and
  symbol kinds behind each name, and a case-insensitive search ranks files
  from it directly; a case-sensitive one tests only those files. Results
  are the same. The lines a match is shown on are found without splitting
  the whole file into lines. Same-machine runs: `Error` 522 to 42 ms and
  `Req` 589 to 42 ms at 302,000 files, 22 to 9 ms and 15 to 6 ms on tokio +
  Django.
  **What it costs:** the dictionary takes memory (63 MB at 302,000 files,
  46 MB for the Linux kernel; resident +4%), and it is built after every
  build on one thread (about 2 s at 302,000 files; on all cores it took
  half the time but fragmented memory: resident grew 160 MB). It is saved
  with the index, so a load restores it instead (index format v9: the
  first start after upgrading rebuilds the index once). Files changed after the
  dictionary was built are searched symbol by symbol until a sixteenth of
  the files (at least 4,096) have changed; the update that crosses that
  line rebuilds it under the write lock.
- **Builds and large watcher batches up to twice as fast.**
  Merging a batch into the index ran on one thread and was half of a
  build's time; posting-list inserts were four fifths of that. The posting
  lists are now split into 64 shards by trigram and each batch is merged
  shard by shard in parallel. On 4 cores the 302,000-file real corpus
  (Linux, LLVM, Rust, Kubernetes) builds in 211 s instead of 471 s, tokio +
  Django in 5.6 s instead of 8.1 s, with the same index and the same
  memory. Searches measured 0 to 13% slower in same-machine runs, on a
  just-built and on a reloaded index alike (tokio + Django; up to 19% for
  one query on the large corpus, from 3 rounds). In one 5-round run the
  common-word search was slower in every round (+11%); in a second, no
  query was, and a regex that never touches the changed code moved as much
  as any. So most of it is noise or code layout, but a small real cost is
  not ruled out.
  `corpus_bench --loaded-queries` now also times the queries on the
  reloaded index.
- **Less memory after a build or a load (Linux).** A build frees far more
  than the index it keeps (file contents, per-batch trigram sets, merge
  buffers), and the allocator held on to those pages, so resident memory
  stayed high until a restart. The server now hands them back to the
  system once a build or a load is done: tokio + Django went from 135 MB
  to 107 MB resident, the Linux kernel tree from 793 MB to 709 MB, in 8 and
  36 ms, outside the write lock. glibc only; macOS, Windows and the musl
  build are unchanged.

### Added
- **Where the memory goes.** `/api/diagnostics` now reports the index's
  estimated heap per structure (`index.memory`: posting lists, the file
  table, symbols, references, the dependency graph, imports), largest
  first, next to the process's resident memory (`index.resident_bytes`,
  Linux). `corpus_bench` prints the same breakdown as a table, with a
  "not estimated" row for the allocator's share, and reports each part as
  a benchmark metric. The scale scoreboard records it per scenario. A
  change that grows memory now shows which structure grew. Measured this
  way, a just-built index holds more than the same index after a restart
  (tokio + Django: 78 MB against 44 MB of heap), because posting lists
  built by inserts keep spare capacity.
- **An instruction-count gate on every push to `main`.** The Benchmarks
  workflow now counts, under callgrind, the instructions each operation
  takes on tokio + Django (build, every query, an incremental update,
  save, load) and fails when one takes more than 3% more than at the last
  passing push (`scripts/bench/icount.sh`, `corpus_bench --icount`).
  Counts barely move between runners, unlike wall time, so added work
  shows the day it lands. Counts are published only from a passing run,
  so the run stays red until the work is removed, or accepted with an
  `Accept-instructions:` line in the commit message. The Criterion suites
  no longer run in CI (they still run with `cargo bench`), and the job
  takes about as long as before (18 minutes on its first run, 20 before).

### Fixed
- **The symbol-search benchmark timed a search no front end runs.** The
  library's `search_symbols` (used by the corpus benchmark, the validator
  and tests) ran an older single-pass search; the web API, gRPC, MCP and
  `fcs` run the two-phase one. `search_symbols` now runs that one too, so
  the published symbol numbers are what a user waits for: on the
  302,000-file corpus about 510 ms for an exact name and 580 ms for a
  prefix, not the 350 ms the benchmark showed (tokio + Django: 19 and
  13 ms, not 13 and 8). Nothing a user runs got slower; the benchmark now
  measures it. Making these faster is the next item (a symbol name index).
- **`fcs check --live --all-content` was cut off on a large index.** The
  server's whole-request timeout (`request_timeout_secs`, 30 s) applied to
  `/api/check`, so a check that read every file of a large tree answered
  504 while the client was ready to wait an hour. The check runs one at a
  time and only reads, so it is no longer timed out. Found by the scale
  scoreboard on a 200,000-file tree with real-sized files.
- **Folders that could not be read went missing without a word.** When
  the build, a restart's check or a rescan could not list a folder
  (permission denied, an I/O error), it logged a debug line and skipped
  it, so its files were missing from every search. Each such folder is
  now logged as a warning and counted with every search: "1 folder could
  not be read, so the files in it are missing from the index", in the web
  UI ("1 folder unreadable"), `fcs`, MCP, `/api/status` and `fcs check
  --live`. The count clears when a later scan (a restart, a rescan) can read the
  folder.
- **Folders the watcher could not watch were not reported.** A folder the
  file watcher could not list was dropped silently, and one it could not
  watch (the OS watch limit) was only logged. Both are now counted while
  watching ("2 folders not watched"): changes in them reach the index only
  at the next rescan or restart.
- **A file-watcher error now triggers a rescan.** An error from the OS
  watcher (its event stream failed) was logged and nothing else, so the
  events it lost were lost for good. The watched folders are now
  reconciled with the disk, at most once every five minutes so a repeating
  error cannot keep a rescan of a large tree running.

### Documentation

- **Benchmark numbers in the README, the book and the site are rendered,
  not typed.** They come from one pinned Benchmarks run
  (`docs/bench/published.json`), now the run for `c635718`, which includes
  the faster build: tokio + Django builds in 4.9 s instead of 5.5 s. CI
  fails when a page and the pinned run disagree. The file-format page said
  format v7; the index has been v8 since v0.16.0.
- **Benchmark numbers now match what fcs does.** The README, the book's
  benchmark pages and the project site showed numbers from 8 September
  (before v0.17.0), with a common word at about 1 ms. They now show the
  v0.19.0 run: a common word over tokio + Django is 24.6 ms at p50, an
  identifier 7.8 ms, and a query with no match 0.6 µs (since refreshed
  from a later run, see above).
- **The v0.17.0 slowdown, named.** v0.17.0 raised the match budget from
  512 to 32,768, so a search ranks many more matches before it returns
  page one. On a ~6,300-file repository that made common-word text
  searches about 10 times slower (about 5 ms to about 55 ms on the same
  machine) and regexes with no literal about 30 times slower (about 3 ms to
  about 100 ms). It buys ranking quality: page one now holds 81% of the
  exhaustive top 50 instead of 10%. Queries with few matches and index
  builds did not change. See the book's Search limits page.
- The release run-book in `docs/DEVELOPMENT.md` now matches how releases
  are cut, and the instruction files say which docs repeat which numbers so
  they are updated together.

## [0.19.0] - 2026-10-24

Searches keep running while the watcher applies changes, the index checks
itself and survives crashes and full disks, and fcs gains diff impact
(`fcs impact`, MCP `diff_impact`) and diagnostic captures. The web UI's
Ranking setting is gone: every search is ranked the same way.

**Upgrading:** no re-index and no config change is needed. The background
scrub (`[diagnostics] scrub`) is on by default and reads about 3,000 files
a minute; set `scrub = false` to turn it off. `fcs --rank` and `rank` in
the REST and gRPC APIs still work but no longer change the order with
default settings. Release binaries are about 3 MB larger because they keep
function names for readable stacks.

### Changed
- **The web UI's Ranking setting (Auto, Fast, Full) is gone.** With the
  default settings all three gave the same results; only a server with a
  file limit (`search.fast_rank_files`) told them apart. Every search is
  ranked the same way, and **BEST MATCHES FIRST** beside the result count
  now opens a short explanation of the order: how hits are scored, that a
  file's hits stay together, that nothing is hidden, and what happens on a
  very broad search. MCP's `search_code` no longer lists `rank` (an old
  client's value is accepted and ignored) and says how results are ranked
  in its description. `fcs --rank` is hidden but still works, as does
  `rank` in the REST and gRPC APIs. The book's Ranking page has the
  measurements behind the choice: stopping at the first page is up to 50
  times faster on tokio + Django, but for common words that page holds none
  of the 50 best hits.
- **`lock_wait` captures leave out the holding back.** A search holds back
  while an install step waits (up to 2 s, as long as recent searches
  take), and that alone could pass the 2 s threshold. The capture now
  counts only the time a writer kept the search out, so it still means a
  step or a save held the lock longer than it should.
- `rdeps:` and `deps:` filters no longer read every indexed path to find
  their file (when it is given as results print it, with or without `:N`)
  nor copy every unresolved import to count the broken ones. On a
  348,000-file index the filter's own cost fell from 464 ms to 0.1 ms
  median (`examples/rdeps_timing.rs`); a partial path or a folder still
  reads every path.
- The server accepts request bodies up to 8 MiB (was 64 KiB), for diffs.

### Fixed
- **Building the index got about 30% slower over October, mostly for
  nothing.** A profile on tokio + Django put 42% of the build's CPU in
  Python import resolution and another fifth of Django's in filtering
  Python references, not in parsing. For every absolute import (most of
  them the standard library), resolution built and normalized candidate
  paths under every folder above the importing file; it now asks one
  folder lookup whether anything is indexed under `root/<first name>`
  before building any. The reference filter looked each reference up from
  the root of the syntax tree and climbed with a call that itself walks
  down from the root; it now walks each file's tree once. Results are
  unchanged (same 9,784 graph edges and 384,986 references; the new
  filter keeps exactly what the old one kept for all 492,103 identifiers
  in Django). Import resolution went from 2.4 s to 0.85 s, the whole
  build from 8.1 s to 6.7 s on the same machine (0.15.2: 6.1 s; the rest
  is the extra Python references and case folding). The corpus benchmark
  now reports the time of each build phase.
- **A search for a common word spent a third of its time sorting hits it
  never returned.** Every hit (22,000 for `return` on tokio + Django) was
  sorted, then sorted again by file, to return one page; and each
  candidate file's src/test path signals were worked out again on every
  search. Now each file's best hit places the file, only the files the
  page reaches have their hits sorted, and the signals come from the
  file's precomputed metadata. Results are identical (53,810 results
  compared across 14 queries, both ranking modes and pages down to
  offset 20,000); `return` went from 51 to 39 ms, a filtered search from
  28 to 21 ms, a regex with no literal from 76 to 61 ms.
- **The count of changes still to apply counted events, not files.** One
  save raises several file-system events, and each was counted, so the
  web page's "N file changes to apply" (and the same count in `fcs`, MCP
  and `/api/status`) overstated the backlog: 372,534 for 250,000 changed
  files in a 500,000-file scale run. Each file now counts once until its
  change is applied.
- **`class ModelForm` put `class ModelFormOptions:` first.** Since plain
  searches match like `rg -F` (0.15.2), a query is also found inside longer
  names, and the shorter line won. A whole-word match now scores 1.5x, so
  the line that has what was typed comes first; the ranking benchmark is
  back to 20 of 20 at rank 1.
- **The benchmark history stopped on 13 Oct 2026.** The ranking check in
  the Benchmarks workflow failed from that release on, and its failure
  skipped the steps that store the numbers, so the trend page stopped
  without a warning. The numbers are now stored first and the check fails
  the run after.
- **Loading an index with one changed file took about 70% longer.**
  When any saved file had changed or gone, loading rebuilt every trigram
  posting list one id at a time through a hash lookup. Now that a file
  saved within seconds of an edit is re-read on load (below), the corpus
  benchmark hit this on every run, as does a restart soon after an
  edit. The lists are now rebuilt in one sorted pass from a lookup table,
  in parallel: on tokio + Django, load after 20 edits went from about
  560 ms to 365 ms (a clean load is about 310 ms). Saves after deletes
  use the same code.
- **Every save killed half way left a file as large as the index.** A
  save writes a temp file next to the index and renames it into place; a
  server killed during the save (`kill -9`, a power cut, the OOM killer)
  left the temp file, and nothing removed it. The server now removes them
  when it starts.
- **A save that failed was forgotten.** With `save_after_updates`, the
  count of unsaved updates was reset whether the save worked or not, so
  after a save onto a full disk the shutdown skipped its save and the
  saved index lacked those updates (the next start re-read the files, so
  no search was wrong, but it took longer). The count now stays until a
  save works.
- **A failed rename could delete the index.** When moving the new index
  into place failed, the old index was deleted before trying again, so a
  second failure left no index at all. The old index is now moved aside
  and put back if the second try fails.
- **A panic while reading a file left it out of the index without a
  word.** During the build, a file whose reading or parsing panicked was
  skipped and not reported. It is now tried again at the end of the
  build, and reported as unreadable if it keeps failing, like a file that
  cannot be read.
- **Every search waited 4 to 5 s soon after a restart.** An index loaded
  from disk keeps its unresolved imports (`import os`, packages outside
  the indexed folders) but not each file's import list. At the end of the
  restart's check of the tree, each of those imports got one last try, all
  at once under the index lock: 368,000 of them on a tree of 200,000
  files. They are now tried in short steps like the rest of the import
  graph; the slowest search after a restart fell from 4.0 s to under 1 s.
- **The `backlog` diagnostic capture fired under steady edits.** It fired
  when changes had been pending for 300 s without a break, which a steady
  stream of edits always causes. It now fires only when the watcher could
  not have applied what is pending in the last 10 s.
- **The watcher could fall far behind while people searched.** While an
  install step waits for the index, new searches hold back so the running
  ones can finish and the step gets in. They held back a fixed 250 ms, so
  with searches longer than that a new one got in before the running ones
  ended and the step kept waiting: in a simulation with four people
  running half-second searches back to back, a change that takes 5 s to
  install took up to 62 s. A search now holds back for one and a half
  times the 90th percentile of recent searches' length (250 ms to 2 s);
  the same change takes about 10 s, and quick searches still wait only
  250 ms behind one slow one. The docs claimed "half speed or better"
  regardless; the book's "Incremental updates" page now has the table.
- **The watcher could index files the build leaves out, by ignore rules.**
  When a file changed, the watcher decided whether it was ignored
  differently from the walk that builds the index (and from ripgrep): a
  rule deeper down (`!a/b/` after `/a`) let files back in from a folder
  that is ignored; the nearest ignore file decided, where a parent's
  `.ignore` or `.rgignore` rule should beat a `.gitignore` next to the
  file; and rules in folders above a nested repository were not applied
  inside it. Such files became searchable after an edit and disappeared
  again at the next restart. The check now follows ripgrep's rules, and
  both are compared with `rg --files` on random trees of ignore files.
- **Folders ignored by a parent's `.gitignore` stayed out only until
  something happened in them.** With `sub/` in the root `.gitignore`, the
  watcher indexed everything in `pkg/sub` when the folder was created,
  renamed in, had an ignore file written in it, or was rescanned after
  lost events: a walk never checks whether the folder it starts from is
  ignored. It now does.
- **A restart re-indexed changed files that are now ignored.** A file that
  changed while the server was down was re-read without checking the
  ignore rules (only the size cap and excludes), so a file a new
  `.gitignore` rule covered came back if it had also changed. Changed files
  now go through the same rules as discovery.
- **A file rewritten within the same second could stay stale.** On file
  systems with coarse timestamps (FAT, HFS+, some network shares), a file
  rewritten with the same size in the same tick it was indexed kept its
  old content in the index across a restart or a rescan after lost events,
  because its size and time had not changed. A file read within 3 seconds
  of being modified is now read again by the next rescan or restart, as
  git does with "racily clean" files.
- **Short queries missed very small files.** A file shorter than three
  bytes with a name shorter than three letters (`x.py` holding `ab`) has no
  trigram, and queries without a trigram of their own (two characters, a
  regex with no literal) scanned only files some posting list named, so rg
  found the line and fcs did not. The index now keeps the set of all its
  files, after a load and incremental changes too.
- **`indexer.extra_binary_extensions` works.** The matching reference
  documented it, but the configuration rejected it as an unknown key. Every
  file walk and eligibility check now takes its settings from one place.
- **Rewriting a large file while it was searched could crash the server.**
  Files over 1 MB were memory-mapped, and when another program truncated
  and rewrote one while a search read it, the server died with SIGBUS.
  Files are now read into memory when searched, so nothing can change
  under a search. The 3-second "vm.max_map_count is very low" pause at
  startup is gone with the mapping.
- **No more "server unavailable" at the end of a large build or while a
  saved index loads.** Resolving the dependency graph at the end of a
  build held the write lock for seconds (about 6 s on 300,000 files), and
  loading a saved index held it for the whole load, so searches were
  turned away. Both now work outside the lock and hold it only for short
  steps or the final swap; a 300,000-file cold build and restart answered
  every search.
- **Ignore files behave like ripgrep's.** `.rgignore` is honoured, and
  when a `.gitignore`, `.ignore` or `.rgignore` changes (a branch switch,
  say) the watcher re-checks the files under it, so newly ignored files
  leave the index and newly allowed ones join it. Before, each file kept
  its old state until it changed itself.
- **A file that can't be read is retried and reported, not silently
  dropped.** A read that failed (a file locked by an editor or virus
  scanner, permissions changing mid-save) removed the file from the index
  until it next changed, and the initial build left such a file out
  without a word. The watcher now keeps the old entry and retries for
  about half a minute; then the file is left out and every search says
  how many files could not be read. A panic while applying watcher
  changes used to skip the whole batch; the batch is now applied again,
  and a change that fails twice is reported the same way.
- **A large change no longer leaves the index "updating" for minutes
  while people search.** Each watcher step got the lock only after the
  running searches finished and then installed just 512 files, so with
  searches running the watcher applied a few hundred files a second. On a
  200,000-file tree, 100,000 rewritten files were still 17,000 behind four
  minutes later. A step now stays in about as long as it waited, up to
  one second; the same change is applied 10 s after it stops, and on a
  million-file tree 200,000 changed files catch up in about 3.5 minutes,
  with no search turned away. `examples/watch_load.rs` now checks that
  the index matches the disk again after the load.
- **Searches no longer stop while the file watcher applies many changes.**
  One slow search plus any watcher step used to stop every other search
  until the slow one finished, and those still waiting after five seconds
  got "Index is currently being updated". The watcher (and the startup
  build) now take the write lock only in a moment when no search holds it,
  so a search waits for a short install step at most, never for other
  searches. On a million-file tree with 50,000 files rewritten at once,
  searches turned away fell from 3% to none and the slowest search from
  5.1 s to 1.5 s, while the watcher applied 5.6 times as many changes in the
  same time. Measurements are in the book (How it works > Incremental
  indexing).

### Added
- **More ways the tests look for wrong answers.** A model-based test
  (`tests/lifecycle_model.rs`) drives the index through random sequences of
  edits, renames, folder moves, ignore-file changes, lost events, saves,
  restarts and crashes, and after every step compares it with a naive
  model, page by page; it found the first two fixes above. Property tests
  (`tests/search_properties.rs`) compare literal and regex searches with
  ripgrep on random text full of case-folding oddities (`ß`, `ſ`, the
  Kelvin sign, the sigmas, dotted and dotless i). A generator of seeded
  Python codebases with an answer key (`tests/python_codebase.rs`) checks
  the import graph, symbol and reference searches and strict
  `-deps`/`-rdeps` scopes, after a build and after the watcher rewrites the
  tree; grimp and Python check the key itself. Book page: How it works >
  How fcs is tested.
- **Every surface walked to the last hit, and the first page pinned.**
  `tests/contract_walk.rs` runs searches with thousands of hits through
  REST, gRPC, MCP (HTTP and `fcs mcp`), `fcs` and the web page in headless
  Chromium, pressing LOAD MORE and "Show N more hits" until they are gone,
  and every surface must show the same hits in the same order with honest
  counts. `tests/rank_snapshot.rs` keeps the first page of about a hundred
  searches in a file, so a change that reorders results shows in its pull
  request. The ripgrep backtest also runs on an index restarted from disk
  and on one the watcher filled (`--history reload|watch`), with regexes
  made up from the corpus (`--generate-regex`), and
  `tests/discovery_oracle.rs` compares the indexed files and the FILES box
  with `rg`.
- **The import graph is checked on monorepo layouts.** Generated
  repositories shaped like real ones (`scripts/graph-oracle/layouts.py`,
  `ts_layouts.mjs`): Python projects side by side each with its own
  `src/`, a root folder that is a package, a namespace package split over
  two folders, and npm workspaces with tsconfig `paths` and `extends`,
  barrels and `.mts`/`.cts`. grimp and TypeScript give the answer. Every
  edge fcs misses or draws wrongly, there and on Django and Vite, is now
  a known gap listed with its reason and exact count in
  `benches/graph/known_gaps.toml`, instead of a count to stay under. The
  Python oracle (`fcs check` included) now builds one graph across
  `ROOT` and `ROOT/src`, so imports from one into the other are compared
  too.
- **Kill and restart.** `examples/chaos.rs` kills the server binary with
  SIGKILL at random moments (during a build, a load from disk, a stream of
  edits, a periodic save, a burst of changes, and the save at shutdown),
  starts it again and checks that it loads the saved index, leaves no temp
  file, catches up with every change and passes `/api/check`. The "Scale
  scoreboard" workflow runs it nightly and on pull requests that touch
  saving or watching (job "Kill and restart").
- **More fault points** (`--features failpoints`, `src/faults.rs`): out of
  file handles, a file gone between listing and reading, a slow disk, a
  panic while reading or parsing a file, events lost by the OS, a save
  onto a full disk and a failed rename, each with a test in
  `tests/fault_injection.rs`.
- **A scale scoreboard.** `examples/scoreboard.rs` runs the server binary
  over HTTP in seven scenarios (cold build, warm restart, branch switch, a
  ten-minute stream of edits, overloaded searching, saves under load, and
  an event-queue overflow) and reports the same numbers for each: searches
  turned away, latency, time until the index matches the disk, memory,
  CPU, diagnostic captures, crashes and what `/api/check` finds. Targets
  are in `benches/scoreboard/targets.toml`; the "Scale scoreboard"
  workflow runs it at 200,000 files nightly and on pull requests that
  touch locking or watching. Book page: Benchmarks > Scale scoreboard.
- **The lock protocol on a simulated clock.** `fast_code_search::sim` runs
  threads in turns on simulated time, and the engine lock and the
  watcher's install steps read and sleep through it, so
  `tests/lock_sim.rs` runs the real code under minutes of made-up search
  load in about a second, the same on every run and OS. It found the
  watcher fix above, and its tuning table is the evidence for the
  hold-back and `MAX_HOLD` values.
- **The index checks itself.** `fcs check --live` asks the running server
  whether its index agrees with itself, whether indexed files still match
  the disk, and whether it holds the files a fresh walk finds. It prints
  counts only, so the output is safe to share (`--show-paths` for file
  names); `GET /api/check` returns the same as JSON. A background scrub
  (`[diagnostics] scrub`, on by default) reads 3,000 files a minute, about
  0.1% of a core. It logs and re-indexes any file the index has out of
  step, which is a lost change, and checks the index's own consistency
  hourly; the counts are on the diagnostics page. The watch, fault and
  backtest harnesses now run the same checks after every scenario. Book
  page: Guides > Checking a live index.
- **Fault-injection tests.** A `failpoints` cargo feature compiles in
  named fault points (`src/faults.rs`) that tests arm to make a read fail
  or a step panic; `tests/fault_injection.rs` uses them to check that the
  build and the watcher retry and report failures. CI runs them on every
  platform.
- **Diagnostic captures.** With `[diagnostics] capture = true` the server
  notices when it is slow or stuck and writes what it was doing to a JSON
  file: a search slower than 5 s or kept waiting over 2 s for the index
  lock, the watcher or index build making no progress for 120 s, a search
  still running after 120 s, the index lock never free, or a watcher
  backlog lasting 300 s. A capture holds the trigger, the search's phase
  timings, the searches running, the index and watcher state, recent
  watcher bursts, memory and threads, the last log lines and, for a hang,
  every thread's state and stack (`gdb` or `eu-stack` when installed).
  Query text, paths and file names are left out unless
  `include_queries = true`. Captures are rate-limited and capped (50 files,
  100 MB). `fcs diagnose [DIR]` summarizes them, and `/api/diagnostics`
  says where they go. Off by default; thresholds are measured
  (`watch_load --capture-dir`). See the book's "Diagnostic captures" page.
  Release binaries now keep function names (about 3 MB larger) so thread
  stacks and panic backtraces are readable.
- **Diff impact: `fcs impact`, `POST /api/impact`, MCP `diff_impact`.**
  Give it a diff (by default your uncommitted changes) and it lists, for
  each function, class, method, type or constant the diff changes, the files
  that use it, best first, with the use lines: finer than the file-level
  `change_impact`. Functions and classes are traced through importers that
  name them (renames followed), methods and fields by name among every
  importer with files naming the class first. Each use is a recorded
  `reference`, a `code` mention or a `text` mention (strings and comments).
  On Django's 248 most recent commits that change Python, checked against Jedi, it lists 100%
  of the files that use a changed function or class (79.5% of those it
  lists are confirmed by Jedi), in 49 ms median per commit. No re-index is
  needed. See the book's [Diff impact](docs/book/src/reference/impact.md)
  page for what it misses.
- Search responses report file changes the watcher has seen and not
  applied yet (`index.pending_changes`). The web UI shows "Index updating ·
  N file changes to apply"; `fcs` and the MCP tools print it as a note.

## [0.18.1] - 2026-10-16

Small fixes to piped `fcs` paths and MCP totals. No re-index or config
change is needed.

### Fixed
- Piped `fcs` output and `fcs -l` print short paths relative to the current
  folder even when the configuration lists a root as a relative path or
  with `..` in it (they were long but still valid paths).
- MCP `search_code` says "total not known" instead of "the scan stopped
  early" when it has no exact total; the note under it gives the reason
  (for example, the index is still being built).

## [0.18.0] - 2026-10-15

Fixes from the 2026-10-15 review of what fcs promises against what it does.
Paging is now consistent, busy files show their best hits, several silent
cut-offs now say so, and the server keeps its index current by default.

**Upgrading:** file watching is now on by default. A config without a
`watch` key now watches its folders; add `watch = false` under `[indexer]`
to keep the old behaviour. No re-index is needed. A file with many matches now shows
its best-scoring hits rather than its first ones, so result order changes
for broad searches. `search.per_file_matches` now defaults to 1,000 (was
100). Files that were skipped as binary only because of control
characters, and SVG files, are now indexed, as ripgrep searches them.

### Changed
- **`watch` defaults to `true`.** The server follows edits, renames and
  deletes unless the config says `watch = false`, matching what the README
  and guides describe. `--init` writes `watch = true`. The `build/` and
  `dist/` default excludes stay, and remain listed as a difference from
  ripgrep in the matching reference.

### Fixed
- **Offset paging no longer repeats or skips hits.** The per-file cap and
  match budget grew with the page's end, so page two could be ranked from
  a different set than page one (an MCP walk of 305 hits returned 155
  distinct ones). Both are now constant below result 1,000 and constant
  per page beyond it, so consecutive pages join up.
- **A busy file shows its best hits.** The per-file cap kept a file's
  first N matches in line order; it now keeps its N best by score.
- **Searches during index build say so.** Results carry a new `indexing`
  limit (web UI: "index still building"; MCP and `fcs` notes) and no
  exact-looking total.
- **`fcs` piped and `fcs -l` no longer stop silently at 50.** `-l` pages
  through every hit and lists every matching file; other output prints a
  stderr note when more hits exist, with the `--offset` for the next page.
  Piped and `-l` paths are relative to the current folder when the
  configuration's roots are known, so `fcs -l foo | xargs $EDITOR` opens
  the right files.
- **Two indexed folders with the same name no longer collide.** Each root
  gets a distinct label (extended with parent folders when names clash),
  so the viewer opens the right file.
- **Go to definition with watch off** no longer jumps to a line that no
  longer holds the symbol; a cached definition is used only if its name is
  still on that line.
- **Symbol search totals are honest.** When a file limit leaves files
  unread, the total is shown as unknown and the `max_files` limit is
  reported; `full` ranking reads every file.
- **Ranking no longer depends on where the repository sits on disk.**
  The `src/`, `lib/` and test signals use the root-relative path.
- **Test files are demoted by their names too** (`_test.go`, `.spec.ts`,
  `.test.js`, `__tests__/`, `test_*.py` and similar), not only by folder.
- **Ignore rules match ripgrep:** `.git/info/exclude` and the global
  gitignore (`core.excludesFile`) are honoured by indexing and watch.
- **Binary detection matches ripgrep:** a file is binary only if its
  first 8 KB contains a NUL byte. SVG is no longer on the binary
  extension list.
- `case:` and `word:` accept only yes/no style values; anything else is
  searched as text. A query that holds only filters now explains how to
  add text to find.
- The index size statistic counts indexed bytes, not files on disk.
- The web UI no longer merges `Foo.ts` and `foo.ts` into one group.

### Documentation
- The book's matching, search limits, memory and ranking pages and the
  CLI guide describe the new behaviour and every remaining difference
  from ripgrep.

## [0.17.0] - 2026-10-15

Search limits are measured, raised and configurable. Page 1 of a search
for a common word now holds most of what an unlimited search would rank
first, and MCP agents can page through every hit.

**Upgrading:** no re-index is needed. Page 1 of a broad search now ranks
different (better) hits than 0.16.x. If your config sets
`server.fast_rank_files`, `server.max_fast_rank_files` or
`server.max_page_offset`, move them to the new `[search]` section; the
old keys still work but log a deprecation warning.

### Changed
- **Page 1 ranks far more of a broad query's matches.** A first page
  collected 512 matches before ranking (the "match budget"); it now
  collects 32,768. On a 372k-file corpus (Linux, LLVM, Rust, Kubernetes)
  page 1 held 10% of the exhaustive top 50 for words with over 100k hits,
  and now holds 81%, at about 80 ms median. Each collected match costs
  about 230 bytes while the search runs, about 7.5 MB per search at the
  new floor. The measurements are on the book's new "Search limits" page.
- **Every search reads files best first.** Files are read in order of the
  per-file score fast ranking used, for every mode, instead of in
  discovery order below 5,000 candidate files. The budget therefore keeps
  the hits of the most likely files. **Ordering side effect:** with the
  bigger budget and the new order, page 1 of a broad search shows
  different (better ranked) hits than before; the result list is still
  plain rank order, and no hit is hidden.
- **No more 2,000-file stop.** Above 5,000 candidate files, auto ranking
  read only the best 2,000 and stopped, even when those files held too few
  hits to fill the page, so a rare string in a big repository could come
  back nearly empty. Searches now read until the budget is spent or the
  deadline passes. A server can still set a cap (`search.fast_rank_files`),
  reported as `max_files` as before. `rank=full` never stops at a file
  cap; `auto` and `fast` honour one.
- **MCP `search_code`** returns 50 hits by default (was 20), allows up to
  1,000 (was 100), takes `offset`, `max_files` and `rank`, and lists 20
  files whose path matches (was 5). When more hits exist it says the
  offset of the next page.

### Added
- `[search]` configuration section for every search limit, read by the
  web UI, REST, gRPC, `fcs` and MCP: `fast_rank_files`,
  `max_fast_rank_files`, `max_page_offset`, `min_match_budget`,
  `match_budget_multiplier`, `max_match_budget`, `per_file_matches`,
  `default_page_size`, `max_page_size`, `max_path_matches`,
  `max_timeout_secs`, `max_context_lines`. Contradictory values (a page
  size above `max_page_size`, a budget floor above its ceiling) stop the
  server at startup, naming the key. The web UI's LOAD MORE follows the
  server's `max_page_size`.
- `[mcp]` configuration section: `default_limit`, `max_limit`,
  `file_matches`, `max_context_lines`, `read_file_lines`,
  `max_read_file_lines`, `graph_default_limit`, `graph_max_limit`.
- `examples/limits_bench.rs`: measures what each limit costs and how far
  it moves page 1 from the exhaustive ranking, on any corpus.

### Deprecated
- `server.fast_rank_files`, `server.max_fast_rank_files` and
  `server.max_page_offset` moved to `[search]`. The old keys still work and
  log a warning; a `[search]` value wins.

### Fixed
- Symbol search read only the best max(200, 8 x page end) files holding a
  matching symbol, and reported its total as exact. It now reports
  `max_files` with the files it read when there were more.

## [0.16.2] - 2026-10-14

Watch mode keeps answering searches on very large trees, and Python
projects whose imports were listed as external now resolve. Re-index a
Python project to pick up the import fixes.

### Changed
- Web UI: Semantic is no longer in the header nav, since semantic search
  is not being developed for now. The page still works at
  `/semantic.html` (and shows its tab while open).

### Fixed
- Watch mode on large trees no longer answers searches with "index is
  being updated" while changes stream in. Applying a batch held the write
  lock for its whole duration, including reading and parsing the files,
  and a per-directory scan of every indexed path made that seconds long on
  big trees (15 s for one batch on 200,000 files). Now only installing the
  parsed files takes the write lock, in steps of at most 512 files with a
  pause between them; directory lookups cost the size of the directory;
  the watcher parses on its own threads instead of the pool searches use;
  and a search waits up to five seconds for a step to finish instead of
  failing at once. On one million files with 2,000 files rewritten a
  second, no search was turned away and the slowest took 0.2 s.
- Python: an absolute import (`import pkg.mod`, `from pkg import x`) in a
  file nested more than 8 folders below its package root resolved to
  nothing, so the dependency explorer listed the project's own modules as
  external packages. Every ancestor folder is now tried. A package in a
  sibling folder put on `PYTHONPATH` (`lib/mypkg` imported from `apps/`)
  is still not found; the graph support page lists it as a known gap.
- Python: in a repository whose root folder has an `__init__.py` (run with
  that folder on `sys.path`), every absolute import of the project's own
  packages was listed as external, because a package folder is never a
  root. The outermost package folder is now tried as a root when it is the
  repository root or no folder above it holds an indexed file.
- Web UI: search history now records searches made by typing, not only
  ones submitted with Enter. A query counts once the box sits unchanged for
  two seconds or you move on (into the results, a preview or a filter);
  carrying on typing replaces that entry rather than leaving prefixes.

## [0.16.1] - 2026-10-14

**Existing indexes are rebuilt once on the first start.** The on-disk
index format is now version 8 (case-insensitive search folds case like
`rg -i`), and symbols are re-extracted for the new reference kinds and
Python references; the rebuild also picks up the dependency-graph fixes
for TypeScript, Python and Rust. Results are
now in rank order with each file's hits together. The v0.16.1 binaries
report their version as 0.16.0.

### Added
- Reference kinds: each reference is a `call`, `type`, `value` or
  `attribute`, shown on the result (web UI tag, `fcs refs` `[call]`, API
  `reference_kind`, MCP `[reference: call]`), and the `kind:` operator
  filters on it: `refs size kind:call`, `-kind:attribute` to leave out
  property reads. The index file format is unchanged (the kind shares the
  column's word); symbols re-extract once on the first start.
- Search by imports: `rdeps:PATH` limits a search to that file and the
  files that import it, `deps:PATH` to the file and what it imports
  (`:N` hops, `:*` for any distance, `-rdeps:` / `-deps:` to leave them
  out, a trailing `/` for a folder). An import that lands on a package's
  entry file (`mod.rs`, `__init__.py`, `index.ts`) reaches the modules
  behind it without costing a hop, one entry file at a time, and only when
  the file names something the module defines; negated filters walk
  strictly. Unknown, ambiguous or graph-less
  anchors are errors, never an empty result. Responses say how many files
  were searched and warn when unresolved project imports may leave files
  out; hits carry their `hops` and nearer files rank higher. In the query
  everywhere, plus `deps` / `rdeps` on the API, gRPC, MCP `search_code`
  and `fcs --deps` / `--rdeps` (the only way to scope a regex), the web
  UI's FILTER panel, the dependency popover and the explorer's
  **Search importers** / **Search its imports** buttons.
- Python references find far more uses: decorators, base classes, type
  annotations, functions and classes used as values, attribute reads and
  exceptions, where they used to find calls only (Flask: 57% of the uses
  Jedi finds, now 98%). Parameters and local variables that share a
  function's name are no longer listed as its references. Existing indexes
  re-extract symbols once on the first start.
- `examples/ref_oracle.rs` and `src/check/oracle/py_refs.py` compare Python
  references with Jedi's; CI runs them on Flask and Click.
- Book page "Reference support": what the References mode finds in each
  language, and what it misses. C and C++ record no references; Python
  records calls only; TypeScript, Java and C# miss many type positions.
- Imports that resolve to no indexed file are counted, and the ones written
  as a path into the project (`./x`, `.x`, `crate::`, `@/`), which point at
  a broken import or an unsupported form, are called out: in the dependency
  explorer's folder map, in `GET /api/graph/modules` (`unresolved_imports`,
  `unresolved_local`), in MCP's `dependency_overview`, and per file in
  `file_dependencies`.
- Docs: "How the import graph is built" explains how Python and
  JavaScript/TypeScript imports are extracted and resolved, how an import
  is judged internal (resolves to an indexed file) or external, and how
  the method is checked against grimp and TypeScript.
- `fcs check [PATH…]`: checks fcs on your own code. It indexes the folders
  in memory, compares queries made up from the files with `rg -F` (exit 1
  on any difference, with each differing query shrunk), and compares the
  import graph with grimp (Python) and TypeScript's own resolver when they
  are installed. The graph oracles' scripts moved to `src/check/oracle/`.
- Web UI: the file viewer, the hover preview and the dependency explorer's
  source pane mark every hit in the whole file in a strip beside the
  scrollbar (query matches; import lines in the explorer). Click the strip
  to jump to the nearest one, including lines not laid out yet (#141). The
  preview's ↑/↓ now step through every match in the file, not only the
  lines on screen.
- Web UI themes: Bold (the existing look, still the default), Corporate and
  Corporate dark. On a first visit the UI asks which to use, previewing each
  live on hover and putting the chosen theme back on mouse-out (one click
  picks a theme and closes the picker); the palette button in the header (replacing the `?` link, which
  went to the Docs page also in the navigation) changes it later. The choice
  is kept per browser. New `[server]` settings: `default_theme` (`"bold"`,
  `"corporate"` or `"corporate-dark"`) for people who haven't chosen, and
  `theme_picker = false` to skip the first-run question on a shared server.
- The ripgrep backtest's `--check-defaults` (on in CI) walks every query
  the way a web UI user pages through it under the server's default
  limits, with Auto and Fast ranking: SEARCH ALL FILES when files were
  skipped, then LOAD MORE to the end. Every match must be shown exactly
  once.
- The ripgrep backtest makes up queries from the corpus (`--generate N
  --seed S`), checks them against `rg -F` and shrinks any that differ to
  the smallest form. CI runs 500 per run.
- Dependency-graph oracles: `examples/graph_oracle.rs` compares the
  import graph with grimp on Django and with TypeScript's own resolver on
  Vite, listing every edge fcs misses or draws wrongly. CI fails on any
  regression. Django now matches grimp exactly; Vite's remaining misses
  are tsconfig `paths`.
- Capped results say which limit cut them. Search responses carry
  `limits_hit` (`max_files`, `match_budget`, `per_file_cap`, `timeout`,
  each with a sentence for people); gRPC sends the names in the
  `fcs-limits-hit` response header, `fcs` prints a "results capped" note
  on stderr, MCP's `search_code` adds a "Results capped" line (also when
  nothing was found), and the web UI names the limit in the count's
  tooltip and shows "stopped at time limit" beside the time.

### Changed
- Import graph: a Rust path into the crate used without a `use`
  (`crate::a::b::f()`, `super::x::Y`) is an import of the module it names,
  so the dependency explorer, rankings and `rdeps:` see it. Rebuild the
  index to pick it up (upgrading from 0.15.x rebuilds it anyway).
- `file:` and `lang:` narrow each other: `file:src/ lang:rust` is the Rust
  files under `src/`, where it used to be every file under `src/` plus
  every Rust file. Repeating one operator still widens it.
- Web UI: the preview icon is back at the end of each match line (#153 had
  moved the preview to the line number, with nothing to show it). Hovering
  the icon previews the file; clicking it pins the preview, which then
  stays open until Escape, a click outside it or its ×, as does clicking
  or scrolling inside it. The line number opens the file and no longer
  previews it on hover. A kept preview now does what the file viewer
  does: Ctrl/⌘-click or double-click a name for its definition (Back in the
  viewer returns to the preview's line), click a line number to select it,
  Copy link, Explore imports. Its Open button is now **Expand**, which shows
  the same file in the large viewer at the preview's line (after ↑/↓ or a
  selected line, not always the original hit).
- Web UI: results show one line of context either side of each hit by
  default (was the match line only). Filter → Context lines still sets it
  and is remembered once you pick a value; `context=0` in a link shows the
  match line only.
- Web UI: the Docs page is a one-page cheat sheet (query operators, regex,
  keyboard shortcuts, `fcs` and MCP, the main API calls) with links into
  the documentation book. It used to be a hand-kept copy of the API
  reference that had drifted from the book. It shows the server's version
  and uses the server's own address in the MCP example.
- Web UI: denser results, about twice as many hits per screen. A hit is its
  code with the line number in a gutter instead of a header row above it;
  the kind shows as a small tag on definitions and references; the line
  number opens the file there (and previews it on hover), the file name
  opens the file, and copy and the explorer link show on hover. Language
  and dependency badges are quiet outlines, and on a phone they hide so the
  file name isn't cut off.
- Web UI: one code viewer for the file viewer, the hover preview and the
  dependency explorer, so line numbers, highlighting and "Show more" look
  and work the same in each.
- Web UI: one rule for labels and buttons. Labels are written in sentence
  case and the theme decides whether they show in capitals (Bold does), so
  buttons, toggles and headings on every page now look alike. The search
  button reads SEARCH instead of EXECUTE_QUERY. The Index pages lost their
  emoji (SVG icons instead) and use the same cards and buttons as the rest,
  and the dependency explorer's buttons and chips match the search page.
- Web UI: file names are never re-cased. The dependency explorer's pane
  title, extension badges and the compact search box in the header showed
  them in capitals; they now show as written.
- Web UI: one FILTER button (the duplicate in the results bar is gone). It
  shows how many path filters are set, the results bar lists them with an
  × to remove each, and the panel has *Clear path filters*. The boxes are
  now called SEARCH ONLY IN and SKIP PATHS and take `,` as well as `;`
  (a comma inside `{…}` is still glob alternation). The API, CLI and MCP
  still separate patterns with `;` only.
- Web UI: the results count includes path matches ("2 FILES · 1 RESULT"),
  the per-hit ranking score moved to the line number's tooltip, and the
  dependency badge reads "imported by N" instead of "N deps".
- Web UI: the indexing bar no longer flashes on every page load (it moved
  the search box), the focus hint says Ctrl+K or ⌘K to match the platform,
  and MAPPED is gone from the header stats (it read 0 B until searches ran).
- Web UI: the SEMANTIC page served by the keyword server says semantic
  search runs as a separate server, instead of claiming the keyword server
  is down (it probed a hard-coded port 8080).
- The INDEX page's sample files are shown relative to the indexed folder.
- Web UI internals: all six pages share one header (`static/shell.js`) and
  one stylesheet of design tokens (`static/app.css`); every colour, shadow
  and font is a token, Tailwind's colours point at the same tokens, and a
  test fails on hard-coded colours. This is groundwork for themes and
  should look the same, apart from the navigation links on the non-search
  pages now matching the search page. The semantic pages use the same SVG
  icons as the rest, so the 3.9 MB icon font is gone.
- A burst of file changes is applied once the tree has been quiet for
  200 ms (at most two seconds), instead of after a fixed two-second
  debounce, so edits show up in search sooner.
- The web API, the gRPC service and offline `fcs` now run every search
  through one shared request path (`search::request`): the same default
  page size, caps, deadline and offset limit, and the same dispatch to
  the engine. Offline `fcs` now enforces `server.max_page_offset` like the
  server does. A new test checks that all three return the same hits in
  the same order.

### Fixed
- Results are in rank order again, with each file's hits together: files in
  the order of their best hit, then every hit of that file by score. Since
  v0.12.0 hits were interleaved by file (every file's best hit before any
  file's second), and since #147 regardless of score, so a file's other
  hits could be pages away. This was never asked for. On the ranking
  suite, precision@5 goes from 0.90 to 1.00; precision@1 is unchanged.
- Web UI: a file's hit badge counted only the hits on the page ("1 hit" for
  a file with seven). Each result now carries `file_hits` (and
  `file_hits_capped`) in the web API, gRPC and `fcs --json`. A file cut at
  a page boundary reads "3 of 7 hits", and **Show 4 more hits in this
  file** fetches the rest in place, kept across **Load more**. The ripgrep
  backtest checks every first-page count against the full result and that
  each file's hits are together.
- Dependency graph: TypeScript and JavaScript imports that go through a
  `tsconfig.json`/`jsconfig.json` `paths` alias or `baseUrl` (`@app/orders`,
  `components/Button`) now link to the file they name, including settings
  inherited through `extends`. They were treated as npm packages, so repos
  that rely on aliases had few edges. Imports of a workspace package by its
  package.json `name` (`@acme/shared`) now link to that package's entry
  point. With no tsconfig `baseUrl` or `paths`, imports are also tried
  from the repository root. `import x = require('./x')` is picked up.
- Python: `from .missing import x` (or `import pkg.missing.sub`) where the
  module does not exist no longer draws an edge to the package above it;
  the import is unresolved, as Python fails it.
- Case-insensitive search now folds case exactly as ripgrep's `-i` does
  (Unicode simple case folding): `s` finds the long `ſ`, `k` the Kelvin
  sign, `μ` the micro sign `µ`, and the Greek symbol forms (`ϐ`, `ϑ`, …)
  their letters. The index stores folded text, so the on-disk index format
  is now version 8 and an existing index is rebuilt once on upgrade. Found
  by `fcs check` on Django.
- LOAD MORE past 1000 results repeated some results and skipped others on
  broad searches, because each deeper page ranked a different sample of
  the matches. Every page that ends at result 1000 or later now ranks the
  same sample, so the pages fit together.
- Web UI: the hover preview shows the whole file and scrolls. It used to
  fetch 12 lines either side of the hit, so a FILES-box row (previewed from
  line 1) showed 13 lines in a window-tall panel, and the mouse wheel
  scrolled the page behind it, which closed it. It now opens at the hit
  (or the top of the file, with no line marked, for a file row), scrolls
  inside itself, has ↑/↓ buttons to step through the query's matches in the
  file, and an Open button for the file viewer. Recently viewed files are
  cached, so hovering again or opening the file doesn't re-fetch it.
- File watching no longer leaves deleted files in the index. Moving a file
  into a directory and deleting that directory soon after left the file's
  old name indexed forever, because the two events were merged into one
  that named only the directory. Watch events are now hints: every path an
  event names is checked against the disk, directories are reconciled, and
  when events are lost (an OS queue overflow) the watched folders are
  reconciled instead.
- Files changed while the initial index was being built could be indexed
  with old content, or stay indexed after being deleted or moved. Content
  that is no longer on disk is now never merged into the index.
- Python imports resolve the way Python 3 does. `from pkg import
  submodule` and `from . import submodule` now point at the submodule,
  not at the package's `__init__.py`. An absolute import no longer
  resolves to a same-named file next to the importer (`import json`
  inside a package reaches a sibling `json.py` no more, and `from django
  import …` inside `backends/` no longer means `backends/django.py`).
  `src/` layouts resolve. On Django the graph went from 87% to 100% of
  grimp's edges, with 287 wrong edges removed.
- The dependency graph of a watched index now ends up the same as a fresh
  build's. Every file's imports are kept, and a change resolves again the
  imports it can affect: a file that appears takes over imports that had
  fallen back to its package, a file that goes hands them back, and an
  edit that removes an import no longer leaves its edge behind when the
  imported file is saved in the same burst. The initial build also
  resolves every import once more at the end, so the graph no longer
  depends on the order files were indexed in.
- JavaScript/TypeScript: `import(/* comment */ './x')`, imports with the
  older `assert { type: 'json' }` and `.cjs` written for a `.cts` file now
  give edges.
- Whole-word search now uses the same word characters as ripgrep's `-w`
  (Unicode `\w`): combining marks such as Indic vowel signs and decomposed
  accents count as part of a word, so `word:yes` no longer matches inside
  Hindi, Malayalam or Telugu words.
- Whole-word search with MATCH CASE no longer misses a match that overlaps
  a rejected one (`}}` at the end of `field}}}`), and the same on the
  Unicode case-folding path.

## [0.15.2] - 2026-10-13

**Search works differently in this release.** Plain-text search is now a
literal, like `rg -F`: several words are one phrase, quotes and a leading
`-` are part of the text, and the `-term` exclusion and quoted phrases are
gone. In the web UI, a regex ignores case unless MATCH CASE is on. See
*Changed* below before upgrading scripts or saved queries.

### Added
- Search finds files by path. Typing or pasting a path (`src/web/api.rs`,
  `api.rs`, an absolute path, Windows `\` separators) lists the matching
  files in a **FILES** box above the results in the web UI, first in MCP
  `search_code`, and above the hits in `fcs` on a terminal. File names rank
  above directories. Content results are unchanged. API: `files=N` on
  `/api/search` adds `files` and `total_files` to the response.
- Web UI: **MATCH CASE** toggle. It means the same in every mode: off,
  upper and lower case are the same in plain text and regex alike; on,
  they are matched exactly. The results header says which applied
  ("ignoring case" or "matching case"). MCP `search_code` takes
  `case_sensitive`.
- Web UI: **SEARCH ALL FILES**. When a query could be in more than 5,000
  files, fast ranking opens only the 2,000 most likely ("Searched 2,000 of
  N files"); the button opens the rest. API: `max_files` on `/api/search`,
  `fcs --max-files`.
- Configuration: `fast_rank_files` (default 2000) sets how many files fast
  ranking opens, `max_fast_rank_files` (default 0, no limit) caps what a
  request may ask for, and `max_page_offset` (default 100000, 0 = no limit)
  sets how deep a client may page. Responses report the cap as
  `max_files_allowed` when one is set.

### Changed
- **Plain-text search is a literal, like `rg -F`.** Several words are one
  phrase: `fn main` finds lines holding `fn main`, not every line holding
  `fn` or `main` in files that contain both. Quotes and a leading `-` are
  part of the text: `"fn main"` finds the words with their quotes and
  `-Wall` finds `-Wall`. The `-term` exclusion and quoted phrases are gone;
  `file:`, `-file:`, `lang:`, `-lang:`, `case:` and `word:` still work. On
  the command line, put a query starting with `-` after `--`.
- Paging is no longer refused past offset 10,000: the limit is
  `max_page_offset` (100,000 by default), and a deep page gets a match
  budget that reaches it.
- Web UI: with MATCH CASE off a regex ignores case, like plain search. The
  API and `fcs -e` keep matching a regex's case by default, as ripgrep does.
- The search row stacks below 1024 px wide so the input keeps its room.

## [0.15.1] - 2026-10-13

Same changes as 0.15.0, with the version number set correctly. The v0.15.0
tag was pushed before the version bump, so its binaries report 0.14.0 and
its release has no changelog notes. Use 0.15.1.

### Added
- Web UI: go to definition. Ctrl+click (⌘+click on a Mac) a name in the
  file viewer or a result snippet to open where it is defined (a list when
  there are several, a text search when there are none), with **Back**
  (Alt+←) in the viewer. Double-click a name for Definition, References and
  Search. Uses the existing symbol and reference search.
- Web UI: the file viewer has a shareable link. Click a line number to
  select a line; **Copy link** copies a URL whose fragment
  (`#file=…&line=…`) reopens the search with that file at that line.
- Web UI: syntax colouring for Dart, Dockerfile, Elixir, Erlang, Haskell,
  Protobuf and Scala (`static/highlight-languages.min.js`, rebuilt by
  `scripts/update-hljs-languages.sh`), and for more extensions of
  languages already bundled (`.pyi`, `.mts`, `.kts`, `.vue`, `.ini`,
  `.diff`, `.graphql`, ...).
- `show_root_name` indexer option (default `true`). Set it to `false` to show
  result paths without the indexed folder's name (`src/main.rs` instead of
  `myapp/src/main.rs`) in the web UI, REST API, `fcs` and MCP. Only applies
  when a single path is indexed; `file:` filters are unaffected.
- Index freshness. With file watching off, the server only sees files as
  they were at its last scan. Search responses, `/api/status` and the progress
  socket now carry `index` (`watching`, `synced_at_ms`, `updated_at_ms`), and
  `/api/status?paths=…` says whether each path is current, stale, not indexed
  yet or deleted. When watching is off, the web UI shows "Index synced 6 min
  ago · watching off" beside the search time, `fcs` prints the same warning
  on stderr (`-q` hides it), `fcs status` reports it, MCP `search_code`
  results end with it, and a new MCP tool, `index_status`, lets an agent
  check the files it just wrote.

### Fixed
- Web UI: search result context and hover previews are syntax-highlighted in
  one pass, so doc comments and other multi-line comments are shown as
  comments instead of having their words coloured as code (also when the
  snippet starts partway into a comment).
- Web UI: the exact hit is outlined on the matched line, where it used to
  blend into the line's yellow background.
- Web UI: Dockerfiles and Makefiles in subfolders (`deploy/Dockerfile`)
  are recognised by name and highlighted, instead of shown as plain text.
- Web UI: code is shown without font ligatures, so `!=`, `=>` and `===`
  appear as typed rather than as `≠` and arrows.

## [0.14.0] - 2026-10-08

### Added
- A ripgrep backtest: `examples/rg_backtest.rs` runs every query in
  `benches/rg_backtest.toml` (124 queries, including cases adapted from
  ripgrep's and Zoekt's own regression tests) through fcs and through
  `rg` over tokio 1.45.0, Django 5.2 and the edge-case files in
  `tests/fixtures/backtest`, and fails on any line one finds and the other
  does not. It also checks that a page of results is repeatable and that
  LOAD MORE ends with every hit exactly once. Runs in CI on changes to the
  engine. Two new documentation pages: how the backtest works and how to
  extend it ("Correctness against ripgrep"), and where fcs's matching
  agrees with or differs from grep, ripgrep and git grep ("Matching
  compared with grep and ripgrep").
- MCP server for coding agents at `/mcp` on the web server (streamable
  HTTP), with six read-only tools: `search_code`, `read_file`,
  `file_dependencies`, `change_impact`, `import_path` and
  `dependency_overview`. `fcs mcp` relays it over stdio for clients that
  launch a command. Browser requests from foreign origins are refused.
- Dependency explorer (GRAPH in the web UI, `/graph.html`): a file's import
  neighbourhood, the impact of changing it (with the affected tests), the
  shortest import chain between two files, and a folder-level module map
  with import cycles marked, beside the file's source with its import lines
  linked. Search results, the deps popover and the file viewer link into it,
  and it links back to keyword search limited to the files on screen.
- `/api/graph/neighborhood`, `/impact`, `/path`, `/modules`, `/imports` and
  `/files` over the import graph (see `docs/API.md`).
- Distribution: `cargo binstall` metadata pointing at the release archives;
  a container image (`ghcr.io/jburrow/fast_code_search`) built per release
  with a documented `docker run` line; release archives signed keylessly
  through Sigstore with the verification command in the release notes; a
  crates.io publish job and a Homebrew-tap update job (formula generated
  from the archives' checksums) that run when their tokens are configured.
- Diagrams in the documentation: the indexing pipeline, how a score is
  composed, and the parts of a deployment.
- `server.enable_grpc` (default `true`) and `--no-grpc` to run without the
  gRPC API.
- Storage health checks. At startup and before each save the server warns
  when the filesystem holding the index is out of inodes or short of space,
  and when a filesystem holding indexed source is out of inodes. A failed
  save with "No space left on device" says whether bytes or inodes ran out,
  since an inode-exhausted disk still shows free space.
- One server per index: the server holds a lock on `<index_path>.lock` and
  writes its PID and addresses to `<index_path>.lock.owner`. A second server given the same `index_path` logs
  which process owns it, loads the index read-only and never saves, so the
  two can no longer overwrite each other's index.
- The diagnostics page has a Service card: web UI and gRPC status, where
  the index is saved (or that it is memory only / read-only), and startup
  and storage problems, which also mark the status as degraded.
  `/api/health` reports the problem count, and the search page shows
  "Server started with N problems" linking to it.
- Regex search help in the web UI: a `?` cheat-sheet next to the REGEX
  toggle with runnable examples and the rules that catch people out; when a
  search finds nothing, close variants that do find something (flexible
  spacing, ignore case, the same text without regex, or regex for a plain
  query that looks like one) are offered as one-click suggestions; an
  invalid regex shows where it is wrong and offers to search the text
  literally or escaped. The regex docs gained examples and pitfalls.

### Security
- rustls 0.23.36 -> 0.23.45 for RUSTSEC-2026-0285 (TLS 1.3 handshake
  messages accepted across encryption-level boundaries).

### Changed
- The keyword search results header is shorter: "1,234 RESULTS · 42 ms"
  instead of "1234 RESULTS FOUND · LATENCY: 42.3ms · FULL (5,678 files)".
  The ranking mode and file count moved to tooltips; the header still says
  when fast ranking searched only some of the files.
- An invalid regex's error now carries the parser's explanation and points
  at the problem in the pattern as typed (it used to repeat only the
  pattern).
- Rust imports through the crate's own name (`use my_crate::search::X` in
  `main.rs`, `src/bin/` and `tests/`) now resolve to the library's files, so
  those files have import edges and the library modules they use get the
  dependency ranking boost. Existing indexes pick this up on rebuild.
- A busy or forbidden port no longer stops the server. If the gRPC or web
  port cannot be bound, the server logs an error naming the port and how to
  find what holds it, and keeps indexing and serving on the other API. It
  exits only when no API could start. Previously a busy gRPC port shut the
  server down at once, interrupting indexing at 0 files while the web UI
  stayed up, and the cause appeared only after Ctrl+C.
- Startup logs where the config came from (or that none was found and
  defaults are used) and the config warnings; both were previously
  discarded because they were emitted before logging was set up.
- An unset `indexer.index_path` is now a warning when indexing starts ("Index
  persistence OFF ... lost when the server stops"), and the ready line shows
  `index=memory only, not saved`. A missing `index_path` directory is a
  warning rather than an error, since the first save creates it.
- `--init` writes every config key with its default. `index_path` is now set
  in the generated file, named after it (`--init work.toml` gives
  `~/.local/share/fast_code_search/work.fcsidx`); `cors_origins`,
  `max_concurrent_searches`, `request_timeout_secs`, `respect_gitignore`,
  `include_extensions`, `exclude_files` and `batch_size` are no longer
  missing or commented out. A test keeps the template complete.
- `fast_code_search_semantic` logs where its config came from, and a busy
  gRPC or web port is a clear error that disables only that API, instead of
  a panic inside a background task that left the process running with
  nothing listening.

- Documentation brought up to date with the above: the dependency
  explorer and `/api/graph/*` in the in-app docs, regex help in the web UI
  and query-syntax pages, `enable_grpc` / `--no-grpc`, the index lock,
  storage warnings and the diagnostics Service card in the API reference,
  configuration cookbook and troubleshooting guide. The deployment guide
  is rewritten around the published container image, HTTP health and
  readiness probes, and one index per server (its gRPC examples relied on
  reflection, which the server does not offer).

### Fixed
- Search correctness review: a file with more than 100 hits now reports
  the total as unknown and LOAD MORE reaches its later lines (it claimed an
  exact count of 100); a regex mentioning `\n` keeps `^` and `$` per line
  (they matched only at the file's ends), and an escaped backslash such as
  `\\n` no longer switches to whole-file matching; symbol search returns
  files whose only match is their name again and honours `-term`; `file:`
  and `-file:` words no longer match the indexed root folder's own name
  (`-file:search` dropped every file of a root named `fast_code_search`);
  a filename-only hit obeys `case:yes` and `word:yes`; `fcs search -e -i`
  against an index file now ignores case, as it does against a server.
- A search that stopped at its match limit returned a different first
  page on almost every run (threads raced for the budget), and LOAD MORE
  repeated some results and skipped others. Documents are now taken in a
  fixed order, and LOAD MORE fetches the whole list again at the bigger
  size and redraws it (up to 1000 results; past that it pages as before),
  so nothing repeats or goes missing. Results already shown can move.
- Greek: a query or line with a word-final sigma (`ς`, or a capital `Σ`
  that lowercases to it) missed matches written with the other sigma.
- A file starting with a UTF-8 byte order mark (an invisible marker some
  editors write) kept it as part of line 1, so `^text` missed that line and
  the result showed a stray character. The mark is now skipped when a file
  is read, as ripgrep does. Files on disk are untouched.
- A regex that can match the empty string (`x*`, `^\s*$`) reported a hit
  on a line past the end of a file that ends with a newline.
- Plain-text search for code containing quotes (`{ "success": True`,
  `"key":"value"`) found nothing: the query parser stripped every `"`, so
  the term became `success:`. A quote now opens a phrase only when it
  wraps a whole token; any other quote is searched for as written.

## [0.13.0] - 2026-09-08

### Added
- A ranking-quality suite: `examples/ranking_quality.rs` runs the labelled
  queries in `benches/ranking_quality.toml` (tokio 1.45.0 + Django 5.2)
  and reports precision@1 / @5; CI fails a ranking regression. Machine
  specifications and every figure of the real-corpus benchmark are written
  as JSON (`corpus_bench --json`) and published to the site per run.
- Rust items wrapped in brace-delimited macro invocations (`cfg_rt! { … }`)
  are extracted as definitions and references. Existing indexes re-extract
  symbols once on the first start.
- The benchmark trend page (`dev/bench/`) is now the project's own: it
  groups the charts by suite, says what each one measures and what would
  move it, labels every axis with its unit and "lower is better", shades
  each run's spread, links each point to its commit, and charts the
  scan-tool comparison: the latest run as one bar chart and each query's
  ripgrep / ugrep / fcs times per commit. The README's
  benchmark section explains the three measurements and how to read them.
- `scripts/bench/compare.sh` times the same queries through ripgrep, ugrep
  and `fcs` on the same tree (warm cache, median of seven runs, machine
  recorded) and refuses FUSE/NFS/SMB mounts, where scan tools are unfairly
  slow. CI runs it and the documentation site shows the result alongside
  the latency and ranking-quality figures, with two new book pages.

### Changed
- Ranking: single-term lines of a multi-term query are capped per file and
  the files holding the phrase are scanned first, so the match budget can
  no longer starve the best hits; the whole-word phrase outranks the phrase
  as a prefix of a longer identifier; test, example, mock and fixture paths
  are demoted at line level; public definitions score higher and the
  start-of-line boost ignores visibility modifiers; symbol search scores
  from the symbol cache before reading files, weights exact names above
  prefixes more strongly, and ranks `impl` blocks below the type. The
  suite went from 0.30 to 1.00 precision@1 on the labelled set.
- Reference search scores each hit by its file (dependency boost and the
  test-path penalty) instead of a flat 1.0, and every result ordering
  breaks ties on the path rather than the file id, so the same query
  returns the same page on every machine.
- The "Background indexing completed" log line reports the bytes of text
  indexed rather than the bytes currently memory-mapped.

## [0.12.1] - 2026-09-08

### Changed (dependencies and CI)
- `toml` 1.x, `actions/upload-artifact` 7, `actions/download-artifact` 8,
  `actions/setup-node` 7, `codecov/codecov-action` 7,
  `softprops/action-gh-release` 3; the Pages deploy has its own concurrency
  group and pushes without force.

### Added
- A documentation site built with mdBook (`docs/book`, published under
  `/docs/` on GitHub Pages): Start here, Guides, Reference, How it works,
  Benchmarks and Project sections. The canonical documents are imported at
  build time with links rewritten; the `fcs` help, server help and
  configuration template pages are generated from the binaries and
  drift-checked in CI (`scripts/docs/`).
- `CODE_OF_CONDUCT.md` (Contributor Covenant 2.1), issue forms for bugs,
  performance problems and feature requests (questions go to Discussions),
  and a fuller security policy with private reporting and response targets.
- `docs/API.md`: the REST, gRPC, server command line and configuration
  reference, moved out of the README.
- Release pages carry an install table, checksum instructions and a
  first-run line alongside the changelog section.

### Changed
- README rewritten as a landing page that leads with how results are
  ranked; benchmark figures are from the latest CI run and link to it.
- The site and web docs no longer state unmeasured claims ("10 GB+",
  "600k+ files"); headline figures come from the benchmark run.
- Internal engineering notes (`plans/`, the instruction-files blueprint)
  moved under `docs/internal/`.

## [0.12.0] - 2026-09-07

### Added
- A Web UI job in CI runs the JavaScript helper tests (`npm test`, Node's
  built-in runner over `static/tests`) and fails if the committed
  `tailwind.css` does not match the markup.

### Changed
- Web UI: the keyword pages use inline SVG icons instead of the 3.9 MB
  icon font (which stays only for the semantic pages); result groups are
  rendered on demand as the page scrolls; the pure helpers live in
  `static/lib/keyword-helpers.js` and the query tokenizer now matches the
  server's rules exactly; the unused dark highlight stylesheet is gone.
- Results interleave files within a score tier: every file's best hit,
  then every file's second hit, and so on, so one file holding a hundred
  matches no longer fills the page. Phrase and all-term tiers still come
  first; paging stays deterministic.

### Fixed
- Web UI: dependency popover failures are shown inline instead of an
  `alert()`; copying a path works on plain-HTTP origins (textarea
  fallback) with a visible Copied/Copy failed state; the hover-preview
  cache is cleared when the index changes, so previews cannot go stale.

## [0.11.0] - 2026-09-07

### Added
- `fcs`, a command-line client (`docs/CLI.md`): sends queries to the running
  server over the REST API with the web UI's syntax and modes (`refs`,
  `symbols`, regex, globs, context), prints `path:line:col:text` when piped
  or grouped colour output on a terminal, `--json` for tools, and uses grep's
  exit codes. When no server answers it searches the on-disk index in-process
  (`--offline`, `--index-path`). `fcs status` reports the server and index.
- A guide to running the server at login on Linux (systemd user unit), macOS
  (launchd agent) and Windows (Task Scheduler), with ready-made unit files
  under `deploy/` (`docs/RUN-AT-STARTUP.md`).
- The API's result types (`SearchResponse`, `SearchResultJson`) are public
  and deserializable, and `reqwest` is a regular dependency (the CLI's
  HTTP client).

Fixes from the 2026-09-06 engine and UI review
(`docs/internal/plans/2026-09-06-keyword-engine-and-ui-review.md`).

### Changed
- Line-mode regex patterns are matched in one pass per file (compiled with
  `(?mR)`, one hit per line, per-line semantics for `^`, `$`, `\s` and CRLF
  kept), removing the per-line start-up cost that made `fn\s+main\(` an
  order of magnitude slower than `main\(` over the same candidates.

### Fixed
- Regex candidate generation treated a repetition as one contiguous copy on
  both sides, so `fo+bar` demanded the literal `fobar` and never found
  `foobar`; repetitions are now run boundaries, and a property test asserts
  the candidate set is a superset of the regex's matches. Case-insensitive
  runs no longer break at `k` and `s` (which previously turned `(?i)task`
  into a full scan).
- `references=true` applied index-time columns to the current file content
  and could panic mid-character on a file edited since indexing (a 500 for
  every reference query until re-index); positions are verified first.
- Rust type mentions are captured as references.
- Multi-term queries rank lines containing more of the terms first and
  emit them before the per-file cap, so `fn main` finds `fn main()`.
- The query syntax no longer swallows `->`, `-Wall`, `-1.5`, `file:` or
  quoted operators; `file:src/` selects everything under `src`.
- `case=false` is honoured in regex mode; a needle containing `\n` no
  longer reports a match range past the end of the line.
- `update_file` kept the edges out of a file but dropped every edge into
  it, zeroing a module's dependents whenever it was edited; the edges are
  kept, and a deleted-then-recreated file regains its dependents.
- Re-indexing an already indexed path replaced its postings instead of
  unioning stale trigrams onto them; suffix path lookups match whole path
  components only, and an empty suffix matches nothing.
- TypeScript call sites (plain and member calls) are captured as
  references; the upstream tags query only had type and `new` mentions. An
  index saved by an earlier extractor has its symbols and references
  re-extracted on load (the fingerprint now carries an extraction schema
  tag), so this reaches existing indexes without a rebuild.
- Multi-term queries prefer the phrase: files whose trigrams contain the
  terms as written are always opened, and lines holding the phrase rank
  above lines holding every term, above single-term lines.
- Reload: gitignore matchers are cached per directory during the
  eligibility pass (it took 14.8 s for 61k files).
- A watcher burst re-indexes modified files with one posting-list pass and
  a parallel parse instead of a full scan per file under the write lock.
- Removing a file releases its memory map instead of keeping it alive in
  the tombstoned slot.
- Two concurrent saves (the watcher's shutdown save racing the indexer's
  final save, a checkpoint racing a watcher save, or two processes sharing
  `index_path`) could delete the index file. Saves are serialized and use
  unique temp files; the rename fallback never removes the target unless the
  temp file still exists.
- `offset` above 10,000 is rejected instead of raising the match budget
  without bound; the derived budget is capped and uses saturating maths.
- Every search runs under an engine deadline just under the request
  timeout, so a scan whose response the HTTP layer abandoned stops on its
  own. The timeout answers 504 with the JSON error envelope and
  `Retry-After` instead of an empty 408, and the concurrency permit and
  latency metric follow the search's real lifetime.
- gRPC shares the REST search semaphore (per-connection limits let N
  connections run N × limit searches), rejects `Index` with no paths, runs
  one `Index` at a time and skips `finalize` when nothing was indexed.
- A config change (exclude pattern, extension list, size cap, `.gitignore`
  rule) now removes newly ineligible files on reload instead of being logged
  and ignored.
- Discovery no longer descends into excluded directories (`.git/objects`,
  `target/`, `node_modules/`), honours `.gitignore` in trees that are not
  git repositories (so it agrees with the watcher), and treats
  `max_file_size = 0` as the default like every other code path.
- On Linux the watcher adds one non-recursive watch per kept directory, so
  excluded trees consume no inotify watches; the change queue is bounded.
- Reload only treats a missing file as removed; a permission or I/O error
  (an unmounted share at boot) keeps the entry instead of wiping the root.
- Configured roots match files at path-component boundaries and the longest
  root wins, so `/x/proj` no longer claims `/x/proj2/...`.
- Config: `~` is expanded, relative paths resolve against the config file's
  directory, `include_extensions` accepts `".rs"`, and an `FCS_CONFIG`
  pointing at a missing file is an error rather than a silent fallback.
- `save_after_updates` counts files rather than watcher batches; checkpoints
  follow `save_after_build`; a second Ctrl+C during a slow shutdown save
  exits immediately.
- REST/gRPC: `references=true` combined with `regex` or `symbols` is a 400;
  the invalid-regex message is no longer doubled; `max=0` means the default
  page size on both surfaces; `/api/context` returns the same
  workspace-relative path as search results; an empty `file=` is a 400
  instead of resolving to an arbitrary file; `/api/ready` stays ready while
  a persisted index is reconciled and no longer reports zero files while
  the write lock is briefly held; the diagnostics self-tests look for the
  sampled file with a file filter, so a healthy large index is no longer
  reported as degraded.
- Metrics: latency buckets compare fractional seconds (1.9 ms counted as
  1 ms); concurrency rejections count in `fcs_search_requests_total`.
- HTML, scripts and stylesheets are served `no-cache` with ETag
  revalidation so an upgrade cannot mix old and new bundles; every response
  carries `X-Content-Type-Options`, `X-Frame-Options` and
  `Referrer-Policy`; `/ws/progress` caps inbound frames.
- Web UI: server-side strings on the diagnostics page are escaped (a file
  extension could inject markup); match highlighting uses the server's
  match offsets and highlights every query term, honouring `case:` and
  regex mode; URL parameters fully determine the search modes (`regex=false`
  turns regex off; stored settings no longer override shared links) and
  toggle visuals follow the real state; `max` and `context` are validated
  (no more `max=NaN`), with 500 and 1000 available; searches get history
  entries so Back restores the previous query; results page with
  `offset` ("Load more"), show `total_matches`, mark truncated lines and
  label filename hits; a REFERENCES toggle; 503 while indexing is retried;
  the progress socket keeps reconnecting; the compact header search no
  longer overlaps the stats; the hover preview dismisses on Escape, scroll
  and outside clicks; dialogs have proper semantics, focus management and a
  focus trap; results are keyboard-navigable with real focus; contrast
  meets AA; the file viewer highlights once and windows large files;
  docs.html matches the API; the "INDEX" header stat is labelled as mapped
  content.


## [0.10.1] - 2026-09-06

### Fixed
- The index could keep a file whose delete or rename-away event the watcher
  backend never delivered (FSEvents on macOS drops the source half of a
  rename), leaving an entry that search could no longer read. After applying
  a batch of changes the engine now checks the siblings of every changed path
  and drops entries whose file is gone.
- Import resolution failed when the importing path was not in canonical form
  (a symlinked temp directory on macOS, a non-verbatim path on Windows); the
  path is now canonicalized once per importing file.
- CI: failing test names, panic messages and crashes are reported as workflow
  annotations; the macOS and Windows test jobs pass.

## [0.10.0] - 2026-09-06

### Fixed
- A freshly loaded index reported absolute display paths (and the UI could
  not round-trip them) until the next finalize, because the configured roots
  were registered after the per-file metadata pass cached the paths.
- **Persisted index corruption after a file removal**: `save_index` compacted the
  file table over tombstoned ids but wrote trigram bitmaps, symbols and dependency
  edges keyed by live id, so every file after a removed one was misattributed or
  lost on reload. Ids are now remapped at save time.
- Every match on line 1 of every file received the 3× symbol-definition boost
  (the synthetic filename symbol at line 0 counted as a definition line).
- Files with a line over 100 KB or extreme nesting were dropped from the whole
  index; they are now text-searchable and only skip symbol extraction.
- The persisted mtime/size described the on-disk state at *save* time, so a
  file edited during a long build was never detected as stale. Both are now
  captured when the content is read.
- Symbols: `.tsx` is parsed with the TSX grammar; JS/TS class methods,
  `const f = () => …`, generator functions, abstract classes and namespaces are
  extracted; symbol line/column now come from the name node (Java `@Override`,
  decorators, attributes and multi-line C signatures reported the wrong line).
- Directory rename/delete events were silent no-ops in the watcher; every file
  under the old path stayed indexed. The watcher now also applies the same
  eligibility rules as the initial build (include extensions, excludes, size).
- A panic on the watcher path poisoned the engine lock and turned every search
  into a 500 until restart; the watcher path is now panic-guarded, runs on an
  8 MB stack, and the REST/gRPC readers recover from a poisoned lock.
- `/api/context` no longer overflows on a huge `context` value and is capped at
  200 lines each side; user regexes have an explicit compile-size limit.
- `DependencyIndex::remove_file` never pruned the filename index and every
  watcher update pushed a duplicate path entry.

### Added
- A real-corpus benchmark (`examples/corpus_bench.rs`, run in CI over pinned
  tokio and Django checkouts, nightly over the Rust compiler tree) reporting
  build throughput, resident memory, query latency percentiles, incremental
  update cost and index save/load.
- `/api/search` paging and budgets: `offset` (deterministic ordering, so pages
  are stable), `timeout_ms`, and `total_matches` / `truncated_by_budget` in the
  response; `has_more` is now derived from the real total. Regex and symbol
  searches report ranking info too.
- Every result carries `line_match_start` / `line_match_end` (byte offsets into
  the full line) and `match_column` (0-based character column) on REST and gRPC.

### Added
- Symbol references: `/api/search?references=true` (gRPC
  `SearchRequest.references`) returns the call sites, type mentions and
  implemented traits of an identifier as `SYMBOL_REFERENCE` results, from the
  grammars' tags queries (the Rust query is supplemented with path and generic
  calls). References are stored compactly (12 bytes each, names interned) and
  persisted.
- Multi-line regex: a pattern that mentions a newline (`\n`, `\r`, `\x0a`) or
  sets the `s` flag (`(?s)begin.*?end`) is matched across lines and reported on
  the line where each match starts. Other patterns stay line-oriented.
- Query syntax for plain-text searches: `"quoted phrases"`, several AND-ed
  terms, `-term`, `file:` / `-file:`, `lang:` / `-lang:`, `case:yes`,
  `word:yes` (REST `case` / `word` parameters and gRPC `case_sensitive` /
  `whole_word` override the in-query switches).
- `/api/ready` (readiness, distinct from `/api/health` liveness), `/metrics`
  (Prometheus text: search counters by outcome, latency histogram, index gauges)
  and the standard `grpc.health.v1` service.
- Config: `server.cors_origins`, `server.max_concurrent_searches`,
  `server.request_timeout_secs`, `indexer.respect_gitignore`; `--web-address`.
- gRPC `SearchRequest.offset` / `rank` / `deadline_ms`; results carry
  `dependency_count`.

### Changed
- Build: the semantic engine is behind the `semantic` Cargo feature (off by
  default; `ml-models` implies it), release binaries use thin LTO and are
  stripped, OpenTelemetry is on 0.32 (one tonic/axum stack), `md5` and `glob` are
  gone (the config fingerprint format changed, so the first start after upgrading
  rebuilds the index once), and `cargo-deny` + Dependabot are wired into CI.
- Trigram extraction folds case per byte and dedupes through a bitset instead of
  lowercasing a copy of every file and hashing every byte.
- Import resolution no longer probes candidate paths through `realpath`
  (17.6 million failing `readlink` calls while building a 6k-file corpus);
  candidates are resolved lexically against the indexed path table. Full
  build of tokio + Django: 25 s -> 3 s.
- Updating one file from the watcher went from 55 ms to 4 ms on that corpus:
  posting-list removal is parallel with a membership fast path, and compiled
  `.gitignore` matchers are cached per directory (keyed by mtime) instead of
  rebuilt per event.
- Memory diet: each indexed file stores its path once (shared with the
  path lookup map) and keeps at most 64 bytes of inline state; only files
  above the mmap threshold allocate mapping state. The evictable fallback
  cache for unmapped large files is gone (they are read per access), and
  the all-documents bitmap is borrowed rather than cloned per short or
  unconstrained query.
- Symbols are extracted with each grammar's own `tags.scm` query (name-node
  positions, upstream-maintained coverage), supplemented by the previous walker;
  new symbol kinds Module, Macro, Field and Property; C++ `.h` headers are
  detected; JSON/TOML/YAML/HTML/CSS/Markdown are no longer parsed; parses are
  capped at 2 s.
- Unknown config keys are rejected and the configuration is validated at
  startup (addresses, limits, index path directory); `OTEL_SDK_DISABLED=true` can
  no longer be overridden.
- REST requests time out (default 30 s), bodies are capped, and searches beyond
  `max_concurrent_searches` get 503 + Retry-After instead of queueing; gRPC has
  a per-request timeout and per-connection concurrency limit.
- gRPC `max_results = 0` now means the default page of 50 (was clamped to 1); the
  `Index` RPC applies the indexer's eligibility rules and no longer holds the
  engine lock for its whole walk.
- `/api/diagnostics` reports the real indexer configuration and caches its
  extension breakdown per index generation; malformed query parameters return the
  JSON error envelope; display paths resolve through their root instead of a
  per-file suffix scan.
- Every query's work is bounded by a match budget (8× the page, minimum 512) and
  an optional deadline; `rank=full` no longer materializes every match of every
  candidate before truncating.
- Regex searches are accelerated for `(?i)` literals and repeated suffixes
  (`abc+`), and compiled patterns are cached; plain-text verification scans the
  whole buffer with `memchr` and builds symbol maps only on the first hit.
- Symbol search consults the symbol cache before reading any file, never drops
  low-scoring files before matching, ranks exact > prefix > substring, and
  returns one row per line.
- **Retrieval no longer reads through a live memory map for files up to 1 MiB**:
  a searcher touching a mapped file that an editor truncated in place was an
  uncatchable SIGBUS that killed the server. Small files (essentially all source
  files) are read into an owned buffer per access; only larger files are mapped.
  This also keeps the mapping count far below `vm.max_map_count`.
- `.gitignore` / `.ignore` files under the indexed paths are honoured by the
  initial build and by the watcher (`indexer.respect_gitignore`, default true).
- **Persisted index format v7** (`FCSIDX05`): a sectioned layout (header with
  version, CRC-32 and section lengths; metadata; fixed-width sorted trigram
  directory; bitmap region). Saving streams the posting lists without an
  intermediate copy and loading memory-maps the file, validates it before
  decoding, and deserializes bitmaps in parallel from the mapping. The format
  also stores unresolved imports (so a checkpoint restore still gains the edge
  when the target file is indexed later), nanosecond mtimes, byte paths,
  run-optimised bitmaps and symbol references. Index files written by earlier
  versions are rebuilt automatically; a golden fixture pins the format.
- Import resolution is now per language (Rust crate/module paths, Python
  relative and package imports, JS/TS extension and `index` probing, `@/`
  aliases) and no longer guesses a same-named file anywhere in the repo for bare
  package names; unresolved imports are retried only when a file with a matching
  name appears instead of after every batch.
- Watcher events are gathered for 200 ms and applied under one write lock with a
  single posting-list pass; ranking metadata and the all-documents cache stay
  current across incremental updates and after a persisted load.
- **Graceful shutdown**: SIGINT/SIGTERM now stop both servers, stop the indexer
  (persisting a checkpoint), save pending watcher updates, and flush telemetry.
- **Security defaults**: both listeners bind to `127.0.0.1`; CORS is off unless
  `server.cors_origins` is set; the gRPC `Index` RPC only accepts paths under the
  configured `indexer.paths` and no longer follows symlinks; a web-UI bind
  failure is fatal instead of silently leaving the server without a REST API.
- Minimum supported Rust is 1.89 (was documented as 1.70); the toolchain is
  pinned via `rust-toolchain.toml`; `reqwest` uses rustls so builds and tests no
  longer need system OpenSSL.
- CI lints all targets and features, checks docs, has an MSRV job, and releases
  are gated on tests; the VS Code extension publishes on `ext-v*` tags only.
  Failing test names are surfaced as workflow annotations.
- Releases no longer ship the `-debug` archives (an unoptimised binary per
  platform); release binaries are built with thin LTO and stripped.

### Repository
- `.gitattributes` normalises line endings; the corrupted `.gitignore` is
  repaired; `onnxruntime/` and the `test_corpus` gitlinks are no longer tracked.

## [0.9.0] - 2026-06-10

### Added
- Real incremental index updates: file modifications, deletions, and renames now
  update the index in place (`TrigramIndex::remove_document`, file-store tombstone /
  refresh, dependency-edge removal) instead of leaving stale entries until restart.
- Watcher now handles rename events (`Modify(Name)`), wiring delete + re-index.
- Search API returns a `has_more` flag; the UI shows "N+ results (raise MAX RESULTS)".
- Always-visible FILTER toggle so filters can be set before the first search.
- Keyboard navigation in results (`j`/`k`/arrows to move, Enter to open, `/` to focus).
- Per-result copy-path button.

### Changed
- API errors are now consistent JSON (`{ "error": … }`); 503s include `Retry-After`.
- Static assets use compile-time content hashes for ETags and honor `If-None-Match`
  (304), eliminating per-request hashing of large embedded assets.
- Hover preview uses the lightweight `/api/context` endpoint with a hover-intent
  delay and per-file cache instead of fetching the whole file.
- Configured `max_file_size` is honored end-to-end (no longer silently capped at 10 MB).
- Docs page documents the `context` param and the file/context/dependencies/diagnostics
  endpoints, the JSON-error/Retry-After convention, and `total_results`/`has_more`.

### Fixed
- Eliminated memory-map UTF-8 undefined behavior and truncation crashes during
  indexing (no more `from_utf8_unchecked` over bytes that can change underneath it;
  single-file indexing reads an owned buffer instead of a live mmap).
- Guarded all indexing paths against panics and recover from poisoned engine locks
  instead of silently dropping batches.
- Hardened persisted-index loading (magic header + allocation-limited decode) and made
  index saves atomic (temp file + fsync + rename).
- Fixed silent result misattribution after reload by remapping trigram document IDs
  onto the compacted IDs assigned when stale/removed files are skipped.
- Regex acceleration no longer drops matches for alternations / optional literals
  (e.g. `hello|world` now returns files containing only one branch).
- Watcher/stale-file exclusions use glob semantics (so `**/.git/**` no longer excludes
  `.github`/`.gitignore`, and Windows backslash paths match correctly).
- Case-insensitive verification is Unicode-aware (e.g. `über` matches `ÜBER`); empty /
  whitespace queries return immediately instead of scanning the whole corpus.
- Web UI: quote-safe HTML escaping and removal of inline event handlers (XSS hardening);
  group-by-file uses the original (correctly-cased) path for display and fetches; search
  requests abort in-flight so stale results can't overwrite newer ones; offline banner
  tracks server recovery; `wss://` under HTTPS; rebuilt CSS; removed dead styles.

## [0.8.0] - 2026-03-27

### Added
- Dependency popover and tooltip interactions in the web UI to improve code navigation context.
- Shared search history utilities used by both keyword and semantic UIs for more consistent query history behavior.

### Changed
- Search result highlighting now uses dynamic, context-aware highlight styles in the keyword UI.
- Reduced duplicated UI styling by removing redundant syntax-highlighting and tooltip rules from keyword-specific CSS and inheriting from shared styles.

### Fixed
- Keyword UI tooltip cleanup behavior now hides context tooltips immediately when opening file views/modal overlays, preventing lingering tooltip artifacts.

## [0.7.5] - 2026-03-24

### Fixed
- Web UI regression where loading `/` or `/semantic.html` with `?q=...` restored the query text but did not execute search automatically on page load.

## [0.7.4] - 2026-03-20

### Changed
- New UI theme for the keyword search interface with improved layout and styling.

## [0.7.3] - 2026-03-20

### Added
- New agent skills: normalize, onboard, optimize, overdrive, quieter, and teach-impeccable for design context.

### Changed
- Updated scoring factors and improved score tooltip information in keyword search results.
- Enhanced UI/UX across diagnostics and semantic pages: improved status banner styles, typography, spacing, input focus styles, and semantic score display.

### Fixed
- Benchmark CI failure: reset local changes before gh-pages branch switch to prevent dirty `Cargo.lock` blocking the workflow (#97).

## [0.7.2] - 2026-03-18

### Added
- Workspace-relative display paths in results (VSCode-style folder name prefix) (#95).
- New agent skills: arrange, audit, bolder, colorize, critique, delight, polish, typeset.

### Changed
- UI improvements: wordmark styling, help icon for docs header link, and accessibility enhancements.
- Improved touch target sizes for mobile accessibility.

### Fixed
- Linting issues across the codebase (#94).

## [0.7.1] - 2026-03-17

### Fixed
- Fix path handling: canonical storage, root-relative display, and proper glob exclusions (#91).
- Fix semantic web API handlers panicking on a poisoned `RwLock` (#85).
- Preserve `dev/bench/` across GitHub Pages deployments (#83).

### Added
- URL state encoding for semantic search UI; refactored shared helpers into `common.js` (#93).
- VS Code configuration and tasks for Rust development.

### Changed
- Refactor code structure for improved readability and maintainability.

## [0.7.0] - 2026-03-03

### Added
- **GitHub Pages site** (`docs/site/`): A static project site deployed to `gh-pages` on every push to `main` via a new `pages.yml` workflow. The site includes a features overview, live CI badge matrix, embedded benchmark trend charts (from `dev/bench/` pushed by `github-action-benchmark`), Codecov coverage badge, dynamically-rendered changelog (from `changelog.json` generated at deploy time), and a Getting Started guide. Theme toggle (dark/light) with OS preference detection. The deploy action uses `clean-exclude: dev/` to preserve benchmark data, and both `pages.yml` and `benchmark.yml` share a `gh-pages-deploy` concurrency group to prevent simultaneous pushes.

## [0.6.9] - 2026-02-28

### Changed
- Version bump and release.

## [0.6.8] - 2026-02-28

### Added
- **Automated performance benchmarks in CI**: A new `benchmark` workflow runs on every push to `main`. It executes `cargo bench` for both `search_benchmark` and `persistence_benchmark` suites using the Criterion bencher output format, runs the validator binary in JSON mode for functional performance validation, and uploads results as a `benchmark-results` artifact. Historical trends are tracked in the `gh-pages` branch via `benchmark-action/github-action-benchmark`, with automatic alerts on >200% regressions.
- **Registered `persistence_benchmark` in `Cargo.toml`**: The `benches/persistence_benchmark.rs` suite (save/load index, trigram deserialization, file staleness checks) was already present but not declared as a `[[bench]]` target. It is now runnable via `cargo bench --bench persistence_benchmark`.

## [0.6.7] - 2026-02-27

### Added
- **Code coverage reports in CI**: A new `coverage` job runs on every push and pull request using `cargo-llvm-cov`. The HTML coverage report is uploaded as a `coverage-report` GitHub Actions artifact and LCOV data is uploaded to Codecov. See the [Code Coverage](#code-coverage) section in the copilot instructions for how to generate reports locally.

### Fixed
- **Website becomes unresponsive during index building**: API handlers (`search`, `stats`, `dependents`, `dependencies`, `diagnostics`) previously called `engine.read()` which blocks until any pending write lock (held during batch merges) is released. Under concurrent load, this caused blocking threads to pile up and exhaust the Tokio thread pool, leading to memory pressure and system unresponsiveness. All handlers now use `engine.try_read()` and immediately return `503 Service Unavailable` with the message _"Index is currently being updated, please try again shortly"_ when the write lock is held. The `status` endpoint was already safe and is unchanged.

## [0.6.6] - 2026-02-27

### Fixed
- **OOM during indexing of large codebases (Vec/HashMap over-allocation)**: After each doubling growth, Rust's `Vec` and `FxHashMap` may carry up to 2× excess capacity. On a 62 000-file corpus this contributes tens to hundreds of MB of wasted heap. Three changes address this:
  1. `SearchEngine::compact_memory()` — a new method that calls `shrink_to_fit()` on `symbol_cache` (both outer Vec and every inner `Vec<Symbol>`), `pending_imports`, and the trigram HashMap. Called automatically every 50 batches during the initial index build, so memory is reclaimed without waiting for `finalize()`.
  2. `SearchEngine::finalize()` — additionally shrinks `file_metadata` and runs a full `compact_memory()` pass once indexing completes.
  3. `TrigramIndex::finalize()` — now includes `shrink_to_fit()` on the `FxHashMap<Trigram, RoaringBitmap>` which can hold ~50% empty bucket slots after bulk insertion.

## [0.6.5] - 2026-02-27

### Fixed
- **`content_fallback` bytes permanently retained in heap (regression since v0.6.2)**: Files whose mmap failed fell back to a `OnceLock<Vec<u8>>` cache, which is write-once — once populated, the bytes could never be freed. Replaced with `Mutex<Option<Vec<u8>>>` so the cached bytes are evictable. Resolves the memory-growth issue tracked in v0.6.4 for Linux deployments approaching the mmap limit.

### Improved
- **Persist symbol caches and import graph in saved index**: When saving the trigram index to disk, the per-file symbol caches and resolved dependency edges are now also persisted. On the next startup, symbols and imports are restored directly from the index file instead of being re-extracted from file contents, eliminating the `rebuild_symbols_and_dependencies` pass for unchanged files and significantly reducing startup latency for large codebases.
  - Symbols are only re-extracted for files whose mtime has changed (stale files), which are queued for re-indexing as before.
  - Persisted indexes from older versions (format < 3) are automatically rebuilt once and saved in the new format.
  - Index persistence format bumped to version 3.

## [0.6.4] - 2026-02-27

### Fixed
- **OOM / memory errors during indexing on large codebases (regression since v0.6.0)**: `BATCH_SIZE` was silently raised from 500 to 2000 in v0.6.0 as part of the Phase-2 parallelisation change (#24). During Phase 1, all `BATCH_SIZE` files are read into memory simultaneously, so peak RAM scales as `batch_size × average_file_size × ~4`. On repos with many sizable files (generated code, minified JS, large headers, etc.) this caused 4× higher indexing-time memory consumption and triggered out-of-memory crashes. The default has been restored to **500**. A new `batch_size` key under `[indexer]` lets operators tune the trade-off between throughput and RAM:
  ```toml
  [indexer]
  batch_size = 1000   # raise on high-RAM machines; lower if still OOM
  ```
- **`content_fallback` Vec permanently retains file bytes in heap (Linux, search-time; tracked)**: Files that fail mmap (Linux: past `vm.max_map_count`; any OS: individual mmap failure) have their content cached in a `OnceLock<Vec<u8>>` on first search access. Because `OnceLock` is write-once, those bytes are never freed. On large repos where most files exceed the mmap soft-limit, every search that returns such a file permanently pins its content in heap memory; after enough searches the process exhausts RAM. A full fix requires refactoring the `&[u8]` lifetime API in `LazyMappedFile`; a TODO comment has been added at the relevant site and the fix is tracked. The `batch_size` fix above is sufficient for Windows users (where the mmap limit is not enforced).

## [0.6.3] - 2026-02-27

### Improved
- **Mmap-limit warning emitted only once**: Previously, every file registered without mmap (after the OS `vm.max_map_count` safe limit was reached) emitted its own `WARN` log line, flooding logs on large codebases. A single warning is now issued when the limit is first hit; all subsequent files are silently registered under the direct-read fallback.
- **Mmap-limit error message clarified**: The internal error text now states that remaining files will be indexed via direct-read fallback (slower retrieval, but search still works) instead of the previous misleading "Cannot index more files" wording.
- **Background indexer logs persistence settings at startup**: When `index_path` is configured, the indexer now emits an `INFO` log with the active `save_after_build`, `save_after_updates`, and `checkpoint_interval_files` values so operators can confirm persistence is active without reading config files.
- **README and startup warning updated**: Messaging around `vm.max_map_count` now accurately describes the graceful fallback behaviour introduced in 0.6.2 rather than implying hard failures.

## [0.6.2] - 2026-02-27

### Fixed
- **Files beyond the OS mmap limit were silently dropped from search results (P0 — Correctness)**: When the Linux `vm.max_map_count` safe limit (85% of system max, typically 55,700 on default kernels) was exceeded during indexing, `add_file()` bailed out and the remaining files were neither registered in the store nor added to the trigram index. On large codebases (e.g. 629k files) only the first ~55,700 files were searchable. Files are now always registered. For files that cannot be memory-mapped (limit exhausted), content is read via `std::fs::read()` at result-retrieval time and the result is cached, so there is no repeated I/O cost.
- **File count in UI showed mmap-capped number instead of true indexed count**: Because the above fix ensures all files are registered, `file_store.len()` — which drives the FILES counter in the web UI and `/api/status` — now reflects the true number of indexed files rather than the OS mmap limit.

## [0.6.1] - 2026-02-27

### Added
- **Checkpoint saves during initial indexing**: New `checkpoint_interval_files` config option under `[indexer]` (default: `0` = disabled). When set (e.g. `20000`), the index is written to `index_path` every N files during the initial build. If the process is killed before completion, the next run loads the checkpoint and re-indexes only the missing files — no compute is lost. Has no effect if `index_path` is not configured.

## [0.6.0] - 2026-02-27

### Added
- **Non-UTF-8 encoding support**: Files in legacy encodings (Latin-1/ISO-8859-1, Windows-1252, Shift-JIS, UTF-16 LE/BE, etc.) are now automatically detected and transcoded to UTF-8 during indexing, making them fully searchable. Zero performance impact on UTF-8 files (fast path via `std::str::from_utf8`).
  - New `transcode_non_utf8` config option under `[indexer]` (default: `true`) to disable transcoding if only UTF-8 codebases are indexed.
  - Transcoding uses `chardetng` (Mozilla's Firefox charset detector) + `encoding_rs` for accurate, battle-tested detection.
  - `files_transcoded` counter exposed in `/api/status`, `/api/diagnostics`, and gRPC `IndexResponse` for visibility.
  - Transcoded files emit an `INFO`-level log entry with the detected encoding name.
- **Filename-only matches now returned**: Searching for a filename that does not appear in the file's content previously returned zero results (the file was shortlisted by the trigram index but then silently dropped). All three search paths now synthesize a `SearchMatch` at line 0 when the query matches the filename stem but no content lines match.

### Fixed
- **Regex trigram lookup not lowercased (P0 — Correctness)**: `search_regex()` passed raw literal text (e.g. `"MyClass"`) to the trigram index which stores only lowercased content. Uppercase literals ≥3 chars returned zero trigram hits, causing missed results or a full scan fallback. The literal is now lowercased before the trigram lookup.
- **Exact-match score boost used case-insensitive comparison (P0 — Correctness)**: `calculate_score_inline()` applied the 2× exact-match boost using the already-lowercased query, so every match received the boost regardless of case. The boost now uses the original (un-lowered) query for a true case-sensitive comparison.
- **C++ template declarations produced duplicate symbols (P1 — Correctness)**: The `template_declaration` tree-sitter handler both pushed template children onto the stack and fell through to the generic push, causing inner nodes to be visited twice and symbols to appear twice in results.
- **Symbol search scanned all documents (P1 — Performance)**: `search_symbols()` called `trigram_index.all_documents()` regardless of query length, checking every indexed file. For queries ≥3 characters the trigram index is now used to narrow candidates first.
- **Line length penalty was too harsh (P1 — Quality)**: The previous `1.0 / (1.0 + len * 0.01)` factor dropped to 0.50 at 100 chars and 0.09 at 1000 chars, severely penalizing function signatures and long lines. Replaced with a gentler logarithmic curve `(1.0 / (1.0 + (len / 100.0).ln_1p())).max(0.3)`.
- **`FAST_RANKING_TOP_N` too low (P2 — Quality)**: Raised from 500 to 2000. The previous limit could cause relevant files with low base scores to be missed in large codebases.
- **Empty search results allocated unnecessary `Vec` (P2 — Performance)**: `search_in_document_scored()`, `search_in_document()`, and `search_in_document_regex()` returned `Some(Vec::new())` on no match. They now return `None`, eliminating spurious allocations.
- **Trigram bleed at filename/content boundary (P2 — Correctness)**: `format!("{}\n{}", filename_stem, content)` generated spurious trigrams spanning the boundary (e.g. the last chars of the filename joined with the first chars of the content). The separator is now three newlines (`"\n\n\n"`) so no meaningful trigram crosses the boundary.
- **Redundant trigram deduplication (P3 — Code quality)**: `TrigramIndex::search()` ran a three-step dedup (extract → `FxHashSet` filter → collect). Replaced with a single call to `extract_unique_trigrams()`.
- **Per-query filename stem allocation (P2 — Performance)**: `filename_matches_query()` re-computed `file_stem().to_lowercase()` on every file for every query. A pre-computed `lowercase_stem: String` field is now stored in `FileMetadata` at index time.
- **`FileName` symbol search returned wrong content**: `search_symbols_in_document()` fetched line 0 of the file content for `FileName` symbols, returning the first line of the file instead of the file path. It now uses the file path as the match content.

## [0.5.8] - 2026-02-26

### Fixed
- **Fix panic in `content_safety_check` on multi-byte UTF-8 content**: `is_binary_content()` sliced a `&str` at a raw byte offset (8192) which panics when that position falls inside a multi-byte UTF-8 character (e.g. CJK text, emoji). The function now operates on `as_bytes()` directly, avoiding the invalid slice.

## [0.5.7] - 2026-02-26

### Added
- **Automatic mmap limit detection**: On Linux the server now reads `vm.max_map_count` at startup, computes a safe 85 % ceiling, and refuses to map additional files once that ceiling is reached — preventing "cannot allocate memory" crashes on RHEL 7 / CentOS 7 and similar systems with low default limits.
- **Startup system-limits logging**: `SystemLimits::collect()` gathers `max_map_count`, current map count, open file descriptors, and `file-max`, then logs them at `INFO` level on every launch.
- **Startup warning for low limits**: When `vm.max_map_count < 131072` the server prints an actionable warning to stderr with both "with sudo" and "without sudo" remediation steps, then pauses 3 seconds before continuing.
- **`diagnose_mmap_error()` helper**: Mmap failures that look like resource-limit errors now produce a rich diagnostic message including current system limits and concrete fix instructions.
- **Deployment troubleshooting docs**: New "Memory Allocation Errors on RHEL7/CentOS7" section in `DEPLOYMENT.md` covering symptoms, root cause, automatic detection behaviour, and two solution paths (with/without sudo).

### Fixed
- **`include_extensions` config now honoured during indexing**: The field was parsed but never forwarded to file discovery; it now filters files correctly.
- **`max_file_size` config now honoured during indexing**: The configured value is forwarded to `FileDiscoveryConfig` instead of hardcoding 10 MB.
- **`watch = true` in config now starts the file watcher**: A background thread is spawned to detect and apply file-system changes.
- **`save_after_updates` now triggers periodic index saves**: The config field was previously ignored; it now saves every N files.
- **Import extraction extended to all supported variants**: `pyi`, `pyw`, `mjs`, `cjs`, `mts`, `cts` now handled.
- **Uniform `/health` status string**: Both keyword and semantic REST APIs now return `"healthy"`.
- **Config template corrected**: `index_path`, `save_after_build`, `save_after_updates`, `watch` placed under `[indexer]`; `telemetry.enabled` default fixed.

## [0.5.6] - 2026-02-26

### Fixed
- **Binary-detection tests use genuine binary bytes**: Test helpers in `file_store`, `lazy_file_store`, `engine`, and `integration_tests` now construct binary payloads from real non-UTF-8 byte sequences instead of synthetic strings, making the tests more robust and accurately representative of real-world binary files.

## [0.5.5] - 2026-02-26

### Changed
- Maintenance release: version/changelog update only; no functional code changes.

## [0.5.4] - 2026-02-25

### Added
- **`exclude_files` config option**: Explicitly exclude specific file paths from indexing.
  Use this when a file causes a crash — identify it via `fcs_last_processed.txt` then add it:
  ```toml
  [indexer]
  exclude_files = ["/repo/src/generated/bad_file.rs"]
  ```
- **Crash probe file `fcs_last_processed.txt`**: Written to the working directory before each
  tree-sitter symbol extraction call. After a SIGABRT crash, read this file to identify the
  exact file being processed at the time of the crash.
- **Per-file DEBUG logging**: Both indexing phases now emit `tracing::debug!` with the file path.
  Run with `RUST_LOG=fast_code_search=debug` to see a rolling log of files being processed,
  making the crashing file identifiable from the last log entry.

## [0.5.3] - 2026-02-25

### Added
- **Version logged on startup**: The binary version is now emitted as a structured log field
  (`version=X.Y.Z`) alongside the startup address line, making it easy to confirm which build
  is running.
- **Debug builds in release artifacts**: Each GitHub release now also ships a debug build
  (with full symbols) per platform, archived as `fast_code_search-VERSION-TARGET-debug.{tar.gz,zip}`.
  Use these builds when diagnosing crashes — `RUST_BACKTRACE=full` will show real function names.

## [0.5.2] - 2026-02-25

### Fixed
- **SIGABRT / heap corruption during parallel batch indexing**: Tree-sitter C parsers were being
  called concurrently from rayon's thread pool (`par_iter` over a 500-file batch). Tree-sitter's
  internal C allocator is not thread-safe under concurrent use, causing intermittent heap corruption
  manifesting as `memory allocation of N bytes failed` + `Aborted (core dumped)` after ~110 batches
  (~55K files). Fixed by splitting `PreIndexedFile::process` into two phases:
  1. `PartialIndexedFile::process` — pure-Rust trigram extraction, runs in parallel safely.
  2. `PreIndexedFile::from_partial` — tree-sitter symbol/import extraction, runs sequentially.
  Trigram extraction (the dominant CPU cost) remains parallel; only the tree-sitter FFI calls
  are serialised.

## [0.5.1] - 2026-02-25

### Fixed
- **OOM crash during background indexing**: `PreIndexedFile` was retaining a full copy of each file's content (`String`, up to 10 MB each) in memory while rayon processed batches in parallel. With a 500-file batch size this could accumulate up to 5 GB of redundant heap on top of the trigram index, causing an allocation failure on large codebases (~60K files). The `content` field has been removed from `PreIndexedFile` — it was only needed transiently inside `process()` to compute trigrams, symbols, and imports, all of which are now the only data carried into `index_batch`.

## [0.5.0] - 2026-02-25

### Added
- **HNSW vector index for semantic search**: Replaced linear O(n) search with HNSW O(log n) algorithm
  - New `hnsw_rs` dependency for fast approximate nearest neighbor search
  - Configurable HNSW parameters in `[indexer.hnsw]` section: `m`, `ef_construction`, `ef_search`
  - Default values optimized for moderate codebases (m=16, ef_construction=200, ef_search=100)
  - Embeddings now persisted alongside HNSW index for proper save/load functionality
  - 2-10x faster query performance for large codebases (>10K chunks)

### Changed
- Semantic search vector index now uses HNSW instead of linear search
- Updated documentation with HNSW configuration and tuning guide
- Memory overhead increased to ~3x indexed code size (from ~2x) due to HNSW graph structure

## [0.4.0] - 2026-02-09

### Added
- **OpenTelemetry distributed tracing**: Both server binaries now export traces via OTLP/gRPC
  - New `[telemetry]` TOML config section with `enabled`, `otlp_endpoint`, and `service_name`
  - Enabled by default — set `enabled = false` or `OTEL_SDK_DISABLED=true` to disable
  - Standard env var overrides: `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_SERVICE_NAME`, `FCS_TRACING_ENABLED`
  - New `src/telemetry.rs` module with `init_telemetry()` / `shutdown_telemetry()` lifecycle
  - Graceful shutdown flushes all pending spans before process exit

- **Tracing instrumentation across the stack**:
  - `#[tracing::instrument]` on all public search methods: `search`, `search_ranked`, `search_with_filter`, `search_with_filter_ranked`, `search_regex`, `search_symbols`
  - `#[tracing::instrument]` on semantic engine `search` method
  - `#[tracing::instrument]` on all gRPC handlers (keyword + semantic)
  - `tower_http::trace::TraceLayer` on both HTTP/REST routers
  - `Server::builder().trace_fn()` on both gRPC servers

- **Semantic model download verification**: Optional SHA256 verification via `FCS_MODEL_SHA256` or `FCS_MODEL_SHA256_<MODEL>`

### Changed
- Tracing subscriber upgraded from simple `FmtSubscriber` to layered `Registry` (fmt + OTel)
- `tracing-subscriber` now uses `registry` feature alongside `env-filter`
- `tower-http` now enables `trace` feature alongside `cors`
- Binary startup reordered: config loads before tracing init so TOML telemetry values are available
- `--init` flag exits before tracing init (no subscriber needed for template generation)

### Fixed
- **Persisted index loads now rebuild symbol and dependency caches** to restore symbol scoring boosts and import-based ranking
- **gRPC keyword service lock handling** now reports lock errors instead of panicking on poisoned locks

### Dependencies
- Added `opentelemetry` 0.27, `opentelemetry_sdk` 0.27 (rt-tokio), `opentelemetry-otlp` 0.27 (tonic), `tracing-opentelemetry` 0.28

## [0.3.0] - 2026-02-06

### Added
- **Expanded language support**: Tree-sitter symbol extraction now supports 18 languages (up from 4)
  - **Programming languages**: Rust, Python, JavaScript, TypeScript, Go, C, C++, Java, C#, Ruby, PHP, Bash
  - **Config & markup**: JSON, TOML, YAML, HTML, CSS, Markdown
  - Additional file extensions: `.pyi`, `.pyw`, `.mjs`, `.cjs`, `.mts`, `.cts`, `.cc`, `.cxx`, `.hpp`, `.hxx`, `.hh`, `.rake`, `.gemspec`, `.jsonc`, `.scss`

- **New symbol types extracted per language**:
  - Go: functions, methods, structs, interfaces, type aliases, constants, variables
  - C/C++: functions, structs, unions, enums, classes, namespaces (C++ templates traversed)
  - Java: classes, interfaces, enums, records, methods, constructors
  - C#: classes, structs, interfaces, enums, records, methods, properties, constructors
  - Ruby: classes, modules, methods (including singleton methods)
  - PHP: classes, interfaces, traits, functions, methods
  - Bash: functions

### Changed
- Symbol extractor now handles C/C++ function declarators correctly (name is in `declarator` field, not `name`)

## [0.2.4] - 2026-02-05

### Added
- **Two-phase ranking system for large-scale search**: Dramatically improves search performance on codebases with 100k+ files
  - Fast mode: Ranks candidates by pre-computed file metadata (symbols, imports, path), reads only top 2000 files
  - Full mode: Reads all candidate files for complete scoring
  - Auto mode (default): Automatically switches to Fast when >5,000 candidates
  - File-level scoring factors: symbol density (+4 max), src/lib location (+2), import count (+5 max), test/example penalty (0.7×)
  - New `rank` API parameter: `auto`, `fast`, or `full`
  - Response includes `rank_mode`, `total_candidates`, and `candidates_searched` metadata

- **Ranking mode UI toggle**: New dropdown in Advanced Options to select ranking mode
  - Results header shows actual mode used and files searched (e.g., "⚡ Fast (2000/100,000 files)")

- **Documentation page**: New `/docs.html` page in Web UI explaining the ranking system, API reference, and path filter patterns

- **Lazy file store**: Memory-mapped file content is now loaded on-demand rather than all at once
  - Reduces memory usage during index loading
  - ~8x faster index loading via parallel file mapping

### Changed
- Search methods now use file metadata ranking instead of early termination for large candidate sets
- `search_regex()` and `search_symbols()` also use fast ranking for consistency

## [0.2.3] - 2026-02-04

### Added
- **Terminal progress bars for index loading**: Added animated progress bars when loading a persisted index from disk
  - Shows phase-specific indicators: 📖 Reading file, 🔄 Deserializing, 🔍 Checking files, 🧠 Restoring index, 📁 Loading files
  - Progress bar shows elapsed time, visual progress, position/total count, and ETA
  - Displays summary with file count when loading completes

- **Whitebox validator binary**: New `fast_code_search_validator` binary for comprehensive search engine validation
  - Generates deterministic synthetic corpus with seeded randomness for reproducible tests
  - Multi-language corpus: Rust (40%), Python (25%), TypeScript (20%), JavaScript (15%)
  - Varied file sizes and complexity (functions, classes, nested structures)
  - Validates index completeness via embedded "needle" markers at known locations
  - Tests all query options: `search()`, `search_with_filter()`, `search_regex()`, `search_symbols()`
  - Verifies line number accuracy and symbol extraction
  - Optional load testing mode with throughput (queries/sec) and latency percentiles (p50/p95/p99)
  - JSON output mode for CI integration
  - Run with: `cargo run --release --bin fast_code_search_validator`

- **Index persistence**: The trigram index can now be saved to disk and loaded on restart for faster startup times
  - Configure `index_path` in config to enable persistence
  - `save_after_build = true` (default) saves after initial indexing completes
  - `save_after_updates = N` saves after N file updates via watcher (disabled by default)
  - Stores config fingerprint for detecting configuration changes
  - File locking ensures safe concurrent access (exclusive writes, shared reads)
  - Multiple read-only servers can share the same index file

- **Incremental reconciliation**: When loading a persisted index, the system reconciles against the current filesystem
  - Detects stale files via mtime + size checks and re-indexes them
  - Detects new files not in index and indexes them
  - Detects removed files and excludes them from results
  - Detects config changes (paths, extensions, excludes) and incrementally updates
  - Background reconciliation allows searches during the update process

- **New indexing status states**: UI shows `loading_index` when loading from disk and `reconciling` during background reconciliation

- **Symbols-only search mode**: New search mode that searches only in discovered symbol names (functions, classes, methods, types, etc.) plus filename matches. Enable via `symbols=true` query parameter in REST API or `symbols_only=true` in gRPC SearchRequest. This provides faster, more targeted results when looking for definitions.

### Changed
- **Updated performance comparison docs**: Added comprehensive benchmark comparison with traditional search tools (ripgrep, ag, git grep, grep) in README.md and PRIOR_ART.md. Includes published benchmark data from ripgrep's official benchmarks, break-even analysis for when indexing pays off, and detailed feature comparison tables.

### Fixed
- **ML models build on Windows**: Fixed ort 2.0 API compatibility issues and CRT linking conflicts
  - Updated `ort` dependency to use `load-dynamic` feature to avoid RuntimeLibrary mismatch with `tokenizers`
  - Fixed import paths for `Session`, `GraphOptimizationLevel`, and `Tensor` in ort 2.0 API
  - Added comprehensive Windows setup documentation in `docs/semantic/SEMANTIC_SEARCH_README.md`
  - Windows users must download ONNX Runtime DLL and set `ORT_DYLIB_PATH` environment variable
- **ONNX Runtime version compatibility**: Downgraded `ort` from 2.0.0-rc.11 to 2.0.0-rc.10
  - rc.11 requires ONNX Runtime >= 1.23.x, but bundled version is 1.22.0
  - rc.10 supports ONNX Runtime 1.18.x - 1.22.x
  - Added Windows launcher scripts (`scripts/run_semantic_server.ps1` and `.bat`) that automatically set `ORT_DYLIB_PATH`

## [0.2.2] - 2026-02-01

### Changed
- **Improved indexing log readability**: The background indexing completion message now formats large numbers with underscore separators (e.g., `89_210`), rounds `files_per_sec` to whole numbers, and removes redundant raw byte values.

## [0.2.1] - 2026-02-01

### Added
- **Indexing completion stats**: When background indexing completes, the log now reports the total size of indexed text and the current process memory usage in human-readable format (e.g., "150.00 MB"). Uses the `sysinfo` crate for cross-platform memory reporting.

## [0.2.0] - 2026-02-01

### Fixed
- **Duplicate search results**: Fixed issue where the same file could appear multiple times in search results when discovered via symlinks or different path representations. The `FileStore` now deduplicates files by canonical path.

### Added
- **Server-side search timing**: Search API response now includes `elapsed_ms` field showing the actual server-side query time in milliseconds. The Web UI displays this instead of client-side round-trip time.

## [0.1.0] - 2026-02-01

Initial release of fast_code_search — a high-performance, in-memory code search service built in Rust, designed to handle 10GB+ codebases with sub-millisecond query times.

### Core Features

#### Trigram-Based Indexing
- Roaring bitmap inverted index maps 3-character sequences to document IDs
- O(n) intersection for candidate document selection
- Efficient storage with compressed bitmaps

#### Memory-Mapped File Storage
- Zero-copy file access via `memmap2` crate
- Handles 10GB+ codebases without loading everything into RAM
- Files indexed in parallel for fast startup

#### Symbol-Aware Search
- Tree-sitter parsing for **Rust**, **Python**, **JavaScript**, and **TypeScript**
- Extracts symbol definitions (functions, classes, methods, types, etc.)
- Import/dependency tracking for enhanced relevance

#### Intelligent Scoring System
- **Symbol definitions**: 3x boost for symbol definitions
- **Exact matches**: 2x boost for case-sensitive matches
- **Source directories**: 1.5x boost for `src/` and `lib/` paths
- **Line position**: Boost for matches at start of lines
- **Dependency awareness**: Factors in file connectivity

#### Parallel Search Engine
- Rayon-powered parallel line-by-line search
- Concurrent processing across all CPU cores
- Sub-millisecond search on large codebases

### Server & API

#### gRPC Streaming API (Port 50051)
- `Search` RPC with streaming results for real-time UI updates
- `Index` RPC for adding directories to the search index
- Protocol Buffers schema in `proto/search.proto`
- Match types: `TEXT`, `SYMBOL_DEFINITION`, `SYMBOL_REFERENCE`

#### REST/JSON API (Port 8080)
- `GET /api/search?q=query&max=50` — Search with JSON response
- `GET /api/stats` — Index statistics (files, size, trigrams)
- `GET /api/status` — Indexing progress and status
- `GET /api/health` — Health check endpoint
- CORS-enabled for browser clients

#### Embedded Web UI
- Static HTML/CSS/JS served from embedded files
- Browser-based search interface at `http://localhost:8080`
- Real-time search results display

### Configuration

#### TOML Configuration Files
- Auto-discovers `fast_code_search.toml` or `config.toml`
- XDG/platform config directories supported
- CLI argument overrides for all settings

#### CLI Options
```
fast_code_search_server [OPTIONS]

Options:
  -c, --config <FILE>       Path to configuration file
  -a, --address <ADDR>      Server listen address
  -i, --index <PATH>        Additional paths to index (repeatable)
      --no-auto-index       Skip automatic indexing on startup
  -v, --verbose             Enable verbose logging
      --init <FILE>         Generate template configuration file
  -h, --help                Print help
  -V, --version             Print version
```

#### Exclude Patterns
- Glob-based exclusion: `node_modules/`, `target/`, `.git/`, `*.min.js`
- Configurable via `indexer.exclude_patterns` in TOML

### Developer Tools

#### Example Clients
- `examples/client.rs` — Basic gRPC client demonstrating index and search
- `examples/benchmark_client.rs` — Performance benchmarking tool

#### Build System
- `build.rs` compiles Protocol Buffers on each build
- Cross-platform support: Linux, macOS, Windows

### Performance Characteristics

| Metric | Value |
|--------|-------|
| Indexing speed | ~100MB/s on modern hardware |
| Search latency | Sub-millisecond for most queries |
| Memory usage | Fraction of codebase size (memory-mapped) |
| Max file size | 10MB (larger files skipped) |

### Requirements

- **Rust**: 1.70 or later
- **protoc**: Protocol Buffers compiler

### Installation

```bash
# Linux/Debian
sudo apt-get install protobuf-compiler

# macOS
brew install protobuf

# Build
cargo build --release
```

[Unreleased]: https://github.com/jburrow/fast_code_search/compare/v0.19.0...HEAD
[0.19.0]: https://github.com/jburrow/fast_code_search/compare/v0.18.1...v0.19.0
[0.18.1]: https://github.com/jburrow/fast_code_search/compare/v0.18.0...v0.18.1
[0.18.0]: https://github.com/jburrow/fast_code_search/compare/v0.17.0...v0.18.0
[0.17.0]: https://github.com/jburrow/fast_code_search/compare/v0.16.2...v0.17.0
[0.16.2]: https://github.com/jburrow/fast_code_search/compare/v0.16.1...v0.16.2
[0.16.1]: https://github.com/jburrow/fast_code_search/compare/v0.15.2...v0.16.1
[0.15.2]: https://github.com/jburrow/fast_code_search/compare/v0.15.1...v0.15.2
[0.15.1]: https://github.com/jburrow/fast_code_search/compare/v0.14.0...v0.15.1
[0.14.0]: https://github.com/jburrow/fast_code_search/compare/v0.13.0...v0.14.0
[0.13.0]: https://github.com/jburrow/fast_code_search/compare/v0.12.1...v0.13.0
[0.12.1]: https://github.com/jburrow/fast_code_search/compare/v0.12.0...v0.12.1
[0.12.0]: https://github.com/jburrow/fast_code_search/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/jburrow/fast_code_search/compare/v0.10.1...v0.11.0
[0.10.1]: https://github.com/jburrow/fast_code_search/compare/v0.10.0...v0.10.1
[0.10.0]: https://github.com/jburrow/fast_code_search/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/jburrow/fast_code_search/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/jburrow/fast_code_search/compare/v0.7.5...v0.8.0
[0.7.5]: https://github.com/jburrow/fast_code_search/compare/v0.7.4...v0.7.5
[0.7.4]: https://github.com/jburrow/fast_code_search/compare/v0.7.3...v0.7.4
[0.7.3]: https://github.com/jburrow/fast_code_search/compare/v0.7.2...v0.7.3
[0.7.2]: https://github.com/jburrow/fast_code_search/compare/v0.7.1...v0.7.2
[0.7.1]: https://github.com/jburrow/fast_code_search/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/jburrow/fast_code_search/compare/v0.6.9...v0.7.0
[0.6.9]: https://github.com/jburrow/fast_code_search/compare/v0.6.8...v0.6.9
[0.6.8]: https://github.com/jburrow/fast_code_search/compare/v0.6.7...v0.6.8
[0.6.7]: https://github.com/jburrow/fast_code_search/compare/v0.6.6...v0.6.7
[0.6.6]: https://github.com/jburrow/fast_code_search/compare/v0.6.5...v0.6.6
[0.6.5]: https://github.com/jburrow/fast_code_search/compare/v0.6.4...v0.6.5
[0.6.4]: https://github.com/jburrow/fast_code_search/compare/v0.6.3...v0.6.4
[0.6.3]: https://github.com/jburrow/fast_code_search/compare/v0.6.2...v0.6.3
[0.6.2]: https://github.com/jburrow/fast_code_search/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/jburrow/fast_code_search/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/jburrow/fast_code_search/compare/v0.5.8...v0.6.0
[0.5.8]: https://github.com/jburrow/fast_code_search/compare/v0.5.7...v0.5.8
[0.5.7]: https://github.com/jburrow/fast_code_search/compare/v0.5.6...v0.5.7
[0.5.6]: https://github.com/jburrow/fast_code_search/compare/v0.5.5...v0.5.6
[0.5.5]: https://github.com/jburrow/fast_code_search/compare/v0.5.4...v0.5.5
[0.5.4]: https://github.com/jburrow/fast_code_search/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/jburrow/fast_code_search/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/jburrow/fast_code_search/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/jburrow/fast_code_search/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/jburrow/fast_code_search/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/jburrow/fast_code_search/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/jburrow/fast_code_search/compare/v0.2.4...v0.3.0
[0.2.4]: https://github.com/jburrow/fast_code_search/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/jburrow/fast_code_search/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/jburrow/fast_code_search/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/jburrow/fast_code_search/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/jburrow/fast_code_search/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jburrow/fast_code_search/releases/tag/v0.1.0
