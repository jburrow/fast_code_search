# C1: immutable segments and lock-free readers

Status: accepted 2026-10-25; C1.0, C1.1 and C1.2 built (`examples/posting_decode.rs`,
`TrigramIndex::postings`, mapped posting lists), C1.3 re-scoped in 3.10, the rest to follow. Plan item C1 of the
2026-10-25 scale and performance plan.

## 1. The problem in one paragraph

The whole index is one `SearchEngine` behind one `RwLock`. Searches read it,
and the watcher, reconcile, reindex and the save all write or hold it. Three
costs follow, and they grow with the number of files:

| Cost | Where | At 302k files today |
|---|---|---|
| Load deserializes every posting list onto the heap | `PersistedIndex::restore_trigram_index` | postings 457 MB of heap (a third of it); about 3 s of a 17 s load (see 3.9) |
| Removing a file scans every posting list | `TrigramIndex::remove_documents` | one parallel pass over every list per watcher batch |
| Save holds the read lock for the whole write, and writers wait for a moment with no search | `save_index_if_needed`, `engine_lock.rs` | save about 16 s under the lock; the hold-back / defer machinery in `engine_lock.rs` exists only to stop writers starving |

`engine_lock.rs` is careful and well tested, but it is a workaround: it decides
who waits. C1 removes the waiting.

## 2. Goals and non-goals

Goals:

1. A search never waits for a writer, and is never answered "index is being
   updated". It reads one consistent version of the index.
2. Load is a map plus the stat pass (with C1.6 for symbols). Posting lists are read from the mapped
   file, not copied onto the heap.
3. Removing or changing a file costs the size of the change, not the size of
   the index.
4. The save is a background merge that holds no lock.
5. Same results. Every oracle from #183 to #191 passes unchanged, and so do
   the ripgrep backtest and the scoreboard.

Non-goals: sharding across machines, a new query language, a new ranking.
Search reads of file content (`LazyFileStore::read`) stay as they are (C3
measures them first).

## 3. The design

### 3.1 Snapshots

```text
           writer (one at a time: watcher, reconcile, reindex)
                 |
                 | builds the next Snapshot, then swaps one pointer
                 v
  current: ArcSwap<Snapshot>  <---- every search: load() the Arc, no lock
                 |
  Snapshot {
      generation,
      base:     Arc<BaseSegment>,         // the mapped index file
      deltas:   Vec<Arc<DeltaSegment>>,   // small, one per watcher batch, merged in tiers
      dead:     Arc<RoaringBitmap>,       // global ids removed since their segment was written
      graph:    Arc<DependencyGraph>,
      freshness: IndexFreshness,          // what this version knows it is missing
  }
```

A search takes `current.load_full()` at the start and uses that `Arc` to the
end. A writer never mutates a published snapshot. It builds the next one
(sharing every unchanged segment by `Arc`) and stores it. A snapshot that no
search holds is freed when its last `Arc` drops.

`arc-swap` is a small, widely used crate made for this. The std-only
alternative is a `RwLock<Arc<Snapshot>>` held only to clone or replace the
pointer (nanoseconds) works too; `arc-swap` was chosen (section 7). Either way only the writers share a mutex, for their own order.

### 3.2 Segments and ids

Each segment holds a contiguous range of global file ids: `offset ..
offset + len`. Inside a segment, posting lists, metadata, symbols and
references use the local id. A global id is `offset + local`.

* **Base segment:** the saved index file, mapped. The directory is already
  fixed-width and sorted (`DirEntry`, 16 bytes), so a trigram lookup is a
  binary search in place. A posting list is decoded from its bytes only when
  a query needs it.
* **Delta segment:** what one watcher batch (or one reconcile step) indexed,
  as today's in-memory structures, then frozen. A changed file gets a new id
  in the new delta, and its old id goes into `dead`.
* **Dead bitmap:** global ids of files removed or replaced since their
  segment was written. Removing a file is one bit, not a pass over every
  posting list.

A text search is then:

```text
candidates = (base.search(q) | delta_1.search(q) | ... ) - dead
```

and `all_documents` is the same union of each segment's live set, minus
`dead`. Ranking, path filters and graph scopes work on global ids as today.

Ids never leave a request: the web API, gRPC and MCP pass paths between
requests (checked: `find_file_id(&params.file)` in `web/api.rs`), so ids may
change between snapshots. Offset paging re-runs the query, as today.

### 3.3 Reading posting lists in place

roaring-rs 0.11 has no zero-copy view of a serialized bitmap, so "in place"
means: the bytes stay in the page cache, and a query decodes only the lists
it touches. At 302k files a dense list is at most five 8 KiB bitset
containers, and ubiquitous trigrams are run containers of a few bytes
(`finalize` optimizes them). Decoding the 5 to 20 lists a query touches
should cost microseconds next to a 2 to 86 ms first page, but this is a
guess until the spike (PR C1.0) measures it on the real corpus. If it is
not cheap, the fallbacks in order are: a small cache of decoded hot lists,
then intersecting serialized containers directly (the roaring format is
simple), then the `croaring` crate's frozen views.

### 3.4 The rest of the engine

| Today (in `SearchEngine`) | With C1 |
|---|---|
| `trigram_index` | per segment, as above |
| `file_store` (paths, `DirIndex`, tombstones) | per segment path table (id to path, and path to id). A reader finds a path by asking the newest segment first, then older ones, skipping dead ids. The watcher keeps its own `path -> global id` map and `DirIndex`, which it changes in place because no reader sees them. Nothing of index size is copied per batch (see 3.8) |
| `symbol_cache`, `reference_cache`, `ref_names` | per segment. Base: decoded at load in the first PRs, then packed and read in place with B3's symbol name index |
| `file_metadata`, `indexed_meta`, `racy_files` | per segment |
| `dependency_index`, import specs, waiting imports | writer-owned. Readers get a frozen base graph plus a small overlay of edges added and removed since, folded into the base at a merge. Never a clone of the whole graph per batch (see 3.8) |
| `regex_cache` | stays a `Mutex` beside the snapshot (it is a cache, not index state) |
| `building`, `watching`, `watch_backlog`, `synced_at_ms` | in the snapshot's `freshness`, so a search reports the state of the version it read |

### 3.5 Merge is the save

A background thread takes the current snapshot `S` and writes a new base
file from it: the base's live files plus every delta, minus `dead`. Posting
lists are written by a k-way merge of the base directory (already sorted)
with each delta's sorted trigrams, one trigram at a time, so the merge never
builds a whole map on the heap. Ids are compacted (dead ids dropped).

When the file is written and synced, the writer swaps in a snapshot whose
base is the new file. Deltas created after `S` stay, with new offsets
(their posting lists use local ids, so only the offset changes). Their
entries in `dead` that point into the old base are remapped through the
old-to-new id map, which only covers files removed after `S`, so it is small.

When to merge: when the deltas pass a threshold (a share of the base, or a
number of segments; measured, not a round number, per CLAUDE.md), and at
shutdown. Small deltas are merged with each other first (tiered, as Lucene
and Zoekt do), so a search reads a handful of segments at most.

Crash safety gets better: deltas are written to disk as they merge (3.8),
so a crash loses only the newest few seconds of changes, and the restart
reconcile (and C2's directory-mtime walk) finds those files changed on
disk. Today a watched index is only saved at shutdown unless
`save_after_updates` is set.

### 3.6 Files on disk and Windows

Windows will not replace or delete a file that is mapped. So a merge cannot
rename over `index.bin` while old snapshots still map it. Base files get a
generation in their name (`index.<gen>.bin`); startup opens the newest that
passes its CRC and deletes older ones; a running server deletes an old
generation when its last snapshot drops. `remove_stale_temp_files` grows to
cover old generations. The index lock (`index_lock.rs`) still guarantees one
writer per index directory.

SIGBUS (the mapped file truncated under us) can only happen if something
other than fcs rewrites the file in place. fcs never does: it writes a new
generation and the lock keeps a second server out. It is documented as a
known limitation rather than coded around.

Disk: a merge needs the old and new base at once, about 2x the index size.
The save already checks for 2x free space (`check_index_storage`), so that
check moves to the merge.

### 3.7 What gets deleted

`engine_lock.rs`'s hold-back, adaptive defer, `SEARCH_READ_WAIT` and the
"index is being updated, try again" answer. `tests/lock_sim.rs` is rewritten
to assert the new property: a search's latency does not depend on what the
writer is doing. `TrigramIndex::remove_documents`' full pass goes too.

### 3.8 A constant stream of changes on a million files

The case: 1,000,000 files, and a batch of changes every second, all day
(a build writing outputs, a generator, an agent editing). The design has
to hold one rule for it: **a batch costs the size of the batch, never the
size of the index.** Anything that copies, scans or rewrites something of
index size per batch is a bug, at any of these steps:

| Step, per batch | Cost | Why it stays small |
|---|---|---|
| Parse the changed files | the changed files | as today |
| New delta segment | the changed files' trigrams | built fresh, then frozen |
| Remove the old versions | one bit per file in `dead` | today this is a pass over every posting list (about 3 GB of them at 1M) under the write lock, every batch |
| Paths | the changed paths | per segment path tables; the watcher's own map changes in place (3.4) |
| Graph | the changed edges | an overlay of added and removed edges, not a cloned graph (3.4) |
| Publish | one pointer swap, plus a copy of the segment list and of `dead` | both are small: a list of at most a few dozen segments, and a bitmap of what changed since the last merge |
| Searches | never wait | they keep reading the snapshot they started with |

**Segments do not pile up.** One delta a second is 3,600 an hour. Deltas
are merged in tiers: when ten deltas of one size exist, a background merge
writes them as one of the next size up, dropping dead ids. Each file's
change is rewritten about once per tier, so the merge work is the change
volume times a handful, and a search reads at most a few dozen segments.
If the writer falls behind, pending events are coalesced into one bigger
batch, as the watcher already does for bursts.

**The big rewrite is rare.** The base (a few GB at 1M) is rewritten only
when the deltas or the dead ids pass a share of it, not on a clock. A
stream that keeps editing the same files kills ids in the deltas, not in
the base, so the base rewrite can be days apart. When it runs it is one
streaming pass in the background, with no lock, and needs disk for a
second base until it finishes.

**A crash loses seconds, not the day.** Today a watched index is not saved
until shutdown unless `save_after_updates` is set (default 0), and a save
rewrites the whole index under the read lock (about 55 s at 1M, inferred).
With C1 each tier merge writes its segment as its own file, and a small
manifest names the live set of files and the dead ids. After a crash the
server maps the last manifest and only the last seconds of changes need
the restart reconcile (C2 makes that walk short too).

**A long search pins an old snapshot.** A regex over every file can run
for seconds. It keeps its snapshot's segments alive until it ends: memory
for a few small deltas, or, if a base merge finished meanwhile, the old
base file on disk for that long. It never blocks the writer.

**What to measure before calling it done** (in the scoreboard and
`watch_load`): the writer's instructions per batch at 100k and at 1M files
are the same within noise; under a 1-second stream at 1M, search p99 is
the same as with no stream, and `fresh_secs` stays about a batch's parse
time; segment count stays bounded over an hour.

This section changes the design in three places, all folded in above and in 3.5:
paths and the graph are per segment or overlays instead of whole copies
published per batch, and deltas are written to disk as they merge instead
of only at a base merge.

## 3.9 Measured in C1.0 (2026-10-25)

On the 302k real corpus (Linux, LLVM, Rust, Kubernetes; 4 vCPU VM),
`examples/posting_decode.rs` over 80 queries (the corpus benchmark's and
every plain backtest query):

* **Decoding in place is cheap.** Finding a query's lists in the mapped
  directory and decoding them adds a median of 13 µs per query over
  intersecting lists already on the heap, at most 254 µs. A first page
  takes 2 to 86 ms. No cache of decoded lists is needed.
* **Postings are smaller than the plan assumed.** With A4's corrected
  accounting they are 457 MB of heap (31% of 1.50 GB resident), not about
  1 GB. Symbols are 323 MB and graph paths 146 MB.
* **Load is not spent on postings.** A server-style load took 17.0 s:
  read and decode the file's metadata 2.7 s, posting lists 2.9 s (0.33 s
  of it decoding; the rest is renumbering every list because 5 files had
  changed), symbols and imports 8.7 s, file metadata 1.2 s, the rest
  checks and registration.

So C1.2 buys the memory (about a third of the heap) and the removal cost,
but only about 3 s of load. The load win is in symbols and imports: C1.6
(packed symbols read in place, with B3) and B6 (profile the load) matter
more for load than C1.2 does. The table below is corrected for this.

## 3.10 Measured after C1.2 (2026-10-27)

C1.2 and the checkpoint a server saves after its build (`save_after_build`)
already changed what a watcher batch costs. Before writing C1.3, it was
measured again (4 vCPU VM, mimalloc):

* **One file's update no longer depends on the index's size.** After the
  checkpoint, a one-file update takes 2.8 ms p50 on tokio + Django (6,300
  files) and 2.95 ms p50 at 302,433 files (`corpus_bench`, #232). That was
  goal 3 of section 2 for the trigram index. Before #232 the benchmark
  timed the update on the just-built index, where it passed over every
  posting list (85% of its instructions); a server never does that.
* **A light stream does not slow searches.** `watch_load --mixed` on
  100,000 generated files, four threads searching on four cores: with no
  changes, search p50 0.96 to 1.0 s and p99 2.0 to 2.2 s. With 20 files
  rewritten a second, p50 1.0 s and p99 1.9 s.
* **A heavy stream does.** With 2,000 files rewritten a second (a branch
  switch, a formatter), search p50 rose to 1.6 to 2.0 s and p99 to 3.6 to
  4.2 s. The watcher's install steps held the write lock 1.3 to 2.8 s
  each, and 16,000 changed files took 92 s to install. Most of that hold
  is the lock protocol, not the work. A step stays in about as long as it
  waited to get in, up to `MAX_HOLD` (1 s), because a writer only gets in
  between searches. That is the trade `engine_lock.rs` makes between drain
  speed and search waits.
* **The work under the lock is building posting lists.** Installing 512
  changed files (about 5 MB) takes 120 to 310 ms on an idle machine, at
  20,000 and at 100,000 files alike. Callgrind on that step: 76% is
  `TrigramIndex::add_document_trigrams`, which does one roaring insert per
  trigram per file. Then 16% is `refresh_file_metadata`, inflated here because the
  probe indexed 20 roots and each path is matched against every root.
  The graph and path tables come to about 10%. About 0.46 M instructions per file.

So C1.3 is re-scoped. The per-batch removal cost it was meant to remove is
gone already. What is left is that the batch's posting lists, symbols and
metadata are built *under* the write lock, and that searches and the
writer take turns. C1.3 builds each batch as a delta segment off the lock,
and its lock-held work becomes "publish the delta". C1.4 then removes the
turn-taking altogether. Together they make the heavy-stream case behave
like the light one: searches stop waiting for install steps, and a
backlog drains at the speed of parsing, not of the gaps between searches.
The goal in section 3.8 is unchanged: under a stream, search p99 equals
p99 with no stream, measured with `watch_load --mixed --burst 200
--interval-ms 100`.

An index kept in memory only (no `index_path`) never checkpoints, so its
watcher batches still pass over every posting list until C1.3 makes
removal one bit there too.

## 4. What it should buy (expected; the PRs measure it)

| Today at 302k (1M, inferred) | With C1 (expected) |
|---|---|
| load 17 to 24 s (about 60 to 80 s) | C1.2: about 3 s less. With C1.6 (symbols in place): map + stat pass, a few seconds |
| postings 457 MB of heap (about 1.5 GB) | page cache, which the OS can evict and share |
| save holds the read lock 16 s (about 55 s) | no lock; merge in the background |
| remove = pass over every posting list | one bit per file |
| searches wait up to 5 s or are turned away | never wait |

Costs to expect and name in the PRs: each search unions a few segments and
decodes the lists it touches (the spike measures it); the merge uses CPU and
disk in the background; memory reports change meaning, because the base
counts as page cache instead of heap (`index.memory` and the book's memory
page need to say so).

## 5. PR sequence

Each PR keeps every oracle, the backtest, `ab.sh --corpus small,real` and
the scoreboard green, and names any slowdown.

| PR | What | Behaviour change |
|---|---|---|
| C1.0 | Spike and measurements: decode cost per query on the real corpus, graph clone cost, snapshot build cost per batch. A short report, no product code | none |
| C1.1 | Postings behind an API. Checked: outside the engine, search code touches only `file_store.get/len` and a few graph calls, and inside it posting lists are reached from the search, the self-check and the save. So the self-check goes through `postings()` / `contains()` now, and the save is rewritten in C1.2. Shipped with C1.0 in one PR | none |
| C1.2 | Base segment read in place, plus the dead bitmap. Load stops deserializing postings; removal stops scanning. Still under today's lock. Format v9, one rebuild on upgrade | load, memory, remove cost |
| C1.3 | Delta segments with local ids, built off the lock; per-segment symbols, refs and metadata (re-scoped in 3.10) | batch install off the lock; removal one bit for an in-memory index too |
| C1.4 | Snapshots published by pointer; searches lock-free; `engine_lock.rs` hold-back deleted; `lock_sim` rewritten | no waiting, no 503 |
| C1.5 | Tiered background merges; deltas written as segment files with a manifest; base merge as the save; generation files; Windows deletion | save off the lock, crash loses seconds |
| C1.6 | Packed base symbols read in place (with B3) | load, memory |

C1.2 alone delivers most of the load and memory win, so if the later steps
stall, the work still pays.

## 6. Tests that carry the change

* The oracles (#183 to #191): fresh build versus watched index, same results.
* New: merge equivalence. A merged base equals a fresh build of the same
  files, compared after canonicalising ids.
* New: snapshot isolation. A search started before a batch sees none of it,
  one started after sees all of it, under `sim` time.
* New: crash during merge (`faults.rs` points in the merge writer): the
  previous generation loads and the reconcile repairs the rest.
* New: per-batch cost independent of index size. The writer's instruction
  count for the same batch on a 100k and a 1M index, within noise, and a
  1-second stream at 1M in the scoreboard with p99 unchanged.
* New on Windows CI: an old generation is deleted once its last snapshot
  drops, and never while one is held.

## 7. Decisions (2026-10-25)

1. The format bump in C1.2 is accepted: every user rebuilds once on upgrade.
2. `arc-swap` for publishing snapshots.
3. Memory reports give heap and mapped bytes separately.
4. C1.0 and C1.1 go ahead of the B items; the rest follows B2.
