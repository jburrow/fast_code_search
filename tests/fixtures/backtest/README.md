# Backtest corpus

Hand-written edge cases for `examples/rg_backtest.rs`: quotes, CRLF line
endings, Unicode, long lines, a file with hundreds of hits, operator-like
text, word boundaries and a small directory tree for `file:` queries. Real
repositories rarely contain these, which is why they are spelled out here.
Do not normalise the line endings of `crlf.txt` (see `.gitattributes`).

`mined/` holds cases adapted from ripgrep's regression tests and Zoekt's
index tests, the kinds of input a trigram prefilter or regex literal
extractor gets wrong. Each query in `benches/rg_backtest.toml` that uses
one names its source in `why`. See docs/book/src/benchmarks/correctness.md.
