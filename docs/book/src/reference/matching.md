# How matching compares with grep, ripgrep and git grep

fcs aims to find exactly the lines ripgrep finds for the same search. Every
difference listed below is either deliberate, or the result of fcs being an
index rather than a scan tool. ripgrep is the reference because it is the
closest well-known tool: the same regex engine (Rust `regex`), Unicode-aware
case folding, and `.gitignore` handling. The [ripgrep
backtest](../benchmarks/correctness.md) checks this on every engine change.
It runs a fixed set of searches through both tools and fails on any line one
finds and the other does not.

## Every search, as ripgrep would run it

The contract in one table: for each kind of fcs search, the ripgrep run
that finds the same lines. "Same lines" is what the backtest checks; fcs
then ranks them and may cap how many it reads (see *Ranked, and capped*
below, and `limits_hit` in every response).

| fcs search | Same lines as | Checked by |
|---|---|---|
| `foo bar` (plain text) | `rg -F -i 'foo bar'` | backtest, generated queries |
| `foo case:yes` (or MATCH CASE, `fcs -s`) | `rg -F 'foo'` | backtest, generated queries |
| `foo word:yes` (or `fcs -w`) | `rg -F -i -w 'foo'` | backtest, generated queries |
| `foo case:yes word:yes` | `rg -F -w 'foo'` | backtest, generated queries |
| Regex on the API or `fcs -e` | `rg 'PATTERN'` (`rg -U` when it mentions `\n`) | backtest |
| Regex with `case=false`, `(?i)` or `fcs -e -i` | `rg -i 'PATTERN'` | backtest |
| `lang:rust foo` | `rg -F -i -t rust 'foo'` | backtest (with `globs`) |
| `file:*.py foo` / `-file:tests foo` | `rg -F -i -g '*.py' 'foo'` / `-g '!tests'`; a bare word such as `file:parser` matches anywhere in the path, which `-g` has no exact form for | backtest (with `globs`) |
| `file:src/ lang:rust foo` | `rg -F -i -g 'src/**/*.rs' 'foo'`: different filters narrow each other, as `-g` and `-t` do | backtest (with `globs`) |
| `rdeps:src/a.rs foo` / `deps:` | `rg -F -i 'foo' FILE…` over the files the import graph puts in scope; rg has no graph | backtest (rg's hits kept to the scope's files) |
| A text search's filename hits (no line number) | none; closest is `rg --files \| rg -i 'foo'` | not compared |
| Path matches (`files=N`, the FILES box) | none; closest is `rg --files \| rg -i` with every word required | unit tests |
| Symbols (SYMBOLS, `fcs symbols`) | none: definitions come from tree-sitter, not from text | engine tests |
| References (REFERENCES, `fcs refs`) | none: call sites come from tree-sitter, not from text | engine tests |

`fcs` on the command line, the web UI, the API, gRPC and MCP all run one
shared request path, and a test runs the same searches through web, gRPC
and offline `fcs` and requires identical hits (`tests/frontend_parity.rs`).

## Where fcs agrees with ripgrep and differs from grep and git grep

| Behaviour | fcs | ripgrep | GNU grep | git grep |
|---|---|---|---|---|
| Regex dialect | Rust `regex` | Rust `regex` (PCRE2 with `-P`) | POSIX BRE/ERE (`-E`), PCRE with `-P` | POSIX BRE/ERE, PCRE with `-P` |
| Byte order mark at the start of a file | ignored, so `^` matches line 1 | ignored (kept with `-E none`) | part of line 1, so `^` misses it | part of line 1, so `^` misses it |
| `$` on a line ending in CRLF | matches before the `\r` | only with `--crlf` | no | no |
| Pattern that mentions `\n` | runs over the whole file and reports the line where the match starts | needs `-U` | not supported | not supported |
| Case-insensitive folding | Unicode simple case folding: `Σ`, `σ` and `ς` are the same letter, so are `s` and the long `ſ`, `k` and the Kelvin sign `K`, the micro sign `µ` and `μ`; `ß` is not `ss` and `İ` is not `i` | the same | depends on the locale | depends on the locale |
| Hidden files (`.github/`, `.env.example`) | searched | skipped unless `--hidden` | searched | searched if tracked |
| Files searched | the indexed tree, honouring `.gitignore` | the tree, honouring `.gitignore` | what you name | tracked files only |

These were checked with GNU grep 3.11, git 2.43 and ripgrep 14.1.

## Where fcs differs from all of them

These follow from fcs being a code search server rather than a line filter.

- **Case-insensitive by default.** Plain text ignores case unless the query
  says `case:yes`; grep, git grep and ripgrep match case by default (`rg
  -i` is the fcs default, `rg -s` is `case:yes`). On the API and in `fcs`
  a regex matches case unless it uses `(?i)` or the request says
  `case=false`; in the web UI the MATCH CASE toggle decides for both. The
  backtest passes `-i` to ripgrep whenever fcs ignores case.
- **Operators.** Plain text is a literal, like `rg -F`: several words are
  one phrase, and quotes and a leading `-` are part of the text. The only
  exceptions are whole words with an argument: `file:`, `-file:`,
  `lang:`, `-lang:`, `deps:`, `rdeps:` (and their `-` forms), `case:`
  and `word:`. To search for one of those
  literally, use a regex. The query is trimmed at both ends (`rg ' x'`
  keeps its leading space). See [Query syntax](query-syntax.md).
- **File names match too.** A text search also lists files whose *name*
  matches, as a hit without a line number. grep has no equivalent, so the
  backtest leaves these out of the comparison.
- **Paths match, separately.** When asked (`files=N` on `/api/search`, the
  web UI's **FILES** box, `fcs` on a terminal, MCP `search_code`), the files
  whose *path* matches are returned apart from the content results, which
  do not change. Here the text is split into words, and every word must
  appear in the path as results show it, case-insensitively unless
  `case:yes`; `\` counts as `/`; `word:yes`, `file:`, `lang:` and
  include/exclude globs apply as they do to content. A term holding a `/` also matches the file's absolute path, so a
  pasted `/home/me/repo/src/main.rs` works, but only when the match reaches
  past the indexed folder (`/home/` alone lists nothing). A regex is
  matched against the shown path. This is closest to `rg --files | rg
  PATTERN`; it is not part of the backtest, which compares content only.
- **One result per line.** A line with several matches is one result. For a
  multi-line match, the result is the line where it starts. ripgrep counts
  lines the same way (`rg -c`).
- **Ranked, and capped.** Results come back best first rather than in file
  order. To keep a broad search fast, a request reads a bounded number of
  matches: a budget of 32,768 for every page that ends before result 1000,
  and 100,000 for every page from there on. One file contributes at most
  its best 1,000 hits (by score, not its first 1,000 lines) before result
  1000, and up to the budget after it. Files are read best first, so the
  budget keeps the hits of the most likely files. Because every page in
  each range ranks the same sample, paging with `offset` (MCP's
  `search_code`, `fcs --offset`, the API) returns each hit once, with no
  repeats or gaps; only the switch at result 1000 can differ, and only for
  a search that hit its budget. When a limit is reached, the total is
  shown as "N+" rather than as a count, and the response names the limit
  (`limits_hit`: `match_budget`, `per_file_cap`, `timeout`, `max_files`
  or `indexing`; `fcs` prints it, MCP adds a "Results capped" line, gRPC
  sends an `fcs-limits-hit` header). `indexing` means the server was
  still building or reconciling its index, so files not indexed yet
  could not match. A server may also cap the files read
  (`search.fast_rank_files`, off by default); the header then says
  "Searched N of M files" and SEARCH ALL FILES opens the rest (up to
  `search.max_fast_rank_files`). Symbol search reads the files holding
  the best-matching symbols (at least 200); when there are more, it says
  so in the same way, and SEARCH ALL FILES, MCP's `max_files` or
  `rank=full` read them all. LOAD MORE in the web UI fetches the whole
  list again at the bigger size, up to 1000 results
  (`search.max_page_size`), so every match ends up shown exactly once.
  Results already on screen can move when it does. Past 1000 results it
  fetches the next page instead. Paging stops at offset 100,000 by
  default (`search.max_page_offset`). ripgrep has none of these limits;
  each is reported when it applies, and [Search
  limits](../benchmarks/search-limits.md) gives the measurements behind
  the defaults.
- **Filters need text.** A query made only of filters (`lang:rust`,
  `file:///etc`) is answered with an error saying there is no text to
  find, not an empty result. `case:` and `word:` take `yes` or `no`
  (also `y`/`n`, `true`/`false`, `1`/`0`, `on`/`off`); with any other
  argument the token is text, so `word:count` searches for `word:count`.
- **Non-UTF-8 files are converted.** Files in Latin-1, Windows-1252,
  Shift-JIS or UTF-16 are converted to UTF-8 when they are indexed
  (`transcode_non_utf8`, on by default). ripgrep only does this for UTF-16
  with a byte order mark, or when given `-E`. grep and git grep match raw
  bytes. A file with a NUL byte in its first 8 KB is binary and not
  indexed (ripgrep's test is the same NUL byte, but it looks wherever it
  reads, so a NUL further in makes rg stop at it while fcs searches the
  whole file). Files with these extensions are not read at all: images
  (`png`, `jpg`, `gif`, `ico`, `bmp`, `webp`), archives, fonts, office
  documents, media, compiled objects and bytecode
  (`indexer.extra_binary_extensions` adds more). SVG is text and is
  indexed.
- **Index scope.** Files over `max_file_size` (10 MB by default) and paths
  matching `exclude_patterns` (`node_modules`, `target`, `.git`, `build`,
  `dist`, Python virtual environments) are not indexed, so they are never
  searched, even when git tracks them (ripgrep searches a tracked
  `build/`). Ignore files apply as in ripgrep: `.gitignore`, `.ignore`,
  `.rgignore`, `.git/info/exclude` and your global git excludes
  (`core.excludesFile`), with the same precedence: a rule in an `.rgignore`
  or `.ignore` anywhere above a file beats a `.gitignore` rule next to it; a
  folder that is ignored stays ignored whatever a rule deeper down says
  (`!a/b/` after `/a` brings nothing back, as in git); and `.gitignore`
  files apply outside git repositories and above nested ones, as with
  `rg --no-require-git`. The build and the watcher's per-file check are
  both compared with `rg --files` on random trees of ignore files
  (`tests/discovery_oracle.rs`). When the watcher sees a
  `.gitignore`, `.ignore` or `.rgignore` change, appear or go away (a
  branch switch, say), it re-checks every file under that folder, as a
  restart would. Edits to `.git/info/exclude` or the global excludes file
  are applied at the next start, since nothing under `.git` is watched.
- **Files that can't be read.** ripgrep prints an error for a file it
  cannot open and searches the rest. fcs tries such a file again for about
  half a minute (it is often locked for a moment by an editor, a build or
  a virus scanner), then leaves it out of the index and says so with every
  search ("1 file could not be read and is left out of the index"); the
  server log names it, and it is indexed again when it next changes and
  can be read.

## Known differences

None at the moment. A known difference would be listed here, and marked
`known` with its reason in `benches/rg_backtest.toml`, so the backtest still
reports it without failing.
