// Package config provides configuration management for fast_code_search.
// It supports TOML file-based configuration with environment variable and
// CLI override support, mirroring the Rust src/config.rs module.
package config

import (
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/BurntSushi/toml"
)

// Config is the root configuration structure, equivalent to Rust's Config.
type Config struct {
	Server    ServerConfig    `toml:"server"`
	Indexer   IndexerConfig   `toml:"indexer"`
	Telemetry TelemetryConfig `toml:"telemetry"`
	Web       WebConfig       `toml:"web"`
}

// ServerConfig contains gRPC server settings.
type ServerConfig struct {
	// GRPCAddr is the address the gRPC server listens on.
	GRPCAddr string `toml:"grpc_addr"`
	// WebAddr is the address the REST/WebUI server listens on.
	WebAddr string `toml:"web_addr"`
	// MaxResults caps results returned per query.
	MaxResults int `toml:"max_results"`
	// StreamChunkSize controls how many results are batched per gRPC stream message.
	StreamChunkSize int `toml:"stream_chunk_size"`
}

// IndexerConfig controls how the search engine indexes files.
type IndexerConfig struct {
	// Paths is the list of directories/files to index.
	Paths []string `toml:"paths"`
	// IncludePatterns are glob patterns for files to include.
	IncludePatterns []string `toml:"include_patterns"`
	// ExcludePatterns are glob patterns for files to exclude.
	ExcludePatterns []string `toml:"exclude_patterns"`
	// MaxFileSizeBytes is the upper limit for indexable file size.
	MaxFileSizeBytes int64 `toml:"max_file_size_bytes"`
	// EnableSymbols controls tree-sitter symbol extraction.
	EnableSymbols bool `toml:"enable_symbols"`
	// EnableDependencies controls import graph construction.
	EnableDependencies bool `toml:"enable_dependencies"`
	// WatchForChanges enables live filesystem monitoring.
	WatchForChanges bool `toml:"watch_for_changes"`
	// PersistIndex enables saving the index to disk.
	PersistIndex bool `toml:"persist_index"`
	// IndexPath is where the index file is saved.
	IndexPath string `toml:"index_path"`
	// BatchSize is how many files are indexed per background batch.
	BatchSize int `toml:"batch_size"`
	// NumWorkers is the number of parallel indexing goroutines.
	NumWorkers int `toml:"num_workers"`
	// DebounceDuration for filesystem event batching.
	DebounceDuration time.Duration `toml:"debounce_duration"`
}

// TelemetryConfig controls OpenTelemetry tracing.
type TelemetryConfig struct {
	Enabled      bool   `toml:"enabled"`
	OTLPEndpoint string `toml:"otlp_endpoint"`
	ServiceName  string `toml:"service_name"`
}

// WebConfig controls the built-in web UI.
type WebConfig struct {
	Enabled    bool   `toml:"enabled"`
	StaticPath string `toml:"static_path"`
}

// DefaultConfig returns a Config populated with sensible defaults.
func DefaultConfig() *Config {
	home, _ := os.UserHomeDir()
	indexPath := filepath.Join(home, ".fast_code_search", "index.bin")

	return &Config{
		Server: ServerConfig{
			GRPCAddr:        "0.0.0.0:50051",
			WebAddr:         "0.0.0.0:8080",
			MaxResults:      100,
			StreamChunkSize: 10,
		},
		Indexer: IndexerConfig{
			Paths: []string{"."},
			IncludePatterns: []string{
				"**/*.rs", "**/*.go", "**/*.py", "**/*.js", "**/*.ts",
				"**/*.java", "**/*.c", "**/*.cpp", "**/*.h", "**/*.hpp",
				"**/*.cs", "**/*.rb", "**/*.php", "**/*.sh", "**/*.toml",
				"**/*.yaml", "**/*.yml", "**/*.json", "**/*.md",
			},
			ExcludePatterns: []string{
				"**/target/**", "**/node_modules/**", "**/.git/**",
				"**/__pycache__/**", "**/vendor/**", "**/dist/**",
				"**/*.min.js", "**/*.min.css",
			},
			MaxFileSizeBytes:   1 << 20, // 1 MiB
			EnableSymbols:      true,
			EnableDependencies: true,
			WatchForChanges:    false,
			PersistIndex:       true,
			IndexPath:          indexPath,
			BatchSize:          200,
			NumWorkers:         4,
			DebounceDuration:   500 * time.Millisecond,
		},
		Telemetry: TelemetryConfig{
			Enabled:     false,
			ServiceName: "fast_code_search",
		},
		Web: WebConfig{
			Enabled: true,
		},
	}
}

// Load reads configuration from a TOML file, overlaying defaults.
// Returns an error if the file exists but cannot be parsed.
func Load(path string) (*Config, error) {
	cfg := DefaultConfig()

	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return cfg, nil
		}
		return nil, fmt.Errorf("reading config %q: %w", path, err)
	}

	if _, err := toml.Decode(string(data), cfg); err != nil {
		return nil, fmt.Errorf("parsing config %q: %w", path, err)
	}
	return cfg, nil
}

// GenerateTemplate writes a commented TOML template to the given path.
func GenerateTemplate(path string) error {
	const template = `# fast_code_search configuration
# Generated template — edit as needed.

[server]
grpc_addr = "0.0.0.0:50051"
web_addr  = "0.0.0.0:8080"
max_results = 100
stream_chunk_size = 10

[indexer]
paths = ["."]
include_patterns = ["**/*.rs","**/*.go","**/*.py","**/*.js","**/*.ts"]
exclude_patterns = ["**/target/**","**/node_modules/**","**/.git/**"]
max_file_size_bytes = 1048576
enable_symbols      = true
enable_dependencies = true
watch_for_changes   = false
persist_index       = true
batch_size          = 200
num_workers         = 4

[telemetry]
enabled      = false
service_name = "fast_code_search"
# otlp_endpoint = "http://localhost:4317"

[web]
enabled = true
`
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	return os.WriteFile(path, []byte(template), 0o644)
}
