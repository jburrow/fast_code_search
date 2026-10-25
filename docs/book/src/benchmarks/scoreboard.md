# Scale scoreboard

The other benchmarks time the engine. This one runs the real server
binary, starts and stops it, searches it over HTTP from several threads the
way people do, and changes the files under it the way branch switches,
formatters and builds do. Every scenario reports the same numbers, and a
number over its target fails the run.

```sh
cargo build --release --bins --example scoreboard
target/release/examples/scoreboard --dir /tmp/fcs-score --files 200000 \
    --targets benches/scoreboard/targets.toml --markdown score.md --json score.json
```

`--scenarios branch-switch,overflow` runs some of them, `--stream-seconds`
sets the stream's length (600 by default). The tree is generated under
`--dir` (small Python files, 1,000 to a folder; `examples/support/load_tree.rs`)
and reused by later runs.

The report holds numbers only: no paths, queries or file contents. That
makes a run on a private tree safe to share. The server's log and any
diagnostic captures stay in `--dir`.

## Scenarios

Each scenario starts the server on its own: from scratch for `cold-build`,
from the saved index for the rest. Searches start at once and go on until
the end. The changes start once the restart's own check of the tree has
finished (see [Known gaps](#known-gaps)).

| Scenario | What it does |
|---|---|
| `cold-build` | Start with no saved index; search while it builds. |
| `warm-restart` | Start from the saved index; search while it loads and checks the tree. |
| `branch-switch` | Rewrite one file in five at once (40,000 at 200,000 files). |
| `stream` | Change 200 files a second for ten minutes: rewrites, plus a new file and a removed one every 100 ms. |
| `slow-mix` | Eight searchers, plus a regex that reads every file every 3 s, while 50 files a second change for two minutes. |
| `save-under-load` | 500 files a second for two minutes, with the index saved every 5,000 applied changes. |
| `overflow` | Rewrite half the files at once: more events than the watcher's queue holds (65,536), so it rescans the tree. |

The searchers cycle through cheap and expensive queries (`task_17_`,
`return value`, `revision`, `default_\w+\(\)`, `task_\d+_amber`,
`helpers.default_cobalt`) with a page of 50, as the web UI asks.

## What is reported

| Number | Meaning | Target |
|---|---|---|
| `turned_away` | Searches answered 503 ("index is being updated") | 0 |
| `refused`, `errors` | No connection after the server first answered; any other failure | 0 |
| `incomplete` | Answered while the index was still being built or checked (the answer says so) | reported |
| `p50_ms`, `p99_ms`, `max_ms` | Search time as the client sees it, HTTP included | per scenario |
| `listen_secs`, `ready_secs` | From start until the server answers anything, and until `/api/ready` | per scenario |
| `fresh_secs` | From the last change until the index matches the disk: nothing pending, and 100 of the changed files checked against the disk with `/api/status?paths=` | per scenario |
| `not_settled` | The index never matched the disk within `--settle-seconds` | 0 |
| `max_pending` | The largest backlog the server reported (sampled every second; the JSON has the curve) | reported |
| `rescans` | "The watched paths will be rescanned" in the server log | reported |
| `peak_rss_mb`, `cpu_secs` | The server process's peak memory and CPU time (Linux) | memory per scenario |
| `memory_mb` | The index's [estimated heap by structure](../how-it-works/memory.md#where-it-goes) at the end of the scenario, largest first, so a memory change names the part that grew | reported |
| `captures`, `stall_captures` | [Diagnostic captures](../guides/diagnostic-captures.md) written; the second leaves out `slow_search`, which an overloaded CPU alone can cause | stalls 0 |
| `crashes` | The server exited before it was told to stop | 0 |
| `check_problems` | What `GET /api/check?all=true` finds once the index has settled: invariants, every file's content, the file set against a fresh walk | 0 |

The zero targets hold at any size. The others are in
`benches/scoreboard/targets.toml`, set at about twice the worse of two runs
(memory: about 30% over), and apply only to runs with the same number of
files.

## Measured

200,000 files on a 4 vCPU, 16 GB Linux VM, 2026-10-23. These are the
second of the two runs behind the targets.

| Scenario | Searches | Incomplete | p50 | p99 | Max | Ready | Fresh after | Max pending | Peak RSS | Stall captures |
|---|---|---|---|---|---|---|---|---|---|---|
| cold-build | 489 | 489 | 169 ms | 638 ms | 939 ms | 49.9 s | – | – | 808 MB | 0 |
| warm-restart | 70 | 63 | 59 ms | 761 ms | 828 ms | 2.3 s | – | – | 671 MB | 0 |
| branch-switch | 203 | 62 | 376 ms | 1.24 s | 1.65 s | – | 7.1 s | 48,248 | 789 MB | 0 |
| stream | 6,316 | 116 | 403 ms | 859 ms | 1.29 s | – | 1.1 s | 1,333 | 883 MB | 0 |
| slow-mix | 2,280 | 976 | 633 ms | 2.25 s | 5.79 s | – | 2.7 s | 523 | 825 MB | 0 |
| save-under-load | 1,344 | 95 | 423 ms | 937 ms | 1.17 s | – | 4.0 s | 5,198 | 1,073 MB | 0 |
| overflow | 536 | 201 | 414 ms | 1.27 s | 1.53 s | – | 32.7 s | 180,708 | 1,335 MB | 0 |

In every scenario: no search turned away, refused or failed, no crash,
and the check found nothing. The server answered within 0.05 s of starting
in both start-up scenarios. `slow-mix` wrote one `slow_search` capture
(5.8 s), with nine searches sharing four CPUs. The stream kept 400 to
1,300 changes pending, a few seconds' work, for ten minutes.

## What it found

- **A 4–5 s stall after every restart.** An index loaded from disk keeps
  its unresolved imports but not the files' import lists. At the end of
  the restart's check, each of those imports got one last try, all at once
  and under the write lock: 368,000 imports at 200,000 files, 4 to 5 s
  during which every search waited. They are now tried in the same short
  steps as the rest of the import graph (about 250 ms each), and the
  slowest search after a restart fell from 4.0 s to under 1 s.
- **A `backlog` capture under a steady stream.** The capture fired when
  changes had been pending for 300 s without a break, which a steady
  stream of edits always causes, however well the watcher keeps up. It now
  fires only when the watcher could not have applied what is pending in
  the last 10 s.
- **`lock_wait` counted the holding back.** A search holds back for a
  waiting install step by design (up to 2 s, see
  [Incremental updates](../how-it-works/incremental.md#holding-back-as-long-as-a-search-runs)),
  and that alone could pass the 2 s threshold. The capture now counts only
  the time a writer kept the search out.

## Known gaps

- **After a restart, answers say the index is incomplete for a while.**
  Loading takes 2 to 3 s at 200,000 files. After that the server walks
  the tree to find files that changed while it was down, which took
  between 4 and 30 s here depending on the search load. Until the walk
  ends, every answer carries the `indexing` note, although only files
  changed since the last run can be missing (`incomplete` in
  `warm-restart`).
- **Changes during the restart's check.** When a large change lands while
  the restart's own check still runs, the check's steps and the watcher's
  steps take turns. A search can then wait for one of each: 2.4 s was seen
  once, with 100,000 files rewritten 3 s after a restart. The scenarios
  wait for the check to finish before they change files, so this case is
  not covered by a target yet.
- **Lost events under a stream.** One of three ten-minute streams lost
  events once, about 280 s in: the kernel's inotify queue
  (`fs.inotify.max_queued_events`, 16,384 by default) overflowed. The
  watcher rescanned the tree and the index matched the disk again within
  about 15 s, with nothing missing. The cause of the sudden burst is not
  known yet. `rescans` is reported, not a target.
- **Only Linux reports memory and CPU.** On other systems those columns
  are empty.

## Where it runs

- **Nightly on GitHub** (workflow "Scale scoreboard"): 200,000 files with
  the full ten-minute stream, against the targets. The table goes to the
  job summary, and the JSON and server logs are kept as an artifact.
- **On pull requests** that touch locking, watching, saving, the web
  server or limits: the same, with a two-minute stream.
- **Before a release, on a large machine**: a million files (`--files
  1000000`), or your own tree with a sanitised report. The time and memory
  targets apply only to 200,000 files, so such a run checks the zero
  targets and records its numbers for comparison with the previous release.
