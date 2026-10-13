# Candidates and regex analysis

A search never scans the whole corpus. It first computes a **candidate
set** from the trigram index, then reads only those files.

## Plain text

The query is case-folded (Unicode simple case folding, the fold ripgrep's
`-i` uses, which is lowercase except for a few letters such as `ſ` and
`µ`; the index stores folded text) and split into overlapping three-byte
windows;
`parse_query` yields `par`, `ars`, `rse`, `se_`, `e_q`, `_qu`, `que`,
`uer`, `ery`. Each trigram has a posting list (a Roaring bitmap of file
ids); intersecting them gives every file that could contain the query,
independent of corpus size. Several words are one phrase, so their
trigrams (spaces included) are intersected the same way.

Queries shorter than three characters have no trigrams and fall back to
all files, bounded by the match budget and ranking mode.

## Regex

The pattern is parsed into a syntax tree and walked for **required
literals**: text that every match must contain. Alternations contribute
the union of their branches' literals (each branch must have one),
optional groups contribute nothing, and a repetition ends a literal run
(so `fo+bar` requires `fo` and `obar`, never `fobar`). Case-insensitive
patterns lower every literal, with `(?i)` handled through simple case
folding so `k` and `s` do not break a run. The literals' trigrams filter
the candidates exactly as a text query would; a pattern with no required
literal of three or more bytes scans everything.

A property test in the suite asserts that for every pattern the candidate
set is a superset of the files the compiled regex actually matches.

## Verification and modes

Candidates are then read and verified: the text query is searched with a
whole-buffer scan (memchr-based, or a Unicode case fold when needed) that
resolves line boundaries only at hits; a regex is compiled once per query
with `(?mR)` and run in one pass per file with one hit per line. Symbol
queries match definition names; reference queries look up the interned
identifier in each candidate's reference table and verify the position
against the current content.

## Budgets

Every search runs under a `QueryRun`: a match budget derived from the page
size and offset (capped), and a deadline just under the request timeout.
When either stops the scan the response says so
(`truncated_by_budget`). A deep page gets a budget that reaches it, so
paging past `max_page_offset` (100,000 by default) is refused rather than
allowed to grow the budget without bound.
