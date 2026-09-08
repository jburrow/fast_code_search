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
  all output to `/dev/null`. `fcs` asks for the best 50 hits; the scan
  tools are given no match limit, because ripgrep's limit is per file and
  ugrep's stops the whole search, and a scan tool cannot know the best 50
  without reading everything anyway.
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
| common word `return` | 36.0 ms | 7.8 ms | 0.90 ms |
| identifier `Config` | 32.3 ms | 10.4 ms | 0.98 ms |
| rare identifier (no match) | 30.2 ms | 8.7 ms | 0.01 ms |
| two words `fn main` | 37.2 ms | 8.0 ms | 1.63 ms |
| regex with literal `fn\s+\w+\(` | 37.4 ms | 15.4 ms | 1.09 ms |
| regex, no literal `[a-z]+_[a-z]+_[a-z]+\(` | 42.2 ms | 8.2 ms | 1.06 ms |
| case-insensitive `(?i)unwrap\(` | 30.7 ms | 7.5 ms | 0.72 ms |

**54 crates from the cargo registry** — 11,451 files, 268 MB on disk.

| Query | ripgrep | fcs (client) | fcs (engine) |
|---|---|---|---|
| common word `return` | 50.8 ms | 12.4 ms | 0.95 ms |
| identifier `Config` | 48.2 ms | 9.2 ms | 1.28 ms |
| rare identifier (no match) | 39.7 ms | 10.6 ms | 0.01 ms |
| two words `fn main` | 40.8 ms | 16.9 ms | 4.46 ms |
| regex with literal `fn\s+\w+\(` | 76.2 ms | 9.8 ms | 1.34 ms |
| regex, no literal `[a-z]+_[a-z]+_[a-z]+\(` | 80.4 ms | 8.7 ms | 2.72 ms |
| case-insensitive `(?i)unwrap\(` | 48.4 ms | 10.7 ms | 0.95 ms |

## Reading the numbers

- **ripgrep is fast.** Thirty-odd milliseconds for a full scan of six
  thousand files is remarkable, and for a one-off search from a shell it
  is the right tool. Nothing here argues otherwise.
- **ugrep is in the CI run.** On the GitHub runner (4 vCPUs) the first
  run put ugrep at 7 ms against ripgrep's 60 ms, which was ugrep's match
  limit stopping the whole search after fifty hits, not a scan. The
  limits were removed from both scan tools for that reason; the
  [latest results](latest.md) page shows the corrected run.
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
