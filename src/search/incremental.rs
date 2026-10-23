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
use crate::search::file_discovery::{EligibilityProbe, FileDiscoveryConfig, FileDiscoveryIterator};
use crate::search::watcher::FileChange;
use crate::search::SearchEngine;
use rayon::prelude::*;
use std::path::{Path, PathBuf};

/// What applying a change did, for logging and save accounting.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ChangeOutcome {
    /// Files (re)indexed.
    pub indexed: usize,
    /// Files removed from the index.
    pub removed: usize,
}

impl std::ops::AddAssign for ChangeOutcome {
    fn add_assign(&mut self, other: Self) {
        self.indexed += other.indexed;
        self.removed += other.removed;
    }
}

impl ChangeOutcome {
    /// True if the index changed at all.
    pub fn changed(&self) -> bool {
        self.indexed > 0 || self.removed > 0
    }
}

/// A burst of changes worked out against the index and the disk, ready to
/// be parsed and applied. See [`plan_changes`].
#[derive(Debug, Default)]
pub struct ChangePlan {
    /// Ids to drop: deleted files and directories, files that became
    /// ineligible, and files in a changed directory that are gone.
    pub(crate) remove: Vec<u32>,
    /// Files to read and (re)index.
    pub(crate) index: Vec<PathBuf>,
}

impl ChangePlan {
    /// Files the plan will read and (re)index.
    pub fn files_to_index(&self) -> &[PathBuf] {
        &self.index
    }
}

/// Apply a burst of changes in one go (plan, parse, install).
///
/// The watch loop runs the same three steps with the lock held only where
/// each needs it ([`plan_changes`] under a read lock, parsing under none,
/// [`apply_plan`] under the write lock), so searches keep being answered
/// while a burst is read from disk.
pub fn apply_changes(
    engine: &mut SearchEngine,
    changes: &[FileChange],
    config: &IndexerConfig,
) -> ChangeOutcome {
    let plan = plan_changes(engine, changes, config);
    let parsed = parse_planned(engine.file_parser(), &plan);
    apply_plan(engine, plan, parsed)
}

/// Work out what a burst of changes means for the index, touching only the
/// disk (stats, directory walks) and reading the engine, so it can run under
/// a read lock while searches go on.
///
/// Events are coalesced per path (the last event for a path wins; a rename
/// is a delete of `from` plus a modify of `to`). A directory hint is
/// reconciled with the disk; a path that vanished, or is no longer
/// eligible, is removed; and because a backend may drop the delete /
/// rename-away half of what happened (FSEvents does), the siblings of every
/// changed path are checked and entries whose file is gone are removed.
pub fn plan_changes(
    engine: &SearchEngine,
    changes: &[FileChange],
    config: &IndexerConfig,
) -> ChangePlan {
    use std::collections::HashMap;

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

    // Each path is planned on its own (stats, directory walks), in
    // parallel; removals are applied before additions whatever the order.
    let probe = EligibilityProbe::new(&discovery_config(config, Vec::new()));
    let parts: Vec<(ChangePlan, Option<PathBuf>)> = order
        .par_iter()
        .map(|path| {
            crate::diagnostics::capture::watcher_progress(1);
            let mut part = ChangePlan::default();
            let is_dir = path.is_dir();
            match ops[path] {
                Op::Delete => part.remove.extend(path_ids(engine, path)),
                Op::Modify => plan_path(engine, path, is_dir, config, &probe, &mut part),
            }
            let dir = if is_dir {
                Some(path.clone())
            } else {
                path.parent().map(Path::to_path_buf)
            };
            (part, dir)
        })
        .collect();

    let mut plan = ChangePlan::default();
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut seen_dirs = std::collections::HashSet::new();
    for (part, dir) in parts {
        plan.remove.extend(part.remove);
        plan.index.extend(part.index);
        if let Some(dir) = dir {
            if seen_dirs.insert(dir.clone()) {
                dirs.push(dir);
            }
        }
    }
    plan.remove.extend(engine.vanished_in(&dirs));
    plan
}

/// Read and parse the files a plan will index, in parallel. Needs no lock.
pub fn parse_planned(
    parser: crate::search::engine::FileParser,
    plan: &ChangePlan,
) -> Vec<(PathBuf, Option<crate::search::PreIndexedFile>)> {
    plan.index
        .par_iter()
        .map(|p| {
            crate::diagnostics::capture::watcher_progress(1);
            (p.clone(), parser.parse(p))
        })
        .collect()
}

/// Apply a plan and its parsed files under the caller's write lock: the
/// removals in one pass over the trigram index, then the parsed files in
/// another, then import resolution for everything the batch touched.
pub fn apply_plan(
    engine: &mut SearchEngine,
    plan: ChangePlan,
    parsed: Vec<(PathBuf, Option<crate::search::PreIndexedFile>)>,
) -> ChangeOutcome {
    let mut outcome = ChangeOutcome {
        indexed: 0,
        removed: engine.remove_files_by_ids(&plan.remove),
    };
    let (indexed, removed) = engine.install_parsed(parsed);
    outcome.indexed += indexed;
    outcome.removed += removed;
    // Files that imported a removed file resolve their imports again (an
    // import may now fall back to the package, or stay unresolved).
    if outcome.removed > 0 {
        engine.resolve_imports_incremental();
    }
    if outcome.changed() {
        engine.updated_at_ms = Some(crate::search::unix_now_ms());
    }
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

/// The index entries `path` stands for: a single file, or — when no file
/// matches exactly (the path was a directory, or is already gone) — every
/// file under it.
fn path_ids(engine: &SearchEngine, path: &Path) -> Vec<u32> {
    match engine.find_file_id_exact(path) {
        Some(id) => vec![id],
        None => engine.file_ids_under(path),
    }
}

/// Add what a modified path needs to `plan`: the files to (re)index under
/// it, and the index entries to drop because the path is gone or no longer
/// eligible.
fn plan_path(
    engine: &SearchEngine,
    path: &Path,
    is_dir: bool,
    config: &IndexerConfig,
    probe: &EligibilityProbe,
    plan: &mut ChangePlan,
) {
    if is_dir {
        return reconcile_dir(engine, path, config, plan);
    }
    if !path.is_file() {
        // Vanished between the event and now; treat as a delete.
        plan.remove.extend(path_ids(engine, path));
        return;
    }
    if config.is_file_excluded(path) || !probe.is_eligible_path(path, None) {
        // A file that is not eligible (wrong extension, excluded) but was
        // previously indexed must be dropped rather than refreshed.
        plan.remove.extend(engine.find_file_id_exact(path));
        return;
    }
    plan.index.push(path.to_path_buf());
}

/// Bring everything under directory `dir` in line with the disk: index
/// eligible files that are new or changed (by size and mtime), and drop
/// indexed files under it that are gone or no longer eligible.
///
/// A directory hint means "something under here may have changed": it was
/// created, renamed in, or events for it were lost. Files whose size and
/// mtime match what was indexed are left alone, so reconciling a large
/// unchanged tree costs a walk and a stat per file, not a re-index.
fn reconcile_dir(engine: &SearchEngine, dir: &Path, config: &IndexerConfig, plan: &mut ChangePlan) {
    let discovery = discovery_config(config, vec![dir.to_string_lossy().to_string()]);
    let mut keep = std::collections::HashSet::new();
    for file in FileDiscoveryIterator::new(&discovery).filter(|f| !config.is_file_excluded(f)) {
        match engine.find_file_id_exact(&file) {
            Some(id) => {
                keep.insert(id);
                if !engine.is_unchanged_on_disk(id, &file) {
                    plan.index.push(file);
                }
            }
            None => plan.index.push(file),
        }
    }
    plan.remove.extend(
        engine
            .file_ids_under(dir)
            .into_iter()
            .filter(|id| !keep.contains(id)),
    );
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
    crate::search::file_discovery::is_eligible(path, &discovery_config(config, Vec::new()))
}

/// Discovery settings for `paths` with the indexer's eligibility rules.
fn discovery_config(config: &IndexerConfig, paths: Vec<String>) -> FileDiscoveryConfig {
    FileDiscoveryConfig {
        paths,
        exclude_patterns: config.exclude_patterns.clone(),
        include_extensions: config.include_extensions.clone(),
        max_file_size: Some(effective_max_size(config)),
        respect_gitignore: config.respect_gitignore,
        ..Default::default()
    }
}
