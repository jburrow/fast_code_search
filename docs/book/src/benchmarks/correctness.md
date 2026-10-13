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

## Generated queries

The fixed set holds the cases someone thought of. `--generate N` adds N
queries made up from the corpus itself: identifiers, substrings, whole
lines, punctuation runs and non-ASCII text, each in random case, some with
MATCH CASE and whole word set. Each runs through fcs and through `rg -F`
(with `-i` and `-w` as set) and the lines must match. A query that differs
is shrunk, dropping characters from either end while it still differs, and
the report ends with the smallest forms as `[[query]]` blocks to add to
`benches/rg_backtest.toml` with the fix.

CI generates 500 per run: with a fixed seed on pull requests, and a new
seed on every push to `main` and once a week, so the set keeps exploring.
Rerun a failure locally with the seed from the report:

```sh
cargo run --release --example rg_backtest -- bench-corpus/tokio bench-corpus/django \
    tests/fixtures/backtest --generate 500 --seed SEED
```

Its first runs found two whole-word bugs, both now fixed and in the query
set: a combining mark (an Indic vowel sign, a decomposed accent) was not
counted as a word character, and with MATCH CASE a match rejected by the
whole-word test skipped the overlapping one after it (`}}` in `field}}}`).

## How a query is translated

| fcs query | ripgrep run | Compared how |
|---|---|---|
| Text (`Runtime`, `fn main`, `"x":`, `-Wall`) | `rg -F -i -e TEXT`, the text as typed minus operators | the same lines exactly |
| `case:yes` | drops `-i` | the same lines exactly |
| `word:yes` | adds `-w` | the same lines exactly |
| `file:` / `lang:` | the query's own `globs` (`-g`) | the same lines exactly |
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

The query set, `benches/rg_backtest.toml`, has 126 queries. Each one says
what it exercises (`why`). They cover literals, short terms, quotes, case
and whole-word options, Unicode, path and language filters, phrases,
leading dashes and regex.

## Beyond matching

Three more checks run on every query:

- **Repeatable.** The first page (50 results) is fetched three times and
  must be identical. A search that stops at its match budget must still
  give the same answer every time.
- **LOAD MORE** (`--check-paging`, on in CI). Each query with up to 1000
  matches is loaded the way the web UI's LOAD MORE does it: the whole list
  again, 50 longer each time, while the server says more exist. Every match
  must end up shown exactly once.
- **Default limits** (`--check-defaults`, on in CI). Everything above runs
  without result limits, so it cannot see a problem the limits themselves
  cause. This check walks each query with up to 20,000 matches
  (`--defaults-max-hits`) the way a web UI user would with the server's
  default settings, once with Auto ranking and once with Fast: the first
  page, SEARCH ALL FILES if ranking skipped files, then LOAD MORE until
  the server says there is no more (the whole list again up to 1000, then
  50 at a time by `offset`). Every match must end up shown exactly once,
  and a total given as an exact count must be the real one whenever every
  candidate file was searched.

## Running it

```bash
# what CI runs (the corpora are shallow clones; see .github/workflows/backtest.yml)
cargo run --release --example rg_backtest -- bench-corpus/tokio bench-corpus/django \
    tests/fixtures/backtest --check-paging --check-defaults --markdown report.md --json report.json

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

The default-limits check was added after fast ranking's 2,000-file cap and
the old offset ceiling were found by hand, not by the backtest. On its first
run it found that LOAD MORE past 1000 results repeated some matches and
skipped others on broad queries (`self.assertEqual` over Django lost 1,213
of 17,717), because each deeper page ranked a different sample. That is
fixed too.

## What it does not cover

- **Symbol and reference searches.** They have no grep equivalent.
- **Ranking order.** That is the [ranking-quality
  suite](ranking-quality.md).
- **API `offset` paging below 1000 results across a search that hit its
  budget.** It is not stable (see [Matching compared with grep and
  ripgrep](../reference/matching.md#where-fcs-differs-from-all-of-them)).
  The web UI never pages that way; its LOAD MORE is checked.
- **Queries with more than 20,000 matches** under the default limits. Each
  deep page re-runs the search, so walking them all is slow; raise
  `--defaults-max-hits` to include them.
- **The search deadline.** The walk runs with no time limit. A search the
  deadline stops is a sample of whatever was read in time.
- **GNU grep's test suite.** It has not been mined yet.
