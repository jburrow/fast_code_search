# Keyword Engine Roadmap — 2026-09-04

Status: IN PROGRESS on branch `keyword-roadmap`. Tasks are ticked `[x]` with a
DONE note as they land; each task is one commit. Produced from a full read of the keyword engine at v0.9.0 (commit
`241d01a`), verified by building and running the suite on Linux (Rust 1.98.1):
lib 166 passed, integration 35 passed, benches compile, clippy 3 warnings,
`cargo fmt --check` 13 hunks in 6 files.

Scope: the keyword engine only — `src/index`, `src/search`, `src/symbols`,
`src/dependencies`, `src/server`, `src/web`, `src/config.rs`, `src/main.rs`,
`src/utils.rs`, `src/diagnostics`, tests, benches, CI and repo hygiene.
Out of scope: everything under `src/semantic*`, `static/semantic.*`,
`docs/semantic`, and `vscode-extension/` (mentioned only where the engine's
API contract affects it).

Line numbers refer to the working tree at the commit above and will drift.
Every finding was verified by reading the code path; the two P0/P1 items marked
"reproduced" were additionally confirmed with a throwaway test.

---

## 1. Where the engine stands

The core design is right and the 0.9.0 hardening work landed properly:

- Trigram postings in Roaring bitmaps with cardinality-ordered intersection and
  early exit; regex acceleration is sound (intersection-of-unions, optional
  subexpressions excluded) and unit-tested for the tricky shapes.
- No `from_utf8_unchecked` anywhere; UTF-8 is re-validated on every access;
  indexing reads owned buffers so a concurrently truncated file cannot SIGBUS
  the indexer.
- Persistence is atomic (temp + fsync + rename), header-checked, allocation-
  limited, and doc ids are remapped on reload when files went stale — with
  regression tests for each.
- Two-phase batch indexing keeps only the merge under the write lock and wraps
  every tree-sitter call in `catch_unwind`.
- The serving layer never blocks the tokio runtime: every engine touch is under
  `spawn_blocking`, readers use `try_read` and return 503 + `Retry-After`
  instead of queueing, errors are a uniform JSON envelope, static assets have
  compile-time ETags.
- The integration suite is genuinely end-to-end (real gRPC + axum on ephemeral
  ports), hermetic, and parallel-safe; CI runs on three OSes with coverage and
  a benchmark trend chart.

The problems are concentrated in four places: (1) the write path after the
initial build — watcher updates, removals, and checkpoints — which is where
every remaining correctness bug lives; (2) ranking signals that are computed
once and then drift; (3) unbounded per-query work; (4) symbol/import
extraction that is much shallower than the README implies for JS/TS, Rust and
Python.

### Headline risks

| Sev | Finding | Where |
|-----|---------|-------|
| P0 | **Saving after any `remove_file` corrupts the persisted index** (reproduced). The save loop compacts over tombstoned ids but trigram bitmaps and the symbol vector keep raw ids; on reload every file after the tombstone is misattributed or lost. Live path: watcher delete → `remove_file` → `save_on_watcher_update`. | `engine.rs:2346-2385`, `lazy_file_store.rs:445-462`, `engine.rs:1062-1096` |
| P1 | **Every match on line 1 of every file gets the 3× definition boost** (verified). The synthetic `FileName` symbol sits at line 0 with `is_definition: true` and is not excluded from `symbol_def_lines`. Shebangs, license headers and `use` lines outrank real definitions. | `engine.rs:577-585`, `2150`, `1985` |
| P1 | Persisted mtime/size are captured at **save** time, not read time. A file edited during a long build is saved with the new mtime and old trigrams, and is never detected as stale. `watch` defaults to off, so nothing repairs it. | `engine.rs:2349` |
| P1 | `content_safety_check` (line > 100 KB or nesting > 500) drops the file from the **entire** index, not just from tree-sitter, and runs even when symbols are disabled. Large JSON fixtures and generated code are unsearchable. | `engine.rs:479`, `utils.rs:175` |
| P1 | `rank=full` (or any corpus with ≤ 5000 candidates) has no work bound; a common query collects every match from every candidate before truncation. `max_results` never limits work. | `engine.rs:1454-1472`, `1615-1636` |
| P1 | Unresolved imports are retried on **every batch** under the engine write lock; stdlib/package imports never resolve, so the pending list grows for the whole build and each retry costs up to 7 `canonicalize` syscalls. | `background_indexer.rs:875`, `engine.rs:948-956` |
| P1 | Directory rename/delete leaves the index permanently wrong: `remove_file(dir)` matches nothing, `update_file(dir)` is a silent no-op. | `watcher.rs:194-199`, `main.rs:616-648` |
| P1 | A panic on the watcher write path poisons the engine lock; every search returns 500 until restart while the indexer keeps going. The watcher runs on a default-stack thread with no `catch_unwind`. | `main.rs:582-597`, `api.rs:233-236` |
| P1 | No graceful shutdown: SIGINT/SIGTERM never saves the index or flushes telemetry. | `main.rs:317-324` |
| P1 | Search-time retrieval reads through a shared mmap; a concurrent truncate-and-rewrite SIGBUSes the whole server. (Indexing was fixed in 0.9.0; retrieval was not.) | `lazy_file_store.rs:77,167-176`, `engine.rs:1842` |
| P1 | gRPC `Index` RPC is unauthenticated, bound to `0.0.0.0`, and indexes any path the caller names; `/api/file` then serves it back. `is_path_in_scope` exists and has zero callers. CORS is `Any`. | `service.rs:319-403`, `config.rs:195-201,310`, `web/mod.rs:44-47` |
| P1 | Symbols: `.tsx` is parsed with the TypeScript grammar (JSX becomes ERROR nodes); JS/TS misses `method_definition` and `const f = () => …`; symbol line/column come from the declaration node so Java `@Override`, TS decorators and multi-line C signatures report the wrong line and the 3× boost lands on the annotation. | `extractor.rs:81,137-181,157` |
| P1 | Imports: Rust `use crate::…` never resolves and `mod foo;` binds to an arbitrary `foo.rs` anywhere in the repo; Python relative imports look for a hidden file `.foo`; the bare-name fallback links `lodash/merge` to any `merge.ts`. The "heavily imported" boost is therefore mostly noise. | `dependencies/mod.rs:87-132` |
| P1 | Repo: the working tree is a pure CRLF flip of 172 files (no `.gitattributes`); `.gitignore` has a UTF-16 fragment spliced into the `onnxruntime/` line so 23 files including a 12 MB DLL are tracked; `test_corpus/` holds three gitlinks with no `.gitmodules`; README claims Rust 1.70+ but the real MSRV is ≥ 1.82 and nothing pins it. | `.gitignore`, `Cargo.toml` |

---

## 2. Review findings by area

Severity: P0 data loss / wrong results in a shipped path; P1 wrong results,
crash, or security exposure under realistic use; P2 performance, drift, or
robustness gap; P3 hygiene.

### 2.1 Index layer (`src/index`)

- **P0** Tombstones invisible to persistence — see headline table. No test saves
  after a removal (`tests/integration_tests.rs:2101-2122` stops short).
- **P1** Retrieval through shared mmap can SIGBUS — see headline table.
- **P2** The mmap-limit guard is inert on the persisted-load path: `mapped_count`
  is incremented only in `add_file`, never in `ensure_mapped`, and never
  decremented (`lazy_file_store.rs:72-90,325-338,438`). One mmap per file
  collides with `vm.max_map_count` (65 530) at roughly 55 k files.
- **P2** `remove_file_by_id` keeps the dead file's mapping alive, pinning the
  unlinked inode (`lazy_file_store.rs:456-462`).
- **P2** `save` fails permanently if any indexed path is non-UTF-8 (serde's
  `Path` impl errors) (`persistence.rs:42`).
- **P2** `#[serde(default)]` under bincode fixint is not forward compatibility;
  `Symbol` is part of the on-disk format with no version guard; two parallel
  version mechanisms (`CURRENT_VERSION = 3`, `FCSIDX01`); no golden-file test.
- **P2** Staleness check is second-granularity mtime + size
  (`persistence.rs:323-341`); a same-size edit within one second is invisible.
- **P2** Save and load materialise two full copies of the index (bitmaps
  serialised into a `HashMap<[u8;3], Vec<u8>>`, symbol cache cloned); the
  monolithic bincode blob cannot be mmapped or loaded lazily. This is the OOM
  point for multi-GB corpora.
- **P2** Bitmaps are never `optimize()`d; ubiquitous trigrams (`"   "`, `"the"`)
  cost a full 8 KB container per 65 k docs where a run container would be bytes.
- **P2** `remove_document` is a full-map `retain` per removed file
  (`trigram.rs:113-120`) and nulls `all_docs_cache`, which is never rebuilt
  until the next `finalize()`.
- **P2** `evict_all_fallbacks` locks a Mutex per file per search and is called
  twice per request (`lazy_file_store.rs:572`, `api.rs:288`, `service.rs:275`).
- **P2** Trigram extraction does one `FxHashSet` insert per byte with capacity
  capped at 1024 (repeated rehash) after a full `to_lowercase()` copy of every
  file (`trigram.rs:48-61`, `engine.rs:506`).
- **P3** `find_by_path_suffix` allocates per file and has no path-boundary
  check (`"ain.rs"` matches `main.rs`); canonicalisation differs between
  `add_file`, `register_file` and `refresh_file_by_id`.
- **P3** Dead code: `file_store.rs` (414 lines, only re-exported) plus its 12
  tests; `extract_trigrams`, `Trigram::from_slice`, `add_document`,
  `num_documents`, `mapped_count`, `get_all_paths`, `was_transcoded`,
  `detected_encoding`, `can_allocate_more`, `diagnose_mmap_error`,
  `is_binary_bytes` (duplicate of `is_binary_content`).
- **P3** Save truncates the temp file before locking (concurrent savers corrupt
  each other); `with_extension("bin.tmp")` turns `foo.idx` into `foo.bin.tmp`;
  version is checked after full deserialisation.

### 2.2 Query and ranking (`src/search/engine.rs`, `regex_search.rs`)

- **P1** Unbounded work in Full mode — see headline table.
- **P1** Line-0 definition boost — see headline table.
- **P2** `file_metadata` is computed only in `finalize()`; `index_batch` and
  `update_file` never touch it and `load_index*` leaves it empty, so Fast mode
  falls back to "first 2000 ids ordered by id" until the background build ends,
  and watcher-added files rank last forever (`engine.rs:1008-1047,1287-1293,
  1370-1386`).
- **P2** Short queries (< 3 bytes) and non-accelerated regexes scan the whole
  corpus; `search_symbols_in_document` touches file content **before**
  consulting the symbol cache (`engine.rs:1842-1850`).
- **P2** Regex acceleration misses `(?i)literal` (translated to `Class` nodes)
  and `abc+`-style trailing repetitions; both fall to a full scan with a
  `warn!` per query. The project's own validator uses `(?i)`
  (`regex_search.rs:97-136`, `fast_code_search_validator.rs:601`).
- **P2** Path filtering allocates a display-path `String` per candidate over the
  full candidate set (`path_filter.rs:183-201`, `engine.rs:769-800`).
- **P2** Per-candidate symbol maps are built eagerly for documents with zero
  matching lines (`engine.rs:2150-2176`).
- **P2** Per-line scalar verification; `memmem` is only used for the `/src/`
  path check (`engine.rs:244-296`). Regex is evaluated 2–3× per matching line
  and per-line matching means multi-line patterns silently return nothing.
- **P2** No query deadline, cancellation or result budget; the read guard is
  held across the whole rayon `par_iter`, and std `RwLock` writer preference
  means one slow Full search turns every concurrent search into a 503.
- **P2** `match_start`/`match_end` are byte offsets into the *truncated*
  content; the VS Code provider applies them as character columns on the real
  line (`vscode-extension/src/providers/textSearchProvider.ts:109-114`).
- **P3** `has_more` is `len >= max`; ties sort nondeterministically
  (`select_nth_unstable_by` on score only) so pagination is impossible.
- **P3** Filename hits appear only when a file has zero content matches; the 5×
  filename boost exists only in Fast mode, so Fast and Full rank differently.
- **P3** Greek final-sigma folding asymmetry between `str::to_lowercase` (index)
  and `char::to_lowercase` (`unicode_ci_find`).
- **P3** Regex compiled per request with no cache and no explicit `size_limit`;
  `regex_syntax::parse` runs twice.
- **P3** `search_ranked` / `search_with_filter_ranked` are ~90 duplicated lines
  with a third copy of candidate selection in `search_regex` and
  `search_symbols`; scoring constants are scattered magic numbers;
  `RegexAnalysis::literals` / `best_literal` are dead.

### 2.3 Indexing pipeline and watcher (`background_indexer.rs`, `watcher.rs`, `file_discovery.rs`, `main.rs`)

- **P1** mtime captured at save time; import retry storm; directory events;
  watcher poison — see headline table.
- **P2** Stale files are queued twice after a checkpoint load (sent explicitly,
  then rediscovered because the skip set is built from valid indices only)
  (`background_indexer.rs:569-621`).
- **P2** Watcher event paths are not canonical, so `find_file_id` takes the O(n)
  suffix scan; on a miss `update_file` falls through to `index_file`, which
  dedupes to the existing id and **adds** trigrams without removing the old
  ones (`engine.rs:2846,833-843`).
- **P2** `pending_imports` is not persisted and `resolve_imports` drains it, so
  incoming edges are lost after a checkpoint restore or for watcher-added files.
- **P2** Rayon's global pool (8 MB stacks for tree-sitter) is built lazily by
  the indexer; a search that runs first wins with 2 MB stacks; `--no-auto-index`
  plus gRPC `Index` never builds it (`background_indexer.rs:161-164`).
- **P2** `.gitignore` is not honoured — only `exclude_patterns`.
- **P2** The watcher bypasses `include_extensions`, the binary-extension list and
  `exclude_files` (`main.rs:578-587`).
- **P2** `save_index` does 1–2 `stat`s per file, per-root lowercase allocations
  per file, and a full symbol clone under the read lock; checkpoints run
  synchronously inside `process_batches` so discovery backs up.
- **P3** `IndexingProgress.errors` is never incremented; discovery
  canonicalises and double-stats every file; `shrink_to_fit` every 50 batches
  while the map is still growing; three copies of the load logic
  (`load_index`, `load_index_with_reconciliation`, `load_index_with_progress`)
  of which only the last is used by the pipeline and they have already
  drifted; symlinks skipped by discovery but followed by the watcher.
- Tests: `background_indexer.rs` has **none**; `process_event` is untested;
  nothing covers directory rename, watcher→engine wiring, double-send, mtime
  staleness, or poisoned-lock behaviour.

### 2.4 Symbols and dependencies (`src/symbols`, `src/dependencies`)

- **P1** `.tsx` grammar, JS/TS coverage, wrong symbol lines, Rust/Python import
  resolution, bare-name false edges — see headline table.
- **P2** Language gaps: Go `type X = Y` dropped; Rust emits a duplicate `Class`
  per `impl` block and misses trait method signatures, `macro_rules!`,
  `union`, `mod`; Python has **zero** module-level variables and no test; C
  misses `typedef` and prototypes; C++ misses in-class method declarations; C#
  misses namespaces and delegates, and the `record_struct_declaration` arm
  matches a node kind that does not exist in the grammar (dead); C/C++
  reference and double-pointer declarators yield names like `& foo()`.
- **P2** C# properties are typed `Method`; there is no Property/Field kind.
- **P2** Python `import_statement` uses field `name`, not `module_name`, so plain
  `import a, b` hits the text fallback and keeps only `a` (`extractor.rs:620`).
- **P2** JS misses `export … from`, dynamic `import()`, backtick `require`,
  `index.{js,ts}` resolution; extension list lacks `.mjs/.cjs/.mts/.cts`.
- **P2** `DependencyIndex::remove_file` never removes from `filename_to_paths`
  (the loop body is a no-op) and `update_file` re-registers, pushing a duplicate
  path per edit — unbounded growth; `path_to_id.retain` is O(N) per removal
  (`dependencies/mod.rs:236-243,50`).
- **P2** JSON/TOML/YAML/HTML/CSS/Markdown are fully parsed by tree-sitter yet
  no node kinds are captured for them — pure waste (multi-MB
  `package-lock.json` parsed for nothing) (`extractor.rs:91-96`).
- **P2** `register_file` calls `canonicalize()` inside the write-locked merge.
- **P2** Ranking: two inconsistent "heavily imported" formulas
  (`1 + log10(n)·0.5` line-level vs `log2(n).min(5)` additive in
  `FileMetadata::compute`); symbol search in Fast mode truncates to the top
  2000 files by base score **before** matching, so an exact symbol in a
  `/test/`-penalised file can be excluded (`engine.rs:1805-1820`).
- **P3** `.h` always C; extension match is case-sensitive; `column` is a byte
  column, undocumented; `Symbol.is_definition` is always true (vestigial, yet
  persisted); a new `Parser` per file; `TreeCursor` allocated per node;
  self-imports counted.
- Tests: no Python, plain JS, C, `.h` or `.tsx` symbol tests; **no test asserts
  `line` or `column`**; the two `dependencies` tests never touch
  `resolve_import_path`, `register_file` or `remove_file`; engine tests admit
  resolution "may or may not" happen.

### 2.5 Serving layer and configuration (`server`, `web`, `config.rs`, `main.rs`)

- **P1** Trust boundary (gRPC `Index`, `0.0.0.0`, CORS `Any`), no graceful
  shutdown — see headline table. Web bind failure inside `tokio::spawn` is
  logged and swallowed; the process runs on with no REST API
  (`main.rs:159-176`).
- **P1** Every path-addressed endpoint (`/api/file`, `/api/context`,
  `/api/dependents`, `/api/dependencies`) degrades to an O(n) suffix scan with
  an allocation per indexed file, because the UI round-trips *display* paths
  that never hit the canonical map; `search_handler` with `context>0` does this
  once per result under the read lock (`engine.rs:2324-2334`, `api.rs:295`).
- **P2** gRPC `max_results` defaults to 1 (proto3 zero clamped) vs REST 50.
- **P2** Contract drift: proto lacks `rank`, `context`, `has_more`,
  `elapsed_ms`, `dependency_count`; `SYMBOL_REFERENCE` is declared but never
  emitted; neither API has case-sensitivity, whole-word, offset or deadline.
- **P2** `rank` is silently ignored for regex and symbol searches; a non-literal
  regex on a large index is silently capped to 2000 files with no
  `rank_mode`/`candidates_searched` in the response.
- **P2** `/api/context` `context` is unbounded and `match_idx + context + 1`
  overflows `usize` (`api.rs:682-750`).
- **P2** `/api/diagnostics` reports a **fabricated** config (hard-coded
  `indexed_paths`, `max_file_size`, `watch_enabled=false`), does an O(n)
  allocating walk on every call, and `force_refresh` is never read.
- **P2** `/api/health` is liveness only; no readiness signal; no gRPC health.
- **P2** gRPC `Index` holds the write lock for the entire walk, ignores
  `exclude_patterns`/`max_file_size`/`exclude_files`, and uses `eprintln!`.
- **P2** Config: no `deny_unknown_fields` (typos silently ignored), no
  `validate()`, no `--web-address` CLI flag; `docs/DEPLOYMENT.md` documents
  `BIND_ADDRESS` and `MAX_CONCURRENT_SEARCHES`, which do not exist.
- **P2** No request timeouts, concurrency limits or body limits on either
  server; 512 slow requests exhaust tokio's blocking pool and stall every
  endpoint.
- **P2** `RUST_LOG` silently overrides `-v`; `FCS_TRACING_ENABLED=true`
  overrides `OTEL_SDK_DISABLED=true`; `#[instrument]` fields are declared but
  never recorded; the tonic trace span carries no method name.
- **P3** `new_with_indexing` / `create_server*` are dead, exported, and still
  use the substring exclusion bug fixed elsewhere in 0.9.0
  (`service.rs:50-206,96-101`); gRPC "streaming" collects everything first;
  3 s `thread::sleep` in async `main`; axum `Query` rejections are plain text;
  WebSocket does a `try_read` per socket per broadcast with a 16-slot buffer
  and no ping.
- Tests: none for `/api/file`, `/api/context`, `/ws/progress`, `context=`,
  `rank=`, 400 JSON for bad regex, 503 + `Retry-After`, CORS, gRPC defaults,
  config precedence or unknown keys; no unit tests at all in `web/`, `server/`,
  `telemetry.rs`.

### 2.6 Tests, CI, hygiene

- **P1** Repo hygiene (CRLF flip, corrupted `.gitignore`, dangling gitlinks,
  unpinned MSRV) — see headline table.
- **P1** `release.yml` is not gated on CI (`needs: build` only), and every
  `v*` tag also re-publishes the VS Code extension regardless of changes.
- **P1** Zero tests for `background_indexer.rs` (1049 lines) and `web/api.rs`
  (1275 lines); no test runs searches concurrently with updates; the watcher's
  rename path and debounce are untested.
- **P2** `cargo fmt --check` fails on 13 real hunks; CI uses floating `stable`
  so the next push goes red. `cargo clippy -- -D warnings` omits
  `--all-targets --all-features`, so tests, benches, examples and `ml-models`
  are never linted.
- **P2** Tests require system OpenSSL because the `reqwest` dev-dependency uses
  `native-tls` (on this machine it needed a manual header workaround);
  `rustls-tls` removes the requirement.
- **P2** The semantic engine is compiled into the keyword server
  unconditionally (`lib.rs:6-8`); `ndarray`, `hnsw_rs`, `sha2` are
  non-optional. `opentelemetry 0.27` drags in a second `tonic 0.12` /
  `axum 0.7` / `prost 0.13` / `rand 0.8` stack (461 packages in the lockfile).
  `md5` (one use), `glob` (one use, alongside `globset`), `tokio-test` (unused)
  and `bincode 1.x` (maintenance-only) are all removable or stale.
- **P2** No `[profile.release]` (no LTO/strip); the release workflow ships
  unstripped debug archives for five targets.
- **P2** Benchmarks use 100–200 files of ~3 KB; every query bench finishes in
  under 1 ms and mostly measures fixed overhead; the 200 % alert threshold on
  shared runners catches only gross regressions; nothing benchmarks symbol
  search, incremental update or reconciliation. `actions/cache` on `target/`
  grows monotonically.
- **P2** `docs/DEVELOPMENT.md` is materially wrong (module tree, "single-
  threaded indexing", `env_logger`, `tree_sitter_cpp::language()`, release
  targets); `docs/REVIEW.md` reviews v0.2.1; `CONTRIBUTING.md` lists shipped
  work as open.
- **P3** 13 `pub fn` candidates with no callers; 215 `.unwrap()` in non-semantic
  `src`; engine.rs at 4234 lines absorbs search, indexing, persistence,
  scoring, display paths and progress.

---

## 3. Roadmap

Phases are ordered by severity and dependency. Each task lists files, the
change, and an acceptance criterion so it can be picked up without this
document's context. Conventions: one task per commit, conventional-commit
messages, `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`
green before merge. Do not modify `vscode-extension/`.

### Phase 0 — Unblock the repo (days)

Goal: a clean, reproducible tree and a green CI before touching engine code.

- [x] **0.1 Line endings.** DONE: `.gitattributes` added (LF default, CRLF for
  `.ps1/.bat/.cmd`, binary for assets); the 172-file CRLF diff was discarded
  after confirming it was EOL-only; index was already LF. Add `.gitattributes` (`* text=auto eol=lf`,
  `*.ps1 *.bat text eol=crlf`, `*.png *.woff2 *.dll *.lib *.exe binary`);
  `git checkout -- .` to discard the current EOL-only diff (verified: `git diff
  --ignore-cr-at-eol --stat` is empty); `git add --renormalize .` once.
  Accept: `git ls-files --eol` shows `i/lf w/lf` for all text files.
- [x] **0.2 Tracked junk.** DONE: `.gitignore` rewritten as ASCII/LF
  (`onnxruntime/`, `tailwindcss.exe`, `*.zip` restored as separate lines,
  `.claude/settings.local.json` added); `onnxruntime` (23 files) and the three
  `test_corpus` gitlinks untracked; crate `exclude` widened. Rewrite the corrupted `.gitignore` line as ASCII;
  `git rm -r --cached onnxruntime test_corpus`; add `onnxruntime/**` to
  `Cargo.toml exclude`; either add `.gitmodules` or drop the gitlinks.
  Accept: fresh clone has no submodule errors; `git ls-files onnxruntime` empty.
- **0.3 Toolchain.** Add `rust-version = "1.82"` and `rust-toolchain.toml`;
  pin the CI toolchain; add an MSRV job; correct "Rust 1.70+" in README,
  CONTRIBUTING, DEVELOPMENT, CHANGELOG. Run `cargo fmt`.
  Accept: `cargo +1.82 check --all-targets` passes; `cargo fmt --check` clean.
- [x] **0.4 CI lint width.** DONE: clippy runs `--all-targets --all-features`,
  `cargo doc` with `-D warnings`, release `build` now `needs: test` (fmt +
  clippy + tests on the tag), extension publishes on `ext-v*` tags only,
  `Swatinem/rust-cache` everywhere. The 8 clippy warnings and 1 rustdoc error
  this exposed were fixed. `cargo clippy --all-targets --all-features -D warnings`
  and `cargo doc --no-deps` in `ci.yml`; `release.yml` `needs:` the test job;
  `publish-extension.yml` triggers on `ext-v*` tags or `paths:
  vscode-extension/**`; switch to `Swatinem/rust-cache`.
- [x] **0.5 Dev-dependency TLS.** DONE: both the dev-dependency and the
  optional `ml-models` `reqwest` use `rustls-tls` (the `--all-features` lint
  otherwise still needed OpenSSL); `tokio-test` removed. `reqwest` dev-dep → `default-features = false,
  features = ["rustls-tls", "json", "blocking"]`; drop `tokio-test`.
  Accept: `cargo test` builds on a machine without `libssl-dev`.

### Phase 1 — Correctness hotfixes → release 0.9.1 (1–2 weeks)

Goal: no path in the shipped binary produces wrong results or loses data.

- [x] **1.1 Persist tombstones correctly (P0).** DONE (commit `0c743cc`):
  `save_index` now builds live-id → position while compacting the file table
  and remaps symbols, dependency edges and (only when something was removed)
  the trigram bitmaps through it; regression test
  `test_save_after_remove_keeps_ids_consistent`. `engine.rs:2337-2411`,
  `persistence.rs`. At save time build `old_id → position` from the compacted
  file list and remap trigram bitmaps, `symbols` and `dependency_edges` through
  it (reuse `remap_trigram_bitmaps`); or persist an `alive` bitmap so position
  equals id. Accept: index 4 files, `remove_file` #1, save, reload — each
  remaining unique token resolves to its own file (the throwaway test used for
  this review fails today with tokens 2 and 3 returning nothing).
- [x] **1.2 Line-0 boost.** DONE (commit `935cd4f`): FileName excluded from
  `symbol_def_lines` in both per-document paths; test
  `test_first_line_does_not_get_definition_boost` fails without the fix.
  `engine.rs:2150`, `1985`. Filter
  `SymbolType::FileName` out of `symbol_def_lines` in both the scored and regex
  paths. Accept: a file with a plain mention on line 1 and a definition on line
  5 ranks line 5 first.
- [x] **1.3 Safety gate scope.** DONE (commit `c7121fa`): `content_safety_check`
  split into the binary check (still skips the file) and
  `tree_sitter_safety_check` (sets `tree_sitter_safe` on the partial file;
  extraction skipped, trigrams kept); test
  `test_long_line_file_is_searchable_without_symbols`. `engine.rs:479`, `utils.rs:175`. Apply
  `content_safety_check` to tree-sitter only; keep the file in the trigram
  index; skip the check when `enable_symbols=false`. Accept: a 200 KB single-
  line JSON file is searchable and has no symbols.
- [x] **1.4 Record mtime/size at read time.** DONE (commit `db73972`):
  captured in `PartialIndexedFile::process`, carried on `PreIndexedFile`,
  kept per id in `SearchEngine::indexed_meta` (seeded from persisted metadata
  on all three load paths), used by `save_index`; test
  `test_edit_between_index_and_save_is_detected_as_stale`. Add both to `PartialIndexedFile` /
  `PreIndexedFile`, store per file, and have `save_index` persist those.
  Accept: edit a file between `index_file` and `save_index`; on reload it is
  reported stale.
- [x] **1.5 Symbol positions.** DONE (commit `97f00eb`): all arms take the
  name node's position; `.tsx` → `LANGUAGE_TSX`; added `method_definition`,
  arrow/function-expression `variable_declarator`, generator functions,
  `abstract_class_declaration`, `internal_module`; three new tests including
  the first line/column assertions. `extractor.rs`. Take `line`/`column` from the
  `name` child, not the declaration node; parse `.tsx` with `LANGUAGE_TSX`;
  add `method_definition` and `variable_declarator` + arrow/function
  expression for JS/TS. Accept: tests asserting line numbers for Java
  `@Override`, TS decorator, C multi-line signature; a `.tsx` React component
  yields its symbols.
- [x] **1.6 Graceful shutdown.** DONE (commit see git log `feat(server):
  graceful shutdown`): signal task + AtomicBool + watch channel; tonic
  `serve_with_shutdown`, axum `with_graceful_shutdown`; indexer discovery and
  batch loops stop on the flag and still finalize + save; watcher saves
  pending updates on exit; main joins everything; web bind is fatal. Verified
  manually end to end (SIGTERM → save → restart finds the edit). `main.rs`. `serve_with_shutdown(ctrl_c/SIGTERM)`
  for tonic, `with_graceful_shutdown` for axum, an `AtomicBool` the indexer
  polls between batches, final `save_index` if dirty, `shutdown_telemetry()`.
  Make web bind failure fatal. Accept: SIGTERM mid-watch leaves a loadable
  index containing the last edit.
- [x] **1.7 Writer-path safety.** DONE: `with_engine_write` (poison recovery +
  `catch_unwind`) on the watcher path, 8 MB watcher stack, rayon pool built
  eagerly in `main`, REST `try_read_engine` and gRPC search recover from
  poison instead of a permanent 500. `main.rs:582-661`. Wrap `update_file` /
  `remove_file` in `catch_unwind`; run the watcher loop on a thread with an
  explicit 8 MB stack; recover from poison via `into_inner()` in `main.rs` and
  `api.rs` (or move the engine to `parking_lot::RwLock`); build the rayon pool
  eagerly in `main` before spawning anything. Accept: a file whose processing
  panics on the watcher path is logged and skipped; searches keep working.
- [x] **1.8 Directory events.** DONE: `search::incremental::apply_change`
  handles file *and* directory paths (`remove_files_under` + discovery over a
  renamed-in directory) and applies the build's eligibility rules via a shared
  `FileDiscoveryIterator::accepts` (this also completes the watcher half of
  2.4). Tests: `test_directory_rename_and_delete_update_index`,
  `test_watcher_change_respects_eligibility`; verified against the live
  server too. `watcher.rs:194-199`, `main.rs:616-648`. On
  `Renamed{from,to}` / `Deleted(p)` with no file id, remove every store entry
  with that path prefix; if `to` is a directory, run discovery over it.
  Accept: integration test renaming a directory of three files.
- [x] **1.9 Input caps.** DONE: `/api/context` capped at 200 lines a side with
  saturating arithmetic; user regexes built with `size_limit` 4 MB /
  `dfa_size_limit` 2 MB (pathological pattern → 400). Two HTTP tests. `api.rs:682-750`. Cap `/api/context` `context` and use
  `saturating_add`; add `RegexBuilder::size_limit`. Accept:
  `context=usize::MAX` returns 400, not a panic.
- [x] **1.10 Trust boundary defaults.** DONE: loopback defaults,
  `server.cors_origins` (empty = same-origin), gRPC `Index` scoped to the
  configured roots via `create_server_with_engine_scoped` and no symlink
  following, README "Network exposure" section. Test
  `test_grpc_index_rejects_paths_outside_scope`. `config.rs:195-201`, `web/mod.rs:44-47`,
  `service.rs:319-403`. Default both binds to `127.0.0.1`; CORS opt-in via
  `server.cors_origins`; gate gRPC `Index` behind `is_path_in_scope` (or remove
  it); document the local-only model in README and DEPLOYMENT. Accept: gRPC
  `Index(["/"])` is rejected; cross-origin fetch fails by default.
- [x] **1.11 Dependency graph bookkeeping.** DONE: `id_to_path` makes
  re-registration idempotent and removal O(1), pruning `filename_to_paths`.
  Test `test_reregister_and_remove_keep_lookups_bounded`. `dependencies/mod.rs:236-243,50`. Make
  `remove_file` actually remove from `filename_to_paths`; dedupe on
  re-register; keep an `id → path` map so removal is O(1). Accept: 1000
  update cycles on one file leave one entry.

Exit: 0.9.1 tagged with regression tests for 1.1, 1.2, 1.3, 1.4, 1.5, 1.8.
**Phase 1 code complete** (all 11 tasks, each with a test or a manual
verification noted above). Tagging 0.9.1 is left to the maintainer; the
CHANGELOG `[Unreleased]` section lists the changes.

### Phase 2 — Incremental index integrity → 0.10 (2–3 weeks)

Goal: the index after an hour of edits is identical to a fresh build.

- [x] **2.1 Batch watcher events.** DONE (commit `9c521e1`): 200 ms gather
  window in `main.rs`, `incremental::apply_changes` coalesces per path and
  removes all doomed ids with one `TrigramIndex::remove_documents` pass.
  Test `test_apply_changes_batches_and_coalesces`. Drain the channel for up to ~200 ms, dedupe
  by path, apply removals with **one** pass over the trigram map using a
  `RoaringBitmap` of doomed ids, re-index the modified set through
  `process_batch`. Accept: a `git checkout` touching 2000 files takes one
  write-lock window, not 2000.
- [x] **2.2 `finalize_incremental(ids)`.** DONE (same commit): the all-docs cache
  is updated in place on add/remove; `refresh_file_metadata` after
  `update_file` and for files whose in-edges changed; `compute_all_file_metadata`
  after every persisted load. Test `test_remove_documents_bulk_keeps_cache_warm`. Recompute `all_docs_cache` and
  `FileMetadata` for touched files (and in-edge targets) after every batch and
  watcher update; call a metadata-only finalize after `load_index*`. Accept:
  a watcher-added file gets the filename boost in Fast mode; no
  `all_documents()` recomputation after the first event.
- [x] **2.3 Canonical roots.** DONE (commit `fcdeace`): `IndexerConfig::
  canonicalize_paths` in `with_overrides`; `update_file`/`remove_file` use
  `find_file_id_exact` (canonical, no suffix fallback). Test
  `test_update_file_uses_canonical_exact_match`. Canonicalise configured paths once in `Config`;
  pass canonical paths to the watcher; remove the suffix fallback from
  `update_file` / `remove_file`. Accept: exact-path lookup hits on every event
  (assert via a counter in tests); no trigram accumulation after 100 updates
  of one file.
- [x] **2.4 One `FileEligibility`.** DONE with 1.8: `FileDiscoveryIterator::
  accepts` / `file_discovery::is_eligible` are used by discovery and the
  watcher (gRPC `Index` still walks on its own — see 4.7). Factor `include_extensions`, binary
  extensions, `exclude_files`, size cap and `PathFilter` into one struct used
  by discovery, the watcher and gRPC `Index`. Accept: with
  `include_extensions=["rs"]`, a changed `.log` is not indexed by the watcher.
- [x] **2.5 Import resolution v2.** DONE (commit `9b89a29`): per-language
  resolvers (Rust crate/self/super + `mod.rs`, Python dots/packages/
  `__init__.py`, JS extensions/`index.*`/`.js`→`.ts`/`@/` aliases), bare names
  resolve to nothing; extractor gains `import a, b`, re-exports, dynamic
  `import()`, backtick `require`. Four new tests. `dependencies/mod.rs`. Per-language
  resolvers: Rust `mod foo;` → sibling `foo.rs` / `foo/mod.rs`, `use
  crate::/super::/self::` against the crate root; Python leading dots as
  parent hops, dotted paths as directories, `__init__.py`; JS `index.*`,
  full extension list, `export … from`, dynamic `import()`. Remove the
  relative→bare fallback; for bare names prefer the nearest ancestor match.
  Accept: unit tests per language; no edge from `lodash/merge` to a local
  `merge.ts`.
- [x] **2.6 Stop the import retry storm.** DONE: parked imports keyed by path
  segment, retried only for stems added in the batch; persisted in the index
  (format v4 / `FCSIDX02`; old files rebuild). Tests
  `test_waiting_import_resolves_when_target_appears`,
  `test_unresolved_imports_survive_reload`. (Bench deferred to 6.5.) Keep a `filename → waiting importers`
  map so a newly indexed file pulls in only its waiters; persist unresolved
  imports in checkpoints; move `canonicalize()` out of the write-locked merge.
  Accept: resolution cost per batch is O(new files), measured in a bench.
- [x] **2.7 Double-send fix.** DONE (commit `5a99885`): `should_skip_discovered`
  covers stale-sent paths; first unit test in `background_indexer.rs`. `background_indexer.rs:569-621`. Add stale paths to
  the skip set (or don't pre-send them). Accept: `files_indexed` equals the
  number of files on disk after a checkpoint restart.
- [x] **2.8 Optional `.gitignore`.** DONE (commit `ddb88d1`): discovery uses the
  `ignore` crate's walker; `is_gitignored` for single watcher paths;
  `indexer.respect_gitignore` (default true). Test
  `test_gitignore_is_respected_and_optional`. Use the `ignore` crate's `WalkBuilder`
  (parallel discovery for free), config flag default on. Accept: a repo with
  `build/` in `.gitignore` and no exclude pattern does not index it.
- [x] **2.9 Tests.** DONE (commit `test: batch pipeline…`): background-indexer
  unit tests (drain/flush, shutdown flag, poisoned-lock recovery), a
  search-during-update concurrency test and a real `notify` watcher test.
  The concurrency test reproduced the retrieval SIGBUS (P1) on its first
  run, so 6.2 was pulled forward: files ≤ 1 MiB are now served by owned
  reads and never mapped (commit `5ed8fe4`). `background_indexer` unit tests (batch drain, checkpoint
  cadence, skip set, panic isolation, poison recovery); a concurrency test
  running searches in a loop during `update_file` / batch merge; a real
  `notify` watcher test (create/modify/delete/rename, short debounce).

### Phase 3 — Bounded, faster queries → 0.11 (2–3 weeks)

Goal: every query's cost is proportional to `max_results` and the candidate
set, never to the corpus.

- [x] **3.1 Work budget and deadline.** DONE (commit `bf38ac9`): `SearchLimits`
  (page, offset, match budget = 8× page min 512, deadline) + `QueryRun`
  checked per document and per match in every search; `SearchRankingInfo`
  reports `truncated_by_budget` / `total_matches`. Test
  `test_match_budget_bounds_work_and_is_reported`. Thread an `AtomicUsize` budget
  (`max_results × k`) and a deadline through the rayon loop in all four search
  entry points; stop scanning a document once the budget is hit; make Full
  mode honour a cap or require explicit opt-in; report `truncated_by_budget`.
  Accept: `rank=full` for `"the"` on a 100 k-file synthetic corpus returns in
  bounded time and memory.
- [x] **3.2 Eviction O(evicted).** DONE: small files (owned reads) are skipped
  without a lock; duplicate handler calls removed. Track the ids with populated fallbacks; drop the
  duplicate call from the handlers.
- [x] **3.3 Regex acceleration.** DONE: mandatory-text extraction covers
  `(?i)` case classes and min≥1 repetitions; 64-entry regex LRU; log demoted.
  Tests `test_case_insensitive_literal_is_accelerated`,
  `test_repetition_prefix_is_required`. Lower simple-case `Class` nodes to literals so
  `(?i)needle` is accelerated; include runs from `Repetition{min≥1}`
  prefixes; demote the "no constraints" log to `debug`; small LRU of compiled
  regexes. Accept: `(?i)needle` and `abc+` show `total_candidates` far below
  the corpus size.
- [x] **3.4 Verification.** DONE (commit `5da3240`): memchr2 whole-buffer
  scan for ASCII needles with line resolution at hits; lazy `SymbolLineMaps`;
  `FileMetadata.display_path` makes path filtering allocation-free. Test
  `test_ascii_line_hits_matches_per_line_search`. `memmem::Finder` over whole-file content with line
  resolution via `memrchr`; lazy symbol maps on first hit; symbol search
  consults the cache before touching content; precompute display path (or an
  extension/language bitmap) in `FileMetadata` so path filtering is
  allocation-free. Accept: search bench on a 10 k-file corpus improves; no
  per-candidate `String` allocations in the filter path.
- [~] **3.5 Shared candidate runner.** Runner DONE (`run_candidates` is the only
  path for all four searches). Still open: `RankingWeights`, unifying the two
  "heavily imported" formulas and the filename boost across modes, and the
  `engine.rs` split. Extract `run_candidates(candidates,
  rank_mode, budget, per_doc_fn)` used by `search_ranked`,
  `search_with_filter_ranked`, `search_regex`, `search_symbols`; move scoring
  constants into a `RankingWeights` struct with `Default`; unify the two
  "heavily imported" formulas and the filename boost across modes.
  Accept: Fast and Full agree on the top result for a golden fixture corpus.
- [x] **3.6 Deterministic ordering and pagination.** DONE: tie-break on
  `(score, file_id, line)`, `offset` in the engine and `/api/search`
  (`total_matches`, `truncated_by_budget`, `offset`; `has_more` from the real
  total). Tests `test_deterministic_order_and_offset_paging`,
  `test_http_search_offset_paging_and_totals`. Tie-break on
  `(score desc, file_id, line)`; add `offset` (REST) / cursor; return
  `total_matches` when the budget was not hit; replace the `len >= max`
  heuristic. Accept: two identical requests return identical order; page 2
  never repeats page 1.
- [x] **3.7 Match offsets contract.** DONE (commit `8f208bb`): `line_match_start/end`
  (bytes into the full line) and `match_column` (char column) on engine, REST
  and gRPC; documented in `docs.html`. Test `test_match_offsets_refer_to_full_line`. Return `line_match_start/end` in bytes into
  the untruncated line plus a UTF-16 column, and all occurrences on the line;
  document the contract in `docs.html`; keep the old fields for one release.
  (The VS Code provider can then be fixed separately.)
- [~] **3.8 Symbol search quality.** Candidates are no longer truncated by base
  score before matching and the symbol cache is consulted before any read
  (DONE). Still open: per-line dedupe and exact > prefix > substring / kind
  weighting. Do not truncate symbol candidates by base
  score before matching; dedupe rows per line; weight exact > prefix >
  substring and by symbol kind.

### Phase 4 — Serving layer and operability → 0.12 (1–2 weeks)

- **4.1 Fix path lookup.** `engine.rs:2324-2334`. Make `find_file_id` reverse
  `make_display_path` (strip root name, rejoin canonical root) so display paths
  hit the O(1) map; in `search_handler` resolve each file once and split lines
  once. Accept: `/api/file` on a 1 M-file synthetic index does no per-file
  allocation.
- **4.2 Contract alignment.** Proto: `max_results == 0` → 50; add `rank`,
  `context_lines`, `offset`, `case_sensitive`, `whole_word`, `deadline_ms`,
  `has_more`, `elapsed_ms`, `dependency_count`; drop or implement
  `SYMBOL_REFERENCE`; stream results from inside `spawn_blocking`. REST:
  honour `rank` for regex/symbols; always return `rank_mode` and
  `candidates_searched`. One documented contract for both.
- **4.3 Limits.** `TimeoutLayer`, `RequestBodyLimitLayer`, a `Semaphore`
  around search (the documented `MAX_CONCURRENT_SEARCHES`), tonic `.timeout()`
  and `.concurrency_limit_per_connection()`.
- **4.4 Readiness and metrics.** `/api/ready` (200 only when the index is
  loaded or the build completed), `tonic-health`, Prometheus `/metrics`
  (search latency histogram, results, 503s, index size, file count).
- **4.5 Config hygiene.** `#[serde(deny_unknown_fields)]`, `Config::validate()`
  (paths exist, addresses parse, `batch_size > 0`, `index_path` parent
  writable), `--web-address`; fix or remove the phantom env vars in
  DEPLOYMENT.md; `-v` sets the default filter and logs when `RUST_LOG`
  overrides it; `OTEL_SDK_DISABLED=true` is final.
- **4.6 Diagnostics.** Plumb `IndexerConfig` into `WebState`; use
  `ConfigSummary::from`; cache the extension breakdown per index generation;
  honour `force_refresh`.
- **4.7 gRPC `Index`.** If kept, route through the background indexer and
  return a job id; otherwise delete it together with `new_with_indexing` and
  `create_server*`.
- **4.8 Telemetry.** Record the instrumented fields; give the tonic span a
  method name; JSON envelope for axum `Query` rejections; WebSocket ping and a
  larger broadcast buffer.
- **4.9 Tests.** `/api/file`, `/api/context` (incl. overflow), `/ws/progress`,
  `context=`, `rank=`, 400 JSON for bad regex, 503 + `Retry-After` under a
  held write lock, CORS, gRPC default `max_results`, gRPC `Index` scope
  rejection, config precedence and unknown keys.

### Phase 5 — Symbols and dependencies v2 → 0.13 (2–3 weeks)

- **5.1 Migrate to tree-sitter `tags.scm` queries** per language
  (definition/name captures). This replaces the 13-language `match` with
  colliding kind names, gives correct name positions for free, isolates each
  grammar's mapping for testing, and makes adding a language a data change.
- **5.2 Kinds.** Add Property, Field, Macro, Namespace, TypeAlias; stop typing
  C# properties as `Method`; dedupe `impl_item` against the struct/enum of the
  same name.
- **5.3 Coverage.** Rust trait method signatures, `macro_rules!`, `union`,
  `mod`; Python module-level assignments and `import a, b`; Go `type_alias`;
  C `typedef`/prototypes; C++ in-class declarations; C# namespaces/delegates;
  correct C/C++ reference and pointer declarators; case-insensitive extension
  match; `.h` heuristic (C++ if the file contains `class`/`namespace`/
  `template`).
- **5.4 Stop parsing config/markup grammars** until captures exist (then add
  cheap ones: Markdown headings, JSON/TOML/YAML top-level keys).
- **5.5 Parser reuse.** Thread-local `Parser` per language; single
  `TreeCursor` traversal; a parse timeout via `parse_with_options`.
- **5.6 Tests.** One symbol test per language asserting names **and** lines,
  including Python, plain JS, C, `.h`, `.tsx`; `DependencyIndex` tests for
  `resolve_import_path` across languages and `remove_file`→`register_file`
  cycles; a golden dependency-count fixture.

### Phase 6 — Scale: persistence v4 and memory diet → 1.0 (4–6 weeks)

Goal: make the README's "multi-gigabyte" claim true and measured.

- **6.1 Persistence format v4.** Sectioned, mmap-able layout: header (magic,
  version, CRC), file table (paths as bytes, nanosecond mtime, size), trigram
  directory, bitmap region. Lazy load; no double materialisation on save or
  load; `optimize()` bitmaps before write; one version mechanism; drop
  `serde(default)`; golden-file compatibility test and a v3 fixture that is
  rejected cleanly.
- [x] **6.2 Retrieval without per-file mmap.** DONE EARLY (commit `5ed8fe4`,
  during 2.9): `MMAP_THRESHOLD_BYTES` = 1 MiB; small files are read into an
  owned buffer per access and never mapped, large files keep the mapping.
  Mmap accounting for the large-file path is still the old code (see index
  findings P2). Owned bounded reads for small files
  (the overwhelming majority) with mmap only above a threshold, or a segmented
  blob store — removes the SIGBUS exposure and the `max_map_count` ceiling
  together. Own the mmap accounting in `LazyFileStore` if mmap stays.
- **6.3 Memory diet.** Intern paths once (`Arc<Path>` or arena) and stop
  duplicating them in `path_to_id`; collapse the three `OnceLock`s + `Mutex`
  per file into one state cell; delete the fallback Mutex cache; return
  `all_documents()` by reference.
- **6.4 Indexing throughput.** Bitset-based unique-trigram extraction with
  on-the-fly ASCII lowercasing (no `to_lowercase()` copy, no per-byte hash
  insert); drop the mid-build `shrink_to_fit`; checkpoint saves on a separate
  thread from a snapshot.
- **6.5 Benchmarks that match the claim.** A pinned, CI-cacheable mid-size real
  corpus (50–100 MB) for search, indexing, incremental update, symbol search
  and reconciliation benches; a nightly large-corpus run with memory and
  file-count reporting; publish "files / GB indexed / RSS / p50 latency" in
  the README instead of the 100-file numbers.
- **6.6 Release profile.** `[profile.release] lto = "thin", codegen-units = 1,
  strip = true`; stop shipping debug archives by default; add a `semantic`
  feature gating `src/semantic*`, `ndarray`, `hnsw_rs`, `sha2` and the
  semantic bin; bump `opentelemetry*` onto `tonic 0.14`; drop `md5`, `glob`;
  add `cargo-deny` and Dependabot.

### Phase 7 — Query language (1.x, after Phase 3)

Each of these needs only the candidate-set plumbing that Phase 3 creates:

- Case-sensitive toggle (case-sensitive verify path; candidates unchanged).
- Whole-word (`\b` check post-match).
- Multi-term AND: intersect per-term trigram bitmaps, then require all terms
  on the line or in the file.
- `file:` / `lang:` / `-term` syntax parsed into `PathFilter` plus exclusion
  terms.
- Multi-line regex (`\n`, `(?s)`) via whole-content matching with line
  resolution.
- Symbol-reference results (`SYMBOL_REFERENCE`) once `tags.scm` provides
  reference captures.

### Cross-cutting: structure and documentation (spread across phases)

- Split `engine.rs` (4234 lines) into `search/{query,ranking,indexing,
  reconcile,display,progress}.rs` and move persistence/reconciliation into
  `index/`; target ≤ 1200 lines for the query module. Do this in Phase 3.5
  when the shared runner lands, not before Phase 1 (keeps hotfix diffs small).
- Delete dead code: `index/file_store.rs`, `load_index` and
  `load_index_with_reconciliation` (make them thin wrappers or remove),
  `service.rs` `create_*`, the 13 uncalled `pub fn`s, `RegexAnalysis::literals`.
- Rewrite `docs/DEVELOPMENT.md` against the code; archive `docs/REVIEW.md` and
  the completed 2026-06-09 plan under `docs/archive/`; refresh
  `CONTRIBUTING.md`; add `PULL_REQUEST_TEMPLATE.md`, `CODEOWNERS`,
  `SECURITY.md`, Dependabot.

---

## 4. Sequencing and release map

| Release | Phases | Theme | Rough size |
|---------|--------|-------|-----------|
| 0.9.1 | 0, 1 | Data-loss and wrong-result fixes, safe defaults | 1–2 weeks |
| 0.10 | 2 | Incremental index equals fresh build | 2–3 weeks |
| 0.11 | 3 | Bounded, faster queries; deterministic paging | 2–3 weeks |
| 0.12 | 4 | API contract, limits, readiness, config | 1–2 weeks |
| 0.13 | 5 | Symbols/imports v2 via tags.scm | 2–3 weeks |
| 1.0 | 6 | Persistence v4, memory diet, real benchmarks | 4–6 weeks |
| 1.x | 7 | Query language | incremental |

Phase 0 and Phase 1 ship together and first. Phases 2 and 3 are independent
and can run in parallel by two people; 4 depends on 3.6 (pagination) for its
proto changes; 5 is independent of 2–4; 6 depends on 2 (incremental
correctness must be settled before the format changes).

## 5. Verification checklist (every phase)

- `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test` (lib + integration) on Linux, macOS, Windows
- Manual: start the server on a real repo; edit, delete, rename files and
  directories while the build runs; kill the process with SIGTERM; restart and
  confirm searches match `rg` for ten sampled tokens.
- Benchmarks: no regression > 20 % on the mid-size corpus (Phase 6.5 onward).
