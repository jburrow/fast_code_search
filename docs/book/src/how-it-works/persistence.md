# Persistence

A saved index turns a restart into a load of a few seconds. The
[file format](../reference/index-format.md) page describes the bytes; this
page describes the behaviour.

## Save

Saves are atomic and serialized: the engine's read lock is held while the
metadata, trigram directory and bitmaps are streamed into a uniquely
named temporary file next to the target, fsynced, and renamed into place.
Two saves cannot interleave, and a crash at any point leaves the previous
file intact.

There are two kinds of save:

- **Checkpoints**, while the server runs (during a long build, after it,
  and every `save_after_updates` changes), keep every file's id: a file
  removed since the load is written as an empty slot. Nothing is
  renumbered, and a posting list no change touched is copied from the
  mapped file as its bytes. Afterwards the server reads the posting lists
  from the file it just wrote and frees the copies it held, so the file it
  loaded at startup is released.
- **The save at shutdown** compacts: the empty slots go and every file
  after them is renumbered, so the file on disk has no gaps.

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
4. Register files lazily (no reads), restore symbols, references and
   edges — or re-extract them from content if the file was written by an
   older extractor — and rebuild the fast-ranking metadata. Every saved
   file gets its saved position back as its id, and the stale and removed
   ones (and a checkpoint's empty slots) are then tombstoned, as a removal
   at runtime is, so the posting lists are used as saved. (Renumbering the
   survivors instead rebuilt every posting list whenever a single file had
   changed, the largest cost of a load.) The gaps go at the save at
   shutdown.

   **The posting lists are not decoded.** The file stays mapped, and a
   query decodes only the lists for its own trigrams, about 13 µs per
   query at 302,000 files (a first page takes 2 to 86 ms). The lists stay
   in the page cache, which the OS can evict and share, instead of on the
   heap: at 302,000 files that is about 450 MB of heap a loaded server no
   longer holds. The mapped lists still name the files removed or
   re-indexed since the save; those ids are kept in a small set the mapped
   lists are read without, so a removal costs one bit instead of a pass
   over every list. Files indexed after the load are held on the heap and
   united with the mapped lists until the next checkpoint writes them out. On Windows, which cannot replace
   a file that is mapped, the lists are decoded at load as before. If the
   lists had to be renumbered (a path saved twice), they are decoded too.
5. Re-index the stale files that the current configuration and ignore
   files still allow, then discover anything new under the roots.
   **Only the folders that changed are read.** The last complete walk
   saved each folder's modification time and a fingerprint of its ignore
   files (`.gitignore`, `.ignore`, `.rgignore`, `.git/info/exclude`).
   Adding, deleting or renaming an entry changes a folder's time, so a
   folder whose time and ignore files match holds no new file. On restart
   every saved folder is stat'ed (in parallel). The changed ones are read
   one level deep, new folders are walked whole, and a folder whose ignore
   files changed is walked whole, since its rules apply below it. This is
   the trick git's untracked cache uses. At 302,000 files (28,616 folders)
   it took 0.18 s where a full walk took 2.6 s with the file system cache
   warm, and 1.3 s where a full walk took 9.2 s with it cold.

   A full walk happens instead when the configuration changed, the user's
   global excludes file (`core.excludesFile`) changed, a root is new, or
   the index has no stamps yet: a fresh build saves them once its walk
   ends, and a checkpoint from a build that did not finish has none. A
   folder modified within 2 seconds of the walk is not stamped (a coarse
   clock could give a later change the same time), and neither is one the
   walk could not read; the folders above them are read again on the next
   restart, so it reaches them.

   **Known limitation.** This relies on the file system updating a
   folder's modification time when an entry is added, removed or renamed,
   as local file systems do. On one that does not (some network or FUSE
   mounts), a file added while the server was stopped is found only when
   the watcher sees it change or its folder changes otherwise. Deleting
   the index file makes the next start walk everything.

Roots are compared in canonical form, so a symlinked temp directory on
macOS or an 8.3 short name on Windows is the same root as its long
spelling.

## When a rebuild happens anyway

Only when the file's format version differs from the server's, older or
newer (a downgrade rebuilds too). The
message names the file; deleting it (or letting the server overwrite it)
is all that is needed.
