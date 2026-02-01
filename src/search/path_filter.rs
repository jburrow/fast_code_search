//! Path filtering using glob patterns for include/exclude file matching.
//!
//! This module provides efficient path filtering that is applied after trigram
//! pre-filtering to further narrow down search candidates based on file paths.

use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use roaring::RoaringBitmap;
use std::path::PathBuf;

/// Filters files by path patterns using glob matching.
///
/// Supports both include and exclude patterns:
/// - Include patterns: Files must match at least one pattern (if any specified)
/// - Exclude patterns: Files must not match any pattern
///
/// Patterns are matched against relative paths from the indexed root.
#[derive(Debug, Default)]
pub struct PathFilter {
    /// Include patterns - file must match at least one (if non-empty)
    include: Option<GlobSet>,
    /// Exclude patterns - file must not match any
    exclude: Option<GlobSet>,
}

impl PathFilter {
    /// Create a new path filter from include and exclude patterns.
    ///
    /// Patterns should be glob patterns like:
    /// - `src/**/*.rs` - All Rust files under src/
    /// - `**/test/**` - Any path containing test/
    /// - `*.{js,ts}` - Files with .js or .ts extension
    ///
    /// Empty pattern lists are treated as "no filter" (match all/exclude none).
    pub fn new(include_patterns: &[String], exclude_patterns: &[String]) -> Result<Self> {
        let include = if include_patterns.is_empty() {
            None
        } else {
            let mut builder = GlobSetBuilder::new();
            for pattern in include_patterns {
                let glob = Glob::new(pattern)
                    .with_context(|| format!("Invalid include glob pattern: {}", pattern))?;
                builder.add(glob);
            }
            Some(builder.build().context("Failed to build include GlobSet")?)
        };

        let exclude = if exclude_patterns.is_empty() {
            None
        } else {
            let mut builder = GlobSetBuilder::new();
            for pattern in exclude_patterns {
                let glob = Glob::new(pattern)
                    .with_context(|| format!("Invalid exclude glob pattern: {}", pattern))?;
                builder.add(glob);
            }
            Some(builder.build().context("Failed to build exclude GlobSet")?)
        };

        Ok(Self { include, exclude })
    }

    /// Parse semicolon-delimited patterns into a vector.
    ///
    /// Example: "src/**/*.rs;lib/**" -> ["src/**/*.rs", "lib/**"]
    pub fn parse_patterns(patterns: &str) -> Vec<String> {
        if patterns.trim().is_empty() {
            return Vec::new();
        }
        patterns
            .split(';')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Create a path filter from semicolon-delimited pattern strings.
    pub fn from_delimited(include: &str, exclude: &str) -> Result<Self> {
        let include_patterns = Self::parse_patterns(include);
        let exclude_patterns = Self::parse_patterns(exclude);
        Self::new(&include_patterns, &exclude_patterns)
    }

    /// Check if a path matches the filter criteria.
    ///
    /// Returns true if:
    /// - The path matches at least one include pattern (or no include patterns specified)
    /// - AND the path does not match any exclude pattern
    pub fn matches(&self, path: &str) -> bool {
        // Check include patterns
        let included = match &self.include {
            Some(set) => set.is_match(path),
            None => true, // No include patterns = include all
        };

        if !included {
            return false;
        }

        // Check exclude patterns
        let excluded = match &self.exclude {
            Some(set) => set.is_match(path),
            None => false, // No exclude patterns = exclude none
        };

        !excluded
    }

    /// Check if this filter has any patterns (include or exclude).
    pub fn is_empty(&self) -> bool {
        self.include.is_none() && self.exclude.is_none()
    }

    /// Filter a set of document IDs based on their paths using a path lookup function.
    ///
    /// This is optimized for use after trigram pre-filtering:
    /// - Takes a bitmap of candidate document IDs
    /// - Uses a callback to look up paths (avoids cloning entire path array)
    /// - Returns a new bitmap with only matching documents
    pub fn filter_documents_with<'a, F>(&self, candidates: &RoaringBitmap, get_path: F) -> RoaringBitmap
    where
        F: Fn(u32) -> Option<&'a std::path::Path>,
    {
        // If no filters, return candidates unchanged
        if self.is_empty() {
            return candidates.clone();
        }

        let mut result = RoaringBitmap::new();
        for doc_id in candidates.iter() {
            if let Some(path) = get_path(doc_id) {
                // Convert to string for matching
                let path_str = path.to_string_lossy();
                if self.matches(&path_str) {
                    result.insert(doc_id);
                }
            }
        }
        result
    }

    /// Filter a set of document IDs based on their paths.
    ///
    /// This is optimized for use after trigram pre-filtering:
    /// - Takes a bitmap of candidate document IDs
    /// - Looks up each path and applies glob matching
    /// - Returns a new bitmap with only matching documents
    /// 
    /// Note: Prefer `filter_documents_with()` for better performance as it
    /// avoids cloning the entire path array.
    pub fn filter_documents(
        &self,
        candidates: &RoaringBitmap,
        id_to_path: &[PathBuf],
    ) -> RoaringBitmap {
        // If no filters, return candidates unchanged
        if self.is_empty() {
            return candidates.clone();
        }

        let mut result = RoaringBitmap::new();
        for doc_id in candidates.iter() {
            if let Some(path) = id_to_path.get(doc_id as usize) {
                // Convert to string for matching
                let path_str = path.to_string_lossy();
                if self.matches(&path_str) {
                    result.insert(doc_id);
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_patterns() {
        assert_eq!(PathFilter::parse_patterns(""), Vec::<String>::new());
        assert_eq!(PathFilter::parse_patterns("  "), Vec::<String>::new());
        assert_eq!(
            PathFilter::parse_patterns("src/**/*.rs"),
            vec!["src/**/*.rs"]
        );
        assert_eq!(
            PathFilter::parse_patterns("src/**/*.rs;lib/**"),
            vec!["src/**/*.rs", "lib/**"]
        );
        assert_eq!(
            PathFilter::parse_patterns("src/**/*.rs ; lib/** ; "),
            vec!["src/**/*.rs", "lib/**"]
        );
    }

    #[test]
    fn test_empty_filter() {
        let filter = PathFilter::new(&[], &[]).unwrap();
        assert!(filter.is_empty());
        assert!(filter.matches("any/path/file.rs"));
        assert!(filter.matches("test/something.py"));
    }

    #[test]
    fn test_include_only() {
        let filter = PathFilter::new(&["src/**/*.rs".to_string()], &[]).unwrap();
        assert!(filter.matches("src/main.rs"));
        assert!(filter.matches("src/lib/utils.rs"));
        assert!(!filter.matches("test/main.rs"));
        assert!(!filter.matches("src/main.py"));
    }

    #[test]
    fn test_exclude_only() {
        let filter = PathFilter::new(&[], &["**/test/**".to_string()]).unwrap();
        assert!(filter.matches("src/main.rs"));
        assert!(!filter.matches("src/test/main.rs"));
        assert!(!filter.matches("test/anything.rs"));
    }

    #[test]
    fn test_include_and_exclude() {
        let filter =
            PathFilter::new(&["src/**/*.rs".to_string()], &["**/test/**".to_string()]).unwrap();
        assert!(filter.matches("src/main.rs"));
        assert!(filter.matches("src/lib/utils.rs"));
        assert!(!filter.matches("src/test/main.rs"));
        assert!(!filter.matches("lib/main.rs")); // Not in src/
    }

    #[test]
    fn test_multiple_patterns() {
        let filter = PathFilter::new(
            &["src/**/*.rs".to_string(), "lib/**/*.rs".to_string()],
            &["**/test/**".to_string(), "**/vendor/**".to_string()],
        )
        .unwrap();
        assert!(filter.matches("src/main.rs"));
        assert!(filter.matches("lib/utils.rs"));
        assert!(!filter.matches("src/test/main.rs"));
        assert!(!filter.matches("src/vendor/dep.rs"));
    }

    #[test]
    fn test_from_delimited() {
        let filter = PathFilter::from_delimited("src/**/*.rs;lib/**", "**/test/**").unwrap();
        assert!(filter.matches("src/main.rs"));
        assert!(filter.matches("lib/anything.txt"));
        assert!(!filter.matches("src/test/main.rs"));
    }

    #[test]
    fn test_filter_documents() {
        let filter = PathFilter::new(&["src/**/*.rs".to_string()], &[]).unwrap();

        let paths = vec![
            PathBuf::from("src/main.rs"),      // 0: matches
            PathBuf::from("test/main.rs"),     // 1: no match
            PathBuf::from("src/lib/utils.rs"), // 2: matches
            PathBuf::from("README.md"),        // 3: no match
        ];

        let mut candidates = RoaringBitmap::new();
        candidates.insert(0);
        candidates.insert(1);
        candidates.insert(2);
        candidates.insert(3);

        let filtered = filter.filter_documents(&candidates, &paths);
        assert!(filtered.contains(0));
        assert!(!filtered.contains(1));
        assert!(filtered.contains(2));
        assert!(!filtered.contains(3));
        assert_eq!(filtered.len(), 2);
    }
}
