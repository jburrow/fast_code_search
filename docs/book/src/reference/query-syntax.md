# Query syntax

The same syntax everywhere: the web UI's search box, `fcs`, and the `q`
parameter of the API. It applies to plain-text queries; with `regex=true`
(`fcs -e`) the whole query is a regular expression instead. For how the
results compare with grep, ripgrep and git grep, see [Matching compared
with grep and ripgrep](matching.md).

| Write | To get |
|---|---|
| `fn main` | Files containing **every** term. Lines holding the phrase `fn main` rank first, then lines holding both terms, then the rest. |
| `"fn main"` | One term: the exact phrase, spaces included. A quote opens a phrase only when it wraps a whole word; other quotes are searched for as written, so `{ "success": True` finds that text with its quotes. |
| `-tests` | Drop files that contain `tests`. Only a `-` before a letter, `_` or quote negates, so `->`, `-1` and `--flag` are ordinary terms. |
| `file:src/` | Only paths under a `src` directory. A bare word matches anywhere in the path (`file:parser`); globs are used as written (`file:*.rs`, `file:crates/**/tests/**`). |
| `-file:vendor` | Never paths matching the pattern. |
| `lang:rust`, `-lang:py` | Only / never files of that language (by extension: `rs`/`rust`, `py`/`python`, `ts`, `js`, `go`, `java`, …). |
| `case:yes` | Match case exactly (the default is case-insensitive). |
| `word:yes` | Whole words only. |

Operators can be combined: `Widget file:src/ -lang:ts -tests case:yes`.

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
- A regex is case-sensitive, unlike plain search: `true` does not find
  `True`. `(?i)` makes the pattern case-insensitive; `case=false` on the
  API adds it for you.
- `{ } ( ) [ ] . * + ? | ^ $ \` are syntax; escape one with `\` to match
  it (`\{`, `\.`). For code exactly as written, search without regex.
- Spaces are exact: `\{ "a"` misses `{"a"`. Use `\s*` for any amount of
  space, including none.
- In the web UI the **?** next to REGEX has runnable examples, and a
  search that finds nothing offers close variants that do.
- Patterns are compiled with size limits; an absurd one is rejected with a
  400 rather than run.
