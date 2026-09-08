# Query syntax

The same syntax everywhere: the web UI's search box, `fcs`, and the `q`
parameter of the API. It applies to plain-text queries; with `regex=true`
(`fcs -e`) the whole query is a regular expression instead.

| Write | To get |
|---|---|
| `fn main` | Files containing **every** term. Lines holding the phrase `fn main` rank first, then lines holding both terms, then the rest. |
| `"fn main"` | One term: the exact phrase, spaces included. |
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
- `(?i)` makes the pattern case-insensitive; the `case` parameter does the
  same.
- Patterns are compiled with size limits; an absurd one is rejected with a
  400 rather than run.
