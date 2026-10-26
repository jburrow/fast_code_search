//! Where the index's memory goes: estimated heap bytes per structure.
//!
//! Memory is what limits a million-file index on a laptop, and resident
//! memory alone does not say which structure to shrink. The breakdown is
//! reported by `/api/diagnostics`, `corpus_bench` and the scoreboard, so a
//! change that grows one part shows up as that part.

use super::*;
use crate::heap_size as h;

/// One part of the index and its estimated heap bytes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MemoryPart {
    /// Short stable name (`postings`, `file_table`, ...), also used as the
    /// metric name in benchmarks.
    pub name: String,
    /// What it holds, for people.
    pub what: String,
    pub bytes: u64,
}

/// Estimated heap bytes of every large structure in the index, largest
/// first. See [`crate::heap_size`] for how each figure is estimated: they
/// leave out allocator overhead, so their sum is below resident memory.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MemoryBreakdown {
    pub parts: Vec<MemoryPart>,
    /// The sum of `parts`.
    pub total_bytes: u64,
    /// Posting lists read in place from the mapped index file: not heap,
    /// and not in `parts`. The OS keeps as much of it in the page cache as
    /// memory allows and can evict it; resident memory counts the pages
    /// queries have touched. 0 before the index is first saved or loaded,
    /// and on Windows.
    #[serde(default)]
    pub mapped_postings_bytes: u64,
}

impl MemoryBreakdown {
    /// The figure for one part, by name (0 when absent).
    pub fn bytes(&self, name: &str) -> u64 {
        self.parts
            .iter()
            .find(|p| p.name == name)
            .map_or(0, |p| p.bytes)
    }
}

impl SearchEngine {
    /// Estimate the heap bytes of each structure. Walks every posting list,
    /// symbol and path once (tens of milliseconds at 300,000 files), under
    /// the caller's read lock; callers cache it per [`Self::generation`].
    pub fn memory_breakdown(&self) -> MemoryBreakdown {
        let (file_table, dir_index) = self.file_store.heap_bytes();
        let (graph_edges, graph_paths) = self.dependency_index.heap_bytes();
        let metadata = h::vec(&self.file_metadata)
            + self
                .file_metadata
                .iter()
                .map(FileMetadata::spelled_heap_bytes)
                .sum::<u64>()
            + h::vec(&self.indexed_meta)
            + h::bitmap(&self.racy_files);
        let symbols = h::vec(&self.symbol_cache)
            + self
                .symbol_cache
                .iter()
                .map(|v| h::vec(v) + v.iter().map(|s| h::string(&s.name)).sum::<u64>())
                .sum::<u64>()
            + self.symbol_names.heap_bytes();
        let references = h::vec(&self.reference_cache)
            + self.reference_cache.iter().map(h::vec).sum::<u64>()
            + h::vec(&self.ref_names)
            + self.ref_names.iter().map(h::string).sum::<u64>()
            + h::map(&self.ref_name_ids)
            + self.ref_name_ids.keys().map(h::string).sum::<u64>();
        // Import paths are the file store's (counted there).
        let spec = |(_, v): &(std::sync::Arc<std::path::Path>, Vec<String>)| {
            h::vec(v) + v.iter().map(h::string).sum::<u64>()
        };
        let imports = h::map(&self.import_specs)
            + self.import_specs.values().map(spec).sum::<u64>()
            + h::map(&self.spec_keys)
            + self
                .spec_keys
                .iter()
                .map(|(k, v)| h::string(k) + h::set(v))
                .sum::<u64>()
            + h::vec(&self.pending_imports)
            + self
                .pending_imports
                .iter()
                .map(|(_, _, v)| h::vec(v) + v.iter().map(h::string).sum::<u64>())
                .sum::<u64>()
            + h::vec(&self.waiting_imports)
            + self
                .waiting_imports
                .iter()
                .flatten()
                .map(|(_, _, s)| h::string(s))
                .sum::<u64>()
            + h::map(&self.waiting_keys)
            + self
                .waiting_keys
                .iter()
                .map(|(k, v)| h::string(k) + h::vec(v))
                .sum::<u64>()
            + h::vec(&self.waiting_local);
        let mut parts: Vec<MemoryPart> = [
            (
                "postings",
                "trigram posting lists",
                self.trigram_index.heap_bytes(),
            ),
            (
                "file_table",
                "indexed paths and the path lookup",
                file_table,
            ),
            ("dir_index", "files by folder (for the watcher)", dir_index),
            (
                "file_metadata",
                "ranking metadata and display paths",
                metadata,
            ),
            ("symbols", "symbol definitions", symbols),
            ("references", "symbol references", references),
            ("graph_edges", "dependency edges and counts", graph_edges),
            (
                "graph_paths",
                "path tables for import resolution",
                graph_paths,
            ),
            (
                "imports",
                "imports as written, and those still unresolved",
                imports,
            ),
        ]
        .into_iter()
        .map(|(name, what, bytes)| MemoryPart {
            name: name.to_string(),
            what: what.to_string(),
            bytes,
        })
        .collect();
        parts.sort_by_key(|p| std::cmp::Reverse(p.bytes));
        let total_bytes = parts.iter().map(|p| p.bytes).sum();
        MemoryBreakdown {
            parts,
            total_bytes,
            mapped_postings_bytes: self.trigram_index.mapped_bytes(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breakdown_grows_with_the_index_and_names_every_part() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let empty = SearchEngine::new().memory_breakdown();
        let mut engine = SearchEngine::new();
        engine.add_root_path(&root);
        let mut files = Vec::new();
        for i in 0..50 {
            let f = root.join(format!("m{i}.py"));
            std::fs::write(
                &f,
                format!("import os\nfrom m{} import thing\n\ndef handler_{i}(x):\n    return os.path.join(x)\n", (i + 1) % 50),
            )
            .unwrap();
            files.push(f);
        }
        engine.update_files(&files);
        let b = engine.memory_breakdown();
        assert_eq!(b.total_bytes, b.parts.iter().map(|p| p.bytes).sum::<u64>());
        assert!(b.total_bytes > empty.total_bytes);
        for part in ["postings", "file_table", "file_metadata", "symbols"] {
            assert!(b.bytes(part) > 0, "{part}: {b:?}");
        }
        assert!(
            b.parts.windows(2).all(|w| w[0].bytes >= w[1].bytes),
            "largest first"
        );
    }
}
