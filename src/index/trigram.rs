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
    let bytes = text.as_bytes();
    let len = bytes.len().saturating_sub(2);
    let mut trigrams = FxHashSet::with_capacity_and_hasher(
        len.min(MAX_INITIAL_TRIGRAM_CAPACITY),
        Default::default(),
    );

    for i in 0..len {
        trigrams.insert(Trigram([bytes[i], bytes[i + 1], bytes[i + 2]]));
    }

    trigrams
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

        // Invalidate cache when documents are added
        self.all_docs_cache = None;
    }

    /// Add a document using pre-computed trigrams (for parallel indexing)
    pub fn add_document_trigrams(&mut self, doc_id: u32, trigrams: FxHashSet<Trigram>) {
        for trigram in trigrams {
            self.trigram_to_docs
                .entry(trigram)
                .or_default()
                .insert(doc_id);
        }

        // Invalidate cache when documents are added
        self.all_docs_cache = None;
    }

    /// Finalize the index after bulk loading. Call this after indexing is complete
    /// to pre-compute the all_documents bitmap for faster regex fallback queries.
    pub fn finalize(&mut self) {
        if self.all_docs_cache.is_none() {
            let mut all_docs = RoaringBitmap::new();
            for docs in self.trigram_to_docs.values() {
                all_docs |= docs;
            }
            self.all_docs_cache = Some(all_docs);
        }
    }

    /// Search for documents containing all trigrams from the query
    pub fn search(&self, query: &str) -> RoaringBitmap {
        let query_trigrams = extract_trigrams(query);

        if query_trigrams.is_empty() {
            return RoaringBitmap::new();
        }

        // Deduplicate trigrams using FxHashSet for better performance
        let unique_trigrams: Vec<_> = {
            let mut seen = FxHashSet::default();
            query_trigrams
                .into_iter()
                .filter(|t| seen.insert(*t))
                .collect()
        };

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

    /// Get all document IDs in the index.
    /// For best performance, call `finalize()` after indexing is complete.
    pub fn all_documents(&self) -> RoaringBitmap {
        if let Some(ref cached) = self.all_docs_cache {
            return cached.clone();
        }
        // Fallback: compute on the fly (slower)
        let mut all_docs = RoaringBitmap::new();
        for docs in self.trigram_to_docs.values() {
            all_docs |= docs;
        }
        all_docs
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
