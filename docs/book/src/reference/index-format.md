# Index file format

The persisted index (`index_path`) is a single file, format **v13**, magic
`FCSIDX10`. Everything is little-endian.

```text
magic[8]  version:u32  crc:u32  meta_len:u64  dir_count:u32  bitmaps_len:u64   (36-byte header)
META     bincode-encoded metadata (see below)
DIR      dir_count × { trigram[3], pad[1], offset:u64, len:u32 }, sorted by trigram
BITMAPS  the Roaring bitmaps, back to back; DIR offsets index this region
```

`crc` is a CRC-32 over META, DIR and BITMAPS. On load the file is
memory-mapped, the magic and both version fields are checked, the section
lengths are validated against the file size, the checksum is verified, and
the directory is checked to be sorted and in bounds before any bincode
decoding happens, so a truncated, corrupt or foreign file is rejected
early and cheaply. The server then keeps the file mapped and reads the
bitmaps in place: a query decodes only the lists for its trigrams, and the
lists stay in the page cache instead of on the heap (on Windows, which
cannot replace a mapped file, they are decoded at load).

## Metadata section

- the configuration fingerprint (with the extraction schema tag, see
  [Upgrading](../guides/upgrading.md)) and the indexed root paths;
- the file table: path (raw bytes, so non-UTF-8 names survive), mtime in
  nanoseconds, size, and which configured root the file belongs to;
- per-file symbols, each distinct name stored once and each symbol packed
  to 14 bytes (v13), and references (reference names interned in one table);
- resolved dependency edges and imports still waiting for their target;
- the symbol name dictionary symbol search filters by (see
  [Candidates](../how-it-works/candidates.md#verification-and-modes)): the
  distinct case-folded names, the files and symbol kinds behind each, and
  the files it does not describe yet. Saved so a load need not rebuild it
  (about 2 s at 302,000 files).
- folder stamps from the last complete walk: each folder's modification
  time, a fingerprint of its ignore files and how many eligible files it
  held (v12), and a fingerprint of the
  user's global excludes file, so a restart re-reads only the folders that
  changed (see [Persistence](../how-it-works/persistence.md#load)).

A load gives each file its position as its id, so every section is used
as saved; files changed or deleted since the save are tombstoned rather
than renumbered. Two kinds of save fill the table differently:

- a **checkpoint**, while the server runs, keeps every id: a file removed
  since the load is an empty slot (empty path), and the metadata lists the
  slots (`dead_slots`). Nothing is renumbered, a posting list no change
  touched is copied as its bytes, and the bitmaps may still name the empty
  slots, which a load masks out.
- a **full save**, at shutdown, compacts: removed files are absent and
  every other section is remapped to the new positions.

## Guarantees

- **Atomic.** A save writes a unique temporary file next to the target,
  fsyncs it, and renames it into place; saves are serialized within the
  process. A crash leaves the previous index intact, and the next start
  removes the temporary file a killed save left.
- **Detects drift.** Each file's mtime and size at read time are stored;
  on load a file whose stat differs is re-read.
- **Pinned.** `tests/fixtures/index-v13.fcsidx` is compared byte for byte
  in the test suite; a deliberate format change bumps the magic and
  regenerates the fixture (`FCS_WRITE_GOLDEN=1 cargo test golden`).
- **Older formats** are rejected with a message asking for a rebuild;
  they are never partially decoded.

The implementation is `src/index/persistence.rs`.
