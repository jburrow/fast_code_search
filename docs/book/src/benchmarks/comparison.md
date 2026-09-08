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

## Results on a laptop

11th Gen Intel Core i5-1135G7, 8 logical CPUs, ext4 on NVMe, ripgrep
15.0.0. ugrep was not installed on this machine; the CI run has it.

**tokio 1.45.0 + Django 5.2** — 6,240 files.

| Query | ripgrep | fcs (client) | fcs (engine) |
|---|---|---|---|
| common word `return` | 40.1 ms | 10.4 ms | 0.90 ms |
| identifier `Config` | 33.0 ms | 11.7 ms | 1.03 ms |
| rare identifier (no match) | 30.4 ms | 7.5 ms | 0.01 ms |
| two words `fn main` | 31.7 ms | 10.5 ms | 1.51 ms |
| regex with literal `fn\s+\w+\(` | 37.5 ms | 10.1 ms | 1.17 ms |
| regex, no literal `[a-z]+_[a-z]+_[a-z]+\(` | 45.5 ms | 9.6 ms | 1.11 ms |
| case-insensitive `(?i)unwrap\(` | 31.2 ms | 9.3 ms | 0.76 ms |

**54 crates from the cargo registry** — 11,451 files, 268 MB on disk.

| Query | ripgrep | fcs (client) | fcs (engine) |
|---|---|---|---|
| common word `return` | 58.6 ms | 16.5 ms | 1.08 ms |
| identifier `Config` | 49.0 ms | 9.3 ms | 1.14 ms |
| rare identifier (no match) | 39.1 ms | 7.3 ms | 0.01 ms |
| two words `fn main` | 42.0 ms | 13.3 ms | 3.66 ms |
| regex with literal `fn\s+\w+\(` | 138.9 ms | 13.3 ms | 1.40 ms |
| regex, no literal `[a-z]+_[a-z]+_[a-z]+\(` | 92.7 ms | 14.5 ms | 2.63 ms |
| case-insensitive `(?i)unwrap\(` | 47.2 ms | 9.9 ms | 0.96 ms |

## Reading the numbers

- **ripgrep is fast.** Thirty-odd milliseconds for a full scan of six
  thousand files is remarkable, and for a one-off search from a shell it
  is the right tool. Nothing here argues otherwise.
- **ugrep is in the CI run.** On the GitHub runner (4 vCPUs) the first
  two runs put ugrep at 6 ms against ripgrep's 60 ms on every query that
  had a match, and at 84 ms on the one that had none. That was ugrep's
  `/dev/null` shortcut above, not a scan; the
  [latest results](latest.md) page shows the corrected run. It is a good
  reminder of why the script and the raw JSON are published with the
  numbers.
- **The engine's time does not grow with the tree.** Between the two
  trees ripgrep's cost doubles to quadruples on the regex rows because it
  reads more bytes and prints more lines; the engine stays near a
  millisecond because the trigram index narrows every query to the few
  files that can match before any text is read, and it prints fifty. On a
  tree ten times larger the scan takes ten times longer and the index
  does not.
- **A query per keystroke changes the arithmetic.** An editor or web UI
  that searches as you type issues ten queries for a ten-letter
  identifier. Forty milliseconds each is a visible stutter; one
  millisecond each is not. This, and not the single-query gap, is why the
  project is a server.
- **The `fcs` client column is mostly process start.** Seven of its eight
  milliseconds are the OS starting a process and one HTTP request. A
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
