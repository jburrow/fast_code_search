//! Every file's symbols, packed.
//!
//! A [`Symbol`] is 48 bytes plus its own heap copy of its name. At 302,000
//! files that was 386 MB, 28% of the server's heap, for 8 million symbols
//! that have under 2 million distinct names. Here each name is stored once,
//! in one string, and a symbol is 16 bytes: the name's number, its line,
//! column, kind and whether it is a definition.
//!
//! Names are counted by the symbols that use them. A name no symbol uses
//! any more stays in the string until the dead outnumber the living, when
//! the table is rebuilt without them.

use super::{Symbol, SymbolType};
use hashbrown::HashTable;
use serde::{Deserialize, Serialize};
use std::hash::BuildHasher;

/// One symbol, with its name as a number in the store's name table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackedSymbol {
    pub name: u32,
    pub line: u32,
    pub column: u32,
    pub kind: u8,
    pub is_definition: bool,
}

/// A symbol as callers see it: the name borrowed from the table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SymbolView<'a> {
    pub name: &'a str,
    pub symbol_type: SymbolType,
    pub line: usize,
    pub column: usize,
    pub is_definition: bool,
}

impl SymbolView<'_> {
    pub fn to_symbol(&self) -> Symbol {
        Symbol {
            name: self.name.to_string(),
            symbol_type: self.symbol_type,
            line: self.line,
            column: self.column,
            is_definition: self.is_definition,
        }
    }
}

fn clamp(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn hash(name: &str) -> u64 {
    rustc_hash::FxBuildHasher.hash_one(name)
}

/// Distinct names back to back, with a hash table from name to number.
#[derive(Default)]
struct Names {
    text: String,
    /// Where name `i` starts in `text`; it ends where the next one starts.
    starts: Vec<u32>,
    /// How many symbols use name `i`.
    uses: Vec<u32>,
    table: HashTable<u32>,
    /// Names no symbol uses.
    dead: usize,
}

impl Names {
    fn get(&self, id: u32) -> &str {
        name_at(&self.text, &self.starts, id as usize)
    }

    fn intern(&mut self, name: &str) -> u32 {
        let Self {
            text,
            starts,
            uses,
            table,
            dead,
        } = self;
        let h = hash(name);
        let id = match table.find(h, |&id| name_at(text, starts, id as usize) == name) {
            Some(&id) => id,
            None => {
                let id = clamp(starts.len());
                starts.push(u32::try_from(text.len()).expect("symbol names over 4 GiB"));
                text.push_str(name);
                uses.push(0);
                *dead += 1;
                table.insert_unique(h, id, |&id| hash(name_at(text, starts, id as usize)));
                id
            }
        };
        let n = &mut uses[id as usize];
        if *n == 0 {
            *dead -= 1;
        }
        *n += 1;
        id
    }

    fn release(&mut self, id: u32) {
        let n = &mut self.uses[id as usize];
        *n -= 1;
        if *n == 0 {
            self.dead += 1;
        }
    }

    fn heap_bytes(&self) -> u64 {
        (self.text.capacity()
            + (self.starts.capacity() + self.uses.capacity()) * 4
            // A slot and a control byte per bucket.
            + self.table.capacity() * 5) as u64
    }
}

fn name_at<'a>(text: &'a str, starts: &[u32], id: usize) -> &'a str {
    let end = starts.get(id + 1).map_or(text.len(), |&s| s as usize);
    &text[starts[id] as usize..end]
}

/// One file's symbols.
#[derive(Clone, Copy)]
pub struct FileSymbols<'a> {
    symbols: &'a [PackedSymbol],
    names: &'a Names,
}

impl<'a> FileSymbols<'a> {
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = SymbolView<'a>> + Clone + use<'a> {
        let names = self.names;
        self.symbols.iter().map(move |s| SymbolView {
            name: names.get(s.name),
            symbol_type: SymbolType::from_index(s.kind),
            line: s.line as usize,
            column: s.column as usize,
            is_definition: s.is_definition,
        })
    }

    pub fn to_vec(&self) -> Vec<Symbol> {
        self.iter().map(|s| s.to_symbol()).collect()
    }
}

/// The symbols of every file, by file id.
#[derive(Default)]
pub struct SymbolStore {
    files: Vec<Box<[PackedSymbol]>>,
    names: Names,
}

impl SymbolStore {
    /// Number of file slots (one past the highest id ever stored).
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// File `id`'s symbols; `None` past the last slot.
    pub fn get(&self, id: u32) -> Option<FileSymbols<'_>> {
        self.files.get(id as usize).map(|s| FileSymbols {
            symbols: s,
            names: &self.names,
        })
    }

    /// File `id`'s symbols, empty past the last slot.
    pub fn of(&self, id: u32) -> FileSymbols<'_> {
        FileSymbols {
            symbols: self.files.get(id as usize).map_or(&[], |s| s),
            names: &self.names,
        }
    }

    pub fn count(&self, id: u32) -> usize {
        self.files.get(id as usize).map_or(0, |s| s.len())
    }

    /// Every symbol with its file id, in id order. Equal names have equal
    /// name numbers.
    pub fn symbols(&self) -> impl Iterator<Item = (u32, &PackedSymbol)> {
        self.files
            .iter()
            .enumerate()
            .flat_map(|(id, syms)| syms.iter().map(move |s| (id as u32, s)))
    }

    /// Name number `id`, as [`Self::symbols`] gives it.
    pub fn name(&self, id: u32) -> &str {
        self.names.get(id)
    }

    /// Name numbers run below this.
    pub fn name_count(&self) -> usize {
        self.names.starts.len()
    }

    /// An empty store with `files` empty slots.
    pub fn with_slots(files: usize) -> Self {
        Self {
            files: vec![Box::default(); files],
            names: Names::default(),
        }
    }

    /// Replace file `id`'s symbols.
    pub fn set(&mut self, id: u32, symbols: Vec<Symbol>) {
        let idx = id as usize;
        if self.files.len() <= idx {
            self.files.resize_with(idx + 1, Box::default);
        }
        let packed: Box<[PackedSymbol]> = symbols
            .iter()
            .map(|s| PackedSymbol {
                name: self.names.intern(&s.name),
                line: clamp(s.line),
                column: clamp(s.column),
                kind: s.symbol_type.index(),
                is_definition: s.is_definition,
            })
            .collect();
        let old = std::mem::replace(&mut self.files[idx], packed);
        for s in old.iter() {
            self.names.release(s.name);
        }
        self.collect_dead_names();
    }

    /// Add one symbol to file `id`'s.
    #[cfg(test)]
    pub(crate) fn push(&mut self, id: u32, symbol: Symbol) {
        let mut symbols = self.of(id).to_vec();
        symbols.push(symbol);
        self.set(id, symbols);
    }

    pub fn clear(&mut self, id: u32) {
        if let Some(slot) = self.files.get_mut(id as usize) {
            for s in std::mem::take(slot).iter() {
                self.names.release(s.name);
            }
        }
        self.collect_dead_names();
    }

    /// Rebuild the name table without unused names once they outnumber
    /// the used ones (and are worth a pass over every symbol).
    fn collect_dead_names(&mut self) {
        let live = self.names.starts.len() - self.names.dead;
        if self.names.dead > 4096 && self.names.dead > live {
            self.rebuild_names();
        }
    }

    /// Rebuild the name table from the names in use. Names get new numbers
    /// in order of first use.
    fn rebuild_names(&mut self) {
        const UNSET: u32 = u32::MAX;
        let old = std::mem::take(&mut self.names);
        let mut new_id = vec![UNSET; old.starts.len()];
        for syms in &mut self.files {
            for s in syms.iter_mut() {
                let slot = &mut new_id[s.name as usize];
                if *slot == UNSET {
                    *slot = self.names.intern(old.get(s.name));
                } else {
                    self.names.uses[*slot as usize] += 1;
                }
                s.name = *slot;
            }
        }
    }

    pub fn shrink_to_fit(&mut self) {
        self.files.shrink_to_fit();
        let n = &mut self.names;
        n.text.shrink_to_fit();
        n.starts.shrink_to_fit();
        n.uses.shrink_to_fit();
        n.table
            .shrink_to_fit(|&id| hash(name_at(&n.text, &n.starts, id as usize)));
    }

    pub fn heap_bytes(&self) -> u64 {
        let files = self.files.capacity() * std::mem::size_of::<Box<[PackedSymbol]>>()
            + self
                .files
                .iter()
                .map(|s| std::mem::size_of_val::<[PackedSymbol]>(s))
                .sum::<usize>();
        files as u64 + self.names.heap_bytes()
    }

    /// The symbols of the files in `slots` (a file id, or `None` for an
    /// empty position), for saving. Only names those files use are kept.
    pub fn to_persisted(&self, slots: &[Option<u32>]) -> PersistedSymbols {
        const UNSET: u32 = u32::MAX;
        let mut new_id = vec![UNSET; self.names.starts.len()];
        let mut out = PersistedSymbols::default();
        out.file_ranges.reserve(slots.len() + 1);
        out.file_ranges.push(0);
        for slot in slots {
            for s in slot.map_or(&[][..], |id| self.of(id).symbols) {
                let id = &mut new_id[s.name as usize];
                if *id == UNSET {
                    *id = clamp(out.name_starts.len());
                    out.name_starts.push(out.names.len() as u32);
                    out.names.push_str(self.names.get(s.name));
                }
                out.symbols.push(PackedSymbol { name: *id, ..*s });
            }
            out.file_ranges.push(clamp(out.symbols.len()));
        }
        out
    }

    /// The store of a loaded index: `place(position)` is the id of the
    /// file saved at that position, if it is still indexed. Positions are
    /// validated by [`PersistedSymbols::validate`].
    pub fn from_persisted(
        saved: PersistedSymbols,
        files: usize,
        place: impl Fn(usize) -> Option<u32>,
    ) -> Self {
        let mut store = Self::with_slots(files);
        let PersistedSymbols {
            names,
            name_starts,
            file_ranges,
            symbols,
        } = saved;
        let mut uses = vec![0u32; name_starts.len()];
        for (pos, range) in file_ranges.windows(2).enumerate() {
            let Some(id) = place(pos).filter(|&id| (id as usize) < files) else {
                continue;
            };
            let syms = &symbols[range[0] as usize..range[1] as usize];
            for s in syms {
                uses[s.name as usize] += 1;
            }
            store.files[id as usize] = syms.into();
        }
        let mut table = HashTable::with_capacity(name_starts.len());
        for id in 0..name_starts.len() {
            let h = hash(name_at(&names, &name_starts, id));
            table.insert_unique(h, id as u32, |&i| {
                hash(name_at(&names, &name_starts, i as usize))
            });
        }
        let dead = uses.iter().filter(|&&n| n == 0).count();
        store.names = Names {
            text: names,
            starts: name_starts,
            uses,
            table,
            dead,
        };
        store.collect_dead_names();
        store
    }
}

/// Every file's symbols as an index saves them: by file position, with
/// each distinct name once.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct PersistedSymbols {
    /// The distinct names, back to back.
    pub names: String,
    /// Where each name starts in `names`.
    pub name_starts: Vec<u32>,
    /// The file at position `i` has `symbols[file_ranges[i]..file_ranges[i + 1]]`.
    /// Empty when the index was saved without files.
    pub file_ranges: Vec<u32>,
    pub symbols: Vec<PackedSymbol>,
}

impl PersistedSymbols {
    /// No file positions (an index with no files).
    pub fn is_empty(&self) -> bool {
        self.file_ranges.len() <= 1
    }

    /// Position `pos`'s symbols.
    pub fn file(&self, pos: usize) -> Vec<Symbol> {
        let (Some(&a), Some(&b)) = (self.file_ranges.get(pos), self.file_ranges.get(pos + 1))
        else {
            return Vec::new();
        };
        self.symbols[a as usize..b as usize]
            .iter()
            .map(|s| Symbol {
                name: name_at(&self.names, &self.name_starts, s.name as usize).to_string(),
                symbol_type: SymbolType::from_index(s.kind),
                line: s.line as usize,
                column: s.column as usize,
                is_definition: s.is_definition,
            })
            .collect()
    }

    /// Pack files given by position.
    pub fn from_files(files: &[Vec<Symbol>]) -> Self {
        let mut store = SymbolStore::default();
        for (id, syms) in files.iter().enumerate() {
            store.set(id as u32, syms.clone());
        }
        let slots: Vec<Option<u32>> = (0..files.len() as u32).map(Some).collect();
        store.to_persisted(&slots)
    }

    /// Whether every offset and number is in range, so that reading the
    /// symbols cannot panic.
    pub fn validate(&self) -> bool {
        let starts_ok = self.name_starts.first().is_none_or(|&s| s == 0)
            && self.name_starts.windows(2).all(|w| w[0] <= w[1])
            && self.name_starts.iter().all(|&s| {
                (s as usize) <= self.names.len() && self.names.is_char_boundary(s as usize)
            });
        let ranges_ok = self.file_ranges.first().is_none_or(|&r| r == 0)
            && self.file_ranges.windows(2).all(|w| w[0] <= w[1])
            && self
                .file_ranges
                .last()
                .is_none_or(|&r| r as usize == self.symbols.len());
        let names = self.name_starts.len();
        starts_ok
            && ranges_ok
            && self
                .symbols
                .iter()
                .all(|s| (s.name as usize) < names && SymbolType::valid_index(s.kind))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym(name: &str, line: usize) -> Symbol {
        Symbol {
            name: name.to_string(),
            symbol_type: SymbolType::Function,
            line,
            column: 4,
            is_definition: true,
        }
    }

    fn names(store: &SymbolStore, id: u32) -> Vec<String> {
        store.of(id).iter().map(|s| s.name.to_string()).collect()
    }

    #[test]
    fn kinds_round_trip() {
        for i in 0..15 {
            assert_eq!(SymbolType::from_index(i).index(), i);
        }
        assert!(!SymbolType::valid_index(15));
    }

    #[test]
    fn stores_each_name_once() {
        let mut store = SymbolStore::default();
        store.set(0, vec![sym("parse", 1), sym("Error", 2)]);
        store.set(2, vec![sym("Error", 7), sym("parse", 9)]);
        assert_eq!(store.len(), 3);
        assert_eq!(store.name_count(), 2);
        assert_eq!(names(&store, 2), ["Error", "parse"]);
        assert!(store.of(1).is_empty() && store.of(99).is_empty());
        assert!(store.get(99).is_none());
        let s = store.of(2).iter().nth(1).unwrap().to_symbol();
        assert_eq!((s.line, s.column, s.is_definition), (9, 4, true));
    }

    #[test]
    fn unused_names_are_dropped_once_they_outnumber_used_ones() {
        let mut store = SymbolStore::default();
        store.set(0, vec![sym("keep", 0)]);
        for round in 0..3 {
            let batch: Vec<Symbol> = (0..3000)
                .map(|i| sym(&format!("n{round}_{i}"), i))
                .collect();
            store.set(1, batch);
        }
        // 9,000 names made, 3,001 in use: 5,999 dead, more than the living
        // and over the 4,096 floor, so the table was rebuilt.
        assert!(store.name_count() < 9001, "{}", store.name_count());
        assert_eq!(names(&store, 0), ["keep"]);
        assert_eq!(store.of(1).iter().next().unwrap().name, "n2_0");
        store.clear(1);
        store.set(1, vec![sym("keep", 3)]);
        assert_eq!(names(&store, 1), ["keep"]);
    }

    #[test]
    fn round_trips_through_persisted_positions() {
        let mut store = SymbolStore::default();
        store.set(0, vec![sym("a", 1)]);
        store.set(1, vec![sym("gone", 1)]);
        store.set(2, vec![sym("b", 2), sym("a", 3)]);
        let saved = store.to_persisted(&[Some(2), None, Some(0)]);
        assert!(saved.validate());
        assert_eq!(saved.name_starts.len(), 2, "only names the saved files use");
        assert_eq!(saved.file(0).len(), 2);
        let loaded = SymbolStore::from_persisted(saved, 4, |pos| [Some(3), None, Some(0)][pos]);
        assert_eq!(names(&loaded, 3), ["b", "a"]);
        assert_eq!(names(&loaded, 0), ["a"]);
        assert!(loaded.of(1).is_empty() && loaded.of(2).is_empty());
    }

    #[test]
    fn rejects_out_of_range_offsets() {
        let good = PersistedSymbols::from_files(&[vec![sym("é", 0), sym("x", 1)]]);
        assert!(good.validate());
        let mut bad = good.clone();
        bad.symbols[0].name = 9;
        assert!(!bad.validate());
        let mut bad = good.clone();
        bad.name_starts[1] = 1; // inside "é"
        assert!(!bad.validate());
        let mut bad = good.clone();
        bad.file_ranges[1] = 5;
        assert!(!bad.validate());
        let mut bad = good;
        bad.symbols[0].kind = 200;
        assert!(!bad.validate());
    }
}
