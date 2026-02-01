# Prior Art: Competitive Analysis of Code Search Tools

This document compares `fast_code_search` to the best code search implementations available, analyzing architectural approaches, performance characteristics, and identifying areas for improvement.

## Executive Summary

`fast_code_search` is a well-engineered code search service that combines several proven techniques. However, compared to production-grade tools like ripgrep, Zoekt, and GitHub Code Search, there are specific areas where improvements would bring significant benefits.

### Current Strengths
- ✅ Trigram-based indexing with Roaring bitmaps
- ✅ Memory-mapped file access
- ✅ Parallel search with rayon
- ✅ Symbol awareness via tree-sitter
- ✅ gRPC streaming API

### Key Improvement Opportunities
- 🔄 Replace byte-by-byte search with SIMD-accelerated algorithms
- 🔄 Add index persistence for faster restarts
- 🔄 Implement incremental indexing with file watching
- 🔄 Enhance regex acceleration with better literal extraction
- 🔄 Add ranking based on code quality signals

---

## 1. ripgrep (rg)

**Repository**: https://github.com/BurntSushi/ripgrep  
**Language**: Rust  
**Category**: Command-line grep replacement

### Architecture Overview

ripgrep is the gold standard for fast text search. Key techniques:

1. **SIMD-Accelerated Substring Search**
   - Uses the `memchr` crate for blazing fast literal matching
   - Vectorized algorithms (SSE2/AVX2/NEON) for 10-20x speedup over naive search
   - Teddy algorithm for simultaneous multi-pattern matching

2. **Regex Engine Optimization**
   - Lazy DFA construction with bounded memory
   - Literal extraction from regex patterns for pre-filtering
   - Hybrid regex engine that switches strategies based on pattern

3. **Smart File Handling**
   - Automatic binary file detection (skips non-text)
   - Respects `.gitignore` by default
   - Memory-maps large files, reads small files directly

### Comparison to fast_code_search

| Feature | ripgrep | fast_code_search | Gap |
|---------|---------|------------------|-----|
| Substring search | memchr/memmem (SIMD) | memchr/memmem (SIMD) | ✅ Equivalent |
| Case-insensitive | SIMD lowercasing | Byte-by-byte loop | **Improve** |
| Regex engine | Optimized hybrid | Standard regex crate | Minor gap |
| Index persistence | None (stateless) | None | N/A |
| Parallel search | Yes (files) | Yes (documents) | ✅ Equivalent |
| Incremental search | Streaming | Streaming | ✅ Equivalent |

### What We Can Adopt from ripgrep

1. **SIMD Case-Insensitive Search** (High Impact)
   ```rust
   // Current implementation (slow):
   fn contains_case_insensitive(haystack: &str, needle_lower: &str) -> bool {
       // Byte-by-byte comparison with manual case folding
   }
   
   // ripgrep approach (fast):
   // Use memchr's Finder with case-folding preprocessing
   // Or: Convert entire haystack to lowercase once, then use memmem
   ```

2. **Better Literal Extraction from Regex** (Medium Impact)
   - ripgrep extracts longer literal prefixes/suffixes for pre-filtering
   - Current regex_search.rs only handles basic concatenation

---

## 2. Zoekt (Google/Sourcegraph)

**Repository**: https://github.com/sourcegraph/zoekt  
**Language**: Go  
**Category**: Production code search engine (Sourcegraph backend)

### Architecture Overview

Zoekt powers Sourcegraph's code search and is based on Google's internal code search (CS).

1. **Persistent Index Format**
   - Sharded on-disk index (`.zoekt` files)
   - Enables instant startup for large codebases
   - Index format: trigrams + content + metadata in single file

2. **N-gram Indexing Strategy**
   - Trigrams for common patterns
   - Also stores 4-grams (quadgrams) for better selectivity
   - Case-folded trigrams for case-insensitive search

3. **Ranking System**
   - File path matching boost
   - Line position boost (earlier lines score higher)
   - Repository priority scores
   - Branch awareness (main/master weighted higher)

4. **Real-time Indexing**
   - inotify-based file watching
   - Incremental reindexing without full rebuild

### Comparison to fast_code_search

| Feature | Zoekt | fast_code_search | Gap |
|---------|-------|------------------|-----|
| Index persistence | Yes (disk) | No (memory only) | **Add** |
| Incremental indexing | Yes (inotify) | No | **Add** |
| N-gram length | 3-grams + 4-grams | 3-grams only | Consider |
| Symbol search | Yes | Yes | ✅ Equivalent |
| Ranking sophistication | High (10+ signals) | Medium (5 signals) | **Improve** |
| Repository awareness | Yes | No | Consider |

### What We Can Adopt from Zoekt

1. **Index Persistence** (High Impact)
   ```rust
   // Add serialization for the trigram index
   pub fn save_index(&self, path: &Path) -> Result<()> {
       // Serialize: TrigramIndex + FileStore metadata + SymbolCache
       // Use bincode or rkyv for fast serialization
   }
   
   pub fn load_index(path: &Path) -> Result<Self> {
       // Deserialize and memory-map the original files
   }
   ```

2. **File Watching for Incremental Updates** (High Impact)
   ```rust
   // Use notify crate for cross-platform file watching
   use notify::{Watcher, RecursiveMode, watcher};
   
   // On file change:
   // 1. Remove old document from trigram index
   // 2. Re-index the changed file
   // 3. Update symbol cache
   ```

3. **Quadgrams for Better Selectivity** (Medium Impact)
   - 4-character n-grams reduce false positives for longer queries
   - Trade-off: 256x more index entries than trigrams
   - Could be stored as a secondary index for common patterns

---

## 3. GitHub Code Search

**Architecture**: Proprietary (based on public engineering blogs)  
**Category**: Planet-scale code search (40M+ repositories)

### Key Innovations

1. **Hierarchical Sharding**
   - Repository-level shards
   - Language-aware partitioning
   - Read replicas for hot repositories

2. **Bloom Filters**
   - Fast negative lookups ("does this shard contain query?")
   - Reduces unnecessary shard scanning
   - Cascading filters: file → directory → repository

3. **Query Understanding**
   - NLP for intent detection (symbol vs. content search)
   - Automatic language scoping
   - Query suggestion/autocomplete

4. **Ranking Signals**
   - Repository stars/forks
   - File recency (recently modified files score higher)
   - Author reputation
   - Code quality metrics (lint scores)

### Comparison to fast_code_search

| Feature | GitHub CS | fast_code_search | Gap |
|---------|-----------|------------------|-----|
| Scale | 40M+ repos | Single codebase | Different scope |
| Bloom filters | Yes | No | Consider |
| Query understanding | Yes (NLP) | No | Consider |
| Ranking signals | 15+ | 5 | **Improve** |
| File recency | Yes | No | **Add** |
| Language detection | Yes | Yes (tree-sitter) | ✅ Equivalent |

### What We Can Adopt from GitHub Code Search

1. **File Recency Boost** (Medium Impact)
   ```rust
   // Store file modification time during indexing
   pub struct FileMetadata {
       pub path: PathBuf,
       pub mtime: SystemTime,
       // ... other fields
   }
   
   // In scoring:
   let age_days = (now - mtime).as_secs() / 86400;
   let recency_boost = 1.0 / (1.0 + (age_days as f64 * 0.01));
   score *= recency_boost;
   ```

2. **Bloom Filter Pre-screening** (Low-Medium Impact)
   - Useful when queries return few results
   - Can skip entire file checks with high probability
   - Trade-off: Additional memory and complexity

---

## 4. Google Code Search (Original)

**Paper**: "Regular Expression Matching with a Trigram Index" (Russ Cox, 2012)  
**Category**: Research/historical reference

### Key Contributions

This paper established the trigram indexing approach used by most code search tools:

1. **Trigram Boolean Queries**
   - Convert regex to trigram query: `abc.*def` → `(abc AND def)`
   - Index stores posting lists (document IDs) per trigram
   - Intersection narrows candidates before regex evaluation

2. **Regex Approximation**
   - Extract required literals from regex AST
   - Handle alternation, repetition, anchors
   - Fallback to full scan for pathological patterns

### What We Already Implement Correctly
- ✅ Trigram extraction from content
- ✅ Roaring bitmap intersection for candidate selection
- ✅ Regex literal extraction (basic)

### Improvement Based on Paper

**Better Regex Trigram Extraction** (Medium Impact)
```rust
// Current: Only extracts simple literals
// Paper approach: Handle more patterns

// For regex: `fn\s+\w+`
// Current: Extracts "fn" (only 2 chars, ignored)
// Better: Recognize `\s+` can't start with 'f', extract "fn" anyway

// For regex: `(abc|def)ghi`
// Current: May extract all three
// Better: Extract only "ghi" (required in all matches)
```

---

## 5. Sourcegraph Structural Search

**Repository**: https://github.com/comby-tools/comby  
**Category**: AST-aware code search

### Key Innovation

Structural search understands code syntax, not just text:

```
# Find all calls to `log(...)` with any arguments
log(:[args])

# Find all function definitions with specific patterns
func :[name](:[params]) { :[body] }
```

### Comparison to fast_code_search

Current symbol extraction is limited to definitions. Structural search would enable:
- Finding all call sites of a function
- Pattern matching on code structure
- Semantic code navigation

### What We Could Adopt

**Enhanced Tree-Sitter Queries** (Medium-High Impact)
```rust
// Current: Extract symbol definitions
// Enhanced: Allow tree-sitter pattern queries

pub fn structural_search(&self, pattern: &str, language: &str) -> Vec<SearchMatch> {
    // Parse pattern into tree-sitter query
    // Search across all files of that language
    // Return matches with captured groups
}
```

---

## 6. Performance Benchmark Comparison

### Methodology
Based on published benchmarks and reasonable estimates for 10GB codebases:

| Tool | Index Time | Search Time | Memory | Index Size |
|------|------------|-------------|--------|------------|
| ripgrep (no index) | N/A | 2-5s | Low | N/A |
| Zoekt | 5-10 min | 10-50ms | Medium | ~30% of source |
| GitHub CS | N/A | 100-500ms | N/A | Distributed |
| **fast_code_search** | 2-5 min | 1-20ms | High* | Memory only |

*Memory usage is high because we memory-map all files. This is efficient for access but shows as high memory usage in process stats.

### Performance Optimization Priorities

Based on benchmark gaps, priorities are:

1. **Index persistence** - Currently rebuilds on every restart
2. **SIMD case-insensitive search** - ripgrep is 5-10x faster here
3. **Incremental indexing** - Zoekt can handle file changes without full rebuild
4. **Better result limits** - Currently does partial sort; could use min-heap

---

## 7. Recommended Improvements

### High Priority (Significant Impact)

#### 1. Index Persistence
**Why**: 10GB codebase takes 2-5 minutes to index on startup
**How**: Serialize trigram index, file metadata, and symbol cache
**Effort**: Medium (1-2 days)

```rust
// New module: src/index/persistence.rs
use bincode::{serialize_into, deserialize_from};

impl SearchEngine {
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        serialize_into(writer, &self.serialize_state())?;
        Ok(())
    }
    
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let state = deserialize_from(reader)?;
        Self::from_serialized_state(state)
    }
}
```

#### 2. Incremental Indexing with File Watching
**Why**: Developers change files frequently; full reindex is wasteful
**How**: Use `notify` crate for cross-platform file watching
**Effort**: Medium (2-3 days)

```rust
// New module: src/index/watcher.rs
use notify::{Watcher, RecursiveMode};

pub struct IndexWatcher {
    engine: Arc<RwLock<SearchEngine>>,
    watcher: RecommendedWatcher,
}

impl IndexWatcher {
    pub fn watch(&mut self, path: &Path) -> Result<()> {
        self.watcher.watch(path, RecursiveMode::Recursive)?;
        Ok(())
    }
    
    fn handle_event(&self, event: Event) {
        match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) => {
                // Re-index the file
            }
            EventKind::Remove(_) => {
                // Remove from index
            }
            _ => {}
        }
    }
}
```

#### 3. SIMD-Accelerated Case-Insensitive Search
**Why**: Current byte-by-byte is 5-10x slower than SIMD
**How**: Use memchr with pre-lowercased haystack, or SIMD case-folding
**Effort**: Low (0.5-1 day)

```rust
// Option 1: Pre-lowercase entire haystack
fn search_in_document_fast(&self, doc_id: u32, query_lower: &str) -> Option<Vec<SearchMatch>> {
    let content = self.file_store.get(doc_id)?.as_str().ok()?;
    let content_lower = content.to_lowercase(); // One-time allocation
    
    // Now use memmem (SIMD) on lowercased content
    let finder = memmem::Finder::new(query_lower.as_bytes());
    // ...
}

// Option 2: Use aho-corasick with case-insensitive matching
use aho_corasick::AhoCorasick;
let ac = AhoCorasick::builder()
    .ascii_case_insensitive(true)
    .build(&[query])?;
```

### Medium Priority (Good Improvements)

#### 4. File Recency in Ranking
**Why**: Recently modified files are more relevant
**How**: Store mtime during indexing, factor into score
**Effort**: Low (0.5 day)

#### 5. Enhanced Regex Literal Extraction
**Why**: Better acceleration for complex patterns
**How**: Extract longer literals, handle more regex constructs
**Effort**: Medium (1-2 days)

#### 6. Query Result Caching
**Why**: Repeated queries are common in IDEs
**How**: LRU cache keyed by (query, filters, limit)
**Effort**: Low (0.5 day)

```rust
use lru::LruCache;

struct QueryCache {
    cache: Mutex<LruCache<QueryKey, Vec<SearchMatch>>>,
}

impl SearchEngine {
    pub fn search_cached(&self, query: &str, max_results: usize) -> Vec<SearchMatch> {
        let key = QueryKey::new(query, max_results);
        
        if let Some(cached) = self.cache.lock().unwrap().get(&key) {
            return cached.clone();
        }
        
        let results = self.search(query, max_results);
        self.cache.lock().unwrap().put(key, results.clone());
        results
    }
}
```

### Low Priority (Nice to Have)

#### 7. Bloom Filters for Negative Lookups
**Why**: Fast "definitely not in this file" checks
**Effort**: Medium (1-2 days)

#### 8. Quadgrams for Better Selectivity
**Why**: Fewer false positives for longer queries
**Effort**: Medium (1-2 days)

#### 9. Structural/AST Search
**Why**: Find code patterns, not just text
**Effort**: High (1-2 weeks)

---

## 8. Architectural Comparison Matrix

| Architecture | fast_code_search | ripgrep | Zoekt | GitHub CS |
|--------------|------------------|---------|-------|-----------|
| **Indexing** |
| Trigram index | ✅ Roaring | ❌ None | ✅ Custom | ✅ Custom |
| Persistent storage | ❌ | ❌ | ✅ | ✅ |
| Incremental updates | ❌ | N/A | ✅ | ✅ |
| **Search** |
| SIMD acceleration | ✅ (case-sensitive only*) | ✅ (memchr) | ✅ | ✅ |
| Regex hybrid engine | ❌ | ✅ | ✅ | ✅ |
| Parallel execution | ✅ (rayon) | ✅ | ✅ | ✅ |
| **Ranking** |
| Symbol awareness | ✅ | ❌ | ✅ | ✅ |
| Dependency graph | ✅ | ❌ | ❌ | ✅ |
| File recency | ❌ | ❌ | ✅ | ✅ |
| **API** |
| gRPC streaming | ✅ | ❌ | ❌ | ✅ |
| REST API | ✅ | ❌ | ✅ | ✅ |
| Web UI | ✅ | ❌ | ✅ | ✅ |

*Note: fast_code_search uses SIMD (memchr/memmem) for case-sensitive substring search, but case-insensitive search currently uses a byte-by-byte loop. See Section 1 for improvement recommendations.

---

## 9. Conclusion

`fast_code_search` implements many best practices from prior art:
- Trigram indexing (from Google Code Search)
- Roaring bitmaps for efficient set operations
- Memory-mapped files for large codebases
- Parallel search with rayon
- Symbol awareness with tree-sitter

The highest-impact improvements would be:
1. **Index persistence** - Eliminates startup latency
2. **Incremental indexing** - Responds to file changes in real-time
3. **SIMD case-insensitive search** - Matches ripgrep performance

These changes would bring `fast_code_search` closer to production-grade tools like Zoekt while maintaining its clean Rust architecture.

---

## References

1. Cox, R. (2012). [Regular Expression Matching with a Trigram Index](https://swtch.com/~rsc/regexp/regexp4.html)
2. BurntSushi/ripgrep: [Architecture documentation](https://github.com/BurntSushi/ripgrep/blob/master/GUIDE.md)
3. Sourcegraph/zoekt: [Design documentation](https://github.com/sourcegraph/zoekt/blob/main/doc/design.md)
4. GitHub Engineering Blog: [How we built GitHub Code Search](https://github.blog/2023-02-06-the-technology-behind-githubs-new-code-search/)
5. Roaring Bitmaps: [RoaringBitmap.org](https://roaringbitmap.org/)
6. Tree-sitter: [tree-sitter.github.io](https://tree-sitter.github.io/tree-sitter/)
