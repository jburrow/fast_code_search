//! File discovery utilities for walking directory trees and filtering files.
//!
//! Provides a unified file discovery mechanism used by both the keyword and semantic
//! search indexers. Handles exclude patterns, binary file detection, and large file filtering.

use crate::search::path_filter::PathFilter;
use crate::utils::{get_binary_extensions, has_binary_extension};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use walkdir::WalkDir;

/// Configuration for file discovery.
#[derive(Debug, Clone)]
pub struct FileDiscoveryConfig {
    /// Paths to search for files.
    pub paths: Vec<String>,

    /// Patterns to exclude (will be matched against path strings).
    /// Common patterns: "**/node_modules/**", "**/target/**", "**/.git/**"
    pub exclude_patterns: Vec<String>,

    /// File extensions to include (empty = all non-binary text files).
    /// When non-empty, only files with a matching extension are indexed.
    /// Example: `["rs", "py", "ts"]`
    pub include_extensions: Vec<String>,

    /// Maximum file size to include (in bytes). Files larger than this are skipped.
    /// Default is 10MB (10 * 1024 * 1024).
    pub max_file_size: Option<u64>,

    /// Additional binary extensions to skip (merged with default list).
    pub extra_binary_extensions: Vec<String>,

    /// Honour `.gitignore` / `.ignore` files (and `.git/info/exclude`) found
    /// under each root, in addition to `exclude_patterns`. Default `true`.
    pub respect_gitignore: bool,
}

impl Default for FileDiscoveryConfig {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            exclude_patterns: Vec::new(),
            include_extensions: Vec::new(),
            max_file_size: Some(10 * 1024 * 1024), // 10MB
            extra_binary_extensions: Vec::new(),
            respect_gitignore: true,
        }
    }
}

impl FileDiscoveryConfig {
    /// Create a config from indexer paths and exclude patterns.
    pub fn new(paths: Vec<String>, exclude_patterns: Vec<String>) -> Self {
        Self {
            paths,
            exclude_patterns,
            ..Default::default()
        }
    }

    /// Get the combined set of binary extensions (default + extra).
    fn get_all_binary_extensions(&self) -> HashSet<String> {
        let mut extensions: HashSet<String> = get_binary_extensions()
            .iter()
            .map(|s| s.to_string())
            .collect();

        for ext in &self.extra_binary_extensions {
            extensions.insert(ext.to_lowercase());
        }

        extensions
    }
}

/// One directory walk: plain `walkdir`, or the `ignore` crate's walker which
/// applies `.gitignore` rules as it descends.
enum Walker {
    Plain(walkdir::FilterEntry<walkdir::IntoIter, DirPredicate>),
    Ignore(ignore::Walk),
}

/// Decides whether a directory is descended into (see [`is_excluded_dir`]).
type DirPredicate = Box<dyn FnMut(&walkdir::DirEntry) -> bool + Send + 'static>;

/// Does `dir` match an exclude pattern as a directory? Patterns are written
/// for files (`**/node_modules/**`, `**/target/**`), so the directory is
/// probed both as itself and with a child appended: pruning it here means
/// `.git/objects`, `target/` and `node_modules/` are never read at all
/// instead of being walked, stat'ed and rejected file by file.
fn is_excluded_dir(filter: &PathFilter, dir: &Path) -> bool {
    let normalized = dir.to_string_lossy().replace('\\', "/");
    let probe = format!("{}/_", normalized.trim_end_matches('/'));
    !filter.matches(&normalized) || !filter.matches(&probe)
}

/// Iterator over discovered files matching the configuration criteria.
pub struct FileDiscoveryIterator {
    /// Stack of directory walkers (one per path).
    walkers: Vec<Walker>,

    /// The eligibility rules every discovered file is checked against.
    rules: EligibilityProbe,
}

/// The single eligibility rule set — exclude patterns, include-extension
/// whitelist, binary extensions, size cap, and `.gitignore` when enabled —
/// without any directory walk. Discovery applies it to every file it finds;
/// the watcher and the reload reconciliation apply it to single paths, so
/// the initial build and incremental updates can never disagree. Cheap to
/// share across threads; build once per config.
#[derive(Clone)]
pub struct EligibilityProbe {
    /// Whether single-path eligibility checks consult `.gitignore` files.
    respect_gitignore: bool,

    /// Ignore matchers per directory (nearest first, up to the `.git`
    /// root), built once per directory for the life of the probe. Checking
    /// 61k files through [`is_gitignored`] alone stat'ed every ancestor's
    /// `.git`, `.gitignore` and `.ignore` per file (about 15 s on reload);
    /// with the chain cached per directory a file costs one map lookup.
    ignore_chains: Arc<std::sync::RwLock<rustc_hash::FxHashMap<PathBuf, Arc<IgnoreChain>>>>,

    /// Whether each folder is ignored, as a walk decides it (an ignored
    /// folder is never entered), cached per folder like `ignore_chains`.
    dirs_ignored: Arc<std::sync::RwLock<rustc_hash::FxHashMap<PathBuf, bool>>>,

    /// The indexed roots (as given and canonical). A walk starts at a root
    /// and never checks the root itself against ignore files, so folder
    /// checks stop there too.
    roots: Vec<PathBuf>,

    /// Compiled exclude glob filter.
    exclude_filter: PathFilter,

    /// Allowed file extensions (lowercased, no leading dot; empty = allow all).
    include_extensions: Vec<String>,

    /// Binary extensions to skip.
    binary_extensions: HashSet<String>,

    /// Maximum file size (None = no limit).
    max_file_size: Option<u64>,
}

impl EligibilityProbe {
    /// Build the rules from `config`. `config.paths` are the indexed roots:
    /// folders above them are not checked against ignore files, as a walk
    /// from a root never checks them. Without roots, folders are checked up
    /// to the repository root (the first folder holding `.git`).
    pub fn new(config: &FileDiscoveryConfig) -> Self {
        // Build a PathFilter from the exclude patterns. Patterns are matched against
        // the full OS path of each discovered file so that patterns like
        // `**/node_modules/**` are evaluated with proper glob semantics instead of
        // the old substring-contains approach.
        let exclude_filter = PathFilter::new(&[], &config.exclude_patterns).unwrap_or_else(|e| {
            tracing::warn!("Invalid exclude pattern(s): {}; exclusions disabled", e);
            PathFilter::default()
        });
        Self {
            respect_gitignore: config.respect_gitignore,
            ignore_chains: Arc::new(std::sync::RwLock::new(Default::default())),
            dirs_ignored: Arc::new(std::sync::RwLock::new(Default::default())),
            roots: config
                .paths
                .iter()
                .flat_map(|p| {
                    let p = PathBuf::from(p);
                    let canonical = p.canonicalize().ok().filter(|c| *c != p);
                    std::iter::once(p).chain(canonical)
                })
                .collect(),
            exclude_filter,
            include_extensions: config
                .include_extensions
                .iter()
                .map(|e| e.trim_start_matches('.').to_lowercase())
                .collect(),
            binary_extensions: config.get_all_binary_extensions(),
            max_file_size: config.max_file_size,
        }
    }

    /// Would discovery index `path`? `known_size` avoids a stat when the
    /// caller already has it.
    pub fn is_eligible_path(&self, path: &Path, known_size: Option<u64>) -> bool {
        if self.respect_gitignore && self.is_gitignored_cached(path) {
            return false;
        }
        self.accepts_with_size(path, known_size)
    }

    /// Is folder `dir` ignored by the ignore files above it (`sub/` in a
    /// parent's `.gitignore`)? A walk never checks the folder it starts
    /// from, so the watcher asks this before reconciling a folder: it used
    /// to index everything in an ignored folder that was created, renamed
    /// in, or had an ignore file of its own written.
    pub fn is_dir_ignored(&self, dir: &Path) -> bool {
        self.respect_gitignore && self.dir_ignored(dir)
    }

    /// Is `path` ignored, as a walk decides it: every folder between the
    /// root and the file is checked as a folder, and an ignored folder is
    /// never entered, so a rule deeper down (`!a/b/` after `/a`) cannot
    /// bring a file back from it, as in git and ripgrep. Then the file
    /// itself, where the nearest ignore file with a rule for it decides.
    ///
    /// (This used to ask each ignore file about the file and its parents
    /// in one go, from the file upwards, so `!a/b/` let `a/b/x` in although
    /// the walk never entered `a`: the watcher indexed a file the build did
    /// not, until the next restart dropped it.)
    fn is_gitignored_cached(&self, path: &Path) -> bool {
        let Some(dir) = path.parent() else {
            return false;
        };
        self.dir_ignored(dir) || self.decides(path, false)
    }

    /// The nearest ignore file with a rule for `path` (a file or folder),
    /// looking only at `path` itself, not its parents.
    fn decides(&self, path: &Path, is_dir: bool) -> bool {
        let Some(parent) = path.parent() else {
            return false;
        };
        self.ignore_chain(parent).ignores(path, is_dir)
    }

    /// Is folder `dir`, or a folder above it below the walk's start, ignored?
    fn dir_ignored(&self, dir: &Path) -> bool {
        // Where a walk starts: an indexed root, or without one the
        // repository root. It is never checked itself.
        if self.roots.iter().any(|r| r == dir)
            || (self.roots.is_empty() && dir.join(".git").exists())
        {
            return false;
        }
        let Some(parent) = dir.parent() else {
            return false;
        };
        if let Ok(map) = self.dirs_ignored.read() {
            if let Some(&ignored) = map.get(dir) {
                return ignored;
            }
        }
        let under_root = self.roots.is_empty() || self.roots.iter().any(|r| dir.starts_with(r));
        let ignored = if under_root {
            self.dir_ignored(parent) || self.decides(dir, true)
        } else {
            // Outside every root (a path the watcher reports above them):
            // only the folder's own rule.
            self.decides(dir, true)
        };
        if let Ok(mut map) = self.dirs_ignored.write() {
            map.insert(dir.to_path_buf(), ignored);
        }
        ignored
    }

    /// The ignore files that apply to entries of `dir`: its own and every
    /// ancestor's (the walk reads its root's parents' too, and with
    /// `require_git(false)` does not stop at a repository's root), and the
    /// user's global excludes rooted at the nearest repository. Built
    /// recursively from the parent's chain so every directory is examined
    /// once.
    fn ignore_chain(&self, dir: &Path) -> Arc<IgnoreChain> {
        if let Ok(map) = self.ignore_chains.read() {
            if let Some(chain) = map.get(dir) {
                return chain.clone();
            }
        }
        let mut chain = IgnoreChain::default();
        chain.dirs.extend(ignores_for_dir(dir));
        let parent = dir.parent().map(|p| self.ignore_chain(p));
        if let Some(parent) = &parent {
            chain.dirs.extend(parent.dirs.iter().cloned());
        }
        chain.global = match parent {
            Some(parent) if !dir.join(".git").exists() => parent.global.clone(),
            _ => global_gitignore_for_repo(dir),
        };
        let chain = Arc::new(chain);
        if let Ok(mut map) = self.ignore_chains.write() {
            map.insert(dir.to_path_buf(), chain.clone());
        }
        chain
    }
}

impl FileDiscoveryIterator {
    /// Create a new file discovery iterator from the given configuration.
    pub fn new(config: &FileDiscoveryConfig) -> Self {
        let rules = EligibilityProbe::new(config);
        let dir_filter = Arc::new(rules.exclude_filter.clone());

        let walkers: Vec<Walker> = config
            .paths
            .iter()
            .filter_map(|path_str| {
                let path = Path::new(path_str);
                if !path.exists() {
                    tracing::warn!(path = %path_str, "Path does not exist, skipping");
                    return None;
                }
                // Do NOT follow symlinks: following them duplicates files when a
                // link points inside the same root (the same file indexed under
                // two paths -> duplicate search results) and can pull in trees
                // outside the requested roots. This matches the default of most
                // code-search tools (e.g. ripgrep).
                //
                // Excluded directories are pruned before they are read (the
                // root itself, depth 0, is always entered).
                Some(if config.respect_gitignore {
                    let filter = dir_filter.clone();
                    Walker::Ignore(
                        ignore::WalkBuilder::new(path)
                            .follow_links(false)
                            // Keep hidden files: exclusions are the user's call
                            // via exclude_patterns (".git" is in the defaults).
                            .hidden(false)
                            .git_ignore(true)
                            .git_exclude(true)
                            // The user's global excludes, as rg and git apply.
                            .git_global(true)
                            .ignore(true)
                            // As ripgrep: `.rgignore` rules take precedence
                            // over `.ignore` and `.gitignore`.
                            .add_custom_ignore_filename(".rgignore")
                            .parents(true)
                            // Honour .gitignore in trees that are not git
                            // repositories too, so discovery agrees with the
                            // watcher's per-path `is_gitignored` check.
                            .require_git(false)
                            .filter_entry(move |e| {
                                e.depth() == 0
                                    || !e.file_type().is_some_and(|t| t.is_dir())
                                    || !is_excluded_dir(&filter, e.path())
                            })
                            .build(),
                    )
                } else {
                    let filter = dir_filter.clone();
                    let predicate: DirPredicate = Box::new(move |e: &walkdir::DirEntry| {
                        e.depth() == 0
                            || !e.file_type().is_dir()
                            || !is_excluded_dir(&filter, e.path())
                    });
                    Walker::Plain(
                        WalkDir::new(path)
                            .follow_links(false)
                            .into_iter()
                            .filter_entry(predicate),
                    )
                })
            })
            .collect();

        Self { walkers, rules }
    }
}

impl EligibilityProbe {
    /// Check if a path matches any exclude pattern.
    fn is_excluded(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        // Use forward slashes so patterns authored on any OS work uniformly.
        let normalized = path_str.replace('\\', "/");
        // PathFilter::matches() returns true when the path is *not* excluded by
        // any exclude pattern (i.e. the file should be kept).  We negate it here
        // to satisfy the `is_excluded` contract expected by the call-site.
        !self.exclude_filter.matches(&normalized)
    }

    /// Check if a file has a binary extension.
    fn has_binary_ext(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            let ext = ext.to_string_lossy().to_lowercase();
            self.binary_extensions.contains(&ext)
        } else {
            false
        }
    }

    /// Check if a file exceeds the size limit.
    fn exceeds_size_limit(&self, path: &Path) -> bool {
        if let Some(max_size) = self.max_file_size {
            if let Ok(metadata) = path.metadata() {
                return metadata.len() > max_size;
            }
        }
        false
    }

    /// The single eligibility rule set: exclude patterns, include-extension
    /// whitelist, binary extensions, size cap. Used by the iterator for every
    /// discovered file and by [`is_eligible`] for watcher events, so the
    /// initial build and incremental updates can never disagree.
    fn accepts(&self, path: &Path) -> bool {
        self.accepts_with_size(path, None)
    }

    /// [`Self::accepts`] with the file size already known (skips the stat).
    fn accepts_with_size(&self, path: &Path, known_size: Option<u64>) -> bool {
        if self.is_excluded(path) {
            return false;
        }
        if !self.include_extensions.is_empty() {
            match path.extension() {
                Some(ext) => {
                    let ext_lower = ext.to_string_lossy().to_lowercase();
                    if !self.include_extensions.contains(&ext_lower) {
                        return false;
                    }
                }
                None => return false, // no extension → skip when filter is active
            }
        }
        if self.has_binary_ext(path) || has_binary_extension(path) {
            return false;
        }
        let too_large = match (known_size, self.max_file_size) {
            (Some(size), Some(max)) => size > max,
            _ => self.exceeds_size_limit(path),
        };
        if too_large {
            tracing::debug!(path = %path.display(), "Skipping file exceeding size limit");
            return false;
        }
        true
    }
}

impl Iterator for FileDiscoveryIterator {
    type Item = PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(walker) = self.walkers.last_mut() {
            // Try to get the next entry from the current walker
            let next: Option<Result<(PathBuf, bool), String>> = match walker {
                Walker::Plain(w) => w.next().map(|r| {
                    r.map(|e| (e.path().to_path_buf(), e.file_type().is_file()))
                        .map_err(|e| e.to_string())
                }),
                Walker::Ignore(w) => w.next().map(|r| {
                    r.map(|e| {
                        let is_file = e.file_type().is_some_and(|t| t.is_file());
                        (e.into_path(), is_file)
                    })
                    .map_err(|e| e.to_string())
                }),
            };
            match next {
                Some(Ok((path, is_file))) => {
                    // Skip non-files
                    if !is_file {
                        continue;
                    }
                    if !self.rules.accepts(&path) {
                        continue;
                    }
                    return Some(path);
                }
                Some(Err(e)) => {
                    tracing::debug!(error = %e, "Error walking directory");
                    continue;
                }
                None => {
                    // Current walker exhausted, move to next
                    self.walkers.pop();
                }
            }
        }

        None
    }
}

/// Would discovery with `config` index `path`? Same rules as
/// [`FileDiscoveryIterator`] (exclude patterns, include extensions, binary
/// extensions, size cap, `.gitignore`); `config.paths` is ignored. Build an
/// [`EligibilityProbe`] instead when checking many paths.
pub fn is_eligible(path: &Path, config: &FileDiscoveryConfig) -> bool {
    EligibilityProbe::new(config).is_eligible_path(path, None)
}

/// Is `path` ignored by the ignore files above it, as a walk from its
/// repository's root decides? See [`EligibilityProbe`], which caches the
/// matchers when checking many paths.
pub fn is_gitignored(path: &Path) -> bool {
    EligibilityProbe::new(&FileDiscoveryConfig::default()).is_gitignored_cached(path)
}

/// The user's global git excludes (`core.excludesFile`, by default
/// `~/.config/git/ignore`), rooted at the repository `root` so they match
/// its files. ripgrep and git apply them; `None` when there are none.
fn global_gitignore_for_repo(root: &Path) -> Option<Arc<ignore::gitignore::Gitignore>> {
    static CACHE: OnceLock<
        Mutex<std::collections::HashMap<PathBuf, Option<Arc<ignore::gitignore::Gitignore>>>>,
    > = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    if let Ok(guard) = cache.lock() {
        if let Some(gi) = guard.get(root) {
            return gi.clone();
        }
    }
    let (gi, _err) = ignore::gitignore::GitignoreBuilder::new(root).build_global();
    let gi = (!gi.is_empty()).then(|| Arc::new(gi));
    if let Ok(mut guard) = cache.lock() {
        guard.insert(root.to_path_buf(), gi.clone());
    }
    gi
}

/// One directory's ignore files, compiled, in [`IGNORE_FILES`] order.
type DirIgnores = [Option<ignore::gitignore::Gitignore>; 4];

/// The ignore files that apply in one directory (see
/// [`EligibilityProbe::ignore_chain`]).
#[derive(Default)]
struct IgnoreChain {
    /// Each directory's files, nearest first.
    dirs: Vec<Arc<DirIgnores>>,
    /// The user's global excludes, if any.
    global: Option<Arc<ignore::gitignore::Gitignore>>,
}

impl IgnoreChain {
    /// Do the ignore files ignore `path` (an entry of the chain's
    /// directory)? As ripgrep decides (the `ignore` crate's
    /// `matched_ignore`): one kind of ignore file at a time, in precedence
    /// order, the nearest file of that kind with a rule for `path` decides,
    /// so an `.ignore` rule two folders up beats a `.gitignore` rule next
    /// to the file; then the global excludes. Only `path` itself is
    /// matched, not its parents: a walk has already decided those.
    fn ignores(&self, path: &Path, is_dir: bool) -> bool {
        let global = self.global.as_deref();
        let kinds = (0..IGNORE_FILES.len())
            .flat_map(|k| self.dirs.iter().filter_map(move |d| d[k].as_ref()))
            .chain(global);
        for gi in kinds {
            match gi.matched(path, is_dir) {
                ignore::Match::Ignore(_) => return true,
                ignore::Match::Whitelist(_) => return false,
                ignore::Match::None => {}
            }
        }
        false
    }
}

/// Compiled ignore matchers per directory, keyed by the modification time
/// and size of each of that directory's ignore files (`.git/info/exclude`,
/// `.gitignore`, `.ignore`, `.rgignore`).
///
/// Compiling a matcher means parsing the file and building a regex per
/// pattern; doing that for every ancestor on every watcher event cost about
/// 10 ms per event on a repository with a typical root `.gitignore`. A
/// changed ignore file is picked up because its mtime and size are part of
/// the key.
type IgnoreKey = [Option<(std::time::SystemTime, u64)>; 4];

/// The kinds of ignore file, highest precedence first, as ripgrep ranks
/// them: `.rgignore` over `.ignore` over `.gitignore` over
/// `.git/info/exclude` (and git's own `.gitignore` over `info/exclude`).
const IGNORE_FILES: [&str; 4] = [".rgignore", ".ignore", ".gitignore", ".git/info/exclude"];
type IgnoreCache = std::collections::HashMap<PathBuf, (IgnoreKey, Arc<DirIgnores>)>;
static IGNORE_CACHE: OnceLock<Mutex<IgnoreCache>> = OnceLock::new();

fn ignores_for_dir(dir: &Path) -> Option<Arc<DirIgnores>> {
    // A repository root's `.git/info/exclude` applies like a `.gitignore`
    // there, as the initial walk (`git_exclude`) and git itself apply it:
    // without it the watcher indexed an excluded file once it changed, and
    // the next restart dropped it again.
    let key: IgnoreKey = IGNORE_FILES.map(|name| {
        std::fs::metadata(dir.join(name))
            .ok()
            .filter(|m| m.is_file())
            .and_then(|m| Some((m.modified().ok()?, m.len())))
    });
    if key.iter().all(Option::is_none) {
        return None;
    }
    let cache = IGNORE_CACHE.get_or_init(|| Mutex::new(IgnoreCache::new()));
    if let Ok(guard) = cache.lock() {
        if let Some((k, gi)) = guard.get(dir) {
            if *k == key {
                return Some(gi.clone());
            }
        }
    }
    let gi: Arc<DirIgnores> = Arc::new(std::array::from_fn(|k| {
        key[k].and_then(|_| {
            let mut builder = ignore::gitignore::GitignoreBuilder::new(dir);
            builder.add(dir.join(IGNORE_FILES[k]));
            builder.build().ok()
        })
    }));
    if let Ok(mut guard) = cache.lock() {
        guard.insert(dir.to_path_buf(), (key, gi.clone()));
    }
    Some(gi)
}

/// Convenience function to discover files from paths with exclude patterns.
pub fn discover_files(paths: &[String], exclude_patterns: &[String]) -> FileDiscoveryIterator {
    let config = FileDiscoveryConfig::new(paths.to_vec(), exclude_patterns.to_vec());
    FileDiscoveryIterator::new(&config)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Roadmap 2.8: `.gitignore` is honoured by discovery and by the single
    /// path check the watcher uses, and can be switched off.
    /// The per-directory matcher cache must notice an edited ignore file.
    #[test]
    fn test_gitignore_cache_follows_edits() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        let file = root.join("gen.rs");
        std::fs::write(&file, "x").unwrap();
        std::fs::write(root.join(".gitignore"), "gen.rs\n").unwrap();
        assert!(is_gitignored(&file));
        // Repeated lookups hit the cache and agree.
        assert!(is_gitignored(&file));
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(root.join(".gitignore"), "other.rs\n").unwrap();
        assert!(!is_gitignored(&file), "edited .gitignore must be re-read");
        std::fs::remove_file(root.join(".gitignore")).unwrap();
        assert!(!is_gitignored(&file));
    }

    /// `.rgignore` is honoured as ripgrep honours it, with ripgrep's
    /// precedence (`.rgignore` over `.ignore` over `.gitignore`), by the
    /// directory walk and by the per-path check the watcher uses alike.
    #[test]
    fn test_rgignore_and_ignore_file_precedence() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join("out")).unwrap();
        std::fs::write(root.join("out/o.py"), "x").unwrap();
        std::fs::write(root.join("a.py"), "x").unwrap();
        std::fs::write(root.join("b.py"), "x").unwrap();
        std::fs::write(root.join("c.py"), "x").unwrap();
        // .gitignore ignores a and b; .ignore lets b back; .rgignore ignores
        // out/ and c, and lets a back.
        std::fs::write(root.join(".gitignore"), "a.py\nb.py\n").unwrap();
        std::fs::write(root.join(".ignore"), "!b.py\n").unwrap();
        std::fs::write(root.join(".rgignore"), "out/\nc.py\n!a.py\n").unwrap();

        let cfg = FileDiscoveryConfig {
            paths: vec![root.to_string_lossy().to_string()],
            ..Default::default()
        };
        let mut found: Vec<String> = FileDiscoveryIterator::new(&cfg)
            .filter(|p| p.extension().is_some_and(|e| e == "py"))
            .map(|p| {
                p.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        found.sort();
        assert_eq!(found, vec!["a.py", "b.py"]);

        let probe = EligibilityProbe::new(&cfg);
        for (file, kept) in [
            ("a.py", true),
            ("b.py", true),
            ("c.py", false),
            ("out/o.py", false),
        ] {
            assert_eq!(
                probe.is_eligible_path(&root.join(file), None),
                kept,
                "{file}"
            );
            assert_eq!(is_gitignored(&root.join(file)), !kept, "{file}");
        }
    }

    /// The per-path check decides as the walk does, in the three cases the
    /// discovery oracle (tests/discovery_oracle.rs) found it did not: a rule
    /// cannot bring a file back from an ignored folder; an `.ignore` rule
    /// further up beats a `.gitignore` rule next to the file; and the rules
    /// above a nested repository still apply to it.
    #[test]
    fn test_per_path_check_agrees_with_the_walk() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path().canonicalize().unwrap();
        for (path, text) in [
            (".gitignore", "/a\n!a/b/\n*.log\n"),
            ("a/b/f.py", "x"),
            (".ignore", "c/**/f.py\n"),
            ("c/.gitignore", "!f.py\n"),
            ("c/f.py", "x"),
            ("c/g.py", "x"),
            ("nested/.git/HEAD", "x"),
            ("nested/x.log", "x"),
            ("nested/x.py", "x"),
        ] {
            std::fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
            std::fs::write(root.join(path), text).unwrap();
        }
        let cfg = FileDiscoveryConfig {
            paths: vec![root.to_string_lossy().to_string()],
            exclude_patterns: vec!["**/.git/**".into()],
            ..Default::default()
        };
        let mut found: Vec<String> = FileDiscoveryIterator::new(&cfg)
            .map(|p| {
                p.strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .filter(|p| p.ends_with(".py") || p.ends_with(".log"))
            .collect();
        found.sort();
        assert_eq!(found, vec!["c/g.py", "nested/x.py"]);

        let probe = EligibilityProbe::new(&cfg);
        for (file, kept) in [
            ("a/b/f.py", false),
            ("c/f.py", false),
            ("c/g.py", true),
            ("nested/x.log", false),
            ("nested/x.py", true),
        ] {
            assert_eq!(
                probe.is_eligible_path(&root.join(file), None),
                kept,
                "{file}"
            );
        }
        assert!(probe.is_dir_ignored(&root.join("a/b")));
        assert!(!probe.is_dir_ignored(&root.join("c")));
    }

    #[test]
    fn test_gitignore_is_respected_and_optional() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join("build")).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join(".gitignore"), "build/\n*.log\n!keep.log\n").unwrap();
        std::fs::write(root.join("build/gen.rs"), "fn gen() {}").unwrap();
        std::fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        std::fs::write(root.join("src/debug.log"), "noise").unwrap();
        std::fs::write(root.join("src/keep.log"), "kept").unwrap();

        let names = |cfg: &FileDiscoveryConfig| -> Vec<String> {
            let mut v: Vec<String> = FileDiscoveryIterator::new(cfg)
                .map(|p| {
                    p.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/")
                })
                .collect();
            v.sort();
            v
        };
        let on = FileDiscoveryConfig {
            paths: vec![root.to_string_lossy().to_string()],
            ..Default::default()
        };
        assert_eq!(
            names(&on),
            vec![".gitignore", "src/keep.log", "src/main.rs"]
        );

        let off = FileDiscoveryConfig {
            respect_gitignore: false,
            ..on.clone()
        };
        assert_eq!(
            names(&off),
            vec![
                ".gitignore",
                "build/gen.rs",
                "src/debug.log",
                "src/keep.log",
                "src/main.rs"
            ]
        );

        // Single-path checks (watcher) agree with the walk.
        assert!(!is_eligible(&root.join("build/gen.rs"), &on));
        assert!(!is_eligible(&root.join("src/debug.log"), &on));
        assert!(is_eligible(&root.join("src/keep.log"), &on));
        assert!(is_eligible(&root.join("src/main.rs"), &on));
        assert!(is_eligible(&root.join("build/gen.rs"), &off));
    }
    use std::fs;
    use tempfile::TempDir;

    fn create_test_files(dir: &TempDir) -> Vec<PathBuf> {
        let files = vec![
            dir.path().join("src/main.rs"),
            dir.path().join("src/lib.rs"),
            dir.path().join("tests/test.rs"),
            dir.path().join("node_modules/pkg/index.js"),
            dir.path().join("target/debug/binary"),
            dir.path().join("image.png"),
            dir.path().join("README.md"),
        ];

        for file in &files {
            if let Some(parent) = file.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(file, "test content").unwrap();
        }

        files
    }

    #[test]
    fn test_file_discovery_basic() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(&temp_dir);

        let config = FileDiscoveryConfig::new(
            vec![temp_dir.path().to_string_lossy().to_string()],
            vec!["**/node_modules/**".to_string(), "**/target/**".to_string()],
        );

        let discovered: Vec<PathBuf> = FileDiscoveryIterator::new(&config).collect();

        // Should find: main.rs, lib.rs, test.rs, README.md
        // Should NOT find: node_modules/*, target/*, image.png
        assert_eq!(discovered.len(), 4);

        let names: Vec<_> = discovered
            .iter()
            .filter_map(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .collect();

        assert!(names.contains(&"main.rs".to_string()));
        assert!(names.contains(&"lib.rs".to_string()));
        assert!(names.contains(&"test.rs".to_string()));
        assert!(names.contains(&"README.md".to_string()));
        assert!(!names.contains(&"index.js".to_string())); // excluded by node_modules
        assert!(!names.contains(&"image.png".to_string())); // binary extension
    }

    #[test]
    fn test_file_discovery_skips_binary_extensions() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("code.rs"), "fn main() {}").unwrap();
        fs::write(temp_dir.path().join("image.png"), "binary").unwrap();
        fs::write(temp_dir.path().join("archive.zip"), "binary").unwrap();

        let config =
            FileDiscoveryConfig::new(vec![temp_dir.path().to_string_lossy().to_string()], vec![]);

        let discovered: Vec<PathBuf> = FileDiscoveryIterator::new(&config).collect();

        assert_eq!(discovered.len(), 1);
        assert!(discovered[0].to_string_lossy().contains("code.rs"));
    }

    #[test]
    fn test_file_discovery_nonexistent_path() {
        let config = FileDiscoveryConfig::new(
            vec!["/nonexistent/path/that/does/not/exist".to_string()],
            vec![],
        );

        let discovered: Vec<PathBuf> = FileDiscoveryIterator::new(&config).collect();
        assert!(discovered.is_empty());
    }
}

#[cfg(test)]
mod prune_tests {
    use super::*;

    fn names(root: &Path, cfg: &FileDiscoveryConfig) -> Vec<String> {
        let mut v: Vec<String> = FileDiscoveryIterator::new(cfg)
            .map(|p| {
                p.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        v.sort();
        v
    }

    /// Excluded directories are pruned (not descended into) by both walkers,
    /// while a root whose own name matches a pattern is still entered.
    #[test]
    fn test_excluded_directories_are_pruned() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        for d in ["node_modules/pkg/deep", "target/debug", "build", "src"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        std::fs::write(root.join("node_modules/pkg/deep/x.js"), "x").unwrap();
        std::fs::write(root.join("target/debug/y.rs"), "y").unwrap();
        std::fs::write(root.join("build/z.rs"), "z").unwrap();
        std::fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

        let cfg = FileDiscoveryConfig {
            paths: vec![root.to_string_lossy().to_string()],
            exclude_patterns: vec![
                "**/node_modules/**".to_string(),
                "**/target/**".to_string(),
                "**/build".to_string(),
            ],
            ..Default::default()
        };
        assert_eq!(names(root, &cfg), vec!["src/main.rs"]);
        let plain = FileDiscoveryConfig {
            respect_gitignore: false,
            ..cfg.clone()
        };
        assert_eq!(names(root, &plain), vec!["src/main.rs"]);

        // The predicate itself: directories match as themselves or via a child.
        let filter = PathFilter::new(&[], &cfg.exclude_patterns).unwrap();
        assert!(is_excluded_dir(&filter, &root.join("node_modules")));
        assert!(is_excluded_dir(&filter, &root.join("target")));
        assert!(is_excluded_dir(&filter, &root.join("build")));
        assert!(!is_excluded_dir(&filter, &root.join("src")));

        // A root that is itself named like an excluded directory is entered.
        let sub_root = root.join("build");
        let cfg2 = FileDiscoveryConfig {
            paths: vec![sub_root.to_string_lossy().to_string()],
            exclude_patterns: vec!["**/build".to_string()],
            ..Default::default()
        };
        assert_eq!(names(&sub_root, &cfg2), vec!["z.rs"]);
    }

    /// Discovery honours `.gitignore` in a tree that is not a git repository,
    /// so it agrees with the per-path check the watcher uses.
    #[test]
    fn test_gitignore_applies_outside_git_repos() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::create_dir_all(root.join("build")).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join(".gitignore"), "build/\n").unwrap();
        std::fs::write(root.join("build/gen.rs"), "fn gen() {}").unwrap();
        std::fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

        let cfg = FileDiscoveryConfig {
            paths: vec![root.to_string_lossy().to_string()],
            ..Default::default()
        };
        assert_eq!(names(root, &cfg), vec![".gitignore", "src/main.rs"]);
        assert!(!is_eligible(&root.join("build/gen.rs"), &cfg));
    }

    /// `include_extensions` accepts `".rs"` as well as `"rs"`, and the probe
    /// can use a known size instead of a stat.
    #[test]
    fn test_probe_extensions_and_known_size() {
        let cfg = FileDiscoveryConfig {
            include_extensions: vec![".RS".to_string()],
            max_file_size: Some(10),
            respect_gitignore: false,
            ..Default::default()
        };
        let probe = EligibilityProbe::new(&cfg);
        assert!(probe.is_eligible_path(Path::new("/nowhere/a.rs"), Some(5)));
        assert!(!probe.is_eligible_path(Path::new("/nowhere/a.rs"), Some(11)));
        assert!(!probe.is_eligible_path(Path::new("/nowhere/a.py"), Some(5)));
    }
}
