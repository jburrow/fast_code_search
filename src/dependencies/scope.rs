//! Graph-scoped search: the files a file reaches through imports (`deps:`)
//! or the files that reach it (`rdeps:`), with how many hops away each is.
//!
//! An import of a package usually lands on the package's entry file
//! (`mod.rs`, `__init__.py`, `index.ts`), not on the module behind it, so a
//! plain one-hop walk would miss everyone who imports a module through its
//! package. An edge from a package entry file to one of its own modules
//! therefore costs no hop: `rdeps:search/ranking.rs` reaches
//! `search/mod.rs` at hop 0 and the files importing `crate::search` at hop 1.
//! Only one entry file is crossed at a time (never `ranking.rs` ->
//! `search/mod.rs` -> `lib.rs` for free), and a file reached across one is
//! kept only when it names something the module on the other side defines,
//! so importing an unrelated item from the same package does not count.
//! Negated filters walk without pass-through: excluding a superset would
//! drop files that do not depend on the anchor.

use super::graph::Direction;
use super::DependencyIndex;
use rustc_hash::FxHashMap;
use std::collections::VecDeque;
use std::path::Path;

/// Extensions the import graph has edges for.
const GRAPH_EXTENSIONS: &[&str] = &[
    "rs", "py", "pyi", "pyw", "js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts",
];

/// The JavaScript / TypeScript ones, whose `index.*` is a package entry.
const JS_EXTENSIONS: &[&str] = &["js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts"];

/// Whether files with this path can have import edges at all.
pub fn has_import_graph(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| GRAPH_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Whether `entry` is the package entry file of `file`: `mod.rs`, `lib.rs`,
/// `main.rs`, `__init__.py` or `index.*` in the same folder, or for Rust
/// `foo.rs` next to the folder `foo/` holding `file`.
pub fn is_package_entry_of(entry: &Path, file: &Path) -> bool {
    if entry == file {
        return false;
    }
    let (Some(name), Some(entry_dir), Some(file_dir)) = (
        entry.file_name().and_then(|n| n.to_str()),
        entry.parent(),
        file.parent(),
    ) else {
        return false;
    };
    let same_dir_entry = matches!(name, "mod.rs" | "lib.rs" | "main.rs" | "__init__.py")
        || name
            .strip_prefix("index.")
            .is_some_and(|ext| JS_EXTENSIONS.contains(&ext));
    if same_dir_entry && entry_dir == file_dir {
        return true;
    }
    // Rust 2018 layout: `search.rs` declares `mod ranking;` in `search/`.
    entry.extension().is_some_and(|e| e == "rs")
        && file.extension().is_some_and(|e| e == "rs")
        && entry
            .file_stem()
            .is_some_and(|stem| entry_dir.join(stem) == file_dir)
}

impl DependencyIndex {
    /// Files reachable from `anchors` in `direction`, each with its hop
    /// count (anchors are hop 0), up to `max_hops` (`None` = no limit).
    /// Rust `mod` edges are followed.
    ///
    /// With `pass_through`, an edge between a package entry file and one of
    /// its own modules costs no hop, at most once in a row (never a chain of
    /// entry files), and a file reached across an entry file is kept only
    /// when `uses(user, definer)` says it names something the module on the
    /// other side defines: for `rdeps:a.rs` through `mod.rs`, an importer of
    /// `mod.rs` must name something from `a.rs`; for `deps:f.rs` through
    /// `mod.rs`, a module behind it must define something `f.rs` names.
    pub fn scope_walk(
        &self,
        anchors: &[u32],
        direction: Direction,
        max_hops: Option<u32>,
        pass_through: bool,
        uses: &dyn Fn(u32, u32) -> bool,
    ) -> FxHashMap<u32, u32> {
        /// A file on the walk: its hops, whether it was reached across an
        /// entry file (and from which file), and the file before it.
        #[derive(Clone, Copy)]
        struct Step {
            node: u32,
            hops: u32,
            free_from: Option<u32>,
            prev: Option<u32>,
        }
        let mut best: FxHashMap<u32, u32> = FxHashMap::default();
        // 0-1 breadth-first search: free edges go to the front.
        let mut queue: VecDeque<Step> = VecDeque::new();
        for &a in anchors {
            if best.insert(a, 0).is_none() {
                queue.push_back(Step {
                    node: a,
                    hops: 0,
                    free_from: None,
                    prev: None,
                });
            }
        }
        while let Some(step) = queue.pop_front() {
            if best.get(&step.node).is_some_and(|&b| b < step.hops) {
                continue; // a shorter route was found after this was queued
            }
            for next in self.neighbours(step.node, direction, true) {
                let (importer, imported) = match direction {
                    Direction::Imports => (step.node, next),
                    Direction::ImportedBy => (next, step.node),
                };
                let free = pass_through
                    && step.free_from.is_none()
                    && match (self.path_of(importer), self.path_of(imported)) {
                        (Some(i), Some(m)) => is_package_entry_of(i, m),
                        _ => false,
                    };
                let keep = match (direction, free) {
                    // Through a package entry into its modules: the module
                    // must define something the file before the entry names.
                    (Direction::Imports, true) => step.prev.is_none_or(|user| uses(user, next)),
                    // Out of an entry file reached from module `a`: the
                    // importer must name something `a` defines.
                    (Direction::ImportedBy, false) => {
                        step.free_from.is_none_or(|definer| uses(next, definer))
                    }
                    _ => true,
                };
                if !keep {
                    continue;
                }
                let hops = if free { step.hops } else { step.hops + 1 };
                if max_hops.is_some_and(|max| hops > max) {
                    continue;
                }
                if best.get(&next).is_none_or(|&b| hops < b) {
                    best.insert(next, hops);
                    let s = Step {
                        node: next,
                        hops,
                        free_from: free.then_some(step.node),
                        prev: Some(step.node),
                    };
                    if free {
                        queue.push_front(s);
                    } else {
                        queue.push_back(s);
                    }
                }
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn index(files: &[&str], edges: &[(usize, usize)]) -> DependencyIndex {
        let mut idx = DependencyIndex::new();
        for (i, f) in files.iter().enumerate() {
            idx.register_canonical_file(i as u32, PathBuf::from(f).into());
        }
        for &(a, b) in edges {
            idx.add_import(a as u32, b as u32);
        }
        idx
    }

    fn sorted(h: &FxHashMap<u32, u32>) -> Vec<(u32, u32)> {
        let mut v: Vec<_> = h.iter().map(|(&k, &v)| (k, v)).collect();
        v.sort_unstable();
        v
    }

    #[test]
    fn package_entries() {
        let e = |a: &str, b: &str| is_package_entry_of(Path::new(a), Path::new(b));
        assert!(e("/r/src/search/mod.rs", "/r/src/search/ranking.rs"));
        assert!(e("/r/src/search.rs", "/r/src/search/ranking.rs"));
        assert!(e("/r/src/lib.rs", "/r/src/web.rs"));
        assert!(e("/r/pkg/__init__.py", "/r/pkg/mod.py"));
        assert!(e("/r/lib/index.ts", "/r/lib/foo.ts"));
        assert!(!e("/r/src/search/mod.rs", "/r/src/search/mod.rs"));
        assert!(!e("/r/src/lib.rs", "/r/src/search/mod.rs"));
        assert!(!e("/r/src/search/ranking.rs", "/r/src/search/mod.rs"));
        assert!(!e("/r/lib/index.md", "/r/lib/foo.ts"));
    }

    #[test]
    fn rdeps_pass_through_the_package_entry() {
        // 0 ranking.rs, 1 search/mod.rs (mod ranking;), 2 web.rs uses
        // crate::search::Weight (resolves to mod.rs), 3 cli.rs uses web,
        // 4 sibling.rs uses super:: (imports mod.rs).
        let idx = index(
            &[
                "/r/src/search/ranking.rs",
                "/r/src/search/mod.rs",
                "/r/src/web.rs",
                "/r/src/cli.rs",
                "/r/src/search/sibling.rs",
            ],
            &[(1, 0), (2, 1), (3, 2), (4, 1)],
        );
        let one = idx.scope_walk(&[0], Direction::ImportedBy, Some(1), true, &|_, _| true);
        assert_eq!(sorted(&one), vec![(0, 0), (1, 0), (2, 1), (4, 1)]);
        let all = idx.scope_walk(&[0], Direction::ImportedBy, None, true, &|_, _| true);
        assert_eq!(sorted(&all), vec![(0, 0), (1, 0), (2, 1), (3, 2), (4, 1)]);
    }

    #[test]
    fn deps_pass_through_into_the_package() {
        // main.py imports pkg (its __init__), which re-exports pkg/a.py.
        let idx = index(
            &[
                "/r/main.py",
                "/r/pkg/__init__.py",
                "/r/pkg/a.py",
                "/r/other.py",
            ],
            &[(0, 1), (1, 2), (2, 3)],
        );
        let one = idx.scope_walk(&[0], Direction::Imports, Some(1), true, &|_, _| true);
        assert_eq!(sorted(&one), vec![(0, 0), (1, 1), (2, 1)]);
    }

    #[test]
    fn pass_through_never_chains_and_checks_names() {
        // 2018 layout: 0 search/ranking.rs, 1 search.rs (mod ranking;),
        // 2 lib.rs (mod search;), 3 bin.rs uses the crate (lib.rs),
        // 4 web.rs and 5 cli.rs import search.rs; only web.rs names
        // something ranking.rs defines.
        let idx = index(
            &[
                "/r/src/search/ranking.rs",
                "/r/src/search.rs",
                "/r/src/lib.rs",
                "/r/src/bin.rs",
                "/r/src/web.rs",
                "/r/src/cli.rs",
            ],
            &[(1, 0), (2, 1), (3, 2), (4, 1), (5, 1)],
        );
        let uses = |user: u32, definer: u32| definer == 0 && user == 4;
        // lib.rs (`mod search;`) and cli.rs import search.rs but name
        // nothing from ranking.rs.
        let walk = idx.scope_walk(&[0], Direction::ImportedBy, Some(1), true, &uses);
        assert_eq!(sorted(&walk), vec![(0, 0), (1, 0), (4, 1)]);
        // Without the name check they count, but lib.rs is a hop (no second
        // free edge), so bin.rs, which imports lib.rs, is out of reach.
        let all = |_: u32, _: u32| true;
        let walk = idx.scope_walk(&[0], Direction::ImportedBy, Some(1), true, &all);
        assert_eq!(sorted(&walk), vec![(0, 0), (1, 0), (2, 1), (4, 1), (5, 1)]);
        // Strict walk (negated filters): every edge is a hop.
        let strict = idx.scope_walk(&[0], Direction::ImportedBy, Some(1), false, &uses);
        assert_eq!(sorted(&strict), vec![(0, 0), (1, 1)]);
        // deps: through search.rs, only modules defining what web.rs names.
        let deps = idx.scope_walk(&[4], Direction::Imports, Some(1), true, &uses);
        assert_eq!(sorted(&deps), vec![(0, 1), (1, 1), (4, 0)]);
        let none = |_: u32, _: u32| false;
        let deps = idx.scope_walk(&[5], Direction::Imports, Some(1), true, &none);
        assert_eq!(sorted(&deps), vec![(1, 1), (5, 0)]);
    }

    #[test]
    fn cycles_terminate_and_keep_the_shortest_route() {
        let idx = index(
            &["/r/a.py", "/r/b.py", "/r/c.py"],
            &[(0, 1), (1, 2), (2, 0), (0, 2)],
        );
        let all = idx.scope_walk(&[0], Direction::Imports, None, true, &|_, _| true);
        assert_eq!(sorted(&all), vec![(0, 0), (1, 1), (2, 1)]);
    }
}
