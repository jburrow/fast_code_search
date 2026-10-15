# Search limits

A ranked search cannot rank what it never read, and reading every hit of
a word like `return` in a large repository costs seconds and gigabytes.
So each search collects a bounded number of matches, ranks them and shows
the best. This page lists those limits, what they cost and what they
hide, measured rather than guessed. Every limit that cuts a search is
reported with the result (`limits_hit`), and every one is set in the
configuration file.

## How it was measured

- **Corpus:** Linux, LLVM, Rust and Kubernetes together (shallow clones of
  their default branches, October 2026): 372,517 files, 607 MB. Common
  tokens have millions of hits: `return` 3.08M, `struct` 3.43M, `static`
  1.26M.
- **Machine:** a 4 vCPU cloud VM (Xeon @ 2.1 GHz, 15 GB RAM), warm page
  cache. Times are engine time, without HTTP. Each figure is the median of
  3 runs.
- **Queries:** 25, grouped by true hit count: **huge** (over 100k hits,
  10 queries), **mid** (5k to 100k, 11) and **small** (under 5k, 4).
- **Reference:** each query is first run with no limits at all, giving
  fcs's own ranking of every match. A setting is judged by how much of
  that exhaustive top 50 its page 1 holds (**top-50**), and in how many
  queries page 1 starts with the exhaustive #1 (**best first**). This
  measures what the limits change, not whether the ranking itself is good.
  Ties barely affect it: at rank 50 most queries tie with 1 to 20 other
  hits.

To repeat it on your own code:

```sh
cargo run --release --example limits_bench -- <dir>... \
    --index /tmp/fcs-bench.idx --queries queries.txt --json out.jsonl
```

The first run builds and saves the index, and later runs load it.
`--only modes,files,budget,page` picks the experiments.

## Match budget

The budget is how many matches a search collects before it ranks.
Before 0.17 a first page collected 562 (a 512 floor). Page 1 was then the
best of the first 562 matches found, out of 3.4M for `struct`:

| Group | top-50 | best first | median / worst |
|---|---|---|---|
| huge | 10% | 0 of 10 | 16 / 55 ms |
| mid | 11% | 1 of 11 | 8 / 26 ms |
| small | 53% | 2 of 4 | 20 / 57 ms |

The budget sweep below is for a page of 50. In it, files are read best
first by the metadata score, which is how every search reads them now:

| Budget | huge: top-50 / best / median / worst | mid: top-50 / best / median / worst |
|---|---|---|
| 512 | 10% / 0 of 10 / 13 / 33 ms | 18% / 3 of 11 / 7 / 18 ms |
| 2,048 | 10% / 0 / 16 / 36 ms | 50% / 7 / 21 / 31 ms |
| 8,192 | 36% / 5 / 27 / 47 ms | 75% / 8 / 49 / 85 ms |
| 16,384 | 43% / 5 / 48 / 74 ms | 89% / 10 / 77 / 97 ms |
| **32,768** | **81% / 9 of 10 / 78 / 118 ms** | **98% / 11 of 11 / 85 / 162 ms** |
| 100,000 | 82% / 9 / 194 / 304 ms | 98% / 11 / 86 / 145 ms |
| 1,000,000 | 82% / 9 / 2,035 / 2,416 ms | 98% / 11 / 82 / 154 ms |

Small queries reach 100% from 2,048 on. **32,768 is where quality stops
improving.** Going to a million adds one point and costs 26 times the time.
Each collected match takes 182 to 277 bytes while the search runs, so
32,768 is about 7.5 MB per search. If all 64 permitted searches hit it at
once, that is about 480 MB.

**Defaults:** `min_match_budget = 32768`, `match_budget_multiplier = 8`
(which only matters for pages ending past 4,096), `max_match_budget =
100000`. The rest of the huge-query gap is the per-file cap (below) and
files with thousands of hits each.

## Reading order

Before 0.17, `rank=full`, which was auto's choice below 5,000 candidates,
read files in the order they were discovered on disk. At the same budget
that order does much worse:

| Order, budget 32,768 | huge: top-50 / best | mid: top-50 / best |
|---|---|---|
| best first (now) | 81% / 9 of 10 | 98% / 11 of 11 |
| discovery order (before) | 31% / 4 of 10 | 91% / 10 of 11 |

Discovery order needed about 8 times the budget (262,144, 520 ms median)
to come close. Every mode now reads best first. Sorting the candidates
costs at most about 14 ms for `struct`'s 142k candidate files.

## Files read (`fast_rank_files`)

Before 0.17, auto ranking switched to "fast" above 5,000 candidate files
and read only the best 2,000. For common words that cost nothing, because
the budget fills within the first files read. The cap mattered only for
queries with many *candidate* files (trigram matches) but few *real* hits:
`handle_error` has 8,046 candidate files and only 173 with a hit.

| Files read | hits on page 1 (page of 50) | top-50 | time |
|---|---|---|---|
| 500 | 21 | 32% | 11 ms |
| 2,000 (the old default) | 50 | 54% | 29 ms |
| 10,000, or no cap | 50 | 100% | 104 ms |

Neither mode was slow at any candidate count (the worst was 102 ms), so
the 5,000 switch protected nothing. **Default:** no cap
(`fast_rank_files = 0`). A search reads until the budget is spent or the
deadline passes. A server may still set a cap, and it is reported as
`max_files` when it bites.

## Page size and MCP

| Page size | median / worst, huge queries |
|---|---|
| 20 | 14 / 38 ms |
| 100 | 13 / 46 ms |
| 500 | 16 / 35 ms |
| 1,000 | 195 / 291 ms |

A page of 1,000 gets the 100,000 budget, so it costs more. It was still
under 0.3 s for 3M-hit words. MCP's `search_code` used to stop at 100 hits
with no way to page. **Defaults:** `mcp.default_limit = 50`,
`mcp.max_limit = 1000`, plus `offset`, `max_files` and `rank` arguments.

## Per-file cap (`per_file_matches`)

Files with more than 100 hits: `return` 5,685, `struct` 7,359, `static`
1,671, `printf` 76. The largest single file has 11,242 `return` hits.

Until 0.17 the cap was 100, applied to a file's *first* 100 hits in file
order and raised to the page end. Two things were wrong with that. Page 1
showed a busy file's earliest hits, not its best (a file with 150 long
comment lines mentioning a word before 150 short assignments to it showed
the comments, score 1.2, and never the assignments, score 7.8). And
because the cap grew with the page end, each `offset` page ranked a
different sample: walking such a search by offset, as MCP tells agents
to, returned 305 hits of which 155 were distinct.

Now a file keeps its best hits by score, and the cap is the same for every
page in a range. **Default:** 1,000, the end of the range (results before
1000): a file's 1,001st-best hit can never rank in the first 1,000
results, so the cap leaves out nothing those pages could show. From result
1000 on the cap is the page's budget, as before. The cost is that every
hit of a capped file is scored (the old cap stopped reading the file at
hit 100), and a busy file uses up to 1,000 of the budget instead of 100.

## Deadline

The slowest default above (a page of 1,000) took 291 ms, and a
million-match budget took 2.4 s. The engine deadline, just under
`request_timeout_secs` (30 s), only matters for regex full scans and
loaded machines. **Default:** unchanged.

## Caveats

- One corpus, on four slow cores. Cost follows the matches collected (the
  budget) more than the repository size. The candidate sort grows with
  the number of candidate files.
- Quality is measured against fcs's own exhaustive ranking, not human
  judgement.
