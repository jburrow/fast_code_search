/**
 * REST client for the fast_code_search keyword server (default port 8080).
 *
 * Endpoints used:
 *   GET /api/search  - text / regex / symbol search
 *   GET /api/health  - liveness check
 *   GET /api/stats   - index statistics
 */

import {
  KeywordHealthResponse,
  KeywordSearchParams,
  KeywordSearchResponse,
  KeywordStatsResponse,
} from "./types.js";

export class KeywordSearchClient {
  private baseUrl: string;

  constructor(host: string, port: number) {
    this.baseUrl = `http://${host}:${port}`;
  }

  /** Update the server address (called when settings change). */
  updateAddress(host: string, port: number): void {
    this.baseUrl = `http://${host}:${port}`;
  }

  /**
   * Execute a keyword search.
   *
   * @param params   Search parameters.
   * @param signal   Optional AbortSignal for cancellation.
   */
  async search(
    params: KeywordSearchParams,
    signal?: AbortSignal
  ): Promise<KeywordSearchResponse> {
    const url = new URL("/api/search", this.baseUrl);
    url.searchParams.set("q", params.q);
    if (params.max !== undefined) {
      url.searchParams.set("max", String(params.max));
    }
    if (params.include) {
      url.searchParams.set("include", params.include);
    }
    if (params.exclude) {
      url.searchParams.set("exclude", params.exclude);
    }
    if (params.regex) {
      url.searchParams.set("regex", "true");
    }
    if (params.symbols) {
      url.searchParams.set("symbols", "true");
    }

    const response = await fetch(url.toString(), { signal });
    if (!response.ok) {
      throw new Error(
        `Keyword search request failed: ${response.status} ${response.statusText}`
      );
    }
    return response.json() as Promise<KeywordSearchResponse>;
  }

  /** Check that the keyword server is reachable. */
  async health(signal?: AbortSignal): Promise<KeywordHealthResponse> {
    const url = new URL("/api/health", this.baseUrl);
    const response = await fetch(url.toString(), { signal });
    if (!response.ok) {
      throw new Error(
        `Health check failed: ${response.status} ${response.statusText}`
      );
    }
    return response.json() as Promise<KeywordHealthResponse>;
  }

  /** Retrieve index statistics from the keyword server. */
  async stats(signal?: AbortSignal): Promise<KeywordStatsResponse> {
    const url = new URL("/api/stats", this.baseUrl);
    const response = await fetch(url.toString(), { signal });
    if (!response.ok) {
      throw new Error(
        `Stats request failed: ${response.status} ${response.statusText}`
      );
    }
    return response.json() as Promise<KeywordStatsResponse>;
  }
}
