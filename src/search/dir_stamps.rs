//! Folder stamps: what a restart needs to re-read only the folders that
//! changed since the last complete walk, the way git's untracked cache
//! does.
//!
//! A complete walk records, for every folder it entered, the folder's
//! modification time and a fingerprint of its ignore files. Creating,
//! deleting or renaming an entry changes a folder's modification time, so
//! on restart a folder whose time and ignore files are unchanged holds the
//! same entries: none of them can be a new file, and its files were
//! already checked one by one by the load's stat pass. Only changed
//! folders are read again (one level, since each subfolder has its own
//! stamp), new folders are walked whole, and a folder whose ignore files
//! changed is walked whole too, since its rules apply below it.
//!
//! Stamps are not trusted when they could be stale:
//! - a folder modified within [`RACY_NS`] of the walk could change again in
//!   the same clock tick, so it is not recorded, and its parents are marked
//!   to be read again so the walk reaches it;
//! - a folder the walk could not read is handled the same way;
//! - a change to the user's global excludes file (`core.excludesFile`), or
//!   to the configuration (the caller checks the fingerprint), means a full
//!   walk.

use crate::index::{DirStamp, DirStamps};
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A folder or ignore file changed this close to the walk is not trusted:
/// coarse file-system clocks (2 s on FAT, 1 s on some network shares) can
/// give a later change the same time.
const RACY_NS: u64 = 2_000_000_000;

/// The ignore files whose rules apply in and below a folder (as discovery
/// reads them), plus `.git` itself, whose appearance starts a repository
/// (and with it `.git/info/exclude` and the global excludes).
const STAMPED: [&str; 5] = [
    ".rgignore",
    ".ignore",
    ".gitignore",
    ".git/info/exclude",
    ".git",
];

/// Tests only: walks count as starting this much later than they do, so
/// folders a test wrote moments before count as settled (a test cannot
/// wait out [`RACY_NS`] at every step). Never set outside tests.
#[doc(hidden)]
pub static TEST_WALK_DELAY_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub(crate) fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
}

fn mtime_ns(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos() as u64)
}

/// Fingerprint of `dir`'s ignore files (which exist, their times and
/// sizes), and the newest of their times.
fn ignore_stamp(dir: &Path) -> (u64, u64) {
    let mut hasher = rustc_hash::FxHasher::default();
    let mut newest = 0;
    for (k, name) in STAMPED.iter().enumerate() {
        if let Ok(meta) = std::fs::metadata(dir.join(name)) {
            let t = mtime_ns(&meta);
            newest = newest.max(t);
            (k, t, meta.len()).hash(&mut hasher);
        }
    }
    (hasher.finish(), newest)
}

/// Fingerprint of the user's global excludes file: where it is, and its
/// time and size.
pub(crate) fn global_stamp() -> u64 {
    let mut hasher = rustc_hash::FxHasher::default();
    if let Some(path) = ignore::gitignore::gitconfig_excludes_path() {
        path.hash(&mut hasher);
        if let Ok(meta) = std::fs::metadata(&path) {
            (mtime_ns(&meta), meta.len()).hash(&mut hasher);
        }
    }
    hasher.finish()
}

/// Collects stamps while a walk runs.
pub(crate) struct StampRecorder {
    started_ns: u64,
    stamps: FxHashMap<PathBuf, DirStamp>,
    /// Folders that could not be stamped (too recent, unreadable): their
    /// parents must be read again on restart.
    untrusted: Vec<PathBuf>,
}

impl StampRecorder {
    pub(crate) fn new() -> Self {
        Self {
            started_ns: now_ns() + TEST_WALK_DELAY_NS.load(std::sync::atomic::Ordering::Relaxed),
            stamps: FxHashMap::default(),
            untrusted: Vec::new(),
        }
    }

    /// A folder the walk entered.
    pub(crate) fn record_dir(&mut self, dir: &Path) {
        let Ok(meta) = std::fs::symlink_metadata(dir) else {
            self.untrusted.push(dir.to_path_buf());
            return;
        };
        let mtime = mtime_ns(&meta);
        let (ignores, newest) = ignore_stamp(dir);
        let cutoff = self.started_ns.saturating_sub(RACY_NS);
        if mtime == 0 || mtime >= cutoff || newest >= cutoff {
            self.untrusted.push(dir.to_path_buf());
            return;
        }
        self.stamps.insert(
            dir.to_path_buf(),
            DirStamp {
                path: dir.to_path_buf(),
                mtime_ns: mtime,
                ignores,
            },
        );
    }

    /// A folder (or an entry in one) the walk could not read.
    pub(crate) fn unreadable(&mut self, path: &Path) {
        self.untrusted.push(path.to_path_buf());
    }

    /// The stamps, merged over `kept` (stamps still valid from before a
    /// restart walk). An untrusted folder is dropped, and every folder above
    /// it is marked to be read again, so a restart walk reaches it.
    pub(crate) fn finish(self, kept: FxHashMap<PathBuf, DirStamp>) -> DirStamps {
        let mut stamps = kept;
        stamps.extend(self.stamps);
        for path in &self.untrusted {
            stamps.remove(path);
            for above in path.ancestors().skip(1) {
                if let Some(stamp) = stamps.get_mut(above) {
                    stamp.mtime_ns = 0;
                }
            }
        }
        let mut dirs: Vec<DirStamp> = stamps.into_values().collect();
        dirs.sort_by(|a, b| a.path.cmp(&b.path));
        DirStamps {
            global: global_stamp(),
            dirs,
        }
    }
}

/// What a restart walk reads, from the saved stamps.
#[derive(Debug, Default)]
pub(crate) struct RestartPlan {
    /// Folders whose entries changed: read one level, skipping `known`
    /// subfolders.
    pub changed: Vec<PathBuf>,
    /// Folders whose ignore files changed: walked whole.
    pub subtrees: Vec<PathBuf>,
    /// Saved folders that still exist, outside `subtrees`: a walk never
    /// enters them (their own stamps say whether to).
    pub known: Arc<FxHashSet<PathBuf>>,
    /// The stamps still valid, kept for the next save.
    pub kept: FxHashMap<PathBuf, DirStamp>,
    /// How many saved folders were checked.
    pub checked: usize,
}

/// The restart walk for `roots` from `saved`, or `None` when a full walk is
/// needed: a root without a stamp (never walked completely), or a changed
/// global excludes file.
pub(crate) fn plan_restart(saved: &DirStamps, roots: &[PathBuf]) -> Option<RestartPlan> {
    if saved.global != global_stamp() {
        tracing::info!("Global git excludes changed since the last walk; walking every folder");
        return None;
    }
    let by_path: FxHashMap<&Path, &DirStamp> =
        saved.dirs.iter().map(|d| (d.path.as_path(), d)).collect();
    if roots.iter().any(|r| !by_path.contains_key(r.as_path())) {
        return None;
    }

    #[derive(PartialEq)]
    enum State {
        Same,
        Changed,
        Ignores,
        Gone,
    }
    let states: Vec<State> = saved
        .dirs
        .par_iter()
        .map(|d| match std::fs::symlink_metadata(&d.path) {
            Ok(meta) if meta.is_dir() => {
                if ignore_stamp(&d.path).0 != d.ignores {
                    State::Ignores
                } else if d.mtime_ns == 0 || mtime_ns(&meta) != d.mtime_ns {
                    State::Changed
                } else {
                    State::Same
                }
            }
            _ => State::Gone,
        })
        .collect();

    let mut subtrees: Vec<PathBuf> = saved
        .dirs
        .iter()
        .zip(&states)
        .filter(|(_, s)| **s == State::Ignores)
        .map(|(d, _)| d.path.clone())
        .collect();
    // Saved stamps are sorted by path, so a folder comes before those below
    // it: keep only the outermost.
    subtrees.sort();
    let mut outermost: Vec<PathBuf> = Vec::new();
    for dir in subtrees {
        if !outermost.last().is_some_and(|top| dir.starts_with(top)) {
            outermost.push(dir);
        }
    }
    let inside = |p: &Path| outermost.iter().any(|top| p.starts_with(top));

    let mut plan = RestartPlan {
        checked: saved.dirs.len(),
        ..Default::default()
    };
    let mut known = FxHashSet::default();
    for (d, state) in saved.dirs.iter().zip(states) {
        if state == State::Gone || inside(&d.path) {
            continue;
        }
        known.insert(d.path.clone());
        match state {
            State::Same => {
                plan.kept.insert(d.path.clone(), d.clone());
            }
            State::Changed => plan.changed.push(d.path.clone()),
            State::Ignores | State::Gone => {}
        }
    }
    plan.subtrees = outermost;
    plan.known = Arc::new(known);
    Some(plan)
}

/// Benchmarks only: plan and run a restart walk, returning how many files
/// it yielded (`None` when a full walk would be needed).
#[doc(hidden)]
pub fn restart_walk_for_bench(
    config: &crate::search::FileDiscoveryConfig,
    saved: &DirStamps,
    roots: &[PathBuf],
) -> Option<usize> {
    let plan = plan_restart(saved, roots)?;
    Some(crate::search::FileDiscoveryIterator::for_restart(config, plan).count())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::{FileDiscoveryConfig, FileDiscoveryIterator};
    use std::collections::BTreeSet;

    /// A recorder whose walk counts as starting 10 s from now, so the
    /// folders a test just made count as settled.
    fn settled() -> StampRecorder {
        StampRecorder {
            started_ns: now_ns() + 10_000_000_000,
            ..StampRecorder::new()
        }
    }

    fn config(root: &Path) -> FileDiscoveryConfig {
        FileDiscoveryConfig {
            paths: vec![root.to_string_lossy().into_owned()],
            exclude_patterns: vec!["**/node_modules/**".to_string()],
            ..Default::default()
        }
    }

    fn full_walk(root: &Path) -> (BTreeSet<PathBuf>, DirStamps) {
        let mut walk = FileDiscoveryIterator::new(&config(root));
        walk.set_recorder(settled());
        let files = (&mut walk).collect();
        (files, walk.take_stamps().unwrap())
    }

    fn restart_walk(root: &Path, saved: &DirStamps) -> (BTreeSet<PathBuf>, DirStamps) {
        let plan = plan_restart(saved, &[root.to_path_buf()]).expect("a plan");
        let mut walk = FileDiscoveryIterator::for_restart(&config(root), plan);
        walk.set_recorder(settled());
        let files = (&mut walk).collect();
        (files, walk.take_stamps().unwrap())
    }

    /// Lets a folder's next change get a different time on file systems
    /// with a coarse clock.
    fn tick() {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    fn tree() -> (tempfile::TempDir, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        for f in [
            "a/x.rs",
            "a/b/y.rs",
            "c/z.rs",
            "node_modules/m.js",
            "top.rs",
        ] {
            let p = root.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "fn f() {}\n").unwrap();
        }
        (temp, root)
    }

    #[test]
    fn a_full_walk_stamps_every_folder_it_enters() {
        let (_t, root) = tree();
        let (files, stamps) = full_walk(&root);
        assert_eq!(files.len(), 4);
        let dirs: Vec<_> = stamps.dirs.iter().map(|d| d.path.clone()).collect();
        assert_eq!(
            dirs,
            vec![
                root.clone(),
                root.join("a"),
                root.join("a/b"),
                root.join("c")
            ],
            "excluded folders are never entered, so not stamped"
        );
        assert_eq!(stamps.global, global_stamp());
    }

    #[test]
    fn a_restart_reads_only_changed_folders() {
        let (_t, root) = tree();
        let (_, saved) = full_walk(&root);

        // Nothing changed: nothing is read, every stamp is kept.
        let (files, stamps) = restart_walk(&root, &saved);
        assert!(files.is_empty(), "{files:?}");
        assert_eq!(stamps, saved);

        // A new file in a/b and a new folder with a nested file under c.
        tick();
        std::fs::write(root.join("a/b/new.rs"), "x\n").unwrap();
        std::fs::create_dir_all(root.join("c/d/e")).unwrap();
        std::fs::write(root.join("c/d/e/deep.rs"), "x\n").unwrap();
        let plan = plan_restart(&saved, std::slice::from_ref(&root)).unwrap();
        let mut changed = plan.changed.clone();
        changed.sort();
        assert_eq!(changed, vec![root.join("a/b"), root.join("c")]);
        assert!(plan.subtrees.is_empty());
        let (files, stamps) = restart_walk(&root, &saved);
        // A restart walk yields the files of the folders it reads; the
        // indexer skips the ones already indexed.
        assert_eq!(
            files,
            BTreeSet::from([
                root.join("a/b/new.rs"),
                root.join("a/b/y.rs"),
                root.join("c/d/e/deep.rs"),
                root.join("c/z.rs"),
            ])
        );
        assert!(!files.contains(&root.join("a/x.rs")), "a/ did not change");
        // The new folders are stamped now, so the next restart reads nothing.
        assert!(stamps.dirs.iter().any(|d| d.path == root.join("c/d/e")));
        let (files, _) = restart_walk(&root, &stamps);
        assert!(files.is_empty(), "{files:?}");
    }

    #[test]
    fn a_changed_ignore_file_walks_its_folder_whole() {
        let (_t, root) = tree();
        std::fs::write(root.join("a/.gitignore"), "b/\n").unwrap();
        let (files, saved) = full_walk(&root);
        assert!(!files.contains(&root.join("a/b/y.rs")));
        assert!(saved.dirs.iter().all(|d| d.path != root.join("a/b")));

        // Edited in place: a/'s own time may not move, its ignore file's does.
        tick();
        std::fs::write(root.join("a/.gitignore"), "nothing\n").unwrap();
        let plan = plan_restart(&saved, std::slice::from_ref(&root)).unwrap();
        assert_eq!(plan.subtrees, vec![root.join("a")]);
        let (files, _) = restart_walk(&root, &saved);
        assert!(files.contains(&root.join("a/b/y.rs")), "{files:?}");
    }

    #[test]
    fn folders_too_recent_to_trust_are_read_again() {
        let (_t, root) = tree();
        // A walk starting now: every folder was just written.
        let mut walk = FileDiscoveryIterator::new(&config(&root)).recording_stamps();
        (&mut walk).for_each(drop);
        let stamps = walk.take_stamps().unwrap();
        assert!(stamps.dirs.is_empty(), "{:?}", stamps.dirs);
        // No root stamp: the next start walks everything.
        assert!(plan_restart(&stamps, std::slice::from_ref(&root)).is_none());

        // One recent folder below settled ones: it is dropped and the
        // folders above it are marked to be read again.
        let mut recorder = settled();
        for dir in [root.clone(), root.join("a"), root.join("c")] {
            recorder.record_dir(&dir);
        }
        recorder.unreadable(&root.join("a/b"));
        let stamps = recorder.finish(FxHashMap::default());
        let time = |p: PathBuf| stamps.dirs.iter().find(|d| d.path == p).map(|d| d.mtime_ns);
        assert_eq!(time(root.clone()), Some(0));
        assert_eq!(time(root.join("a")), Some(0));
        assert!(time(root.join("c")).unwrap() > 0);
        let plan = plan_restart(&stamps, std::slice::from_ref(&root)).unwrap();
        let mut changed = plan.changed.clone();
        changed.sort();
        assert_eq!(changed, vec![root.clone(), root.join("a")]);
    }

    #[test]
    fn a_full_walk_is_needed_when_the_global_excludes_change() {
        let (_t, root) = tree();
        let (_, mut saved) = full_walk(&root);
        assert!(plan_restart(&saved, std::slice::from_ref(&root)).is_some());
        saved.global ^= 1;
        assert!(plan_restart(&saved, std::slice::from_ref(&root)).is_none());
    }

    #[test]
    fn a_cut_short_walk_gives_no_stamps() {
        let (_t, root) = tree();
        let mut walk = FileDiscoveryIterator::new(&config(&root));
        walk.set_recorder(settled());
        walk.next();
        assert!(walk.take_stamps().is_none());
    }
}
