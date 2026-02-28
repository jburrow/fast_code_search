# Fast Code Search – VSCode Extension

A VSCode extension that integrates the [fast_code_search](https://github.com/jburrow/fast_code_search) server directly into VSCode's native search UI.

---

## Features

| Feature | Description |
|---------|-------------|
| **Keyword Search** | Trigram-indexed, near-instant full-text search across millions of files |
| **Semantic Search** | Natural-language code search powered by ML embeddings — results appear in the dedicated **"AI Results"** section of the Search panel (requires semantic server) |
| **Symbols-Only Mode** | Restrict results to function / class definitions |
| **Server Status** | Inspect server health and index statistics from the output channel |

---

## Requirements

You need at least one running `fast_code_search` server:

```bash
# Keyword server (port 8080)
cargo run --release --bin fast_code_search

# Semantic server (port 8081, optional – requires ml-models feature)
cargo run --release --bin fast_code_search_semantic --features ml-models
```

See the [project README](https://github.com/jburrow/fast_code_search) for server setup details.

---

## Installation

### From a `.vsix` file

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

1. Start the `fast_code_search` server (see Requirements).
2. Open a workspace folder in VSCode.
3. Use **Ctrl+Shift+F** (or the Search sidebar) to search – results are served by the fast_code_search server.

### Search Modes

| Action | Keyboard Shortcut | Command |
|--------|-------------------|---------|
| Toggle symbols-only mode | – | `Fast Code Search: Toggle Symbols-Only Mode` |
| Show server status | – | `Fast Code Search: Show Server Status` |

Keyword search results appear in the standard search panel. When the semantic server is enabled (`fastCodeSearch.semanticServer.enabled: true`), semantic results appear automatically in the **"AI Results"** section of the same panel — no mode-switching needed.

---

## Configuration

All settings are available in **File → Preferences → Settings** under **"Fast Code Search"**.

| Setting | Default | Description |
|---------|---------|-------------|
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
```

The extension registers two providers for the `file:` URI scheme:

- **`FastCodeSearchProvider`** implements `TextSearchProvider` and routes all
  standard workspace searches to the keyword (trigram-indexed) server.
- **`SemanticSearchProvider`** implements `AITextSearchProvider` and routes searches
  to the semantic (ML-embedding) server. Its results appear in the dedicated
  **"AI Results"** section of VSCode's Search panel whenever
  `fastCodeSearch.semanticServer.enabled` is `true`.

---

## Troubleshooting

**"Fast Code Search (keyword): Failed to fetch"**  
→ The keyword server is not running or the configured host/port is wrong. Start the server and verify the `fastCodeSearch.keywordServer.*` settings.

**Semantic search not working**  
→ Ensure `fastCodeSearch.semanticServer.enabled` is `true` and the semantic server is running on the configured port.

**Results appear for wrong files**  
→ The server must be indexing the same workspace root. Restart indexing if files have changed.

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

## License

MIT – see [LICENSE](../LICENSE).
