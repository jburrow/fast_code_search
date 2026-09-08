# First index, first search

## 1. Write a configuration

```bash
mkdir -p ~/.config/fast_code_search
fast_code_search_server --init ~/.config/fast_code_search/config.toml
```

Open the file and set `paths` to the directories you want searchable. The
defaults for everything else are sensible; the keys worth knowing early:

```toml
[indexer]
paths = ["~/work", "~/src/another-repo"]     # ~ expands; relative paths resolve against this file
index_path = "~/.local/share/fast_code_search/index.fcsidx"   # persist the index across restarts
watch = true                                   # follow edits, renames and deletes
```

Both binaries find this file on their own (`$FCS_CONFIG`,
`./fast_code_search.toml`, then `~/.config/fast_code_search/config.toml`),
so you rarely need `--config`. The full key list is in
[API and configuration](../imported/api.md#configuration-file).

## 2. Start the server

```bash
fast_code_search_server
```

It logs how many files it discovered and indexes them in batches; searches
work while the build is still running. A first build of a 60k-file
workspace takes a couple of minutes (tree-sitter symbol extraction is most
of it); later starts load the saved index in seconds and reconcile what
changed while the server was down.

## 3. Search

From another terminal:

```bash
fcs 'fn main'                 # lines holding both words; the phrase ranks first
fcs symbols SearchEngine      # definitions only
fcs refs SearchEngine         # call sites and type mentions
fcs -e 'fn\s+\w+\(' -g '*.rs' # a regex, Rust files only
fcs status                    # is the server up, what does it hold
```

Or open <http://127.0.0.1:8080> for the web UI, or call the API:

```bash
curl "http://127.0.0.1:8080/api/search?q=fn%20main&max=5"
```

## 4. Keep it running

The server is meant to stay up: the index lives in memory and the watcher
keeps it current. [Run at startup](../imported/run-at-startup.md) shows the
per-user service on Linux, macOS and Windows.
