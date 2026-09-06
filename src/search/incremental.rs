//! Apply a single watcher event to the engine.
//!
//! The watcher reports *paths*; whether a path is a file or a directory is
//! only known here. Directory renames and deletes used to be silent no-ops
//! (`remove_file(dir)` matched nothing and `update_file(dir)` failed to read a
//! directory), leaving every file under the old name in the index forever.
//! This module handles both shapes and applies the same eligibility rules the
//! initial build uses (exclude patterns, include extensions, size cap,
//! exclude_files) when indexing a directory that appeared or was renamed in.

use crate::config::IndexerConfig;
use crate::search::file_discovery::{FileDiscoveryConfig, FileDiscoveryIterator};
use crate::search::watcher::FileChange;
use crate::search::SearchEngine;
use std::path::Path;

/// What applying a change did, for logging and save accounting.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ChangeOutcome {
    /// Files (re)indexed.
    pub indexed: usize,
    /// Files removed from the index.
    pub removed: usize,
}

impl ChangeOutcome {
    /// True if the index changed at all.
    pub fn changed(&self) -> bool {
        self.indexed > 0 || self.removed > 0
    }
}

/// Apply a burst of changes under one write lock.
///
/// Events are coalesced per path (the last event for a path wins; a rename
/// is a delete of `from` plus a modify of `to`), all removals are applied in
/// a single pass over the trigram index, then the surviving modifications
/// are indexed. A `git checkout` touching thousands of files therefore costs
/// one lock window and one posting-list scan instead of one per file.
pub fn apply_changes(
    engine: &mut SearchEngine,
    changes: &[FileChange],
    config: &IndexerConfig,
) -> ChangeOutcome {
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Op {
        Delete,
        Modify,
    }

    // Ordered coalescing: insertion order is kept so directory removals happen
    // before files that were moved into a directory of the same name, etc.
    let mut order: Vec<PathBuf> = Vec::new();
    let mut ops: HashMap<PathBuf, Op> = HashMap::new();
    let mut set = |path: &PathBuf, op: Op| {
        if !ops.contains_key(path) {
            order.push(path.clone());
        }
        ops.insert(path.clone(), op);
    };
    for change in changes {
        match change {
            FileChange::Modified(p) => set(p, Op::Modify),
            FileChange::Deleted(p) => set(p, Op::Delete),
            FileChange::Renamed { from, to } => {
                set(from, Op::Delete);
                set(to, Op::Modify);
            }
        }
    }

    // Phase 1: collect every id to remove (files, or whole directories for
    // paths with no id) and drop them in one pass.
    let mut doomed: Vec<u32> = Vec::new();
    for path in &order {
        if ops[path] != Op::Delete {
            continue;
        }
        match engine.find_file_id(&path.to_string_lossy()) {
            Some(id) => doomed.push(id),
            None => doomed.extend(engine.file_ids_under(path)),
        }
    }
    let removed = engine.remove_files_by_ids(&doomed);

    // Phase 2: (re)index the modified paths.
    let mut outcome = ChangeOutcome {
        indexed: 0,
        removed,
    };
    for path in &order {
        if ops[path] != Op::Modify {
            continue;
        }
        let o = index_path(engine, path, config);
        outcome.indexed += o.indexed;
        outcome.removed += o.removed;
    }

    // Phase 3: a backend may have dropped the delete / rename-away half of
    // what just happened (FSEvents does), so check the siblings of every
    // changed path and drop entries whose file is gone.
    let mut dirs: Vec<PathBuf> = Vec::new();
    for path in &order {
        let dir = if path.is_dir() {
            path.clone()
        } else if let Some(parent) = path.parent() {
            parent.to_path_buf()
        } else {
            continue;
        };
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    outcome.removed += engine.prune_vanished_in(&dirs);
    outcome
}

/// Apply one file-system change to the engine under the caller's write lock
/// (a one-element [`apply_changes`], including the vanished-sibling check).
pub fn apply_change(
    engine: &mut SearchEngine,
    change: &FileChange,
    config: &IndexerConfig,
) -> ChangeOutcome {
    apply_changes(engine, std::slice::from_ref(change), config)
}

/// Remove `path` from the index: a single file, or — when no file id matches
/// (the path was a directory, or is already gone) — every file under it.
fn remove_path(engine: &mut SearchEngine, path: &Path) -> usize {
    if engine.remove_file(path) {
        1
    } else {
        engine.remove_files_under(path)
    }
}

/// Index or re-index `path`: a single eligible file, or every eligible file
/// under a directory (e.g. the target of a directory rename).
fn index_path(engine: &mut SearchEngine, path: &Path, config: &IndexerConfig) -> ChangeOutcome {
    if path.is_dir() {
        let discovery = FileDiscoveryConfig {
            paths: vec![path.to_string_lossy().to_string()],
            exclude_patterns: config.exclude_patterns.clone(),
            include_extensions: config.include_extensions.clone(),
            max_file_size: Some(effective_max_size(config)),
            respect_gitignore: config.respect_gitignore,
            ..Default::default()
        };
        let mut indexed = 0;
        for file in FileDiscoveryIterator::new(&discovery) {
            if config.is_file_excluded(&file) {
                continue;
            }
            match engine.update_file(&file) {
                Ok(()) => indexed += 1,
                Err(e) => tracing::warn!(
                    path = %file.display(),
                    error = %e,
                    "Failed to index file under changed directory"
                ),
            }
        }
        return ChangeOutcome {
            indexed,
            removed: 0,
        };
    }

    if !path.is_file() {
        // Vanished between the event and now; treat as a delete.
        return ChangeOutcome {
            indexed: 0,
            removed: remove_path(engine, path),
        };
    }
    if !is_eligible_file(path, config) {
        // A file that is not eligible (wrong extension, excluded) but was
        // previously indexed must be dropped rather than refreshed.
        return ChangeOutcome {
            indexed: 0,
            removed: usize::from(engine.remove_file(path)),
        };
    }
    match engine.update_file(path) {
        Ok(()) => ChangeOutcome {
            indexed: 1,
            removed: 0,
        },
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "Failed to update file in index");
            ChangeOutcome::default()
        }
    }
}

/// `max_file_size = 0` means "the default" everywhere (engine, watcher,
/// gRPC Index and initial discovery alike).
pub(crate) fn effective_max_size(config: &IndexerConfig) -> u64 {
    if config.max_file_size == 0 {
        crate::search::PartialIndexedFile::DEFAULT_MAX_FILE_SIZE
    } else {
        config.max_file_size
    }
}

/// The same eligibility rules discovery applies, for a single watcher path:
/// exclude patterns (glob), include extensions, binary extensions, size cap,
/// and the exact-path `exclude_files` list.
pub fn is_eligible_file(path: &Path, config: &IndexerConfig) -> bool {
    if config.is_file_excluded(path) {
        return false;
    }
    let discovery = FileDiscoveryConfig {
        paths: Vec::new(),
        exclude_patterns: config.exclude_patterns.clone(),
        include_extensions: config.include_extensions.clone(),
        max_file_size: Some(effective_max_size(config)),
        respect_gitignore: config.respect_gitignore,
        ..Default::default()
    };
    crate::search::file_discovery::is_eligible(path, &discovery)
}
