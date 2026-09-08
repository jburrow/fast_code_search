# Benchmark methodology

Every number this project publishes comes from a run you can open, on a
machine whose specification is recorded with the result. Three tiers:

| Tier | Corpus | What it measures | Where it runs |
|---|---|---|---|
| Synthetic (Criterion) | generated Rust-like source, 50 lines per file, 50–1,000 files | the query path in isolation: text, regex, filters, paging; index build; save and load | every push to `main` |
| Real corpus | [tokio 1.45.0](https://github.com/tokio-rs/tokio/tree/tokio-1.45.0) + [Django 5.2](https://github.com/django/django/tree/5.2): 6,237 files, 38.9 MB | full build, resident memory, save and load, p50/p95 per query kind, incremental update | every push to `main` |
| Large corpus | [rust-lang/rust 1.89.0](https://github.com/rust-lang/rust/tree/1.89.0) | file count, throughput, memory at scale | nightly |

Runs live in the
[Benchmarks workflow](https://github.com/jburrow/fast_code_search/actions/workflows/benchmark.yml);
the synthetic tier is charted per commit at
[dev/bench](https://jburrow.github.io/fast_code_search/dev/bench/) by
github-action-benchmark, which fails the run on a regression beyond 2×.

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
  the same path the watcher uses.

## What the machine is

GitHub-hosted `ubuntu-latest` runners: 4 vCPU, 16 GB RAM, SSD, shared
tenancy. Absolute numbers on your hardware will differ; the value of the
CI numbers is that they are comparable commit to commit. Local runs are
labelled as such wherever they appear.

## Reproduce

```bash
cargo run --release --example corpus_bench -- path/to/repo [more paths] --label mine
cargo bench                                 # synthetic tier
cargo bench --bench search_benchmark        # search only
```

`corpus_bench` prints the same table CI does (`--markdown`) and can emit
bencher lines for tracking (`--bencher`).

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

## Comparison with scan tools

`scripts/bench/compare.sh <dir>` times the same queries through ripgrep,
ugrep (if installed) and `fcs` against a server on the same tree, warm
cache, median of seven runs, and records the machine. It reports both the
`fcs` client's wall time (process start plus HTTP) and the engine's own
time. The point is not that a scan tool is slow — it is the right tool for
a one-off search — but that an index answers in the same time however
large the tree is, and ranks. Results are on the
[latest results](latest.md) page when the comparison job has run.
