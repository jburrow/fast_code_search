# `fcs` — the command-line client

`fcs` searches the index kept by a running `fast_code_search_server` from a
terminal, with grep-style output and exit codes, so it slots into editors,
shell pipelines and scripts. When no server is running it can search the
on-disk index directly.

![fcs in a terminal: a phrase search, symbol definitions, references, a regex and a files-only listing](images/demos/cli.gif)

```bash
fcs 'fn main'                    # lines holding the phrase, like rg -F -i
fcs -- -Wall                     # a query starting with - goes after --
fcs -e 'fn\s+\w+\(' -g '*.rs'    # regex, Rust files only
fcs refs SearchEngine            # call sites and type mentions of an identifier
fcs symbols parse_query          # definitions only
fcs -l TODO | xargs $EDITOR      # files containing TODO
vim -q <(fcs --format vimgrep TODO)   # results as a quickfix list
fcs status                       # is the server up, what does it hold, how current is it
fcs mcp                          # MCP over stdio, for coding agents
fcs check                        # compare fcs with ripgrep and the import resolvers on your code
```

## Design

**Client first.** The server holds the index in memory, watches the tree and
answers a query in milliseconds. `fcs` sends every query to it over the REST
API (`/api/search`) rather than re-reading files, so a search costs the same
as a request to the web UI. The intended setup on a developer machine is one
server started at login (see [RUN-AT-STARTUP.md](RUN-AT-STARTUP.md)) and any
number of `fcs` invocations.

**Same semantics as the web UI.** The query syntax, modes and result shape
are the server's. `fcs` adds nothing the API does not have, so what you see
in a terminal is what the UI would show for the same query.

**Offline fallback.** When the server does not answer and an index file is
known (`index_path` in the configuration, or `--index-path`), `fcs` loads that
file in-process and runs the same engine calls the server does. It is
read-only: nothing is written back. A note on stderr says the fallback was
used, because the on-disk index may be behind the working tree (changed
files are re-read, deleted ones dropped, but new files are not discovered).
`--offline` forces this path; `--no-offline` turns the fallback into an
error.

**Grep conventions.** Output is `path:line:col:text` when stdout is not a
terminal (what `vim -q`, VS Code's terminal links and `grep`-aware tools
expect) and grouped by file with colour on a terminal. Exit status is 0 when
there were matches, 1 when there were none, 2 on error. A closed pipe
(`fcs … | head`) is not an error.

## Finding the server

In order: `--server URL`, `$FCS_SERVER`, the `web_address` of the
configuration file, then `http://127.0.0.1:8080`. A bind address such as
`0.0.0.0:8080` is turned into a loopback address. The configuration file is
`--config FILE`, else `$FCS_CONFIG`, `./fast_code_search.toml`, then
`~/.config/fast_code_search/config.toml` (the same order as the server).

## Options

| Option | Meaning |
|---|---|
| `QUERY` | Literal text, like `rg -F`: several words are one phrase, quotes and a leading `-` are part of it (put such a query after `--`). `file:PATTERN`, `-file:PATTERN`, `lang:rust`, `case:yes`, `word:yes` as in the web UI. |
| `-e`, `--regex` | Treat the query as a regular expression (matched one line at a time; mention `\n` for multi-line patterns). |
| `-s`, `--case-sensitive` / `-i`, `--ignore-case` | Override case handling (plain text ignores case by default, a regex matches it; `-e -i` ignores it). |
| `-w`, `--word` | Whole words only. |
| `-n`, `--max N` | Hits to print (default 50, up to the server's `search.max_page_size`, 1,000). When more exist, a note on stderr says so and gives the `--offset` for the next page. With `-l`, the most files to list (default: every matching file). Not grep's line-number flag. |
| `--offset N` | Skip the first N hits; ordering is deterministic, so `--offset 50` is page two. Up to the server's `search.max_page_offset` (100,000 by default). |
| `-C`, `--context N` | Lines of context around each hit (up to 10). |
| `-g`, `--glob GLOB` | Only files matching the glob (repeatable). `src/**/*.rs`, `*.py`, `tests` (a bare name matches anywhere in the path). |
| `--exclude GLOB` | Skip files matching the glob (repeatable). |
| `--rank auto\|fast\|full` | Every mode reads candidate files best first until enough matches are collected to rank; `auto` and `fast` honour a file limit, `full` never stops at one. |
| `--max-files N` | Most candidate files to read (default: the server's `search.fast_rank_files`, no limit; up to its `search.max_fast_rank_files`). |
| `--timeout-ms MS` | Stop scanning after MS and return the best hits so far. |
| `-l`, `--files-with-matches` | Print every matching file path once, best first. `fcs` pages through all the hits to find them, so the list is complete unless `-n` or the server's `search.max_page_offset` stops it (then stderr says so). |
| `--format grouped\|vimgrep\|json`, `--json` | Output layout (default: grouped on a terminal, vimgrep when piped). JSON is the server's response object. |
| `--absolute` | Print absolute paths (mapped through the configured roots). |
| `--color auto\|always\|never` | ANSI colour; `auto` means a terminal without `NO_COLOR`. |
| `-q`, `--quiet` | No summary or fallback notes on stderr. |
| `--server URL`, `--config FILE` | Where the server and configuration are. |
| `--offline`, `--no-offline`, `--index-path FILE` | Control the on-disk fallback. |

Subcommands: `search` (the default; `fcs QUERY` works), `refs NAME`
(references: call sites, type mentions), `symbols QUERY` (definitions only),
`status`.

## Output formats

Grouped (terminal):

```
fast_code_search/src/search/engine/mod.rs
  664: impl SearchEngine {
  601 [def]: pub struct SearchEngine {
```

On a terminal, up to five files whose *path* matches the query are listed
first, under `Files (N paths match)`, so `fcs src/web/api.rs` finds the
file even when no line mentions it. Piped and `-l` output leave them out.

vimgrep (piped): `path:line:col:text`, column 1-based; with `-C`, context
lines use `-` as the separator and `--` ends each block, as `grep -C` does.

JSON: the object `/api/search` returns (`results`, `total_results`,
`has_more`, `total_matches`, `truncated_by_budget`, `elapsed_ms`, …). Each
result carries `line_number`, `match_column`, `line_match_start` /
`line_match_end` (byte offsets in the full line) and `match_type`
(`TEXT`, `SYMBOL_DEFINITION`, `SYMBOL_REFERENCE`).

## Editor integration

- **Vim / Neovim**: `:cexpr system('fcs --format vimgrep ' . shellescape(q))`,
  or `set grepprg=fcs\ --format\ vimgrep grepformat=%f:%l:%c:%m` and use
  `:grep`.
- **VS Code**: run `fcs` in the integrated terminal; `path:line:col` output
  is clickable. With `--absolute` the links work from any working directory.
- **fzf**: `fcs -l QUERY | fzf | xargs $EDITOR`.

## MCP for coding agents

`fcs mcp` serves the [Model Context Protocol](https://modelcontextprotocol.io)
over stdio by relaying each message to the server's `/mcp` endpoint, for
agents that launch a command rather than connect to a URL:

```json
{ "mcpServers": { "fast_code_search": { "command": "fcs", "args": ["mcp"] } } }
```

It finds the server like any `fcs` command (`--server`, `$FCS_SERVER`, the
configuration). Stdout carries only protocol messages; when the server is
down each request is answered with an error saying so. Clients that speak
HTTP can use `http://127.0.0.1:8080/mcp` directly. The tools are listed in
[API.md](API.md#mcp-coding-agents).

## When the index is behind

A server running with `watch = false` (watching is on by default) only knows the files as
they were at its last scan. `fcs` then ends a search with a note on stderr,
such as `fcs: Index last synced with disk 2 h 0 min ago (…); file watching is
off, so files written since then may be missing or out of date.` `-q` hides
it, and `fcs status` always reports when the index last synced and whether
watching is on. Restart the server to rescan, or set `watch = true` under
`[indexer]`. See [API.md](API.md#index-freshness).

## Checking fcs on your code

`fcs check [PATH…]` indexes the folders (default: the configuration's
`paths`, else the current folder) afresh in memory, compares 200 queries
made up from the files with `rg -F`, and compares the import graph with
grimp (Python) and TypeScript's resolver when they are installed. It prints
a markdown report and exits 1 when a search differs from ripgrep. Options:
`--queries N`, `--seed N` (repeat a run), `--rg PATH`, `--no-search`,
`--no-graph`. It needs no server. See the book's
[correctness page](book/src/benchmarks/correctness.md#on-your-own-code-fcs-check).

## Exit status

| Code | Meaning |
|---|---|
| 0 | At least one match (or, on a terminal, a matching path) was printed. |
| 1 | The search ran and found nothing. |
| 2 | Error: bad arguments, invalid regex, server error, no server and no index. |

## Limitations

- The fallback needs an `index_path`; a server started without one leaves
  nothing to search offline.
- Offline searches load the index each time (a few seconds for tens of
  thousands of files) and do not discover files added since the last save.
- On a terminal, result paths are workspace-relative (`root-name/…`) unless
  `--absolute` can map them through the configuration's `paths`. Piped
  (vimgrep) output and `-l` print paths relative to the current folder when
  the configuration's roots are known, so `fcs -l foo | xargs sed …` opens
  the right files (absolute for files outside the current folder); without
  the roots they stay workspace-relative.
