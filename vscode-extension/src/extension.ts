/**
 * Extension entry point for Fast Code Search.
 *
 * Registers:
 *   - TextSearchProvider    (keyword search — native VSCode search integration)
 *   - AITextSearchProvider  (semantic search — shows in "AI Results" section)
 *   - Status bar item        (shows current mode: Keyword | Semantic)
 *   - Commands
 *       fastCodeSearch.toggleSymbolsOnly   – restrict search to symbols
 *       fastCodeSearch.showServerStatus    – display server health in output channel
 *       fastCodeSearch.downloadServer      – download the server binary
 *       fastCodeSearch.startServer         – start the managed server process
 *       fastCodeSearch.stopServer          – stop the managed server process
 *       fastCodeSearch.restartServer       – restart the managed server process
 */

import * as vscode from "vscode";
import { KeywordSearchClient } from "./api/keywordClient.js";
import { SemanticSearchClient } from "./api/semanticClient.js";
import { FastCodeSearchProvider } from "./providers/textSearchProvider.js";
import { SemanticSearchProvider } from "./providers/semanticSearchProvider.js";
import { ServerManager } from "./server/serverManager.js";

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

  // Server lifecycle manager
  const serverManager = new ServerManager(context, outputChannel);
  context.subscriptions.push(serverManager);

  // Register the TextSearchProvider for the "file" scheme so that VSCode's
  // built-in search UI delegates to our keyword server.
  const provider = new FastCodeSearchProvider(keywordClient);
  context.subscriptions.push(
    vscode.workspace.registerTextSearchProvider("file", provider)
  );

  // Register the AITextSearchProvider for the "file" scheme so that VSCode
  // shows semantic results in the dedicated "AI Results" section of the
  // Search panel when the semantic server is enabled.
  const semanticProvider = new SemanticSearchProvider(semanticClient);
  context.subscriptions.push(
    vscode.workspace.registerAITextSearchProvider("file", semanticProvider)
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
        updateStatusBar(statusBar, serverManager);
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
  statusBar.command = "fastCodeSearch.toggleSymbolsOnly";
  statusBar.tooltip = "Fast Code Search – click to toggle symbols-only mode";
  updateStatusBar(statusBar, serverManager);
  statusBar.show();
  context.subscriptions.push(statusBar);

  // -------------------------------------------------------------------------
  // Commands
  // -------------------------------------------------------------------------

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
        updateStatusBar(statusBar, serverManager);
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

        outputChannel.appendLine(
          `Managed server process: ${serverManager.isRunning() ? "running" : "stopped"}`
        );
        outputChannel.appendLine(
          `Binary path: ${serverManager.getBinaryPath()}`
        );
        outputChannel.appendLine(
          `Binary installed: ${serverManager.isBinaryInstalled()}`
        );
      }
    )
  );

  // Download server binary
  context.subscriptions.push(
    vscode.commands.registerCommand(
      "fastCodeSearch.downloadServer",
      async () => {
        const version = resolveServerVersion(context);
        await vscode.window.withProgress(
          {
            location: vscode.ProgressLocation.Notification,
            title: "Fast Code Search: Downloading server binary",
            cancellable: false,
          },
          async (progress) => {
            try {
              await serverManager.downloadBinary(version, progress);
              updateStatusBar(statusBar, serverManager);
              vscode.window.showInformationMessage(
                `Fast Code Search server binary downloaded (${version}).`
              );
            } catch (err) {
              const msg = err instanceof Error ? err.message : String(err);
              vscode.window.showErrorMessage(
                `Fast Code Search: Download failed – ${msg}`
              );
              outputChannel.appendLine(`Download error: ${msg}`);
            }
          }
        );
      }
    )
  );

  // Start server
  context.subscriptions.push(
    vscode.commands.registerCommand(
      "fastCodeSearch.startServer",
      async () => {
        outputChannel.show(true);
        await startManagedServer(serverManager, context, outputChannel, statusBar);
      }
    )
  );

  // Stop server
  context.subscriptions.push(
    vscode.commands.registerCommand("fastCodeSearch.stopServer", () => {
      serverManager.stopServer();
      updateStatusBar(statusBar, serverManager);
      vscode.window.showInformationMessage(
        "Fast Code Search: server stopped."
      );
    })
  );

  // Restart server
  context.subscriptions.push(
    vscode.commands.registerCommand(
      "fastCodeSearch.restartServer",
      async () => {
        outputChannel.show(true);
        serverManager.stopServer();
        await startManagedServer(serverManager, context, outputChannel, statusBar);
      }
    )
  );

  outputChannel.appendLine("Fast Code Search extension activated.");

  // -------------------------------------------------------------------------
  // Auto-start server (best-effort, non-blocking)
  // -------------------------------------------------------------------------
  const autoStart = vscode.workspace
    .getConfiguration("fastCodeSearch")
    .get<boolean>("autoStartServer", true);

  if (autoStart) {
    void autoStartServer(serverManager, context, outputChannel, statusBar);
  }
}

export function deactivate(): void {
  // Nothing to clean up; subscriptions are disposed automatically.
}

// ---------------------------------------------------------------------------
// Auto-start logic
// ---------------------------------------------------------------------------

/**
 * Attempt to auto-start the server on extension activation.
 *
 * Flow:
 *  1. If the configured keyword server is already reachable, do nothing.
 *  2. If the binary is not installed and the platform is supported, offer
 *     to download it.
 *  3. If the binary is available, start the server against the current
 *     workspace folders.
 */
async function autoStartServer(
  manager: ServerManager,
  context: vscode.ExtensionContext,
  out: vscode.OutputChannel,
  statusBar: vscode.StatusBarItem
): Promise<void> {
  // Skip if the server is already reachable (user-managed external server)
  if (await isServerReachable()) {
    out.appendLine(
      "Keyword server already reachable – skipping auto-start."
    );
    return;
  }

  // No workspace open – nothing to index
  const folders = vscode.workspace.workspaceFolders;
  if (!folders || folders.length === 0) {
    return;
  }

  if (!manager.isBinaryInstalled()) {
    if (!manager.isPlatformSupported()) {
      out.appendLine(
        "Auto-start skipped: platform not supported for auto-download. " +
          "Set fastCodeSearch.serverBinaryPath to use a custom binary."
      );
      return;
    }

    const choice = await vscode.window.showInformationMessage(
      "Fast Code Search: Server binary not found. Download it now?",
      "Download",
      "Not Now"
    );
    if (choice !== "Download") {
      return;
    }

    const version = resolveServerVersion(context);
    await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title: "Fast Code Search: Downloading server binary",
        cancellable: false,
      },
      async (progress) => {
        await manager.downloadBinary(version, progress);
      }
    );
  }

  await startManagedServer(manager, context, out, statusBar);
}

/** Check if the configured keyword server is already accepting requests. */
async function isServerReachable(): Promise<boolean> {
  const cfg = vscode.workspace.getConfiguration("fastCodeSearch");
  const host = cfg.get<string>("keywordServer.host", "localhost");
  const port = cfg.get<number>("keywordServer.port", 8080);
  try {
    const res = await fetch(`http://${host}:${port}/api/health`);
    return res.ok;
  } catch {
    return false;
  }
}

/** Start the server and update the status bar, showing errors as notifications. */
async function startManagedServer(
  manager: ServerManager,
  context: vscode.ExtensionContext,
  out: vscode.OutputChannel,
  statusBar: vscode.StatusBarItem
): Promise<void> {
  const folders = vscode.workspace.workspaceFolders ?? [];
  const paths = folders.map((f) => f.uri.fsPath);

  try {
    await manager.startServer(paths);
    updateStatusBar(statusBar, manager);
    vscode.window.showInformationMessage(
      `Fast Code Search server started (indexing ${paths.length} folder${paths.length === 1 ? "" : "s"}).`
    );
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    vscode.window.showErrorMessage(`Fast Code Search: ${msg}`);
    out.appendLine(`Start error: ${msg}`);
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Determine the server version to download.
 *
 * Uses `fastCodeSearch.serverVersion` from settings, falling back to the
 * extension's own version so that a freshly-installed extension always
 * downloads a matching binary.
 *
 * The string is normalised to start with "v" (e.g. "0.1.0" → "v0.1.0").
 */
function resolveServerVersion(context: vscode.ExtensionContext): string {
  const configured = vscode.workspace
    .getConfiguration("fastCodeSearch")
    .get<string>("serverVersion", "latest")
    .trim();

  if (configured && configured !== "latest") {
    return configured.startsWith("v") ? configured : `v${configured}`;
  }

  // Fall back to the extension's own version
  const extVersion = context.extension.packageJSON?.version;
  if (typeof extVersion === "string" && extVersion) {
    return extVersion.startsWith("v") ? extVersion : `v${extVersion}`;
  }

  return "latest";
}

/** Refresh the status-bar label to reflect the current mode settings. */
function updateStatusBar(
  item: vscode.StatusBarItem,
  manager: ServerManager
): void {
  const cfg = vscode.workspace.getConfiguration("fastCodeSearch");
  const symbols: boolean = cfg.get("symbolsOnly", false);
  const symSuffix = symbols ? " [symbols]" : "";
  const running = manager.isRunning();
  item.text = `$(search) Fast Code Search${symSuffix}`;
  item.tooltip = running
    ? "Fast Code Search – server running (click to toggle symbols-only)"
    : "Fast Code Search – server not running (click to toggle symbols-only)";
  item.backgroundColor = running
    ? undefined
    : new vscode.ThemeColor("statusBarItem.warningBackground");
}

