// Package web implements the REST/JSON API and WebSocket progress stream,
// equivalent to the Rust src/web/api.rs and src/web/mod.rs modules.
package web

import (
	"encoding/json"
	"fmt"
	"log/slog"
	"net/http"
	"strconv"
	"strings"

	"github.com/jburrow/fast_code_search/internal/config"
	"github.com/jburrow/fast_code_search/internal/diagnostics"
	"github.com/jburrow/fast_code_search/internal/search"
)

// SearchQuery mirrors the Rust SearchQuery REST request body.
type SearchQuery struct {
	Query           string   `json:"q"`
	MaxResults      int      `json:"max"`
	IsRegex         bool     `json:"regex"`
	CaseInsensitive bool     `json:"case_insensitive"`
	SymbolsOnly     bool     `json:"symbols"`
	IncludePaths    []string `json:"include"`
	ExcludePaths    []string `json:"exclude"`
}

// SearchResponse wraps search results for the REST endpoint.
type SearchResponse struct {
	Query   string               `json:"query"`
	Results []SearchResultJSON   `json:"results"`
	Total   int                  `json:"total"`
}

// SearchResultJSON is the JSON form of a single match.
type SearchResultJSON struct {
	FilePath         string  `json:"file_path"`
	Content          string  `json:"content"`
	LineNumber       int     `json:"line_number"`
	Score            float64 `json:"score"`
	MatchType        int     `json:"match_type"`
	MatchStart       int     `json:"match_start"`
	MatchEnd         int     `json:"match_end"`
	ContentTruncated bool    `json:"content_truncated,omitempty"`
}

// IndexRequest is the REST body for triggering a re-index.
type IndexRequest struct {
	Paths []string `json:"paths"`
}

// Router builds the HTTP handler tree.
func Router(engine *search.Engine, cfg *config.Config) http.Handler {
	mux := http.NewServeMux()

	api := &apiHandler{engine: engine, cfg: cfg}

	mux.HandleFunc("GET /health", api.healthCheck)
	mux.HandleFunc("GET /api/search", api.searchGET)
	mux.HandleFunc("POST /api/search", api.searchPOST)
	mux.HandleFunc("POST /api/index", api.indexPOST)
	mux.HandleFunc("GET /api/diagnostics", api.diagnosticsGET)
	mux.HandleFunc("GET /api/stats", api.statsGET)

	// Serve embedded static files if static directory is available.
	if cfg.Web.StaticPath != "" {
		fs := http.FileServer(http.Dir(cfg.Web.StaticPath))
		mux.Handle("/", fs)
	} else {
		mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
			if r.URL.Path == "/" {
				w.Header().Set("Content-Type", "text/html; charset=utf-8")
				fmt.Fprint(w, indexHTML)
				return
			}
			http.NotFound(w, r)
		})
	}

	// Wrap with CORS + logging middleware.
	return corsMiddleware(loggingMiddleware(mux))
}

// apiHandler groups all REST handler methods.
type apiHandler struct {
	engine *search.Engine
	cfg    *config.Config
}

func (a *apiHandler) healthCheck(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

func (a *apiHandler) statsGET(w http.ResponseWriter, r *http.Request) {
	stats := a.engine.Stats()
	writeJSON(w, http.StatusOK, stats)
}

func (a *apiHandler) diagnosticsGET(w http.ResponseWriter, r *http.Request) {
	report := diagnostics.Gather(a.engine)
	writeJSON(w, http.StatusOK, report)
}

// searchGET handles GET /api/search?q=...&max=...&regex=true etc.
func (a *apiHandler) searchGET(w http.ResponseWriter, r *http.Request) {
	q := r.URL.Query()
	sq := SearchQuery{
		Query:           q.Get("q"),
		IsRegex:         parseBool(q.Get("regex")),
		CaseInsensitive: parseBool(q.Get("case_insensitive")),
		SymbolsOnly:     parseBool(q.Get("symbols")),
	}
	if maxStr := q.Get("max"); maxStr != "" {
		if v, err := strconv.Atoi(maxStr); err == nil {
			sq.MaxResults = v
		}
	}
	if inc := q.Get("include"); inc != "" {
		sq.IncludePaths = strings.Split(inc, ",")
	}
	if exc := q.Get("exclude"); exc != "" {
		sq.ExcludePaths = strings.Split(exc, ",")
	}
	a.executeSearch(w, sq)
}

// searchPOST handles POST /api/search with a JSON body.
func (a *apiHandler) searchPOST(w http.ResponseWriter, r *http.Request) {
	var sq SearchQuery
	if err := json.NewDecoder(r.Body).Decode(&sq); err != nil {
		writeError(w, http.StatusBadRequest, "invalid JSON: "+err.Error())
		return
	}
	a.executeSearch(w, sq)
}

func (a *apiHandler) executeSearch(w http.ResponseWriter, sq SearchQuery) {
	if sq.Query == "" {
		writeError(w, http.StatusBadRequest, "query parameter 'q' is required")
		return
	}
	if sq.MaxResults <= 0 {
		sq.MaxResults = 100
	}

	opts := search.SearchOptions{
		Query:           sq.Query,
		MaxResults:      sq.MaxResults,
		IsRegex:         sq.IsRegex,
		CaseInsensitive: sq.CaseInsensitive,
		SymbolsOnly:     sq.SymbolsOnly,
		IncludePatterns: sq.IncludePaths,
		ExcludePatterns: sq.ExcludePaths,
	}

	var matches []search.SearchMatch
	var err error

	if sq.SymbolsOnly {
		matches = a.engine.SearchSymbols(sq.Query, sq.MaxResults)
	} else {
		matches, err = a.engine.Search(opts)
		if err != nil {
			slog.Error("search error", "query", sq.Query, "err", err)
			writeError(w, http.StatusInternalServerError, err.Error())
			return
		}
	}

	results := make([]SearchResultJSON, len(matches))
	for i, m := range matches {
		results[i] = SearchResultJSON{
			FilePath:         m.FilePath,
			Content:          m.Content,
			LineNumber:       m.LineNumber,
			Score:            m.Score,
			MatchType:        int(m.MatchType),
			MatchStart:       m.MatchStart,
			MatchEnd:         m.MatchEnd,
			ContentTruncated: m.ContentTruncated,
		}
	}
	writeJSON(w, http.StatusOK, SearchResponse{
		Query:   sq.Query,
		Results: results,
		Total:   len(results),
	})
}

func (a *apiHandler) indexPOST(w http.ResponseWriter, r *http.Request) {
	var req IndexRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid JSON: "+err.Error())
		return
	}
	if len(req.Paths) == 0 {
		writeError(w, http.StatusBadRequest, "paths must not be empty")
		return
	}
	n := a.engine.IndexBatch(req.Paths, 0)
	writeJSON(w, http.StatusOK, map[string]any{
		"files_indexed": n,
		"message":       "indexed successfully",
	})
}

// ── helpers ──────────────────────────────────────────────────────────────────

func writeJSON(w http.ResponseWriter, code int, v any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(code)
	enc := json.NewEncoder(w)
	enc.SetIndent("", "  ")
	if err := enc.Encode(v); err != nil {
		slog.Error("writeJSON encode error", "err", err)
	}
}

func writeError(w http.ResponseWriter, code int, msg string) {
	writeJSON(w, code, map[string]string{"error": msg})
}

func parseBool(s string) bool {
	s = strings.ToLower(strings.TrimSpace(s))
	return s == "true" || s == "1" || s == "yes"
}

func corsMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Access-Control-Allow-Origin", "*")
		w.Header().Set("Access-Control-Allow-Methods", "GET,POST,OPTIONS")
		w.Header().Set("Access-Control-Allow-Headers", "Content-Type,Authorization")
		if r.Method == http.MethodOptions {
			w.WriteHeader(http.StatusNoContent)
			return
		}
		next.ServeHTTP(w, r)
	})
}

func loggingMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		slog.Debug("http request", "method", r.Method, "path", r.URL.Path)
		next.ServeHTTP(w, r)
	})
}

// indexHTML is a minimal landing page served when no static directory is configured.
const indexHTML = `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>fast_code_search</title>
  <style>
    body { font-family: sans-serif; max-width: 700px; margin: 60px auto; }
    input { width: 60%; padding: 8px; font-size: 1rem; }
    button { padding: 8px 18px; font-size: 1rem; cursor: pointer; }
    pre { background:#f4f4f4; padding:12px; overflow-x:auto; border-radius:4px; }
  </style>
</head>
<body>
  <h1>fast_code_search</h1>
  <p>High-performance in-memory code search service — Go implementation.</p>
  <input id="q" placeholder="Search query…" />
  <button onclick="doSearch()">Search</button>
  <pre id="out"></pre>
  <script>
    async function doSearch() {
      const q = document.getElementById('q').value;
      const res = await fetch('/api/search?q=' + encodeURIComponent(q) + '&max=20');
      const data = await res.json();
      document.getElementById('out').textContent = JSON.stringify(data, null, 2);
    }
    document.getElementById('q').addEventListener('keydown', e => {
      if (e.key === 'Enter') doSearch();
    });
  </script>
</body>
</html>`
