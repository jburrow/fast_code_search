/**
 * TypeScript interfaces matching the fast_code_search REST API response types.
 */

/** A single keyword search result from the fast_code_search server (port 8080). */
export interface KeywordSearchResult {
  file_path: string;
  content: string;
  line_number: number;
  match_start: number;
  match_end: number;
  content_truncated: boolean;
  score: number;
  match_type: "TEXT" | "SYMBOL_DEFINITION" | "SYMBOL_REFERENCE";
  dependency_count: number;
}

/** Response from `GET /api/search` on the keyword server. */
export interface KeywordSearchResponse {
  results: KeywordSearchResult[];
  query: string;
  total_results: number;
  elapsed_ms: number;
  rank_mode?: string;
  total_candidates?: number;
  candidates_searched?: number;
}

/** Response from `GET /api/health` on the keyword server. */
export interface KeywordHealthResponse {
  status: string;
  version: string;
}

/** Response from `GET /api/stats` on the keyword server. */
export interface KeywordStatsResponse {
  num_files: number;
  total_size: number;
  num_trigrams: number;
  dependency_edges: number;
  total_content_bytes: number;
}

/** A single semantic search result from the semantic server (port 8081). */
export interface SemanticSearchResult {
  file_path: string;
  content: string;
  start_line: number;
  end_line: number;
  similarity_score: number;
  chunk_type: "FIXED" | "FUNCTION" | "CLASS" | "MODULE";
  symbol_name?: string;
}

/** Response from `GET /api/search` on the semantic server. */
export interface SemanticSearchResponse {
  results: SemanticSearchResult[];
  query: string;
  total_results: number;
  elapsed_ms: number;
}

/** Response from `GET /api/health` on the semantic server. */
export interface SemanticHealthResponse {
  status: string;
  version: string;
}

/** Response from `GET /api/stats` on the semantic server. */
export interface SemanticStatsResponse {
  num_files: number;
  num_chunks: number;
  embedding_dim: number;
  cache_size: number;
}

/** Parameters for a keyword search request. */
export interface KeywordSearchParams {
  q: string;
  max?: number;
  include?: string;
  regex?: boolean;
  symbols?: boolean;
  exclude?: string;
}

/** Parameters for a semantic search request. */
export interface SemanticSearchParams {
  q: string;
  max?: number;
}
