//! Background indexing module for the keyword search engine.
//!
//! Handles file discovery, parallel batch indexing, persistence loading/saving,
//! and progress reporting via terminal and WebSocket broadcast.

use crate::config::IndexerConfig;
use crate::search::file_discovery::{FileDiscoveryConfig, FileDiscoveryIterator};
use crate::search::{
    IndexingProgress, IndexingStatus, LoadIndexResult, LoadingPhase, PartialIndexedFile,
    PreIndexedFile, ProgressBroadcaster, SearchEngine, SharedIndexingProgress,
};
use crate::utils::{format_bytes, format_number};

use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;
use tracing::info;

/// Channel buffer size for file discovery pipeline.
const CHANNEL_BUFFER: usize = 5000;

/// Configuration for the background indexer.
pub struct BackgroundIndexerConfig {
    /// The indexer configuration from the config file.
    pub indexer_config: IndexerConfig,

    /// Shared reference to the search engine.
    pub engine: Arc<RwLock<SearchEngine>>,

    /// Shared indexing progress state for UI visibility.
    pub progress: SharedIndexingProgress,

    /// Broadcast channel for WebSocket progress updates.
    pub progress_tx: ProgressBroadcaster,
}

/// Helper to update progress and broadcast to WebSocket clients.
fn broadcast_progress(
    progress: &SharedIndexingProgress,
    tx: &ProgressBroadcaster,
    update_fn: impl FnOnce(&mut IndexingProgress),
) {
    if let Ok(mut p) = progress.write() {
        update_fn(&mut p);
        // Broadcast to WebSocket clients (ignore errors if no subscribers)
        let _ = tx.send(p.clone());
    }
}

/// Progress bar manager for terminal output.
struct ProgressBarManager {
    progress_bar: Arc<Mutex<ProgressBar>>,
    spinner_style: Arc<ProgressStyle>,
    bar_style: Arc<ProgressStyle>,
}

impl ProgressBarManager {
    fn new() -> Self {
        let spinner_style = ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap();

        let bar_style = ProgressStyle::default_bar()
            .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("█▓░");

        let progress_bar = ProgressBar::new_spinner();
        progress_bar.set_style(spinner_style.clone());
        progress_bar.enable_steady_tick(std::time::Duration::from_millis(80));

        Self {
            progress_bar: Arc::new(Mutex::new(progress_bar)),
            spinner_style: Arc::new(spinner_style),
            bar_style: Arc::new(bar_style),
        }
    }

    fn set_spinner(&self, message: &str) {
        if let Ok(pb) = self.progress_bar.lock() {
            pb.set_style(self.spinner_style.as_ref().clone());
            pb.set_message(message.to_string());
        }
    }

    fn set_bar(&self, total: u64, position: u64, message: &str) {
        if let Ok(pb) = self.progress_bar.lock() {
            pb.set_style(self.bar_style.as_ref().clone());
            pb.set_length(total);
            pb.set_position(position);
            pb.set_message(message.to_string());
        }
    }

    fn finish(&self) {
        if let Ok(pb) = self.progress_bar.lock() {
            pb.finish_and_clear();
        }
    }

    /// Update the terminal progress bar based on loading phase.
    fn update_for_loading_phase(
        &self,
        phase: LoadingPhase,
        total: Option<usize>,
        processed: Option<usize>,
    ) {
        match phase {
            LoadingPhase::ReadingFile => {
                self.set_spinner("📖 Reading index file from disk...");
            }
            LoadingPhase::Deserializing => {
                self.set_spinner("🔄 Deserializing index data...");
            }
            LoadingPhase::CheckingFiles => {
                if let (Some(total), Some(processed)) = (total, processed) {
                    self.set_bar(
                        total as u64,
                        processed as u64,
                        "🔍 Checking files for changes",
                    );
                }
            }
            LoadingPhase::RestoringTrigrams => {
                self.set_spinner("🧠 Restoring search index...");
            }
            LoadingPhase::MappingFiles => {
                self.set_spinner("📁 Registering files...");
            }
            LoadingPhase::RebuildingSymbols => {
                if let (Some(total), Some(processed)) = (total, processed) {
                    self.set_bar(
                        total as u64,
                        processed as u64,
                        "🔧 Rebuilding symbols and imports",
                    );
                } else {
                    self.set_spinner("🔧 Rebuilding symbols and imports...");
                }
            }
            LoadingPhase::None => {}
        }
    }
}

/// Run the background indexing process.
///
/// This function handles:
/// - Loading persisted index if available
/// - Discovering files to index (new or stale)
/// - Parallel batch indexing with rayon
/// - Incremental import resolution
/// - Progress reporting to terminal and WebSocket
/// - Saving index after completion if configured
pub fn run(config: BackgroundIndexerConfig) {
    // Configure rayon with larger stack size (8MB) to handle tree-sitter recursion
    rayon::ThreadPoolBuilder::new()
        .stack_size(8 * 1024 * 1024)
        .build_global()
        .ok(); // Ignore error if already initialized

    let BackgroundIndexerConfig {
        indexer_config,
        engine: index_engine,
        progress: index_progress,
        progress_tx: index_progress_tx,
    } = config;

    let total_start = Instant::now();
    info!("Background indexing {} path(s)", indexer_config.paths.len());

    // Log active persistence settings so the user knows what to expect
    if let Some(ref p) = indexer_config.index_path {
        info!(
            index_path = %p,
            save_after_build = indexer_config.save_after_build,
            save_after_updates = indexer_config.save_after_updates,
            checkpoint_interval_files = indexer_config.checkpoint_interval_files,
            "Index persistence enabled"
        );
    }

    // Initialize progress
    broadcast_progress(&index_progress, &index_progress_tx, |p| {
        *p = IndexingProgress::start()
    });

    // Try to load persisted index if configured
    let (load_result, loaded_from_persistence) = try_load_persisted_index(
        &indexer_config,
        &index_engine,
        &index_progress,
        &index_progress_tx,
    );

    // Determine what needs to be indexed.
    // When restoring from a checkpoint, always scan all configured paths so
    // that files not yet indexed at the time of the checkpoint (e.g. because
    // the process was killed mid-build) are picked up.  Files that are already
    // in the index and unchanged are skipped inside the discovery thread via
    // the `already_indexed_files` set.
    let paths_to_index: Vec<String> = if loaded_from_persistence {
        let mut paths = indexer_config.paths.clone();
        // Also include any completely new paths that weren't in the old config.
        if let Some(ref result) = load_result {
            for p in &result.new_paths {
                if !paths.contains(p) {
                    paths.push(p.clone());
                }
            }
        }
        paths
    } else if let Some(ref result) = load_result {
        result.new_paths.clone()
    } else {
        indexer_config.paths.clone()
    };

    // Build the skip set: files already validly indexed (unchanged mtime/size).
    // Only populated when we loaded from a checkpoint so that re-scanning the
    // configured paths doesn't re-index files that are already up to date.
    let already_indexed_files: std::collections::HashSet<PathBuf> = if loaded_from_persistence {
        load_result
            .as_ref()
            .map(|r| r.already_indexed_files.iter().cloned().collect())
            .unwrap_or_default()
    } else {
        std::collections::HashSet::new()
    };
    let already_indexed_files = Arc::new(already_indexed_files);

    if loaded_from_persistence && !already_indexed_files.is_empty() {
        info!(
            already_indexed = already_indexed_files.len(),
            scanning_paths = paths_to_index.len(),
            "Checkpoint loaded - scanning all paths for unindexed files (skipping already-indexed)"
        );
    }

    let stale_files: Vec<PathBuf> = load_result
        .as_ref()
        .map(|r| r.stale_files.clone())
        .unwrap_or_default();

    // Count removed entries so the final save is not skipped when files were
    // deleted from disk but no stale/new files needed re-indexing
    // (total_indexed == 0 but removed_files_count > 0 means the index changed).
    let removed_files_count = load_result
        .as_ref()
        .map(|r| r.removed_files.len() + r.removed_paths.len())
        .unwrap_or(0);

    // Run the indexing pipeline
    let (total_indexed, batch_num, final_discovered) = run_indexing_pipeline(
        &paths_to_index,
        stale_files,
        &indexer_config,
        &index_engine,
        &index_progress,
        &index_progress_tx,
        loaded_from_persistence,
        already_indexed_files,
    );

    // Final import resolution
    finalize_imports(&index_engine, &index_progress, &index_progress_tx);

    // Log completion stats
    let elapsed = total_start.elapsed();
    log_completion_stats(total_indexed, final_discovered, elapsed, &index_engine);

    // Save index if configured
    if indexer_config.save_after_build {
        save_index_if_needed(
            &indexer_config,
            &index_engine,
            loaded_from_persistence,
            total_indexed,
            removed_files_count,
        );
    }

    // Update progress: completed
    let actual_file_count = index_engine
        .read()
        .map(|e| e.get_stats().num_files)
        .unwrap_or(0);

    let files_per_sec = if elapsed.as_secs_f64() > 0.0 {
        total_indexed as f64 / elapsed.as_secs_f64()
    } else {
        0.0
    };

    broadcast_progress(&index_progress, &index_progress_tx, |p| {
        p.status = IndexingStatus::Completed;
        p.files_indexed = actual_file_count;
        p.files_discovered = if loaded_from_persistence {
            actual_file_count
        } else {
            final_discovered
        };
        p.current_path = None;
        p.total_batches = batch_num;
        p.current_batch = batch_num;
        p.message = if loaded_from_persistence && total_indexed < actual_file_count {
            format!(
                "Ready: {} files loaded from cache, {} updated in {:.1}s",
                actual_file_count,
                total_indexed,
                elapsed.as_secs_f64()
            )
        } else {
            format!(
                "Indexing complete: {} files in {:.1}s ({:.0} files/sec)",
                actual_file_count,
                elapsed.as_secs_f64(),
                files_per_sec
            )
        };
    });
}

/// Try to load a persisted index from disk.
fn try_load_persisted_index(
    indexer_config: &IndexerConfig,
    index_engine: &Arc<RwLock<SearchEngine>>,
    index_progress: &SharedIndexingProgress,
    index_progress_tx: &ProgressBroadcaster,
) -> (Option<LoadIndexResult>, bool) {
    let Some(ref index_path_str) = indexer_config.index_path else {
        return (None, false);
    };

    let index_path = Path::new(index_path_str);
    if !index_path.exists() {
        return (None, false);
    }

    broadcast_progress(index_progress, index_progress_tx, |p| {
        p.status = IndexingStatus::LoadingIndex;
        p.loading_phase = LoadingPhase::ReadingFile;
        p.message = String::from("Loading persisted index...");
    });

    info!(path = %index_path.display(), "Attempting to load persisted index");

    let pb_manager = ProgressBarManager::new();
    pb_manager.set_spinner("📂 Loading persisted index...");

    let result = {
        let mut engine = match index_engine.write() {
            Ok(e) => e,
            Err(e) => {
                tracing::error!(error = %e, "Failed to acquire engine lock");
                return (None, false);
            }
        };

        // Create a callback that updates both WebSocket progress and terminal progress bar
        let progress_ref = index_progress;
        let tx_ref = index_progress_tx;

        engine.load_index_with_progress(
            index_path,
            indexer_config,
            |phase, total, processed, message| {
                // Update broadcast progress for web UI
                broadcast_progress(progress_ref, tx_ref, |p| {
                    p.loading_phase = phase;
                    p.loading_total_files = total;
                    p.loading_files_processed = processed;
                    p.message = message.to_string();
                });

                // Update terminal progress bar
                pb_manager.update_for_loading_phase(phase, total, processed);
            },
        )
    };

    pb_manager.finish();

    match result {
        Ok(result) => {
            let files_loaded = index_engine
                .read()
                .map(|e| e.get_stats().num_files)
                .unwrap_or(0);

            println!(
                "✅ Loaded {} files from persisted index",
                format_number(files_loaded)
            );
            if !result.stale_files.is_empty() || !result.new_paths.is_empty() {
                println!(
                    "   {} stale files to re-index, {} new paths to scan",
                    result.stale_files.len(),
                    result.new_paths.len()
                );
            }

            info!(
                files_loaded = files_loaded,
                stale_files = result.stale_files.len(),
                removed_files = result.removed_files.len(),
                new_paths = result.new_paths.len(),
                config_compatible = result.config_compatible,
                "Loaded persisted index"
            );

            broadcast_progress(index_progress, index_progress_tx, |p| {
                p.files_indexed = files_loaded;
                p.loading_phase = LoadingPhase::None;
                p.loading_total_files = None;
                p.loading_files_processed = None;
                p.message = format!("Loaded {} files from cache, reconciling...", files_loaded);
            });

            (Some(result), true)
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to load persisted index, will rebuild");
            (None, false)
        }
    }
}

/// Run the main indexing pipeline with file discovery and batch processing.
#[allow(clippy::too_many_arguments)]
fn run_indexing_pipeline(
    paths_to_index: &[String],
    stale_files: Vec<PathBuf>,
    indexer_config: &IndexerConfig,
    index_engine: &Arc<RwLock<SearchEngine>>,
    index_progress: &SharedIndexingProgress,
    index_progress_tx: &ProgressBroadcaster,
    loaded_from_persistence: bool,
    already_indexed_files: Arc<std::collections::HashSet<PathBuf>>,
) -> (usize, usize, usize) {
    let (tx, rx) = mpsc::sync_channel::<PathBuf>(CHANNEL_BUFFER);

    // Shared counters for progress tracking
    let files_discovered = Arc::new(AtomicUsize::new(0));
    let discovery_done = Arc::new(AtomicBool::new(false));

    // Spawn file discovery thread
    let discovery_handle = spawn_discovery_thread(
        paths_to_index.to_vec(),
        stale_files,
        indexer_config.exclude_patterns.clone(),
        indexer_config.include_extensions.clone(),
        indexer_config.max_file_size,
        tx,
        files_discovered.clone(),
        discovery_done.clone(),
        index_progress.clone(),
        index_progress_tx.clone(),
        already_indexed_files,
    );

    // Update status
    broadcast_progress(index_progress, index_progress_tx, |p| {
        if loaded_from_persistence {
            p.status = IndexingStatus::Reconciling;
            p.message = String::from("Reconciling index with filesystem...");
        } else {
            p.status = IndexingStatus::Indexing;
            p.message = String::from("Discovering and indexing files...");
        }
    });

    // Main indexing loop
    let (total_indexed, batch_num) = process_batches(
        rx,
        &files_discovered,
        &discovery_done,
        index_engine,
        index_progress,
        index_progress_tx,
        indexer_config,
        loaded_from_persistence,
    );

    // Wait for discovery thread; log if it panicked
    if let Err(e) = discovery_handle.join() {
        tracing::error!("File discovery thread panicked: {:?}", e);
    }

    let final_discovered = files_discovered.load(Ordering::Relaxed);

    // Update total batches
    broadcast_progress(index_progress, index_progress_tx, |p| {
        p.files_discovered = final_discovered;
        p.total_batches = batch_num;
        p.current_batch = batch_num;
    });

    (total_indexed, batch_num, final_discovered)
}

/// Spawn the file discovery thread.
#[allow(clippy::too_many_arguments)]
fn spawn_discovery_thread(
    paths_to_index: Vec<String>,
    stale_files: Vec<PathBuf>,
    exclude_patterns: Vec<String>,
    include_extensions: Vec<String>,
    max_file_size: u64,
    tx: SyncSender<PathBuf>,
    files_discovered: Arc<AtomicUsize>,
    discovery_done: Arc<AtomicBool>,
    discovery_progress: SharedIndexingProgress,
    discovery_progress_tx: ProgressBroadcaster,
    already_indexed_files: Arc<std::collections::HashSet<PathBuf>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        // Pre-compile exclude patterns the same way FileDiscoveryIterator does
        // so that stale files are subject to the same exclusion rules as newly
        // discovered files. Without this, a file that was indexed before a
        // "**/target/**" pattern was added would still be sent for re-indexing
        // when it becomes stale.
        let compiled_excludes: Vec<String> = exclude_patterns
            .iter()
            .map(|p| p.trim_matches('*').trim_matches('/').to_string())
            .filter(|p| !p.is_empty())
            .collect();

        // First, send stale files that need re-indexing
        for stale_path in stale_files {
            if !stale_path.exists() {
                continue;
            }
            // Apply exclude_patterns before queueing for re-indexing
            let path_str = stale_path.to_string_lossy();
            if compiled_excludes
                .iter()
                .any(|pattern| path_str.contains(pattern.as_str()))
            {
                tracing::debug!(
                    path = %stale_path.display(),
                    "Skipping stale file that matches an exclude pattern"
                );
                continue;
            }
            if tx.send(stale_path).is_err() {
                return; // Receiver dropped
            }
            files_discovered.fetch_add(1, Ordering::Relaxed);
        }

        // Then discover files from paths that need indexing.
        // Pass the original (unstripped) patterns — FileDiscoveryIterator
        // calls compile_exclude_patterns() internally.
        let discovery_config = FileDiscoveryConfig {
            paths: paths_to_index,
            exclude_patterns,
            include_extensions,
            max_file_size: Some(max_file_size),
            ..Default::default()
        };

        for path in FileDiscoveryIterator::new(&discovery_config) {
            // Skip files already validly indexed from a checkpoint.  We try
            // both the original path and its canonicalized form to match
            // however the file_store stored the path.
            if !already_indexed_files.is_empty() {
                let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                if already_indexed_files.contains(&canonical)
                    || already_indexed_files.contains(&path)
                {
                    continue;
                }
            }

            if tx.send(path).is_err() {
                break; // Receiver dropped
            }

            let count = files_discovered.fetch_add(1, Ordering::Relaxed) + 1;

            // Update progress periodically (every 1000 files)
            if count.is_multiple_of(1000) {
                if let Ok(mut p) = discovery_progress.write() {
                    p.files_discovered = count;
                    p.message = format!("Discovering... {} files found", count);
                    let _ = discovery_progress_tx.send(p.clone());
                }
            }
        }

        // Signal discovery complete
        discovery_done.store(true, Ordering::Release);
        let final_count = files_discovered.load(Ordering::Relaxed);
        info!(file_count = final_count, "File discovery completed");
    })
}

/// Process files in batches from the discovery channel.
#[allow(clippy::too_many_arguments)]
fn process_batches(
    rx: mpsc::Receiver<PathBuf>,
    files_discovered: &Arc<AtomicUsize>,
    discovery_done: &Arc<AtomicBool>,
    index_engine: &Arc<RwLock<SearchEngine>>,
    index_progress: &SharedIndexingProgress,
    index_progress_tx: &ProgressBroadcaster,
    indexer_config: &IndexerConfig,
    loaded_from_persistence: bool,
) -> (usize, usize) {
    let batch_size = indexer_config.batch_size.max(1);
    let mut batch: Vec<PathBuf> = Vec::with_capacity(batch_size);
    let mut total_indexed = 0usize;
    let mut batch_num = 0usize;
    // Track the total at the last checkpoint to compute a delta, avoiding the
    // LCM(batch_size, interval) trap that `is_multiple_of` creates when
    // total_indexed grows in fixed batch_size steps.
    let mut last_checkpoint_indexed = 0usize;

    loop {
        match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(path) => {
                batch.push(path);

                if batch.len() >= batch_size {
                    let indexed = process_batch(
                        &mut batch,
                        &mut batch_num,
                        files_discovered,
                        index_engine,
                        index_progress,
                        index_progress_tx,
                        &indexer_config.exclude_files,
                        indexer_config.transcode_non_utf8,
                    );
                    total_indexed += indexed;

                    // Checkpoint save during initial build every N files.
                    // Compare against a running delta so the checkpoint fires at the
                    // first batch boundary *after* `checkpoint_interval_files` new
                    // files have accumulated, regardless of common factors between
                    // batch_size and the configured interval.
                    if indexer_config.checkpoint_interval_files > 0
                        && total_indexed - last_checkpoint_indexed
                            >= indexer_config.checkpoint_interval_files
                    {
                        info!(
                            files_indexed = total_indexed,
                            interval = indexer_config.checkpoint_interval_files,
                            "Checkpoint: saving index mid-build"
                        );
                        save_index_if_needed(
                            indexer_config,
                            index_engine,
                            loaded_from_persistence,
                            total_indexed,
                            0,
                        );
                        last_checkpoint_indexed = total_indexed;
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if discovery_done.load(Ordering::Acquire) && rx.try_recv().is_err() {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    // Process remaining files
    if !batch.is_empty() {
        let indexed = process_batch(
            &mut batch,
            &mut batch_num,
            files_discovered,
            index_engine,
            index_progress,
            index_progress_tx,
            &indexer_config.exclude_files,
            indexer_config.transcode_non_utf8,
        );
        total_indexed += indexed;

        info!(
            batch = batch_num,
            files_indexed = total_indexed,
            "Final batch indexed"
        );
    }

    (total_indexed, batch_num)
}

/// Process a single batch of files.
#[allow(clippy::too_many_arguments)]
fn process_batch(
    batch: &mut Vec<PathBuf>,
    batch_num: &mut usize,
    files_discovered: &Arc<AtomicUsize>,
    index_engine: &Arc<RwLock<SearchEngine>>,
    index_progress: &SharedIndexingProgress,
    index_progress_tx: &ProgressBroadcaster,
    exclude_files: &[String],
    transcode_non_utf8: bool,
) -> usize {
    *batch_num += 1;
    let batch_start = Instant::now();
    let batch_len = batch.len();

    // Update progress
    let discovered = files_discovered.load(Ordering::Relaxed);
    broadcast_progress(index_progress, index_progress_tx, |p| {
        p.current_batch = *batch_num;
        p.files_discovered = discovered;
        p.message = format!("Indexing batch {} ({} files)...", batch_num, batch_len);
    });

    // Phase 1 (parallel, pure Rust — no tree-sitter C FFI):
    // Extract file content and trigrams across rayon threads safely.
    // Files listed in `exclude_files` are silently skipped here.
    //
    // Pre-normalize the exclude patterns once to avoid repeated string
    // allocations inside the per-file parallel filter closure.
    let normalized_exclude_files: Vec<String> =
        exclude_files.iter().map(|e| e.replace('\\', "/")).collect();
    let partial_with_flags: Vec<(PartialIndexedFile, bool)> = batch
        .par_iter()
        .filter(|path| {
            if normalized_exclude_files.is_empty() {
                return true;
            }
            let path_str = path.to_string_lossy().replace('\\', "/");
            let excluded = normalized_exclude_files.iter().any(|e| *e == path_str);
            if excluded {
                tracing::warn!(path = %path.display(), "Skipping excluded file");
            }
            !excluded
        })
        .filter_map(|path| {
            tracing::debug!(path = %path.display(), "Phase1: reading and extracting trigrams");
            PartialIndexedFile::process(path, transcode_non_utf8)
        })
        .collect();

    let batch_transcoded: usize = partial_with_flags.iter().filter(|(_, t)| *t).count();
    let partial: Vec<PartialIndexedFile> = partial_with_flags.into_iter().map(|(p, _)| p).collect();

    // Phase 2 (parallel — tree-sitter `Parser` is `Send + Sync` in tree-sitter v0.26+,
    // so independent parser instances can run safely on concurrent rayon threads):
    let pre_indexed: Vec<PreIndexedFile> = partial
        .into_par_iter()
        .filter_map(|p| {
            // Clone path before moving `p` into the closure so the error
            // handler can still log which file triggered the panic.
            let path_for_log = p.path.clone();
            tracing::debug!(path = %path_for_log.display(), "Phase2: extracting symbols");

            // Catch panics from tree-sitter C FFI on malformed files.
            // from_partial already wraps individual tree-sitter calls, but this
            // outer catch_unwind provides defense-in-depth against unexpected
            // panics anywhere in the processing pipeline.
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                PreIndexedFile::from_partial(p)
            })) {
                Ok(result) => Some(result),
                Err(_) => {
                    tracing::error!(
                        path = %path_for_log.display(),
                        "Phase2 panicked during symbol extraction"
                    );
                    None
                }
            }
        })
        .collect();

    let batch_indexed_count = pre_indexed.len();

    // Merge into engine and incrementally resolve imports
    {
        let mut engine = match index_engine.write() {
            Ok(e) => e,
            Err(e) => {
                tracing::error!(
                    error = %e,
                    files_dropped = batch.len(),
                    "Failed to acquire write lock on search engine; skipping batch merge"
                );
                batch.clear();
                return 0;
            }
        };
        engine.index_batch(pre_indexed);
        engine.resolve_imports_incremental();
    }

    // Update progress
    broadcast_progress(index_progress, index_progress_tx, |p| {
        p.files_indexed += batch_indexed_count;
        p.files_transcoded += batch_transcoded;
    });

    if (*batch_num).is_multiple_of(10) {
        info!(
            batch = *batch_num,
            files_discovered = discovered,
            batch_ms = batch_start.elapsed().as_millis(),
            "Batch indexed"
        );
    }

    batch.clear();
    batch_indexed_count
}

/// Finalize import resolution after all batches are processed.
fn finalize_imports(
    index_engine: &Arc<RwLock<SearchEngine>>,
    index_progress: &SharedIndexingProgress,
    index_progress_tx: &ProgressBroadcaster,
) {
    let mut engine = match index_engine.write() {
        Ok(e) => e,
        Err(e) => {
            tracing::error!(
                error = %e,
                "Failed to acquire write lock on search engine; skipping import finalization"
            );
            return;
        }
    };

    let pending_count = engine.pending_imports_count();

    if pending_count > 0 {
        broadcast_progress(index_progress, index_progress_tx, |p| {
            p.status = IndexingStatus::ResolvingImports;
            p.current_path = None;
            p.message = format!(
                "Resolving {} remaining import dependencies...",
                pending_count
            );
        });

        info!(
            pending_imports = pending_count,
            "Resolving remaining import dependencies..."
        );
        engine.resolve_imports();
    }

    info!("Finalizing index...");
    engine.finalize();

    let stats = engine.get_stats();
    info!(
        dependency_edges = stats.dependency_edges,
        "Import resolution completed"
    );
}

/// Log completion statistics.
fn log_completion_stats(
    total_indexed: usize,
    final_discovered: usize,
    elapsed: std::time::Duration,
    index_engine: &Arc<RwLock<SearchEngine>>,
) {
    let files_per_sec = if elapsed.as_secs_f64() > 0.0 {
        total_indexed as f64 / elapsed.as_secs_f64()
    } else {
        0.0
    };

    let indexed_size = index_engine
        .read()
        .map(|e| e.get_stats().total_size)
        .unwrap_or(0);

    // Get current process memory usage
    let process_memory = {
        use sysinfo::{Pid, ProcessesToUpdate, System};
        let pid = Pid::from_u32(std::process::id());
        let mut sys = System::new();
        sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
        sys.process(pid).map(|p| p.memory()).unwrap_or(0)
    };

    info!(
        elapsed_secs = format!("{:.1}", elapsed.as_secs_f64()),
        files_indexed = %format_number(total_indexed),
        files_discovered = %format_number(final_discovered),
        files_per_sec = format!("{:.0}", files_per_sec),
        indexed_size = %format_bytes(indexed_size),
        process_memory = %format_bytes(process_memory),
        "Background indexing completed"
    );
}

/// Called by the file-watcher loop after each successful file update.
/// Saves the index to disk when the `save_after_updates` threshold is reached.
pub fn save_on_watcher_update(
    indexer_config: &IndexerConfig,
    engine: &Arc<RwLock<SearchEngine>>,
    total_updates: usize,
) {
    if indexer_config.save_after_updates > 0
        && total_updates.is_multiple_of(indexer_config.save_after_updates)
    {
        // Watcher updates increment by 1, so is_multiple_of is exact here.
        save_index_if_needed(indexer_config, engine, true, total_updates, 0);
    }
}

/// Save the index to disk if configured and appropriate.
fn save_index_if_needed(
    indexer_config: &IndexerConfig,
    index_engine: &Arc<RwLock<SearchEngine>>,
    loaded_from_persistence: bool,
    total_indexed: usize,
    removed_files_count: usize,
) {
    let Some(ref index_path_str) = indexer_config.index_path else {
        return;
    };

    // Only save if the index actually changed since it was loaded.
    // `removed_files_count` covers the case where files were deleted from disk
    // but no new/stale files were re-indexed — the in-memory index is already
    // correct (deleted-file entries are absent) but the on-disk copy still has
    // them, so we must resave to avoid re-processing the same deletions on every
    // subsequent startup.
    let should_save = if loaded_from_persistence {
        total_indexed > 0 || removed_files_count > 0
    } else {
        true
    };

    if !should_save {
        info!("Index unchanged, skipping save");
        return;
    }

    let index_path = Path::new(index_path_str);
    info!(path = %index_path.display(), "Saving index to disk...");

    match index_engine.read() {
        Ok(engine) => {
            if let Err(e) = engine.save_index(index_path, indexer_config) {
                tracing::error!(
                    error = %e,
                    path = %index_path.display(),
                    "Failed to save index"
                );
            } else {
                info!(
                    path = %index_path.display(),
                    files = engine.get_stats().num_files,
                    "Index saved successfully"
                );
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to acquire read lock to save index");
        }
    }
}
