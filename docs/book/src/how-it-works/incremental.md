# Incremental updates

With `watch = true` the server follows the tree and keeps the index
current without a rebuild.

## Watching

On Linux one inotify watch is installed per directory that survives the
exclude patterns, so `node_modules`, `target` and `.git/objects` cost no
watches at all; new directories get watches when they appear. macOS
(FSEvents) and Windows watch each root recursively.

Events are treated as **hints**, not facts. An event only says which
paths may have changed; when the batch is applied, each path is checked
against the disk: a file is indexed or dropped, and a directory is
reconciled (new or changed files indexed, files that are gone removed).
Backends merge, reorder and lose events, so nothing depends on the kind
of an event or on matching the two halves of a rename. Events are
gathered until there has been a quiet gap of 200 ms (at most two
seconds), then applied as one batch.

When events are known to be lost (the OS reports an overflow, or the
bounded queue fills up), the watched roots are reconciled with the disk
instead. Reconciling skips files whose size and modification time match
what was indexed, so it costs a walk and a stat per file, not a rebuild.

## Changes during the initial build

Files are read outside the write lock, so a file can be edited, renamed
or deleted between being read by the initial build and being merged into
the index. Before a batch is merged each file is checked again: content
that is no longer on disk is never installed, a deleted file is skipped,
and a rewritten one is read again.

## Applying a batch

A batch is applied in three steps, and only the last one takes the write
lock, so searches keep being answered while a batch is read from disk:

1. **Plan** (under a read lock, which searches share). Events are
   coalesced per path. Deleted paths and directories are looked up;
   modified paths are checked against the disk (directories reconciled as
   above; vanished or newly ineligible paths dropped). As a safety net for
   backends that lose the delete half of a rename, the parent directory of
   every changed path is listed and entries whose file is gone are
   dropped. The index keeps its files by directory, so all of this costs
   the size of the batch and the directories it touches, not the size of
   the index.
2. **Read and parse** (no lock). The files to (re)index are read and
   parsed in parallel on the watcher's own threads (half the cores), so
   the searches running on the shared thread pool are not queued behind
   a burst.
3. **Install** (write lock, in steps). The removals go first, with a
   single pass over every posting list; then the parsed files, 512 at a
   time, each step under its own write lock with a short pause between
   steps for the searches that arrived meanwhile. A step that had to wait
   for searches to get the lock keeps installing for about as long as it
   waited, up to one second (see "Catching up" below). A modified file
   keeps its id; edges *into* it are kept, and a file that was deleted
   and comes back regains its dependents. A path is looked up again at
   install time, since the index may have changed since it was planned
   (the initial build can run alongside), and a file that changed again
   since it was read has an event of its own on the way.

**Imports.** Every file's imports are kept as written. After each step
the files whose imports could resolve differently now are resolved again:
those that imported a removed file, and those with an import naming a
file that appeared. That covers an import that fell back to a package
(`from . import m` reaching `__init__.py` while `m.py` is missing) moving
to the new file, and back. The dependency graph is therefore a function
of the files on disk, the same as a fresh build would draw.

## Searching while changes are applied

Searches and the watcher share one lock: searches read under it, install
steps write. A writer that *queues* for such a lock makes it turn every new
reader away until the writer has had its turn, and the writer gets its
turn only when every running search has finished. So one slow search (a
regex over a million files) and a one-file install step used to stop every
other search for as long as the slow one ran, and searches still waiting
after five seconds got "Index is currently being updated".

Writers no longer queue. An install step (and each batch of the initial
build) takes the write lock in a moment when no search holds it, trying
every couple of milliseconds. While it waits, a new search holds back for
up to 250 ms so the running ones can finish and the step can get in, and
then goes ahead whether or not it did. A search therefore waits for a
step's short hold (tens of milliseconds), never for other searches. A
search that arrives during a step waits for it (about a second at most);
one still waiting after five seconds gets a 503 with `Retry-After: 1`
(gRPC: `UNAVAILABLE`), which `fcs` and the web UI retry on their own.

While changes wait to be applied, searches see the index as it was. They
say so rather than pass it off as current: the `index` part of every
search response carries `pending_changes`, the events the watcher has
queued plus the files of the burst being applied, and the web UI shows
"Index updating · N file changes to apply" beside the timing; `fcs` and the
MCP tools print the same as a note.

Measured with `examples/watch_load.rs --mixed` on a generated tree of one
million Python files (4 cores, four threads searching back to back with a
mix of cheap, common-word and regex queries, the way the web handler runs
them):

| Load | Version | Turned away | Slowest search | p99 | Mean | Files applied |
|---|---|---|---|---|---|---|
| 50,000 files rewritten at once, every 10 s, for 40 s | before | 9 (3.0%) | 5.1 s | 4.3 s | 433 ms | 24,550 |
| | after | 0 | 1.5 s | 1.3 s | 684 ms | 138,739 |
| 200 files rewritten every 100 ms, for 30 s | before | 4 (1.8%) | 5.0 s | 4.5 s | 511 ms | 10,643 |
| | after | 0 | 1.4 s | 1.1 s | 591 ms | 31,757 |

"Files applied" counts what the watcher had installed when the run ended:
before, searches held it back and most of the changes were still waiting.
The mean search is slower after (433 to 684 ms in the bulk run) for two
reasons: the watcher now does three to six times the work in the same time
on the same four cores, and a search holds back while a step waits. In
exchange, no search waits seconds and none is turned away. "Files applied"
can exceed the files rewritten: a file rewritten twice is installed twice,
and in the bulk run the watcher's event queue filled, so it reconciled the
tree as well. Without that holding back
the watcher did not get in once in a 30 s run on a 200,000-file tree, so
nothing was applied at all.

## Catching up

A step gets the lock only once the searches running at that moment have
finished, so while people search each step costs about one search's
wait. With a fixed 512 files per step that made the watcher crawl: on
the 200,000-file tree with four threads searching (about half a second
per search), a change of 100,000 files was applied at about 400 files a
second and was still 17,000 files behind four minutes after the change.
The server looked as if it was updating for ever, with the "file changes
to apply" count barely moving. Nobody searching, the same change applied
at about 5,000 files a second.

A step now stays in about as long as it waited to get in, capped at one
second: the watcher installs at least as much as it waits, so a backlog
drains at half speed or better however busy the searches are, and a
search waits at most about a second for a step. `watch_load` now keeps
searching after the changes stop and measures how long until the index
matches the disk again:

| Tree | Change | Version | Index matches disk | Slowest search | Mean search |
|---|---|---|---|---|---|
| 200,000 files | 100,000 rewritten at once | before | not after 240 s (17,200 files behind) | 2.2 s | 547 ms |
| | | after | 10 s after the change stopped | 2.1 s | 717 ms |
| 1,000,000 files | 200,000 rewritten at once | after | 214 s | 2.3 s | 866 ms |

No search was turned away in any of these runs. The million-file run
overflowed the watcher's event queue twice, so it reconciled the whole
tree as well (about 40 s of walking a million files, during which the
pending count does not move); the rest is installing.

## Saving

After `save_after_updates` file changes the index is saved; it is also
saved on shutdown when anything changed since the last save, so an edit
made a second before Ctrl+C is on disk when the server starts again.

## How this is tested

`tests/watch_harness.rs` runs the server's own watch loop over a
generated Python and TypeScript tree, changes it the way people and tools
do (edits, editor-style atomic saves, renames across directories,
directory renames and deletes, moves into excluded directories, bursts of
dozens of changes, changes during the initial build), then compares the
live index with a fresh index of the same disk: the file set, every hit
for every vocabulary word, symbol definitions and dependency edges. The
random sequences print their seed; replay one with
`FCS_WATCH_SEED=<seed> cargo test --test watch_harness`. Another test
holds a long search while a burst waits and checks that the step that
waited installs more than one chunk. Another test
searches continuously while a burst of changes is applied and fails if a
search is turned away or waits too long; another holds a long search while
a burst waits to be installed and checks that other searches are answered
and report the changes still to apply.

For larger trees, `examples/watch_load.rs` generates a tree of small
scripts (a million, if you like), streams rewrites, creates and deletes
into it through the real watch loop while threads search, and reports how
many searches were answered or turned away and the latency percentiles.
`--mixed` searches the way the web handler does, with queries that read
most of the tree; a large `--burst` with a long `--interval-ms` is a bulk
change such as a branch switch. After the changes stop it keeps searching
until the index matches the disk again and reports how long that took, or
"NOT SETTLED" after `--settle-seconds`; `--index-path` adds checkpoint
saves:

```text
cargo run --release --example watch_load -- --dir /tmp/fcs-load --files 1000000 \
    --burst 50000 --interval-ms 10000 --seconds 40 --mixed
```
