# Diff impact

`fcs impact`, `POST /api/impact` and the MCP tool `diff_impact` take a
unified diff and say, for each function, class, method, type or constant it
changes, which files use it. File-level impact (every file that imports a
changed file, as `change_impact` and the dependency explorer show it) lists
thousands of files for a busy module. This lists the files that name the
changed thing.

```console
$ fcs impact                    # your uncommitted changes (git diff HEAD)
$ fcs impact --staged           # what you are about to commit
$ fcs impact main               # everything since main, to the working tree
$ git diff main... | fcs impact --diff -
```

```text
modified pkg/util.py: 2 changed symbols

pkg/util.py:12  function parse_date  (modified, signature)
  3 files (2 reference, 1 code, 0 text only); searched importers that name it
  pkg/api/views.py  [reference]
    41: when = parse_date(request.GET["since"])
  pkg/settings.py  [code]
    3: from pkg.util import parse_date as parse
  tests/test_util.py  [reference, test]
    18: assert parse_date("2024-01-02") == date(2024, 1, 2)
    … 4 more lines
```

(Made-up example.) Piped, the output is `path:line:col:text`, one line per
use, for `vim -q` and other quickfix readers; `--json` gives the full report.
The exit status is 0 when some file uses the changed code, 1 when none does,
2 on error.

## How it works

1. **The diff.** Each file in the diff is matched to an indexed file (by the
   repository root when given, then by the path search results print, then
   by a unique path suffix). The indexed copy is one side of the change; the
   other side is rebuilt by applying the diff to it, forwards or backwards,
   whichever applies. So the same answer comes from `git diff` of a watched
   working tree (the index holds the new side) and from a diff whose old side
   is indexed. A file the index does not hold, or whose indexed copy matches
   neither side, is listed with a note and not analysed.
2. **Changed symbols.** Both sides are parsed with tree-sitter (Python,
   JavaScript, TypeScript, Rust) and each changed line is put in the
   innermost definition that contains it, so a change inside a method is a
   change to the method, not to the class. A change to a definition's
   signature lines is marked `signature`. Definitions present on one side
   only are `added` or `removed`. Lines outside every definition (imports,
   module-level code) are counted on the file and not traced. Other
   languages get the definitions fcs's symbol extractor finds, with a change
   put in the nearest definition above it (and a note saying so).
3. **Who uses it.** For a function, class, type or constant, fcs walks the
   import graph from the changed file along "imported by" edges, but only
   into files that mention the name (checked with the trigram index first,
   then the recorded references, then the text) and into package entry files
   (`__init__.py`, `index.*`, `mod.rs`, `lib.rs`, `main.rs`), which re-export.
   An import that renames it (`import { parse as p }`, `from x import parse
   as p`, `use x::parse as p`) adds the new name to the search. A
   constructor (`__init__`, `constructor`) is searched as its class.
4. **Members.** A method or field is used through a value whose type fcs
   does not know, so it is searched by name among every file that imports
   its file, directly or not. Files that also name the class come first.
   Methods are listed only where fcs recorded a reference to the name.
5. **Tiers.** Each use line is a `reference` (a recorded call, type, value or
   attribute reference), `code` (the name as a whole word in code, such as
   an import line or a JSX tag) or `text` (only in a string or a comment: a
   dotted path in a settings file is a real use, prose is not). A file's
   tier is its best line's. Files are ordered by tier, then (members)
   whether they name the class, then code before tests, then by how widely
   used the file is, then path.

Nothing is dropped: every file found is counted in the totals and in the
tier counts. What is listed is capped (200 files per symbol and 3 lines per
file by default; `-n`/`--lines`, `max_files`/`max_lines`, 0 for all), and a
capped list says so. At most 500 changed symbols are searched per request;
the rest are listed without users, with a note.

## Measured accuracy

On Django at 79502d0, for the 248 most recent commits that change Python files (the last 400 commits of history), each
commit's diff (old side: the commit's parent, new side: the indexed tree)
was analysed and compared with the files where [Jedi](https://jedi.readthedocs.io)
resolves a use to the changed definition (`src/check/oracle/py_impact.py`).
**Found** is the share of Jedi's files that fcs lists, **right** the share
of listed files Jedi confirms. Files the diff itself changes are left out
of both. CI repeats this on every change to the impact code
(`.github/workflows/impact-oracle.yml`, `examples/impact_oracle.rs`).

| Symbols | Scored | Found | Right |
|---|---|---|---|
| Functions, classes, constants | 226 | 100% (1,267 of 1,267 files) | 79.5% |
| Methods and fields | 131 | 100% (212 of 212) | 13.0% |

"Right" is a lower bound: Jedi misses dynamic uses that fcs's text tier
finds, such as dotted paths in settings and `mock.patch("pkg.mod.name")`.
For members, fcs finds every file but also lists the files that call a
method of the same name on something else; files naming the class come
first so they are read first.

The evaluation before this was built
(Django and Sentry, Python and TypeScript, against Jedi and the TypeScript
compiler's `findReferences`) found the same picture: functions and classes
100% found and 83–91% right with all three tiers; references alone miss
two thirds of TypeScript uses (JSX tags, re-exports, type positions), which
is why the code and text tiers are kept.

## Speed

On Django (5,700 files), the analysis of one commit's diff takes 49 ms
median and 309 ms p95 (commits that change many methods at once are the
slow ones). The walk is lazy: a file is read only when the trigram index
says it may hold the name. Measured during the evaluation on 348,000 files
(16 copies of Sentry), one changed symbol took 0.5 ms median and 3 ms p95;
a synthetic hub function called from 200,000 of 400,000 files took 2 s,
reading each caller once.

## What it does not do

- **No types.** A method is found by name, so every `save()` in the
  importers is listed for a change to one `save()` (class-named files first).
  Uses through `getattr`, dependency injection, or a method passed as a
  value are not references, so they are found only by the code tier
  (functions, classes) or not at all (methods).
- **One level.** It lists the direct users of each changed symbol. It does
  not then follow the functions those users define to their own users.
- **Import graph languages only for the walk.** Python, JavaScript,
  TypeScript and Rust have import edges. In other languages (Go, Java, C#,
  C, C++ and the rest) a symbol is searched by name across the whole index,
  with a note, so same-named things elsewhere are listed too. C and C++
  record no references, so their uses are `code` or `text` only. Per-language
  coverage of references: [Reference support](reference-support.md); of
  import edges: [Dependency graph support](graph-support.md).
- **Language-called methods.** A change to `__eq__`, `__str__` or
  `__iter__` is found only where it is named explicitly; the note says so.
- **Module-level changes** (a changed import, a top-level statement) are
  counted per file, not traced.
- **The index must hold one side.** With file watching on (the default),
  the index is the working tree, so `fcs impact` (uncommitted changes) and
  `fcs impact REV` (REV to the working tree) work. A diff between two other
  commits works only if the index holds one of them; otherwise each file
  says "matches neither side".
- **gRPC** has no impact call; use REST, MCP or `fcs`.
- `fcs impact` is a subcommand, so searching for the word `impact` needs
  `fcs search impact`.
