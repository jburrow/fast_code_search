//! Read-only queries over the import graph: breadth-first levels in either
//! direction, the shortest import chain between two files, and strongly
//! connected components (import cycles).
//!
//! Everything works on file ids and the in-memory `imports` / `imported_by`
//! sets, so a query costs O(nodes + edges visited) and never touches disk.
//! Rust parent-to-child module edges (`mod foo;`) are containment rather than
//! dependencies; callers choose whether to follow them.

use super::DependencyIndex;
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::VecDeque;

/// Which way to walk an edge `a imports b`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// From a file to the files it imports (its dependencies).
    Imports,
    /// From a file to the files that import it (its dependents).
    ImportedBy,
}

impl DependencyIndex {
    /// Edges touching `id` in either direction, containment included.
    pub fn degree(&self, id: u32) -> usize {
        self.imports.get(&id).map_or(0, |s| s.len()) + self.get_import_count(id) as usize
    }

    /// Neighbours of `id` in `direction`, sorted by id for stable output,
    /// optionally skipping Rust parent-to-child containment edges.
    pub fn neighbours(&self, id: u32, direction: Direction, containment: bool) -> Vec<u32> {
        let set = match direction {
            Direction::Imports => self.imports.get(&id),
            Direction::ImportedBy => self.imported_by.get(&id),
        };
        let mut out: Vec<u32> = set
            .into_iter()
            .flatten()
            .copied()
            .filter(|&other| {
                containment
                    || match direction {
                        Direction::Imports => !self.is_containment(id, other),
                        Direction::ImportedBy => !self.is_containment(other, id),
                    }
            })
            .collect();
        out.sort_unstable();
        out
    }

    /// Breadth-first levels from `start`: `levels[0] == [start]`, and
    /// `levels[d]` holds the files first reached after `d` edges. Stops after
    /// `max_depth` levels (`None` = until nothing new is reached).
    pub fn levels(
        &self,
        start: u32,
        direction: Direction,
        max_depth: Option<usize>,
        containment: bool,
    ) -> Vec<Vec<u32>> {
        let mut seen = FxHashSet::default();
        seen.insert(start);
        let mut levels = vec![vec![start]];
        while max_depth.is_none_or(|max| levels.len() <= max) {
            let mut next = Vec::new();
            for &node in levels.last().expect("levels is never empty") {
                for n in self.neighbours(node, direction, containment) {
                    if seen.insert(n) {
                        next.push(n);
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            levels.push(next);
        }
        levels
    }

    /// Shortest chain of imports `from -> … -> to`, as the files along it
    /// (both ends included), or `None` when `to` is not reachable.
    pub fn shortest_path(&self, from: u32, to: u32, containment: bool) -> Option<Vec<u32>> {
        let mut prev: FxHashMap<u32, u32> = FxHashMap::default();
        let mut queue = VecDeque::from([from]);
        prev.insert(from, from);
        while let Some(node) = queue.pop_front() {
            if node == to {
                let mut chain = vec![to];
                let mut cur = to;
                while cur != from {
                    cur = prev[&cur];
                    chain.push(cur);
                }
                chain.reverse();
                return Some(chain);
            }
            for n in self.neighbours(node, Direction::Imports, containment) {
                if let std::collections::hash_map::Entry::Vacant(e) = prev.entry(n) {
                    e.insert(node);
                    queue.push_back(n);
                }
            }
        }
        None
    }

    /// Import cycles: every file that is part of a strongly connected
    /// component of two or more files, mapped to that component's index.
    pub fn cycles(&self, containment: bool) -> FxHashMap<u32, u32> {
        let mut ids: Vec<u32> = self.file_ids().collect();
        ids.sort_unstable();
        let dense: FxHashMap<u32, usize> = ids.iter().enumerate().map(|(i, &id)| (id, i)).collect();
        let adjacency: Vec<Vec<usize>> = ids
            .iter()
            .map(|&id| {
                self.neighbours(id, Direction::Imports, containment)
                    .into_iter()
                    .filter_map(|n| dense.get(&n).copied())
                    .collect()
            })
            .collect();
        let components = strongly_connected_components(&adjacency);
        let mut sizes: FxHashMap<usize, u32> = FxHashMap::default();
        for &c in &components {
            *sizes.entry(c).or_default() += 1;
        }
        ids.iter()
            .zip(&components)
            .filter(|(_, c)| sizes[c] > 1)
            .map(|(&id, &c)| (id, c as u32))
            .collect()
    }
}

/// Tarjan's algorithm over a dense adjacency list, iterative so a long import
/// chain cannot overflow the stack. Returns each node's component index.
pub fn strongly_connected_components(adjacency: &[Vec<usize>]) -> Vec<usize> {
    const UNVISITED: usize = usize::MAX;
    let n = adjacency.len();
    let mut index = vec![UNVISITED; n];
    let mut low = vec![0usize; n];
    let mut on_stack = vec![false; n];
    let mut component = vec![UNVISITED; n];
    let mut stack = Vec::new();
    let mut next_index = 0;
    let mut next_component = 0;
    // (node, position of the next edge to look at)
    let mut call: Vec<(usize, usize)> = Vec::new();

    for root in 0..n {
        if index[root] != UNVISITED {
            continue;
        }
        call.push((root, 0));
        while let Some(&mut (v, ref mut edge)) = call.last_mut() {
            if *edge == 0 && index[v] == UNVISITED {
                index[v] = next_index;
                low[v] = next_index;
                next_index += 1;
                stack.push(v);
                on_stack[v] = true;
            }
            if let Some(&w) = adjacency[v].get(*edge) {
                *edge += 1;
                if index[w] == UNVISITED {
                    call.push((w, 0));
                } else if on_stack[w] {
                    low[v] = low[v].min(index[w]);
                }
                continue;
            }
            // All edges of v done: close its component if it is a root.
            if low[v] == index[v] {
                loop {
                    let w = stack.pop().expect("v is on the stack");
                    on_stack[w] = false;
                    component[w] = next_component;
                    if w == v {
                        break;
                    }
                }
                next_component += 1;
            }
            call.pop();
            if let Some(&(parent, _)) = call.last() {
                low[parent] = low[parent].min(low[v]);
            }
        }
    }
    component
}

/// Is `path` a test file? Directory names (`tests/`, `test/`, `__tests__/`,
/// `spec/`) and file names (`test_x.py`, `x_test.go`, `x.test.ts`,
/// `x.spec.js`, Rust's `tests.rs`) both count.
pub fn is_test_path(path: &str) -> bool {
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    let mut parts = lower.rsplit('/');
    let file = parts.next().unwrap_or_default();
    if parts.any(|dir| matches!(dir, "test" | "tests" | "__tests__" | "spec" | "specs")) {
        return true;
    }
    let stem = file.split('.').next().unwrap_or_default();
    stem == "tests"
        || stem == "test"
        || stem.starts_with("test_")
        || stem.ends_with("_test")
        || stem.ends_with("_tests")
        || file.contains(".test.")
        || file.contains(".spec.")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Five files registered under fake paths: 0 -> 1 -> 2 -> 0 is a cycle,
    /// 3 -> 1, and 4 imports nothing.
    fn sample() -> DependencyIndex {
        let mut idx = DependencyIndex::new();
        for i in 0..5u32 {
            idx.register_canonical_file(i, format!("/r/f{i}.py").into());
        }
        idx.add_imports_batch(vec![(0, 1), (1, 2), (2, 0), (3, 1)]);
        idx
    }

    #[test]
    fn levels_walk_both_directions() {
        let idx = sample();
        assert_eq!(
            idx.levels(3, Direction::Imports, None, true),
            vec![vec![3], vec![1], vec![2], vec![0]]
        );
        assert_eq!(
            idx.levels(1, Direction::ImportedBy, Some(1), true),
            vec![vec![1], vec![0, 3]]
        );
        assert_eq!(idx.levels(4, Direction::Imports, None, true), vec![vec![4]]);
    }

    #[test]
    fn shortest_path_follows_imports_only() {
        let idx = sample();
        assert_eq!(idx.shortest_path(3, 0, true), Some(vec![3, 1, 2, 0]));
        assert_eq!(idx.shortest_path(0, 3, true), None);
        assert_eq!(idx.shortest_path(2, 2, true), Some(vec![2]));
    }

    #[test]
    fn cycles_report_only_multi_file_components() {
        let idx = sample();
        let cycles = idx.cycles(true);
        assert_eq!(cycles.len(), 3);
        assert_eq!(cycles[&0], cycles[&1]);
        assert_eq!(cycles[&1], cycles[&2]);
        assert!(!cycles.contains_key(&3) && !cycles.contains_key(&4));
    }

    #[test]
    fn tarjan_handles_a_long_chain_without_recursion() {
        let n = 200_000;
        let adjacency: Vec<Vec<usize>> = (0..n)
            .map(|i| if i + 1 < n { vec![i + 1] } else { vec![0] })
            .collect();
        let comp = strongly_connected_components(&adjacency);
        assert!(comp.iter().all(|&c| c == comp[0]));
    }

    #[test]
    fn rust_parent_to_child_edges_are_containment() {
        let mut idx = DependencyIndex::new();
        let files = [
            "/r/src/lib.rs",
            "/r/src/search/mod.rs",
            "/r/src/search/query.rs",
            "/r/src/util.rs",
            "/r/src/search/engine/mod.rs",
        ];
        for (i, f) in files.iter().enumerate() {
            idx.register_canonical_file(i as u32, (*f).into());
        }
        assert!(idx.is_containment(0, 1), "lib.rs -> search/mod.rs");
        assert!(idx.is_containment(0, 3), "lib.rs -> util.rs");
        assert!(idx.is_containment(1, 2), "search/mod.rs -> search/query.rs");
        assert!(
            idx.is_containment(1, 4),
            "search/mod.rs -> search/engine/mod.rs"
        );
        assert!(
            !idx.is_containment(2, 3),
            "query.rs -> util.rs is a dependency"
        );
        assert!(!idx.is_containment(2, 1), "child -> parent is a dependency");
        assert!(
            !idx.is_containment(0, 2),
            "grandchild is not a direct child"
        );
        idx.register_canonical_file(5, "/r/src/main.rs".into());
        assert!(
            !idx.is_containment(5, 3),
            "main.rs beside lib.rs uses the library's modules"
        );

        idx.add_imports_batch(vec![(1, 2), (2, 1)]);
        assert!(idx.cycles(false).is_empty(), "containment hides the cycle");
        assert_eq!(idx.cycles(true).len(), 2);
        assert_eq!(
            idx.neighbours(1, Direction::Imports, false),
            Vec::<u32>::new()
        );
        assert_eq!(
            idx.neighbours(2, Direction::ImportedBy, false),
            Vec::<u32>::new()
        );
    }

    #[test]
    fn test_paths() {
        for p in [
            "repo/tests/integration.rs",
            "repo/src/search/engine/tests.rs",
            "pkg/test_core.py",
            "pkg/core_test.go",
            "web/app.test.ts",
            "web/app.spec.js",
            "web/__tests__/x.js",
        ] {
            assert!(is_test_path(p), "{p}");
        }
        for p in [
            "repo/src/testing_utils.rs",
            "repo/src/contest.py",
            "latest.rs",
        ] {
            assert!(!is_test_path(p), "{p}");
        }
    }
}
