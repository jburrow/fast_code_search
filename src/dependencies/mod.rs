//! Dependency tracking module for Fast Code Search
//!
//! Tracks import relationships between files to enable dependency-based
//! ranking of search results. Files that are imported by many other files
//! receive a ranking boost.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Tracks import/dependency relationships between files in the index.
///
/// Maintains bidirectional mappings:
/// - `imports`: file_id -> set of file_ids it imports
/// - `imported_by`: file_id -> set of file_ids that import it
#[derive(Debug, Default)]
pub struct DependencyIndex {
    /// Map from file_id to the set of file_ids it imports
    imports: HashMap<u32, HashSet<u32>>,
    /// Reverse index: file_id -> files that import it
    imported_by: HashMap<u32, HashSet<u32>>,
    /// Cached import counts for fast scoring lookups
    import_counts: HashMap<u32, u32>,
    /// Map from normalized path to file_id for import resolution
    path_to_id: HashMap<PathBuf, u32>,
    /// Inverted index: filename -> list of full paths (for fast non-relative import lookup)
    filename_to_paths: HashMap<String, Vec<PathBuf>>,
}

impl DependencyIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a file path with its ID for import resolution
    pub fn register_file(&mut self, file_id: u32, path: &Path) {
        // Store normalized path for matching
        let stored_path = if let Ok(canonical) = path.canonicalize() {
            canonical
        } else {
            // Fallback to the path as-is if canonicalization fails
            path.to_path_buf()
        };
        
        // Add to filename inverted index for fast non-relative lookups
        if let Some(filename) = stored_path.file_name().and_then(|s| s.to_str()) {
            self.filename_to_paths
                .entry(filename.to_string())
                .or_default()
                .push(stored_path.clone());
        }
        
        self.path_to_id.insert(stored_path, file_id);
    }

    /// Add an import relationship: `from_file` imports `to_file`
    pub fn add_import(&mut self, from_file: u32, to_file: u32) {
        // Add forward edge
        self.imports
            .entry(from_file)
            .or_default()
            .insert(to_file);

        // Add reverse edge
        self.imported_by
            .entry(to_file)
            .or_default()
            .insert(from_file);

        // Update cached count
        let count = self.imported_by.get(&to_file).map(|s| s.len()).unwrap_or(0);
        self.import_counts.insert(to_file, count as u32);
    }

    /// Add import from raw import path string, resolving it relative to the source file
    pub fn add_import_from_path(
        &mut self,
        from_file_id: u32,
        from_file_path: &Path,
        import_path: &str,
    ) -> Option<u32> {
        let resolved = self.resolve_import_path(from_file_path, import_path)?;
        let to_file_id = self.path_to_id.get(&resolved).copied()?;
        self.add_import(from_file_id, to_file_id);
        Some(to_file_id)
    }

    /// Resolve an import path relative to the importing file.
    /// This method is thread-safe and only requires &self.
    pub fn resolve_import_path(&self, from_file: &Path, import_path: &str) -> Option<PathBuf> {
        let parent = from_file.parent()?;

        // Handle relative imports
        if import_path.starts_with('.') {
            let resolved = parent.join(import_path);
            // Try with common extensions
            for ext in &["", ".rs", ".py", ".js", ".ts", ".jsx", ".tsx"] {
                let with_ext = if ext.is_empty() {
                    resolved.clone()
                } else {
                    resolved.with_extension(&ext[1..])
                };
                if let Ok(canonical) = with_ext.canonicalize() {
                    if self.path_to_id.contains_key(&canonical) {
                        return Some(canonical);
                    }
                }
            }
        }

        // For non-relative imports, use the filename inverted index (O(1) lookup)
        // instead of scanning all paths (O(n))
        let import_filename = Path::new(import_path)
            .file_name()
            .and_then(|s| s.to_str())?;

        // Try exact filename match first
        if let Some(paths) = self.filename_to_paths.get(import_filename) {
            if let Some(path) = paths.first() {
                return Some(path.clone());
            }
        }

        // Try with common extensions appended
        for ext in &[".rs", ".py", ".js", ".ts", ".jsx", ".tsx"] {
            let with_ext = format!("{}{}", import_filename, ext);
            if let Some(paths) = self.filename_to_paths.get(&with_ext) {
                if let Some(path) = paths.first() {
                    return Some(path.clone());
                }
            }
        }

        None
    }

    /// Get file ID for a resolved path. Thread-safe.
    pub fn get_file_id(&self, path: &Path) -> Option<u32> {
        self.path_to_id.get(path).copied()
    }

    /// Batch insert multiple import edges. More efficient than repeated add_import calls.
    pub fn add_imports_batch(&mut self, edges: Vec<(u32, u32)>) {
        for (from_file, to_file) in edges {
            self.imports
                .entry(from_file)
                .or_default()
                .insert(to_file);
            self.imported_by
                .entry(to_file)
                .or_default()
                .insert(from_file);
        }
        
        // Update cached counts in bulk
        for (&file_id, dependents) in &self.imported_by {
            self.import_counts.insert(file_id, dependents.len() as u32);
        }
    }

    /// Get the number of files that import the given file
    pub fn get_import_count(&self, file_id: u32) -> u32 {
        self.import_counts.get(&file_id).copied().unwrap_or(0)
    }

    /// Get all files that import the given file (dependents)
    pub fn get_dependents(&self, file_id: u32) -> Vec<u32> {
        self.imported_by
            .get(&file_id)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Get all files that the given file imports (dependencies)
    pub fn get_dependencies(&self, file_id: u32) -> Vec<u32> {
        self.imports
            .get(&file_id)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Get total number of dependency edges in the graph
    pub fn total_edges(&self) -> usize {
        self.imports.values().map(|s| s.len()).sum()
    }

    /// Get total number of files with at least one dependent
    pub fn files_with_dependents(&self) -> usize {
        self.imported_by.len()
    }

    /// Clear all dependency information
    pub fn clear(&mut self) {
        self.imports.clear();
        self.imported_by.clear();
        self.import_counts.clear();
        self.path_to_id.clear();
        self.filename_to_paths.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_import() {
        let mut index = DependencyIndex::new();
        index.add_import(1, 2);
        index.add_import(3, 2);
        index.add_import(4, 2);

        assert_eq!(index.get_import_count(2), 3);
        assert_eq!(index.get_dependents(2).len(), 3);
        assert_eq!(index.get_dependencies(1), vec![2]);
    }

    #[test]
    fn test_bidirectional() {
        let mut index = DependencyIndex::new();
        index.add_import(1, 2);

        assert!(index.get_dependencies(1).contains(&2));
        assert!(index.get_dependents(2).contains(&1));
    }
}
