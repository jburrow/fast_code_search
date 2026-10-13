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
| `case:yes` | Match case exactly (the default is case-insensitive). |
| `word:yes` | Whole words only. |

Operators can be combined: `Widget file:src/ -lang:ts case:yes`. They are
whole words with an argument: `file:` alone, or `url=file:x`, is text.
Operators between words join the words with one space, so `fn file:src/
main` searches for `fn main` under `src/`. The ends of the query are
trimmed; spaces inside it are kept. To search for operator-looking text
such as `file:x`, use a regex.

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
