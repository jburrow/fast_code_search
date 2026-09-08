# Index file format

The persisted index (`index_path`) is a single file, format **v7**, magic
`FCSIDX05`. Everything is little-endian.

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
early and cheaply. Bitmaps are deserialized straight out of the mapping in
parallel; nothing is copied in between.

## Metadata section

- the configuration fingerprint (with the extraction schema tag, see
  [Upgrading](../guides/upgrading.md)) and the indexed root paths;
- the file table: path (raw bytes, so non-UTF-8 names survive), mtime in
  nanoseconds, size, and which configured root the file belongs to;
- per-file symbols and references (reference names interned in one table);
- resolved dependency edges and imports still waiting for their target.

File positions in the table are compacted (removed files are absent), and
every other section is remapped to those positions on save and back to
live ids on load, so ids stay stable across restarts.

## Guarantees

- **Atomic.** A save writes a unique temporary file next to the target,
  fsyncs it, and renames it into place; saves are serialized within the
  process. A crash leaves the previous index intact.
- **Detects drift.** Each file's mtime and size at read time are stored;
  on load a file whose stat differs is re-read.
- **Pinned.** `tests/fixtures/index-v7.fcsidx` is compared byte for byte
  in the test suite; a deliberate format change bumps the magic and
  regenerates the fixture (`FCS_WRITE_GOLDEN=1 cargo test golden`).
- **Older formats** are rejected with a message asking for a rebuild;
  they are never partially decoded.

The implementation is `src/index/persistence.rs`.
