use roaring::RoaringBitmap;
use std::collections::{HashMap, HashSet};

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

/// Inverted index mapping trigrams to document IDs using roaring bitmaps
#[derive(Default)]
pub struct TrigramIndex {
    // Map from trigram to set of document IDs containing that trigram
    trigram_to_docs: HashMap<Trigram, RoaringBitmap>,
}

impl TrigramIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a document to the index
    pub fn add_document(&mut self, doc_id: u32, content: &str) {
        let trigrams = extract_trigrams(content);
        
        // Deduplicate trigrams to avoid redundant HashMap lookups and bitmap insertions
        let unique_trigrams: HashSet<Trigram> = trigrams.into_iter().collect();
        
        for trigram in unique_trigrams {
            self.trigram_to_docs
                .entry(trigram)
                .or_default()
                .insert(doc_id);
        }
    }

    /// Add a document using pre-computed trigrams (for parallel indexing)
    pub fn add_document_trigrams(&mut self, doc_id: u32, trigrams: HashSet<Trigram>) {
        for trigram in trigrams {
            self.trigram_to_docs
                .entry(trigram)
                .or_default()
                .insert(doc_id);
        }
    }

    /// Search for documents containing all trigrams from the query
    pub fn search(&self, query: &str) -> RoaringBitmap {
        let query_trigrams = extract_trigrams(query);
        
        if query_trigrams.is_empty() {
            return RoaringBitmap::new();
        }

        // Start with documents containing the first trigram
        let mut result = self.trigram_to_docs
            .get(&query_trigrams[0])
            .cloned()
            .unwrap_or_default();

        // Intersect with documents containing each subsequent trigram
        for trigram in &query_trigrams[1..] {
            if let Some(docs) = self.trigram_to_docs.get(trigram) {
                result &= docs;
            } else {
                // If any trigram is not in the index, no documents match
                return RoaringBitmap::new();
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
        let mut all_docs = RoaringBitmap::new();
        for docs in self.trigram_to_docs.values() {
            all_docs |= docs;
        }
        all_docs.len() as u32
    }

    /// Get all document IDs in the index
    pub fn all_documents(&self) -> RoaringBitmap {
        let mut all_docs = RoaringBitmap::new();
        for docs in self.trigram_to_docs.values() {
            all_docs |= docs;
        }
        all_docs
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
