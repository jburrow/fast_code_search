# Against ripgrep and ugrep

A scan tool reads every file on every query. An index reads the tree once
and answers from memory. This page puts the two side by side on the same
machine, the same tree and the same queries, and says plainly what the
numbers do and do not mean. The script is
[`scripts/bench/compare.sh`](https://github.com/jburrow/fast_code_search/blob/main/scripts/bench/compare.sh);
the benchmark workflow runs it on every push and the result appears on the
[latest results](latest.md) page.

## How it is measured

- The same query string goes to every tool. Literal rows use ripgrep's
  fixed-string, case-insensitive mode, which is what a plain `fcs` search
  does; regex rows are given to both as regular expressions.
- Warm cache, median of seven runs, wall time from process start to exit,
  every tool's output written to the same scratch file. `fcs` asks for
  the best 50 hits; the scan tools are given no match limit, because a
  scan tool cannot know the best 50 without reading everything anyway.
- Output goes to a file, not `/dev/null`, because ugrep (like GNU grep)
  detects a `/dev/null` stdout and quietly switches to "stop at the
  first match". The first CI run timed that as a 6 ms "scan" of forty
  megabytes.
- `fcs (client)` is the command-line client talking to a server over HTTP.
  It includes starting the process and the round trip. `fcs (engine)` is
  the time the server reports for the search itself, which is what an
  editor holding a connection sees.
- The server is started fresh on the tree and timing begins only after
  its build has finished. Build time is not in the table; it is on the
  [latest results](latest.md) page.
- The script refuses to run on a FUSE, NFS or SMB mount. On an external
  NTFS drive the same ripgrep query took about one second instead of
  forty milliseconds, and publishing that would have been dishonest.

## Results on the CI runner

<!-- bench:comparison-run -->
From [run 40411254740](https://github.com/jburrow/fast_code_search/actions/runs/40411254740)
on commit `b1f9287` (2026-10-26, version 0.19.0): GitHub-hosted `ubuntu-latest`,
AMD EPYC 7763, 4 vCPU, ripgrep 14.1.0, ugrep 5.0.0.
<!-- /bench:comparison-run -->

<!-- bench:comparison-table -->
**tokio 1.45.0 + Django 5.2**, 7,601 files.

| Query | ripgrep | ugrep | fcs (client) | fcs (engine) |
|---|---|---|---|---|
| common word `return` | 71.3 ms | 94.2 ms | 31.4 ms | 30.75 ms |
| identifier `Config` | 61.3 ms | 89.6 ms | 13.6 ms | 10.53 ms |
| rare identifier `xq9zv_nothing_here` | 57.6 ms | 89.7 ms | 4.9 ms | 0.02 ms |
| two words `fn main` | 73.5 ms | 86.8 ms | 8.6 ms | 3.79 ms |
| regex with literal `fn\s+\w+\(` | 81.6 ms | 115.9 ms | 33.3 ms | 31.39 ms |
| regex, no literal `[a-z]+_[a-z]+_[a-z]+\(` | 83.0 ms | 131.9 ms | 44.1 ms | 44.39 ms |
| case-insensitive `(?i)unwrap\(` | 59.4 ms | 86.0 ms | 9.6 ms | 4.88 ms |
<!-- /bench:comparison-table -->

The [latest results](latest.md) page shows the newest run. Hand-run laptop
tables from before v0.17.0 used to sit here, with the engine near 1 ms on
every row. They were removed because v0.17.0 deliberately made queries with
many matches slower (see below), so they no longer describe fcs.

## Reading the numbers

- **ripgrep is fast.** Sixty-odd milliseconds on a 4-vCPU runner for a
  full scan of seven thousand files is remarkable, and for a one-off search from a shell it
  is the right tool. Nothing here argues otherwise.
- **ugrep is in the CI run.** On the GitHub runner (4 vCPUs) the first
  two runs put ugrep at 6 ms against ripgrep's 60 ms on every query that
  had a match, and at 84 ms on the one that had none. That was ugrep's
  `/dev/null` shortcut above, not a scan; the
  [latest results](latest.md) page shows the corrected run. It is a good
  reminder of why the script and the raw JSON are published with the
  numbers.
- **The engine's time follows the matches, not the tree.** The trigram
  index narrows every query to the files that can match before any text
  is read, so a rare identifier costs microseconds however large the tree
  is. A word that matches thousands of lines costs tens of milliseconds,
  because fcs reads and ranks up to 32,768 matches (the match budget)
  before it returns the best fifty. Before v0.17.0 the budget was 512 and
  those rows took about a millisecond, but page one held only 10% of the
  exhaustive top 50 instead of 81%; see
  [search limits](search-limits.md). A scan tool's time grows with every
  byte in the tree; the engine's stops growing at the budget.
- **A query per keystroke changes the arithmetic.** An editor or web UI
  that searches as you type issues ten queries for a ten-letter
  identifier. As the identifier gets longer it matches fewer lines, so
  later keystrokes cost a few milliseconds or less, while a scan pays the
  full read every time. This, and not the single-query gap, is why the
  project is a server.
- **The `fcs` client column is mostly process start.** About five
  milliseconds of each row are the OS starting a process and one HTTP
  request. A
  client that keeps a connection, such as the web UI or an editor
  extension, sees the engine column.
- **Scan tools do not rank.** Both tools return matching lines; only one
  of them puts the definition of `Runtime` first. That is measured
  separately on the [ranking quality](ranking-quality.md) page.
- **Zoekt** is the closest comparable design — a trigram index behind a
  server — and a comparison against it is planned once the corpus and
  query set are shared between the two; see the
  [roadmap](../project/roadmap.md).

## Reproduce it

```bash
cargo build --release --bins
scripts/bench/compare.sh path/to/tree [more trees] --runs 7 --json compare.json --markdown compare.md
```

`rg` must be on the path; `ugrep` is used if present. The tree must be on
a local filesystem.
