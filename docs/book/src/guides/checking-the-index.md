# Checking a live index

A search engine that has lost a change does not fail: it answers less.
fcs checks its own index in three ways so that kind of loss shows up
as a number:

- **Invariants.** The index agrees with itself. Every posting list names
  only files that exist. Every file is scanned by queries too short for
  trigrams. Path lookups, the folder index and display paths agree.
  Nothing is cached for removed files. The dependency graph's two
  directions and counts match.
- **Content.** For each indexed file, the version that was indexed is
  still what is on disk, and every trigram of it is in the index.
- **Discovery.** The indexed files are the ones a fresh walk of the
  disk, with the same settings, would index.

## `fcs check --live`

```text
$ fcs check --live
index: 412,880 files
invariants: ok
content (2000 files read): ok
discovery (412,880 files on disk to index): ok
background scrub: 1,204,113 files checked (2 full passes), 0 out of step; 31 consistency checks, last clean
no problems found in 9.4 s
```

This asks the running server (`--server`, or the configured address) to
check its index. The output is counts only, so you can paste it anywhere.
Add `--show-paths` to list example files for each kind of problem, or
`--json` for the full report. Exit status 1 means problems were found.

- `--content N` reads N files spread over the index (default 2,000;
  `--all-content` reads every file). Reading every file of a large index
  takes minutes. `/api/check` is not cut off by `request_timeout_secs`,
  and `fcs` waits up to an hour for it. One check runs at a time; a
  second gets 429 until the first is done.
- `--no-discovery` skips the walk of the disk.

A change the watcher has not applied yet looks like a difference for a
moment. Anything found is checked a second time, once the watcher has
caught up (or after ten seconds), and only what remains is reported. With
file watching off, files changed since the last scan show up as
`changed`, `gone`, `missing` or `extra` until the server rescans them;
the output says so.

What each kind means:

| Kind | Meaning |
|---|---|
| `content.changed` | The file on disk differs from the indexed version (a lost change if it lasts) |
| `content.gone` | Deleted from disk, still in the index |
| `content.missing_trigrams` | Unchanged, but some of its text would not find it: a bug, please report it |
| `content.not_indexable` | Unchanged, but no longer something fcs indexes (binary, too large) |
| `discovery.missing` | On disk and indexable, not in the index |
| `discovery.extra` | In the index, but gone or excluded now (an ignore rule, an exclude pattern) |
| `*.unreadable` | Can't be read (reported, not counted as a problem) |
| `folders: N could not be read` | Folders the last scan could not list, so their files are not indexed (reported, not counted as a problem; `--show-paths` names them) |
| `folders: N could not be watched` | Folders the file watcher could not watch (reported, not counted) |
| `invariants.*` | The index disagrees with itself: a bug, please report it |

The same report is available as JSON from `GET /api/check`. It accepts
`content`, `all`, `discovery` and `examples` parameters, and runs one
check at a time. The check takes only read locks, so searches keep being
answered. The longest lock is the invariant pass, about 0.6 s on
200,000 files; the watcher waits for that, searches do not.

## The background scrub

The server also checks itself continuously, a few files at a time
(`[diagnostics] scrub`, on by default):

- It reads 3,000 files a minute (`scrub_files_per_minute`) in a round
  over the index. Measured at about 52,000 files a second on one thread,
  that is roughly 0.1% of a core. A million files take under six hours
  per pass.
- A file that is out of step for over a minute counts as drift: a change
  the watcher lost. So does a file whose trigrams are missing. Drift is
  logged as a warning, counted, and queued for the watcher to index again.
- The invariants are checked every hour (`scrub_invariants_minutes`).

The counts are in `fcs check --live`, on the diagnostics page (Index
scrub) and in `/api/diagnostics` under `scrub`. Any non-zero drift is
worth reporting: it means a change was lost and would have stayed lost.

## Where the checks also run

- The watch harness (`tests/watch_harness.rs`) runs the invariants and
  the content check on every file after every scenario.
- The fault-injection tests (`tests/fault_injection.rs`) run the
  invariants after each injected failure, and check that the scrub finds
  and fixes a change the watcher never heard about.
- The ripgrep backtest and `fcs check` check the invariants of the index
  they build before comparing results.
