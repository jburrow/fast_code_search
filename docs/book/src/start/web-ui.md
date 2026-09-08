# Web UI

The server embeds a single-page UI at its web address (default
<http://127.0.0.1:8080>). Nothing to install or build; it is served from
the binary.

<img src="https://raw.githubusercontent.com/jburrow/fast_code_search/main/docs/images/web-ui.png" alt="The web UI searching for 'trigram'" style="max-width:100%">

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

## Reading results

- Hover a hit to preview the surrounding lines; click **View file** for the
  whole file with the line highlighted (large files open in a window with
  "Show more").
- The **deps** chip on a file opens its importers and imports.
- `j`/`k` move the keyboard selection, Enter opens it, Escape closes any
  dialog; the copy button puts the path on the clipboard.

## Other pages

- **Index** (`/diagnostics.html`): file counts, extension breakdown, the
  configuration in effect, and self-tests that search for sampled files.
- **Docs** (`/docs.html`): the REST reference with live examples.

The UI talks to the same `/api/search` you can call yourself, so anything
it shows is available to scripts.
