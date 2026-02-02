//! Query embedding cache for semantic search
//!
//! Caches query embeddings to avoid re-computing for repeated queries.
//! Uses LRU eviction policy to keep memory usage bounded.

use rustc_hash::FxHashMap;
use std::collections::VecDeque;

/// Simple LRU cache for query embeddings
pub struct QueryCache {
    cache: FxHashMap<String, Vec<f32>>,
    order: VecDeque<String>,
    capacity: usize,
}

impl QueryCache {
    /// Create a new cache with given capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: FxHashMap::default(),
            order: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Get cached embedding for a query
    pub fn get(&mut self, query: &str) -> Option<&Vec<f32>> {
        if self.cache.contains_key(query) {
            // Move to end (most recently used)
            self.order.retain(|k| k != query);
            self.order.push_back(query.to_string());
            self.cache.get(query)
        } else {
            None
        }
    }

    /// Insert a new query embedding
    pub fn insert(&mut self, query: String, embedding: Vec<f32>) {
        // If already exists, remove from order
        if self.cache.contains_key(&query) {
            self.order.retain(|k| k != &query);
        } else if self.cache.len() >= self.capacity {
            // Evict least recently used
            if let Some(old_key) = self.order.pop_front() {
                self.cache.remove(&old_key);
            }
        }

        // Insert new entry
        self.cache.insert(query.clone(), embedding);
        self.order.push_back(query);
    }

    /// Get the number of cached queries
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Clear the cache
    pub fn clear(&mut self) {
        self.cache.clear();
        self.order.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_basic() {
        let mut cache = QueryCache::new(3);

        cache.insert("query1".to_string(), vec![1.0, 0.0]);
        cache.insert("query2".to_string(), vec![0.0, 1.0]);

        assert_eq!(cache.len(), 2);
        assert!(cache.get("query1").is_some());
        assert!(cache.get("query3").is_none());
    }

    #[test]
    fn test_cache_eviction() {
        let mut cache = QueryCache::new(2);

        cache.insert("query1".to_string(), vec![1.0, 0.0]);
        cache.insert("query2".to_string(), vec![0.0, 1.0]);
        cache.insert("query3".to_string(), vec![1.0, 1.0]);

        // query1 should be evicted
        assert_eq!(cache.len(), 2);
        assert!(cache.get("query1").is_none());
        assert!(cache.get("query2").is_some());
        assert!(cache.get("query3").is_some());
    }

    #[test]
    fn test_cache_lru() {
        let mut cache = QueryCache::new(2);

        cache.insert("query1".to_string(), vec![1.0, 0.0]);
        cache.insert("query2".to_string(), vec![0.0, 1.0]);

        // Access query1 to make it most recently used
        let _ = cache.get("query1");

        // Insert query3 - should evict query2 (least recently used)
        cache.insert("query3".to_string(), vec![1.0, 1.0]);

        assert!(cache.get("query1").is_some());
        assert!(cache.get("query2").is_none());
        assert!(cache.get("query3").is_some());
    }
}
