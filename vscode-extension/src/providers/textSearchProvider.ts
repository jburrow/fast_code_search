/**
 * TextSearchProvider implementation for fast_code_search keyword search.
 *
 * Delegates all searches to the keyword server (trigram-based).
 * Semantic search is handled separately by {@link SemanticSearchProvider}
 * which implements the {@link vscode.AITextSearchProvider} interface.
 */

import * as vscode from "vscode";
import { KeywordSearchClient } from "../api/keywordClient.js";
import { isAbortError } from "../utils/errors.js";

/**
 * Convert a file path returned by the server into a VS Code URI.
 *
 * The server now returns root-relative paths (e.g. `src/main.rs`) with
 * forward-slash separators. When the path is relative, we join it with
 * each workspace folder until we find one that could contain the file
 * (simple heuristic: first match wins, which is correct for single-root
 * workspaces and works well for multi-root ones too).
 */
function resolveFileUri(filePath: string): vscode.Uri {
  // Absolute paths (Unix `/...` or Windows `C:\...`) are used as-is.
  if (filePath.startsWith("/") || /^[A-Za-z]:[\\/]/.test(filePath)) {
    return vscode.Uri.file(filePath);
  }
  const folders = vscode.workspace.workspaceFolders ?? [];
  if (folders.length > 0) {
    return vscode.Uri.joinPath(folders[0].uri, filePath);
  }
  return vscode.Uri.file(filePath);
}

export class FastCodeSearchProvider implements vscode.TextSearchProvider {
  constructor(private readonly keywordClient: KeywordSearchClient) {}

  async provideTextSearchResults(
    query: vscode.TextSearchQuery,
    options: vscode.TextSearchOptions,
    progress: vscode.Progress<vscode.TextSearchResult>,
    token: vscode.CancellationToken
  ): Promise<vscode.TextSearchComplete> {
    const cfg = vscode.workspace.getConfiguration("fastCodeSearch");
    const maxResults: number = cfg.get(
      "maxResults",
      options.maxResults ?? 100
    );
    const symbolsOnly: boolean = cfg.get("symbolsOnly", false);

    const abortController = new AbortController();
    token.onCancellationRequested(() => abortController.abort());

    try {
      await this.runKeywordSearch(
        query,
        options,
        progress,
        abortController.signal,
        maxResults,
        symbolsOnly
      );
    } catch (err: unknown) {
      if (isAbortError(err)) {
        // Search cancelled – not an error
        return { limitHit: false };
      }
      const msg = err instanceof Error ? err.message : String(err);
      vscode.window.showErrorMessage(`Fast Code Search (keyword): ${msg}`);
      return { limitHit: false };
    }

    return { limitHit: false };
  }

  // -------------------------------------------------------------------------
  // Private helpers
  // -------------------------------------------------------------------------

  private async runKeywordSearch(
    query: vscode.TextSearchQuery,
    options: vscode.TextSearchOptions,
    progress: vscode.Progress<vscode.TextSearchResult>,
    signal: AbortSignal,
    maxResults: number,
    symbolsOnly: boolean
  ): Promise<void> {
    const include = (options.includes ?? []).join(";");
    const exclude = (options.excludes ?? []).join(";");

    const response = await this.keywordClient.search(
      {
        q: query.pattern,
        max: maxResults,
        include: include || undefined,
        exclude: exclude || undefined,
        regex: query.isRegExp ?? false,
        symbols: symbolsOnly,
      },
      signal
    );

    for (const result of response.results) {
      if (signal.aborted) {
        break;
      }

      // line_number is 1-based; VSCode Range is 0-based
      const lineIndex = Math.max(0, result.line_number - 1);
      const matchStart = result.match_start ?? 0;
      const matchEnd = result.match_end ?? result.content.length;

      progress.report({
        uri: resolveFileUri(result.file_path),
        ranges: new vscode.Range(lineIndex, matchStart, lineIndex, matchEnd),
        preview: {
          text: result.content,
          matches: new vscode.Range(0, matchStart, 0, matchEnd),
        },
      });
    }
  }
}
