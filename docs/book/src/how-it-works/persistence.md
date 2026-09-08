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

## Load

1. Map the file, validate header, lengths, checksum and directory order.
2. **Stat pass**: one `stat` per persisted file, in parallel. A file whose
   mtime or size differs is *stale*; a file that is definitely gone
   (`ENOENT`) is *removed*; any other error keeps the entry, so a permission
   problem or an unmounted share cannot wipe a root from the checkpoint.
3. **Eligibility pass**: every surviving file is checked against the
   current configuration and `.gitignore` rules (gitignore matchers cached
   per directory), so a rule change takes effect without a rebuild.
4. Register files lazily (no reads), restore the posting lists from the
   mapping in parallel, restore symbols, references and edges — or
   re-extract them from content if the file was written by an older
   extractor — and rebuild the fast-ranking metadata.
5. Re-index the stale files, then discover anything new under the roots.

Roots are compared in canonical form, so a symlinked temp directory on
macOS or an 8.3 short name on Windows is the same root as its long
spelling.

## When a rebuild happens anyway

Only when the file's format version is older than the server's. The
message names the file; deleting it (or letting the server overwrite it)
is all that is needed.
