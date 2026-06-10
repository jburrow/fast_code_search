# Engine & UI Improvement Plan — 2026-06-09

Status: NOT STARTED. Check off tasks as completed; if you deviate from a task, note why inline.

This plan was produced by a code review of the keyword search engine (crashes during
indexing + correctness bugs) and the web UI. Every task is self-contained: files, what
to do, and acceptance criteria. No conversation context is required to implement it.

Out of scope: the VS Code extension (`vscode-extension/`) — explicitly excluded by the
project owner. Do not modify anything under that directory.

Conventions for the implementing agent:
- One task (or one tightly-coupled group) per commit. Conventional-commit messages.
- Run `cargo build` and `cargo test` after each Rust task.
- Phases are ordered by severity and dependency. Within a phase, tasks are independent
  unless noted. Do not start Phase 2 before Phase 1 is green.

---

## Phase 1 — Engine crash fixes (P0)

### 1.1 Eliminate mmap/UTF-8 undefined behavior and truncation crashes
- [x] Files: `src/index/file_store.rs` (~45–52), `src/index/lazy_file_store.rs` (~171–183, ~209–233)
  DONE: removed `from_utf8_unchecked` + the cached `utf8_valid` flag in both stores
  (now re-validate with safe `from_utf8`/`String::from_utf8` each read). Rewrote
  `engine.index_file` to read an owned buffer via `PartialIndexedFile::process` +
  `PreIndexedFile::from_partial` + `index_batch` (no live mmap across extraction; also
  gets safety-check-before-register and the tree-sitter panic guard for free).
- Content is served via `unsafe from_utf8_unchecked`, guarded by a `OnceLock`-cached
  validity flag. The cache outlives the bytes it validated: the fallback path re-reads
  evicted bytes from disk, and the mmap path reads a live mapping another process can
  rewrite. If a mapped file is truncated on disk, touching pages past EOF is an
  uncatchable SIGBUS / EXCEPTION_IN_PAGE_ERROR — the most likely cause of the reported
  indexing crashes.
- Do: remove the `unchecked` conversions and the permanent validity caches; re-validate
  with `std::str::from_utf8` on each fresh read (it is SIMD-accelerated and cheap).
  During indexing (trigram/symbol extraction), read file contents into an owned buffer
  instead of holding a live mmap across the whole extraction — this is the only complete
  fix for truncation SIGBUS. Keep mmap for search-time reads if desired, but never
  `unsafe`-bless bytes that weren't just validated.
- Accept: no `from_utf8_unchecked` remains in `src/index/`; indexing a file that is
  concurrently truncated/rewritten (add a test that rewrites a file between add and read)
  returns an error or fresh content, never UB.

### 1.2 Panic guards on all indexing paths + poison recovery
- [x] Files: `src/search/background_indexer.rs` (~766–783), `src/search/engine.rs` (`index_file`, ~759–771)
  DONE: wrapped Phase-1 `process` in `catch_unwind`; `index_file` now routes through the
  guarded `from_partial`. Batch-merge and import-finalization write locks recover from
  poison via `into_inner()` instead of dropping the batch.
- Phase 2 of batch indexing already wraps tree-sitter in `catch_unwind` (engine.rs
  ~485–496) because it panics on pathological files. Phase 1 of batch indexing (file IO,
  transcoding, trigram extraction under rayon) and the watcher path `index_file` have no
  guard. One panic kills the indexer thread or poisons the engine `RwLock`, after which
  every batch is dropped (`return 0`) and the index silently ends up partial.
- Do: wrap the Phase-1 per-file closure and `index_file`'s extractor calls in
  `std::panic::catch_unwind(AssertUnwindSafe(...))`, logging and skipping the bad file.
  For lock poisoning: recover via `PoisonError::into_inner()` (or migrate the engine lock
  to `parking_lot::RwLock`, which doesn't poison) so one panic can't disable all
  subsequent indexing.
- Accept: a file whose processing panics is logged + skipped; remaining files index
  normally; no code path treats `lock().err()` as "skip the batch silently".

### 1.3 Harden persisted-index loading
- [x] File: `src/index/persistence.rs` (~118–155)
  DONE: added an 8-byte magic header (`FCSIDX01`) validated before any decoding; body
  now decoded with `bincode::options().with_fixint_encoding().with_limit(file_len)` so a
  bogus length prefix errors instead of aborting on a huge allocation. Tests added for
  truncated / random-bytes / bogus-length files.
- The version field is checked only after full bincode deserialization, with no magic
  header and no allocation limit. A corrupt/stale `index.bin` can make bincode interpret
  a garbage length prefix as a multi-GB allocation → process abort at startup.
- Do: write a small fixed header (magic bytes + format version) before the bincode body;
  on load, validate the header first, then deserialize with
  `bincode::options().with_limit(file_len)`. Treat any failure as "rebuild index" (the
  existing `try_load` graceful path).
- Accept: loading a truncated, corrupted, or old-version index file never aborts; it logs
  and falls back to a rebuild. Add tests with a truncated file and a random-bytes file.

---

## Phase 2 — Index correctness (P0/P1)

### 2.1 Remap trigram doc IDs on index reload (silent result corruption)
- [x] Files: `src/search/engine.rs` (all three load paths), `src/index/persistence.rs`
  DONE: added `build_orig_to_new_map` (from ACTUAL registration ids) +
  `remap_trigram_bitmaps` (with O(files) identity fast-path); all three load paths now
  register files first, build the map, and remap the restored bitmaps.
  `restore_symbols_and_deps` takes the same map. Round-trip test added
  (`test_reload_remaps_trigram_ids_after_stale_file`).
- Persisted trigram bitmaps key on save-time file IDs (positions in `persisted.files`).
  On reload, only still-valid files are re-added, receiving compacted IDs. Symbols/deps
  are remapped via `orig_to_new_id`; the trigram bitmaps are NOT — so if even one file is
  stale, every later file's matches are attributed to the wrong file.
- Do: build the `orig_to_new_id` map from the **actual IDs returned by `add_file`** (not
  assumed positions — `add_file` can fail or dedupe), then rebuild each restored
  `RoaringBitmap` keeping only valid original IDs mapped to their new IDs. Apply in all
  three load paths (or better: extract the shared reconciliation into one function).
- Accept: a round-trip test — index 5 files, save, delete file #2 on disk, reload —
  searches return correct paths for all remaining files.

### 2.2 Real incremental updates: refresh / remove / rename
- [x] Files: trigram.rs, lazy_file_store.rs, dependencies/mod.rs, engine.rs, watcher.rs, main.rs
  DONE: `TrigramIndex::remove_document`; `LazyFileStore::remove_file_by_id` (tombstone) +
  `refresh_file_by_id` (fresh mmap/caches); `DependencyIndex::remove_file`; engine
  `update_file` now strips old data and re-extracts under the same id, `remove_file`
  added; watcher handles `Modify(Name)` renames (two-path and single-path); main.rs wires
  `Deleted` to `remove_file` and `Renamed` to remove-old + index-new. Integration test
  `test_incremental_update_remove_rename`.
- Today: `update_file` → `add_file` early-returns for known paths (old mmap + frozen
  caches kept; modified content never indexed; on shrink, see crash 1.1). `TrigramIndex`
  has no removal API. Deletes are a logged no-op. The watcher drops renames entirely (on
  Windows they arrive as `EventKind::Modify(ModifyKind::Name)` with `[old, new]`; the
  code takes `paths.first()` — the old path — fails `is_file()`, returns None).
- Do:
  - `TrigramIndex::remove_document(doc_id)`: strip the ID from every posting bitmap (or
    maintain a deleted-docs bitmap intersected out at query time); drop empty entries.
  - `LazyFileStore::refresh_file(path)`: replace the entry for an existing ID with a
    fresh mapping and fresh `OnceLock` caches. `remove_file(path)`: tombstone the ID.
  - `update_file`: remove old trigrams + symbols + dependency edges for the ID, refresh
    the store entry, re-extract.
  - Watcher: match `Modify(ModifyKind::Name)` explicitly — two paths ⇒ emit
    `Deleted(old)` + `Modified(new)`; one path ⇒ `Deleted` if it no longer exists, else
    `Modified`. Wire `FileChange::Deleted` in `main.rs` to engine removal.
- Accept: integration test — index a dir; modify a file (content replaced), delete one,
  rename one; after watcher processing, searches reflect new content only, deleted and
  old-named paths return nothing.

### 2.3 Atomic index save
- [x] File: `src/index/persistence.rs` (~118–127)
  DONE (with 1.3): `save` writes to `<path>.bin.tmp` under an exclusive lock, explicit
  `flush()` + `sync_all()`, then atomic `rename` over the target (with Windows
  remove-then-rename fallback). Test asserts no temp file is left and re-save works.
- `File::create` truncates the existing index BEFORE acquiring the exclusive lock;
  concurrent shared-lock readers see a truncated file. The final `BufWriter` flush
  happens in drop, errors swallowed (disk-full looks like success).
- Do: serialize to `<path>.tmp`, explicit `flush()?` + `sync_all()?`, then rename over
  the target. Keep the lock if cross-process coordination is still desired.
- Accept: killing the process mid-save leaves the previous index intact and loadable.

### 2.4 Indexing pipeline robustness (smaller, batched together)
- [x] background_indexer drain race: timeout-with-discovery-done now drains stragglers
  into the batch instead of discarding an `Ok(path)`.
- [x] `index_file` safety-check-before-register: done via the 1.1 rewrite (process()
  rejects unsafe/binary/oversized before any id is assigned).
- [x] file_discovery `follow_links(false)` (avoids duplicate symlinked trees / out-of-root).
- [x] `max_file_size` plumbed through `PartialIndexedFile::process` (param; 0 = default
  cap), engine field, and process_batch; stale-file loop now applies the size cap.
- [x] `elapsed_secs` uses `saturating_sub`.

---

## Phase 3 — Search correctness (P1)

### 3.1 Regex acceleration drops matches on alternation/optional literals
- [x] Files: `src/search/regex_search.rs`, consumer `src/search/engine.rs`
  DONE: analysis now produces sound `constraints` (intersection of per-constraint
  unions); alternations contribute a union of branch literals (only when every branch
  has one), optional/`min==0` subexprs contribute nothing. Engine `regex_candidate_docs`
  intersects the union sets; falls back to full scan when there are no constraints. Unit
  tests + integration test `test_regex_alternation_returns_all_branches`.
- Literals from alternation branches and `min == 0` repetitions are treated as REQUIRED
  trigram pre-filters: `hello|world` filters out every file containing only `world`.
- Do: track requiredness during HIR walk (a literal under an alternation is required only
  if common to all branches; under `min == 0` repetition, never). For pure alternations,
  union the trigram candidate sets of all branches. If no required literal exists, fall
  back to full scan.
- Accept: tests for `hello|world`, `(abc)?def`, `(x|y)z` return complete results.

### 3.2 Watcher/stale-filter exclusions use globs, not substrings
- [x] Files: `src/search/watcher.rs`, `src/search/background_indexer.rs`, `src/search/path_filter.rs`
  DONE: `PathFilter::expand_pattern` adds `**/name/**` for bare directory names; added
  `is_excluded` + `exclude_only`. Watcher builds a `PathFilter` once and uses it
  (`should_exclude` now glob-based); stale-file loop uses the same. Tests cover
  `.github`/`.gitignore` not matched by `.git`, Windows backslash paths, bare names.
- Patterns are trimmed to bare strings and matched with `contains()`: `**/.git/**`
  becomes `.git`, which excludes `.github/` and `.gitignore` from watching; backslashed
  Windows paths never match multi-component patterns. File discovery uses proper globs
  via `PathFilter`, so subsystems disagree.
- Do: reuse `PathFilter` (with backslash→slash normalization) in `process_event`,
  `should_exclude`, and the stale-file loop. In `PathFilter::normalize_pattern`, when a
  bare directory name has no glob metacharacters, also add `**/name/**` (gitignore
  directory semantics) so bare patterns work in file discovery too.
- Accept: edits to `.github/workflows/*.yml` and `.gitignore` are picked up by the
  watcher; `src/targeted.rs` is not excluded by a `target` pattern; behavior identical
  with `/` and `\` separators.

### 3.3 Case-folding consistency + empty-query guard
- [x] File: `src/search/engine.rs`
  DONE: empty/whitespace queries early-return in `search_ranked`,
  `search_with_filter_ranked`, `search_symbols`. Chose to make VERIFICATION
  Unicode-aware (the trigram layer is already Unicode-consistent on both sides): added
  `unicode_ci_find`; `contains_case_insensitive` / `find_match_position_case_insensitive`
  use it for non-ASCII needles, so `über` now matches `ÜBER` end-to-end. ASCII fast path
  unchanged. Unit tests added.
- Index-time trigrams use Unicode `to_lowercase()`; search-time verification folds ASCII
  only — `ÜBER` is found as a candidate for `über`, then silently dropped. Separately, an
  empty/whitespace query falls into the short-query branch, gets ALL documents, and the
  empty needle "matches" every line (full-corpus garbage scan).
- Do: early-return empty results for empty/whitespace queries in `search_ranked`,
  `search_with_filter_ranked`, `search_symbols`. For folding: either build trigrams from
  an ASCII-folded copy (keeping the two layers consistent) or add a Unicode-aware
  verification slow path when the query contains non-ASCII. Pick one and document it.
- Accept: empty query returns instantly with zero results; case-insensitive non-ASCII
  search either matches consistently or is documented ASCII-only at BOTH layers.

---

## Phase 4 — Web UI bugs (P0/P1 within UI)

### 4.1 XSS: quote-safe escaping + remove inline JS handlers
- [x] Files: `static/common.js`, `static/keyword.js`
  DONE: `escapeHtml` rewritten (replace-based, escapes `& < > " '`). Removed all inline
  `onmouseenter/onclick` handlers from the deps badge, deps popover links/"more", and the
  dep-modal close button; wired via `addEventListener` + `dataset`. (diagnostics pages'
  static `onclick="loadDiagnostics(true)"` left — no interpolated data, not a vector.)
- `escapeHtml` (textContent/innerHTML trick) does not escape `"` or `'`. File paths and
  queries are interpolated into inline handlers (`onmouseenter="showDepsTooltip(this,'…')"`)
  and attributes (`data-query="…"`, `title="…"`). A path or query containing a quote
  breaks out → arbitrary JS. This is a code-search tool indexing arbitrary repos: paths
  are attacker-influenced.
- Do: rewrite `escapeHtml` as replace-based covering `& < > " '`. Remove ALL inline
  `onclick`/`onmouseenter` string handlers; use `addEventListener` + `dataset` (the
  `.view-file-btn` wiring at keyword.js ~923–929 is the model — extend it to the deps
  badge and popover links).
- Accept: index a repo containing a file named `a'b"c.txt` (and a query containing
  quotes); no script execution, paths render correctly, all buttons work.

### 4.2 Fix group-by-file path normalization (regression from commit 1f4f1d6)
- [x] File: `static/keyword.js` (`groupResultsByFile`)
  DONE: normalized string used only as the Map key; group `filePath` now stores the first
  hit's original `file_path` for display and fetches.
- The lowercased/slash-normalized group KEY is also stored as the displayed/fetched
  `filePath`: headers show `searchengine.rs` instead of `SearchEngine.rs`, and group-level
  view/deps buttons send the lowercased path to `/api/file`/`/api/dependents` → 404 on
  case-sensitive systems. Per-hit buttons use the original path, so two buttons in the
  same card disagree.
- Do: use the normalized string as the Map key only; store the first hit's original
  `file_path` for display and fetches.
- Accept: mixed-case paths display verbatim; group-header view button works.

### 4.3 Search race + double-fetch
- [x] File: `static/keyword.js`, `static/common.js`
  DONE: module-level `_searchAbort` AbortController aborted at the top of `performSearch`;
  `fetch` passes the signal; `AbortError` is ignored. `debounce` gained a `.cancel()`,
  called on Enter so the query isn't fetched twice.
- No AbortController/sequence token: a slow earlier query can overwrite newer results.
  Enter doesn't cancel the pending debounce → same query fetched twice.
- Do: module-level `AbortController`, abort at top of `performSearch`, pass signal to
  fetch, ignore `AbortError`; clear the debounce timer inside `performSearch`.
- Accept: rapid typing then Enter issues no out-of-order renders and no duplicate fetch
  for the final query.

### 4.4 Web UI state/lifecycle fixes (batch)
- [x] Offline banner: `checkBackendHealth` re-run from `progressWS.onConnected` and
  `onServerOffline` so the banner tracks server recovery/loss.
- [x] URL-state shadowing: local `loadStateFromUrl` now opens the VISIBLE `#filter-panel`
  when filter params are present (was opening the hidden `.advanced-options`); dead
  `URL_FIELDS` removed and the call site simplified to `loadStateFromUrl()`.
- [x] Dark-mode hljs: index.html now always loads the light highlight theme.
- [x] Stale `tailwind.css`: rebuilt via `npm install && npm run build:css`; verified the
  previously-missing classes (`top-full max-h-64 overflow-y-auto shadow-md ml-auto`) are
  now present. (CI guard not added — noted as follow-up.)
- [x] HTTPS: ProgressWebSocket uses `wss:` under https; semantic probe uses the page
  protocol and a `window.SEMANTIC_PORT` override (default 8081).
- [x] Deleted dead/corrupt `static/common.css` and `static/keyword.css` (linked by no
  page). Kept `input.css` (it is the Tailwind build SOURCE) and `semantic.css`.

---

## Phase 5 — Web UI usability (P2)

- [ ] 5.1 Filter discoverability: the only FILTER toggle lives inside `#results-header`
  (hidden until first search) — index.html ~503–514. Move/duplicate it next to the
  REGEX/SYMBOLS toggles so include/exclude/max/context can be set before searching.
- [ ] 5.2 Surface server error bodies: keyword.js:752 throws `response.statusText`,
  discarding useful 400 bodies ("Invalid regex pattern: …") and the 503 indexing message.
  Read `await response.text()` and render it (populateFileView ~530–534 already does).
- [ ] 5.3 Truncation honesty: server `total_results` == page length (api.rs:299). Add a
  real total or `has_more` flag server-side; UI shows "50+ shown — raise MAX RESULTS".
- [ ] 5.4 History pollution: `saveToHistory` fires on every debounced keystroke-search
  (keyword.js:758) storing prefixes ("per", "perfor", …). Save only on explicit submit
  (Enter / button / history-select) or after idle.
- [ ] 5.5 Progress panel: `completed` status keeps a 100% bar forever (keyword.js:320) —
  hide a few seconds after completion. Also stats flicker to 0 during indexing because
  `get_stats_from_engine` returns zeros when the write lock is held (api.rs ~732–747) —
  cache last-known-good stats server-side or skip the update client-side.
- [ ] 5.6 Keyboard navigation: results list has no keyboard support — add arrow/j-k
  navigation between hits + Enter to open the file modal; make history-dropdown
  highlight scroll into view; Escape restores the typed query; consider `/` to focus
  search.
- [ ] 5.7 Copy-path + open-in-editor: add a copy button per group header
  (navigator.clipboard) and an optional `vscode://file/<abs>:<line>` link (config-gated).

---

## Phase 6 — API DX + web perf (P2)

- [ ] 6.1 Consistent JSON errors: handlers return plain-text bodies on error, JSON on
  success (api.rs ~185–194 et al). Return `Json({ "error": … })` everywhere.
- [ ] 6.2 503 ergonomics: add `Retry-After: 1` to `WouldBlock` 503s; UI auto-retries once
  or shows the "index loading" state instead of "Service Unavailable".
- [ ] 6.3 ETag handling: mod.rs ~137–147 computes MD5 of every asset per request
  (including a multi-MB font) and never honors `If-None-Match`. Precompute hashes once;
  return 304 on match.
- [ ] 6.4 Subset the icon font: `static/fonts/material-symbols-outlined.woff2` is 3.8 MB
  for ~13 used glyphs (search, auto_awesome, analytics, description, help_outline,
  warning, code, function, tune, open_in_new, history, close, settings_suggest). Subset
  with `fonttools` (keep ligatures for those names) or switch to inline SVGs → ~5–10 KB,
  also shrinks the rust-embed binary. Eliminates the raw-ligature-text flash.
- [ ] 6.5 Hover tooltip fetches the whole file per mouseenter (keyword.js:927 →
  populateFileView) with no delay or cache; the purpose-built `/api/context` endpoint
  (api.rs ~655–721) is never used. Add ~200 ms hover-intent delay, use `/api/context`,
  cache by path.
- [ ] 6.6 Docs page: document `context` param, `/api/file`, `/api/context`,
  `/api/dependencies`, `/api/diagnostics`, and `total_results` semantics (docs.html
  ~300–426).
- [ ] 6.7 Misc cosmetics: drop `relative` from sticky header (index.html:415); hide or
  collapse `#nav-search` below ~640px (overlaps wordmark); replace emoji ranking labels
  (⚡📊🔄, keyword.js:766) and diagnostics emoji with material symbols for design
  consistency; use API `match_start`/`match_end` (already returned, unused) for exact
  highlight in regex mode; remove dead `searchTimeout` (keyword.js:38) and unused
  common.js helpers (StatusPoller etc.).

---

## Suggested implementation order

1. Phase 1 (crashes) — ship alone, immediately.
2. Phase 2.1 + 2.2 (index correctness) — biggest "weird behavior" fixes.
3. Phase 4.1–4.3 (web UI bugs incl. XSS and grouping regression).
4. Remaining phases in any order; 6.4 (font subset) is the best effort/value cosmetic.

## Verification checklist (after each phase)

- `cargo build && cargo test`
- Manual: start server on a real repo, edit/delete/rename files while indexing runs,
  search in browser at http://localhost:8080, check DevTools console for errors.
