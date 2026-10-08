# Backtest corpus

Hand-written edge cases for `examples/rg_backtest.rs`: quotes, CRLF line
endings, Unicode, long lines, a file with hundreds of hits, operator-like
text, word boundaries and a small directory tree for `file:` queries. Real
repositories rarely contain these, which is why they are spelled out here.
Do not normalise the line endings of `crlf.txt` (see `.gitattributes`).
