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
    hits: u64,
    misses: u64,
}

impl QueryCache {
    /// Create a new cache with given capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: FxHashMap::default(),
            order: VecDeque::with_capacity(capacity),
            capacity,
            hits: 0,
            misses: 0,
        }
    }

    /// Get cached embedding for a query
    pub fn get(&mut self, query: &str) -> Option<&Vec<f32>> {
        if self.cache.contains_key(query) {
            self.hits += 1;
            // Move to end (most recently used)
            self.order.retain(|k| k != query);
            self.order.push_back(query.to_string());
            self.cache.get(query)
        } else {
            self.misses += 1;
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
        self.hits = 0;
        self.misses = 0;
    }

    /// Get the cache hit rate as a fraction in [0.0, 1.0].
    /// Returns `None` if no lookups have been performed yet.
    pub fn hit_rate(&self) -> Option<f64> {
        let total = self.hits + self.misses;
        if total == 0 {
            None
        } else {
            Some(self.hits as f64 / total as f64)
        }
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

    #[test]
    fn test_cache_hit_rate_no_lookups() {
        let cache = QueryCache::new(3);
        assert_eq!(cache.hit_rate(), None);
    }

    #[test]
    fn test_cache_hit_rate() {
        let mut cache = QueryCache::new(3);

        cache.insert("query1".to_string(), vec![1.0, 0.0]);

        // One miss
        let _ = cache.get("missing");
        assert_eq!(cache.hit_rate(), Some(0.0));

        // One hit
        let _ = cache.get("query1");
        assert_eq!(cache.hit_rate(), Some(0.5)); // 1 hit / 2 total

        // Another hit
        let _ = cache.get("query1");
        // 2 hits / 3 total ≈ 0.666...
        let rate = cache.hit_rate().unwrap();
        assert!((rate - 2.0 / 3.0).abs() < 1e-10);
    }
}
