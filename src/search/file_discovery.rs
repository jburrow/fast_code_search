//! File discovery utilities for walking directory trees and filtering files.
//!
//! Provides a unified file discovery mechanism used by both the keyword and semantic
//! search indexers. Handles exclude patterns, binary file detection, and large file filtering.

use crate::utils::{get_binary_extensions, has_binary_extension};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Configuration for file discovery.
#[derive(Debug, Clone)]
pub struct FileDiscoveryConfig {
    /// Paths to search for files.
    pub paths: Vec<String>,

    /// Patterns to exclude (will be matched against path strings).
    /// Common patterns: "**/node_modules/**", "**/target/**", "**/.git/**"
    pub exclude_patterns: Vec<String>,

    /// Maximum file size to include (in bytes). Files larger than this are skipped.
    /// Default is 10MB (10 * 1024 * 1024).
    pub max_file_size: Option<u64>,

    /// Additional binary extensions to skip (merged with default list).
    pub extra_binary_extensions: Vec<String>,
}

impl Default for FileDiscoveryConfig {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            exclude_patterns: Vec::new(),
            max_file_size: Some(10 * 1024 * 1024), // 10MB
            extra_binary_extensions: Vec::new(),
        }
    }
}

impl FileDiscoveryConfig {
    /// Create a config from indexer paths and exclude patterns.
    pub fn new(paths: Vec<String>, exclude_patterns: Vec<String>) -> Self {
        Self {
            paths,
            exclude_patterns,
            ..Default::default()
        }
    }

    /// Pre-compile exclude patterns for efficient matching.
    /// Strips leading/trailing wildcards and slashes for substring matching.
    fn compile_exclude_patterns(&self) -> Vec<String> {
        self.exclude_patterns
            .iter()
            .map(|p| p.trim_matches('*').trim_matches('/').to_string())
            .collect()
    }

    /// Get the combined set of binary extensions (default + extra).
    fn get_all_binary_extensions(&self) -> HashSet<String> {
        let mut extensions: HashSet<String> = get_binary_extensions()
            .iter()
            .map(|s| s.to_string())
            .collect();

        for ext in &self.extra_binary_extensions {
            extensions.insert(ext.to_lowercase());
        }

        extensions
    }
}

/// Iterator over discovered files matching the configuration criteria.
pub struct FileDiscoveryIterator {
    /// Stack of walkdir iterators (one per path).
    walkers: Vec<walkdir::IntoIter>,

    /// Compiled exclude patterns for substring matching.
    exclude_patterns: Vec<String>,

    /// Binary extensions to skip.
    binary_extensions: HashSet<String>,

    /// Maximum file size (None = no limit).
    max_file_size: Option<u64>,
}

impl FileDiscoveryIterator {
    /// Create a new file discovery iterator from the given configuration.
    pub fn new(config: &FileDiscoveryConfig) -> Self {
        let walkers: Vec<walkdir::IntoIter> = config
            .paths
            .iter()
            .filter_map(|path_str| {
                let path = Path::new(path_str);
                if path.exists() {
                    Some(WalkDir::new(path).follow_links(true).into_iter())
                } else {
                    tracing::warn!(path = %path_str, "Path does not exist, skipping");
                    None
                }
            })
            .collect();

        Self {
            walkers,
            exclude_patterns: config.compile_exclude_patterns(),
            binary_extensions: config.get_all_binary_extensions(),
            max_file_size: config.max_file_size,
        }
    }

    /// Check if a path matches any exclude pattern.
    fn is_excluded(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        self.exclude_patterns
            .iter()
            .any(|pattern| path_str.contains(pattern))
    }

    /// Check if a file has a binary extension.
    fn has_binary_ext(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            let ext = ext.to_string_lossy().to_lowercase();
            self.binary_extensions.contains(&ext)
        } else {
            false
        }
    }

    /// Check if a file exceeds the size limit.
    fn exceeds_size_limit(&self, path: &Path) -> bool {
        if let Some(max_size) = self.max_file_size {
            if let Ok(metadata) = path.metadata() {
                return metadata.len() > max_size;
            }
        }
        false
    }
}

impl Iterator for FileDiscoveryIterator {
    type Item = PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(walker) = self.walkers.last_mut() {
            // Try to get the next entry from the current walker
            match walker.next() {
                Some(Ok(entry)) => {
                    let path = entry.path();

                    // Skip non-files
                    if !entry.file_type().is_file() {
                        continue;
                    }

                    // Skip excluded paths
                    if self.is_excluded(path) {
                        continue;
                    }

                    // Skip binary files
                    if self.has_binary_ext(path) || has_binary_extension(path) {
                        continue;
                    }

                    // Skip files exceeding size limit
                    if self.exceeds_size_limit(path) {
                        tracing::debug!(
                            path = %path.display(),
                            "Skipping file exceeding size limit"
                        );
                        continue;
                    }

                    return Some(path.to_path_buf());
                }
                Some(Err(e)) => {
                    tracing::debug!(error = %e, "Error walking directory");
                    continue;
                }
                None => {
                    // Current walker exhausted, move to next
                    self.walkers.pop();
                }
            }
        }

        None
    }
}

/// Convenience function to discover files from paths with exclude patterns.
pub fn discover_files(paths: &[String], exclude_patterns: &[String]) -> FileDiscoveryIterator {
    let config = FileDiscoveryConfig::new(paths.to_vec(), exclude_patterns.to_vec());
    FileDiscoveryIterator::new(&config)
}

/// Convenience function to discover files with full configuration options.
pub fn discover_files_with_config(config: &FileDiscoveryConfig) -> FileDiscoveryIterator {
    FileDiscoveryIterator::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_files(dir: &TempDir) -> Vec<PathBuf> {
        let files = vec![
            dir.path().join("src/main.rs"),
            dir.path().join("src/lib.rs"),
            dir.path().join("tests/test.rs"),
            dir.path().join("node_modules/pkg/index.js"),
            dir.path().join("target/debug/binary"),
            dir.path().join("image.png"),
            dir.path().join("README.md"),
        ];

        for file in &files {
            if let Some(parent) = file.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(file, "test content").unwrap();
        }

        files
    }

    #[test]
    fn test_file_discovery_basic() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(&temp_dir);

        let config = FileDiscoveryConfig::new(
            vec![temp_dir.path().to_string_lossy().to_string()],
            vec!["**/node_modules/**".to_string(), "**/target/**".to_string()],
        );

        let discovered: Vec<PathBuf> = FileDiscoveryIterator::new(&config).collect();

        // Should find: main.rs, lib.rs, test.rs, README.md
        // Should NOT find: node_modules/*, target/*, image.png
        assert_eq!(discovered.len(), 4);

        let names: Vec<_> = discovered
            .iter()
            .filter_map(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .collect();

        assert!(names.contains(&"main.rs".to_string()));
        assert!(names.contains(&"lib.rs".to_string()));
        assert!(names.contains(&"test.rs".to_string()));
        assert!(names.contains(&"README.md".to_string()));
        assert!(!names.contains(&"index.js".to_string())); // excluded by node_modules
        assert!(!names.contains(&"image.png".to_string())); // binary extension
    }

    #[test]
    fn test_file_discovery_skips_binary_extensions() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("code.rs"), "fn main() {}").unwrap();
        fs::write(temp_dir.path().join("image.png"), "binary").unwrap();
        fs::write(temp_dir.path().join("archive.zip"), "binary").unwrap();

        let config =
            FileDiscoveryConfig::new(vec![temp_dir.path().to_string_lossy().to_string()], vec![]);

        let discovered: Vec<PathBuf> = FileDiscoveryIterator::new(&config).collect();

        assert_eq!(discovered.len(), 1);
        assert!(discovered[0].to_string_lossy().contains("code.rs"));
    }

    #[test]
    fn test_file_discovery_nonexistent_path() {
        let config = FileDiscoveryConfig::new(
            vec!["/nonexistent/path/that/does/not/exist".to_string()],
            vec![],
        );

        let discovered: Vec<PathBuf> = FileDiscoveryIterator::new(&config).collect();
        assert!(discovered.is_empty());
    }
}
