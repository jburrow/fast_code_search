# Incremental updates

With `watch = true` the server follows the tree and keeps the index
current without a rebuild.

## Watching

On Linux one inotify watch is installed per directory that survives the
exclude patterns, so `node_modules`, `target` and `.git/objects` cost no
watches at all; new directories get watches when they appear. macOS
(FSEvents) and Windows watch each root recursively. Events are coalesced
for the debounce interval (two seconds) and queued; the queue is bounded,
and if it ever fills, events are dropped with a warning and the sibling
check below heals what it can.

## Applying a batch

A batch is applied under one write lock in three phases:

1. **Deletes and renames-away** are collected and removed with a single
   pass over every posting list.
2. **Modified and created paths** are planned (directories expanded to
   their eligible files; vanished or newly ineligible paths dropped), then
   re-indexed together: their old postings stripped in one pass, the files
   read and parsed in parallel, and installed under their existing ids so
   nothing else has to be renumbered. Edges *into* a modified file are
   kept; a file that was deleted and comes back regains its dependents.
3. **Sibling check.** Some backends drop the delete half of a rename; the
   parent directories of every changed path are checked and entries whose
   file is gone are removed.

Between batches searches run normally; a search that arrives while the
lock is held gets a 503 with `Retry-After: 1`, which `fcs` and the web UI
retry on their own.

## Saving

After `save_after_updates` file changes the index is saved; it is also
saved on shutdown when anything changed since the last save, so an edit
made a second before Ctrl+C is on disk when the server starts again.
