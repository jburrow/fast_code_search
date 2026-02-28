/**
 * AITextSearchProvider implementation for fast_code_search semantic search.
 *
 * Registered for the "file" URI scheme so that VSCode shows semantic results in
 * the dedicated "AI Results" section of the Search panel alongside the standard
 * keyword results.
 */

import * as vscode from "vscode";
import { SemanticSearchClient } from "../api/semanticClient.js";
import { isAbortError } from "../utils/errors.js";

export class SemanticSearchProvider implements vscode.AITextSearchProvider {
  constructor(private readonly semanticClient: SemanticSearchClient) {}

  async provideAITextSearchResults(
    query: string,
    options: vscode.TextSearchOptions,
    progress: vscode.Progress<vscode.TextSearchResult>,
    token: vscode.CancellationToken
  ): Promise<vscode.TextSearchComplete> {
    const cfg = vscode.workspace.getConfiguration("fastCodeSearch");
    const semanticEnabled: boolean = cfg.get("semanticServer.enabled", false);

    if (!semanticEnabled) {
      return { limitHit: false };
    }

    const maxResults: number = cfg.get(
      "maxResults",
      options.maxResults ?? 100
    );

    const abortController = new AbortController();
    token.onCancellationRequested(() => abortController.abort());

    try {
      const response = await this.semanticClient.search(
        { q: query, max: maxResults },
        abortController.signal
      );

      for (const result of response.results) {
        if (abortController.signal.aborted) {
          break;
        }

        // Semantic results span multiple lines (1-based → 0-based)
        const startLine = Math.max(0, result.start_line - 1);
        const endLine = Math.max(startLine, result.end_line - 1);
        const previewText = result.content;

        progress.report({
          uri: vscode.Uri.file(result.file_path),
          ranges: new vscode.Range(
            startLine,
            0,
            endLine,
            Number.MAX_SAFE_INTEGER
          ),
          preview: {
            text: previewText,
            matches: new vscode.Range(0, 0, 0, previewText.length),
          },
        });
      }
    } catch (err: unknown) {
      if (isAbortError(err)) {
        return { limitHit: false };
      }
      const msg = err instanceof Error ? err.message : String(err);
      vscode.window.showErrorMessage(`Fast Code Search (semantic): ${msg}`);
    }

    return { limitHit: false };
  }
}
