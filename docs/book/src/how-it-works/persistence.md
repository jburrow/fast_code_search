# Persistence

A saved index turns a restart into a load of a few seconds. The
[file format](../reference/index-format.md) page describes the bytes; this
page describes the behaviour.

## Save

Saves are atomic and serialized: the engine's read lock is held while the
metadata, trigram directory and bitmaps are streamed into a uniquely
named temporary file next to the target, fsynced, and renamed into place.
Two saves cannot interleave, and a crash at any point leaves the previous
file intact. Checkpoints during a long build use the same path.

- **A save that is killed** (`kill -9`, a power cut, the out-of-memory
  killer) leaves its temporary file (`<index_path>` with `.tmp.<pid>.<n>`
  in place of its extension, as large as the index). The next server to
  take the index lock removes them before it loads ("Removed temp files
  left by an index save that did not finish" in the log).
- **A failed rename** (Windows refuses to replace a file another process
  has open) is tried once more with the old index moved aside; if that
  fails too, the old index is put back. The save fails, and the index on
  disk is the previous one.
- **A failed save** (disk full) is logged as an error, and with
  `save_after_updates` the updates it held still count: the next save,
  after more updates or at shutdown, writes them. The index on disk is
  older until then, so the next start re-reads the files changed since.

`tests/fault_injection.rs` fails the write and the rename on purpose
(fault points `save.write` and `save.rename`), and `examples/chaos.rs`
kills the server binary during saves, builds and shutdowns and checks that
the next start loads the index and catches up (see
[How fcs is tested](testing.md#killed-at-any-moment)).

## One server per index

A server takes an exclusive lock on `<index_path>.lock` before it loads
and records its PID and listen addresses in `<index_path>.lock.owner`.
A second server started with the same `index_path` logs which process
holds it, loads the index read-only and never saves, so two servers can no
longer overwrite each other's file. The lock is released when the process
exits, however it exits.

## Load

1. Map the file, validate header, lengths, checksum and directory order.
2. **Stat pass**: one `stat` per persisted file, in parallel. A file whose
   mtime or size differs is *stale* (a file that was read within 3 seconds
   of being modified is saved with no time, so it is always stale: on a
   file system with coarse timestamps it could have changed in the same
   tick); a file that is definitely gone
   (`ENOENT`) is *removed*; any other error keeps the entry, so a permission
   problem or an unmounted share cannot wipe a root from the checkpoint.
3. **Eligibility pass**: every surviving file is checked against the
   current configuration and `.gitignore` rules (gitignore matchers cached
   per directory), so a rule change takes effect without a rebuild.
4. Register files lazily (no reads), restore the posting lists from the
   mapping in parallel, restore symbols, references and edges — or
   re-extract them from content if the file was written by an older
   extractor — and rebuild the fast-ranking metadata.
5. Re-index the stale files that the current configuration and ignore
   files still allow, then discover anything new under the roots.

Roots are compared in canonical form, so a symlinked temp directory on
macOS or an 8.3 short name on Windows is the same root as its long
spelling.

## When a rebuild happens anyway

Only when the file's format version differs from the server's, older or
newer (a downgrade rebuilds too). The
message names the file; deleting it (or letting the server overwrite it)
is all that is needed.
