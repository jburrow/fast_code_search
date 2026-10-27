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
    /// Record the (mtime, size) of the content just indexed under `id`,
    /// and whether it was read too soon after a change to vouch for it.
    pub(super) fn set_indexed_meta(&mut self, id: u32, mtime: u64, size: u64, racy: bool) {
        let idx = id as usize;
        if self.indexed_meta.len() <= idx {
            self.indexed_meta.resize(idx + 1, (0, 0));
        }
        self.indexed_meta[idx] = (mtime, size);
        if racy {
            self.racy_files.insert(id);
        } else {
            self.racy_files.remove(id);
        }
    }

    /// Was file `id` read so soon after a change that its size and mtime
    /// cannot vouch for its content?
    pub fn is_racy(&self, id: u32) -> bool {
        self.racy_files.contains(id)
    }

    /// Does `path` (the file indexed as `id`) still have the size and mtime
    /// it had when it was indexed, and can they vouch for its content? A
    /// racy file is never "unchanged": it is read again.
    pub fn is_unchanged_on_disk(&self, id: u32, path: &std::path::Path) -> bool {
        if self.is_racy(id) {
            return false;
        }
        let Some(&(mtime, size)) = self.indexed_meta.get(id as usize) else {
            return false;
        };
        match std::fs::metadata(path) {
            Ok(meta) => {
                meta.len() == size && crate::index::persistence::mtime_secs_of(&meta) == mtime
            }
            Err(_) => false,
        }
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
                self.set_indexed_meta(id, m.mtime, m.size, false);
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
        lists: Vec<(Trigram, roaring::RoaringBitmap)>,
        orig_to_new: &rustc_hash::FxHashMap<u32, u32>,
        persisted_files_len: usize,
    ) -> Vec<(Trigram, roaring::RoaringBitmap)> {
        // Fast path: identity mapping (clean reload — nothing stale/removed and no
        // dedupe, so every original id maps to itself and none are missing).
        // Avoids rebuilding every posting list. This is O(files), not O(postings).
        let is_identity =
            orig_to_new.len() == persisted_files_len && orig_to_new.iter().all(|(&o, &n)| o == n);
        if is_identity {
            return lists;
        }

        use rayon::prelude::*;
        Self::remap_lists(lists.par_iter().map(|(t, docs)| (t, docs)), orig_to_new)
    }

    /// [`Self::remap_lists`] over a map, for tests.
    #[cfg(test)]
    pub(super) fn remap_trigram_bitmaps_ref(
        map: &rustc_hash::FxHashMap<Trigram, roaring::RoaringBitmap>,
        orig_to_new: &rustc_hash::FxHashMap<u32, u32>,
    ) -> rustc_hash::FxHashMap<Trigram, roaring::RoaringBitmap> {
        use rayon::prelude::*;
        Self::remap_lists(map.par_iter(), orig_to_new)
    }

    /// Every posting list with each doc id translated through
    /// `orig_to_new`, dropping ids with no mapping and lists left empty.
    /// Used on load (persisted position -> live id) when files had to be
    /// renumbered.
    pub(super) fn remap_lists<'a, C>(
        lists: impl rayon::iter::ParallelIterator<Item = (&'a Trigram, &'a roaring::RoaringBitmap)>,
        orig_to_new: &rustc_hash::FxHashMap<u32, u32>,
    ) -> C
    where
        C: rayon::iter::FromParallelIterator<(Trigram, roaring::RoaringBitmap)>,
    {
        use rayon::prelude::*;
        let remap = IdRemap::new(orig_to_new);
        lists
            .filter_map(|(trigram, bitmap)| remap.list(*trigram, bitmap))
            .collect()
    }

    /// Restore symbol caches and dependency graph directly from persisted data,
    /// using the actual `original index → new file id` map. Each file's
    /// symbols and references are moved out of `persisted`, not copied.
    pub fn restore_symbols_and_deps(
        &mut self,
        orig_to_new: &rustc_hash::FxHashMap<u32, u32>,
        persisted: &mut crate::index::PersistedIndex,
    ) {
        // Size the symbol cache to the number of files actually registered.
        let total_new_files = self.file_store.len();
        // Each saved position's symbols go to the file now holding it.
        let mut place: Vec<Option<u32>> = Vec::new();
        for (&orig_idx, &new_id) in orig_to_new {
            let orig_idx = orig_idx as usize;
            if place.len() <= orig_idx {
                place.resize(orig_idx + 1, None);
            }
            place[orig_idx] = Some(new_id);
        }
        // Register files in dependency_index for future import resolution.
        let mut ids: Vec<u32> = orig_to_new
            .values()
            .copied()
            .filter(|&id| (id as usize) < total_new_files)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        // The symbols and the import graph's path tables are independent and
        // each about a fifth of a load at 302,000 files: build them at once.
        let symbols = std::mem::take(&mut persisted.symbols);
        let file_store = &self.file_store;
        let dependency_index = &mut self.dependency_index;
        let (symbol_cache, ()) = rayon::join(
            || {
                crate::symbols::SymbolStore::from_persisted(symbols, total_new_files, |pos| {
                    place.get(pos).copied().flatten()
                })
            },
            || {
                dependency_index.register_canonical_files(
                    ids.iter()
                        .filter_map(|&id| Some((id, file_store.shared_path(id)?.clone()))),
                )
            },
        );
        self.symbol_cache = symbol_cache;
        self.symbol_names = Default::default();
        self.note_symbol_names_replaced();
        self.install_references(
            std::mem::take(&mut persisted.reference_names),
            total_new_files,
        );
        for (&orig_idx, &new_id) in orig_to_new {
            if (new_id as usize) >= total_new_files {
                continue;
            }
            if let Some(refs) = persisted.references.get_mut(orig_idx as usize) {
                self.set_packed_references(new_id, std::mem::take(refs));
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
                let path = self.interned_path(new_id, path);
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
        self.symbol_cache = crate::symbols::SymbolStore::with_slots(total_files);
        self.symbol_names = Default::default();
        self.note_symbol_names_replaced();
        self.install_references(Vec::new(), total_files);
        self.pending_imports.clear();
        self.waiting_imports.clear();
        self.waiting_keys.clear();
        self.waiting_local.clear();
        self.import_specs.clear();
        self.spec_keys.clear();
        self.dirty_importers.clear();
        self.recently_added_stems.clear();
        self.dependency_index.clear();

        // Re-register all files for import resolution
        for file_id in 0..total_files as u32 {
            if let Some(path) = self.file_store.shared_path(file_id) {
                self.dependency_index
                    .register_canonical_file(file_id, path.clone());
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
                let mut references = Vec::new();
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

                                let (extracted_symbols, extracted_imports, extracted_refs) =
                                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                        extractor
                                            .extract_all_with_refs(&content)
                                            .unwrap_or_default()
                                    }))
                                    .unwrap_or_else(|_| {
                                        warn!(
                                            "Symbol/import extraction panicked for file '{}'. Continuing without symbols.",
                                            path.display()
                                        );
                                        (Vec::new(), Vec::new(), Vec::new())
                                    });

                                symbols = extracted_symbols;
                                references = extracted_refs;
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
                    references,
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

            self.symbol_cache.set(entry.file_id, entry.symbols);
            self.store_references(entry.file_id, entry.references);

            let path = self.interned_path(entry.file_id, &entry.path);
            self.record_import_specs(entry.file_id, &path, &entry.imports);
            if !entry.imports.is_empty() {
                self.pending_imports
                    .push((entry.file_id, path, entry.imports));
            }
        }

        progress_callback(total_files, total_files);

        self.resolve_imports();
        self.finalize();

        stats
    }

    /// Save the index to a file for persistence. Removed files are dropped
    /// and the rest renumbered by position, so the file has no gaps; use
    /// it when the server stops (and for tools). While the server runs,
    /// [`Self::checkpoint_index`] is cheaper.
    pub fn save_index(
        &self,
        path: &std::path::Path,
        config: &crate::config::IndexerConfig,
    ) -> anyhow::Result<()> {
        self.write_index(path, config, false)
    }

    /// Save the index keeping every file's id: removed files are written
    /// as empty slots (`PersistedIndex::dead_slots`). No posting list is
    /// renumbered, a mapped list no change touched is copied as its bytes,
    /// and afterwards the posting lists are read in place from the file
    /// just written (except on Windows), so the previous file is released.
    /// The slots go at the next [`Self::save_index`].
    pub fn checkpoint_index(
        &self,
        path: &std::path::Path,
        config: &crate::config::IndexerConfig,
    ) -> anyhow::Result<()> {
        self.write_index(path, config, true)
    }

    fn write_index(
        &self,
        path: &std::path::Path,
        config: &crate::config::IndexerConfig,
        keep_ids: bool,
    ) -> anyhow::Result<()> {
        use crate::index::persistence::get_mtime;
        use crate::index::{PersistedFileMetadata, PersistedIndex};

        // Collect file metadata with source base path tracking.
        //
        // Unless `keep_ids`, `files` is *compacted*: tombstoned (removed) ids
        // are skipped, so a file's position in `files` is not its live id
        // once anything has been removed. Everything else persisted (trigram
        // bitmaps, symbols, edges) must therefore be remapped from live id ->
        // position, otherwise reload attributes every file after a tombstone
        // to the wrong path. With `keep_ids` a tombstone is an empty slot and
        // position = id.
        // Configured roots in the form the store keys files by (canonical,
        // forward slashes, lowercased), computed once: a root written as a
        // symlinked or non-verbatim path (`/var/...` on macOS, `C:\...` on
        // Windows) must still claim its files.
        let root_keys: Vec<(String, String)> = config
            .paths
            .iter()
            .map(|base| {
                let canonical = super::canonicalize_lossy(std::path::Path::new(base));
                let key = canonical
                    .to_string_lossy()
                    .replace('\\', "/")
                    .to_lowercase()
                    .trim_end_matches('/')
                    .to_string();
                (base.clone(), key)
            })
            .collect();

        let total_ids = self.file_store.len() as u32;
        let mut files = Vec::new();
        let mut live_ids: Vec<u32> = Vec::new();
        let mut dead_slots: Vec<u32> = Vec::new();
        let mut id_to_pos: FxHashMap<u32, u32> = FxHashMap::default();
        for id in 0..total_ids {
            let Some(mapped_file) = self.file_store.get(id) else {
                if keep_ids {
                    dead_slots.push(id);
                    files.push(PersistedFileMetadata {
                        path: std::path::PathBuf::new(),
                        mtime: 0,
                        size: 0,
                        source_base_path: None,
                    });
                }
                continue;
            };
            {
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
                    // Saved as unknown, so the next load reads it again.
                    Some(_) if self.racy_files.contains(id) => 0,
                    Some((m, _)) => m,
                    None => get_mtime(&mapped_file.path).unwrap_or(0),
                };

                // Determine which base path this file belongs to
                // Longest root that contains the file at a path-component
                // boundary: `/x/proj` must not claim `/x/proj2/b.rs`, or
                // dropping `proj` from the config would report every
                // `proj2` file removed.
                let file_normalized = mapped_file
                    .path
                    .to_string_lossy()
                    .replace('\\', "/")
                    .to_lowercase();
                let source_base = root_keys
                    .iter()
                    .filter(|(_, key)| {
                        file_normalized == *key || file_normalized.starts_with(&format!("{key}/"))
                    })
                    .max_by_key(|(_, key)| key.len())
                    .map(|(base, _)| base.clone());

                let size = match recorded {
                    Some((_, sz)) => sz as usize,
                    None => mapped_file.len().unwrap_or(0),
                };

                files.push(PersistedFileMetadata {
                    path: mapped_file.path.to_path_buf(),
                    mtime,
                    size: size as u64,
                    source_base_path: source_base,
                });
            }
        }

        // The live id at each position of `files` (`None` for an empty slot).
        let slots: Vec<Option<u32>> = if keep_ids {
            let mut slots: Vec<Option<u32>> = (0..total_ids).map(Some).collect();
            for &id in &dead_slots {
                slots[id as usize] = None;
            }
            slots
        } else {
            live_ids.iter().copied().map(Some).collect()
        };

        // Collect per-file symbol caches (parallel to the files Vec)
        let symbols = self.symbol_cache.to_persisted(&slots);

        // Per-file references, by position; the name table is global.
        let references: Vec<Vec<crate::symbols::extractor::PackedRef>> = slots
            .iter()
            .map(|slot| {
                slot.map(|id| self.references_of(id).to_vec())
                    .unwrap_or_default()
            })
            .collect();
        drop(slots);

        // Collect resolved dependency edges, remapped onto positions; edges that
        // touch a removed file are dropped.
        let dependency_edges: Vec<(u32, u32)> = self
            .dependency_index
            .get_all_edges()
            .into_iter()
            .filter_map(|(from, to)| Some((*id_to_pos.get(&from)?, *id_to_pos.get(&to)?)))
            .collect();

        // Trigram bitmaps: written as they are when every id is its
        // position (ids kept, or no tombstones), otherwise remapped onto
        // positions.
        let same_ids = keep_ids || live_ids.len() as u32 == total_ids;
        let remapped_trigrams = (!same_ids).then(|| -> Vec<(Trigram, roaring::RoaringBitmap)> {
            let remap = IdRemap::new(&id_to_pos);
            self.trigram_index
                .map_postings(|t, docs| remap.list(t, &docs))
        });

        // Imports still waiting for their target, keyed by position, so a
        // checkpoint restore can still gain the edge when the target lands.
        let pending_imports: Vec<(u32, PathBuf, Vec<String>)> = self
            .waiting_imports_by_file()
            .into_iter()
            .filter_map(|(id, path, imports)| Some((*id_to_pos.get(&id)?, path, imports)))
            .collect();

        // The symbol name dictionary, by position, so the load need not
        // rebuild it.
        let mut position = vec![u32::MAX; total_ids as usize];
        for (&id, &pos) in &id_to_pos {
            position[id as usize] = pos;
        }
        let symbol_names = self
            .symbol_names
            .to_persisted(&position, self.symbol_cache.len());
        drop(position);

        // Create persisted index with config fingerprint
        let mut persisted = PersistedIndex::new(
            config.fingerprint(),
            config.paths.clone(),
            files,
            symbols,
            dependency_edges,
            pending_imports,
            self.reference_names().to_vec(),
            references,
        )?;
        persisted.symbol_names = symbol_names;
        persisted.dir_stamps = self.dir_stamps.clone();
        persisted.dead_slots = dead_slots;
        match remapped_trigrams {
            Some(remapped) => persisted.save(path, remapped)?,
            None => {
                // The file has every list under the ids held in memory, so
                // queries can read it in place from now on, and the file
                // mapped before (if any) is released.
                let mapped = self
                    .trigram_index
                    .with_postings_to_save(|postings| persisted.save_and_map(path, postings))?;
                if let Some(mapped) = mapped {
                    self.trigram_index.rebase(mapped, total_ids);
                }
            }
        }

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

        // Per-phase wall time, reported in the final log line so a slow load
        // says which step was slow.
        let started = std::time::Instant::now();
        let mut last = started;
        let mut phase_ms: Vec<(&'static str, u128)> = Vec::new();
        let mut mark = |name: &'static str| {
            let now = std::time::Instant::now();
            phase_ms.push((name, (now - last).as_millis()));
            last = now;
        };

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
        let mut persisted = PersistedIndex::load(path)?;
        let total_files = persisted.files.len();
        mark("read_and_decode");

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
        // Folder stamps describe the walk under the configuration they
        // were taken with.
        self.dir_stamps = persisted.dir_stamps.take().filter(|_| config_compatible);
        // Empty slots a checkpoint kept for removed files: never checked,
        // registered as tombstones so every later id comes back as saved.
        let mut dead_slot = vec![false; persisted.files.len()];
        for &pos in &persisted.dead_slots {
            if let Some(d) = dead_slot.get_mut(pos as usize) {
                *d = true;
            }
        }
        let mut file_statuses = batch_check_files(&persisted.files, &removed_paths);
        file_statuses.retain(|(idx, _)| !dead_slot[*idx]);
        mark("check_files");

        // The stat pass only sees "changed on disk". A file that is unchanged
        // but no longer eligible — a new exclude pattern, extension list,
        // size cap or .gitignore rule — must be dropped too, or a config
        // change never takes effect for files already in the index.
        if let Some(config) = config {
            use rayon::prelude::*;
            let probe = crate::search::EligibilityProbe::new(
                &config.discovery_config(config.paths.clone()),
            );
            let ineligible = file_statuses
                .par_iter_mut()
                .filter(|(_, status)| matches!(status, FileStatus::Valid))
                .map(|(idx, status)| {
                    let meta = &persisted.files[*idx];
                    if config.is_file_excluded(&meta.path)
                        || !probe.is_eligible_path(&meta.path, Some(meta.size))
                    {
                        *status = FileStatus::Removed;
                        1
                    } else {
                        0
                    }
                })
                .sum::<usize>();
            if ineligible > 0 {
                tracing::info!(
                    files = ineligible,
                    "Dropping indexed files that the current configuration excludes"
                );
            }
            mark("check_eligibility");
        }
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

            // Every saved file is registered, the stale and removed ones
            // too, so ids normally come back equal to the saved positions
            // and nothing has to be renumbered. The files that did not
            // survive are then tombstoned, as a removal at runtime is:
            // dropping them from the numbering instead shifted every later
            // id and rebuilt every posting list, which was the largest cost
            // of a load with even one changed file.
            let paths_to_register: Vec<std::path::PathBuf> =
                persisted.files.iter().map(|f| f.path.clone()).collect();
            let total_content_bytes: u64 = valid_file_indices
                .iter()
                .map(|&idx| persisted.files[idx].size)
                .sum();

            // Lazy registration: no I/O, the persisted size stands in for the
            // content until a file is first read.
            self.file_store.reserve(paths_to_register.len());
            let all_ids: Vec<u32> = paths_to_register
                .iter()
                .zip(&dead_slot)
                .map(|(path, &dead)| {
                    if dead {
                        self.file_store.push_dead_slot()
                    } else {
                        self.file_store.register_file(path)
                    }
                })
                .collect();
            let new_ids: Vec<u32> = valid_file_indices.iter().map(|&i| all_ids[i]).collect();
            orig_to_new = Self::build_orig_to_new_map(&valid_file_indices, &new_ids);
            let mut doomed = roaring::RoaringBitmap::new();
            for (idx, &id) in all_ids.iter().enumerate() {
                if id != u32::MAX && !orig_to_new.contains_key(&(idx as u32)) {
                    doomed.insert(id);
                }
            }
            // A path saved twice registers once: never tombstone an id a
            // surviving file holds.
            for &id in orig_to_new.values() {
                doomed.remove(id);
            }
            let in_place = all_ids
                .iter()
                .enumerate()
                .all(|(idx, &id)| id == idx as u32);
            self.seed_indexed_meta_from_persisted(&valid_file_indices, &new_ids, &persisted);
            self.file_store.add_content_bytes(total_content_bytes);
            mark("register_files");

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
            // With the saved ids, the posting lists are read in place from
            // the mapped file: nothing is decoded now, and the lists stay
            // page cache rather than heap. The files that did not survive
            // are voided in them (one bit each). Windows cannot
            // replace a mapped file, which the next save does, so there
            // (and when files had to be renumbered) they are decoded.
            let mapped = in_place && cfg!(not(windows));
            self.trigram_index = if mapped {
                crate::index::TrigramIndex::with_base(
                    std::mem::take(&mut persisted.trigram_index),
                    all_ids.len() as u32,
                )
            } else {
                let lists = persisted.restore_posting_lists()?;
                let lists = if in_place {
                    lists
                } else {
                    Self::remap_trigram_bitmaps(lists, &orig_to_new, persisted.files.len())
                };
                crate::index::TrigramIndex::from_trigram_map(lists)
            };
            for id in &doomed {
                self.file_store.remove_file_by_id(id);
            }
            // Every live id has a document, including files too short to
            // have a trigram.
            self.trigram_index
                .set_documents(self.file_store.live_ids().collect());
            if in_place {
                self.trigram_index.remove_documents(&doomed);
            }
            self.trigram_index.finalize();
            mark("trigrams");
        }

        let mut symbols_reextracted = false;
        if !self.file_store.is_empty() {
            let loaded = self.file_store.len();
            progress(
                LoadingPhase::RebuildingSymbols,
                Some(loaded),
                Some(0),
                "Restoring symbols and import graph...",
            );

            // Persisted symbols are restored only when they were extracted
            // by the current schema; otherwise (an older index, or no
            // symbols saved) they are re-extracted from the files.
            let schema_current =
                crate::config::symbols_schema_current(&persisted.config_fingerprint);
            if !schema_current && !persisted.symbols.is_empty() {
                tracing::info!(
                    saved = %persisted.config_fingerprint,
                    "Persisted symbols predate the current extractor; re-extracting"
                );
            }
            symbols_reextracted = persisted.symbols.is_empty() || !schema_current;
            if !symbols_reextracted {
                self.restore_symbols_and_deps(&orig_to_new, &mut persisted);
                if let Some(saved) = persisted.symbol_names.take() {
                    let mut id = vec![u32::MAX; persisted.files.len()];
                    for (&orig, &new) in &orig_to_new {
                        if let Some(slot) = id.get_mut(orig as usize) {
                            *slot = new;
                        }
                    }
                    self.symbol_names = symbol_names::SymbolNameIndex::from_persisted(
                        saved,
                        &id,
                        self.symbol_cache.len(),
                    );
                    self.note_symbol_names_replaced();
                }
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
            mark("symbols_and_imports");
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
        mark("file_metadata");
        // Restored from the file above unless the symbols were re-extracted
        // or the file had none.
        if !self.symbol_names.is_built() {
            self.rebuild_symbol_names();
        }
        mark("symbol_names");

        let already_indexed_files: Vec<std::path::PathBuf> = valid_file_indices
            .iter()
            .map(|&idx| persisted.files[idx].path.clone())
            .collect();

        tracing::info!(
            path = %path.display(),
            files_loaded = self.file_store.live_len(),
            stale_files = stale_files.len(),
            removed_files = removed_files.len(),
            new_paths = new_paths.len(),
            removed_paths = removed_paths.len(),
            config_compatible,
            reconciled = config.is_some(),
            total_ms = started.elapsed().as_millis(),
            phases_ms = ?phase_ms,
            "Index loaded from disk"
        );

        Ok(LoadIndexResult {
            stale_files,
            removed_files,
            new_paths,
            removed_paths,
            config_compatible,
            already_indexed_files,
            symbols_reextracted,
        })
    }
}

/// A translation of document ids (`orig_to_new`) applied to posting lists:
/// a dense table instead of a hash lookup per posting, and, when the
/// mapping keeps ids in order (it does unless registration deduped paths),
/// each list is rebuilt in one sorted pass rather than by inserting ids
/// one at a time. A save after a removal takes this path for every
/// posting list, so it is worth keeping cheap.
pub(super) struct IdRemap {
    dense: Vec<u32>,
    in_order: bool,
}

impl IdRemap {
    pub(super) fn new(orig_to_new: &rustc_hash::FxHashMap<u32, u32>) -> Self {
        let len = orig_to_new.keys().max().map_or(0, |&m| m as usize + 1);
        let mut dense = vec![u32::MAX; len];
        for (&o, &n) in orig_to_new {
            dense[o as usize] = n;
        }
        let mut last = None;
        let in_order = dense.iter().filter(|&&n| n != u32::MAX).all(|&n| {
            let ok = last.is_none_or(|l| n > l);
            last = Some(n);
            ok
        });
        Self { dense, in_order }
    }

    /// `bitmap` with every id translated, ids with no mapping dropped;
    /// `None` when nothing is left.
    pub(super) fn list(
        &self,
        trigram: Trigram,
        bitmap: &roaring::RoaringBitmap,
    ) -> Option<(Trigram, roaring::RoaringBitmap)> {
        let ids = bitmap.iter().filter_map(|old_id| {
            self.dense
                .get(old_id as usize)
                .copied()
                .filter(|&n| n != u32::MAX)
        });
        let remapped = if self.in_order {
            roaring::RoaringBitmap::from_sorted_iter(ids)
                .expect("an in-order id mapping keeps postings sorted")
        } else {
            ids.collect()
        };
        (!remapped.is_empty()).then_some((trigram, remapped))
    }
}
