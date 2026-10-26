use roaring::RoaringBitmap;
use rustc_hash::{FxHashMap, FxHashSet};
use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use super::persistence::{DirEntry, PersistedTrigramIndex, Posting};

/// Maximum initial capacity for trigram sets.
/// Limits memory pre-allocation for very large files.
const MAX_INITIAL_TRIGRAM_CAPACITY: usize = 1024;

/// A trigram is a sequence of 3 characters
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Trigram([u8; 3]);

impl Trigram {
    pub fn new(bytes: [u8; 3]) -> Self {
        Trigram(bytes)
    }

    /// Get the underlying bytes
    pub fn as_bytes(&self) -> [u8; 3] {
        self.0
    }
}

/// Extract trigrams from text
pub fn extract_trigrams(text: &str) -> Vec<Trigram> {
    let bytes = text.as_bytes();
    let len = bytes.len().saturating_sub(2);
    let mut trigrams = Vec::with_capacity(len);

    for i in 0..len {
        // Direct construction is safe since we know we have at least 3 bytes
        trigrams.push(Trigram([bytes[i], bytes[i + 1], bytes[i + 2]]));
    }

    trigrams
}

/// Extract unique trigrams from text directly into a FxHashSet.
/// More efficient than extracting to Vec and then deduplicating.
#[inline]
pub fn extract_unique_trigrams(text: &str) -> FxHashSet<Trigram> {
    unique_trigrams_folded(text.as_bytes(), false)
}

/// Unique trigrams of `text` **case-folded the way the index expects**
/// (`search::fold::fold_str`, Unicode simple case folding), without
/// materializing a folded copy of the whole buffer when the text is ASCII
/// (the overwhelmingly common case: the fold happens per byte during
/// extraction). Non-ASCII text goes through `fold_str` so it matches the
/// query side exactly.
pub fn extract_unique_trigrams_lowercase(text: &str) -> FxHashSet<Trigram> {
    if text.is_ascii() {
        unique_trigrams_folded(text.as_bytes(), true)
    } else {
        unique_trigrams_folded(crate::search::fold::fold_str(text).as_bytes(), false)
    }
}

// 2^24 bits (2 MiB) per thread: one bit per possible trigram. Deduping
// through a bitset costs one test-and-set per byte instead of a hash
// insert per byte; only the bits actually set are cleared afterwards, so a
// small file pays only for its own trigrams.
thread_local! {
    static TRIGRAM_BITSET: std::cell::RefCell<Vec<u64>> =
        std::cell::RefCell::new(vec![0u64; (1usize << 24) / 64]);
}

#[inline]
fn unique_trigrams_folded(bytes: &[u8], ascii_fold: bool) -> FxHashSet<Trigram> {
    let len = bytes.len().saturating_sub(2);
    if len == 0 {
        return FxHashSet::default();
    }
    TRIGRAM_BITSET.with(|cell| {
        let mut bits = cell.borrow_mut();
        let mut unique: Vec<Trigram> = Vec::with_capacity(len.min(MAX_INITIAL_TRIGRAM_CAPACITY));
        let fold = |b: u8| {
            if ascii_fold {
                b.to_ascii_lowercase()
            } else {
                b
            }
        };
        let mut b0 = fold(bytes[0]);
        let mut b1 = fold(bytes[1]);
        for &raw in &bytes[2..] {
            let b2 = fold(raw);
            let key = ((b0 as usize) << 16) | ((b1 as usize) << 8) | b2 as usize;
            let (word, bit) = (key >> 6, 1u64 << (key & 63));
            if bits[word] & bit == 0 {
                bits[word] |= bit;
                unique.push(Trigram([b0, b1, b2]));
            }
            b0 = b1;
            b1 = b2;
        }
        // Reset only what we touched.
        for t in &unique {
            let key = ((t.0[0] as usize) << 16) | ((t.0[1] as usize) << 8) | t.0[2] as usize;
            bits[key >> 6] &= !(1u64 << (key & 63));
        }
        let mut set = FxHashSet::with_capacity_and_hasher(unique.len(), Default::default());
        set.extend(unique);
        set
    })
}

/// Posting lists are split across this many maps by trigram, so a batch is
/// merged by one thread per shard ([`TrigramIndex::add_documents`]). The
/// merge used to run on one thread and was half of a build's time on the
/// real corpus (229 of 430 s at 302,000 files on 4 cores), four fifths of it
/// in posting-list inserts. 64 gives every core several shards to balance
/// over, and the shards stay large (about 5,500 lists each on that corpus).
const SHARDS: usize = 64;

/// Which shard holds `trigram`: the top bits of a multiplicative hash, so
/// shards are even however skewed the trigrams are.
#[inline]
fn shard_of(trigram: &Trigram) -> usize {
    let [a, b, c] = trigram.0;
    (u32::from_le_bytes([a, b, c, 0]).wrapping_mul(0x9E37_79B9) >> 26) as usize
}

/// Inverted index mapping trigrams to document IDs using roaring bitmaps.
///
/// After a load or a checkpoint the posting lists live in the index file,
/// mapped (the base), and a query decodes only the lists it needs: they are
/// page cache the OS can evict, not heap (C1 of the scale plan). Lists for
/// documents indexed since are held on the heap (`shards`) and united with
/// the mapped ones. A document removed or re-indexed since the file was
/// written goes into the base's `void` set, and every mapped list is read
/// without it, so removing a document costs one bit, not a pass over every
/// list.
pub struct TrigramIndex {
    // Trigram -> documents containing it, split by `shard_of`. FxHashMap is
    // faster than std HashMap for small keys like Trigram. With a base,
    // only the documents indexed since it was written.
    shards: Vec<FxHashMap<Trigram, RoaringBitmap>>,
    // Every document added and not removed, whether or not it has any
    // trigram: the candidates for a query with no trigram of its own (a
    // short query, a regex with no literal). It used to be the union of the
    // posting lists, so a file shorter than three bytes with a short name
    // (`x.py` holding `ab`) was never searched by such a query.
    all_docs: RoaringBitmap,
    // The posting lists in the mapped index file. Behind a lock only so a
    // checkpoint, which runs under the engine's read lock, can move it to
    // the file it just wrote (`rebase`); a query clones the `Arc` and
    // releases the lock at once.
    base: RwLock<Option<Arc<Base>>>,
    // Set by `rebase`: every list in `shards` is also in the new base, so
    // the shards are dropped at the next change (`settle`).
    shards_in_base: AtomicBool,
}

/// The mapped posting lists and what no longer holds of them.
#[derive(Clone)]
struct Base {
    postings: Arc<PersistedTrigramIndex>,
    /// Ids the file knows (every id below this was assigned when it was
    /// written). Only these can be voided.
    ids: u32,
    /// Documents removed or re-indexed since the file was written: their
    /// entries in the mapped lists are stale.
    void: RoaringBitmap,
}

impl Base {
    /// `entry`'s documents, without the void ones.
    fn decode(&self, entry: &DirEntry) -> RoaringBitmap {
        let mut docs = self.postings.decode(entry);
        if !self.void.is_empty() {
            docs -= &self.void;
        }
        docs
    }
}

impl Default for TrigramIndex {
    fn default() -> Self {
        Self {
            shards: (0..SHARDS).map(|_| FxHashMap::default()).collect(),
            all_docs: RoaringBitmap::new(),
            base: RwLock::new(None),
            shards_in_base: AtomicBool::new(false),
        }
    }
}

impl TrigramIndex {
    /// Estimated heap bytes of the posting lists, the tables included. See
    /// [`crate::heap_size`].
    pub fn heap_bytes(&self) -> u64 {
        use crate::heap_size as h;
        self.shards
            .iter()
            .map(|m| h::map(m) + m.values().map(h::bitmap).sum::<u64>())
            .sum::<u64>()
            + h::vec(&self.shards)
            + h::bitmap(&self.all_docs)
            + self
                .base()
                .map_or(0, |b| b.postings.heap_bytes() + h::bitmap(&b.void))
    }

    /// Bytes of posting lists read in place from the mapped index file:
    /// page cache, not heap. 0 when there is no mapped file.
    pub fn mapped_bytes(&self) -> u64 {
        self.base().map_or(0, |b| b.postings.mapped_bytes())
    }

    pub fn new() -> Self {
        Self::default()
    }

    /// An index whose posting lists are read in place from a loaded index
    /// file that assigned ids below `ids`. Call [`Self::set_documents`]
    /// with the live documents, and [`Self::remove_documents`] with those
    /// that did not survive.
    pub fn with_base(postings: PersistedTrigramIndex, ids: u32) -> Self {
        let index = Self::default();
        index.set_base(postings, ids);
        index
    }

    /// Read the posting lists in place from `postings`, a file just
    /// written from this index with the same document ids, below `ids`,
    /// and with no stale entries ([`Self::with_postings_to_save`]). The lists
    /// held on the heap are all in it, and are dropped at the next change
    /// or [`Self::settle`]. Takes `&self` because the checkpoint runs under
    /// the engine's read lock, while queries run: a query reads either
    /// version, and both give the same answers.
    pub fn rebase(&self, postings: PersistedTrigramIndex, ids: u32) {
        self.set_base(postings, ids);
        self.shards_in_base.store(true, Ordering::Release);
    }

    fn set_base(&self, postings: PersistedTrigramIndex, ids: u32) {
        *self.base.write().unwrap_or_else(|p| p.into_inner()) = Some(Arc::new(Base {
            postings: Arc::new(postings),
            ids,
            void: RoaringBitmap::new(),
        }));
    }

    /// Whether posting lists are read from a mapped file.
    pub fn has_base(&self) -> bool {
        self.base().is_some()
    }

    fn base(&self) -> Option<Arc<Base>> {
        self.base.read().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// Mark the mapped entries of `docs` stale (removed or re-indexed).
    fn void(&mut self, docs: impl IntoIterator<Item = u32>) {
        let base = self.base.get_mut().unwrap_or_else(|p| p.into_inner());
        if let Some(base) = base {
            let ids = base.ids;
            let mut known = docs.into_iter().filter(|&d| d < ids).peekable();
            if known.peek().is_some() {
                Arc::make_mut(base).void.extend(known);
            }
        }
    }

    /// Drop the heap lists that a [`Self::rebase`] made redundant.
    pub fn settle(&mut self) {
        if std::mem::take(self.shards_in_base.get_mut()) {
            for map in &mut self.shards {
                *map = FxHashMap::default();
            }
        }
    }

    fn list_mut(&mut self, trigram: Trigram) -> &mut RoaringBitmap {
        self.shards[shard_of(&trigram)].entry(trigram).or_default()
    }

    /// Add a document to the index
    pub fn add_document(&mut self, doc_id: u32, content: &str) {
        self.settle();
        self.void([doc_id]);
        // Use extract_unique_trigrams for efficiency - avoids Vec allocation + HashSet conversion
        for trigram in extract_unique_trigrams(content) {
            self.list_mut(trigram).insert(doc_id);
        }
        self.all_docs.insert(doc_id);
    }

    /// Add a document using pre-computed trigrams (for parallel indexing)
    pub fn add_document_trigrams(&mut self, doc_id: u32, trigrams: FxHashSet<Trigram>) {
        self.settle();
        self.void([doc_id]);
        for trigram in trigrams {
            self.list_mut(trigram).insert(doc_id);
        }
        self.all_docs.insert(doc_id);
    }

    /// Add many documents at once, in parallel: each document's trigrams
    /// are sorted into shards (in parallel over the documents), then each
    /// shard takes its share of every document (in parallel over the
    /// shards). The result is the same as [`Self::add_document_trigrams`]
    /// for each document in turn. Each id must appear once.
    pub fn add_documents(&mut self, docs: &[(u32, FxHashSet<Trigram>)]) {
        use rayon::prelude::*;
        if docs.is_empty() {
            return;
        }
        self.settle();
        self.void(docs.iter().map(|(id, _)| *id));
        // A few chunks per thread, so a chunk of large files doesn't hold
        // up the rest. Chunks keep documents in order, so each list gets
        // its ids in the order given (ascending for new files: the fast
        // append path of a roaring insert).
        let chunk = docs.len().div_ceil(rayon::current_num_threads() * 4).max(1);
        let buckets: Vec<Vec<Vec<(Trigram, u32)>>> = docs
            .par_chunks(chunk)
            .map(|chunk| {
                let mut buckets: Vec<Vec<(Trigram, u32)>> = vec![Vec::new(); SHARDS];
                for (id, trigrams) in chunk {
                    for t in trigrams {
                        buckets[shard_of(t)].push((*t, *id));
                    }
                }
                buckets
            })
            .collect();
        self.shards
            .par_iter_mut()
            .enumerate()
            .for_each(|(shard, map)| {
                for chunk in &buckets {
                    for &(t, id) in &chunk[shard] {
                        map.entry(t).or_default().insert(id);
                    }
                }
            });
        for (id, _) in docs {
            self.all_docs.insert(*id);
        }
    }

    /// Remove a document from the index (for incremental updates / deletions).
    ///
    /// Strips `doc_id` from every posting list and updates the all-documents
    /// cache so a removed file can never reappear as a candidate. There is no
    /// per-document trigram list, so this is one pass over every posting
    /// list; the pass is parallel and each list only pays a membership test
    /// (the update path of one 7 KB file went from 13 ms to well under 1 ms
    /// on a 134k-trigram index). With a mapped base only the lists indexed
    /// since are scanned: the document is voided in the base's.
    pub fn remove_document(&mut self, doc_id: u32) {
        use rayon::prelude::*;
        self.settle();
        self.void([doc_id]);
        self.shards.par_iter_mut().for_each(|map| {
            for docs in map.values_mut() {
                if docs.contains(doc_id) {
                    docs.remove(doc_id);
                }
            }
        });
        self.all_docs.remove(doc_id);
    }

    /// Remove many documents in a single pass over the posting lists.
    ///
    /// A watcher burst (branch switch, formatter run) used to cost one full
    /// map scan *per file*; this costs one (parallel) scan per batch. The
    /// all-documents cache is updated in place rather than invalidated.
    /// Posting lists left empty stay in the map until [`Self::finalize`]
    /// prunes them: an empty list can never produce a candidate.
    pub fn remove_documents(&mut self, doomed: &RoaringBitmap) {
        use rayon::prelude::*;
        if doomed.is_empty() {
            return;
        }
        self.settle();
        self.void(doomed.iter());
        if doomed.len() == 1 {
            if let Some(id) = doomed.min() {
                self.remove_document(id);
            }
            return;
        }
        self.shards.par_iter_mut().for_each(|map| {
            for docs in map.values_mut() {
                if docs.intersection_len(doomed) > 0 {
                    *docs -= doomed;
                }
            }
        });
        self.all_docs -= doomed;
    }

    /// Release over-allocated bucket memory from incremental inserts.
    /// Complements `finalize()` — safe to call at any time between batches.
    pub fn shrink_to_fit(&mut self) {
        self.settle();
        for map in &mut self.shards {
            map.shrink_to_fit();
        }
    }

    /// Finalize the index after bulk loading. Call this after indexing is complete
    /// to pre-compute the all_documents bitmap for faster regex fallback queries.
    /// Also shrinks the internal HashMap to release over-allocated bucket memory.
    pub fn finalize(&mut self) {
        use rayon::prelude::*;
        self.settle();
        self.all_docs.optimize();
        // Run-length encode dense posting lists: ubiquitous trigrams (spaces,
        // "the", newline+indent) cover nearly every document and shrink from
        // an 8 KB bitmap container per 65k docs to a few bytes, on disk and
        // in memory. Cheap; a no-op for lists that are already optimal.
        // Posting lists emptied by removals are dropped here rather than on
        // every removal.
        self.shards.par_iter_mut().for_each(|map| {
            map.retain(|_, docs| {
                if docs.is_empty() {
                    return false;
                }
                docs.optimize();
                true
            });
            // Release over-allocated hash-map bucket slots accumulated during
            // incremental inserts. FxHashMap doubles capacity on rehash; after
            // bulk load the table may be ~50% empty.
            map.shrink_to_fit();
        });
    }

    /// Search for documents containing all trigrams from the query
    pub fn search(&self, query: &str) -> RoaringBitmap {
        let unique_trigrams = extract_unique_trigrams(query);

        if unique_trigrams.is_empty() {
            return RoaringBitmap::new();
        }

        // Find all matching bitmaps and check for missing trigrams
        let base = self.base();
        let mut bitmaps: Vec<Cow<'_, RoaringBitmap>> = Vec::with_capacity(unique_trigrams.len());
        for trigram in &unique_trigrams {
            match self.list(base.as_deref(), trigram) {
                Some(docs) => bitmaps.push(docs),
                // If any trigram is not in the index, no documents match
                None => return RoaringBitmap::new(),
            }
        }

        // Sort by cardinality (smallest first) for optimal intersection order
        bitmaps.sort_by_key(|b| b.len());

        // Start with smallest bitmap and intersect with others
        let mut bitmaps = bitmaps.into_iter();
        let mut result = bitmaps.next().unwrap_or_default().into_owned();
        for bitmap in bitmaps {
            result &= &*bitmap;
            // Early exit if result becomes empty
            if result.is_empty() {
                return result;
            }
        }

        result
    }

    /// `trigram`'s documents: the mapped list (decoded, without the void
    /// documents) united with the heap list. `None` when neither has it.
    fn list<'s>(
        &'s self,
        base: Option<&Base>,
        trigram: &Trigram,
    ) -> Option<Cow<'s, RoaringBitmap>> {
        let heap = self.shards[shard_of(trigram)].get(trigram);
        let mapped = base.and_then(|b| b.postings.find(*trigram).map(|e| b.decode(e)));
        match (mapped, heap) {
            (None, None) => None,
            (None, Some(docs)) => Some(Cow::Borrowed(docs)),
            (Some(mut docs), heap) => {
                if let Some(heap) = heap {
                    docs |= heap;
                }
                Some(Cow::Owned(docs))
            }
        }
    }

    /// Get total number of trigrams in the index
    pub fn num_trigrams(&self) -> usize {
        match self.base() {
            None => self.shards.iter().map(|m| m.len()).sum(),
            Some(base) => {
                base.postings.len()
                    + self
                        .shards
                        .iter()
                        .flat_map(|m| m.keys())
                        .filter(|t| base.postings.find(**t).is_none())
                        .count()
            }
        }
    }

    /// All document IDs in the index, with or without trigrams.
    pub fn all_documents(&self) -> Cow<'_, RoaringBitmap> {
        Cow::Borrowed(&self.all_docs)
    }

    /// Set the documents the index holds (after [`Self::from_trigram_map`],
    /// whose posting lists do not name documents without trigrams).
    pub fn set_documents(&mut self, docs: RoaringBitmap) {
        self.all_docs = docs;
    }

    /// Every posting list, in no particular order, as queries see it
    /// (mapped lists are decoded, `Cow::Owned`; callers must not assume a
    /// borrow).
    pub fn postings(&self) -> Box<dyn Iterator<Item = (Trigram, Cow<'_, RoaringBitmap>)> + '_> {
        let heap = self.shards.iter().flat_map(|m| m.iter());
        let Some(base) = self.base() else {
            return Box::new(heap.map(|(t, docs)| (*t, Cow::Borrowed(docs))));
        };
        let in_base = {
            let base = base.clone();
            move |t: &Trigram| base.postings.find(*t).is_some()
        };
        let mapped = (0..base.postings.len()).map(move |i| {
            let t = base.postings.entries()[i].trigram;
            (t, self.list(Some(&base), &t).expect("listed"))
        });
        Box::new(
            mapped.chain(
                heap.filter(move |(t, _)| !in_base(t))
                    .map(|(t, docs)| (*t, Cow::Borrowed(docs))),
            ),
        )
    }

    /// Run `f` on every posting list in parallel, as queries see it. For
    /// a save that renumbers.
    pub fn map_postings<R, C>(
        &self,
        f: impl Fn(Trigram, Cow<'_, RoaringBitmap>) -> Option<R> + Sync,
    ) -> C
    where
        R: Send,
        C: rayon::iter::FromParallelIterator<R>,
    {
        use rayon::prelude::*;
        let heap = self.shards.par_iter().flat_map(|m| m.par_iter());
        let Some(base) = self.base() else {
            return heap
                .filter_map(|(t, docs)| f(*t, Cow::Borrowed(docs)))
                .collect();
        };
        let mapped = base.postings.entries().par_iter().map(|e| {
            let docs = self.list(Some(&base), &e.trigram).expect("listed");
            (e.trigram, docs)
        });
        mapped
            .chain(
                heap.filter(|(t, _)| base.postings.find(**t).is_none())
                    .map(|(t, docs)| (*t, Cow::Borrowed(docs))),
            )
            .filter_map(|(t, docs)| f(t, docs))
            .collect()
    }

    /// Call `save` with the posting lists to write to a file that keeps
    /// every document id (a checkpoint), as queries see them. A mapped list
    /// that neither a void document nor a heap list changes is passed as
    /// its bytes, without being written out again.
    pub fn with_postings_to_save<R>(
        &self,
        save: impl FnOnce(Vec<(Trigram, Posting<'_>)>) -> R,
    ) -> R {
        use rayon::prelude::*;
        let heap = self.shards.par_iter().flat_map(|m| m.par_iter());
        let Some(base) = self.base() else {
            return save(heap.map(|(t, docs)| (*t, Posting::from(docs))).collect());
        };
        let mapped = base.postings.entries().par_iter().map(|e| {
            let heap = self.shards[shard_of(&e.trigram)].get(&e.trigram);
            let posting = if heap.is_none() && base.void.is_empty() {
                Posting::Bytes(base.postings.bitmap_bytes(e))
            } else {
                let raw = base.postings.decode(e);
                if heap.is_none() && raw.is_disjoint(&base.void) {
                    Posting::Bytes(base.postings.bitmap_bytes(e))
                } else {
                    let mut docs = raw - &base.void;
                    if let Some(heap) = heap {
                        docs |= heap;
                    }
                    Posting::from(docs)
                }
            };
            (e.trigram, posting)
        });
        let postings = mapped
            .chain(
                heap.filter(|(t, _)| base.postings.find(**t).is_none())
                    .map(|(t, docs)| (*t, Posting::from(docs))),
            )
            .collect();
        save(postings)
    }

    /// Whether `trigram`'s posting list names `doc_id`.
    pub fn contains(&self, trigram: &Trigram, doc_id: u32) -> bool {
        if self.shards[shard_of(trigram)]
            .get(trigram)
            .is_some_and(|docs| docs.contains(doc_id))
        {
            return true;
        }
        self.base().is_some_and(|b| {
            !b.void.contains(doc_id)
                && b.postings
                    .find(*trigram)
                    .is_some_and(|e| b.postings.decode(e).contains(doc_id))
        })
    }

    /// The posting lists held on the heap (all of them when there is no
    /// mapped file).
    pub fn lists(&self) -> impl Iterator<Item = (&Trigram, &RoaringBitmap)> {
        self.shards.iter().flat_map(|m| m.iter())
    }

    /// One posting list as queries see it, for tests.
    #[cfg(test)]
    pub(crate) fn posting(&self, trigram: &Trigram) -> Option<Cow<'_, RoaringBitmap>> {
        self.list(self.base().as_deref(), trigram)
    }

    /// One posting list, to corrupt on purpose in tests of the self-check.
    #[cfg(test)]
    pub(crate) fn posting_mut(&mut self, trigram: &Trigram) -> Option<&mut RoaringBitmap> {
        self.shards[shard_of(trigram)].get_mut(trigram)
    }

    /// Restore the index from persisted posting lists.
    /// The documents are taken from the posting lists; call
    /// [`Self::set_documents`] when some may have no trigram.
    pub fn from_trigram_map(lists: impl IntoIterator<Item = (Trigram, RoaringBitmap)>) -> Self {
        use rayon::prelude::*;
        // Sort into shards by moving (no hashing), then build every shard's
        // table and the union of all lists in parallel: on load this runs
        // over every posting list.
        let mut buckets: Vec<Vec<(Trigram, RoaringBitmap)>> =
            (0..SHARDS).map(|_| Vec::new()).collect();
        for (trigram, docs) in lists {
            buckets[shard_of(&trigram)].push((trigram, docs));
        }
        let shards: Vec<FxHashMap<Trigram, RoaringBitmap>> = buckets
            .into_par_iter()
            .map(|bucket| bucket.into_iter().collect())
            .collect();
        let all_docs = shards
            .par_iter()
            .map(|map| {
                let mut all = RoaringBitmap::new();
                for docs in map.values() {
                    all |= docs;
                }
                all
            })
            .reduce(RoaringBitmap::new, |a, b| a | b);
        Self {
            shards,
            all_docs,
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Roadmap 6.4: the bitset/fold extractor must equal the reference
    /// (hash-set over `fold_str`), for ASCII and non-ASCII input, and
    /// leave the thread-local bitset clean between calls.
    #[test]
    fn test_lowercase_extraction_matches_reference() {
        let reference = |t: &str| -> Vec<Trigram> {
            let lower = crate::search::fold::fold_str(t);
            let mut v: Vec<Trigram> = extract_unique_trigrams(&lower).into_iter().collect();
            v.sort_by_key(|t| t.0);
            v
        };
        for text in [
            "Hello, World! HELLO again.",
            "ÜBER über Straße",
            "ſ\u{212A}µ ΟΣ",
            "ab",
            "",
            "aaaa\nAAAA",
        ] {
            let mut got: Vec<Trigram> = extract_unique_trigrams_lowercase(text)
                .into_iter()
                .collect();
            got.sort_by_key(|t| t.0);
            assert_eq!(got, reference(text), "{text:?}");
        }
        // Second call must not see stale bits from the first.
        let a = extract_unique_trigrams_lowercase("abcdef");
        let b = extract_unique_trigrams_lowercase("abcdef");
        assert_eq!(a.len(), 4);
        assert_eq!(a, b);
    }

    /// Roadmap 2.1/2.2: bulk removal strips ids in one pass and keeps the
    /// all-documents cache correct instead of invalidating it.
    #[test]
    fn test_remove_documents_bulk_keeps_cache_warm() {
        let mut idx = TrigramIndex::new();
        idx.add_document(0, "hello world");
        idx.add_document(1, "hello there");
        idx.add_document(2, "world peace");
        idx.finalize();
        assert_eq!(idx.all_documents().len(), 3);

        let mut doomed = RoaringBitmap::new();
        doomed.insert(0);
        doomed.insert(2);
        idx.remove_documents(&doomed);

        let all: Vec<u32> = idx.all_documents().iter().collect();
        assert_eq!(all, vec![1]);
        // "wor" only appeared in removed docs -> pruned; "hel" survives with doc 1.
        // The emptied list is pruned by finalize, not by the removal itself.
        assert!(idx
            .posting(&Trigram::new(*b"wor"))
            .is_some_and(|d| d.is_empty()));
        idx.finalize();
        assert!(idx.posting(&Trigram::new(*b"wor")).is_none());
        let hel: Vec<u32> = idx
            .posting(&Trigram::new(*b"hel"))
            .unwrap()
            .iter()
            .collect();
        assert_eq!(hel, vec![1]);

        // Adding keeps it current too, even for a document too short to
        // have a trigram.
        idx.add_document(3, "hello again");
        idx.add_document(4, "ab");
        assert_eq!(
            idx.all_documents().iter().collect::<Vec<_>>(),
            vec![1, 3, 4]
        );
    }

    /// The parallel batch merge gives the same posting lists as adding
    /// documents one at a time, ids out of order included (a re-indexed
    /// file keeps its older id).
    #[test]
    fn test_add_documents_matches_one_at_a_time() {
        let texts: Vec<String> = (0..500)
            .map(|i| {
                format!(
                    "fn task_{i}() {{ return value_{} + {i}; }} // {}",
                    i % 7,
                    i * 31
                )
            })
            .collect();
        let ids: Vec<u32> = (0..500u32)
            .map(|i| if i % 50 == 0 { i / 50 } else { i + 1000 })
            .collect();
        let mut one = TrigramIndex::new();
        let mut docs = Vec::new();
        for (id, text) in ids.iter().zip(&texts) {
            one.add_document_trigrams(*id, extract_unique_trigrams(text));
            docs.push((*id, extract_unique_trigrams(text)));
        }
        let mut batch = TrigramIndex::new();
        batch.add_documents(&docs[..250]);
        batch.add_documents(&docs[250..]);
        assert_eq!(one.num_trigrams(), batch.num_trigrams());
        for (t, docs) in one.lists() {
            assert_eq!(batch.posting(t), Some(Cow::Borrowed(docs)), "{t:?}");
        }
        assert_eq!(one.all_documents(), batch.all_documents());
    }

    #[test]
    fn test_trigram_extraction() {
        let text = "hello";
        let trigrams = extract_trigrams(text);
        assert_eq!(trigrams.len(), 3); // "hel", "ell", "llo"
    }

    #[test]
    fn test_index_and_search() {
        let mut index = TrigramIndex::new();

        index.add_document(0, "hello world");
        index.add_document(1, "hello rust");
        index.add_document(2, "goodbye world");

        let results = index.search("hello");
        assert!(results.contains(0));
        assert!(results.contains(1));
        assert!(!results.contains(2));

        let results = index.search("world");
        assert!(results.contains(0));
        assert!(!results.contains(1));
        assert!(results.contains(2));
    }
}
