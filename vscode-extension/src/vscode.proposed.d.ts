/**
 * Proposed API type stubs for VSCode TextSearchProvider.
 *
 * These declarations extend the `vscode` module to include the TextSearchProvider
 * proposed API (https://github.com/microsoft/vscode/blob/main/src/vscode-dts/vscode.proposed.textSearchProvider.d.ts).
 *
 * Enable in package.json with: "enabledApiProposals": ["textSearchProvider"]
 */

declare module "vscode" {
  export interface TextSearchQuery {
    /** The text pattern to search for. */
    pattern: string;
    /** Whether the pattern is a regular expression. */
    isRegExp?: boolean;
    /** Whether the search is case-sensitive. */
    isCaseSensitive?: boolean;
    /** Whether to match whole words only. */
    isWordMatch?: boolean;
  }

  export interface TextSearchPreviewOptions {
    /** The number of lines in the preview. */
    matchLines: number;
    /** The number of characters included per line. */
    charsPerLine: number;
  }

  export interface TextSearchOptions {
    /** The root folder to search within. */
    folder: Uri;
    /** Glob patterns for files to include. */
    includes: string[];
    /** Glob patterns for files to exclude. */
    excludes: string[];
    /** Whether to use .gitignore/.ignore files. */
    useIgnoreFiles: boolean;
    /** Whether to follow symbolic links. */
    followSymlinks: boolean;
    /** Whether the search has been cancelled. */
    isCanceled: boolean;
    /** Maximum number of results to return. */
    maxResults: number;
    /** Options that control how the search result is previewed. */
    previewOptions?: TextSearchPreviewOptions;
    /** Maximum file size (in bytes) to search. */
    maxFileSize?: number;
    /** Encoding of the files to search (e.g. "utf8"). */
    encoding?: string;
    /** Number of context lines to include after a match. */
    afterContext?: number;
    /** Number of context lines to include before a match. */
    beforeContext?: number;
  }

  export interface TextSearchMatchPreview {
    /** The matching lines of text. */
    text: string;
    /** The Range within `text` corresponding to the search match(es). */
    matches: Range | Range[];
  }

  export interface TextSearchMatch {
    /** The file in which the match was found. */
    uri: Uri;
    /** The Range of the match within the file. */
    ranges: Range | Range[];
    /** A preview of the match. */
    preview: TextSearchMatchPreview;
  }

  export interface TextSearchContext {
    /** The file in which the context line was found. */
    uri: Uri;
    /** The context line content. */
    text: string;
    /** The line number of this context line. */
    lineNumber: number;
  }

  /** A result returned by a TextSearchProvider. */
  export type TextSearchResult = TextSearchMatch | TextSearchContext;

  export interface TextSearchCompleteMessage {
    text: string;
    trusted?: boolean;
    type: TextSearchCompleteMessageType;
  }

  export enum TextSearchCompleteMessageType {
    Information = 1,
    Warning = 2,
  }

  export interface TextSearchComplete {
    /** Whether the search hit the maximum number of results. */
    limitHit?: boolean;
    /** Optional messages to display to the user. */
    message?: TextSearchCompleteMessage | TextSearchCompleteMessage[];
  }

  export interface TextSearchProvider {
    /**
     * Provide results that match the given text pattern.
     * @param query The parameters for this query.
     * @param options Options for this search.
     * @param progress A progress callback that must be invoked for each result.
     * @param token A cancellation token.
     */
    provideTextSearchResults(
      query: TextSearchQuery,
      options: TextSearchOptions,
      progress: Progress<TextSearchResult>,
      token: CancellationToken
    ): ProviderResult<TextSearchComplete>;
  }

  /**
   * A provider for AI-powered (semantic) text search results.
   *
   * **Note:** This is a proposed API — enable with `"aiTextSearchProvider"` in
   * `enabledApiProposals`.
   */
  export interface AITextSearchProvider {
    /**
     * Provide AI-powered search results that match the given natural-language query.
     *
     * Results appear in a dedicated "AI Results" section of the Search panel.
     *
     * @param query  The raw search query string entered by the user.
     * @param options  Options for this search (include/exclude globs, etc.).
     * @param progress  A progress callback that must be invoked for each result.
     * @param token  A cancellation token.
     */
    provideAITextSearchResults(
      query: string,
      options: TextSearchOptions,
      progress: Progress<TextSearchResult>,
      token: CancellationToken
    ): ProviderResult<TextSearchComplete>;
  }

  export namespace workspace {
    /**
     * Register a text search provider.
     *
     * **Note:** This is a proposed API.
     *
     * @param scheme The URI scheme to search in.
     * @param provider The provider.
     * @returns A {@link Disposable} that unregisters this provider when being disposed.
     */
    export function registerTextSearchProvider(
      scheme: string,
      provider: TextSearchProvider
    ): Disposable;

    /**
     * Register an AI text search provider.
     *
     * Results are displayed in a dedicated "AI Results" section of the Search panel.
     *
     * **Note:** This is a proposed API — enable with `"aiTextSearchProvider"` in
     * `enabledApiProposals`.
     *
     * @param scheme  The URI scheme to search within (e.g. `"file"`).
     * @param provider  The AI search provider.
     * @returns A {@link Disposable} that unregisters this provider when being disposed.
     */
    export function registerAITextSearchProvider(
      scheme: string,
      provider: AITextSearchProvider
    ): Disposable;
  }
}
