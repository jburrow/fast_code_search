# Feature Plan: Non-UTF-8 Encoding Support

## Problem Statement

Files stored in legacy encodings (Latin-1/ISO-8859-1, Windows-1252, Shift-JIS, EUC-KR, UTF-16, etc.) are silently skipped during indexing because two gates reject them:

1. `PartialIndexedFile::process` → `std::fs::read_to_string()` fails on non-UTF-8 → returns `None`
2. `LazyFileStore` → `as_str()` → `std::str::from_utf8()` fails → returns `Err`

These files are legitimate text, just in a different encoding. This is common in older C/C++ codebases, Japanese/Korean/Chinese projects, and Windows-legacy repos.

## Design Goals

- **Index non-UTF-8 text files** by detecting encoding and transcoding to UTF-8
- **Preserve zero-copy for UTF-8 files** (no performance regression for the 95%+ case)
- **Make encoding support configurable** (opt-in or automatic, with config knob)
- **Log transcoded files** for visibility

## Architecture Decision: Where to Transcode

**Option A — Transcode at file-read time (in FileStore/LazyFileStore):** Replace the raw `mmap` with a `Cow<str>` that is either a zero-copy `&str` (valid UTF-8) or an owned transcoded `String`. This is the cleanest layering — everything above gets `&str` regardless.

**Option B — Transcode only in indexing paths:** Keep the file stores as-is, transcode in `PartialIndexedFile::process` and `index_file()`. Simpler but means search-time reads of non-UTF-8 files still fail (no match content returned).

**Recommended: Option A** — consistent behavior at all layers.

## Implementation Plan

### Phase 1: Add encoding detection dependency

| Step | Detail |
|------|--------|
| 1.1 | Add `encoding_rs = "0.8"` to `Cargo.toml` — Mozilla's battle-tested encoding library (used by Firefox, same as `chardetng`) |
| 1.2 | Add `chardetng = "0.1"` to `Cargo.toml` — charset detection (same algorithm as Firefox) |

These are small, pure-Rust, no-unsafe crates with no transitive dependencies beyond `encoding_rs`.

### Phase 2: Encoding detection utility in `src/utils.rs`

Add a function:

```rust
/// Detect encoding of raw bytes and transcode to UTF-8 if needed.
/// Returns Ok(None) if already valid UTF-8 (zero-copy path).
/// Returns Ok(Some(String)) with transcoded content for non-UTF-8 text.
/// Returns Err if the content appears to be binary (not text in any encoding).
pub fn transcode_to_utf8(bytes: &[u8]) -> Result<Option<String>, &'static str>
```

Logic:
1. Try `std::str::from_utf8(bytes)` — if OK, return `Ok(None)` (zero-copy, fast path)
2. Check for UTF-8 BOM (`EF BB BF`) — strip it and re-validate
3. Check for UTF-16 BOM (`FF FE` or `FE FF`) — decode via `encoding_rs`
4. Use `chardetng::EncodingDetector` on first 8KB to guess encoding
5. If confidence is sufficient, transcode via `encoding_rs::Encoding::decode()`
6. Run `is_binary_content()` on the result as a sanity check
7. Return `Ok(Some(transcoded_string))`

### Phase 3: Add transcoded content storage to `LazyMappedFile`

```rust
pub struct LazyMappedFile {
    pub path: PathBuf,
    mmap: OnceLock<Result<Mmap, String>>,
    utf8_valid: OnceLock<bool>,
    /// Transcoded UTF-8 content for non-UTF-8 files (None if natively UTF-8)
    transcoded: OnceLock<Option<String>>,
    /// Detected encoding name for diagnostics
    detected_encoding: OnceLock<Option<&'static str>>,
}
```

Update `as_str()`:
```rust
pub fn as_str(&self) -> Result<&str> {
    let mmap = self.ensure_mapped()?;
    
    // Fast path: already valid UTF-8
    let is_valid = *self.utf8_valid
        .get_or_init(|| std::str::from_utf8(mmap).is_ok());
    
    if is_valid {
        return Ok(unsafe { std::str::from_utf8_unchecked(mmap) });
    }
    
    // Slow path: try transcoding
    let transcoded = self.transcoded.get_or_init(|| {
        match crate::utils::transcode_to_utf8(mmap) {
            Ok(Some(s)) => {
                tracing::info!(
                    path = %self.path.display(),
                    encoding = self.detected_encoding.get().flatten().unwrap_or("unknown"),
                    "Transcoded non-UTF-8 file"
                );
                Some(s)
            }
            _ => None,
        }
    });
    
    match transcoded {
        Some(s) => Ok(s.as_str()),
        None => anyhow::bail!("File is not valid text: {}", self.path.display()),
    }
}
```

Same pattern for `MappedFile` in `src/index/file_store.rs`.

### Phase 4: Update `PartialIndexedFile::process`

Replace `std::fs::read_to_string(path).ok()?` with:
```rust
let raw_bytes = std::fs::read(path).ok()?;
let content = match std::str::from_utf8(&raw_bytes) {
    Ok(s) => s.to_string(),  // UTF-8 fast path
    Err(_) => match crate::utils::transcode_to_utf8(&raw_bytes) {
        Ok(Some(s)) => {
            tracing::debug!(path = %path.display(), "Transcoded non-UTF-8 file for indexing");
            s
        }
        _ => return None,  // Binary or unrecognizable
    },
};
```

### Phase 5: Config option

Add to `IndexerConfig` in `src/config.rs`:

```rust
/// Enable encoding detection for non-UTF-8 text files (default: true)
/// When enabled, files in encodings like Latin-1, Shift-JIS, UTF-16 etc.
/// are automatically transcoded to UTF-8 for indexing.
/// Disable this if you only work with UTF-8 codebases for slightly faster indexing.
#[serde(default = "default_true")]
pub transcode_non_utf8: bool,
```

Pass this flag through to `PartialIndexedFile::process` and `LazyFileStore` so transcoding can be disabled.

### Phase 6: Diagnostics/visibility

- Add a counter to `IndexingProgress`: `files_transcoded: usize` — visible in the web UI progress
- Log at `info` level when a file is transcoded, including the detected encoding name
- Add to the `/api/diagnostics` endpoint: list of transcoded files with their encodings

### Phase 7: Tests

| Test | What it validates |
|------|-------------------|
| `test_transcode_utf8_passthrough` | Valid UTF-8 returns `Ok(None)` — zero-copy fast path |
| `test_transcode_latin1` | Latin-1 encoded "café" transcodes correctly |
| `test_transcode_utf16_le_bom` | UTF-16 LE with BOM decodes correctly |
| `test_transcode_utf16_be_bom` | UTF-16 BE with BOM decodes correctly |
| `test_transcode_shift_jis` | Shift-JIS Japanese text decodes correctly |
| `test_transcode_binary_rejected` | Binary content returns `Err` |
| `test_index_latin1_file` | Integration: Latin-1 file is indexed and searchable |
| `test_config_disable_transcoding` | `transcode_non_utf8 = false` skips encoding detection |

## Performance Impact

| Scenario | Impact |
|----------|--------|
| UTF-8 files (95%+ of codebase) | **Zero** — `from_utf8()` succeeds fast, no transcoding attempted |
| Non-UTF-8 files | ~1ms per file for detection + transcode (one-time at index) |
| Memory | Non-UTF-8 files hold both `Mmap` and `String` — typically negligible count |
| Search-time | Non-UTF-8 files return from `transcoded` cache — one extra pointer deref |

## Dependency Cost

| Crate | Size | Transitive deps |
|-------|------|-----------------|
| `encoding_rs` | ~360KB, pure Rust | 0 (optionally `cfg-if`) |
| `chardetng` | ~45KB, pure Rust | `encoding_rs` only |

## Files Changed (Estimated)

| File | Changes |
|------|---------|
| `Cargo.toml` | Add 2 dependencies |
| `src/utils.rs` | Add `transcode_to_utf8()` + tests |
| `src/index/file_store.rs` | Add `transcoded` field to `MappedFile`, update `as_str()` |
| `src/index/lazy_file_store.rs` | Add `transcoded` field to `LazyMappedFile`, update `as_str()` |
| `src/search/engine.rs` | Update `PartialIndexedFile::process` to use `std::fs::read` + transcode |
| `src/config.rs` | Add `transcode_non_utf8` option |
| `src/search/background_indexer.rs` | Increment `files_transcoded` counter |
| `tests/integration_tests.rs` | Add Latin-1/UTF-16 indexing integration tests |

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| Encoding mis-detection → garbage in index | `chardetng` uses same algorithm as Firefox; also `content_safety_check()` runs after transcoding |
| Memory increase from dual storage | Only affects non-UTF-8 files which are typically <1% of a codebase |
| Persistence format change | No change — persisted index stores trigrams/metadata, not raw content. Transcoding happens at load time. |
| `encoding_rs` safety | Mozilla-maintained, used in production Firefox, fuzzed extensively |
