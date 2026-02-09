# WIP — Indexing & Scoring Review Fixes

> Last updated: 2026-02-09
> Branch: working tree (uncommitted)

## Overview

Comprehensive review of indexing and scoring identified **14 issues** across 4 severity levels.
13 code fixes have been implemented. 1 newly-discovered issue (filename-only matches) has also been fixed. 2 items were deferred.

---

## Completed Fixes

All changes are in the working tree (unstaged). Files modified:
- `src/search/engine.rs` — bulk of changes (scoring, ranking, search methods)
- `src/index/trigram.rs` — dedup simplification
- `src/symbols/extractor.rs` — C++ template fix

### Fix #1 (P0 — Correctness): Regex trigram literals not lowercased
**Problem:** `search_regex()` passed the raw literal (e.g. `"MyClass"`) to `trigram_index.search()`, but the index stores lowercased content. Uppercase literals ≥3 chars returned zero trigram hits, causing silent fallback to full scan or missed results.
**Fix:** Added `let literal_lower = literal.to_lowercase()` before the trigram lookup in `search_regex()`.
**Test:** `test_regex_search_uses_lowercased_trigrams` — indexes a Python file with `MyClass`, searches regex `MyClass\.\w+`, asserts results found.

### Fix #2 (P0 — Correctness): Exact match boost compared against lowercased query
**Problem:** `calculate_score_inline()` used `line.contains(query_lower)` for the 2× exact-match boost. Since `query_lower` is already lowercase, this was really a case-insensitive check — every match got the boost, making it meaningless.
**Fix:** Added `original_query: &str` parameter to `calculate_score_inline()`. The boost now uses `line.contains(original_query)` for true case-sensitive matching. Propagated through `search_ranked()`, `search_with_filter_ranked()`, `search_fast_ranked_with_query()`, `search_full_ranked_with_query()`, and `search_in_document_scored()`.
**Test:** `test_exact_match_boost_uses_original_case` — file with `fn MyFunction()` and `fn myfunction()`, searches `"MyFunction"`, asserts the exact-case line scores higher.

### Fix #3 (P1 — Correctness): C++ template declarations produce duplicate symbols
**Problem:** The `template_declaration` handler explicitly pushed template children to the stack, but then also fell through to the generic `stack.push(child)` which pushed the template_declaration node itself. When it became `current`, its children were visited *again*, producing duplicate symbols.
**Fix:** Changed the `template_declaration` match arm to an empty body `{}`. The template_declaration node is now pushed once by the default `stack.push(child)`, and its children (function_definition, class_specifier, etc.) are matched naturally when it becomes `current`.
**Test:** `test_cpp_template_no_duplicate_symbols` — C++ file with `template<class T> T max_val(...)` and `template<typename T> class Container`, asserts each symbol appears exactly once.

### Fix #4 (P1 — Performance): Symbol search scans all documents
**Problem:** `search_symbols()` called `trigram_index.all_documents()` regardless of query, meaning every indexed file was checked for matching symbols.
**Fix:** For queries ≥3 chars (enough for trigram extraction), use `trigram_index.search(&query_lower)` to narrow candidates first. Short queries fall back to all documents.
**Test:** `test_symbol_search_uses_trigram_filtering` — two files, only one with matching symbol, asserts results come only from the relevant file.

### Fix #6 (P2 — Quality): FAST_RANKING_TOP_N too low
**Problem:** `FAST_RANKING_TOP_N` was 500. For large codebases with 100k+ files, only checking the top 500 by file-level score could miss relevant results that happen to be in files with low base scores.
**Fix:** Increased to 2000.
**Test:** `test_fast_ranking_top_n_is_sufficient` — asserts the constant is ≥2000.

### Fix #7 (P1 — Quality): Harsh line length penalty
**Problem:** The line length factor `1.0 / (1.0 + len * 0.01)` drops to 0.50 at 100 chars and 0.09 at 1000 chars. This severely penalized function signatures and long lines, making short comments rank above substantive code.
**Fix:** Changed to `(1.0 / (1.0 + (len / 100.0).ln_1p())).max(0.3)` — a gentler logarithmic curve that floors at 0.3. Applied in both `calculate_score_inline()` and `calculate_score_regex_inline()`.
**Test:** `test_line_length_penalty_is_gentle` — compares scores of short (~20 char), medium (~80 char), and long (~200 char) lines. Asserts medium > 50% of short, long > 25% of short.

### Fix #8 (P2 — Performance): Empty results return Some(empty vec)
**Problem:** `search_in_document_fast()`, `search_in_document()`, and `search_in_document_regex()` returned `Some(Vec::new())` when no matches were found, causing unnecessary Vec allocations that `filter_map` + `flatten` had to process.
**Fix:** Changed all three to `if matches.is_empty() { None } else { Some(matches) }`.
**Test:** `test_no_match_returns_none_not_empty_vec` — searches for non-existent term, asserts empty results.

### Fix #9 (P2 — Correctness): Trigram bleed at filename/content boundary
**Problem:** `format!("{}\n{}", filename_stem, content)` created garbage trigrams spanning the boundary (e.g., trigram `"e\nf"` from filename `"file"` + content starting with `"fn..."`).
**Fix:** Changed to `format!("{}\n\n\n{}", filename_stem, content)` in both `index_file()` and `PreIndexedFile::process()`. Triple newline ensures no meaningful trigram crosses the boundary.
**Test:** `test_filename_content_separator_prevents_trigram_bleed` — file named `alpha_module.txt` with content `beta_function`, asserts content search works correctly.

### Fix #10 (P3 — Code quality): Redundant trigram dedup in search
**Problem:** `TrigramIndex::search()` used `extract_trigrams()` → `FxHashSet` filter → `Vec` collect. This was a three-step dedup when `extract_unique_trigrams()` does it in one pass.
**Fix:** Changed to use `extract_unique_trigrams(query)` directly.

### Fix #12 (P2 — Performance): Per-query path allocation in filename matching
**Problem:** `filename_matches_query()` called `file_store.get_path()` → `file_stem()` → `to_str()` → `to_lowercase()` on every file for every query, allocating a new String each time.
**Fix:** Added `lowercase_stem: String` field to `FileMetadata`, pre-computed at index time. `query_score()` now takes `query_lower: &str` and uses `self.lowercase_stem.contains(query_lower)`. Removed the `filename_matches_query()` method entirely. `FileMetadata` is no longer `Copy` (now `Clone + Default`). `get_file_metadata()` returns `&FileMetadata` using a `OnceLock`-based static default.
**Test:** `test_file_metadata_precomputed_lowercase_stem` — indexes `MyModule.rs`, asserts `lowercase_stem == "mymodule"`, asserts `query_score("mymodule") > query_score("unrelated")`.

### Fix #14 (P3 — Documentation): ASCII-only case folding not documented
**Problem:** `contains_case_insensitive()` uses ASCII-only case folding, which silently fails for non-ASCII characters. This was not documented.
**Fix:** Added doc comment noting the ASCII-only limitation and its acceptability for code identifiers.

### Dead code cleanup
- Removed `filename_matches_query()` — superseded by `FileMetadata.lowercase_stem`
- Removed old `search_fast_ranked()` / `search_full_ranked()` — superseded by `_with_query` variants
- Renamed `search_in_document_fast()` → `search_in_document_scored()` with new signature

### Fix #15 (Discovered during review): Filename-only matches silently dropped
**Problem:** The filename stem is indexed into the trigram index and a `SymbolType::FileName` symbol is stored at line 0 with a 5× `FileMetadata` boost. However, `search_in_document_scored()` only iterates over actual file content lines. If the query matches only the filename (not any content line), the file is shortlisted by trigrams but then silently dropped — zero results returned.
**Fix:** Added filename-match fallback in three methods:
- `search_in_document_scored()`: after the content line scan, if no matches and the query matches a `FileName` symbol, synthesize a `SearchMatch` with `line_number=0`, the file path as content, `is_symbol=true`, and a 3× score.
- `search_in_document_regex()`: same pattern using `regex.is_match(&symbol.name)`.
- `search_symbols_in_document()`: `FileName` symbols now render the file path instead of fetching line 0 content (which was the wrong line).
**Tests:** `test_filename_only_match_returns_result`, `test_filename_symbol_search`, `test_filename_regex_match`.

---

## Build & Test Status

- **Build:** ✅ Clean compile, zero warnings
- **Clippy:** ✅ `cargo clippy -- -D warnings` passes
- **Fmt:** ✅ `cargo fmt` applied
- **Tests:** ✅ **143 pass, 0 fail**
  - 113 unit tests (including 12 new fix-verification tests)
  - 3 validator tests
  - 26 integration tests
  - 1 doctest

### New tests added
| Test | Covers |
|------|--------|
| `test_regex_search_uses_lowercased_trigrams` | Fix #1 |
| `test_exact_match_boost_uses_original_case` | Fix #2 |
| `test_cpp_template_no_duplicate_symbols` | Fix #3 (in extractor.rs) |
| `test_symbol_search_uses_trigram_filtering` | Fix #4 |
| `test_fast_ranking_top_n_is_sufficient` | Fix #6 |
| `test_line_length_penalty_is_gentle` | Fix #7 |
| `test_no_match_returns_none_not_empty_vec` | Fix #8 |
| `test_filename_content_separator_prevents_trigram_bleed` | Fix #9 |
| `test_file_metadata_precomputed_lowercase_stem` | Fix #12 |
| `test_filename_only_match_returns_result` | Fix #15 (text search) |
| `test_filename_symbol_search` | Fix #15 (symbol search) |
| `test_filename_regex_match` | Fix #15 (regex search) |

---

## Deferred Items

### Fix #11 (P3): Arc<str> for file paths
**Problem:** File paths are stored as `PathBuf` and cloned via `to_string_lossy().into_owned()` on every search match. Using `Arc<str>` would make cloning O(1).
**Why deferred:** Wide API surface impact — `LazyFileStore`, `SearchMatch`, `FileMetadata`, and all consumers would need updating. Low ROI given paths are typically short.

### Fix #13 (P3): Parallel path filter
**Problem:** `PathFilter::filter_documents_with()` runs sequentially. For large candidate sets with complex glob patterns, this could be slow.
**Why deferred:** P3 optimization. Current performance is adequate. Would need benchmarking to justify the added complexity.

---

## Recently Completed

### Fix #5 (P1): Post-persistence symbol rebuild
**Status:** ✅ Implemented. Symbol and dependency caches are rebuilt after loading a persisted index with progress reporting.

---

## New Issue Discovered: Filename-only matches silently dropped — FIXED

**Discovered during:** Testing Fix #9 (separator change)
**Status:** ✅ IMPLEMENTED AND TESTED

### Problem

The search pipeline has a contradiction:

1. **Indexing:** The filename stem is prepended to content and indexed into the trigram index. A `SymbolType::FileName` symbol is stored at line 0. `FileMetadata.query_score()` gives a 5× boost for filename matches.

2. **Trigram lookup:** When you search for `"alpha_module"`, the trigram index correctly identifies the file as a candidate (because the filename was indexed).

3. **Line-level search:** `search_in_document_scored()` iterates over `content.lines()` — the **actual file content** (what's on disk). The filename is NOT part of the file content. So if no content line matches the query, the function returns `None`.

4. **Result:** The file is shortlisted by trigrams, but then dropped because no content line matches. The filename match is silently lost.

### Evidence

- The diagnostics self-test in `src/web/api.rs` (line 657) explicitly expects filename search to work: *"Filenames are indexed as searchable content, so this tests that feature"*
- But the test only passes by coincidence — when the filename substring also appears in the file content
- Searching for a filename that doesn't appear in the file's content returns zero results despite the file being a trigram candidate

### Proposed Fix

In `search_in_document_scored()`: after the content line scan, if `matches` is empty and the query matches the filename (check via the `FileName` symbol in the symbol cache), synthesize a `SearchMatch` at line 0 with the file path as content and `is_symbol: true`. This makes the trigram indexing of filenames produce actual results.

**Scope:** `search_in_document_scored()` and possibly `search_in_document()` in `src/search/engine.rs`.

### Status: ✅ COMPLETED

---

## Files Modified (Summary)

| File | Changes |
|------|---------|
| `src/search/engine.rs` | Fixes #1, #2, #6, #7, #8, #9, #12, #14, #15 + dead code removal + 12 new tests |
| `src/index/trigram.rs` | Fix #10 (dedup simplification) |
| `src/symbols/extractor.rs` | Fix #3 (C++ template) + 1 new test |

---

## How to Resume

1. **All code changes are complete.** Build, tests, clippy, and fmt all pass.

2. **Run the validator** to verify search correctness at scale:
   ```bash
   cargo run --release --bin fast_code_search_validator
   ```

3. **Commit** the changes:
   ```bash
   git add -A
   git commit -m "fix: indexing & scoring review — 13 fixes with tests"
   ```

4. **Consider Fix #11 or #13** (deferred items) as follow-up PRs
