# CLAUDE.md

Rules for coding agents working in this repository. The code map and
conventions are in [`.github/copilot-instructions.md`](.github/copilot-instructions.md);
build, test and release commands are in [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md).
This file holds the rules that past mistakes taught us. They are not
suggestions: if a change cannot follow one, stop and ask the maintainer.

## Search behaviour

- **Every match, in rank order.** fcs is a ranked search engine. Never add
  behaviour that drops, collapses, defers or hides matches. Any limit must be
  visible to the user (a note, a count, a "more exist" line), in the web UI,
  `fcs`, gRPC and MCP alike.
- **Match ripgrep.** Plain search behaves like `rg -F`. Every divergence from
  grep, ripgrep or git grep is listed in
  [`docs/book/src/reference/matching.md`](docs/book/src/reference/matching.md).
- **A backtest query for every search fix**, in `benches/rg_backtest.toml`.
  The backtest compares the exact query text with `rg`, never a translation.
- **Limits and defaults need measured evidence** (`examples/limits_bench.rs`,
  `docs/book/src/benchmarks/search-limits.md`), not a round number.
- **Document known limitations** in the book before the work is called done.
- **Never re-case** a filename, path or query anywhere in the UI.

## Performance

Speed is the product. From 13 to 24 October 2026 the Benchmarks workflow
failed on every push to `main` and nobody looked. In that time index builds
got about 30% slower and index loading 80% slower, from changes nobody
measured. Some of the cost bought correctness; most was waste that a
profiler found in an hour.

- **Before merging a change to indexing, search, persistence or the watcher,
  run the same-machine A/B:**

  ```bash
  scripts/bench/ab.sh            # base origin/main vs this tree, tokio + Django
  ```

  Paste its table into the PR's **Performance** section. Numbers from
  different machines, or from GitHub runners on different days, are not
  comparable; only an A/B from one machine in one sitting is.
- **A slowdown is a decision, not an accident.** If the script marks a
  metric **slower** (median over 10% and no round overlap with the base),
  either fix it or name the slowdown and what it buys in the PR and in the
  CHANGELOG entry. Smaller moves are within this machine's noise: main
  against itself moved single metrics by up to 18% over 5 rounds.
- **Profile before calling a cost inherent.** `valgrind --tool=callgrind`
  on `target/release/examples/corpus_bench` (collect everything; rayon
  workers miss a toggle on the main thread), then `callgrind_annotate
  --inclusive=yes`. Say in the PR what the profile showed.

## Keeping main healthy

- **Don't merge while a workflow on `main` is red** without saying why in the
  PR. A failing workflow on `main` opens an issue labelled `main-red`
  (`.github/workflows/main-health.yml`); fix it or explain it before stacking
  more work on top.
- **Never skip, disable or loosen a test or a gate to get green.** A quality
  gate that fails (for example ranking quality in the Benchmarks workflow)
  is a bug report.
- A flaky test gets an issue with its measured failure rate and a fix at
  the root, not retries until green.

## Pull requests

- One PR per piece of work; fold follow-ups into the open PR.
- Fill in [`.github/PULL_REQUEST_TEMPLATE.md`](.github/PULL_REQUEST_TEMPLATE.md),
  including **Performance** when the code paths above change.
- User-visible changes get a CHANGELOG `[Unreleased]` entry.
- Releases and tags are made by the maintainer (bump PR first, then the tag).

## Privacy

Never copy names, numbers, paths, code or text from a user's private code,
logs or screenshots into the repository, a PR, a commit message or a test.
Reproduce with public code (tokio, Django) or made-up examples.
