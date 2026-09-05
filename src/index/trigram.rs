use roaring::RoaringBitmap;
use rustc_hash::{FxHashMap, FxHashSet};

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

    pub fn from_slice(slice: &[u8]) -> Option<Self> {
        if slice.len() >= 3 {
            Some(Trigram([slice[0], slice[1], slice[2]]))
        } else {
            None
        }
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

/// Unique trigrams of `text` **lowercased the way the index expects**
/// (`str::to_lowercase`), without materializing a lowercase copy of the
/// whole buffer when the text is ASCII (the overwhelmingly common case:
/// the fold happens per byte during extraction). Non-ASCII text still goes
/// through `to_lowercase()` so multi-byte case mappings match the query
/// side exactly.
pub fn extract_unique_trigrams_lowercase(text: &str) -> FxHashSet<Trigram> {
    if text.is_ascii() {
        unique_trigrams_folded(text.as_bytes(), true)
    } else {
        unique_trigrams_folded(text.to_lowercase().as_bytes(), false)
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

/// Inverted index mapping trigrams to document IDs using roaring bitmaps
#[derive(Default)]
pub struct TrigramIndex {
    // Map from trigram to set of document IDs containing that trigram
    // FxHashMap is faster than std HashMap for small keys like Trigram
    trigram_to_docs: FxHashMap<Trigram, RoaringBitmap>,
    // Cached bitmap of all document IDs (for regex fallback)
    all_docs_cache: Option<RoaringBitmap>,
}

impl TrigramIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a document to the index
    pub fn add_document(&mut self, doc_id: u32, content: &str) {
        // Use extract_unique_trigrams for efficiency - avoids Vec allocation + HashSet conversion
        let unique_trigrams = extract_unique_trigrams(content);

        for trigram in unique_trigrams {
            self.trigram_to_docs
                .entry(trigram)
                .or_default()
                .insert(doc_id);
        }

        if let Some(cache) = self.all_docs_cache.as_mut() {
            cache.insert(doc_id);
        }
    }

    /// Add a document using pre-computed trigrams (for parallel indexing)
    pub fn add_document_trigrams(&mut self, doc_id: u32, trigrams: FxHashSet<Trigram>) {
        for trigram in trigrams {
            self.trigram_to_docs
                .entry(trigram)
                .or_default()
                .insert(doc_id);
        }

        // Keep the all-documents cache warm across incremental adds instead of
        // forcing a full union over every posting list on the next short query.
        if let Some(cache) = self.all_docs_cache.as_mut() {
            cache.insert(doc_id);
        }
    }

    /// Remove a document from the index (for incremental updates / deletions).
    ///
    /// Strips `doc_id` from every posting list, drops trigram entries whose
    /// posting list becomes empty, and invalidates the all-documents cache so a
    /// removed file can never reappear as a candidate. O(number of trigrams),
    /// which is acceptable for the infrequent update/delete path.
    pub fn remove_document(&mut self, doc_id: u32) {
        let mut doomed = RoaringBitmap::new();
        doomed.insert(doc_id);
        self.remove_documents(&doomed);
    }

    /// Remove many documents in a single pass over the posting lists.
    ///
    /// A watcher burst (branch switch, formatter run) used to cost one full
    /// map scan *per file*; this costs one scan per batch. The all-documents
    /// cache is updated in place rather than invalidated.
    pub fn remove_documents(&mut self, doomed: &RoaringBitmap) {
        if doomed.is_empty() {
            return;
        }
        self.trigram_to_docs.retain(|_, docs| {
            if docs.intersection_len(doomed) > 0 {
                *docs -= doomed;
            }
            !docs.is_empty()
        });
        if let Some(cache) = self.all_docs_cache.as_mut() {
            *cache -= doomed;
        }
    }

    /// Release over-allocated bucket memory from incremental inserts.
    /// Complements `finalize()` — safe to call at any time between batches.
    pub fn shrink_to_fit(&mut self) {
        self.trigram_to_docs.shrink_to_fit();
    }

    /// Finalize the index after bulk loading. Call this after indexing is complete
    /// to pre-compute the all_documents bitmap for faster regex fallback queries.
    /// Also shrinks the internal HashMap to release over-allocated bucket memory.
    pub fn finalize(&mut self) {
        if self.all_docs_cache.is_none() {
            let mut all_docs = RoaringBitmap::new();
            for docs in self.trigram_to_docs.values() {
                all_docs |= docs;
            }
            self.all_docs_cache = Some(all_docs);
        }
        // Run-length encode dense posting lists: ubiquitous trigrams (spaces,
        // "the", newline+indent) cover nearly every document and shrink from
        // an 8 KB bitmap container per 65k docs to a few bytes, on disk and
        // in memory. Cheap; a no-op for lists that are already optimal.
        for docs in self.trigram_to_docs.values_mut() {
            docs.optimize();
        }
        // Release over-allocated hash-map bucket slots accumulated during incremental inserts.
        // FxHashMap doubles capacity on rehash; after bulk load the table may be ~50% empty.
        self.trigram_to_docs.shrink_to_fit();
    }

    /// Search for documents containing all trigrams from the query
    pub fn search(&self, query: &str) -> RoaringBitmap {
        let unique_trigrams = extract_unique_trigrams(query);

        if unique_trigrams.is_empty() {
            return RoaringBitmap::new();
        }

        // Find all matching bitmaps and check for missing trigrams
        let mut bitmaps: Vec<&RoaringBitmap> = Vec::with_capacity(unique_trigrams.len());
        for trigram in &unique_trigrams {
            if let Some(docs) = self.trigram_to_docs.get(trigram) {
                bitmaps.push(docs);
            } else {
                // If any trigram is not in the index, no documents match
                return RoaringBitmap::new();
            }
        }

        // Sort by cardinality (smallest first) for optimal intersection order
        bitmaps.sort_by_key(|b| b.len());

        // Start with smallest bitmap and intersect with others
        let mut result = bitmaps[0].clone();
        for bitmap in &bitmaps[1..] {
            result &= *bitmap;
            // Early exit if result becomes empty
            if result.is_empty() {
                return result;
            }
        }

        result
    }

    /// Get total number of trigrams in the index
    pub fn num_trigrams(&self) -> usize {
        self.trigram_to_docs.len()
    }

    /// Get total number of documents in the index
    pub fn num_documents(&self) -> u32 {
        if let Some(ref cached) = self.all_docs_cache {
            return cached.len() as u32;
        }
        // Fallback: compute on the fly
        let mut all_docs = RoaringBitmap::new();
        for docs in self.trigram_to_docs.values() {
            all_docs |= docs;
        }
        all_docs.len() as u32
    }

    /// All document IDs in the index.
    ///
    /// Borrowed from the cache that `finalize()` maintains; only an index that
    /// has never been finalized pays for a fresh union.
    pub fn all_documents(&self) -> std::borrow::Cow<'_, RoaringBitmap> {
        if let Some(ref cached) = self.all_docs_cache {
            return std::borrow::Cow::Borrowed(cached);
        }
        let mut all_docs = RoaringBitmap::new();
        for docs in self.trigram_to_docs.values() {
            all_docs |= docs;
        }
        std::borrow::Cow::Owned(all_docs)
    }

    /// Get a reference to the internal trigram-to-docs map for persistence
    pub fn get_trigram_map(&self) -> &FxHashMap<Trigram, RoaringBitmap> {
        &self.trigram_to_docs
    }

    /// Restore the index from a persisted trigram map
    pub fn from_trigram_map(trigram_to_docs: FxHashMap<Trigram, RoaringBitmap>) -> Self {
        Self {
            trigram_to_docs,
            all_docs_cache: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Roadmap 6.4: the bitset/fold extractor must equal the reference
    /// (hash-set over `to_lowercase()`), for ASCII and non-ASCII input, and
    /// leave the thread-local bitset clean between calls.
    #[test]
    fn test_lowercase_extraction_matches_reference() {
        let reference = |t: &str| -> Vec<Trigram> {
            let lower = t.to_lowercase();
            let mut v: Vec<Trigram> = extract_unique_trigrams(&lower).into_iter().collect();
            v.sort_by_key(|t| t.0);
            v
        };
        for text in [
            "Hello, World! HELLO again.",
            "ÜBER über Straße",
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

        // Cache was updated in place (still present, still correct).
        assert!(idx.all_docs_cache.is_some());
        let all: Vec<u32> = idx.all_documents().iter().collect();
        assert_eq!(all, vec![1]);
        // "wor" only appeared in removed docs -> pruned; "hel" survives with doc 1.
        assert!(idx.get_trigram_map().get(&Trigram::new(*b"wor")).is_none());
        let hel: Vec<u32> = idx.get_trigram_map()[&Trigram::new(*b"hel")]
            .iter()
            .collect();
        assert_eq!(hel, vec![1]);

        // Adding keeps the cache warm too.
        idx.add_document(3, "hello again");
        assert!(idx.all_docs_cache.is_some());
        assert_eq!(idx.all_documents().len(), 2);
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
