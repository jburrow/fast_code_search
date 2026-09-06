//! Dependency tracking module for Fast Code Search
//!
//! Tracks import relationships between files to enable dependency-based
//! ranking of search results. Files that are imported by many other files
//! receive a ranking boost.

use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Tracks import/dependency relationships between files in the index.
///
/// Maintains bidirectional mappings:
/// - `imports`: file_id -> set of file_ids it imports
/// - `imported_by`: file_id -> set of file_ids that import it
#[derive(Debug, Default)]
pub struct DependencyIndex {
    /// Map from file_id to the set of file_ids it imports
    imports: FxHashMap<u32, FxHashSet<u32>>,
    /// Reverse index: file_id -> files that import it
    imported_by: FxHashMap<u32, FxHashSet<u32>>,
    /// Cached import counts for fast scoring lookups
    import_counts: FxHashMap<u32, u32>,
    /// Map from normalized path to file_id for import resolution
    path_to_id: HashMap<PathBuf, u32>,
    /// Inverted index: filename -> list of full paths (for fast non-relative import lookup)
    filename_to_paths: HashMap<String, Vec<PathBuf>>,
    /// Reverse of `path_to_id`, so removal and re-registration are O(1)
    /// instead of a full scan (and so `filename_to_paths` can be pruned).
    id_to_path: FxHashMap<u32, PathBuf>,
}

/// Resolve `.` and `..` components without touching the file system.
///
/// Correct for paths whose existing prefix is already canonical (no
/// symlinked directories), which is what every candidate built from a
/// registered path is. A `..` that would climb above the root is dropped.
pub fn normalize_lexically(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                if !matches!(
                    out.components().next_back(),
                    None | Some(Component::RootDir) | Some(Component::Prefix(_))
                ) {
                    out.pop();
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

impl DependencyIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a file path with its ID for import resolution.
    ///
    /// Re-registering an id (the watcher's update path) is idempotent: the
    /// previous path mapping for that id is dropped first, so repeated edits
    /// of one file never accumulate duplicate `filename_to_paths` entries.
    pub fn register_file(&mut self, file_id: u32, path: &Path) {
        // Only a path we have not seen needs the realpath walk.
        if self.path_to_id.get(path) == Some(&file_id) {
            return;
        }
        let stored_path = match path.canonicalize() {
            Ok(canonical) => canonical,
            // Fallback to the path as-is if canonicalization fails
            Err(_) => path.to_path_buf(),
        };
        self.register_canonical_file(file_id, stored_path);
    }

    /// [`Self::register_file`] for a path that is already canonical (the
    /// file store's key, a persisted path): no file-system access at all.
    /// Restoring a 61k-file index spent 16 s in realpath walks here.
    pub fn register_canonical_file(&mut self, file_id: u32, stored_path: PathBuf) {
        if self.path_to_id.get(&stored_path) == Some(&file_id) {
            return;
        }
        if let Some(previous) = self.id_to_path.get(&file_id) {
            if *previous == stored_path {
                return; // already registered under exactly this path
            }
            self.unregister_path_lookups(file_id);
        }

        // Add to filename inverted index for fast non-relative lookups
        if let Some(filename) = stored_path.file_name().and_then(|s| s.to_str()) {
            self.filename_to_paths
                .entry(filename.to_string())
                .or_default()
                .push(stored_path.clone());
        }

        self.path_to_id.insert(stored_path.clone(), file_id);
        self.id_to_path.insert(file_id, stored_path);
    }

    /// Drop `file_id` from `path_to_id`, `id_to_path` and `filename_to_paths`.
    fn unregister_path_lookups(&mut self, file_id: u32) {
        let Some(path) = self.id_to_path.remove(&file_id) else {
            return;
        };
        self.path_to_id.remove(&path);
        if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
            if let Some(paths) = self.filename_to_paths.get_mut(filename) {
                paths.retain(|p| *p != path);
                if paths.is_empty() {
                    self.filename_to_paths.remove(filename);
                }
            }
        }
    }

    /// Add an import relationship: `from_file` imports `to_file`
    pub fn add_import(&mut self, from_file: u32, to_file: u32) {
        // Add forward edge
        self.imports.entry(from_file).or_default().insert(to_file);

        // Add reverse edge
        self.imported_by
            .entry(to_file)
            .or_default()
            .insert(from_file);

        // Update cached count
        let count = self.imported_by.get(&to_file).map(|s| s.len()).unwrap_or(0);
        self.import_counts.insert(to_file, count as u32);
    }

    /// Add import from raw import path string, resolving it relative to the source file
    pub fn add_import_from_path(
        &mut self,
        from_file_id: u32,
        from_file_path: &Path,
        import_path: &str,
    ) -> Option<u32> {
        let resolved = self.resolve_import_path(from_file_path, import_path)?;
        let to_file_id = self.path_to_id.get(&resolved).copied()?;
        self.add_import(from_file_id, to_file_id);
        Some(to_file_id)
    }

    /// Resolve an import path to an indexed file, using the *importing* file's
    /// language (by extension) to pick the resolution rules.
    ///
    /// Only structural resolutions are attempted: Rust module paths against
    /// the crate root / module directory, Python relative and package
    /// imports, JS/TS relative paths (with extension and `index` probing) and
    /// `@/`-style source-root aliases. Bare package names (`react`, `os`,
    /// `serde`) resolve to `None`; there is deliberately no "any file with that
    /// name anywhere in the repo" fallback, which produced false edges that
    /// inflated the "heavily imported" ranking boost on unrelated files.
    ///
    /// This method is thread-safe and only requires `&self`.
    pub fn resolve_import_path(&self, from_file: &Path, import_path: &str) -> Option<PathBuf> {
        let ext = from_file
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            "rs" => self.resolve_rust(from_file, import_path),
            "py" | "pyi" | "pyw" => self.resolve_python(from_file, import_path),
            "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" => {
                self.resolve_js(from_file, import_path)
            }
            _ => None,
        }
    }

    /// Is `candidate` an indexed file?
    ///
    /// Candidates are built by joining import segments onto a registered
    /// (canonical) path, so the only thing standing between them and the
    /// map key is `.` / `..` components. Those are resolved lexically; the
    /// file system is never consulted. Resolving on disk here used to cost a
    /// `realpath` walk (one `readlink` per component) for every probe, and
    /// Python's ancestor search alone probes dozens of paths per import:
    /// a 6k-file corpus made 17 million `readlink` calls during its build.
    fn indexed(&self, candidate: &Path) -> Option<PathBuf> {
        if self.path_to_id.contains_key(candidate) {
            return Some(candidate.to_path_buf());
        }
        let normalized = normalize_lexically(candidate);
        if normalized != candidate && self.path_to_id.contains_key(&normalized) {
            return Some(normalized);
        }
        None
    }

    // ---------------------------------------------------------------- Rust

    /// Directory that holds the *child modules* of `file`: `foo/` for
    /// `foo.rs`, the containing directory for `mod.rs` / `lib.rs` / `main.rs`
    /// (and any other crate-root style file such as `build.rs` or a bin).
    fn rust_module_dir(file: &Path) -> Option<PathBuf> {
        let parent = file.parent()?;
        let stem = file.file_stem()?.to_str()?;
        Some(match stem {
            "mod" | "lib" | "main" => parent.to_path_buf(),
            _ => parent.join(stem),
        })
    }

    /// `src/` of the nearest enclosing crate (directory with `Cargo.toml`),
    /// falling back to the file's own directory.
    fn rust_crate_src(file: &Path) -> PathBuf {
        let mut dir = file.parent();
        while let Some(d) = dir {
            if d.join("Cargo.toml").is_file() {
                let src = d.join("src");
                return if src.is_dir() { src } else { d.to_path_buf() };
            }
            dir = d.parent();
        }
        file.parent().map(Path::to_path_buf).unwrap_or_default()
    }

    fn resolve_rust(&self, from_file: &Path, import_path: &str) -> Option<PathBuf> {
        // `use a::b::{c, d};` / `use a::*;` -> drop the group / glob tail.
        let cleaned = import_path
            .split('{')
            .next()
            .unwrap_or("")
            .trim()
            .trim_end_matches("::")
            .trim_end_matches('*')
            .trim_end_matches("::");
        // `use x as y` -> x
        let cleaned = cleaned.split_whitespace().next().unwrap_or("");
        let mut segments: Vec<&str> = cleaned
            .split("::")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if segments.is_empty() {
            return None;
        }

        // Base directory the remaining segments are resolved against.
        let base = match segments[0] {
            "crate" => {
                segments.remove(0);
                Self::rust_crate_src(from_file)
            }
            "self" => {
                segments.remove(0);
                Self::rust_module_dir(from_file)?
            }
            "super" => {
                let mut dir = Self::rust_module_dir(from_file)?;
                while segments.first() == Some(&"super") {
                    segments.remove(0);
                    dir = dir.parent()?.to_path_buf();
                }
                dir
            }
            // `mod foo;` or `use foo::bar` naming a sibling module — or an
            // external crate, which simply fails to resolve below.
            _ => Self::rust_module_dir(from_file)?,
        };
        if segments.is_empty() {
            // `use crate;` / `use super::*;` etc. — the module file itself.
            let candidates = [base.join("mod.rs"), base.with_extension("rs")];
            return candidates.iter().find_map(|c| self.indexed(c));
        }
        // Try the longest module prefix first: `crate::a::b::Item` is the
        // file `a/b.rs` (or `a/b/mod.rs`); `Item` is a symbol, not a file.
        for k in (1..=segments.len()).rev() {
            let mut dir = base.clone();
            for seg in &segments[..k - 1] {
                dir.push(seg);
            }
            let last = segments[k - 1];
            let as_file = dir.join(format!("{last}.rs"));
            let as_mod = dir.join(last).join("mod.rs");
            if let Some(p) = self.indexed(&as_file).or_else(|| self.indexed(&as_mod)) {
                return Some(p);
            }
        }
        None
    }

    // -------------------------------------------------------------- Python

    fn resolve_python(&self, from_file: &Path, import_path: &str) -> Option<PathBuf> {
        let dots = import_path.chars().take_while(|&c| c == '.').count();
        let rest = &import_path[dots..];
        let segments: Vec<&str> = rest.split('.').filter(|s| !s.is_empty()).collect();
        let dir = from_file.parent()?;

        if dots > 0 {
            // `.foo` = sibling package/module, `..foo` = parent package, ...
            let mut base = dir.to_path_buf();
            for _ in 1..dots {
                base = base.parent()?.to_path_buf();
            }
            return self.python_candidates(&base, &segments);
        }

        // Absolute import: the nearest enclosing directory (walking up a
        // bounded number of levels) that contains the package/module wins.
        // Stdlib and site-packages names find nothing and resolve to None.
        let mut base = Some(dir);
        for _ in 0..8 {
            let Some(b) = base else { break };
            if let Some(found) = self.python_candidates(b, &segments) {
                return Some(found);
            }
            base = b.parent();
        }
        None
    }

    /// `a.b.c` under `base` -> `a/b/c.py`, `a/b/c/__init__.py`, and for
    /// `from a.b import name` where `name` is a symbol, `a/b.py` / `a/b/__init__.py`.
    fn python_candidates(&self, base: &Path, segments: &[&str]) -> Option<PathBuf> {
        if segments.is_empty() {
            return self.indexed(&base.join("__init__.py"));
        }
        for k in (1..=segments.len()).rev() {
            let mut dir = base.to_path_buf();
            for seg in &segments[..k - 1] {
                dir.push(seg);
            }
            let last = segments[k - 1];
            let as_module = dir.join(format!("{last}.py"));
            let as_package = dir.join(last).join("__init__.py");
            if let Some(p) = self
                .indexed(&as_module)
                .or_else(|| self.indexed(&as_package))
            {
                return Some(p);
            }
        }
        None
    }

    // --------------------------------------------------------------- JS/TS

    const JS_EXTENSIONS: [&'static str; 10] = [
        "ts", "tsx", "js", "jsx", "mjs", "cjs", "mts", "cts", "d.ts", "json",
    ];

    fn resolve_js(&self, from_file: &Path, import_path: &str) -> Option<PathBuf> {
        let dir = from_file.parent()?;
        let spec = import_path.trim();
        if spec.starts_with("./") || spec.starts_with("../") || spec == "." || spec == ".." {
            return self.js_candidates(&dir.join(spec));
        }
        // Source-root aliases (`@/x`, `~/x`): nearest ancestor with a
        // package.json or tsconfig.json, then `src/x` or `x` under it.
        if let Some(rest) = spec.strip_prefix("@/").or_else(|| spec.strip_prefix("~/")) {
            let mut d = Some(dir);
            while let Some(root) = d {
                if root.join("package.json").is_file() || root.join("tsconfig.json").is_file() {
                    return self
                        .js_candidates(&root.join("src").join(rest))
                        .or_else(|| self.js_candidates(&root.join(rest)));
                }
                d = root.parent();
            }
            return None;
        }
        // Bare package specifiers (`react`, `lodash/merge`, `@scope/pkg`) are
        // external: never guess a same-named local file.
        None
    }

    /// Probe `p`, `p.<ext>`, `p` with `.js` swapped for `.ts`/`.tsx`, and
    /// `p/index.<ext>`.
    fn js_candidates(&self, p: &Path) -> Option<PathBuf> {
        if p.extension().is_some() {
            if let Some(found) = self.indexed(p) {
                return Some(found);
            }
            // TS convention: `import './foo.js'` refers to `foo.ts` on disk.
            if let Some(stem) = p.to_str().and_then(|s| {
                s.strip_suffix(".js")
                    .or_else(|| s.strip_suffix(".jsx"))
                    .or_else(|| s.strip_suffix(".mjs"))
            }) {
                for ext in ["ts", "tsx", "mts"] {
                    if let Some(found) = self.indexed(Path::new(&format!("{stem}.{ext}"))) {
                        return Some(found);
                    }
                }
            }
        }
        let base = p.to_str()?;
        for ext in Self::JS_EXTENSIONS {
            if let Some(found) = self.indexed(Path::new(&format!("{base}.{ext}"))) {
                return Some(found);
            }
        }
        for ext in ["ts", "tsx", "js", "jsx", "mjs", "cjs"] {
            if let Some(found) = self.indexed(&p.join(format!("index.{ext}"))) {
                return Some(found);
            }
        }
        None
    }

    /// Get file ID for a resolved path. Thread-safe.
    pub fn get_file_id(&self, path: &Path) -> Option<u32> {
        self.path_to_id.get(path).copied()
    }

    /// Batch insert multiple import edges. More efficient than repeated add_import calls.
    pub fn add_imports_batch(&mut self, edges: Vec<(u32, u32)>) {
        // Track only the to_file IDs touched by this batch so we update
        // import_counts in O(batch) rather than O(N_total).
        let mut touched_to_files = FxHashSet::default();
        for (from_file, to_file) in edges {
            self.imports.entry(from_file).or_default().insert(to_file);
            self.imported_by
                .entry(to_file)
                .or_default()
                .insert(from_file);
            touched_to_files.insert(to_file);
        }

        // Update cached counts only for files touched by this batch
        for to_file in touched_to_files {
            let count = self.imported_by.get(&to_file).map(|s| s.len()).unwrap_or(0);
            self.import_counts.insert(to_file, count as u32);
        }
    }

    /// Get the number of files that import the given file
    pub fn get_import_count(&self, file_id: u32) -> u32 {
        self.import_counts.get(&file_id).copied().unwrap_or(0)
    }

    /// Get all files that import the given file (dependents)
    pub fn get_dependents(&self, file_id: u32) -> Vec<u32> {
        self.imported_by
            .get(&file_id)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Get all files that the given file imports (dependencies)
    pub fn get_dependencies(&self, file_id: u32) -> Vec<u32> {
        self.imports
            .get(&file_id)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Get total number of dependency edges in the graph
    pub fn total_edges(&self) -> usize {
        self.imports.values().map(|s| s.len()).sum()
    }

    /// Get all import edges as (from_file_id, to_file_id) pairs
    pub fn get_all_edges(&self) -> Vec<(u32, u32)> {
        self.imports
            .iter()
            .flat_map(|(&from, targets)| targets.iter().map(move |&to| (from, to)))
            .collect()
    }

    /// Remove a file and all edges referencing it (for incremental updates).
    ///
    /// Drops the file from both the forward (`imports`) and reverse
    /// (`imported_by`) graphs, updates cached `import_counts` for any file whose
    /// dependent set changed, and removes it from the path lookups. Safe to call
    /// for an id that isn't present (no-op for the graph parts).
    pub fn remove_file(&mut self, file_id: u32) {
        // Forward edges out of `file_id`: remove the reverse entry on each target.
        if let Some(targets) = self.imports.remove(&file_id) {
            for to_file in targets {
                if let Some(set) = self.imported_by.get_mut(&to_file) {
                    set.remove(&file_id);
                    let count = set.len() as u32;
                    if count == 0 {
                        self.imported_by.remove(&to_file);
                        self.import_counts.remove(&to_file);
                    } else {
                        self.import_counts.insert(to_file, count);
                    }
                }
            }
        }

        // Reverse edges into `file_id`: remove the forward entry on each source.
        if let Some(sources) = self.imported_by.remove(&file_id) {
            for from_file in sources {
                if let Some(set) = self.imports.get_mut(&from_file) {
                    set.remove(&file_id);
                    if set.is_empty() {
                        self.imports.remove(&from_file);
                    }
                }
            }
        }
        self.import_counts.remove(&file_id);

        // Remove from path lookups so the id is no longer resolvable (O(1)
        // via id_to_path; also prunes the filename index so a removed path
        // can never be picked as a bare-name resolution candidate).
        self.unregister_path_lookups(file_id);
    }

    /// Clear all dependency information
    pub fn clear(&mut self) {
        self.imports.clear();
        self.imported_by.clear();
        self.import_counts.clear();
        self.path_to_id.clear();
        self.filename_to_paths.clear();
        self.id_to_path.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(files: &[(&str, &str)]) -> (tempfile::TempDir, DependencyIndex) {
        let temp = tempfile::TempDir::new().unwrap();
        let mut idx = DependencyIndex::new();
        for (i, (rel, body)) in files.iter().enumerate() {
            let p = temp.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, body).unwrap();
            idx.register_file(i as u32, &p);
        }
        (temp, idx)
    }

    fn id_of(temp: &tempfile::TempDir, idx: &DependencyIndex, rel: &str) -> Option<u32> {
        idx.get_file_id(&temp.path().join(rel).canonicalize().unwrap())
    }

    /// Roadmap 2.5: Rust `use crate::…`, `super::`, `self::` and `mod foo;`
    /// resolve against the crate root / module directory; `foo/mod.rs` is
    /// tried; external crates resolve to nothing.
    #[test]
    fn test_resolve_rust_module_paths() {
        let (temp, idx) = setup(&[
            ("Cargo.toml", "[package]\n"),
            ("src/main.rs", "mod net; mod util;\n"),
            ("src/util.rs", ""),
            ("src/net/mod.rs", "mod tcp;\n"),
            (
                "src/net/tcp.rs",
                "use super::udp; use crate::util::helper;\n",
            ),
            ("src/net/udp.rs", ""),
            ("other/util.rs", "// decoy with the same name\n"),
        ]);
        let f = |rel: &str| temp.path().join(rel);
        let r = |from: &str, imp: &str| {
            idx.resolve_import_path(&f(from), imp)
                .and_then(|p| idx.get_file_id(&p))
        };
        assert_eq!(r("src/main.rs", "util"), id_of(&temp, &idx, "src/util.rs"));
        assert_eq!(
            r("src/main.rs", "net"),
            id_of(&temp, &idx, "src/net/mod.rs")
        );
        assert_eq!(
            r("src/net/mod.rs", "tcp"),
            id_of(&temp, &idx, "src/net/tcp.rs")
        );
        assert_eq!(
            r("src/net/tcp.rs", "super::udp"),
            id_of(&temp, &idx, "src/net/udp.rs")
        );
        assert_eq!(
            r("src/net/tcp.rs", "crate::util::helper"),
            id_of(&temp, &idx, "src/util.rs"),
            "crate:: path with a trailing symbol segment"
        );
        assert_eq!(
            r("src/net/tcp.rs", "crate::net::{tcp, udp}"),
            id_of(&temp, &idx, "src/net/mod.rs")
        );
        assert_eq!(
            r("src/net/tcp.rs", "serde::Deserialize"),
            None,
            "external crate"
        );
        assert_eq!(r("src/net/tcp.rs", "std::collections::HashMap"), None);
        // The decoy `other/util.rs` must never be picked for `mod util;`.
        assert_ne!(
            r("src/main.rs", "util"),
            id_of(&temp, &idx, "other/util.rs")
        );
    }

    /// Roadmap 2.5: Python relative imports walk parent packages, dotted
    /// imports are directories, packages resolve to `__init__.py`, and
    /// stdlib names resolve to nothing.
    #[test]
    fn test_resolve_python_imports() {
        let (temp, idx) = setup(&[
            ("pkg/__init__.py", ""),
            ("pkg/core.py", ""),
            ("pkg/sub/__init__.py", ""),
            (
                "pkg/sub/leaf.py",
                "from ..core import x; from . import sibling; import pkg.core\n",
            ),
            ("pkg/sub/sibling.py", ""),
            (
                "scripts/run.py",
                "import pkg.sub.leaf; from pkg import core\n",
            ),
        ]);
        let f = |rel: &str| temp.path().join(rel);
        let r = |from: &str, imp: &str| {
            idx.resolve_import_path(&f(from), imp)
                .and_then(|p| idx.get_file_id(&p))
        };
        assert_eq!(
            r("pkg/sub/leaf.py", "..core"),
            id_of(&temp, &idx, "pkg/core.py")
        );
        assert_eq!(
            r("pkg/sub/leaf.py", ".sibling"),
            id_of(&temp, &idx, "pkg/sub/sibling.py")
        );
        assert_eq!(
            r("pkg/sub/leaf.py", "."),
            id_of(&temp, &idx, "pkg/sub/__init__.py")
        );
        assert_eq!(
            r("pkg/sub/leaf.py", "pkg.core"),
            id_of(&temp, &idx, "pkg/core.py")
        );
        assert_eq!(
            r("scripts/run.py", "pkg.sub.leaf"),
            id_of(&temp, &idx, "pkg/sub/leaf.py")
        );
        assert_eq!(
            r("scripts/run.py", "pkg.sub"),
            id_of(&temp, &idx, "pkg/sub/__init__.py")
        );
        assert_eq!(
            r("scripts/run.py", "pkg"),
            id_of(&temp, &idx, "pkg/__init__.py")
        );
        assert_eq!(r("scripts/run.py", "os"), None, "stdlib");
        assert_eq!(r("scripts/run.py", "os.path"), None);
    }

    /// Roadmap 2.5: JS/TS relative imports probe extensions, `.js` -> `.ts`,
    /// and `index.*`; `@/` aliases resolve against the package root; bare
    /// package names never bind to a same-named local file.
    #[test]
    fn test_resolve_js_imports() {
        let (temp, idx) = setup(&[
            ("package.json", "{}"),
            ("src/app.tsx", ""),
            ("src/util/index.ts", ""),
            ("src/util/merge.ts", ""),
            ("src/lib/helpers.ts", ""),
            ("src/components/Button.tsx", ""),
        ]);
        let f = |rel: &str| temp.path().join(rel);
        let r = |from: &str, imp: &str| {
            idx.resolve_import_path(&f(from), imp)
                .and_then(|p| idx.get_file_id(&p))
        };
        assert_eq!(
            r("src/app.tsx", "./util"),
            id_of(&temp, &idx, "src/util/index.ts")
        );
        assert_eq!(
            r("src/app.tsx", "./util/merge"),
            id_of(&temp, &idx, "src/util/merge.ts")
        );
        assert_eq!(
            r("src/app.tsx", "./lib/helpers.js"),
            id_of(&temp, &idx, "src/lib/helpers.ts")
        );
        assert_eq!(
            r("src/components/Button.tsx", "../lib/helpers"),
            id_of(&temp, &idx, "src/lib/helpers.ts")
        );
        assert_eq!(
            r("src/app.tsx", "@/components/Button"),
            id_of(&temp, &idx, "src/components/Button.tsx")
        );
        assert_eq!(r("src/app.tsx", "react"), None, "bare package");
        assert_eq!(
            r("src/app.tsx", "lodash/merge"),
            None,
            "must not bind to the local merge.ts"
        );
        assert_eq!(r("src/app.tsx", "./missing"), None);
    }

    /// Roadmap 6.5 finding: candidate probing must not touch the file
    /// system. `..` / `.` in a candidate are resolved lexically and a
    /// candidate that is not indexed is simply absent from the map.
    #[test]
    fn test_normalize_lexically() {
        let n = |s: &str| normalize_lexically(Path::new(s));
        assert_eq!(n("/a/b/../c/./d.rs"), PathBuf::from("/a/c/d.rs"));
        assert_eq!(n("/a/../../b"), PathBuf::from("/b"));
        assert_eq!(n("a/./b/../c"), PathBuf::from("a/c"));
        assert_eq!(n("/a/b"), PathBuf::from("/a/b"));
    }

    #[test]
    fn test_relative_candidates_resolve_without_disk_probing() {
        let (temp, idx) = setup(&[
            ("src/util.ts", ""),
            ("src/x/y.ts", ""),
            ("pkg/__init__.py", ""),
            ("pkg/sub/mod.py", ""),
        ]);
        let from = |rel: &str| temp.path().join(rel).canonicalize().unwrap();
        let id_of = |rel: &str| idx.get_file_id(&temp.path().join(rel).canonicalize().unwrap());
        assert_eq!(
            idx.resolve_import_path(&from("src/x/y.ts"), "../util")
                .and_then(|p| idx.get_file_id(&p)),
            id_of("src/util.ts")
        );
        assert_eq!(
            idx.resolve_import_path(&from("pkg/sub/mod.py"), "..")
                .and_then(|p| idx.get_file_id(&p)),
            id_of("pkg/__init__.py")
        );
        // A candidate that would exist on disk but is not indexed resolves
        // to nothing (the file system is not consulted).
        std::fs::write(temp.path().join("src/x/z.ts"), "").unwrap();
        assert_eq!(idx.resolve_import_path(&from("src/x/y.ts"), "./z"), None);
    }

    /// Roadmap 1.11: re-registering an id (watcher update path) must not
    /// accumulate duplicate filename entries, and removal must prune every
    /// lookup so the path cannot resolve afterwards.
    #[test]
    fn test_reregister_and_remove_keep_lookups_bounded() {
        let temp = tempfile::TempDir::new().unwrap();
        let a = temp.path().join("util.py");
        let b = temp.path().join("main.py");
        std::fs::write(&a, "x = 1\n").unwrap();
        std::fs::write(&b, "import util\n").unwrap();

        let mut idx = DependencyIndex::new();
        idx.register_file(1, &b);
        for _ in 0..1000 {
            idx.register_file(0, &a);
        }
        assert_eq!(idx.filename_to_paths["util.py"].len(), 1);
        assert_eq!(idx.path_to_id.len(), 2);

        // Resolvable while present ...
        assert_eq!(idx.add_import_from_path(1, &b, "util"), Some(0));
        assert_eq!(idx.get_import_count(0), 1);

        // ... and fully gone after removal.
        idx.remove_file(0);
        assert!(!idx.filename_to_paths.contains_key("util.py"));
        assert_eq!(idx.path_to_id.len(), 1);
        assert_eq!(idx.get_import_count(0), 0);
        assert_eq!(idx.add_import_from_path(1, &b, "util"), None);
    }

    #[test]
    fn test_add_import() {
        let mut index = DependencyIndex::new();
        index.add_import(1, 2);
        index.add_import(3, 2);
        index.add_import(4, 2);

        assert_eq!(index.get_import_count(2), 3);
        assert_eq!(index.get_dependents(2).len(), 3);
        assert_eq!(index.get_dependencies(1), vec![2]);
    }

    #[test]
    fn test_bidirectional() {
        let mut index = DependencyIndex::new();
        index.add_import(1, 2);

        assert!(index.get_dependencies(1).contains(&2));
        assert!(index.get_dependents(2).contains(&1));
    }
}
