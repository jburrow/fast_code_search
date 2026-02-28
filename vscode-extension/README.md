# Fast Code Search – VSCode Extension

A VSCode extension that integrates the [fast_code_search](https://github.com/jburrow/fast_code_search) server directly into VSCode's native search UI.

---

## Features

| Feature | Description |
|---------|-------------|
| **Keyword Search** | Trigram-indexed, near-instant full-text search across millions of files |
| **Semantic Search** | Natural-language code search powered by ML embeddings — results appear in the dedicated **"AI Results"** section of the Search panel (requires semantic server) |
| **Symbols-Only Mode** | Restrict results to function / class definitions |
| **Auto-Download** | On first use the extension downloads and starts the correct server binary for your platform automatically |
| **Server Status** | Inspect server health and index statistics from the output channel |

---

## Installation

### From the VS Code Marketplace _(recommended)_

1. Open VS Code.
2. Press **Ctrl+P** and run:
   ```
   ext install fast-code-search.fast-code-search
   ```
   or search for **"Fast Code Search"** in the Extensions view.
3. Open a workspace folder.
4. The extension automatically downloads the server binary for your platform and starts the server. Progress is shown in the notification area.

> **Supported platforms for auto-download:** Linux x86-64, Linux arm64, macOS x86-64, macOS arm64, Windows x86-64.  
> On other platforms, or if you prefer to manage the binary yourself, set `fastCodeSearch.serverBinaryPath` to point at an existing `fast_code_search_server` binary.

### Using a custom binary

If you already have the server binary (e.g. built from source), disable auto-download and point the extension at your binary:

```json
{
  "fastCodeSearch.autoStartServer": false,
  "fastCodeSearch.serverBinaryPath": "/usr/local/bin/fast_code_search_server"
}
```

Then start the server manually (see below) or use the command palette.

### From a `.vsix` file (manual)

```bash
cd vscode-extension
npm install
npm run build
npx vsce package
code --install-extension fast-code-search-0.1.0.vsix
```

### During development

1. Open the `vscode-extension/` folder in VSCode.
2. Press **F5** to launch an Extension Development Host.
3. The extension will be active in the new window.

---

## Usage

1. Open a workspace folder in VSCode.
2. The extension auto-starts the server and indexes the workspace.
3. Use **Ctrl+Shift+F** (or the Search sidebar) to search – results are served by the fast_code_search server.

### Search Modes

| Action | Keyboard Shortcut | Command |
|--------|-------------------|---------|
| Toggle symbols-only mode | – | `Fast Code Search: Toggle Symbols-Only Mode` |
| Show server status | – | `Fast Code Search: Show Server Status` |
| Download server binary | – | `Fast Code Search: Download Server Binary` |
| Start server | – | `Fast Code Search: Start Server` |
| Stop server | – | `Fast Code Search: Stop Server` |
| Restart server | – | `Fast Code Search: Restart Server` |

Keyword search results appear in the standard search panel. When the semantic server is enabled (`fastCodeSearch.semanticServer.enabled: true`), semantic results appear automatically in the **"AI Results"** section of the same panel — no mode-switching needed.

---

## Configuration

All settings are available in **File → Preferences → Settings** under **"Fast Code Search"**.

| Setting | Default | Description |
|---------|---------|-------------|
| `fastCodeSearch.autoStartServer` | `true` | Automatically download (if needed) and start the server when a workspace is opened |
| `fastCodeSearch.serverVersion` | `"latest"` | Server binary version to download. Use `"latest"` or pin to a tag like `"v0.2.0"` |
| `fastCodeSearch.serverBinaryPath` | `""` | Absolute path to a pre-installed binary. Overrides auto-download when set |
| `fastCodeSearch.keywordServer.host` | `localhost` | Keyword server hostname |
| `fastCodeSearch.keywordServer.port` | `8080` | Keyword server port |
| `fastCodeSearch.semanticServer.enabled` | `false` | Enable semantic server (AI Results) |
| `fastCodeSearch.semanticServer.host` | `localhost` | Semantic server hostname |
| `fastCodeSearch.semanticServer.port` | `8081` | Semantic server port |
| `fastCodeSearch.maxResults` | `100` | Maximum results per search |
| `fastCodeSearch.symbolsOnly` | `false` | Search only in symbol definitions |

### Example `settings.json`

```json
{
  "fastCodeSearch.keywordServer.host": "localhost",
  "fastCodeSearch.keywordServer.port": 8080,
  "fastCodeSearch.semanticServer.enabled": true,
  "fastCodeSearch.semanticServer.port": 8081,
  "fastCodeSearch.maxResults": 200
}
```

---

## Architecture

```
VSCode Search UI
      │
      ├─► FastCodeSearchProvider (TextSearchProvider)
      │         └─► KeywordSearchClient  →  GET http://localhost:8080/api/search
      │
      └─► SemanticSearchProvider (AITextSearchProvider)   ← "AI Results" section
                └─► SemanticSearchClient →  GET http://localhost:8081/api/search

ServerManager
      ├─► detect platform → Rust target triple
      ├─► download binary from GitHub Releases (if not cached)
      └─► spawn / supervise fast_code_search_server process
```

The extension registers two providers for the `file:` URI scheme:

- **`FastCodeSearchProvider`** implements `TextSearchProvider` and routes all
  standard workspace searches to the keyword (trigram-indexed) server.
- **`SemanticSearchProvider`** implements `AITextSearchProvider` and routes searches
  to the semantic (ML-embedding) server. Its results appear in the dedicated
  **"AI Results"** section of VSCode's Search panel whenever
  `fastCodeSearch.semanticServer.enabled` is `true`.
- **`ServerManager`** handles downloading the platform-specific binary from GitHub
  Releases, caching it in the extension's global storage directory, and managing
  the server process lifecycle.

---

## Troubleshooting

**"Fast Code Search (keyword): Failed to fetch"**  
→ The keyword server is not running or the configured host/port is wrong. Start the server and verify the `fastCodeSearch.keywordServer.*` settings.

**Automatic download failed**  
→ Check the "Fast Code Search" output channel for the error message. Common causes:
  - No internet access / firewall blocking GitHub.
  - Platform not in the supported list above.  
  Set `fastCodeSearch.serverBinaryPath` to a manually-downloaded binary as a workaround.

**Semantic search not working**  
→ Ensure `fastCodeSearch.semanticServer.enabled` is `true` and the semantic server is running on the configured port.

**Results appear for wrong files**  
→ The server must be indexing the same workspace root. Use **Fast Code Search: Restart Server** to re-index.

---

## Development

```bash
cd vscode-extension

# Install dependencies
npm install

# Type-check (no output = success)
npm run compile

# Build extension bundle
npm run build:dev

# Watch mode (rebuilds on save)
npm run watch

# Lint
npm run lint
```

---

## Publishing to the VS Code Marketplace

The extension is published automatically by the `publish-extension` GitHub Actions workflow whenever a new release tag is pushed. The workflow:

1. Builds the extension bundle (`npm run build`).
2. Packages it into a `.vsix` file.
3. Publishes to the marketplace using `vsce publish`.

To publish manually:

```bash
cd vscode-extension
npm install
npm run build
npx vsce publish --pat <YOUR_MARKETPLACE_PAT>
```

Set the `VSCE_PAT` secret in the GitHub repository settings for automated publishing.

---

## License

MIT – see [LICENSE](../LICENSE).

