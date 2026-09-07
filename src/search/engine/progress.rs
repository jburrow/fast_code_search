//! Split out of the search engine module; see `engine/mod.rs`.

use super::*;

pub struct SearchStats {
    pub num_files: usize,
    pub total_size: u64,
    pub num_trigrams: usize,
    pub dependency_edges: usize,
    /// Total bytes of text content indexed
    pub total_content_bytes: u64,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct RebuildCacheStats {
    pub files_processed: usize,
    pub files_skipped: usize,
    pub symbols_extracted: usize,
    pub imports_extracted: usize,
}

#[derive(Debug)]
pub(super) struct RebuildEntry {
    pub(super) file_id: u32,
    pub(super) path: std::path::PathBuf,
    pub(super) symbols: Vec<Symbol>,
    pub(super) imports: Vec<String>,
    pub(super) references: Vec<crate::symbols::extractor::SymbolRef>,
    pub(super) had_content: bool,
}

/// Result of loading a persisted index with reconciliation
#[derive(Debug, Clone)]
pub struct LoadIndexResult {
    /// Files that were modified since indexing (need re-indexing)
    pub stale_files: Vec<std::path::PathBuf>,
    /// Files that no longer exist (removed from index)
    pub removed_files: Vec<std::path::PathBuf>,
    /// Paths that are new in config (need full indexing)
    pub new_paths: Vec<String>,
    /// Paths that were removed from config (files removed from index)
    pub removed_paths: Vec<String>,
    /// Whether the config fingerprint matches (false = config changed)
    pub config_compatible: bool,
    /// Files already validly indexed (unchanged since last index build).
    /// Used to skip re-indexing when scanning for unindexed files after a
    /// partial/checkpoint load.
    pub already_indexed_files: Vec<std::path::PathBuf>,
    /// Symbols and references were re-extracted from file content rather
    /// than restored (older extractor, or none saved): the in-memory index
    /// now differs from the file and should be saved even if no file
    /// changed, or the next start pays the extraction again.
    pub symbols_reextracted: bool,
}

/// Status of the indexing process
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IndexingStatus {
    /// No indexing is in progress
    #[default]
    Idle,
    /// Loading persisted index from disk
    LoadingIndex,
    /// Discovering files to index
    Discovering,
    /// Actively indexing files
    Indexing,
    /// Reconciling persisted index with current filesystem
    Reconciling,
    /// Resolving import dependencies
    ResolvingImports,
    /// Indexing completed successfully
    Completed,
}

/// Sub-phases during index loading for detailed progress reporting
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LoadingPhase {
    /// Not currently loading
    #[default]
    None,
    /// Reading index file from disk
    ReadingFile,
    /// Deserializing the persisted index data
    Deserializing,
    /// Checking files for staleness
    CheckingFiles,
    /// Restoring trigram index
    RestoringTrigrams,
    /// Memory-mapping files
    MappingFiles,
    /// Rebuilding symbol and dependency caches
    RebuildingSymbols,
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
    /// Number of files transcoded from non-UTF-8 encodings
    pub files_transcoded: usize,
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
    /// Sub-phase during index loading (when status == LoadingIndex)
    #[serde(skip_serializing_if = "is_loading_phase_none")]
    pub loading_phase: LoadingPhase,
    /// Total files in persisted index (for loading progress)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loading_total_files: Option<usize>,
    /// Files processed during loading
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loading_files_processed: Option<usize>,
    /// Number of files loaded from a persisted cache (baseline for reconciliation progress)
    #[serde(skip_serializing_if = "is_usize_zero")]
    pub files_loaded_from_cache: usize,
}

/// Helper for serde skip_serializing_if
fn is_loading_phase_none(phase: &LoadingPhase) -> bool {
    *phase == LoadingPhase::None
}

/// Helper for serde skip_serializing_if
fn is_usize_zero(v: &usize) -> bool {
    *v == 0
}

impl Default for IndexingProgress {
    fn default() -> Self {
        Self {
            status: IndexingStatus::Idle,
            files_discovered: 0,
            files_indexed: 0,
            files_transcoded: 0,
            current_batch: 0,
            total_batches: 0,
            current_path: None,
            started_at: None,
            errors: 0,
            message: String::from("Ready"),
            loading_phase: LoadingPhase::None,
            loading_total_files: None,
            loading_files_processed: None,
            files_loaded_from_cache: 0,
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
        // saturating_sub guards against wall-clock going backwards (NTP step,
        // VM resume), which would otherwise underflow-panic in debug or yield a
        // ~584-million-year elapsed value in release.
        Some(now.saturating_sub(started) as f64 / 1000.0)
    }

    /// Calculate progress percentage (0-100)
    pub fn progress_percent(&self) -> u8 {
        match self.status {
            IndexingStatus::Idle => 0,
            IndexingStatus::LoadingIndex => {
                // Show more granular progress during loading based on phase
                match self.loading_phase {
                    LoadingPhase::None => 1,
                    LoadingPhase::ReadingFile => 2,
                    LoadingPhase::Deserializing => 5,
                    LoadingPhase::CheckingFiles => {
                        // 10-30% for file checking
                        if let (Some(total), Some(processed)) =
                            (self.loading_total_files, self.loading_files_processed)
                        {
                            if total > 0 {
                                let pct = (processed as f64 / total as f64) * 20.0;
                                return (10.0 + pct).min(30.0) as u8;
                            }
                        }
                        15
                    }
                    LoadingPhase::RestoringTrigrams => 40,
                    LoadingPhase::MappingFiles => {
                        // 50-90% for file mapping
                        if let (Some(total), Some(processed)) =
                            (self.loading_total_files, self.loading_files_processed)
                        {
                            if total > 0 {
                                let pct = (processed as f64 / total as f64) * 40.0;
                                return (50.0 + pct).min(90.0) as u8;
                            }
                        }
                        60
                    }
                    LoadingPhase::RebuildingSymbols => {
                        if let (Some(total), Some(processed)) =
                            (self.loading_total_files, self.loading_files_processed)
                        {
                            if total > 0 {
                                let pct = (processed as f64 / total as f64) * 15.0;
                                return (80.0 + pct).min(95.0) as u8;
                            }
                        }
                        85
                    }
                }
            }
            IndexingStatus::Discovering => 5,
            IndexingStatus::Indexing => {
                if self.total_batches == 0 {
                    10
                } else {
                    let batch_progress =
                        (self.current_batch as f64 / self.total_batches as f64) * 80.0;
                    (10.0 + batch_progress).min(90.0) as u8
                }
            }
            IndexingStatus::Reconciling => {
                // When reconciling after a cache load, compute progress based on
                // newly-indexed files vs newly-discovered files so the bar actually
                // moves rather than sitting at a fixed 92%.
                let new_discovered = self
                    .files_discovered
                    .saturating_sub(self.files_loaded_from_cache);
                if new_discovered == 0 {
                    // No new files yet (discovery still starting or nothing to do)
                    92
                } else {
                    let new_indexed = self
                        .files_indexed
                        .saturating_sub(self.files_loaded_from_cache);
                    let pct = new_indexed as f64 / new_discovered as f64;
                    // Map [0, 1] → [92, 99] so we leave room for ResolvingImports
                    (92.0 + pct * 7.0).min(99.0) as u8
                }
            }
            IndexingStatus::ResolvingImports => 96,
            IndexingStatus::Completed => 100,
        }
    }
}

/// Shared indexing progress state for use across threads
pub type SharedIndexingProgress = std::sync::Arc<std::sync::RwLock<IndexingProgress>>;

/// Broadcast channel for sending progress updates to WebSocket clients
/// Use sender.subscribe() to get a receiver for each WebSocket connection
pub type ProgressBroadcaster = tokio::sync::broadcast::Sender<IndexingProgress>;

/// Create a new progress broadcaster with reasonable capacity
/// Returns both the sender (for publishing updates) and receiver (for subscribing)
pub fn create_progress_broadcaster() -> ProgressBroadcaster {
    // Buffer 64 messages: a large batch run broadcasts often and every
    // connected tab consumes at its own pace; lagging clients skip ahead.
    let (tx, _rx) = tokio::sync::broadcast::channel(64);
    tx
}
