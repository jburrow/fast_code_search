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
   single pass over every posting list; then the parsed files, at most
   512 per step, each step under its own write lock with a short pause
   between steps for the searches that arrived meanwhile. A modified file
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

A search that arrives during an install step waits for it (up to five
seconds) instead of being turned away. Only if the lock is still held
after that does it get a 503 with `Retry-After: 1` (gRPC: `UNAVAILABLE`),
which `fcs` and the web UI retry on their own. On a generated tree of one
million Python files with 2,000 files rewritten a second, no search was
turned away and the slowest took 0.2 s (see `examples/watch_load.rs`).

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
searches continuously while a burst of changes is applied and fails if a
search is turned away or waits too long.

For larger trees, `examples/watch_load.rs` generates a tree of small
scripts (a million, if you like), streams rewrites, creates and deletes
into it through the real watch loop while threads search, and reports how
many searches were answered or turned away and the latency percentiles:

```text
cargo run --release --example watch_load -- --dir /tmp/fcs-load --files 1000000
```
