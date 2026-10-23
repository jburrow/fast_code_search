# How fcs is tested

fcs makes a few promises that a wrong answer breaks quietly: a search shows
every match, in rank order, and finds what ripgrep finds; the index follows
the disk through edits, branch switches, lost events and restarts; searches
are answered while the index changes; the import graph, symbols and
references are what [graph support](../reference/graph-support.md) and
[reference support](../reference/reference-support.md) say. A missing hit
looks like an empty result, not an error, so each promise has a test whose
answer comes from somewhere other than fcs's own code: ripgrep, a
language's own resolver, a deliberately naive model, or a generator that
wrote every line and knows the answer.

## The suites

| Suite | Checks | Against | Run |
|---|---|---|---|
| Unit tests (`src/**`) | each part on its own | hand-written cases | `cargo test --lib` |
| `tests/integration_tests.rs` | the server end to end, saving and loading | hand-written cases | `cargo test` |
| `tests/frontend_parity.rs` | web API, gRPC and `fcs` give the same hits in the same order | each other | `cargo test` |
| `tests/watch_harness.rs` | the watch loop over scripted and random edits | a fresh index of the same disk | `cargo test` |
| `tests/lifecycle_model.rs` | random sequences of edits, renames, folder moves and deletes, ignore files, binary and oversized files, lost events, saves, restarts and crashes; after every step: the file set, every word's hits read page by page (no repeats, each file's hits together), the index's self-checks | a naive model (a map of files, a hand-written ignore matcher, a line-by-line search) | `cargo test --test lifecycle_model` |
| `tests/search_properties.rs` | literal (`-i`, `-s`, `-w`) and regex searches on random text with case-folding oddities and multi-byte characters | `rg` | `cargo test --test search_properties` (needs `rg`) |
| `tests/python_codebase.rs` | the import graph, symbol search, reference search and strict `-deps:`/`-rdeps:` scopes on generated Python trees, after a build and after the watcher rewrites the tree | the generator's answer key, itself checked by Python and grimp | `cargo test --test python_codebase` |
| `tests/contract_walk.rs` | one search walked to its last hit through every surface: REST at several page sizes and random offsets, gRPC, MCP over HTTP and `fcs mcp`, `fcs` (piped, `-l`, offline) and the web page in headless Chromium with LOAD MORE and "Show N more hits" pressed until they are gone; no hit twice or missing, each file's hits together, files by best hit, honest counts | the engine's exhaustive list, itself checked against the files on disk and the generator's reference key | `cargo test --test contract_walk` (the web page needs Playwright, see below) |
| `tests/rank_snapshot.rs` | the first 20 hits and the total of about a hundred searches; the same corpus at two places on disk must rank the same | `tests/snapshots/rank_order.txt`, changed only on purpose | `cargo test --test rank_snapshot` |
| `tests/discovery_oracle.rs` | which files are indexed (the build and the watcher's per-file check) on random trees of `.gitignore`, `.ignore`, `.rgignore` and `.git/info/exclude` rules; the FILES box with a regex | `rg --files`, and `rg` over the path list | `cargo test --test discovery_oracle` (needs `rg`) |
| `tests/lock_sim.rs` | the engine lock and the watcher's install steps under minutes of made-up load (back-to-back half-second searches, one 20 s search among quick ones, a busy server during a branch switch): every search answered, no search waits past its hold-back plus one step, backlogs drain at half speed or better, a slow search delays only the writer | bounds derived from the protocol, on a simulated clock (`fast_code_search::sim`): the real lock code, the same schedule on every run and OS, four minutes in about a second | `cargo test --test lock_sim` (`-- --ignored --nocapture` prints the tuning table) |
| `tests/fault_injection.rs` | failed reads (permission, out of file handles, a file gone between listing and reading), a slow disk, panics while reading, parsing or applying changes, lost changes and lost events, a save onto a full disk or a failed rename: each is retried, reported or repaired, searches are not held up, and the saved index stays whole | expected recovery | `cargo test --features failpoints --test fault_injection` (fault points are listed in `src/faults.rs`) |
| `examples/rg_backtest.rs` | fixed and generated queries and regexes on tokio, Django and fixtures, with paging and default limits, on an index built in one go, restarted from disk (`--history reload`) and filled by the watcher (`--history watch`) | `rg` | see [Correctness against ripgrep](../benchmarks/correctness.md) |
| `examples/graph_oracle.rs` | the import graph on Django, Vite, generated deep trees and generated monorepo layouts (projects side by side with their own `src/`, a root folder that is a package, a split namespace package, npm workspaces with tsconfig `paths` and `extends`); every edge missed or wrong must be a known gap listed with its reason and count | grimp, TypeScript's resolver | CI job "graph oracle", gaps in `benches/graph/known_gaps.toml` |
| `examples/ref_oracle.rs`, `examples/impact_oracle.rs` | Python references and diff impact on real repositories | Jedi | CI jobs "ref oracle", "impact oracle" |
| `examples/scoreboard.rs` | the server binary over HTTP in seven scenarios (cold build, warm restart, branch switch, ten-minute stream, slow mix, saves under load, event-queue overflow): searches turned away, latency, time until the index matches the disk, memory, CPU, diagnostic captures, crashes, `/api/check` | targets in `benches/scoreboard/targets.toml` | nightly and on pull requests that touch locking or watching (workflow "Scale scoreboard"), see [Scale scoreboard](../benchmarks/scoreboard.md) |
| `examples/chaos.rs` | `kill -9` of the server binary at random moments during a build, a load, a stream of edits, a periodic save, a burst and the shutdown save; after each, a restart must load the saved index (never reject it), leave no temp file, catch up with every change and pass `/api/check` | the files on disk | nightly and on pull requests that touch saving or watching (workflow "Scale scoreboard", job "Kill and restart"), see [below](#killed-at-any-moment) |
| `examples/watch_load.rs`, `examples/limits_bench.rs` | searches answered and latency while a large tree changes; where limits bite | measured numbers | by hand, see [Incremental updates](incremental.md#how-this-is-tested) |

Every test that drives the index also runs its self-checks
(`check_invariants`: posting lists name only live files, path lookups and
the folder index agree, caches hold nothing for removed files, the graph's
two directions agree), the same checks `fcs check --live` and the server's
background scrub run (see [Checking a live index](../guides/checking-the-index.md)).

## The generated Python codebases

`tests/support/pygen.rs` writes packages nested up to four deep, each with
an `__init__.py` and modules, from a seed. Modules import each other in
every form graph support lists (absolute, aliased, several on one line,
parenthesised lists, one to three dots, imports inside functions and under
`if TYPE_CHECKING:`; cycles happen) and use what they import in every
position reference support lists as found: calls, attribute calls,
`await`, instantiation, `obj.m()`, `Cls().m()`, `Cls.m(x)`, `super().m()`,
decorators, base classes, annotations (`list[C]`, `C | None`, `Optional[C]`),
`raise`/`except`, and values in lists, dicts, keyword arguments and
comparisons. Names also appear where they are not references (strings,
comments, docstrings, string annotations). Every name is unique, so "the
references of `fn_0042`" has one right answer, which the generator recorded
as it wrote the line.

Before the key is trusted, Python compiles every file and grimp must draw
exactly the key's edges, line for line. Then the test rewrites the tree as
another seed's and hands the changes to the watcher in a few bursts: the
incremental result must match the new key as a fresh build does.

## Rank order

Everything above checks *which* hits a search finds and that a list is
well formed; none notices when a change quietly reorders the first page,
the page people read. `tests/rank_snapshot.rs` pins it: the first 20 hits
of about a hundred searches (words, generated names, regexes, symbols,
references) are kept in `tests/snapshots/rank_order.txt`. A ranking change
is meant to move hits, so a difference is not a failure to fix but a
change to accept on purpose:

```text
FCS_UPDATE_SNAPSHOTS=1 cargo test --test rank_snapshot
```

and the snapshot's diff then shows in the pull request what moved.

## The web page

The contract walk's browser leg runs `scripts/e2e/web_walk.cjs` with
Playwright. Locally:

```text
npm install --no-save --prefix /tmp/pw playwright@1.56.1
/tmp/pw/node_modules/.bin/playwright install chromium
NODE_PATH=/tmp/pw/node_modules FCS_REQUIRE_BROWSER=1 cargo test --release --test contract_walk
```

Without Playwright the leg prints a note and is skipped; CI's Linux test
job sets `FCS_REQUIRE_BROWSER`, so there it fails instead.

## Killed at any moment

`examples/chaos.rs` starts the server binary on a generated tree of 20,000
files and kills it with SIGKILL, round after round, at a random moment of
one of these:

| Kind | Killed |
|---|---|
| `build` | during a build from scratch, which saves a checkpoint every quarter of the tree |
| `load` | while it loads the saved index and checks the tree |
| `stream` | while 300 files a second are rewritten, created and removed, and saved every 300 updates; the edits go on for a second after the kill |
| `save` | 0 to 20 ms after one of those saves begins (it watches for the save's temp file every millisecond) |
| `burst` | while the watcher installs a tenth of the files rewritten at once |
| `shutdown` | 0 to 20 ms after the save at shutdown (SIGTERM) begins |

After each kill it starts the server again and requires: the saved index
is loaded, or there was none (a kill must never leave one the next start
rejects); no temp file of a killed save is left once the server answers;
the server becomes ready and catches up (nothing pending, the changed
files indexed as they are on disk); `/api/check?all=true` finds nothing;
and it does not crash. The seed is printed, and `--seed N` replays the
same kill moments; a failing round keeps its server log as
`failed-<round>.log`.

60 rounds on a 4 vCPU VM, 2026-10-23 (seed 4):

| Killed during | Rounds | Killed mid save | No index after | Index rejected | Temp files left | Slowest ready | Slowest catch-up | Check problems |
|---|---|---|---|---|---|---|---|---|
| build | 10 | 1 | 3 | 0 | 0 | 1.8 s | 1.8 s | 0 |
| load | 10 | 0 | 0 | 0 | 0 | 0.3 s | 0.5 s | 0 |
| stream | 10 | 1 | 0 | 0 | 0 | 0.4 s | 0.8 s | 0 |
| save | 10 | 10 | 0 | 0 | 0 | 0.3 s | 0.8 s | 0 |
| burst | 10 | 0 | 0 | 0 | 0 | 0.3 s | 0.8 s | 0 |
| shutdown | 10 | 10 | 0 | 0 | 0 | 0.4 s | 0.8 s | 0 |

"No index after" is a build killed before its first checkpoint: the next
start builds from scratch. A build killed later loads the checkpoint and
indexes the rest. CI runs 40 rounds on pull requests (seed 1) and 100
nightly (a new seed).

This tests a killed process, not a power cut: the kernel still writes out
what the process wrote. A save fsyncs its file before the rename and, on
Linux and macOS, the folder after it, so a power cut should leave either
the old index or the new one; nothing here simulates that.

## Reproducing a failure

- proptest suites print the shrunk sequence or query that failed, which is
  usually enough to write a named test (the lifecycle model keeps the ones
  it found that way). `PROPTEST_RNG_SEED=N` replays a run and
  `PROPTEST_CASES=N` runs more: `PROPTEST_CASES=3000 cargo test --release
  --test lifecycle_model`.
- `FCS_PYGEN_SEEDS=1,2,3` picks the generated trees (`FCS_PYGEN_PACKAGES`
  their size) and `FCS_PYGEN_KEEP=DIR` writes them to `DIR` to look at.
- `FCS_WATCH_SEED=N` replays a random watch-harness run.

On pull requests CI uses fixed seeds, so a pull request is not failed by a
case it did not cause; pushes to `main` use new seeds each time and keep
exploring. CI runs the lifecycle model 600 times, the search properties
2,000 times and the discovery oracle 1,000 times (job "fcs vs ripgrep",
which also runs the backtest on a reloaded and a watched index) and 40
generated Python trees with grimp (job "graph oracle").

## What these tests found

- Files with no trigram (two bytes in `x.py`) were never searched by short
  queries (self-checks).
- `extra_binary_extensions` was documented but rejected (self-checks).
- The watcher indexed folders ignored by a parent's `.gitignore` when
  something happened in them, and a restart re-indexed changed files that
  were now ignored (lifecycle model).
- A file rewritten in the same timestamp tick with the same size could
  stay stale across a restart on coarse file systems (found while writing
  the lifecycle model; now covered by a unit test).
- The watcher's per-file ignore check disagreed with the build and with
  ripgrep in three ways: a rule deeper down (`!a/b/` after `/a`) brought
  files back from an ignored folder; the nearest ignore file won even when
  a parent's `.ignore` should beat a `.gitignore` next to the file; and
  rules above a nested repository were dropped inside it (discovery
  oracle).
- Searches held back a fixed 250 ms for a waiting install step, too short
  for half-second searches: the watcher drained a backlog at 8% of its
  speed, not the "half speed or better" the docs claimed (lock
  simulation; see [Incremental updates](incremental.md#holding-back-as-long-as-a-search-runs)).
- After every restart from a saved index, every search waited 4 to 5 s
  while the import graph's parked imports were retried in one lock hold;
  a `backlog` diagnostic fired under steady edits the watcher kept up with
  (scale scoreboard).
- A save killed half way left a temp file as large as the index, for
  good; a save that failed (disk full) was forgotten, so the shutdown
  skipped its save; a failed rename deleted the index before trying
  again; and a panic while reading a file during the build left it out
  without a word (fault points and the kill-and-restart run).
- Earlier probes found a crash when a large file was rewritten during a
  search, "server unavailable" at the end of a build, and lost changes
  after failed reads or a panic; the fault and watch tests cover them.
