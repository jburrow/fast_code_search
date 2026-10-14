# Query syntax

The same syntax everywhere: the web UI's search box, `fcs`, and the `q`
parameter of the API. It applies to plain-text queries; with `regex=true`
(`fcs -e`) the whole query is a regular expression instead. For how the
results compare with grep, ripgrep and git grep, see [Matching compared
with grep and ripgrep](matching.md).

The text you type is searched for as written, like `rg -F`. The operators
below are the only exceptions; everything else, spaces, quotes and dashes
included, is part of the text.

| Write | To get |
|---|---|
| `fn main` | Lines holding the phrase `fn main`, with exactly one space. Several words are one phrase. |
| `"fn main"` | Lines holding `"fn main"` with its quotes. Quotes are text: `{ "success": True` finds that text as written. |
| `-Wall` | Lines holding `-Wall`. A leading `-` is text too (`->`, `-1`, `--flag`). On the command line put such a query after `--`: `fcs -- -Wall`. |
| `file:src/` | Only paths under a `src` directory. A bare word matches anywhere in the path (`file:parser`); globs are used as written (`file:*.rs`, `file:crates/**/tests/**`). |
| `-file:vendor` | Never paths matching the pattern. |
| `lang:rust`, `-lang:py` | Only / never files of that language (by extension: `rs`/`rust`, `py`/`python`, `ts`, `js`, `go`, `java`, …). |
| `rdeps:src/search/ranking.rs` | Only that file and the files that import it. See [Search by imports](#search-by-imports-deps-and-rdeps). |
| `deps:app/main.py` | Only that file and the files it imports. |
| `case:yes` | Match case exactly (the default is case-insensitive). |
| `word:yes` | Whole words only. |

Operators can be combined: `Widget file:src/ -lang:ts case:yes`. Filters
of different kinds narrow each other (`file:src/ lang:rust` is the Rust
files under `src/`); repeating one kind widens it (`lang:rust lang:py` is
either language, `file:src/ file:lib/` either folder). They are
whole words with an argument: `file:` alone, or `url=file:x`, is text.
Operators between words join the words with one space, so `fn file:src/
main` searches for `fn main` under `src/`. The ends of the query are
trimmed; spaces inside it are kept. To search for operator-looking text
such as `file:x`, use a regex.

## Search by imports: `deps:` and `rdeps:`

These limit a search to the files the import graph connects to one file,
so the question "where is this used, among the code a change to this file
could affect?" is one query:

| Write | To search |
|---|---|
| `Weight rdeps:src/search/ranking.rs` | `ranking.rs` and the files that import it |
| `Weight rdeps:src/search/ranking.rs:2` | the same, up to two imports away |
| `Weight rdeps:src/search/ranking.rs:*` | everything that reaches `ranking.rs`, at any distance |
| `config deps:app/main.py` | `main.py` and the files it imports (`:N` and `:*` as above, 10 at most) |
| `Weight rdeps:src/search/` | every file under the folder, and the files that import any of them |
| `Weight -rdeps:src/search/ranking.rs` | everything except those files |

- **The path** is the file's path as results show it, or any trailing part
  of it that names one file (`rdeps:ranking.rs` when only one file ends
  that way). A path that matches no file, or several, is an error that
  lists them; so is a file in a language the graph does not cover (edges
  exist for Rust, Python and JavaScript/TypeScript, see [Dependency graph
  support](graph-support.md)).
- **Combining**: repeating `rdeps:` (or `deps:`) searches the union; a
  `deps:` and an `rdeps:` together search the files in both; both narrow
  `file:` and `lang:`, and they narrow each other.
- **Package entry files are passed through, carefully.** An import of a
  package usually lands on its entry file (`mod.rs`, `lib.rs`, `main.rs`,
  `__init__.py`, `index.ts`/`index.js`), not on the module behind it. So an
  edge from an entry file to a module of its own package does not count as
  a hop, and a file reached across one is kept only if it names something
  the module on the other side defines (a function, type, constant or
  macro as a whole word, or the module itself in a path such as
  `ranking::` or `./ranking`). `rdeps:src/search/ranking.rs` therefore
  finds the files that `use crate::search::RankingWeights`, but not every
  importer of `crate::search`. Only one entry file is crossed in a row
  (`ranking.rs` to `search/mod.rs`, never on to `lib.rs` for free). A child
  module that imports its parent (Rust `use super::*`) is an ordinary hop:
  use `:2`.
- **Negated filters walk strictly.** `-rdeps:` and `-deps:` follow import
  edges only, with no pass-through, so they never leave out a file that
  does not depend on the anchor.
- **Rust paths without a `use`** (`crate::search::fold::fold_str(…)`,
  `super::util::Thing`) count as imports of the module they name.
- **The scope is only as complete as the graph.** The results header (and
  the `scope` object of the API, MCP's text, `fcs`'s summary) says how many
  files were searched, and warns when imports in those files are written
  as project paths (`./x`, `crate::x`) but resolve to no file: the scope
  may be missing whatever they point at.
- **Ranking**: nearer files rank a little higher; each hit's `hops` (0 for
  the file itself and its package entry) is in the API response and the
  row's tooltip.
- **Everywhere**: in the query on any surface; for a regex, whose query is
  all pattern, as the `deps` / `rdeps` API parameters (semicolon-separated,
  a leading `-` to leave out), `fcs --deps PATH[:N]` / `--rdeps`, MCP
  `search_code`'s `deps` / `rdeps`, gRPC's `deps` / `rdeps` fields. In the
  web UI they are the FILTER panel's **Files that import** and **Files
  imported by** boxes, and the dependency popover and the explorer's
  **Search importers** / **Search its imports** buttons fill them in.

Inside the scope, matching is exactly what it is without one: a scoped
search finds what ripgrep finds over the same files.

## Case

Plain text ignores case unless the query says `case:yes`. A regex matches
case unless it starts with `(?i)` or the request says to ignore it. The
web UI's **MATCH CASE** toggle (`case` on the API, `fcs -s` / `-i`, MCP
`case_sensitive`) sets it for every mode: off, upper and lower case are
the same in plain text and regex alike; on, they are matched exactly. The
results header always says which applied ("ignoring case" or "matching
case").

## Modes

| Mode | UI / CLI / API | Returns |
|---|---|---|
| Text (default) | — / `fcs` / — | Lines matching the query, ranked |
| Regex | REGEX / `fcs -e` / `regex=true` | Rust `regex` syntax, matched one line at a time. To span lines, mention `\n` or set `(?s)` |
| Symbols | SYMBOLS / `fcs symbols` / `symbols=true` | Definitions only (functions, types, classes, constants, …) plus filename matches |
| References | REFERENCES / `fcs refs` / `references=true` | Call sites and type mentions of the identifier |

References are exact and case-sensitive on the identifier; `file:` and
`lang:` still narrow the files.

## Regex notes

- Literals in the pattern pre-filter through the trigram index; a pattern
  with no required literal of three or more characters scans every file.
- `^` and `$` anchor at line boundaries (CRLF included); `\s` does not
  cross a line break unless the pattern is a multi-line one.
- On the API and `fcs -e`, a regex is case-sensitive unless it starts
  with `(?i)` or the request says `case=false` (`fcs -e -i`). In the web
  UI it follows MATCH CASE, like plain search.
- `{ } ( ) [ ] . * + ? | ^ $ \` are syntax; escape one with `\` to match
  it (`\{`, `\.`). For code exactly as written, search without regex.
- Spaces are exact: `\{ "a"` misses `{"a"`. Use `\s*` for any amount of
  space, including none.
- In the web UI the **?** next to REGEX has runnable examples, and a
  search that finds nothing offers close variants that do.
- Patterns are compiled with size limits; an absurd one is rejected with a
  400 rather than run.
