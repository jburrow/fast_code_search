# How matching compares with grep, ripgrep and git grep

fcs aims to find exactly the lines ripgrep finds for the same search. Every
difference listed below is either deliberate, or the result of fcs being an
index rather than a scan tool. ripgrep is the reference because it is the
closest well-known tool: the same regex engine (Rust `regex`), Unicode-aware
case folding, and `.gitignore` handling. The [ripgrep
backtest](../benchmarks/correctness.md) checks this on every engine change.
It runs a fixed set of searches through both tools and fails on any line one
finds and the other does not.

## Where fcs agrees with ripgrep and differs from grep and git grep

| Behaviour | fcs | ripgrep | GNU grep | git grep |
|---|---|---|---|---|
| Regex dialect | Rust `regex` | Rust `regex` (PCRE2 with `-P`) | POSIX BRE/ERE (`-E`), PCRE with `-P` | POSIX BRE/ERE, PCRE with `-P` |
| Byte order mark at the start of a file | ignored, so `^` matches line 1 | ignored (kept with `-E none`) | part of line 1, so `^` misses it | part of line 1, so `^` misses it |
| `$` on a line ending in CRLF | matches before the `\r` | only with `--crlf` | no | no |
| Pattern that mentions `\n` | runs over the whole file and reports the line where the match starts | needs `-U` | not supported | not supported |
| Case-insensitive folding | Unicode simple case folding: `Σ`, `σ` and `ς` are the same letter; `ß` is not `ss` | the same | depends on the locale | depends on the locale |
| Hidden files (`.github/`, `.env.example`) | searched | skipped unless `--hidden` | searched | searched if tracked |
| Files searched | the indexed tree, honouring `.gitignore` | the tree, honouring `.gitignore` | what you name | tracked files only |

These were checked with GNU grep 3.11, git 2.43 and ripgrep 14.1.

## Where fcs differs from all of them

These follow from fcs being a code search server rather than a line filter.

- **Case-insensitive by default.** Text searches ignore case unless the query
  says `case:yes`. grep, git grep and ripgrep are case-sensitive by default.
  A regex is case-sensitive unless it uses `(?i)` or the request sets the
  case option. The backtest passes `-i` to ripgrep whenever fcs ignores
  case.
- **Several words are not a phrase.** `fn main` finds files that contain
  both words, anywhere in the file. Lines holding the phrase rank first,
  then lines holding both words, then at most three lines per file holding
  only one of them. Quote the words (`"fn main"`) to search for the exact
  phrase, as grep would.
- **Operators.** `-term`, `file:`, `lang:`, `case:` and `word:` are query
  syntax, not text. To search for one literally, quote it: `"-test"`,
  `"file:"`. A `-` only negates before a letter, `_` or quote, so `->`,
  `-1.5` and `--flag` are searched as written. See [Query
  syntax](query-syntax.md).
- **File names match too.** A text search also lists files whose *name*
  matches, as a hit without a line number. grep has no equivalent, so the
  backtest leaves these out of the comparison.
- **One result per line.** A line with several matches is one result. For a
  multi-line match, the result is the line where it starts. ripgrep counts
  lines the same way (`rg -c`).
- **Ranked, and capped.** Results come back best first rather than in file
  order. To keep a broad search fast, a request reads a bounded number of
  matches: by default 100 from one file on the first page, and a budget of
  about eight times the requested page. When either limit is reached, the
  total is shown as "N+" rather than as a count. LOAD MORE in the web UI
  fetches the whole list again at the bigger size, up to 1000 results, so
  every match ends up shown exactly once. Results already on screen can
  move when it does. Past 1000 results it fetches the next page instead.
  An API client asking for `offset` pages of a search that hit its budget
  can see a result repeated or skipped between pages. Asking again from
  offset 0 with a larger `max` avoids that.
- **Non-UTF-8 files are converted.** Files in Latin-1, Windows-1252,
  Shift-JIS or UTF-16 are converted to UTF-8 when they are indexed
  (`transcode_non_utf8`, on by default). ripgrep only does this for UTF-16
  with a byte order mark, or when given `-E`. grep and git grep match raw
  bytes. Binary files are not indexed.
- **Index scope.** Files over `max_file_size` (10 MB by default) and paths
  matching `exclude_patterns` (`node_modules`, `target`, `.git`, `build`,
  `dist`, Python virtual environments) are not indexed, so they are never
  searched.

## Known differences

None at the moment. A known difference would be listed here, and marked
`known` with its reason in `benches/rg_backtest.toml`, so the backtest still
reports it without failing.
