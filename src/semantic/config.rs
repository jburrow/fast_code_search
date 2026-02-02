//! Configuration management for semantic search
//!
//! Supports loading configuration from TOML files with CLI overrides,
//! mirroring the pattern used in the traditional search config.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Main semantic search configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SemanticConfig {
    #[serde(default)]
    pub server: SemanticServerConfig,

    #[serde(default)]
    pub indexer: SemanticIndexerConfig,
}

/// Server-related configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticServerConfig {
    /// Address to bind the gRPC server to
    #[serde(default = "default_grpc_address")]
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
pub struct SemanticIndexerConfig {
    /// Paths to index on startup
    #[serde(default)]
    pub paths: Vec<String>,

    /// Glob patterns to exclude
    #[serde(default = "default_exclude_patterns")]
    pub exclude_patterns: Vec<String>,

    /// Chunk size in lines (for now, simple line-based chunking)
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,

    /// Overlap between chunks in lines
    #[serde(default = "default_chunk_overlap")]
    pub chunk_overlap: usize,

    /// Path to persistent index storage (if set, index will be saved/loaded)
    #[serde(default)]
    pub index_path: Option<String>,
}

fn default_grpc_address() -> String {
    "0.0.0.0:50052".to_string()
}

fn default_web_address() -> String {
    "0.0.0.0:8081".to_string()
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

fn default_chunk_size() -> usize {
    50 // 50 lines per chunk
}

fn default_chunk_overlap() -> usize {
    5 // 5 lines overlap
}

impl Default for SemanticServerConfig {
    fn default() -> Self {
        Self {
            address: default_grpc_address(),
            web_address: default_web_address(),
            enable_web_ui: default_enable_web_ui(),
        }
    }
}

impl Default for SemanticIndexerConfig {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            exclude_patterns: default_exclude_patterns(),
            chunk_size: default_chunk_size(),
            chunk_overlap: default_chunk_overlap(),
            index_path: None,
        }
    }
}

impl SemanticConfig {
    /// Load configuration from a file
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: SemanticConfig = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        Ok(config)
    }

    /// Try to load configuration from default locations
    ///
    /// Search order:
    /// 1. FCS_SEMANTIC_CONFIG environment variable
    /// 2. ./fast_code_search_semantic.toml (current directory)
    /// 3. ~/.config/fast_code_search/semantic.toml (user config)
    pub fn from_default_locations() -> Result<Option<(Self, PathBuf)>> {
        // Check environment variable first
        if let Ok(env_path) = std::env::var("FCS_SEMANTIC_CONFIG") {
            let path = PathBuf::from(&env_path);
            if path.exists() {
                let config = Self::from_file(&path)?;
                return Ok(Some((config, path)));
            }
        }

        // Check current directory
        let local_path = PathBuf::from("fast_code_search_semantic.toml");
        if local_path.exists() {
            let config = Self::from_file(&local_path)?;
            return Ok(Some((config, local_path)));
        }

        // Check user config directory
        if let Some(config_dir) = dirs::config_dir() {
            let user_path = config_dir
                .join("fast_code_search")
                .join("semantic.toml");
            if user_path.exists() {
                let config = Self::from_file(&user_path)?;
                return Ok(Some((config, user_path)));
            }
        }

        Ok(None)
    }

    /// Generate a template configuration file
    pub fn generate_template() -> String {
        r#"# Fast Code Search - Semantic Search Configuration
# Generated template - customize as needed

[server]
# Address to bind the gRPC server to
address = "0.0.0.0:50052"

# Address to bind the HTTP/Web UI server to
web_address = "0.0.0.0:8081"

# Enable the web UI and REST API
enable_web_ui = true

[indexer]
# Paths to index on startup
# Add your project directories here
paths = [
    # "C:/code/my-project",
    # "C:/code/another-project",
    # "/home/user/projects/my-app",
]

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

# Chunk size in lines (default: 50)
chunk_size = 50

# Overlap between chunks in lines (default: 5)
chunk_overlap = 5

# Path to persistent index storage (optional)
# If set, the index will be saved to disk and loaded on restart
# index_path = "/var/lib/fast_code_search_semantic/index"
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
        let config = SemanticConfig::default();
        assert_eq!(config.server.address, "0.0.0.0:50052");
        assert_eq!(config.server.web_address, "0.0.0.0:8081");
        assert!(config.indexer.paths.is_empty());
        assert!(!config.indexer.exclude_patterns.is_empty());
    }

    #[test]
    fn test_parse_minimal_config() {
        let toml = r#"
[server]
address = "127.0.0.1:50052"

[indexer]
paths = ["/code/project"]
"#;
        let config: SemanticConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.server.address, "127.0.0.1:50052");
        assert_eq!(config.indexer.paths, vec!["/code/project"]);
    }

    #[test]
    fn test_generate_template() {
        let template = SemanticConfig::generate_template();
        assert!(template.contains("[server]"));
        assert!(template.contains("[indexer]"));
        assert!(template.contains("paths"));
    }
}
