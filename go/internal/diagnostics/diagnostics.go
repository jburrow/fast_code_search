// Package diagnostics provides detailed server health and index statistics,
// equivalent to the Rust src/diagnostics/mod.rs module.
package diagnostics

import (
	"runtime"
	"time"

	"github.com/jburrow/fast_code_search/internal/search"
)

// HealthStatus describes the current server state.
type HealthStatus struct {
	Status    string `json:"status"`
	Uptime    string `json:"uptime"`
	StartedAt string `json:"started_at"`
}

// IndexStats summarises the search index.
type IndexStats struct {
	FilesIndexed  int    `json:"files_indexed"`
	TotalBytes    int64  `json:"total_bytes"`
	NumTrigrams   int    `json:"num_trigrams"`
	TotalPostings uint64 `json:"total_postings"`
	NumSymbols    int    `json:"num_symbols"`
}

// SystemInfo reports OS and runtime metrics.
type SystemInfo struct {
	OS          string `json:"os"`
	Arch        string `json:"arch"`
	NumCPU      int    `json:"num_cpu"`
	GoVersion   string `json:"go_version"`
	HeapAllocMB float64 `json:"heap_alloc_mb"`
}

// DiagnosticsReport is the full diagnostics payload.
type DiagnosticsReport struct {
	Health     HealthStatus `json:"health"`
	Index      IndexStats   `json:"index"`
	System     SystemInfo   `json:"system"`
	GeneratedAt string      `json:"generated_at"`
}

var startTime = time.Now()

// Gather builds a complete DiagnosticsReport from the given engine.
func Gather(engine *search.Engine) DiagnosticsReport {
	stats := engine.Stats()
	uptime := time.Since(startTime).Truncate(time.Second).String()

	var ms runtime.MemStats
	runtime.ReadMemStats(&ms)

	return DiagnosticsReport{
		Health: HealthStatus{
			Status:    "ok",
			Uptime:    uptime,
			StartedAt: startTime.UTC().Format(time.RFC3339),
		},
		Index: IndexStats{
			FilesIndexed:  stats.FilesIndexed,
			TotalBytes:    stats.TotalBytes,
			NumTrigrams:   stats.NumTrigrams,
			TotalPostings: stats.TotalPostings,
			NumSymbols:    stats.NumSymbols,
		},
		System: SystemInfo{
			OS:          runtime.GOOS,
			Arch:        runtime.GOARCH,
			NumCPU:      runtime.NumCPU(),
			GoVersion:   runtime.Version(),
			HeapAllocMB: float64(ms.HeapAlloc) / (1 << 20),
		},
		GeneratedAt: time.Now().UTC().Format(time.RFC3339),
	}
}
