//! Split out of the search engine module; see `engine/mod.rs`.

use super::*;
use tracing::warn;

/// Progress sink used by the loaders: `(phase, total, done, message)`.
type LoadProgressSink<'a> = &'a mut dyn FnMut(LoadingPhase, Option<usize>, Option<usize>, &str);

impl SearchEngine {
    /// Build the `original persisted file index → new file id` map from the
    /// **actual** ids assigned during registration.
    ///
    /// `new_ids[p]` is the id assigned to the file at persisted index
    /// `valid_file_indices[p]`, or `u32::MAX` if that file failed to register
    /// (e.g. it was deleted between the staleness check and registration).
    /// Failed registrations are dropped from the map so their trigrams, symbols,
    /// and dependency edges are discarded rather than aliased onto another file.
    /// Record the (mtime, size) of the content just indexed under `id`.
    pub(super) fn set_indexed_meta(&mut self, id: u32, mtime: u64, size: u64) {
        let idx = id as usize;
        if self.indexed_meta.len() <= idx {
            self.indexed_meta.resize(idx + 1, (0, 0));
        }
        self.indexed_meta[idx] = (mtime, size);
    }

    /// After reload, the persisted (mtime, size) of each still-valid file is
    /// the identity of the content whose trigrams were restored; carry it
    /// forward so the next save does not re-stat.
    pub(super) fn seed_indexed_meta_from_persisted(
        &mut self,
        valid_file_indices: &[usize],
        new_ids: &[u32],
        persisted: &crate::index::PersistedIndex,
    ) {
        for (&idx, &id) in valid_file_indices.iter().zip(new_ids.iter()) {
            if id != u32::MAX {
                let m = &persisted.files[idx];
                self.set_indexed_meta(id, m.mtime, m.size);
            }
        }
    }

    pub(super) fn build_orig_to_new_map(
        valid_file_indices: &[usize],
        new_ids: &[u32],
    ) -> rustc_hash::FxHashMap<u32, u32> {
        valid_file_indices
            .iter()
            .zip(new_ids.iter())
            .filter(|(_, &new_id)| new_id != u32::MAX)
            .map(|(&orig_idx, &new_id)| (orig_idx as u32, new_id))
            .collect()
    }

    /// Remap a restored trigram map (keyed by *original* persisted doc ids) onto
    /// the new file ids assigned during load.
    ///
    /// This MUST mirror the same `orig_to_new` mapping used for symbols and
    /// dependency edges: persisted bitmaps reference positions in
    /// `persisted.files`, but reload only re-registers the still-valid files and
    /// assigns them fresh compacted ids. Without this remap, a single stale file
    /// shifts every later file's id and search hits get attributed to the wrong
    /// file. Original ids with no entry in `orig_to_new` (stale/removed/failed)
    /// are dropped, and trigrams whose postings become empty are pruned.
    pub(super) fn remap_trigram_bitmaps(
        map: rustc_hash::FxHashMap<Trigram, roaring::RoaringBitmap>,
        orig_to_new: &rustc_hash::FxHashMap<u32, u32>,
        persisted_files_len: usize,
    ) -> rustc_hash::FxHashMap<Trigram, roaring::RoaringBitmap> {
        // Fast path: identity mapping (clean reload — nothing stale/removed and no
        // dedupe, so every original id maps to itself and none are missing).
        // Avoids rebuilding every posting list. This is O(files), not O(postings).
        let is_identity =
            orig_to_new.len() == persisted_files_len && orig_to_new.iter().all(|(&o, &n)| o == n);
        if is_identity {
            return map;
        }

        Self::remap_trigram_bitmaps_ref(&map, orig_to_new)
    }

    /// Borrowing form of [`Self::remap_trigram_bitmaps`]: always builds a new
    /// map with every doc id translated through `orig_to_new`, dropping ids
    /// with no mapping and trigrams whose postings become empty. Used both on
    /// load (persisted position -> live id) and on save (live id -> position).
    pub(super) fn remap_trigram_bitmaps_ref(
        map: &rustc_hash::FxHashMap<Trigram, roaring::RoaringBitmap>,
        orig_to_new: &rustc_hash::FxHashMap<u32, u32>,
    ) -> rustc_hash::FxHashMap<Trigram, roaring::RoaringBitmap> {
        let mut out: rustc_hash::FxHashMap<Trigram, roaring::RoaringBitmap> =
            rustc_hash::FxHashMap::default();
        for (trigram, bitmap) in map {
            let mut remapped = roaring::RoaringBitmap::new();
            for old_id in bitmap.iter() {
                if let Some(&new_id) = orig_to_new.get(&old_id) {
                    remapped.insert(new_id);
                }
            }
            if !remapped.is_empty() {
                out.insert(*trigram, remapped);
            }
        }
        out
    }

    /// Restore symbol caches and dependency graph directly from persisted data,
    /// using the actual `original index → new file id` map.
    pub fn restore_symbols_and_deps(
        &mut self,
        orig_to_new: &rustc_hash::FxHashMap<u32, u32>,
        persisted: &crate::index::PersistedIndex,
    ) {
        // Size the symbol cache to the number of files actually registered.
        let total_new_files = self.file_store.len();
        self.symbol_cache = vec![Vec::new(); total_new_files];

        // Register files in dependency_index for future import resolution
        // and restore per-file symbol caches, keyed by the real new ids.
        for (&orig_idx, &new_id) in orig_to_new {
            if (new_id as usize) >= total_new_files {
                continue;
            }
            if let Some(path) = self.file_store.get_path(new_id) {
                self.dependency_index.register_file(new_id, path);
            }
            if let Some(syms) = persisted.symbols.get(orig_idx as usize) {
                self.symbol_cache[new_id as usize] = syms.clone();
            }
        }

        // Restore dependency edges, remapping original indices to new file IDs
        // and dropping edges whose endpoints were stale/removed.
        let remapped_edges: Vec<(u32, u32)> = persisted
            .dependency_edges
            .iter()
            .filter_map(|&(from_orig, to_orig)| {
                let new_from = orig_to_new.get(&from_orig)?;
                let new_to = orig_to_new.get(&to_orig)?;
                Some((*new_from, *new_to))
            })
            .collect();
        self.dependency_index.add_imports_batch(remapped_edges);

        // Re-park imports that were still unresolved at save time (files that
        // were not restored are stale and will be re-extracted anyway).
        for (orig_idx, path, imports) in &persisted.pending_imports {
            if let Some(&new_id) = orig_to_new.get(orig_idx) {
                for import in imports {
                    self.park_import(new_id, path.clone(), import.clone());
                }
            }
        }
    }

    pub fn rebuild_symbols_and_dependencies_with_progress<F>(
        &mut self,
        mut progress_callback: F,
    ) -> RebuildCacheStats
    where
        F: FnMut(usize, usize),
    {
        let total_files = self.file_store.len();
        if total_files == 0 {
            return RebuildCacheStats::default();
        }

        // Reset derived state
        self.symbol_cache = vec![Vec::new(); total_files];
        self.pending_imports.clear();
        self.waiting_imports.clear();
        self.waiting_keys.clear();
        self.recently_added_stems.clear();
        self.dependency_index.clear();

        // Re-register all files for import resolution
        for file_id in 0..total_files as u32 {
            if let Some(path) = self.file_store.get_path(file_id) {
                self.dependency_index.register_file(file_id, path);
            }
        }

        let file_store = &self.file_store;
        let enable_symbols = self.enable_symbols;

        progress_callback(0, total_files);

        let entries: Vec<RebuildEntry> = (0..total_files as u32)
            .into_par_iter()
            .filter_map(|file_id| {
                let path = file_store.get_path(file_id)?.to_path_buf();

                let mut symbols = Vec::new();
                let mut imports = Vec::new();
                let mut had_content = false;

                if let Some(file) = file_store.get(file_id) {
                    if let Ok(content) = file.as_str() {
                        had_content = true;

                        // Skip tree-sitter extraction when symbols are disabled
                        if enable_symbols {
                            // Skip files that are unsafe for tree-sitter parsing
                            if let Some(reason) = crate::utils::content_safety_check(&content) {
                                tracing::debug!(
                                    path = %path.display(),
                                    reason = reason,
                                    "Skipping unsafe file during symbol rebuild"
                                );
                                // Fall through — filename symbol is still added below
                            } else {
                                let extractor = SymbolExtractor::new(&path);

                                let (extracted_symbols, extracted_imports) =
                                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                        extractor.extract_all(&content).unwrap_or_default()
                                    }))
                                    .unwrap_or_else(|_| {
                                        warn!(
                                            "Symbol/import extraction panicked for file '{}'. Continuing without symbols.",
                                            path.display()
                                        );
                                        (Vec::new(), Vec::new())
                                    });

                                symbols = extracted_symbols;
                                imports = extracted_imports
                                    .into_iter()
                                    .map(|i| i.path)
                                    .collect();
                            }
                        }
                    }
                }

                // Always add filename symbol for filename-only matches.
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if !stem.is_empty() {
                        symbols.push(Symbol {
                            name: stem.to_string(),
                            symbol_type: SymbolType::FileName,
                            line: 0,
                            column: 0,
                            is_definition: true,
                        });
                    }
                }

                Some(RebuildEntry {
                    file_id,
                    path,
                    symbols,
                    imports,
                    had_content,
                })
            })
            .collect();

        let mut stats = RebuildCacheStats {
            files_processed: entries.len(),
            ..RebuildCacheStats::default()
        };

        for entry in entries {
            stats.symbols_extracted += entry.symbols.len();
            stats.imports_extracted += entry.imports.len();
            if !entry.had_content {
                stats.files_skipped += 1;
            }

            if entry.file_id as usize >= self.symbol_cache.len() {
                self.symbol_cache
                    .resize(entry.file_id as usize + 1, Vec::new());
            }
            self.symbol_cache[entry.file_id as usize] = entry.symbols;

            if !entry.imports.is_empty() {
                self.pending_imports
                    .push((entry.file_id, entry.path, entry.imports));
            }
        }

        progress_callback(total_files, total_files);

        self.resolve_imports();
        self.finalize();

        stats
    }

    /// Save the index to a file for persistence
    pub fn save_index(
        &self,
        path: &std::path::Path,
        config: &crate::config::IndexerConfig,
    ) -> anyhow::Result<()> {
        use crate::index::persistence::get_mtime;
        use crate::index::{PersistedFileMetadata, PersistedIndex};

        // Collect file metadata with source base path tracking.
        //
        // `files` is *compacted*: tombstoned (removed) ids are skipped, so a
        // file's position in `files` is not its live id once anything has been
        // removed. Everything else persisted (trigram bitmaps, symbols, edges)
        // must therefore be remapped from live id -> position, otherwise reload
        // attributes every file after a tombstone to the wrong path.
        let total_ids = self.file_store.len() as u32;
        let mut files = Vec::new();
        let mut live_ids: Vec<u32> = Vec::new();
        let mut id_to_pos: FxHashMap<u32, u32> = FxHashMap::default();
        for id in 0..total_ids {
            if let Some(mapped_file) = self.file_store.get(id) {
                id_to_pos.insert(id, files.len() as u32);
                live_ids.push(id);
                // Prefer the identity recorded when the content was read; only
                // stat as a fallback for ids with no record.
                let recorded = self
                    .indexed_meta
                    .get(id as usize)
                    .copied()
                    .filter(|&(m, sz)| m != 0 || sz != 0);
                let mtime = match recorded {
                    Some((m, _)) => m,
                    None => get_mtime(&mapped_file.path).unwrap_or(0),
                };

                // Determine which base path this file belongs to
                let source_base = config
                    .paths
                    .iter()
                    .find(|base| {
                        let base_normalized = base.replace('\\', "/").to_lowercase();
                        let file_normalized = mapped_file
                            .path
                            .to_string_lossy()
                            .replace('\\', "/")
                            .to_lowercase();
                        file_normalized.starts_with(&base_normalized)
                    })
                    .cloned();

                // Use len_if_mapped() to avoid triggering lazy loading during save
                // If file isn't mapped yet, get size from filesystem
                let size = match recorded {
                    Some((_, sz)) => sz as usize,
                    None => mapped_file.len_if_mapped().unwrap_or_else(|| {
                        std::fs::metadata(&mapped_file.path)
                            .map(|m| m.len() as usize)
                            .unwrap_or(0)
                    }),
                };

                files.push(PersistedFileMetadata {
                    path: mapped_file.path.to_path_buf(),
                    mtime,
                    size: size as u64,
                    source_base_path: source_base,
                });
            }
        }

        // Collect per-file symbol caches (parallel to the compacted files Vec)
        let symbols: Vec<Vec<crate::symbols::extractor::Symbol>> = live_ids
            .iter()
            .map(|&id| {
                self.symbol_cache
                    .get(id as usize)
                    .cloned()
                    .unwrap_or_default()
            })
            .collect();

        // Collect resolved dependency edges, remapped onto positions; edges that
        // touch a removed file are dropped.
        let dependency_edges: Vec<(u32, u32)> = self
            .dependency_index
            .get_all_edges()
            .into_iter()
            .filter_map(|(from, to)| Some((*id_to_pos.get(&from)?, *id_to_pos.get(&to)?)))
            .collect();

        // Trigram bitmaps: borrow as-is when ids are already dense (no
        // tombstones), otherwise build a remapped copy keyed by position.
        let is_identity = live_ids.len() as u32 == total_ids;
        let remapped_trigrams;
        let trigram_map: &FxHashMap<Trigram, roaring::RoaringBitmap> = if is_identity {
            self.trigram_index.get_trigram_map()
        } else {
            remapped_trigrams =
                Self::remap_trigram_bitmaps_ref(self.trigram_index.get_trigram_map(), &id_to_pos);
            &remapped_trigrams
        };

        // Imports still waiting for their target, keyed by position, so a
        // checkpoint restore can still gain the edge when the target lands.
        let pending_imports: Vec<(u32, PathBuf, Vec<String>)> = self
            .waiting_imports_by_file()
            .into_iter()
            .filter_map(|(id, path, imports)| Some((*id_to_pos.get(&id)?, path, imports)))
            .collect();

        // Create persisted index with config fingerprint
        let persisted = PersistedIndex::new(
            config.fingerprint(),
            config.paths.clone(),
            files,
            trigram_map,
            symbols,
            dependency_edges,
            pending_imports,
        )?;
        persisted.save(path)?;

        tracing::info!(
            path = %path.display(),
            files = self.file_store.len(),
            trigrams = self.trigram_index.num_trigrams(),
            config_fingerprint = %config.fingerprint(),
            "Index saved to disk"
        );

        Ok(())
    }

    /// Check if a persisted index exists and is usable
    pub fn can_load_index(path: &std::path::Path) -> bool {
        path.exists()
    }

    /// Load an index from disk and reconcile it against the current
    /// configuration.
    ///
    /// Returns which files are stale or gone and which configured paths were
    /// added or removed since the index was saved, so the caller can index
    /// exactly the difference. Configured paths are registered as roots for
    /// display-path computation.
    pub fn load_index_with_reconciliation(
        &mut self,
        path: &std::path::Path,
        config: &crate::config::IndexerConfig,
    ) -> anyhow::Result<LoadIndexResult> {
        self.load_index_inner(path, Some(config), &mut |_, _, _, _| {})
    }

    /// [`load_index_with_reconciliation`](Self::load_index_with_reconciliation)
    /// with a progress callback.
    ///
    /// The callback receives `(phase, total, done, message)` as loading moves
    /// through reading and deserializing the file, checking files for
    /// staleness, registering the surviving files, restoring the trigram
    /// index and restoring (or rebuilding) symbols and imports.
    pub fn load_index_with_progress<F>(
        &mut self,
        path: &std::path::Path,
        config: &crate::config::IndexerConfig,
        mut progress_callback: F,
    ) -> anyhow::Result<LoadIndexResult>
    where
        F: FnMut(LoadingPhase, Option<usize>, Option<usize>, &str),
    {
        self.load_index_inner(path, Some(config), &mut progress_callback)
    }

    /// Load an index from disk without a configuration to reconcile against.
    ///
    /// Every persisted file that still exists unchanged is restored; the
    /// returned list holds the files that need re-indexing because they
    /// changed or disappeared.
    pub fn load_index(
        &mut self,
        path: &std::path::Path,
    ) -> anyhow::Result<Vec<std::path::PathBuf>> {
        let mut result = self.load_index_inner(path, None, &mut |_, _, _, _| {})?;
        result.stale_files.append(&mut result.removed_files);
        Ok(result.stale_files)
    }

    /// The single load path behind the three public loaders.
    ///
    /// Without a `config` there is nothing to reconcile: the fingerprint is
    /// taken as compatible and no configured paths are added, removed or
    /// registered as roots. Files are registered lazily (no I/O) with the
    /// persisted size and mtime, then trigrams, symbols and dependency edges
    /// are remapped onto the ids the registration actually assigned.
    fn load_index_inner(
        &mut self,
        path: &std::path::Path,
        config: Option<&crate::config::IndexerConfig>,
        progress: LoadProgressSink<'_>,
    ) -> anyhow::Result<LoadIndexResult> {
        use crate::index::persistence::{batch_check_files, FileStatus, PersistedIndex};

        progress(
            LoadingPhase::ReadingFile,
            None,
            None,
            "Reading index file from disk...",
        );
        progress(
            LoadingPhase::Deserializing,
            None,
            None,
            "Deserializing index data...",
        );
        let persisted = PersistedIndex::load(path)?;
        let total_files = persisted.files.len();

        let (config_compatible, new_paths, removed_paths) = match config {
            Some(config) => {
                let current_fingerprint = config.fingerprint();
                let compatible = persisted.is_config_compatible(&current_fingerprint);
                if !compatible {
                    tracing::info!(
                        old_fingerprint = %persisted.config_fingerprint,
                        new_fingerprint = %current_fingerprint,
                        "Config fingerprint changed, will reconcile"
                    );
                }
                (
                    compatible,
                    persisted.paths_to_add(&config.paths),
                    persisted.paths_to_remove(&config.paths),
                )
            }
            None => (true, Vec::new(), Vec::new()),
        };

        progress(
            LoadingPhase::CheckingFiles,
            Some(total_files),
            Some(0),
            &format!("Checking {} files for changes...", total_files),
        );
        let file_statuses = batch_check_files(&persisted.files, &removed_paths);
        progress(
            LoadingPhase::CheckingFiles,
            Some(total_files),
            Some(total_files),
            &format!("Checked {} files", total_files),
        );

        let mut stale_files = Vec::new();
        let mut removed_files = Vec::new();
        let mut valid_file_indices = Vec::new();
        for (idx, status) in file_statuses {
            match status {
                FileStatus::Valid => valid_file_indices.push(idx),
                FileStatus::Stale => stale_files.push(persisted.files[idx].path.clone()),
                FileStatus::Removed => removed_files.push(persisted.files[idx].path.clone()),
            }
        }
        let valid_count = valid_file_indices.len();

        // Map from persisted position -> new file id, built from the ids the
        // registration actually assigned so trigrams/symbols/deps stay aligned.
        let mut orig_to_new: rustc_hash::FxHashMap<u32, u32> = rustc_hash::FxHashMap::default();

        if valid_count > 0 {
            progress(
                LoadingPhase::MappingFiles,
                Some(valid_count),
                Some(0),
                &format!("Registering {} files...", valid_count),
            );

            let paths_to_register: Vec<std::path::PathBuf> = valid_file_indices
                .iter()
                .map(|&idx| persisted.files[idx].path.clone())
                .collect();
            let total_content_bytes: u64 = valid_file_indices
                .iter()
                .map(|&idx| persisted.files[idx].size)
                .sum();

            // Lazy registration: no I/O, the persisted size stands in for the
            // content until a file is first read.
            self.file_store.reserve(paths_to_register.len());
            let new_ids = self.file_store.register_files_bulk(&paths_to_register);
            orig_to_new = Self::build_orig_to_new_map(&valid_file_indices, &new_ids);
            self.seed_indexed_meta_from_persisted(&valid_file_indices, &new_ids, &persisted);
            self.file_store.add_content_bytes(total_content_bytes);

            progress(
                LoadingPhase::MappingFiles,
                Some(valid_count),
                Some(valid_count),
                &format!("Registered {} files (lazy loading enabled)", valid_count),
            );

            progress(
                LoadingPhase::RestoringTrigrams,
                None,
                None,
                "Restoring search index...",
            );
            let trigram_map = persisted.restore_trigram_index()?;
            let remapped =
                Self::remap_trigram_bitmaps(trigram_map, &orig_to_new, persisted.files.len());
            self.trigram_index = crate::index::TrigramIndex::from_trigram_map(remapped);
            self.trigram_index.finalize();
        }

        if !self.file_store.is_empty() {
            let loaded = self.file_store.len();
            progress(
                LoadingPhase::RebuildingSymbols,
                Some(loaded),
                Some(0),
                "Restoring symbols and import graph...",
            );

            if !persisted.symbols.is_empty() {
                self.restore_symbols_and_deps(&orig_to_new, &persisted);
                tracing::info!(
                    files_restored = valid_count,
                    "Restored symbol and dependency caches from persisted index"
                );
            } else {
                // Index written without symbols: re-extract from file contents.
                let stats =
                    self.rebuild_symbols_and_dependencies_with_progress(|processed, total| {
                        progress(
                            LoadingPhase::RebuildingSymbols,
                            Some(total),
                            Some(processed),
                            "Rebuilding symbols and import graph...",
                        );
                    });
                tracing::info!(
                    symbols_rebuilt = stats.symbols_extracted,
                    imports_rebuilt = stats.imports_extracted,
                    files_skipped = stats.files_skipped,
                    "Rebuilt symbol and dependency caches after load (no persisted symbols)"
                );
            }

            progress(
                LoadingPhase::RebuildingSymbols,
                Some(loaded),
                Some(loaded),
                "Symbol and dependency caches ready",
            );
        }

        // Configured paths are the roots for display-path computation. They
        // must be known before the metadata pass below caches display paths,
        // otherwise results show absolute paths until the next finalize.
        if let Some(config) = config {
            for path_str in &config.paths {
                self.add_root_path(std::path::Path::new(path_str));
            }
        }

        // Fast-mode ranking needs per-file metadata; without it a freshly
        // loaded index ranks by id order until the background finalize runs.
        self.compute_all_file_metadata();
        self.generation += 1;

        let already_indexed_files: Vec<std::path::PathBuf> = valid_file_indices
            .iter()
            .map(|&idx| persisted.files[idx].path.clone())
            .collect();

        tracing::info!(
            path = %path.display(),
            files_loaded = self.file_store.len(),
            stale_files = stale_files.len(),
            removed_files = removed_files.len(),
            new_paths = new_paths.len(),
            removed_paths = removed_paths.len(),
            config_compatible,
            reconciled = config.is_some(),
            "Index loaded from disk"
        );

        Ok(LoadIndexResult {
            stale_files,
            removed_files,
            new_paths,
            removed_paths,
            config_compatible,
            already_indexed_files,
        })
    }
}
