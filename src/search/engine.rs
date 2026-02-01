use crate::dependencies::DependencyIndex;
use crate::index::{FileStore, TrigramIndex, extract_trigrams, Trigram};
use crate::symbols::{Symbol, SymbolExtractor};
use anyhow::Result;
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

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

/// Pre-processed file data ready to be merged into the engine.
/// This is computed in parallel and then merged with a lock.
pub struct PreIndexedFile {
    /// Path to the file
    pub path: PathBuf,
    /// File content (owned for thread safety)
    pub content: String,
    /// Unique trigrams extracted from the content
    pub trigrams: HashSet<Trigram>,
    /// Extracted symbols
    pub symbols: Vec<Symbol>,
    /// Extracted import paths
    pub imports: Vec<String>,
}

impl PreIndexedFile {
    /// Maximum file size to process (10MB) - larger files are skipped to avoid memory issues
    const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

    /// Process a file in parallel - does all CPU-heavy work without needing engine access
    /// Uses catch_unwind to handle tree-sitter stack overflows gracefully
    pub fn process(path: &Path) -> Option<Self> {
        // Check file size first - skip very large files
        let metadata = std::fs::metadata(path).ok()?;
        if metadata.len() > Self::MAX_FILE_SIZE {
            return None;
        }

        // Read file content
        let content = std::fs::read_to_string(path).ok()?;
        
        // Extract trigrams (safe, no recursion)
        let trigram_vec = extract_trigrams(&content);
        let trigrams: HashSet<Trigram> = trigram_vec.into_iter().collect();
        
        // Extract symbols with panic protection (tree-sitter can stack overflow on complex files)
        let extractor = SymbolExtractor::new(path);
        let symbols = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            extractor.extract(&content).unwrap_or_default()
        })).unwrap_or_default();
        
        // Extract imports with panic protection
        let imports = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            extractor
                .extract_imports(&content)
                .ok()
                .map(|imports| imports.into_iter().map(|i| i.path).collect())
                .unwrap_or_default()
        })).unwrap_or_default();
        
        Some(PreIndexedFile {
            path: path.to_path_buf(),
            content,
            trigrams,
            symbols,
            imports,
        })
    }
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

    /// Index a batch of pre-processed files.
    /// This is the merge step after parallel processing - only this needs the write lock.
    /// Returns the number of files successfully indexed.
    pub fn index_batch(&mut self, batch: Vec<PreIndexedFile>) -> usize {
        let mut count = 0;
        
        for pre_indexed in batch {
            // Add file to store - this also memory-maps it
            let file_id = match self.file_store.add_file(&pre_indexed.path) {
                Ok(id) => id,
                Err(_) => continue,
            };
            
            // Register file in dependency index
            self.dependency_index.register_file(file_id, &pre_indexed.path);
            
            // Add trigrams to index (using pre-computed trigrams)
            self.trigram_index.add_document_trigrams(file_id, pre_indexed.trigrams);
            
            // Store symbols
            while self.symbol_cache.len() <= file_id as usize {
                self.symbol_cache.push(Vec::new());
            }
            self.symbol_cache[file_id as usize] = pre_indexed.symbols;
            
            // Store imports for later resolution
            if !pre_indexed.imports.is_empty() {
                self.pending_imports.push((
                    file_id,
                    pre_indexed.path,
                    pre_indexed.imports,
                ));
            }
            
            count += 1;
        }
        
        count
    }

    /// Resolve all pending imports after indexing is complete.
    /// Call this after all files have been indexed to build the dependency graph.
    /// 
    /// Uses two-phase parallel resolution:
    /// 1. Parallel path resolution using rayon (CPU-bound, thread-safe)
    /// 2. Sequential graph insertion (requires &mut self)
    pub fn resolve_imports(&mut self) {
        let pending = std::mem::take(&mut self.pending_imports);
        
        // Phase 1: Parallel path resolution - collect (from_id, to_id) pairs
        // Uses &self on dependency_index (thread-safe read-only methods)
        let edges: Vec<(u32, u32)> = pending
            .par_iter()
            .flat_map(|(file_id, file_path, import_paths)| {
                import_paths
                    .par_iter()
                    .filter_map(|import_path| {
                        let resolved = self.dependency_index.resolve_import_path(file_path, import_path)?;
                        let to_id = self.dependency_index.get_file_id(&resolved)?;
                        Some((*file_id, to_id))
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        // Phase 2: Sequential batch insert (requires &mut self)
        self.dependency_index.add_imports_batch(edges);
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

/// Status of the indexing process
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexingStatus {
    /// No indexing is in progress
    Idle,
    /// Discovering files to index
    Discovering,
    /// Actively indexing files
    Indexing,
    /// Resolving import dependencies
    ResolvingImports,
    /// Indexing completed successfully
    Completed,
}

impl Default for IndexingStatus {
    fn default() -> Self {
        Self::Idle
    }
}

/// Progress information for the indexing process
#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexingProgress {
    /// Current status of the indexing process
    pub status: IndexingStatus,
    /// Number of files discovered during file discovery phase
    pub files_discovered: usize,
    /// Number of files indexed so far
    pub files_indexed: usize,
    /// Current batch number (1-based)
    pub current_batch: usize,
    /// Total number of batches to process
    pub total_batches: usize,
    /// Current path being processed (for display)
    pub current_path: Option<String>,
    /// Timestamp when indexing started (Unix epoch millis)
    pub started_at: Option<u64>,
    /// Number of errors encountered
    pub errors: usize,
    /// Message describing current activity
    pub message: String,
}

impl Default for IndexingProgress {
    fn default() -> Self {
        Self {
            status: IndexingStatus::Idle,
            files_discovered: 0,
            files_indexed: 0,
            current_batch: 0,
            total_batches: 0,
            current_path: None,
            started_at: None,
            errors: 0,
            message: String::from("Ready"),
        }
    }
}

impl IndexingProgress {
    /// Create a new progress tracker starting the indexing process
    pub fn start() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Self {
            status: IndexingStatus::Discovering,
            started_at: Some(now),
            message: String::from("Starting file discovery..."),
            ..Default::default()
        }
    }

    /// Calculate elapsed time in seconds
    pub fn elapsed_secs(&self) -> Option<f64> {
        let started = self.started_at?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Some((now - started) as f64 / 1000.0)
    }

    /// Calculate progress percentage (0-100)
    pub fn progress_percent(&self) -> u8 {
        match self.status {
            IndexingStatus::Idle => 0,
            IndexingStatus::Discovering => 5,
            IndexingStatus::Indexing => {
                if self.total_batches == 0 {
                    10
                } else {
                    let batch_progress = (self.current_batch as f64 / self.total_batches as f64) * 85.0;
                    (10.0 + batch_progress).min(95.0) as u8
                }
            }
            IndexingStatus::ResolvingImports => 96,
            IndexingStatus::Completed => 100,
        }
    }
}

/// Shared indexing progress state for use across threads
pub type SharedIndexingProgress = std::sync::Arc<std::sync::RwLock<IndexingProgress>>;

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
