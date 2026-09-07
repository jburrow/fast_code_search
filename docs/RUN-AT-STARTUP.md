# Running the server at login

The command-line client (`fcs`), the web UI and editor integrations all
expect one `fast_code_search_server` per machine, holding the index in memory
and watching your checkouts. This guide starts it automatically for your
user account at login, on Linux (systemd), macOS (launchd) and Windows
(Task Scheduler). Ready-made unit files are in [`deploy/`](../deploy).

For a shared, always-on service under its own account see the systemd
section of [DEPLOYMENT.md](DEPLOYMENT.md).

## 1. Install the binaries

```bash
cargo build --release
mkdir -p ~/.local/bin
cp target/release/fast_code_search_server target/release/fcs ~/.local/bin/
```

Make sure `~/.local/bin` is on your `PATH` (it is by default on most Linux
distributions and in recent macOS shells; otherwise add it in your shell
profile). Or download a release archive from GitHub and unpack the two
binaries there.

## 2. Write the configuration

```bash
mkdir -p ~/.config/fast_code_search ~/.local/share/fast_code_search
fast_code_search_server --init ~/.config/fast_code_search/config.toml
```

Edit the file. The settings that matter for a developer machine:

```toml
[server]
address = "127.0.0.1:50051"      # gRPC
web_address = "127.0.0.1:8080"   # REST + web UI; fcs uses this

[indexer]
paths = ["~/work", "~/src/other-repo"]          # ~ is expanded
exclude_patterns = ["**/node_modules/**", "**/target/**", "**/.git/**",
                    "**/build/**", "**/dist/**", "**/.venv/**"]

# Persist the index so restarts take seconds instead of a full rebuild,
# and fcs can search offline when the server is down.
index_path = "~/.local/share/fast_code_search/index.fcsidx"
save_after_build = true
checkpoint_interval_files = 20000

# Follow edits, renames and deletes as they happen.
watch = true
save_after_updates = 100
```

Relative paths in the file resolve against the file's own directory. Both
the server and `fcs` find this file automatically (`~/.config/fast_code_search/config.toml`);
`FCS_CONFIG=/path/to/file` overrides that for both.

Check it once by hand before automating it:

```bash
fast_code_search_server --config ~/.config/fast_code_search/config.toml
```

Open http://127.0.0.1:8080 or, in another terminal, `fcs status` and
`fcs 'fn main'`. Stop it with Ctrl+C (the index is saved on the way out).

### Linux: inotify watches

Watching a large tree needs one inotify watch per directory. The server
logs how many it installed and warns when the limit is hit. Raise the
limit if you see that warning:

```bash
echo fs.inotify.max_user_watches=524288 | sudo tee /etc/sysctl.d/60-fast-code-search.conf
sudo sysctl --system
```

## 3. Start it at login

### Linux (systemd user service)

Copy [`deploy/systemd/fast-code-search.service`](../deploy/systemd/fast-code-search.service)
to `~/.config/systemd/user/` and enable it:

```bash
mkdir -p ~/.config/systemd/user
cp deploy/systemd/fast-code-search.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now fast-code-search
systemctl --user status fast-code-search
```

The unit runs `%h/.local/bin/fast_code_search_server --config %h/.config/fast_code_search/config.toml`
(`%h` is your home directory), restarts it if it crashes, and stops it
cleanly at logout so the index is saved. Logs:

```bash
journalctl --user -u fast-code-search -f
```

To keep the server running while you are logged out (SSH sessions, a shared
workstation), enable lingering once: `loginctl enable-linger $USER`.

### macOS (launchd user agent)

Copy [`deploy/launchd/com.fastcodesearch.server.plist`](../deploy/launchd/com.fastcodesearch.server.plist)
to `~/Library/LaunchAgents/`, replace `__HOME__` with your home directory,
and load it:

```bash
mkdir -p ~/Library/LaunchAgents ~/Library/Logs
sed "s|__HOME__|$HOME|g" deploy/launchd/com.fastcodesearch.server.plist \
  > ~/Library/LaunchAgents/com.fastcodesearch.server.plist
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.fastcodesearch.server.plist
launchctl print gui/$(id -u)/com.fastcodesearch.server | head
```

Logs go to `~/Library/Logs/fast_code_search.log`. To stop or unload:

```bash
launchctl bootout gui/$(id -u)/com.fastcodesearch.server
```

On macOS the server watches each root with FSEvents, so no watch limit
applies. If the binaries were downloaded rather than built locally, clear
the quarantine flag first: `xattr -d com.apple.quarantine ~/.local/bin/fast_code_search_server`.

### Windows (Task Scheduler)

Put `fast_code_search_server.exe` and `fcs.exe` in a directory on your
`PATH` (for example `%LOCALAPPDATA%\fast_code_search\bin`) and the
configuration at `%APPDATA%\fast_code_search\config.toml` (the default
location on Windows). Then, in PowerShell:

```powershell
.\deploy\windows\Install-StartupTask.ps1
```

The script registers a task named `FastCodeSearch` that runs the server
hidden at logon of the current user, restarting it up to three times if it
exits. Inspect it with `Get-ScheduledTask FastCodeSearch`, start it now
with `Start-ScheduledTask FastCodeSearch`, and remove it with
`.\deploy\windows\Install-StartupTask.ps1 -Uninstall`. The server's log is
written next to the configuration file (`server.log`).

## 4. Verify

```bash
fcs status
```

should report the server as reachable, its version and the number of files
indexed. The first start builds the index (minutes for hundreds of thousands
of files); later starts load the saved index in seconds and reconcile it
with what changed while the server was down.

## Troubleshooting

- **`fcs: cannot reach the search server`** — the service is not running or
  listens elsewhere. Check `systemctl --user status fast-code-search` /
  `launchctl print` / `Get-ScheduledTask`, then the log. `fcs --server URL`
  targets another address; `FCS_SERVER` sets it for a shell.
- **Port already in use** — another instance is running (perhaps started by
  hand). Stop it, or change `web_address` / `address` in the configuration.
- **Index rebuilt on every start** — `index_path` is unset, or points at a
  directory the service cannot write. The log says where it tried to save.
- **Changes are not picked up** — `watch = false`, or the inotify limit was
  hit (Linux); see the log for "Failed to watch".
- **Old files keep appearing after editing `exclude_patterns`** — they are
  dropped on the next start; a running server applies the new rules only to
  files it re-indexes.
