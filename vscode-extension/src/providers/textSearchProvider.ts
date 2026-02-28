/**
 * TextSearchProvider implementation for fast_code_search.
 *
 * Routes searches to either the keyword server (trigram-based) or the
 * semantic server (embedding-based) depending on the active mode.
 */

import * as vscode from "vscode";
import { KeywordSearchClient } from "../api/keywordClient.js";
import { SemanticSearchClient } from "../api/semanticClient.js";

export class FastCodeSearchProvider implements vscode.TextSearchProvider {
  constructor(
    private readonly keywordClient: KeywordSearchClient,
    private readonly semanticClient: SemanticSearchClient
  ) {}

  async provideTextSearchResults(
    query: vscode.TextSearchQuery,
    options: vscode.TextSearchOptions,
    progress: vscode.Progress<vscode.TextSearchResult>,
    token: vscode.CancellationToken
  ): Promise<vscode.TextSearchComplete> {
    const cfg = vscode.workspace.getConfiguration("fastCodeSearch");
    const preferSemantic: boolean = cfg.get("preferSemanticSearch", false);
    const semanticEnabled: boolean = cfg.get(
      "semanticServer.enabled",
      false
    );
    const maxResults: number = cfg.get(
      "maxResults",
      options.maxResults ?? 100
    );
    const symbolsOnly: boolean = cfg.get("symbolsOnly", false);

    const abortController = new AbortController();
    token.onCancellationRequested(() => abortController.abort());

    const useSemantic = preferSemantic && semanticEnabled;

    try {
      if (useSemantic) {
        await this.runSemanticSearch(
          query,
          options,
          progress,
          abortController.signal,
          maxResults
        );
      } else {
        await this.runKeywordSearch(
          query,
          options,
          progress,
          abortController.signal,
          maxResults,
          symbolsOnly
        );
      }
    } catch (err: unknown) {
      if (isAbortError(err)) {
        // Search cancelled – not an error
        return { limitHit: false };
      }
      const mode = useSemantic ? "semantic" : "keyword";
      const msg =
        err instanceof Error ? err.message : String(err);
      vscode.window.showErrorMessage(
        `Fast Code Search (${mode}): ${msg}`
      );
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
        uri: vscode.Uri.file(result.file_path),
        ranges: new vscode.Range(lineIndex, matchStart, lineIndex, matchEnd),
        preview: {
          text: result.content,
          matches: new vscode.Range(0, matchStart, 0, matchEnd),
        },
      });
    }
  }

  private async runSemanticSearch(
    query: vscode.TextSearchQuery,
    _options: vscode.TextSearchOptions,
    progress: vscode.Progress<vscode.TextSearchResult>,
    signal: AbortSignal,
    maxResults: number
  ): Promise<void> {
    const response = await this.semanticClient.search(
      { q: query.pattern, max: maxResults },
      signal
    );

    for (const result of response.results) {
      if (signal.aborted) {
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
  }
}

/** Returns true when an error represents a fetch/AbortController cancellation. */
function isAbortError(err: unknown): boolean {
  return (
    err instanceof Error &&
    (err.name === "AbortError" || err.message.includes("abort"))
  );
}
