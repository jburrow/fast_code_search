/**
 * Extension entry point for Fast Code Search.
 *
 * Registers:
 *   - TextSearchProvider  (native VSCode search integration)
 *   - Status bar item      (shows current mode: Keyword | Semantic)
 *   - Commands
 *       fastCodeSearch.toggleSemanticMode  – switch between keyword / semantic
 *       fastCodeSearch.toggleSymbolsOnly   – restrict search to symbols
 *       fastCodeSearch.showServerStatus    – display server health in output channel
 */

import * as vscode from "vscode";
import { KeywordSearchClient } from "./api/keywordClient.js";
import { SemanticSearchClient } from "./api/semanticClient.js";
import { FastCodeSearchProvider } from "./providers/textSearchProvider.js";

// ---------------------------------------------------------------------------
// Extension lifecycle
// ---------------------------------------------------------------------------

export function activate(context: vscode.ExtensionContext): void {
  const outputChannel = vscode.window.createOutputChannel("Fast Code Search");
  context.subscriptions.push(outputChannel);

  outputChannel.appendLine("Fast Code Search extension activating…");

  // Build API clients from current configuration
  const cfg = vscode.workspace.getConfiguration("fastCodeSearch");
  const keywordClient = new KeywordSearchClient(
    cfg.get("keywordServer.host", "localhost"),
    cfg.get("keywordServer.port", 8080)
  );
  const semanticClient = new SemanticSearchClient(
    cfg.get("semanticServer.host", "localhost"),
    cfg.get("semanticServer.port", 8081)
  );

  // Register the TextSearchProvider for the "file" scheme so that VSCode's
  // built-in search UI delegates to our server.
  const provider = new FastCodeSearchProvider(keywordClient, semanticClient);
  context.subscriptions.push(
    vscode.workspace.registerTextSearchProvider("file", provider)
  );

  // Re-initialise clients when settings change
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration("fastCodeSearch")) {
        const updated = vscode.workspace.getConfiguration("fastCodeSearch");
        keywordClient.updateAddress(
          updated.get("keywordServer.host", "localhost"),
          updated.get("keywordServer.port", 8080)
        );
        semanticClient.updateAddress(
          updated.get("semanticServer.host", "localhost"),
          updated.get("semanticServer.port", 8081)
        );
        updateStatusBar(statusBar);
        outputChannel.appendLine("Configuration updated.");
      }
    })
  );

  // -------------------------------------------------------------------------
  // Status bar
  // -------------------------------------------------------------------------

  const statusBar = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Right,
    100
  );
  statusBar.command = "fastCodeSearch.toggleSemanticMode";
  statusBar.tooltip = "Fast Code Search – click to toggle search mode";
  updateStatusBar(statusBar);
  statusBar.show();
  context.subscriptions.push(statusBar);

  // -------------------------------------------------------------------------
  // Commands
  // -------------------------------------------------------------------------

  // Toggle keyword ↔ semantic
  context.subscriptions.push(
    vscode.commands.registerCommand(
      "fastCodeSearch.toggleSemanticMode",
      async () => {
        const current = vscode.workspace
          .getConfiguration("fastCodeSearch")
          .get("preferSemanticSearch", false);
        await vscode.workspace
          .getConfiguration("fastCodeSearch")
          .update(
            "preferSemanticSearch",
            !current,
            vscode.ConfigurationTarget.Global
          );
        updateStatusBar(statusBar);
        const mode = !current ? "semantic" : "keyword";
        vscode.window.showInformationMessage(
          `Fast Code Search: switched to ${mode} mode.`
        );
      }
    )
  );

  // Toggle symbols-only
  context.subscriptions.push(
    vscode.commands.registerCommand(
      "fastCodeSearch.toggleSymbolsOnly",
      async () => {
        const current = vscode.workspace
          .getConfiguration("fastCodeSearch")
          .get("symbolsOnly", false);
        await vscode.workspace
          .getConfiguration("fastCodeSearch")
          .update(
            "symbolsOnly",
            !current,
            vscode.ConfigurationTarget.Global
          );
        updateStatusBar(statusBar);
        const state = !current ? "enabled" : "disabled";
        vscode.window.showInformationMessage(
          `Fast Code Search: symbols-only mode ${state}.`
        );
      }
    )
  );

  // Show server status
  context.subscriptions.push(
    vscode.commands.registerCommand(
      "fastCodeSearch.showServerStatus",
      async () => {
        outputChannel.show(true);
        outputChannel.appendLine("\n--- Server Status ---");

        const kHost = vscode.workspace
          .getConfiguration("fastCodeSearch")
          .get("keywordServer.host", "localhost");
        const kPort = vscode.workspace
          .getConfiguration("fastCodeSearch")
          .get("keywordServer.port", 8080);
        outputChannel.appendLine(
          `Keyword server: http://${kHost}:${kPort}`
        );
        try {
          const health = await keywordClient.health();
          outputChannel.appendLine(
            `  status: ${health.status}  version: ${health.version}`
          );
          const stats = await keywordClient.stats();
          outputChannel.appendLine(
            `  files: ${stats.num_files}  trigrams: ${stats.num_trigrams}`
          );
        } catch (err) {
          outputChannel.appendLine(
            `  ERROR: ${err instanceof Error ? err.message : String(err)}`
          );
        }

        const semanticEnabled = vscode.workspace
          .getConfiguration("fastCodeSearch")
          .get("semanticServer.enabled", false);
        if (semanticEnabled) {
          const sHost = vscode.workspace
            .getConfiguration("fastCodeSearch")
            .get("semanticServer.host", "localhost");
          const sPort = vscode.workspace
            .getConfiguration("fastCodeSearch")
            .get("semanticServer.port", 8081);
          outputChannel.appendLine(
            `Semantic server: http://${sHost}:${sPort}`
          );
          try {
            const health = await semanticClient.health();
            outputChannel.appendLine(
              `  status: ${health.status}  version: ${health.version}`
            );
            const stats = await semanticClient.stats();
            outputChannel.appendLine(
              `  files: ${stats.num_files}  chunks: ${stats.num_chunks}`
            );
          } catch (err) {
            outputChannel.appendLine(
              `  ERROR: ${err instanceof Error ? err.message : String(err)}`
            );
          }
        } else {
          outputChannel.appendLine("Semantic server: disabled");
        }
      }
    )
  );

  outputChannel.appendLine("Fast Code Search extension activated.");
}

export function deactivate(): void {
  // Nothing to clean up; subscriptions are disposed automatically.
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Refresh the status-bar label to reflect the current mode settings. */
function updateStatusBar(item: vscode.StatusBarItem): void {
  const cfg = vscode.workspace.getConfiguration("fastCodeSearch");
  const semantic: boolean = cfg.get("preferSemanticSearch", false);
  const symbols: boolean = cfg.get("symbolsOnly", false);

  const modeLabel = semantic ? "$(telescope) Semantic" : "$(search) Keyword";
  const symSuffix = symbols ? " [symbols]" : "";
  item.text = `${modeLabel}${symSuffix}`;
}
