# Web UI

The server embeds a single-page UI at its web address (default
<http://127.0.0.1:8080>). Nothing to install or build; it is served from
the binary.

<img src="https://raw.githubusercontent.com/jburrow/fast_code_search/main/docs/images/demos/search.gif" alt="Typing TrigramIndex: the struct definitions rank first, the file viewer opens at the match, then a quoted phrase search for fn main" style="max-width:100%">

## Searching

- Type and press Enter (or wait: the page searches as you type after a short
  pause). `Ctrl`/`Cmd`+`K` focuses the box from anywhere; `/` does too.
- **REGEX**, **SYMBOLS** and **REFERENCES** switch modes; **FILTER** opens
  page size, ranking mode, context lines and include/exclude globs.
- The [query syntax](../reference/query-syntax.md) works in the box:
  `"exact phrase"`, `-term`, `file:src/`, `lang:rust`, `case:yes`.
- Results are grouped by file, best hit first, with every query term
  highlighted from the server's match offsets. **Load more** pages through
  the rest; the summary line says how many matches exist and whether the
  scan was cut short by its budget.
- The URL carries the query and modes, so a link reproduces a search; Back
  returns to the previous one.

**SYMBOLS** keeps only the lines that define the name; **REFERENCES**
lists every place it is used:

<img src="https://raw.githubusercontent.com/jburrow/fast_code_search/main/docs/images/demos/references.gif" alt="SearchEngine as a text search, then with SYMBOLS, then with REFERENCES" style="max-width:100%">

## Regex help

- The **?** next to **REGEX** opens a cheat-sheet: common patterns you can
  run with one click, and the rules that catch people out (regex is
  case-sensitive, `{ ( . *` and friends need a `\` to match literally,
  spaces are exact).
- When a search finds nothing, the page tries close variants (flexible
  spacing, ignoring case, the same text without regex, or regex for a
  plain query that looks like one) and offers the ones that find
  something.
- An invalid regex shows where the pattern is wrong and offers to search
  the text literally or with its special characters escaped.

<img src="https://raw.githubusercontent.com/jburrow/fast_code_search/main/docs/images/demos/regex.gif" alt="The regex cheat-sheet, an unclosed group with suggested fixes, and a regex that finds nothing with an Ignore case suggestion" style="max-width:100%">

## Reading results

- Hover a hit to preview the surrounding lines; click **View file** for the
  whole file with the line highlighted (large files open in a window with
  "Show more").
- The **deps** chip on a file opens its importers and imports. The tree
  icon on a result (and **Explore imports** in the file viewer) opens the
  file in the dependency explorer.
- `j`/`k` move the keyboard selection, Enter opens it, Escape closes any
  dialog; the copy button puts the path on the clipboard.

## Dependency explorer

**Graph** (`/graph.html`) shows the import graph around one file, next to
its source. Pick a file on the left (most connected first) and switch view:

- **Neighbourhood**: what the file imports on the left, what imports it on
  the right, up to four hops each way. Busy levels fold into "+N more";
  click to list them.
- **Impact**: every file a change could reach, level by level, and the test
  files among them.
- **Path**: the shortest chain of imports between two files.
- **Module map**: folders laid out so each only imports folders to its
  left; red marks import cycles.

Click a box to read the file and a line to open the import statement it
stands for; double-click (or Enter) to centre the graph on it. Import lines
in the source are highlighted and link to their target; imports of
packages and the standard library are marked *external*. **Search these
files** runs a keyword search limited to the files on screen.

Rust `mod foo;` declarations (and `pub use foo::X` re-exports of a child
module) are hidden by default since they are structure rather than
dependencies; **Show mod declarations** brings them back. Imports are
resolved for Rust, Python and JavaScript/TypeScript.

<img src="https://raw.githubusercontent.com/jburrow/fast_code_search/main/docs/images/demos/graph.gif" alt="The dependency explorer centred on src/search/engine/mod.rs, previewing ranking.rs, then the Impact view" style="max-width:100%">

## Other pages

- **Index** (`/diagnostics.html`): file counts, extension breakdown, the
  configuration in effect, and self-tests that search for sampled files.
  Its **Service** card says whether the web UI and gRPC started, where the
  index is saved (or that it is memory only or read-only), and any startup
  or storage problems. When there are some, the search page shows "Server
  started with N problems" with a link to it.
- **Docs** (`/docs.html`): the REST reference with live examples.

The UI talks to the same `/api/search` you can call yourself, so anything
it shows is available to scripts.
