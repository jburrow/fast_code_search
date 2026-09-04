//! Dependency tracking module for Fast Code Search
//!
//! Tracks import relationships between files to enable dependency-based
//! ranking of search results. Files that are imported by many other files
//! receive a ranking boost.

use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Tracks import/dependency relationships between files in the index.
///
/// Maintains bidirectional mappings:
/// - `imports`: file_id -> set of file_ids it imports
/// - `imported_by`: file_id -> set of file_ids that import it
#[derive(Debug, Default)]
pub struct DependencyIndex {
    /// Map from file_id to the set of file_ids it imports
    imports: FxHashMap<u32, FxHashSet<u32>>,
    /// Reverse index: file_id -> files that import it
    imported_by: FxHashMap<u32, FxHashSet<u32>>,
    /// Cached import counts for fast scoring lookups
    import_counts: FxHashMap<u32, u32>,
    /// Map from normalized path to file_id for import resolution
    path_to_id: HashMap<PathBuf, u32>,
    /// Inverted index: filename -> list of full paths (for fast non-relative import lookup)
    filename_to_paths: HashMap<String, Vec<PathBuf>>,
    /// Reverse of `path_to_id`, so removal and re-registration are O(1)
    /// instead of a full scan (and so `filename_to_paths` can be pruned).
    id_to_path: FxHashMap<u32, PathBuf>,
}

impl DependencyIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a file path with its ID for import resolution.
    ///
    /// Re-registering an id (the watcher's update path) is idempotent: the
    /// previous path mapping for that id is dropped first, so repeated edits
    /// of one file never accumulate duplicate `filename_to_paths` entries.
    pub fn register_file(&mut self, file_id: u32, path: &Path) {
        // Store normalized path for matching
        let stored_path = if let Ok(canonical) = path.canonicalize() {
            canonical
        } else {
            // Fallback to the path as-is if canonicalization fails
            path.to_path_buf()
        };

        if let Some(previous) = self.id_to_path.get(&file_id) {
            if *previous == stored_path {
                return; // already registered under exactly this path
            }
            self.unregister_path_lookups(file_id);
        }

        // Add to filename inverted index for fast non-relative lookups
        if let Some(filename) = stored_path.file_name().and_then(|s| s.to_str()) {
            self.filename_to_paths
                .entry(filename.to_string())
                .or_default()
                .push(stored_path.clone());
        }

        self.path_to_id.insert(stored_path.clone(), file_id);
        self.id_to_path.insert(file_id, stored_path);
    }

    /// Drop `file_id` from `path_to_id`, `id_to_path` and `filename_to_paths`.
    fn unregister_path_lookups(&mut self, file_id: u32) {
        let Some(path) = self.id_to_path.remove(&file_id) else {
            return;
        };
        self.path_to_id.remove(&path);
        if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
            if let Some(paths) = self.filename_to_paths.get_mut(filename) {
                paths.retain(|p| *p != path);
                if paths.is_empty() {
                    self.filename_to_paths.remove(filename);
                }
            }
        }
    }

    /// Add an import relationship: `from_file` imports `to_file`
    pub fn add_import(&mut self, from_file: u32, to_file: u32) {
        // Add forward edge
        self.imports.entry(from_file).or_default().insert(to_file);

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
        // Track only the to_file IDs touched by this batch so we update
        // import_counts in O(batch) rather than O(N_total).
        let mut touched_to_files = FxHashSet::default();
        for (from_file, to_file) in edges {
            self.imports.entry(from_file).or_default().insert(to_file);
            self.imported_by
                .entry(to_file)
                .or_default()
                .insert(from_file);
            touched_to_files.insert(to_file);
        }

        // Update cached counts only for files touched by this batch
        for to_file in touched_to_files {
            let count = self.imported_by.get(&to_file).map(|s| s.len()).unwrap_or(0);
            self.import_counts.insert(to_file, count as u32);
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

    /// Get all import edges as (from_file_id, to_file_id) pairs
    pub fn get_all_edges(&self) -> Vec<(u32, u32)> {
        self.imports
            .iter()
            .flat_map(|(&from, targets)| targets.iter().map(move |&to| (from, to)))
            .collect()
    }

    /// Remove a file and all edges referencing it (for incremental updates).
    ///
    /// Drops the file from both the forward (`imports`) and reverse
    /// (`imported_by`) graphs, updates cached `import_counts` for any file whose
    /// dependent set changed, and removes it from the path lookups. Safe to call
    /// for an id that isn't present (no-op for the graph parts).
    pub fn remove_file(&mut self, file_id: u32) {
        // Forward edges out of `file_id`: remove the reverse entry on each target.
        if let Some(targets) = self.imports.remove(&file_id) {
            for to_file in targets {
                if let Some(set) = self.imported_by.get_mut(&to_file) {
                    set.remove(&file_id);
                    let count = set.len() as u32;
                    if count == 0 {
                        self.imported_by.remove(&to_file);
                        self.import_counts.remove(&to_file);
                    } else {
                        self.import_counts.insert(to_file, count);
                    }
                }
            }
        }

        // Reverse edges into `file_id`: remove the forward entry on each source.
        if let Some(sources) = self.imported_by.remove(&file_id) {
            for from_file in sources {
                if let Some(set) = self.imports.get_mut(&from_file) {
                    set.remove(&file_id);
                    if set.is_empty() {
                        self.imports.remove(&from_file);
                    }
                }
            }
        }
        self.import_counts.remove(&file_id);

        // Remove from path lookups so the id is no longer resolvable (O(1)
        // via id_to_path; also prunes the filename index so a removed path
        // can never be picked as a bare-name resolution candidate).
        self.unregister_path_lookups(file_id);
    }

    /// Clear all dependency information
    pub fn clear(&mut self) {
        self.imports.clear();
        self.imported_by.clear();
        self.import_counts.clear();
        self.path_to_id.clear();
        self.filename_to_paths.clear();
        self.id_to_path.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Roadmap 1.11: re-registering an id (watcher update path) must not
    /// accumulate duplicate filename entries, and removal must prune every
    /// lookup so the path cannot resolve afterwards.
    #[test]
    fn test_reregister_and_remove_keep_lookups_bounded() {
        let temp = tempfile::TempDir::new().unwrap();
        let a = temp.path().join("util.py");
        let b = temp.path().join("main.py");
        std::fs::write(&a, "x = 1\n").unwrap();
        std::fs::write(&b, "import util\n").unwrap();

        let mut idx = DependencyIndex::new();
        idx.register_file(1, &b);
        for _ in 0..1000 {
            idx.register_file(0, &a);
        }
        assert_eq!(idx.filename_to_paths["util.py"].len(), 1);
        assert_eq!(idx.path_to_id.len(), 2);

        // Resolvable while present ...
        assert_eq!(idx.add_import_from_path(1, &b, "util"), Some(0));
        assert_eq!(idx.get_import_count(0), 1);

        // ... and fully gone after removal.
        idx.remove_file(0);
        assert!(!idx.filename_to_paths.contains_key("util.py"));
        assert_eq!(idx.path_to_id.len(), 1);
        assert_eq!(idx.get_import_count(0), 0);
        assert_eq!(idx.add_import_from_path(1, &b, "util"), None);
    }

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
