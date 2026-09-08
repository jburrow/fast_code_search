# Configuration cookbook

Recipes for the situations that come up most. Keys are documented in
[API and configuration](../imported/api.md#configuration-file); the
complete template with every default is in
[Configuration template](../reference/config-template.md).

## One developer, many repositories

List each checkout as its own root. The first path component of every
result is the root's directory name, so `proj/src/main.rs` is unambiguous
even when several roots have a `src/`.

```toml
[indexer]
paths = ["~/work/api", "~/work/web", "~/work/tools", "~/oss/tokio"]
exclude_patterns = ["**/node_modules/**", "**/target/**", "**/.git/**",
                    "**/build/**", "**/dist/**", "**/.venv/**", "**/__pycache__/**"]
index_path = "~/.local/share/fast_code_search/index.fcsidx"
watch = true
```

Roots that do not exist at start are skipped with a warning and picked up
on the next start; a root that is unreachable when a saved index loads
(an unmounted share) keeps its files rather than dropping them.

## A monorepo

One root, and excludes that name the build output of every toolchain in
it. Patterns are globs matched against the full path; a bare directory
name (`node_modules`) matches at any depth. Excluded directories are never
read, so a generous list costs nothing.

```toml
paths = ["/srv/monorepo"]
exclude_patterns = ["**/node_modules/**", "**/target/**", "**/.git/**",
                    "**/bazel-*/**", "**/out/**", "**/*.min.js", "**/*.map",
                    "**/package-lock.json", "**/Cargo.lock"]
respect_gitignore = true
max_file_size = 2097152          # 2 MB: skips generated blobs
checkpoint_interval_files = 20000
```

## Only some languages

```toml
include_extensions = ["rs", "py", "ts", "tsx", "toml", "md"]   # a leading dot is fine too
```

Anything not listed is not indexed at all.

## Skip generated or vendored code but keep it browsable

Use `exclude_patterns` for whole trees and `exclude_files` for individual
paths. A `.gitignore` in any indexed tree is honoured as well (also outside
git repositories), so what your VCS ignores, the index ignores.

## Change a rule on a running index

Edit the file and restart the server. Files that are no longer eligible
under the new patterns, extensions, size cap or `.gitignore` are dropped
when the saved index is reconciled; the log reports how many.

## Shared server for a team

Bind to an address others can reach only behind something that
authenticates: the server has no login and returns file contents.

```toml
[server]
address = "0.0.0.0:50051"
web_address = "0.0.0.0:8080"
cors_origins = ["https://ide.example.com"]   # only if a page on another origin calls the API
max_concurrent_searches = 64
request_timeout_secs = 30
```

See [Deployment](../imported/deployment.md) for a reverse-proxy example and
the system-wide systemd unit, and the [security policy](../imported/security.md).

## Where the config file lives

`--config FILE` wins; otherwise `$FCS_CONFIG`, then `./fast_code_search.toml`,
then `~/.config/fast_code_search/config.toml` (`%APPDATA%\fast_code_search\config.toml`
on Windows). `~` is expanded in every path-valued key; relative paths
resolve against the config file's own directory. Both the server and `fcs`
use the same lookup.
