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

A batch is applied under one write lock in three phases:

1. **Deletes and renames-away** are collected and removed with a single
   pass over every posting list.
2. **Modified and created paths** are planned (directories reconciled
   as above; vanished or newly ineligible paths dropped), then
   re-indexed together: their old postings stripped in one pass, the files
   read and parsed in parallel, and installed under their existing ids so
   nothing else has to be renumbered. Edges *into* a modified file are
   kept; a file that was deleted and comes back regains its dependents.
3. **Sibling check.** As a last safety net, the parent directories of
   every changed path are checked and entries whose file is gone are
   removed.

Between batches searches run normally; a search that arrives while the
lock is held gets a 503 with `Retry-After: 1`, which `fcs` and the web UI
retry on their own.

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
`FCS_WATCH_SEED=<seed> cargo test --test watch_harness`.
