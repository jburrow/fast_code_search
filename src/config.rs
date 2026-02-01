//! Configuration management for fast_code_search
//!
//! Supports loading configuration from TOML files with CLI overrides.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,

    #[serde(default)]
    pub indexer: IndexerConfig,
}

/// Server-related configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Address to bind the gRPC server to
    #[serde(default = "default_address")]
    pub address: String,
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

    /// Glob patterns to exclude
    #[serde(default = "default_exclude_patterns")]
    pub exclude_patterns: Vec<String>,

    /// Maximum file size to index in bytes (default 10MB)
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
}

fn default_address() -> String {
    "0.0.0.0:50051".to_string()
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

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            indexer: IndexerConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            address: default_address(),
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
        }
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
