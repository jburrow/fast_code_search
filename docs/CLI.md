# `fcs` — the command-line client

`fcs` searches the index kept by a running `fast_code_search_server` from a
terminal, with grep-style output and exit codes, so it slots into editors,
shell pipelines and scripts. When no server is running it can search the
on-disk index directly.

```bash
fcs 'fn main'                    # lines holding both words; the phrase ranks first
fcs -e 'fn\s+\w+\(' -g '*.rs'    # regex, Rust files only
fcs refs SearchEngine            # call sites and type mentions of an identifier
fcs symbols parse_query          # definitions only
fcs -l TODO | xargs $EDITOR      # files containing TODO
vim -q <(fcs --format vimgrep TODO)   # results as a quickfix list
fcs status                       # is the server up, what does it hold
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
| `QUERY` | Words must all appear in a file; lines with the phrase rank first. `"quoted phrase"`, `-term`, `file:PATTERN`, `-file:PATTERN`, `lang:rust`, `case:yes`, `word:yes` as in the web UI. |
| `-e`, `--regex` | Treat the query as a regular expression (matched one line at a time; mention `\n` for multi-line patterns). |
| `-s`, `--case-sensitive` / `-i`, `--ignore-case` | Override case handling (plain text is case-insensitive by default). |
| `-w`, `--word` | Whole words only. |
| `-n`, `--max N` | Hits to print (default 50, up to 1000). |
| `--offset N` | Skip the first N hits; ordering is deterministic, so `--offset 50` is page two. |
| `-C`, `--context N` | Lines of context around each hit (up to 10). |
| `-g`, `--glob GLOB` | Only files matching the glob (repeatable). `src/**/*.rs`, `*.py`, `tests` (a bare name matches anywhere in the path). |
| `--exclude GLOB` | Skip files matching the glob (repeatable). |
| `--rank auto\|fast\|full` | Ranking mode; `auto` reads every candidate up to 5,000 files and samples by file metadata beyond that. |
| `--timeout-ms MS` | Stop scanning after MS and return the best hits so far. |
| `-l`, `--files-with-matches` | Print file paths only, once each. |
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

## Exit status

| Code | Meaning |
|---|---|
| 0 | At least one match was printed. |
| 1 | The search ran and found nothing. |
| 2 | Error: bad arguments, invalid regex, server error, no server and no index. |

## Limitations

- The fallback needs an `index_path`; a server started without one leaves
  nothing to search offline.
- Offline searches load the index each time (a few seconds for tens of
  thousands of files) and do not discover files added since the last save.
- Result paths are workspace-relative (`root-name/…`) unless `--absolute`
  can map them through the configuration's `paths`.
