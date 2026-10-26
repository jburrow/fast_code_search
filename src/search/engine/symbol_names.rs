//! The distinct symbol names of the index, case-folded, with the files that
//! define each one.
//!
//! Symbol search used to test every symbol of every file whose text
//! contains the query: at 302,000 files a common name such as `Error`
//! meant 5.3 million symbols in 85,000 files, to find the 7,000 files that
//! define a matching one (95% of the search). Those 5.3 million symbols
//! have 1.6 million distinct folded names, 35 MB of text. One `memmem` pass
//! over that text finds the matching names, and their file lists are the
//! only files symbol search needs to look at.
//!
//! The dictionary is a filter, never the answer: the files it returns are
//! checked symbol by symbol as before, so results are the same. Folding is
//! one character to one character, so a name matches the query (exactly,
//! case-sensitively or not, or as a substring) only if its folded form
//! contains the folded query.
//!
//! It is built when the index is finalized or loaded. A file whose symbols
//! change after that is marked stale and always checked; when stale files
//! pile up the dictionary is rebuilt.

use crate::index::PersistedSymbolNames;
use crate::search::fold::fold_char;
use crate::symbols::{Symbol, SymbolType};
use memchr::memmem;
use roaring::RoaringBitmap;
use rustc_hash::FxHashMap;

/// Separates names in [`SymbolNameIndex::text`]. A query that contains it
/// is not filtered; a name that contains it marks its file stale.
const SEP: char = '\n';

/// A type, function or anything not below.
pub(super) const KIND_OTHER: u8 = 1;
/// A variable or constant.
pub(super) const KIND_VALUE: u8 = 2;
/// The file's own name.
pub(super) const KIND_FILE_NAME: u8 = 4;

fn kind_bit(kind: &SymbolType) -> u8 {
    match kind {
        SymbolType::Variable | SymbolType::Constant => KIND_VALUE,
        SymbolType::FileName => KIND_FILE_NAME,
        _ => KIND_OTHER,
    }
}

/// The largest of `factor(kind)` over the kinds set in `kinds`.
pub(super) fn kind_factor(kinds: u8, factor: impl Fn(&SymbolType) -> f64) -> f64 {
    [
        (KIND_OTHER, SymbolType::Function),
        (KIND_VALUE, SymbolType::Variable),
        (KIND_FILE_NAME, SymbolType::FileName),
    ]
    .iter()
    .filter(|(bit, _)| kinds & bit != 0)
    .map(|(_, kind)| factor(kind))
    .fold(f64::MIN, f64::max)
}

fn name_hash(name: &str) -> u64 {
    use std::hash::BuildHasher;
    #[cfg(test)]
    if tests::COLLIDE.with(|c| c.get()) {
        return 0;
    }
    rustc_hash::FxBuildHasher.hash_one(name)
}

/// Name `id` of a dictionary's `text` (without its separator).
fn name_at<'a>(text: &'a str, starts: &[u32], id: usize) -> &'a str {
    let end = starts.get(id + 1).map_or(text.len(), |&s| s as usize) - 1;
    &text[starts[id] as usize..end]
}

/// [`fold_str`] into `out`, without allocating once `out` has grown.
fn fold_into(s: &str, out: &mut String) {
    if s.is_ascii() {
        out.extend(s.bytes().map(|b| b.to_ascii_lowercase() as char));
    } else {
        out.extend(s.chars().map(fold_char));
    }
}

/// A name in the dictionary and the files that define it.
pub(super) struct NameHit<'a> {
    pub name: &'a str,
    pub files: &'a [u32],
    pub kinds: &'a [u8],
}

#[derive(Default)]
pub(super) struct SymbolNameIndex {
    /// Every distinct folded name, each followed by [`SEP`].
    text: String,
    /// Where each name starts in `text`, ascending.
    starts: Vec<u32>,
    /// The files defining name `i` are `files[file_ranges[i]..file_ranges[i + 1]]`.
    file_ranges: Vec<u32>,
    files: Vec<u32>,
    /// Parallel to `files`: which kinds of symbol the file defines under the
    /// name ([`KIND_OTHER`], [`KIND_VALUE`], [`KIND_FILE_NAME`]).
    kinds: Vec<u8>,
    /// Files at or past this id were added after the build: always checked.
    built_files: u32,
    /// Files below `built_files` whose symbols changed since: always checked.
    stale: RoaringBitmap,
    /// One past the highest new file id marked stale.
    files_seen: u32,
    /// False until the first build: while an index is first built (or
    /// loaded) every file is new and none of this is used yet.
    built: bool,
}

impl SymbolNameIndex {
    /// One thread on purpose: built on rayon's threads (1.2 s instead of
    /// 2.2 s at 302,000 files) the temporaries fragmented those threads'
    /// allocator arenas, and resident memory grew by 160 MB for a 63 MB
    /// dictionary.
    pub(super) fn build(cache: &[Vec<Symbol>]) -> Self {
        const NONE: u32 = u32::MAX;
        // Names by their 64-bit hash; the rare name whose hash another name
        // already holds goes to `collisions`.
        let mut ids: FxHashMap<u64, u32> = FxHashMap::default();
        let mut collisions: FxHashMap<String, u32> = FxHashMap::default();
        let mut text = String::new();
        let mut starts: Vec<u32> = Vec::new();
        // Each symbol's name id, in cache order (NONE: not in the dictionary).
        let mut symbol_ids: Vec<u32> = Vec::with_capacity(cache.iter().map(Vec::len).sum());
        let mut stale = RoaringBitmap::new();
        let mut buf = String::new();
        for (file, symbols) in cache.iter().enumerate() {
            for symbol in symbols {
                if symbol.name.contains(SEP) {
                    stale.insert(file as u32);
                    symbol_ids.push(NONE);
                    continue;
                }
                buf.clear();
                fold_into(&symbol.name, &mut buf);
                let add = |text: &mut String, starts: &mut Vec<u32>| {
                    starts.push(text.len() as u32);
                    text.push_str(&buf);
                    text.push(SEP);
                    starts.len() as u32 - 1
                };
                let hash = name_hash(&buf);
                let id = match ids.get(&hash) {
                    None => {
                        let id = add(&mut text, &mut starts);
                        ids.insert(hash, id);
                        id
                    }
                    Some(&id) if name_at(&text, &starts, id as usize) == buf => id,
                    Some(_) => match collisions.get(buf.as_str()) {
                        Some(&id) => id,
                        None => {
                            let id = add(&mut text, &mut starts);
                            collisions.insert(buf.clone(), id);
                            id
                        }
                    },
                };
                symbol_ids.push(id);
            }
        }
        drop((ids, collisions));
        let names = starts.len();

        // The file lists, filled in place: files come in order, so each
        // name's files arrive ascending and a repeat of a (name, file) pair
        // is always that name's last entry so far. Count, then fill.
        let pairs = || {
            cache.iter().enumerate().flat_map(|(file, symbols)| {
                symbols
                    .iter()
                    .map(move |s| (file as u32, kind_bit(&s.symbol_type)))
            })
        };
        let mut last_file = vec![NONE; names];
        let mut counts = vec![0u32; names];
        for (&id, (file, _)) in symbol_ids.iter().zip(pairs()) {
            if id != NONE && last_file[id as usize] != file {
                last_file[id as usize] = file;
                counts[id as usize] += 1;
            }
        }
        let mut file_ranges = Vec::with_capacity(names + 1);
        let mut total = 0u32;
        file_ranges.push(0);
        for count in &counts {
            total += count;
            file_ranges.push(total);
        }
        drop(counts);
        let mut files = vec![0u32; total as usize];
        let mut kinds = vec![0u8; total as usize];
        // The next free slot of each name's list.
        let mut fill: Vec<u32> = file_ranges[..names].to_vec();
        last_file.fill(NONE);
        for (&id, (file, kind)) in symbol_ids.iter().zip(pairs()) {
            if id == NONE {
                continue;
            }
            let id = id as usize;
            if last_file[id] == file {
                kinds[fill[id] as usize - 1] |= kind;
            } else {
                last_file[id] = file;
                files[fill[id] as usize] = file;
                kinds[fill[id] as usize] = kind;
                fill[id] += 1;
            }
        }
        Self {
            text,
            starts,
            file_ranges,
            files,
            kinds,
            built_files: cache.len() as u32,
            stale,
            files_seen: 0,
            built: true,
        }
    }

    /// Note that file `file`'s symbols changed (or it is new) since the
    /// build. Returns whether so many have that the dictionary should be
    /// rebuilt.
    pub(super) fn mark_stale(&mut self, file: u32) -> bool {
        if !self.built {
            return false;
        }
        if file < self.built_files {
            self.stale.insert(file);
        } else {
            self.files_seen = self.files_seen.max(file + 1);
        }
        let added = u64::from(self.files_seen.saturating_sub(self.built_files));
        // One rebuild per this many changed files keeps the rebuild (about
        // 2 s at 302,000 files) a small share of the updates themselves.
        self.stale.len() + added > (u64::from(self.built_files) / 16).max(4096)
    }

    /// How many files the dictionary covers (files from this id on are new).
    #[cfg(test)]
    pub(super) fn built_files(&self) -> u32 {
        self.built_files
    }

    /// The names containing `query_lower` (already folded), each once;
    /// `None` when the dictionary cannot tell (an empty query, or one with
    /// a line break). Covers only the files not in [`Self::unindexed`].
    pub(super) fn matching_names(&self, query_lower: &str) -> Option<Vec<NameHit<'_>>> {
        if query_lower.is_empty() || query_lower.contains(SEP) {
            return None;
        }
        // A match never spans SEP (the query has none), so each one lies in
        // the name that starts at or before it.
        let mut hits = Vec::new();
        let mut last = usize::MAX;
        for pos in memmem::find_iter(self.text.as_bytes(), query_lower.as_bytes()) {
            let id = self.starts.partition_point(|&s| s as usize <= pos) - 1;
            if id == last {
                continue;
            }
            last = id;
            let range = self.file_ranges[id] as usize..self.file_ranges[id + 1] as usize;
            hits.push(NameHit {
                name: name_at(&self.text, &self.starts, id),
                files: &self.files[range.clone()],
                kinds: &self.kinds[range],
            });
        }
        Some(hits)
    }

    /// Files out of `num_files` the dictionary does not describe: changed
    /// or added since it was built. They are checked symbol by symbol.
    pub(super) fn unindexed(&self, num_files: usize) -> RoaringBitmap {
        let mut out = self.stale.clone();
        if num_files as u32 > self.built_files {
            out.insert_range(self.built_files..num_files as u32);
        }
        out
    }

    /// Every file out of `num_files` that may define a symbol matching
    /// `query_lower`; `None` when the dictionary cannot tell.
    pub(super) fn files_matching(
        &self,
        query_lower: &str,
        num_files: usize,
    ) -> Option<RoaringBitmap> {
        let mut out = self.unindexed(num_files);
        for hit in self.matching_names(query_lower)? {
            out.extend(hit.files.iter().copied());
        }
        Some(out)
    }

    pub(super) fn is_built(&self) -> bool {
        self.built
    }

    /// For saving: file ids become positions (`position[id]`, `u32::MAX`
    /// for a file not saved). Files the dictionary does not describe out of
    /// `num_files` are listed as stale. `None` before the first build.
    pub(super) fn to_persisted(
        &self,
        position: &[u32],
        num_files: usize,
    ) -> Option<PersistedSymbolNames> {
        if !self.built {
            return None;
        }
        let pos = |id: u32| {
            position
                .get(id as usize)
                .copied()
                .filter(|&p| p != u32::MAX)
        };
        let (file_ranges, files, kinds) = self.remap(pos);
        Some(PersistedSymbolNames {
            text: self.text.clone(),
            starts: self.starts.clone(),
            file_ranges,
            files,
            kinds,
            stale: self.unindexed(num_files).iter().filter_map(pos).collect(),
        })
    }

    /// For loading: positions become ids (`id[position]`, `u32::MAX` for a
    /// file not loaded); the index has `num_files` files, all described.
    pub(super) fn from_persisted(
        saved: PersistedSymbolNames,
        id: &[u32],
        num_files: usize,
    ) -> Self {
        let map = |p: u32| id.get(p as usize).copied().filter(|&i| i != u32::MAX);
        let mut index = Self {
            text: saved.text,
            starts: saved.starts,
            file_ranges: saved.file_ranges,
            files: saved.files,
            kinds: saved.kinds,
            built_files: num_files as u32,
            stale: saved.stale.into_iter().filter_map(map).collect(),
            files_seen: 0,
            built: true,
        };
        let (file_ranges, files, kinds) = index.remap(map);
        index.file_ranges = file_ranges;
        index.files = files;
        index.kinds = kinds;
        index
    }

    /// The file lists with every file mapped through `map`, dropping files
    /// it has no entry for.
    fn remap(&self, map: impl Fn(u32) -> Option<u32>) -> (Vec<u32>, Vec<u32>, Vec<u8>) {
        let mut file_ranges = Vec::with_capacity(self.file_ranges.len());
        let mut files = Vec::with_capacity(self.files.len());
        let mut kinds = Vec::with_capacity(self.kinds.len());
        file_ranges.push(0);
        for name in 0..self.file_ranges.len().saturating_sub(1) {
            let range = self.file_ranges[name] as usize..self.file_ranges[name + 1] as usize;
            for (&file, &kind) in self.files[range.clone()].iter().zip(&self.kinds[range]) {
                if let Some(to) = map(file) {
                    files.push(to);
                    kinds.push(kind);
                }
            }
            file_ranges.push(files.len() as u32);
        }
        (file_ranges, files, kinds)
    }

    pub(super) fn heap_bytes(&self) -> u64 {
        (self.text.capacity()
            + 4 * (self.starts.capacity() + self.file_ranges.capacity() + self.files.capacity())
            + self.kinds.capacity()) as u64
            + self.stale.serialized_size() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::fold::fold_str;

    thread_local! {
        /// Every name hashes alike, to test the collision path.
        pub(super) static COLLIDE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    #[test]
    fn names_with_one_hash_stay_apart() {
        let cache = vec![file(&["alpha", "beta", "alpha"]), file(&["gamma", "beta"])];
        COLLIDE.with(|c| c.set(true));
        let idx = SymbolNameIndex::build(&cache);
        COLLIDE.with(|c| c.set(false));
        assert_eq!(idx.starts.len(), 3, "alpha, beta, gamma once each");
        assert_eq!(ids(idx.files_matching("beta", 2)), vec![0, 1]);
        assert_eq!(ids(idx.files_matching("alpha", 2)), vec![0]);
        assert_eq!(ids(idx.files_matching("gamma", 2)), vec![1]);
    }

    fn file(names: &[&str]) -> Vec<Symbol> {
        names
            .iter()
            .map(|n| Symbol {
                name: n.to_string(),
                symbol_type: SymbolType::Function,
                line: 0,
                column: 0,
                is_definition: true,
            })
            .collect()
    }

    fn ids(b: Option<RoaringBitmap>) -> Vec<u32> {
        b.expect("filtered").iter().collect()
    }

    #[test]
    fn finds_files_by_folded_substring() {
        let cache = vec![
            file(&["Request", "parse"]),
            file(&["isRequired", "Error"]),
            file(&["Ünïcode", "REQ"]),
            file(&[]),
        ];
        let idx = SymbolNameIndex::build(&cache);
        assert_eq!(ids(idx.files_matching("req", 4)), vec![0, 1, 2]);
        assert_eq!(ids(idx.files_matching("error", 4)), vec![1]);
        assert_eq!(ids(idx.files_matching(&fold_str("ÜNÏ"), 4)), vec![2]);
        assert_eq!(ids(idx.files_matching("nothing", 4)), Vec::<u32>::new());
        // A match never spans two names.
        assert_eq!(ids(idx.files_matching("tparse", 4)), Vec::<u32>::new());
        assert!(idx.files_matching("a\nb", 4).is_none());
    }

    #[test]
    fn stale_and_new_files_are_always_returned() {
        let cache = vec![file(&["alpha"]), file(&["beta"]), file(&["bad\nname"])];
        let mut idx = SymbolNameIndex::build(&cache);
        assert_eq!(ids(idx.files_matching("zzz", 3)), vec![2]);
        assert!(!idx.mark_stale(1));
        let mut empty = SymbolNameIndex::default();
        assert!(
            (0..10_000).all(|f| !empty.mark_stale(f)),
            "no rebuilds before the first build"
        );
        assert_eq!(ids(idx.files_matching("alpha", 5)), vec![0, 1, 2, 3, 4]);
    }

    /// Case-insensitive symbol search scores files from the dictionary
    /// alone, so a folded name must contain the folded query exactly when
    /// the per-symbol check accepts the name.
    #[test]
    fn folded_names_match_exactly_when_the_symbol_check_does() {
        let names = [
            "Kelvin\u{212A}",
            "ſtate",
            "STATE",
            "Straße",
            "STRASSE",
            "ΣΊΣΥΦΟΣ",
            "σίσυφος",
            "plain",
            "İstanbul",
            "ǅemal",
            "ﬁle",
            "Ωmega",
            "\u{2126}hm",
            "MiXeD_Case",
        ];
        let cache: Vec<Vec<Symbol>> = names.iter().map(|n| file(&[n])).collect();
        let idx = SymbolNameIndex::build(&cache);
        let queries = [
            "k", "K", "s", "ST", "state", "ß", "ss", "σ", "Σ", "ς", "ίσυ", "ain", "i", "İ", "ǆ",
            "ǅ", "fi", "ﬁ", "ω", "Ω", "mixed_c", "x",
        ];
        for query in queries {
            let q = fold_str(query).into_owned();
            let got = idx.files_matching(&q, names.len()).unwrap();
            for (i, n) in names.iter().enumerate() {
                let check = super::super::text::contains_case_insensitive(n, &q);
                assert_eq!(got.contains(i as u32), check, "{query:?} in {n:?}");
                if n.contains(query) {
                    assert!(check, "case-sensitive {query:?} in {n:?}");
                }
            }
        }
    }

    #[test]
    fn kinds_are_merged_per_name_and_file() {
        let mut syms = file(&["value", "value", "thing"]);
        syms[0].symbol_type = SymbolType::Constant;
        let idx = SymbolNameIndex::build(&[syms]);
        let hits = idx.matching_names("value").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "value");
        assert_eq!(hits[0].files, &[0]);
        assert_eq!(hits[0].kinds, &[KIND_VALUE | KIND_OTHER]);
        let f = |k: &SymbolType| match k {
            SymbolType::Constant | SymbolType::Variable => 0.9,
            SymbolType::FileName => 0.001,
            _ => 1.0,
        };
        assert_eq!(kind_factor(KIND_VALUE | KIND_OTHER, f), 1.0);
        assert_eq!(kind_factor(KIND_VALUE, f), 0.9);
        assert_eq!(kind_factor(KIND_FILE_NAME | KIND_VALUE, f), 0.9);
    }

    #[test]
    fn save_and_load_map_files_through_positions() {
        // Files 0..4; 1 is not saved (removed), 3 is stale, 4 is new.
        let cache = vec![
            file(&["alpha"]),
            file(&["alpha", "beta"]),
            file(&["beta"]),
            file(&["alpha"]),
        ];
        let mut idx = SymbolNameIndex::build(&cache);
        idx.mark_stale(3);
        idx.mark_stale(4);
        let position = [0, u32::MAX, 1, 2, 3];
        let saved = idx.to_persisted(&position, 5).unwrap();
        assert_eq!(saved.stale, vec![2, 3]);
        // Loaded with the positions as ids 10.. (a file of 13).
        let loaded = SymbolNameIndex::from_persisted(saved, &[10, 11, 12, 13], 14);
        assert!(loaded.is_built());
        let names = |q: &str| -> Vec<(String, Vec<u32>)> {
            loaded
                .matching_names(q)
                .unwrap()
                .into_iter()
                .map(|h| (h.name.to_string(), h.files.to_vec()))
                .collect()
        };
        assert_eq!(names("alpha"), vec![("alpha".to_string(), vec![10, 12])]);
        assert_eq!(names("beta"), vec![("beta".to_string(), vec![11])]);
        assert_eq!(
            loaded.unindexed(14).iter().collect::<Vec<_>>(),
            vec![12, 13]
        );
        assert!(SymbolNameIndex::default().to_persisted(&[], 0).is_none());
    }
}
