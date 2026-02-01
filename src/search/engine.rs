use crate::dependencies::DependencyIndex;
use crate::index::{FileStore, TrigramIndex};
use crate::symbols::{Symbol, SymbolExtractor};
use anyhow::Result;
use rayon::prelude::*;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub file_id: u32,
    pub file_path: String,
    pub line_number: usize,
    pub content: String,
    pub score: f64,
    pub is_symbol: bool,
    pub dependency_count: u32,
}

pub struct SearchEngine {
    pub file_store: FileStore,
    pub trigram_index: TrigramIndex,
    pub dependency_index: DependencyIndex,
    symbol_cache: Vec<Vec<Symbol>>,
    /// Pending imports to resolve after all files are indexed
    pending_imports: Vec<(u32, std::path::PathBuf, Vec<String>)>,
}

impl SearchEngine {
    pub fn new() -> Self {
        Self {
            file_store: FileStore::new(),
            trigram_index: TrigramIndex::new(),
            dependency_index: DependencyIndex::new(),
            symbol_cache: Vec::new(),
            pending_imports: Vec::new(),
        }
    }

    /// Index a file
    pub fn index_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let file_id = self.file_store.add_file(path)?;

        // Register file in dependency index for import resolution
        self.dependency_index.register_file(file_id, path);

        // Get the content
        let content = self
            .file_store
            .get(file_id)
            .and_then(|f| f.as_str().ok())
            .unwrap_or("");

        // Index the content with trigrams
        self.trigram_index.add_document(file_id, content);

        // Extract symbols
        let extractor = SymbolExtractor::new(path);
        let symbols = extractor.extract(content).unwrap_or_default();

        // Extract imports and store for later resolution
        if let Ok(imports) = extractor.extract_imports(content) {
            if !imports.is_empty() {
                let import_paths: Vec<String> = imports.into_iter().map(|i| i.path).collect();
                self.pending_imports
                    .push((file_id, path.to_path_buf(), import_paths));
            }
        }

        // Ensure symbol_cache is large enough
        while self.symbol_cache.len() <= file_id as usize {
            self.symbol_cache.push(Vec::new());
        }
        self.symbol_cache[file_id as usize] = symbols;

        Ok(())
    }

    /// Resolve all pending imports after indexing is complete.
    /// Call this after all files have been indexed to build the dependency graph.
    pub fn resolve_imports(&mut self) {
        let pending = std::mem::take(&mut self.pending_imports);
        for (file_id, file_path, import_paths) in pending {
            for import_path in import_paths {
                self.dependency_index
                    .add_import_from_path(file_id, &file_path, &import_path);
            }
        }
    }

    /// Search for a query using parallel processing
    pub fn search(&self, query: &str, max_results: usize) -> Vec<SearchMatch> {
        // Find candidate documents using trigram index
        let candidate_docs = self.trigram_index.search(query);
        
        // Convert to vector for parallel processing
        let doc_ids: Vec<u32> = candidate_docs.iter().collect();

        // Search in parallel using rayon
        let mut matches: Vec<SearchMatch> = doc_ids
            .par_iter()
            .filter_map(|&doc_id| {
                self.search_in_document(doc_id, query)
            })
            .flatten()
            .collect();

        // Sort by score (descending)
        matches.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        // Return top results
        matches.into_iter().take(max_results).collect()
    }

    fn search_in_document(&self, doc_id: u32, query: &str) -> Option<Vec<SearchMatch>> {
        let file = self.file_store.get(doc_id)?;
        let content = file.as_str().ok()?;
        let path = file.path.to_string_lossy().to_string();

        let mut matches = Vec::new();
        let query_lower = query.to_lowercase();

        // Get symbols for this file
        let symbols = self.symbol_cache.get(doc_id as usize)?;

        // Get dependency count for this file
        let dependency_count = self.dependency_index.get_import_count(doc_id);

        // Search in each line
        for (line_num, line) in content.lines().enumerate() {
            if line.to_lowercase().contains(&query_lower) {
                // Calculate score (includes dependency boost)
                let score = self.calculate_score(
                    &path,
                    line,
                    query,
                    line_num,
                    symbols,
                    doc_id,
                );

                // Check if this is a symbol match
                let is_symbol = symbols.iter().any(|s| {
                    s.line == line_num && s.name.to_lowercase().contains(&query_lower)
                });

                matches.push(SearchMatch {
                    file_id: doc_id,
                    file_path: path.clone(),
                    line_number: line_num + 1, // 1-based line numbers
                    content: line.to_string(),
                    score,
                    is_symbol,
                    dependency_count,
                });
            }
        }

        Some(matches)
    }

    fn calculate_score(
        &self,
        path: &str,
        line: &str,
        query: &str,
        line_num: usize,
        symbols: &[Symbol],
        file_id: u32,
    ) -> f64 {
        let mut score = 1.0;

        // Boost for exact matches
        if line.contains(query) {
            score *= 2.0;
        }

        // Boost for symbol definitions
        if symbols.iter().any(|s| s.line == line_num && s.is_definition) {
            score *= 3.0;
        }

        // Boost for primary source directories
        if path.contains("/src/") || path.contains("/lib/") {
            score *= 1.5;
        }

        // Boost for shorter lines (more relevant)
        let line_len_factor = 1.0 / (1.0 + (line.len() as f64 / 100.0));
        score *= line_len_factor;

        // Boost for query appearing at the start of the line
        if line.trim_start().to_lowercase().starts_with(&query.to_lowercase()) {
            score *= 1.5;
        }

        // Boost for files that are dependencies of many other files
        // Uses log scale to prevent very popular files from dominating
        let import_count = self.dependency_index.get_import_count(file_id);
        if import_count > 0 {
            let dependency_boost = 1.0 + (import_count as f64).log10() * 0.5;
            score *= dependency_boost;
        }

        score
    }

    pub fn get_stats(&self) -> SearchStats {
        SearchStats {
            num_files: self.file_store.len(),
            total_size: self.file_store.total_size(),
            num_trigrams: self.trigram_index.num_trigrams(),
            dependency_edges: self.dependency_index.total_edges(),
        }
    }

    /// Get files that depend on the given file (import it)
    pub fn get_dependents(&self, file_id: u32) -> Vec<u32> {
        self.dependency_index.get_dependents(file_id)
    }

    /// Get files that the given file depends on (imports)
    pub fn get_dependencies(&self, file_id: u32) -> Vec<u32> {
        self.dependency_index.get_dependencies(file_id)
    }

    /// Get file path by ID
    pub fn get_file_path(&self, file_id: u32) -> Option<String> {
        self.file_store.get(file_id).map(|f| f.path.to_string_lossy().to_string())
    }

    /// Find file ID by path
    pub fn find_file_id(&self, path: &str) -> Option<u32> {
        for id in 0..self.file_store.len() as u32 {
            if let Some(file) = self.file_store.get(id) {
                let file_path = file.path.to_string_lossy();
                if file_path.contains(path) || path.contains(&*file_path) {
                    return Some(id);
                }
            }
        }
        None
    }
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SearchStats {
    pub num_files: usize,
    pub total_size: u64,
    pub num_trigrams: usize,
    pub dependency_edges: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_search_engine() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "hello world").unwrap();
        writeln!(file, "hello rust").unwrap();
        writeln!(file, "goodbye world").unwrap();
        drop(file);

        let mut engine = SearchEngine::new();
        engine.index_file(&file_path).unwrap();

        let results = engine.search("hello", 10);
        assert_eq!(results.len(), 2);

        let results = engine.search("world", 10);
        assert_eq!(results.len(), 2);
    }
}
