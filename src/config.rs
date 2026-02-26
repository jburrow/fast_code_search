//! Configuration management for fast_code_search
//!
//! Supports loading configuration from TOML files with CLI overrides.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::utils::normalize_path_for_comparison;

/// Telemetry / OpenTelemetry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    /// Enable OpenTelemetry trace export (default: false)
    /// Can be overridden by env var FCS_TRACING_ENABLED or OTEL_SDK_DISABLED
    #[serde(default)]
    pub enabled: bool,

    /// OTLP exporter endpoint (default: http://localhost:4317)
    /// Can be overridden by env var OTEL_EXPORTER_OTLP_ENDPOINT
    #[serde(default = "default_otlp_endpoint")]
    pub otlp_endpoint: String,

    /// Service name reported to the collector (default: fast_code_search)
    /// Can be overridden by env var OTEL_SERVICE_NAME
    #[serde(default = "default_service_name")]
    pub service_name: String,
}

fn default_otlp_endpoint() -> String {
    "http://localhost:4317".to_string()
}

fn default_service_name() -> String {
    "fast_code_search".to_string()
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            otlp_endpoint: default_otlp_endpoint(),
            service_name: default_service_name(),
        }
    }
}

impl TelemetryConfig {
    /// Apply environment variable overrides.
    /// Env vars take precedence over TOML config values.
    pub fn with_env_overrides(mut self) -> Self {
        // OTEL_SDK_DISABLED=true → disabled (official OTel convention)
        if let Ok(val) = std::env::var("OTEL_SDK_DISABLED") {
            if val.eq_ignore_ascii_case("true") {
                self.enabled = false;
            }
        }
        // FCS_TRACING_ENABLED=false → disabled (project-specific kill-switch)
        if let Ok(val) = std::env::var("FCS_TRACING_ENABLED") {
            self.enabled = val.eq_ignore_ascii_case("true") || val == "1";
        }
        if let Ok(val) = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
            if !val.is_empty() {
                self.otlp_endpoint = val;
            }
        }
        if let Ok(val) = std::env::var("OTEL_SERVICE_NAME") {
            if !val.is_empty() {
                self.service_name = val;
            }
        }
        self
    }
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,

    #[serde(default)]
    pub indexer: IndexerConfig,

    #[serde(default)]
    pub telemetry: TelemetryConfig,
}

/// Server-related configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Address to bind the gRPC server to
    #[serde(default = "default_address")]
    pub address: String,

    /// Address to bind the HTTP/Web UI server to
    #[serde(default = "default_web_address")]
    pub web_address: String,

    /// Enable the web UI and REST API
    #[serde(default = "default_enable_web_ui")]
    pub enable_web_ui: bool,
}

/// Indexer-related configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerConfig {
    /// Paths to index on startup
    #[serde(default)]
    pub paths: Vec<String>,

    /// File extensions to include (empty means all text files)
    #[serde(default)]
    pub include_extensions: Vec<String>,

    /// Glob-like patterns to exclude (matched as path substrings during discovery)
    #[serde(default = "default_exclude_patterns")]
    pub exclude_patterns: Vec<String>,

    /// Maximum file size to index in bytes (default 10MB)
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,

    /// Path to persistent index storage (if set, index will be saved/loaded)
    #[serde(default)]
    pub index_path: Option<String>,

    /// Enable file watcher for incremental indexing
    #[serde(default)]
    pub watch: bool,

    /// Save index after initial build completes (default: true if index_path is set)
    #[serde(default = "default_true")]
    pub save_after_build: bool,

    /// Save index after N file updates (0 = disabled, off by default)
    /// When enabled, the index is periodically saved after this many files are updated
    #[serde(default)]
    pub save_after_updates: usize,

    /// Exact file paths to permanently exclude from indexing.
    /// Use this to skip files that cause crashes or other issues.
    /// After a crash, check `fcs_last_processed.txt` in the working directory
    /// to identify the offending file, then add its absolute path here.
    ///
    /// Example:
    ///   exclude_files = ["/repo/src/generated/huge_file.rs"]
    #[serde(default)]
    pub exclude_files: Vec<String>,

    /// Enable encoding detection for non-UTF-8 text files (default: true).
    /// When enabled, files in encodings like Latin-1, Shift-JIS, UTF-16 etc.
    /// are automatically transcoded to UTF-8 for indexing.
    /// Disable this if you only work with UTF-8 codebases for slightly faster indexing.
    #[serde(default = "default_true")]
    pub transcode_non_utf8: bool,
}

fn default_address() -> String {
    "0.0.0.0:50051".to_string()
}

fn default_web_address() -> String {
    "0.0.0.0:8080".to_string()
}

fn default_enable_web_ui() -> bool {
    true
}

fn default_exclude_patterns() -> Vec<String> {
    vec![
        "**/node_modules/**".to_string(),
        "**/target/**".to_string(),
        "**/.git/**".to_string(),
        "**/build/**".to_string(),
        "**/dist/**".to_string(),
        "**/__pycache__/**".to_string(),
        "**/venv/**".to_string(),
        "**/.venv/**".to_string(),
    ]
}

fn default_max_file_size() -> u64 {
    10 * 1024 * 1024 // 10MB
}

fn default_true() -> bool {
    true
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            address: default_address(),
            web_address: default_web_address(),
            enable_web_ui: default_enable_web_ui(),
        }
    }
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            include_extensions: Vec::new(),
            exclude_patterns: default_exclude_patterns(),
            max_file_size: default_max_file_size(),
            index_path: None,
            watch: false,
            save_after_build: true,
            save_after_updates: 0, // Disabled by default
            exclude_files: Vec::new(),
            transcode_non_utf8: true,
        }
    }
}

impl IndexerConfig {
    /// Generate a fingerprint of the indexer configuration
    /// Used to detect config changes that require re-indexing
    pub fn fingerprint(&self) -> String {
        // Normalize and sort paths for consistent fingerprinting
        let mut sorted_paths: Vec<_> = self
            .paths
            .iter()
            .map(|p| normalize_path_for_comparison(p))
            .collect();
        sorted_paths.sort();

        // Sort extensions
        let mut sorted_exts = self.include_extensions.to_vec();
        sorted_exts.sort();

        // Sort exclude patterns
        let mut sorted_excludes = self.exclude_patterns.to_vec();
        sorted_excludes.sort();

        // Sort excluded files
        let mut sorted_excluded_files = self.exclude_files.to_vec();
        sorted_excluded_files.sort();

        // Create a deterministic string representation
        let config_str = format!(
            "paths:{:?}|exts:{:?}|excludes:{:?}|max_size:{}|exclude_files:{:?}|transcode_non_utf8:{}",
            sorted_paths, sorted_exts, sorted_excludes, self.max_file_size, sorted_excluded_files, self.transcode_non_utf8
        );

        // Generate MD5 hash
        format!("{:x}", md5::compute(config_str.as_bytes()))
    }

    /// Check if a file path is explicitly excluded via `exclude_files`.
    pub fn is_file_excluded(&self, path: &std::path::Path) -> bool {
        if self.exclude_files.is_empty() {
            return false;
        }
        let path_str = path.to_string_lossy().replace('\\', "/");
        self.exclude_files.iter().any(|excluded| {
            let excluded_normalized = excluded.replace('\\', "/");
            path_str == excluded_normalized
        })
    }

    /// Check if a path is within the configured index paths
    pub fn is_path_in_scope(&self, path: &std::path::Path) -> bool {
        let path_str = path.to_string_lossy().replace('\\', "/").to_lowercase();
        self.paths.iter().any(|base| {
            let base_normalized = base.replace('\\', "/").to_lowercase();
            path_str.starts_with(&base_normalized)
        })
    }
}

impl Config {
    /// Load configuration from a file
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        Ok(config)
    }

    /// Try to load configuration from default locations
    ///
    /// Search order:
    /// 1. FCS_CONFIG environment variable
    /// 2. ./fast_code_search.toml (current directory)
    /// 3. ~/.config/fast_code_search/config.toml (user config)
    pub fn from_default_locations() -> Result<Option<(Self, PathBuf)>> {
        // Check environment variable first
        if let Ok(env_path) = std::env::var("FCS_CONFIG") {
            let path = PathBuf::from(&env_path);
            if path.exists() {
                let config = Self::from_file(&path)?;
                return Ok(Some((config, path)));
            }
        }

        // Check current directory
        let local_path = PathBuf::from("fast_code_search.toml");
        if local_path.exists() {
            let config = Self::from_file(&local_path)?;
            return Ok(Some((config, local_path)));
        }

        // Check user config directory
        if let Some(config_dir) = dirs::config_dir() {
            let user_path = config_dir.join("fast_code_search").join("config.toml");
            if user_path.exists() {
                let config = Self::from_file(&user_path)?;
                return Ok(Some((config, user_path)));
            }
        }

        Ok(None)
    }

    /// Generate a template configuration file
    pub fn generate_template() -> String {
        r#"# Fast Code Search Configuration
# Generated template - customize as needed

[server]
# Address to bind the gRPC server to
address = "0.0.0.0:50051"

[indexer]
# Paths to index on startup
# Add your project directories here
paths = [
    # "C:/code/my-project",
    # "C:/code/another-project",
    # "/home/user/projects/my-app",
]

# File extensions to include (empty = all text files)
# Uncomment and customize to limit indexed file types
# include_extensions = ["rs", "py", "js", "ts", "go", "c", "cpp", "h", "java"]

# Patterns to exclude from indexing
exclude_patterns = [
    "**/node_modules/**",
    "**/target/**",
    "**/.git/**",
    "**/build/**",
    "**/dist/**",
    "**/__pycache__/**",
    "**/venv/**",
    "**/.venv/**",
]

# Maximum file size to index in bytes (default: 10MB)
max_file_size = 10485760

# Path to persistent index storage (optional)
# If set, the index will be saved to disk and loaded on restart for faster startup
# The index file stores trigrams, file metadata, and config fingerprint for reconciliation
# index_path = "/var/lib/fast_code_search/index.bin"

# Save index after initial build completes (default: true)
# Only effective when index_path is set
# save_after_build = true

# Save index after N file updates via watcher (default: 0 = disabled)
# When enabled with a non-zero value, the index is periodically saved after this many files are updated
# Useful for long-running servers to persist incremental changes
# save_after_updates = 0

# Enable file watcher for incremental indexing (default: false)
# When enabled, changes to indexed files are detected and re-indexed automatically
# watch = false

[telemetry]
# Enable OpenTelemetry trace export (default: false)
# Set to true to enable OTLP export (console logging is always active)
# Env overrides: OTEL_SDK_DISABLED=true, FCS_TRACING_ENABLED=true
enabled = false

# OTLP gRPC exporter endpoint (default: http://localhost:4317)
# Env override: OTEL_EXPORTER_OTLP_ENDPOINT
otlp_endpoint = "http://localhost:4317"

# Service name reported to the collector
# Env override: OTEL_SERVICE_NAME
service_name = "fast_code_search"
"#
        .to_string()
    }

    /// Write template config to the specified path
    pub fn write_template(path: &Path) -> Result<()> {
        let template = Self::generate_template();

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory: {}", parent.display())
            })?;
        }

        std::fs::write(path, template)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;

        Ok(())
    }

    /// Merge CLI overrides into the configuration
    pub fn with_overrides(mut self, address: Option<String>, extra_paths: Vec<String>) -> Self {
        if let Some(addr) = address {
            self.server.address = addr;
        }

        // Append extra paths from CLI
        self.indexer.paths.extend(extra_paths);

        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.server.address, "0.0.0.0:50051");
        assert!(config.indexer.paths.is_empty());
        assert!(!config.indexer.exclude_patterns.is_empty());
    }

    #[test]
    fn test_parse_minimal_config() {
        let toml = r#"
[server]
address = "127.0.0.1:8080"

[indexer]
paths = ["/code/project"]
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.server.address, "127.0.0.1:8080");
        assert_eq!(config.indexer.paths, vec!["/code/project"]);
    }

    #[test]
    fn test_generate_template() {
        let template = Config::generate_template();
        assert!(template.contains("[server]"));
        assert!(template.contains("[indexer]"));
        assert!(template.contains("paths"));
    }
}
