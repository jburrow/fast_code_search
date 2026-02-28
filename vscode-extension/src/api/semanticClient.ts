/**
 * REST client for the fast_code_search semantic server (default port 8081).
 *
 * Endpoints used:
 *   GET /api/search  - natural-language semantic search
 *   GET /api/health  - liveness check
 *   GET /api/stats   - index statistics
 */

import {
  SemanticHealthResponse,
  SemanticSearchParams,
  SemanticSearchResponse,
  SemanticStatsResponse,
} from "./types.js";

export class SemanticSearchClient {
  private baseUrl: string;

  constructor(host: string, port: number) {
    this.baseUrl = `http://${host}:${port}`;
  }

  /** Update the server address (called when settings change). */
  updateAddress(host: string, port: number): void {
    this.baseUrl = `http://${host}:${port}`;
  }

  /**
   * Execute a semantic search.
   *
   * @param params  Search parameters.
   * @param signal  Optional AbortSignal for cancellation.
   */
  async search(
    params: SemanticSearchParams,
    signal?: AbortSignal
  ): Promise<SemanticSearchResponse> {
    const url = new URL("/api/search", this.baseUrl);
    url.searchParams.set("q", params.q);
    if (params.max !== undefined) {
      url.searchParams.set("max", String(params.max));
    }

    const response = await fetch(url.toString(), { signal });
    if (!response.ok) {
      throw new Error(
        `Semantic search request failed: ${response.status} ${response.statusText}`
      );
    }
    return response.json() as Promise<SemanticSearchResponse>;
  }

  /** Check that the semantic server is reachable. */
  async health(signal?: AbortSignal): Promise<SemanticHealthResponse> {
    const url = new URL("/api/health", this.baseUrl);
    const response = await fetch(url.toString(), { signal });
    if (!response.ok) {
      throw new Error(
        `Health check failed: ${response.status} ${response.statusText}`
      );
    }
    return response.json() as Promise<SemanticHealthResponse>;
  }

  /** Retrieve index statistics from the semantic server. */
  async stats(signal?: AbortSignal): Promise<SemanticStatsResponse> {
    const url = new URL("/api/stats", this.baseUrl);
    const response = await fetch(url.toString(), { signal });
    if (!response.ok) {
      throw new Error(
        `Stats request failed: ${response.status} ${response.statusText}`
      );
    }
    return response.json() as Promise<SemanticStatsResponse>;
  }
}
