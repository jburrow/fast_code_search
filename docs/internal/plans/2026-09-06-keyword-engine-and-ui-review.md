# Keyword engine and web UI review (2026-09-06)

Scope: the keyword search engine (trigram index, query execution, regex,
symbols and references, incremental indexing, persistence, watcher), the
REST/gRPC layer and the keyword web UI (`static/index.html`, `keyword.js`,
`common.js`, `docs.html`, `diagnostics.html`). Semantic search and the VS Code
extension were out of scope.

Method: every in-scope source file was read; suspected defects were reproduced
against the live local server (v0.10.1, 61,427 files from the workspace) with
curl, in the browser, or with scratch crates built against the library. Each
finding is marked **Confirmed** (reproduced) or **Plausible** (strong code-level
evidence, not reproduced). Reviewed at commit `0f82ba0`.

Overall: the engine is in good shape structurally (atomic single-writer save,
CRC-validated mmap load, budgeted and deadline-bounded queries, `try_read` lock
discipline, regex hardening). The serious problems cluster in four places:
concurrency around saves and long searches, regex candidate generation,
incremental update cost, and a UI that ignores half of the API contract.

## Status (2026-09-07)

Implemented on `main` (see `CHANGELOG.md` [Unreleased]): every priority item
and sections 1–4 except the following, left open deliberately:

- 1.10 interned reference names are never freed (bounded by the number of
  distinct identifiers; a rebuild reclaims them).
- 2 (item 8) parked imports: dead importers are now dropped when they are
  retried; per-`(file, import)` deduplication is not done.
- 6: JavaScript helper tests (`npm test`) and a Tailwind rebuild check now
  run in CI's Web UI job (added after v0.11.0).

After v0.11.0 the remaining UI items were closed too: result diversity
(hits interleaved by file within a tier), on-demand rendering of result
groups, inline SVG icons instead of the icon font, the inline dependency
errors / clipboard fallback / preview-cache invalidation trio, and the
unused dark stylesheet. The page stays single-theme by design.

Follow-ups found while verifying on the live index: the reload eligibility
pass initially cost 14.8 s (gitignore chains are now cached per directory),
and ranking lines by term count was not enough under fast ranking, so
multi-term queries now prefer the phrase (files containing it are always
opened; phrase lines outrank all-term lines outrank single-term lines).

## Priorities

| # | Severity | Area | Finding |
|---|----------|------|---------|
| 1 | Critical | Persistence | Two concurrent saves share one tmp file and the fallback deletes the index; reachable on Ctrl+C with `watch = true` |
| 2 | High | Regex | Repetition constraints require a literal that never exists (`fo+bar` misses `foobar`) |
| 3 | High | References | Stale indexed columns sliced against live content panic on non-ASCII lines, turning `references=true` into a 500 |
| 4 | High | Dependencies | `update_file` drops all edges *into* the file; editing a popular module zeroes its dependents |
| 5 | High | API | `offset` is unclamped and raises the match budget without bound; one request can hold the read lock for 15 s and 600 MB |
| 6 | High | API | The 30 s timeout drops the response but the search keeps running; three timed-out requests peaked at 3 GB RSS |
| 7 | High | Reconcile | Config changes (excludes, extensions, size, gitignore) are logged and ignored on reload; excluded files stay indexed forever |
| 8 | High | Incremental | Each modified file does a full posting-list scan under the write lock; a branch switch blocks all searches for seconds to minutes |
| 9 | High | UI | Match highlighting rebuilds hits from the raw query string and ignores the server's match offsets |
| 10 | High | UI | Multi-term queries (`fn main`) show lines matching *any* term in file order, so the top results rarely contain what was typed |
| 11 | High | UI | Stored XSS on the diagnostics page through file-extension and error strings |

## 1. Engine core

### 1.1 Regex candidate generation misses repetitions — Confirmed, High

`src/search/regex_search.rs:136` (`mandatory_text`) treats `X+` and `X{n}` as
one contiguous copy on both sides, so `P X+ S` demands the literal `PXS`.
`fo+bar` yields the trigram constraint `fobar`, and a file containing `foobar`
is never a candidate.

Live: `fo+bar` (regex) returns 105 candidates and no `fn foobar()` hit;
`foo+bar` returns 385 candidates and finds it. Scratch: `xa{3}y` on `xaaay`
returns nothing.

Fix: treat a repetition as a run boundary. Emit `prefix + sub.repeat(min)` as
one constraint and start the next run with `sub.repeat(min)`, so `PX` and `XS`
are required but `PXS` never is. Add a property test asserting trigram
candidates are a superset of files where the compiled regex matches.

### 1.2 `(?i)` runs break on `k` and `s` — Confirmed, Medium

`regex_search.rs:152-188` rejects a case-insensitive class when it contains
Kelvin sign or long s (`(?i)k`, `(?i)s`), which regex-syntax always adds. The
literal run breaks at every k or s, so `(?i)task`, `(?i)mask`, `(?i)pass`
produce no constraints and scan all 61k files. Accept a class member when its
simple case fold equals the target.

### 1.3 Reference lookup panics on stale positions — Confirmed, High

`src/search/engine/text.rs:313` via `query.rs:649-662`: `references_in_document`
applies the tree-sitter column captured at index time to the *current* file
content. If the file changed before the watcher ran, or watching is off,
`char_column` slices mid-character and panics. Rayon re-raises it inside
`spawn_blocking`, so `/api/search?references=true` answers 500 until the file
is re-indexed. Clamp to char boundaries and skip the reference unless
`line.get(start..end) == Some(name)`.

### 1.4 Reference extraction gaps — Confirmed, Medium

Call sites in TypeScript and JavaScript are not captured: `raceFilter(` in
vscode-copilot-chat and `escapeHtml(` in `keyword.js` return no references
(template-literal uses in `common.js` do). Rust type mentions are not
captured either: `SearchEngine` with `references=true` returns only Python
hits. Python works. Users get a silent empty result rather than an
"unsupported for this language" signal.

### 1.5 Multi-term query semantics surprise users — Confirmed, High (UX)

`query_syntax.rs` documents "several terms must all appear in a file, and
lines matching any of them are returned". `query.rs:1170-1190` then emits
lines in document order, reported by the first term that matched, capped at
`MAX_MATCHES_PER_DOC = 100` per file and by the global match budget. There is
no boost for lines matching every term.

Live: `fn main` returns `fn range(&self)`, `fn apply_fixes<'a>(` and similar
as the top hits; `impl SearchEngine` returns `impl QueryRun {` first. The
quoted form `"fn main"` works, but nothing in the UI suggests it.

Fix (either): default to phrase semantics for plain queries and make AND an
explicit operator (`and:` or `+term`), or keep AND but score lines by the number
of distinct terms they contain and emit those first, highlighting every term.

### 1.6 Query syntax has no escape — Confirmed, Medium

`query_syntax.rs:53-87`: `->`, `-Wall`, `-1.5`, `file:`, `file:///` and even
`"file:"` are consumed as operators and return 0 results with no error;
`-> fn` silently excludes every file containing `>`. Only treat `-` as negation
when followed by `[A-Za-z0-9_"]`, keep quoted tokens literal, and treat
`file:`/`lang:` with an empty argument as a plain term.

### 1.7 `file:src/` never matches — Confirmed, Medium

`query_syntax.rs:121-134`: a trailing slash becomes the glob `**/src/`, which
only matches paths *ending* in `src/`. `fn file:src/` returns 0 candidates;
`fn file:src` works. The unit test `operators_are_parsed` asserts the broken
glob. Strip the trailing slash and expand to `**/src/**`.

### 1.8 `case=false` ignored in regex mode — Confirmed, Low

`src/web/api.rs:426-468`: `FAST_CODE_SEARCH&regex=true&case=false` returns 6
matches instead of 471. Prepend `(?i)` when case-insensitivity is explicit, or
document the asymmetry.

### 1.9 Regex with `\s+` is an order of magnitude slower than expected — Observed, Medium

With identical candidate sets (4,225 files): `main\(` regex runs in 43 ms,
`fn\s+main\(` in 350–700 ms. The regex crate falls back to a slow prefix
scan when the required literal is short and common. Worth profiling
`search_in_document_regex`: matching per line instead of per file, and
regex compilation caching, are the usual suspects.

### 1.10 Smaller engine defects

- **Needle with `\n`** (`text.rs:387`, `query.rs:1180`): matches across lines,
  and `match_end` points past the line. Confirmed, Low.
- **Re-indexing an existing path unions trigrams** (`engine/mod.rs:764`):
  `index_batch` on a known path adds onto the old posting lists without
  stripping them; stale trigrams keep the file as a false candidate. Happens
  when the watcher beats discovery during the initial build. Confirmed, Low.
- **Suffix path match is not component-aligned** (`lazy_file_store.rs:552`):
  `/api/file?file=e/mod.rs` resolves to `.../message/mod.rs`. Confirmed, Low.
- **Regex path drops hits when `symbol_cache` lacks the slot** (`query.rs:866`,
  `?` vs the text path's `unwrap_or`). Plausible, Low.
- **Interned reference names are never freed** (`engine/mod.rs:1329`).
  Code evidence, Low.

## 2. Indexing lifecycle

### 2.1 Concurrent saves delete the index — Confirmed, Critical

`src/index/persistence.rs:267` uses one shared tmp name (`<index>.bin.tmp`)
and `:350-358` falls back to `remove_file(path)` then rename when the first
rename fails. With two writers: B's `File::create` truncates A's in-flight tmp,
A renames the truncated file into place, B writes into that inode, B's rename
fails (tmp gone), the fallback deletes the real index, and the second rename
fails too. A scratch test lost the file in 6 of 6 rounds.

This is reachable with the default config: on Ctrl+C the watcher thread's
`save_after_watcher_shutdown` (`main.rs:272-276`) and the indexer's final
`save_index_if_needed` (`background_indexer.rs:303-311`) run on two threads
holding only read locks. Checkpoint saves versus `save_on_watcher_update`, and
two processes sharing `index_path`, are the same race. The advisory file locks
protect nothing because the writer locks the tmp inode while readers lock the
target.

Fix: serialize saves behind a process-wide mutex in `save_index_if_needed`;
use a unique tmp (`NamedTempFile::new_in(parent)` + `persist`); only take the
remove-then-rename fallback when the tmp still exists.

### 2.2 Reload ignores config changes — Confirmed, High

`persist.rs:542-560` computes `config_compatible` and only logs it;
`batch_check_files` classifies files by stat only. A file unchanged on disk but
now excluded by `exclude_patterns`, `include_extensions`, `max_file_size` or a
new `.gitignore` rule stays indexed indefinitely. Run each Valid file through
`is_eligible_file` after the stat pass and reclassify failures as Removed.

### 2.3 Modify bursts scan every posting list per file — Confirmed, High

`apply_changes` batches deletes into one `remove_files_by_ids` pass but each
Modify calls `update_file`, which runs `remove_document(id)` as a parallel
scan of every trigram, under the write lock. Measured in release: 300
modifies = 299 ms versus 300 deletes = 27 ms on a 1.8k-trigram index; at 9 ms
per file on a 3k-file/238k-trigram index a 5,000-file branch switch holds the
lock about 45 s, during which every search returns 503. Strip all modified ids
in one `remove_documents` pass, parse files outside the lock, then merge.

### 2.4 Discovery and the watcher descend into excluded trees — Confirmed, Medium

`file_discovery.rs:244-283` applies excludes only to files, so `.git/objects`,
`target/` and `node_modules/` are fully walked and stat'ed on every startup and
directory event (20k excluded entries cost 118 ms versus 1 ms). `watcher.rs:342`
watches the root recursively, installing an inotify watch on every excluded
subdirectory, which is what exhausts `max_user_watches`. Prune directories with
`filter_entry`, and add non-recursive watches per kept directory. The watcher's
`mpsc` channel is unbounded and fills while the index-load write lock is held.

### 2.5 gitignore disagreement outside git repos — Confirmed, Medium

Discovery uses `ignore::WalkBuilder` with `require_git = true`, so `.gitignore`
is ignored in a non-git tree; `is_gitignored` honours it anywhere. Editing a
gitignored file in such a tree removes it from the index. Set
`require_git(false)` or make both paths agree.

### 2.6 Root prefix match has no separator check — Confirmed, Medium

`persist.rs:343-355` picks the first root whose string is a prefix of the file
path: `/x/proj` and `/x/proj2` collide, and dropping `proj` from the config
reports every `proj2` file removed. Use `Path::starts_with` on canonical paths
and prefer the longest root.

### 2.7 An unreachable root is persisted as "all files deleted" — Confirmed, Medium

`persistence.rs:655-656` maps every `metadata()` error to Removed and the
resulting `removed_files_count > 0` forces a save. Booting before a network
mount is up wipes that root from the checkpoint. Only treat ENOENT/ENOTDIR as
removed, and keep entries for a root that is itself missing.

### 2.8 Config edge cases — Confirmed, Low to Medium

- `max_file_size = 0` means "default" everywhere except initial discovery,
  which then indexes nothing (`background_indexer.rs:603,636`).
- No tilde expansion: `~/...` paths warn or fail validation.
- Relative `index_path`/`paths`/`static_dir` resolve against the CWD, not the
  config file; `FCS_CONFIG` pointing at a missing file silently uses defaults.
- `include_extensions = [".rs"]` matches nothing.
- `validate()` requires the index directory to exist while `save()` would
  create it.
- `save_after_updates` counts watcher batches, not files; checkpoints are
  written even with `save_after_build = false` while the final save is
  skipped, so the on-disk index permanently lacks the tail.
- A second Ctrl+C during a slow shutdown save is swallowed.

## 3. REST and gRPC

### 3.1 Unclamped `offset` removes the match budget — Confirmed, High

`engine/mod.rs:119-125` via `api.rs:375` and `service.rs:121`: `with_offset`
sets `match_budget = (offset + max) * 8`. `q=\s&regex=true&rank=full&offset=99999999`
returns 200 after 9–15 s with 3.3 M `total_matches`, an empty page, and +600 MB
RSS. Clamp `offset` (400 beyond, say, 10,000) and give `match_budget` an
absolute ceiling. `offset + max_results` also wraps on release builds and panics
under overflow checks (`offset=18446744073709551615`).

### 3.2 Timeouts drop the response, not the work — Confirmed, High

`web/mod.rs:160-163`: `TimeoutLayer` drops the handler future, which releases
the semaphore permit, but the `spawn_blocking` search continues holding the
read lock, and `metrics.record_search` never runs. Three concurrent 30 s
searches produced three `408` responses with empty bodies while CPU kept
rising and RSS peaked at 3.0 GB. Always set an engine deadline from the
request timeout, acquire the permit inside the blocking closure, and return a
JSON 503/504 with `Retry-After` instead of an empty 408.

### 3.3 gRPC has no shared concurrency limit — Plausible, Medium

`service.rs:143-207`: `concurrency_limit_per_connection` is per connection, so
N connections × 64 searches, no default deadline, same offset hole. Share the
REST semaphore. The `Index` RPC with empty `paths` still takes the write lock
for `resolve_imports` + `finalize`, has no guard against concurrent calls, and
appends to `root_paths` each time.

### 3.4 Path resolution is too lenient — Confirmed, Medium

`find_file_id` falls back to a suffix match: `/api/file?file=` returns an
arbitrary file, `?file=.rs` a random Rust file. Reject empty `file` with 400
and require component-aligned, unique matches.

### 3.5 Invalid regex is a 400 only sometimes — Confirmed, Low

`[unclosed&regex=true` is 400; the same with `symbols=true` or
`references=true` is 200 with zero results, because `references` and `symbols`
silently win over `regex`. The 400 message is also doubled:
`Invalid regex pattern: Invalid regex pattern: [unclosed` (`regex_search.rs:92`
and `api.rs:466` both prefix). Reject mutually exclusive mode flags.

### 3.6 Other API findings

- `/api/diagnostics` self-tests check whether a sampled file appears in the
  top-N results, so a healthy index reports `degraded` (live: 2 of 5 fail).
  Confirmed, Medium.
- `/api/context` and diagnostics return absolute host paths; search and
  `/api/file` return display paths. Confirmed, Medium.
- `/api/stats.total_size` is `file_store.total_mapped_size()`, the bytes
  currently mmapped by lazy loads. It grew from 5.7 MB to 16.4 MB over 15
  minutes of searching and is labelled "INDEX" in the header and "TOTAL SIZE"
  on the diagnostics page. Confirmed, Low.
- REST `max=0` gives 1 result, gRPC `max_results=0` gives 50. Confirmed, Low.
- Histogram buckets use truncated integer milliseconds; rejected and
  timed-out searches are not counted. Confirmed, Low.
- No `X-Content-Type-Options`, CSP or `X-Frame-Options`; HTML and JS are
  cached for an hour under unversioned URLs, so upgrades can mix old and new
  bundles. Confirmed, Low.
- `/api/ready` returns 503 during `Reconciling` even though searches are
  served, and reports zero files whenever the write lock is briefly held.
  Plausible, Low.
- `/ws/progress` has no connection cap and accepts 64 MiB inbound frames it
  never reads. Plausible, Low.

## 4. Web UI

### 4.1 Hands-on observations (live at 1440×900, 375×812, light and dark)

- **Search modes leak between visits and override shared links.** Toggles
  persist in `localStorage` and URL params only ever turn a mode *on*
  (`keyword.js:139-140`). Opening `/?q=[unclosed&regex=true` ran with
  `symbols=true` from a previous session; `/?q=the&regex=false` ran with
  `regex=true`. Confirmed.
- **Toggle visuals desync from state.** When a mode is set from the URL the
  checkbox is checked but the label keeps its inactive style, so REGEX looked
  off while a regex ran and SYMBOLS looked on while off. Confirmed.
- **`?max=1000` breaks the page.** The select tops out at 250 while the API
  and docs allow 1,000; an unknown value yields `max=NaN` and every search
  returns a red 400. Confirmed.
- **Header compact search overlaps the stats.** The `nav-query` box
  (x 571–831) sits on top of `FILES:` / `CONTENT:` (x 705–880) at 1440 px and
  hides them entirely at 800 px. Confirmed.
- **Hover preview tooltip is sticky.** It only hides on mouseleave or when a
  modal opens (`keyword.js:1023,1403`); it stayed on screen across Escape and
  a viewport change and covered the mobile layout. Confirmed.
- **No paging.** `offset`, `total_matches` and `truncated_by_budget` are
  unused; the only affordance is "50+ RESULTS (raise MAX RESULTS)", which is
  wrong advice when `has_more` comes from budget truncation. Confirmed.
- **No result diversity.** With `max=250`, one file contributed 48 of the
  hits. Confirmed.
- **Rendering cost.** 250 hits with 3 context lines produced 12,842 DOM nodes
  and a 79,000 px page; a forced reflow took about 350 ms. Confirmed.
- **No dark theme** (single light theme by design, `index.html:10-13`), and
  no browser history: searches use `replaceState`, so Back leaves the site.
  Confirmed.
- **Console noise:** every page load probes `127.0.0.1:8081` for the semantic
  server and logs a connection error. Confirmed.
- Ctrl/Cmd+K focus, `/` shortcut, `j`/`k` selection, Escape closing the file
  modal, mobile layout without horizontal scroll, and error rendering for a
  400 all work.

### 4.2 Code review

- **Stored XSS on diagnostics** — Confirmed, High. `diagnostics.html:498`
  interpolates `files_by_extension[].extension` into `innerHTML` unescaped,
  plus `version`, `uptime_human`, `generated_at` and `error.message`. A file
  named `x.<img src=x onerror=...>` in any indexed tree renders as markup once
  its "extension" reaches the top 10. Escape every server string with the
  quote-escaping `escapeHtml` from `common.js`.
- **Highlighting ignores server offsets** — Confirmed, High.
  `keyword.js:507-539` builds one case-insensitive literal from the whole
  query and never reads `match_start`/`match_end`. `parse lang:rust`,
  `fn\s+\w+` (regex), `case:yes Foo` and every multi-term query highlight
  nothing or the wrong thing. Wrap `content[match_start..match_end]` on the
  match line and tokenize like `query_syntax.rs` for context lines.
- **Sticky invisible filters** — Confirmed, Medium. Include/exclude globs are
  restored from `localStorage` on every visit but the panel only opens for URL
  params, so a glob typed weeks ago silently filters everything.
- **503 during indexing is a sticky error** — Confirmed, Medium. `api.rs:983`
  returns 503 + `Retry-After: 1` whenever the write lock is held;
  `keyword.js:826-830` shows `Error: Index is currently being updated` until
  the next keystroke. Retry once or twice under the same `AbortController`.
- **WebSocket reconnect gives up after ~3.5 minutes** — Confirmed, Medium.
  `common.js:478,514` caps at 10 attempts while the offline banner promises
  automatic reconnection.
- **File viewer highlights line by line with no size cap** — Confirmed,
  Medium. `keyword.js:562-582` calls `hljs.highlightElement` per line on the
  whole file; block comments are mis-coloured; the hover tooltip runs
  `highlightAuto` over every language per line.
- **Accessibility** — Confirmed, Medium. `j`/`k` selection is a visual
  outline on a `div` with no `tabindex`/`aria-selected`; modals have no
  `role="dialog"`, focus trap or focus restore; `#results` has no
  `aria-live`; icon buttons expose ligature text as their name; `#query` has
  no label; `text-outline` (#7a785f on #fefae4, 4.2:1 at 10 px) and line
  numbers (#9e9c80 on white, 2.8:1) are under AA.
- **Ignored response fields** — Confirmed, Low. `content_truncated` (lines
  cut at 500 bytes) is not marked; filename hits (`line_number: 0`) display as
  "line 0" and open the viewer at line 0; there is no REFERENCES toggle even
  though the API and docs support it.
- **docs.html drift** — Confirmed, Low. Claims regex/symbol queries omit
  ranking metadata (they don't); `offset`/`timeout_ms` rows have two of four
  cells; `case`/`word` params, `/api/ready` and `/metrics` are undocumented;
  `/api/file` is described as returning `path` but the field is `file`; the
  dependency boost is documented as `1 + log10` but is `1 + 0.5·log10`; the
  UI's 3-character minimum is documented nowhere and not enforced server-side.
- **Readiness manager targets selectors the page never renders**
  (`common.js:376` `.result-item` versus `.result-group`), so a not-ready
  transition wipes results. Confirmed, Low.
- **Dead and duplicated code**: `loadStateFromUrl`/`syncUrlFromState` in
  `common.js` are shadowed by same-named functions in `keyword.js`;
  `formatElapsed`, `highlightMatches`, `showEmpty`, `StatusPoller` and
  `highlight-dark.min.css` are unused; `.refresh-btn` is defined twice; the
  3.9 MB unsubsetted icon font ships `font-display: block`.

## 5. Strengths worth keeping

- Save is atomic for a single writer, load validates magic, versions, section
  bounds, CRC and directory ordering before decoding, and a golden fixture
  pins the format.
- Read-not-mmap for small files removes the SIGBUS-on-truncate hazard; regex
  size and DFA limits reject pathological patterns with a 400 in milliseconds;
  tree-sitter runs under a timeout and `catch_unwind`.
- Match budget, deadline, deterministic tie order and stable offset paging;
  byte and character offsets into the full line; CRLF handling.
- `try_read` everywhere with 503 + `Retry-After`, poison recovery, body
  limits, loopback default bind, CORS off by default, compile-time ETags
  with working 304s, and file endpoints that only resolve indexed files.
- The keyword page escapes every server string it renders; handlers use
  `data-*` attributes and `addEventListener`; searches, hover previews and
  dependency popovers all use `AbortController`.

## 6. Test coverage gaps

- No regex property test (candidates ⊇ regex matches) and no repetition or
  `(?i)` k/s cases.
- No test for content changed on disk after indexing (stale symbol and
  reference positions).
- `update_file`/`remove_file` are never checked for dependency-edge
  retention, parked-import growth, or tombstone and mmap release.
- No concurrent-save, two-process, or `run_background_indexer` end-to-end
  test (checkpoint, shutdown mid-build, resume).
- No reload test that a changed exclude, extension, size or gitignore rule
  removes files.
- No `offset`/match-budget bound, default-deadline, or timeout-body test; no
  `/api/file` tests; gRPC has no invalid-regex, negative-value, mode-conflict
  or empty-`Index` tests.
- No JavaScript tests, no browser tests, no CI step that rebuilds
  `static/tailwind.css` (a committed artifact), and no contract test pinning
  the JSON field names the UI consumes.

## 7. Suggested order of work

1. **Stop data loss and runaway requests** (days): serialize saves with a
   unique tmp file (2.1); clamp `offset` and cap the match budget (3.1);
   default engine deadline from the HTTP timeout and JSON timeout responses
   (3.2); share the semaphore with gRPC (3.3).
2. **Correctness of search results** (days): repetition constraints and
   case-fold classes in regex analysis (1.1, 1.2); char-boundary clamping in
   references (1.3); dependency edges on `update_file` (item 4); query-syntax
   escapes and `file:dir/` (1.6, 1.7); choose and implement multi-term
   semantics (1.5).
3. **Availability during change** (a week): batch modify removals outside the
   lock (2.3); prune excluded directories from discovery and the watcher
   (2.4); apply eligibility on reload (2.2); ENOENT-only removal and root
   prefix fix (2.6, 2.7).
4. **UI contract** (a week): escape diagnostics output (4.2); highlight from
   server offsets; honour and clear URL mode params, validate `max`; paging
   with `offset`; retry on 503; fix the header overlap and tooltip dismissal;
   dialog semantics and focus management; correct docs.html.
5. **Hygiene**: config path handling (2.8), metrics buckets, security and
   cache headers, dead code removal, a JS test harness and a Tailwind build
   check in CI.
