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
- Every tool is asked for at most 50 hits, warm cache, median of seven
  runs, wall time from process start to exit.
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
| common word `return` | 38.8 ms | 8.3 ms | 1.05 ms |
| identifier `Config` | 35.4 ms | 7.6 ms | 1.09 ms |
| rare identifier (no match) | 33.8 ms | 6.5 ms | 0.01 ms |
| two words `fn main` | 34.3 ms | 8.5 ms | 1.64 ms |
| regex with literal `fn\s+\w+\(` | 38.9 ms | 7.5 ms | 1.30 ms |
| regex, no literal `[a-z]+_[a-z]+_[a-z]+\(` | 44.2 ms | 7.5 ms | 1.26 ms |
| case-insensitive `(?i)unwrap\(` | 34.3 ms | 7.2 ms | 0.85 ms |

**54 crates from the cargo registry** — 11,451 files, 268 MB on disk.

| Query | ripgrep | fcs (client) | fcs (engine) |
|---|---|---|---|
| common word `return` | 51.5 ms | 8.2 ms | 1.07 ms |
| identifier `Config` | 43.9 ms | 11.0 ms | 1.03 ms |
| rare identifier (no match) | 38.6 ms | 7.5 ms | 0.01 ms |
| two words `fn main` | 40.5 ms | 10.7 ms | 3.65 ms |
| regex with literal `fn\s+\w+\(` | 64.7 ms | 15.4 ms | 1.43 ms |
| regex, no literal `[a-z]+_[a-z]+_[a-z]+\(` | 78.2 ms | 8.6 ms | 1.77 ms |
| case-insensitive `(?i)unwrap\(` | 44.6 ms | 8.9 ms | 1.47 ms |

## Reading the numbers

- **ripgrep is fast.** Forty milliseconds for a full scan of six thousand
  files is remarkable, and for a one-off search from a shell it is the
  right tool. Nothing here argues otherwise.
- **The engine's time does not grow with the tree.** Between the two
  trees ripgrep's cost roughly doubles on the regex rows because it reads
  twice as many bytes; the engine stays near a millisecond because the
  trigram index narrows every query to the few files that can match
  before any text is read. On a tree ten times larger the scan takes ten
  times longer and the index does not.
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
