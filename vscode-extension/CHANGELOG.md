# Changelog

All notable changes to the **Fast Code Search** VSCode extension will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [Unreleased]

### Added
- **Auto-download**: on first use the extension downloads the correct `fast_code_search_server` binary for the current platform (Linux x86-64/arm64, macOS x86-64/arm64, Windows x86-64) from GitHub Releases.
- **Auto-start**: the server process is automatically started against the open workspace folders when a workspace is opened.
- `ServerManager` class (`src/server/serverManager.ts`) encapsulates binary detection, download with HTTP redirect following, archive extraction, process spawning, and health-check polling.
- New configuration settings:
  - `fastCodeSearch.autoStartServer` – enable/disable automatic server management (default `true`).
  - `fastCodeSearch.serverVersion` – pin the binary version to download, or use `"latest"` (default).
  - `fastCodeSearch.serverBinaryPath` – point at an existing binary to bypass auto-download.
- New commands:
  - `Fast Code Search: Download Server Binary`
  - `Fast Code Search: Start Server`
  - `Fast Code Search: Stop Server`
  - `Fast Code Search: Restart Server`
- Status bar item now shows a warning colour when the managed server is not running.
- `Show Server Status` command now reports managed-process state, binary path, and install status.
- GitHub Actions workflow (`.github/workflows/publish-extension.yml`) for automated marketplace publishing on release tags.

---

## [0.1.0] – 2026-02-28

### Added
- TextSearchProvider for native VSCode search integration (keyword mode via trigram index).
- Semantic search support (optional, requires semantic server on port 8081).
- Toggle command `fastCodeSearch.toggleSemanticMode` with keyboard shortcut `Ctrl+Alt+S`.
- Toggle command `fastCodeSearch.toggleSymbolsOnly` to restrict results to symbol definitions.
- Command `fastCodeSearch.showServerStatus` that prints server health + index stats to the output channel.
- Status bar item showing current search mode (Keyword / Semantic) and symbols-only state.
- Configuration schema: keyword/semantic server host, port, max results, mode preferences.
- AbortController-based cancellation – cancelling a VSCode search aborts the in-flight HTTP request.
- Automatic client reconfiguration when VSCode settings change.

