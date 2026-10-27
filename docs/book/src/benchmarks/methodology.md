# Benchmark methodology

Every number this project publishes comes from a run you can open, on a
machine whose specification is recorded with the result. Four tiers:

| Tier | Corpus | What it measures | Where it runs |
|---|---|---|---|
| Instruction counts | tokio + Django (below), under callgrind | instructions per operation: build, each query, incremental update, save, load; fails on 3% more than the last passing push | every push to `main` |
| Synthetic (Criterion) | generated Rust-like source, 50 lines per file, 50–1,000 files | the query path in isolation: text, regex, filters, paging; index build; save and load | locally (`cargo bench`); in CI until October 2026 |
| Real corpus | [tokio 1.45.0](https://github.com/tokio-rs/tokio/tree/tokio-1.45.0) + [Django 5.2](https://github.com/django/django/tree/5.2): 6,237 files, 38.9 MB | full build, resident memory, save and load, p50/p95 per query kind, incremental update | every push to `main` |
| Large corpus | [rust-lang/rust 1.89.0](https://github.com/rust-lang/rust/tree/1.89.0) | file count, throughput, memory at scale | nightly |

Runs live in the
[Benchmarks workflow](https://github.com/jburrow/fast_code_search/actions/workflows/benchmark.yml);
the real-corpus tier is charted per commit at
[dev/bench](https://jburrow.github.io/fast_code_search/dev/bench/) by
github-action-benchmark.

## Instruction counts

Wall time on a shared runner moves by tens of percent between days, so a
5% slowdown hides in it. An instruction count barely moves: the same
binary on the same tree executes the same instructions on any x86 runner.
`scripts/bench/icount.sh` runs `corpus_bench --icount` under callgrind,
which counts every thread's instructions between two marker calls around
each operation, and compares the counts with the last ones a passing push
published (`dev/bench/results/icount.json` on the site). An operation over
3% more fails the run, and main-health opens a `main-red` issue.

Counts are published only from a passing run, so a failure stays red until
the added work is removed or accepted. Accepting is a decision, named in
the squash commit message of the change that adds it:

```text
Accept-instructions: index_build, text/common
```

(`all` accepts every operation.) The workflow passes those names to
`icount.sh --accept`, the run passes, and its counts become the new base.

What counts do not see: cache misses, memory bandwidth, I/O, lock waits and
how well work spreads over cores. A change that makes code faster by doing
the same work in parallel (B2 did) shows no gain here, and one that makes it
slower by contention shows no loss. The same-machine A/B (`scripts/bench/ab.sh`)
stays the measure a PR quotes; counts catch added work between A/Bs.
Counts are taken with one rayon thread, which counts the same work
without the cost of spreading it, and each query is counted five times
(median). Run-to-run noise on one commit, measured on 2026-10-25 over eight
runs: 0.5% or less for every operation except symbol/exact and
symbol/prefix, which moved by up to 4.2% for a reason not yet found; those
two fail only at 10%. On the GitHub runner the count takes about 10
minutes, about what the Criterion suites it replaced took: the first run
([40246218157](https://github.com/jburrow/fast_code_search/actions/runs/40246218157))
finished the whole job in 18 minutes, against 20 for the run before it.

## How queries are timed

- The index is built once; queries run against it warm, 50 results per
  page, after a warm-up iteration. p50 and p95 are over repeated
  invocations of the same query.
- "Common" is a word present in most files (`return`); "identifier" a
  name present in a few; "no match" a string in none (which measures
  candidate rejection alone).
- Regex cases: a pattern with a required literal, a case-insensitive one,
  and one with no literal (a full scan).
- The incremental case rewrites one file and applies the change through
  the same path the watcher uses, after saving a checkpoint of the built
  index as a server does once a build is done (`save_after_build`, on by
  default). From then on the posting lists are read from that file and an
  update changes only the small in-memory lists, as in a running server.
  Before 27 October 2026 the update ran on the just-built index and passed
  over every posting list, which a server never does (about 9 ms against
  about 2 ms on tokio + Django). The index save that follows is then of
  the checkpointed index, as a server's later saves are.

## What the machine is

GitHub-hosted `ubuntu-latest` runners: 4 vCPU, 16 GB RAM, SSD, shared
tenancy. Absolute numbers on your hardware will differ; the value of the
CI numbers is that they are comparable commit to commit. Local runs are
labelled as such wherever they appear.

## Reproduce

```bash
cargo run --release --example corpus_bench -- path/to/repo [more paths] --label mine
scripts/bench/icount.sh                     # instruction counts (needs valgrind, ~16 min)
cargo bench                                 # synthetic tier
cargo bench --bench search_benchmark        # search only
```

`corpus_bench` prints the same table CI does (`--markdown`) and can emit
bencher lines for tracking (`--bencher`).

## Reading the trend charts

The [trend page](../../dev/bench/) charts every measurement per commit to
`main`. Each chart is a cost, so lower is better; the vertical axis is time
per operation in the unit that fits the chart (or bytes for memory). The
shaded band is that run's spread: median to 95th percentile for the
real-corpus rows (Criterion's reported deviation for the synthetic rows
recorded before October 2026). A
step on every chart at the same commit is a change of runner, not of code.
Each chart carries a sentence saying what it measures and what would move
it.

## Ranking quality

Latency says how fast the engine is; this says whether what it puts first
is what a reader wanted. `benches/ranking_quality.toml` holds labelled
queries over the pinned corpora — text, symbols, regex and references —
each naming the file and line that should come first (`struct Runtime` →
the definition in `runtime.rs`, not a doc comment; `Field` in symbols mode
→ the model field, not `FieldOverridePost` in a test). The suite reports
**precision@1** (the expected result is the first hit) and **precision@5**,
runs on every push, and fails the run below 0.90 / 0.95, so a change that
makes searches faster but worse cannot land. Its first run scored 0.30 and
surfaced six ranking defects; see the [changelog](../imported/changelog.md).

```bash
cargo run --release --example ranking_quality -- bench-corpus/tokio bench-corpus/django
cargo run --release --example ranking_quality -- … --explain 'struct Runtime'   # why a query ranks as it does
```

## Correctness against ripgrep

Fast and well ranked is worthless if a match is missing. A backtest runs
144 searches through fcs and through ripgrep on the same trees and fails on
any line one finds and the other does not; see [Correctness against
ripgrep](correctness.md) for how it works and how to extend it.

## Comparison with scan tools

`scripts/bench/compare.sh <dir>` times the same queries through ripgrep,
ugrep (if installed) and `fcs` against a server on the same tree, warm
cache, median of seven runs, and records the machine. It reports both the
`fcs` client's wall time (process start plus HTTP) and the engine's own
time. The point is not that a scan tool is slow — it is the right tool for
a one-off search — but that an index answers in the same time however
large the tree is, and ranks. Results are on the
[latest results](latest.md) page when the comparison job has run.
