# Correctness against ripgrep

A search tool that misses a line is worse than a slow one: nobody notices
the line that was not shown. Unit tests check the cases someone thought of.
The backtest checks fcs against an independent, widely used tool on real
code, and runs on every change to the engine.

## The idea

ripgrep is the oracle. For each search in a fixed query set, the backtest:

1. runs the search through the fcs engine with every result limit switched
   off,
2. runs the same search through `rg` with the flags that mean the same
   thing,
3. compares the two answers line by line.

A line ripgrep reports and fcs does not is a **miss**. A line fcs reports and
ripgrep does not is an **extra**. Either one fails the run, unless the query
is marked `known` with a written reason.

The translation to `rg` flags comes from the query alone, never from fcs's
own matching code, so a bug in fcs cannot hide by also being in the oracle.
Where fcs syntax has no rg equivalent (`file:`, `lang:`), the query states
what it means in rg terms, as `-g` globs written by hand.

ripgrep was chosen over grep and git grep because it shares fcs's regex
engine and Unicode case folding, so any difference is a real difference.
See [Matching compared with grep and ripgrep](../reference/matching.md) for
where fcs, ripgrep, grep and git grep behave differently, and why.

## How a query is translated

| fcs query | ripgrep run | Compared how |
|---|---|---|
| One term (`Runtime`) | `rg -F -i -e Runtime` | the same lines exactly |
| `case:yes` | drops `-i` | the same lines exactly |
| `word:yes` | adds `-w` | the same lines exactly |
| Several terms (`fn main`) | one `rg -F` run per term | the files must be those holding every term; every line holding all terms must be reported; every reported line must hold at least one term (fcs keeps only a few single-term lines per file by design) |
| `-term` | an rg run for `term`, then those files are removed | as above |
| `file:` / `lang:` | the query's own `globs` (`-g`) | as above |
| Regex | `rg -e PATTERN`, plus `-U` when the pattern spans lines | the line where each match starts |

rg always runs with `--no-config -uu --crlf`, one tree at a time. Only files
fcs indexed are compared, so differences in which files get searched (size
limits, excluded folders, binary files) never show up as search
differences. File-name hits have no rg counterpart and are left out.

## What it runs on

- **Real code:** tokio 1.45.0 and Django 5.2, the same pinned trees as the
  speed benchmarks. About 6,000 files of Rust and Python.
- **Edge cases:** `tests/fixtures/backtest`, small files that real
  repositories rarely contain. They include CRLF line endings, a file with
  no final newline, a file with 250 matches, a very long line, Unicode
  (Greek, German, Japanese, emoji), quotes, operator-like text, a byte order
  mark and a vertical tab.
- **Cases borrowed from other tools:** `tests/fixtures/backtest/mined`
  holds 37 cases adapted from ripgrep's regression tests
  (`tests/regression.rs`, named after the issue they fixed) and Zoekt's
  index tests (`index/index_test.go`). These are the kinds of patterns a
  trigram index or regex literal extractor tends to get wrong: one- and
  two-byte terms, trigrams present but in the wrong order or split across
  files, optional groups and `{0}` repeats, character classes, flags
  changed mid-pattern, word boundaries next to punctuation, and matches that
  start on a newline.

The query set, `benches/rg_backtest.toml`, has 124 queries. Each one says
what it exercises (`why`). They cover literals, short terms, quotes, case
and whole-word options, Unicode, path and language filters, several terms,
negation and regex.

## Beyond matching

Two more checks run on every query:

- **Repeatable.** The first page (50 results) is fetched three times and
  must be identical. A search that stops at its match budget must still
  give the same answer every time.
- **LOAD MORE** (`--check-paging`, on in CI). Each query with up to 1000
  matches is loaded the way the web UI's LOAD MORE does it: the whole list
  again, 50 longer each time, while the server says more exist. Every match
  must end up shown exactly once.

## Running it

```bash
# what CI runs (the corpora are shallow clones; see .github/workflows/backtest.yml)
cargo run --release --example rg_backtest -- bench-corpus/tokio bench-corpus/django \
    tests/fixtures/backtest --check-paging --markdown report.md --json report.json

# just the edge cases: no clones needed, a few seconds
cargo run --release --example rg_backtest -- tests/fixtures/backtest

# only queries whose text contains "needle"
cargo run --release --example rg_backtest -- tests/fixtures/backtest --only needle
```

The report lists every query with ripgrep's and fcs's line counts, and shows
sample lines for each miss or extra. The run exits non-zero on any
difference that is not marked `known`. CI runs it on every pull request that
touches `src/`, the query set or the fixtures, and on every push to `main`.
The report appears in the job summary.

## Keeping it honest

- **Every search bug fixed gets a query.** When a search bug is fixed, add
  the query that showed it to `benches/rg_backtest.toml`, with a fixture
  file if the corpora do not already contain the case. Then it stays fixed.
- **A difference is fixed or explained, never hidden.** If fcs differs from
  ripgrep on purpose, mark the query `known = "…"` with the reason, and
  list it under "Known differences" in [Matching compared with grep and
  ripgrep](../reference/matching.md). The backtest still reports it and
  says when it stops differing, so a stale `known` is noticed.
- **The oracle stays independent.** Never derive rg flags or globs from fcs
  code. Write them out in the query.

## What it has caught

When it was first run, the backtest found that a broad search returned a
different first page on almost every run, and that LOAD MORE repeated some
results and skipped others. It also found that Greek words with a final
sigma were missed, that a regex able to match nothing reported a line past
the end of a file, and that a byte order mark stopped `^` from matching
line 1. All five are fixed (see the [changelog](../imported/changelog.md)).

## What it does not cover

- **Symbol and reference searches.** They have no grep equivalent.
- **Ranking order.** That is the [ranking-quality
  suite](ranking-quality.md).
- **API `offset` paging across a search that hit its budget.** It is not
  stable (see [Matching compared with grep and
  ripgrep](../reference/matching.md#where-fcs-differs-from-all-of-them)).
  Only the web UI's LOAD MORE is checked.
- **GNU grep's test suite.** It has not been mined yet.
