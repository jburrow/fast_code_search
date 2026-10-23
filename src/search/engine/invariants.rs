//! Self-checks: does the index agree with itself, and with the disk?
//!
//! A posting list that misses a file is a match that is never shown; a
//! path map that disagrees with the folder index is a change the watcher
//! applies to the wrong file. Neither shows up as an error: searches just
//! answer less. These checks make such states visible. They run after
//! every step of the watch, fault and model tests, in `fcs check --server`
//! against a live server, and (a sample of files at a time) in the server's
//! background scrub.

use super::*;
use std::collections::BTreeMap;

/// Examples kept per kind of violation; the count covers them all.
const EXAMPLES_PER_KIND: usize = 5;

/// What [`SearchEngine::check_invariants`] found.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InvariantReport {
    /// Live files checked.
    pub files: usize,
    /// Problems by kind (`posting_to_removed_file`, `path_map`, `graph`, …).
    pub counts: BTreeMap<String, usize>,
    /// A few examples of each kind. They name files and paths.
    pub examples: BTreeMap<String, Vec<String>>,
}

impl InvariantReport {
    /// True when nothing is wrong.
    pub fn is_clean(&self) -> bool {
        self.counts.is_empty()
    }

    /// Problems of every kind.
    pub fn total(&self) -> usize {
        self.counts.values().sum()
    }

    fn push(&mut self, kind: &str, detail: String) {
        *self.counts.entry(kind.to_string()).or_default() += 1;
        let examples = self.examples.entry(kind.to_string()).or_default();
        if examples.len() < EXAMPLES_PER_KIND {
            examples.push(detail);
        }
    }
}

impl std::fmt::Display for InvariantReport {
    /// One line per kind with its examples, or "clean".
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_clean() {
            return write!(f, "index invariants hold for {} files", self.files);
        }
        writeln!(
            f,
            "{} index invariant violations over {} files:",
            self.total(),
            self.files
        )?;
        for (kind, n) in &self.counts {
            writeln!(f, "  {kind}: {n}")?;
            for e in self.examples.get(kind).into_iter().flatten() {
                writeln!(f, "    {e}")?;
            }
        }
        Ok(())
    }
}

/// How one indexed file compares with the disk (see
/// [`SearchEngine::check_file`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileCheck {
    /// Unchanged since it was indexed, and every trigram of its content is
    /// in the index.
    Current,
    /// Changed on disk since it was indexed (a change still to apply, or
    /// one that was lost).
    Changed,
    /// Gone from disk but still in the index.
    Gone,
    /// Could not be read.
    Unreadable,
    /// Unchanged, but no longer something the indexer would index (binary
    /// or too large).
    NotIndexable,
    /// Unchanged, but this many of its trigrams do not list it: searches for
    /// text containing them never look at this file.
    MissingTrigrams(usize),
}

impl SearchEngine {
    /// Check that the index agrees with itself: the posting lists name only
    /// live files and every live file is a candidate for queries with no
    /// trigram; path lookups, the folder index and display paths agree; the
    /// symbol, reference and import caches hold nothing for removed files;
    /// and the dependency graph's two directions and counts agree and join
    /// live files. Reads no file; about a second on a million files.
    pub fn check_invariants(&self) -> InvariantReport {
        let mut report = InvariantReport::default();
        let mut found: Vec<(&'static str, String)> = Vec::new();
        self.file_store.check_consistency(&mut found);
        for (kind, detail) in found.drain(..) {
            report.push(kind, detail);
        }

        let live: roaring::RoaringBitmap = self.file_store.live_ids().collect();
        report.files = live.len() as usize;
        let name = |id: u32| {
            self.file_store
                .get_path(id)
                .map(|p| self.make_display_path(p))
                .unwrap_or_else(|| format!("#{id}"))
        };

        // Postings: a removed file named by a posting list is a candidate
        // that can't be read; a live file missing from the all-documents
        // set is never scanned by a query without trigrams.
        for (trigram, docs) in self.trigram_index.get_trigram_map() {
            if !docs.is_subset(&live) {
                let dead = docs - &live;
                report.push(
                    "posting_to_removed_file",
                    format!(
                        "trigram {:?} lists {} removed files (e.g. #{})",
                        String::from_utf8_lossy(&trigram.as_bytes()),
                        dead.len(),
                        dead.min().unwrap_or(0)
                    ),
                );
            }
        }
        let all = self.trigram_index.all_documents();
        if *all != live {
            for id in (&live - &*all).iter() {
                report.push(
                    "file_not_in_all_documents",
                    format!("{} is never scanned by short queries", name(id)),
                );
            }
            for id in (&*all - &live).iter() {
                report.push(
                    "removed_file_in_all_documents",
                    format!("removed file #{id} is still scanned by short queries"),
                );
            }
        }

        // Display paths name one file each (two roots with the same name
        // used to share them, and the wrong file opened).
        let mut shown: FxHashMap<String, u32> = FxHashMap::default();
        for id in live.iter() {
            let display = name(id);
            if let Some(other) = shown.insert(display.clone(), id) {
                report.push(
                    "display_path_clash",
                    format!("{display} is shown for files #{other} and #{id}"),
                );
            }
        }

        // Per-file caches hold nothing for removed files.
        let is_live = |id: u32| live.contains(id);
        for (id, symbols) in self.symbol_cache.iter().enumerate() {
            if !symbols.is_empty() && !is_live(id as u32) {
                report.push(
                    "cache_for_removed_file",
                    format!("removed file #{id} keeps {} symbols", symbols.len()),
                );
            }
        }
        for (id, refs) in self.reference_cache.iter().enumerate() {
            if !refs.is_empty() && !is_live(id as u32) {
                report.push(
                    "cache_for_removed_file",
                    format!("removed file #{id} keeps {} references", refs.len()),
                );
            }
        }
        for &id in self.import_specs.keys() {
            if !is_live(id) {
                report.push(
                    "cache_for_removed_file",
                    format!("removed file #{id} keeps its imports"),
                );
            }
        }

        // The graph: both directions, counts, and only live files.
        self.dependency_index.check_consistency(is_live, &mut found);
        for (kind, detail) in found.drain(..) {
            report.push(kind, detail);
        }
        for id in live.iter() {
            if self.dependency_index.path_of(id).is_none() {
                report.push(
                    "graph",
                    format!("{} is not registered for import resolution", name(id)),
                );
            }
        }
        report
    }

    /// Compare one indexed file with the disk: is it unchanged since it was
    /// indexed, and if so does every trigram of its content (and name) list
    /// it? `None` if `id` is not a live file. Reads the file.
    pub fn check_file(&self, id: u32) -> Option<FileCheck> {
        let path = self.file_store.get_path(id)?;
        let meta = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Some(FileCheck::Gone),
            Err(_) => return Some(FileCheck::Unreadable),
        };
        let indexed = self
            .indexed_meta
            .get(id as usize)
            .copied()
            .unwrap_or((0, 0));
        let on_disk = (crate::index::persistence::mtime_secs_of(&meta), meta.len());
        // (0, 0): what was indexed was not recorded; compare the content.
        if indexed != (0, 0) && indexed != on_disk {
            return Some(FileCheck::Changed);
        }
        let partial = match PartialIndexedFile::process_checked(
            path,
            self.transcode_non_utf8,
            self.max_file_size,
        ) {
            Ok(Some((partial, _))) => partial,
            Ok(None) => return Some(FileCheck::NotIndexable),
            Err(_) => return Some(FileCheck::Unreadable),
        };
        let postings = self.trigram_index.get_trigram_map();
        let missing = partial
            .trigrams
            .iter()
            .filter(|t| !postings.get(t).is_some_and(|docs| docs.contains(id)))
            .count();
        Some(if missing == 0 {
            FileCheck::Current
        } else if indexed == (0, 0) {
            // Unknown indexed version: missing trigrams may just be a
            // change still to apply.
            FileCheck::Changed
        } else {
            FileCheck::MissingTrigrams(missing)
        })
    }

    /// Ids of every live file, in order.
    pub fn live_file_ids(&self) -> Vec<u32> {
        self.file_store.live_ids().collect()
    }
}
