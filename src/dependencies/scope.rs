//! Graph-scoped search: the files a file reaches through imports (`deps:`)
//! or the files that reach it (`rdeps:`), with how many hops away each is.
//!
//! An import of a package usually lands on the package's entry file
//! (`mod.rs`, `__init__.py`, `index.ts`), not on the module behind it, so a
//! plain one-hop walk would miss everyone who imports a module through its
//! package. An edge from a package entry file to one of its own modules
//! therefore costs no hop: `rdeps:search/ranking.rs` reaches
//! `search/mod.rs` at hop 0 and the files importing `crate::search` at hop 1.
//! The result can hold a file that imports something else from the same
//! package, which is the safe side for "what could this change affect".

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
    /// Rust `mod` edges are followed; an edge from a package entry file to
    /// one of its own modules costs no hop.
    pub fn scope_walk(
        &self,
        anchors: &[u32],
        direction: Direction,
        max_hops: Option<u32>,
    ) -> FxHashMap<u32, u32> {
        let mut hops: FxHashMap<u32, u32> = FxHashMap::default();
        // 0-1 breadth-first search: free edges go to the front.
        let mut queue: VecDeque<(u32, u32)> = VecDeque::new();
        for &a in anchors {
            if hops.insert(a, 0).is_none() {
                queue.push_back((a, 0));
            }
        }
        while let Some((node, d)) = queue.pop_front() {
            if hops.get(&node).is_some_and(|&best| best < d) {
                continue; // a shorter route was found after this was queued
            }
            for next in self.neighbours(node, direction, true) {
                let (importer, imported) = match direction {
                    Direction::Imports => (node, next),
                    Direction::ImportedBy => (next, node),
                };
                let free = match (self.path_of(importer), self.path_of(imported)) {
                    (Some(i), Some(m)) => is_package_entry_of(i, m),
                    _ => false,
                };
                let nd = if free { d } else { d + 1 };
                if max_hops.is_some_and(|max| nd > max) {
                    continue;
                }
                if hops.get(&next).is_none_or(|&best| nd < best) {
                    hops.insert(next, nd);
                    if free {
                        queue.push_front((next, nd));
                    } else {
                        queue.push_back((next, nd));
                    }
                }
            }
        }
        hops
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn index(files: &[&str], edges: &[(usize, usize)]) -> DependencyIndex {
        let mut idx = DependencyIndex::new();
        for (i, f) in files.iter().enumerate() {
            idx.register_canonical_file(i as u32, PathBuf::from(f));
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
        let one = idx.scope_walk(&[0], Direction::ImportedBy, Some(1));
        assert_eq!(sorted(&one), vec![(0, 0), (1, 0), (2, 1), (4, 1)]);
        let all = idx.scope_walk(&[0], Direction::ImportedBy, None);
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
        let one = idx.scope_walk(&[0], Direction::Imports, Some(1));
        assert_eq!(sorted(&one), vec![(0, 0), (1, 1), (2, 1)]);
    }

    #[test]
    fn cycles_terminate_and_keep_the_shortest_route() {
        let idx = index(
            &["/r/a.py", "/r/b.py", "/r/c.py"],
            &[(0, 1), (1, 2), (2, 0), (0, 2)],
        );
        let all = idx.scope_walk(&[0], Direction::Imports, None);
        assert_eq!(sorted(&all), vec![(0, 0), (1, 1), (2, 1)]);
    }
}
