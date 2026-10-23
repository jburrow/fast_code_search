# Diagnostic captures

A server that is slow or stuck for a moment is hard to diagnose after the
fact: by the time anyone looks, the moment has passed. With captures on,
the server notices such moments itself and writes what it was doing to a
JSON file, to read later with `fcs diagnose` or to attach to a bug report.

Captures are off by default. Turn them on in the configuration:

```toml
[diagnostics]
capture = true
# dir = ""                  # default: a `diagnostics` folder next to index_path
```

The startup log says where captures go. Nothing leaves the machine.

## What sets a capture off

| Capture | When | Default |
|---|---|---|
| `slow_search` | A search took longer than this, from the request to the reply | `slow_search_secs = 5` |
| `lock_wait` | A search waited longer than this for the index lock (a watcher step or a save held it) | `lock_wait_secs = 2` |
| `watcher_stalled` | The watcher had changes to apply and made no progress (planned, parsed or installed nothing) for this long | `stall_secs = 120` |
| `indexer_stalled` | The index build reported no progress for this long | `stall_secs` |
| `search_stuck` | A search has been running for this long | `stall_secs` |
| `lock_held` | The index lock was never free for this long | `stall_secs` |
| `backlog` | The watcher has had changes to apply for this long without a break: it is falling behind, though still moving | `backlog_secs = 300` |

The last four are hangs. Their captures also say what every thread is doing
(see below).

A watchdog thread looks at the server once a second to notice the hangs
and the backlog. Searches register while they run, so a capture lists the
searches running at the time. While captures are off, none of this runs;
the hooks in the search path cost one atomic load.

### Why these thresholds

The defaults sit well above what a healthy server under heavy load does,
so a capture means something is wrong rather than merely busy.
`examples/watch_load.rs --capture-dir DIR` runs the real watcher and
searches with captures on at the defaults. It rewrites files while four
threads search with a mix of rare, common and regex queries, then reports
what was captured. On a healthy run, nothing should be:

| Tree | Change | Slowest search | Index matches disk | Captures |
|---|---|---|---|---|
| 200,000 files | 100,000 rewritten at once | 1.8 s | 1.4 s after the change stopped | none |
| 200,000 files | 50,000 rewritten in two halves | 1.6 s | 1.3 s after | none |
| 200,000 files | 200 files every 100 ms for 60 s (71,000) | 2.0 s | 9.7 s after | none |

(4 CPUs; 169 to 477 searches per run, none turned away, mean 0.6 to
0.7 s.) The slowest search was 2.0 s against a 5 s threshold, and the
lock waits are part of it.

Earlier runs set the other bounds:

- A search waits for an install step at most about a second plus its
  hold-back for a waiting writer: as long as recent searches took, 250 ms
  to 2 s (see [Incremental updates](../how-it-works/incremental.md)). So
  with quick searches, 2 s of waiting means a step held the lock longer
  than it is designed to; with searches of a second or more, look at
  their length first.
- The longest the watcher goes without counting progress while healthy is
  a full-tree rescan after its event queue overflowed: about 40 s walking
  a million files. `stall_secs = 120` leaves three times that.
- On a million-file tree, 200,000 rewritten files took 214 s to apply while
  searches ran. `backlog_secs = 300` captures a backlog that lasts longer
  than that.

## What a capture holds

Each capture is one file, `fcs-diag-<time>-<kind>.json`, with:

- the trigger: what happened, what was measured, and the threshold;
- for `slow_search`, the search: where it came from (REST/MCP or gRPC),
  its mode, and how long it spent waiting for the index lock, searching,
  and building the reply, plus how many candidate files it read and which
  limits it hit;
- the searches running at the time, how long each had been running and
  whether it held the lock;
- the latest sample of the index: file count, whether it is building or
  watching, changes waiting to be applied, the watcher's progress counter,
  and the build's progress;
- the last 20 watcher bursts: events, files indexed and removed, install
  steps, the longest lock hold, and how long each took;
- process figures: memory (RSS), thread count, CPUs, uptime;
- the last log lines (`log_lines = 300`);
- for hangs, every thread's name and state (running, sleeping, waiting on
  the disk) and, when `gdb` or `eu-stack` is installed, every thread's
  stack. Attaching takes a second or two, during which the server is
  paused; set `thread_stacks = false` to skip the stacks. Thread states
  and stacks are recorded on Linux only.

### Privacy

Your repository may be a work one, so captures leave out what was searched
and which files were involved. Query text becomes `<redacted>` (the length
is kept), and in log lines and build progress, quoted text other than
numbers, versions and sizes becomes `<text>`, anything that looks like a path or a file name becomes `<path>`,
and the values of fields such as `query=` and `path=` become `<redacted>`.
Set `include_queries = true` to keep them, for example to reproduce a slow
query yourself. Stacks name fcs's own functions and source files, never
your files (`gdb` is told not to print function arguments). Release
binaries keep their function names (but no debug information) so that
stacks are readable.

### How many

Captures of one kind are at least `min_interval_secs = 300` apart, and a
hang is captured once until it clears. At most `max_files = 50` captures
and `max_total_mb = 100` megabytes are kept; the oldest are deleted.

## Reading captures

```text
$ fcs diagnose
3 captures in /home/me/.local/share/fast_code_search/diagnostics, 2026-10-23 09:12:01 to 2026-10-23 11:40:02

  slow_search      2
  watcher_stalled  1

Slowest search: 7.9 s (regex search via rest): waited 0.0 s for the index, searched 7.6 s, built the reply in 0.3 s

Newest first:

2026-10-23 11:40:02  watcher_stalled  v0.18.1
  the watcher has 5123 changes to apply and has made no progress for 120 s
  index: 412000 files, 5123 changes to apply, lock busy
  last watcher burst: 2048 events, 2048 indexed, 0 removed, 3 steps, longest hold 1.0 s, took 6.1 s
  threads: 41 (1 running, 0 waiting on the disk)
    fcs-watch-0: fast_code_search::search::incremental::apply_plan
  file: …/fcs-diag-20261023T104002.118Z-watcher_stalled.json
…
```

`fcs diagnose DIR` reads another folder (one someone sent you, say). It
exits 1 when there are no captures. The **Index** page's diagnostics
(`/api/diagnostics`) show the folder and how many captures were written
since the server started.

For a bug report, attach the capture files: with the defaults they hold
no query text or file names, but read them first if you set
`include_queries = true`.
