# Changelog

All notable changes to the **Fast Code Search** VSCode extension will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

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
