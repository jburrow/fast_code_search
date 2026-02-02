# Design: Regex Queries and Path Filtering

This document outlines how to add support for **regex queries** and **path filtering** (include/exclude folders using glob patterns) to fast_code_search without degrading search performance.

## Table of Contents

- [Deep Dive: How Regex and Trigrams Work Together](#deep-dive-how-regex-and-trigrams-work-together)
- [Deep Dive: How Path Globs Work Performantly](#deep-dive-how-path-globs-work-performantly)

- [Executive Summary](#executive-summary)
- [Current Architecture Overview](#current-architecture-overview)
- [Part 1: Regex Query Support](#part-1-regex-query-support)
  - [Design Goals](#design-goals)
  - [Strategy: Trigram-Accelerated Regex](#strategy-trigram-accelerated-regex)
  - [Implementation Plan](#implementation-plan)
  - [API Changes](#api-changes)
  - [Performance Considerations](#performance-considerations)
- [Part 2: Path Filtering (Include/Exclude Glob)](#part-2-path-filtering-includeexclude-glob)
  - [Design Goals](#design-goals-1)
  - [Strategy: Path Index with Glob Matching](#strategy-path-index-with-glob-matching)
  - [Implementation Plan](#implementation-plan-1)
  - [API Changes](#api-changes-1)
  - [Performance Considerations](#performance-considerations-1)
- [Combined Example Usage](#combined-example-usage)
- [Implementation Phases](#implementation-phases)
- [Dependencies](#dependencies)

---

## Executive Summary

This design proposes two new features for fast_code_search:

1. **Regex Query Support**: Use trigram extraction from regex literals to pre-filter candidates, then apply full regex matching only on the filtered set. This preserves the O(candidates) performance characteristic.

2. **Path Filtering**: Add per-query include/exclude glob patterns that filter the file set before content search, enabling folder-scoped searches without re-indexing.

Both features maintain the core performance advantage of trigram-based candidate filtering while adding powerful query capabilities.

---

## Current Architecture Overview

The current search flow:

```
Query "hello"
    │
    ▼
┌────────────────────────┐
│ Extract trigrams:      │
│ "hel", "ell", "llo"    │
└──────────┬─────────────┘
           │
           ▼
┌────────────────────────┐
│ Bitmap intersection:   │
│ docs("hel") ∩          │
│ docs("ell") ∩          │
│ docs("llo")            │
└──────────┬─────────────┘
           │
           ▼
┌────────────────────────┐
│ Candidate documents    │
│ (small subset of all)  │
└──────────┬─────────────┘
           │
           ▼ (parallel)
┌────────────────────────┐
│ Line-by-line search    │
│ with substring match   │
└──────────┬─────────────┘
           │
           ▼
┌────────────────────────┐
│ Scored results         │
└────────────────────────┘
```

**Key insight**: The trigram index dramatically reduces the search space. We can layer regex and path filtering on top without changing this core approach.

---

## Part 1: Regex Query Support

### Design Goals

1. **Preserve performance**: Regex search should not scan all files - still use trigram pre-filtering
2. **Gradual fallback**: Pure wildcards (`.*`) with no literals fall back to full scan (with warning)
3. **Compatibility**: Support standard Rust `regex` crate syntax
4. **Optional**: Regex mode should be opt-in via a flag or special prefix

### Strategy: Trigram-Accelerated Regex

The key insight is that most useful regex patterns contain **literal substrings** that can be extracted and used for trigram filtering:

| Regex Pattern | Extractable Literals | Trigrams |
|---------------|---------------------|----------|
| `fn\s+\w+` | `"fn"` (too short) | None - needs fallback |
| `async fn\s+handle` | `"async fn"`, `"handle"` | ✓ Multiple trigrams |
| `impl\s+Display\s+for` | `"impl"`, `"Display"`, `"for"` | ✓ Many trigrams |
| `\.unwrap\(\)` | `".unwrap()"` | ✓ Many trigrams |
| `[0-9]+` | None | Fallback to full scan |

#### Literal Extraction Algorithm

We can use the `regex-syntax` crate to parse regex and extract literal prefixes/sequences:

```rust
use regex_syntax::hir::{Hir, HirKind, Literal};

fn extract_literals(pattern: &str) -> Vec<String> {
    let hir = regex_syntax::parse(pattern).ok()?;
    extract_literals_from_hir(&hir)
}

fn extract_literals_from_hir(hir: &Hir) -> Vec<String> {
    let mut literals = Vec::new();
    
    match hir.kind() {
        HirKind::Literal(Literal::Unicode(c)) => {
            literals.push(c.to_string());
        }
        HirKind::Concat(subs) => {
            // Concatenate consecutive literals
            let mut current = String::new();
            for sub in subs {
                if let HirKind::Literal(Literal::Unicode(c)) = sub.kind() {
                    current.push(*c);
                } else {
                    // Hit non-literal - save what we have and recurse
                    if current.len() >= 3 {
                        // Has at least one trigram
                        literals.push(current.clone());
                    }
                    current.clear();
                    literals.extend(extract_literals_from_hir(sub));
                }
            }
            // Don't forget trailing literal
            if current.len() >= 3 {
                literals.push(current);
            }
        }
        HirKind::Alternation(alts) => {
            // For alternation, extract from all branches
            for alt in alts {
                literals.extend(extract_literals_from_hir(alt));
            }
        }
        _ => {}
    }
    
    literals
}
```

#### Search Flow with Regex

```
Regex Query: `impl\s+Display\s+for\s+(\w+)`
    │
    ▼
┌────────────────────────────┐
│ Parse regex                │
│ Extract literals:          │
│ ["impl", "Display", "for"] │
└──────────┬─────────────────┘
           │
           ▼
┌────────────────────────────┐
│ Extract trigrams from      │
│ each literal               │
│ Union of all trigram sets  │
└──────────┬─────────────────┘
           │
           ▼
┌────────────────────────────┐
│ Bitmap intersection        │
│ (same as literal search)   │
└──────────┬─────────────────┘
           │
           ▼ (parallel)
┌────────────────────────────┐
│ Line-by-line REGEX match   │  ← Changed from substring match
│ using compiled Regex       │
└──────────┬─────────────────┘
           │
           ▼
┌────────────────────────────┐
│ Scored results             │
└────────────────────────────┘
```

### Implementation Plan

#### 1. Add Dependencies

```toml
# Cargo.toml
regex = "1.10"
regex-syntax = "0.8"
```

#### 2. Create Regex Search Module

New file: `src/search/regex_search.rs`

```rust
use regex::Regex;
use regex_syntax::hir::{Hir, HirKind};

/// Result of analyzing a regex pattern for trigram acceleration
pub struct RegexAnalysis {
    /// Compiled regex for matching
    pub regex: Regex,
    /// Extracted literal strings that can be used for trigram filtering
    pub literals: Vec<String>,
    /// Whether this regex can be accelerated (has usable literals)
    pub is_accelerated: bool,
}

impl RegexAnalysis {
    /// Analyze a regex pattern and extract literals for trigram pre-filtering
    pub fn analyze(pattern: &str) -> Result<Self, regex::Error> {
        let regex = Regex::new(pattern)?;
        
        let literals = match regex_syntax::parse(pattern) {
            Ok(hir) => extract_literals_from_hir(&hir),
            Err(_) => vec![],
        };
        
        // Regex is accelerated if we have at least one literal >= 3 chars
        let is_accelerated = literals.iter().any(|l| l.len() >= 3);
        
        Ok(Self {
            regex,
            literals,
            is_accelerated,
        })
    }
}

fn extract_literals_from_hir(hir: &Hir) -> Vec<String> {
    // Implementation as described above
    // ...
}
```

#### 3. Modify SearchEngine

Update `src/search/engine.rs`:

```rust
impl SearchEngine {
    /// Search using a regex pattern with trigram acceleration
    pub fn search_regex(&self, pattern: &str, max_results: usize) -> Result<Vec<SearchMatch>> {
        let analysis = RegexAnalysis::analyze(pattern)?;
        
        // Get candidate documents using extracted literals
        let candidates = if analysis.is_accelerated {
            // Use trigrams from literals
            // We use UNION because:
            // 1. Extracted literals may come from different parts of the regex
            //    (e.g., alternations, or separate literal sequences)
            // 2. A document only needs to match the overall regex, not all literals
            // 3. Union casts a wider net that parallel search will refine
            // For a single contiguous literal, trigram_index.search() already
            // does intersection of its trigrams internally.
            let mut combined = RoaringBitmap::new();
            for literal in &analysis.literals {
                let literal_candidates = self.trigram_index.search(literal);
                combined |= &literal_candidates;
            }
            combined
        } else {
            // No acceleration possible - warn and return all documents
            tracing::warn!(
                pattern = %pattern,
                "Regex has no extractable literals - falling back to full scan"
            );
            self.get_all_document_ids()
        };
        
        // Parallel regex search
        let doc_ids: Vec<u32> = candidates.iter().collect();
        let matches: Vec<SearchMatch> = doc_ids
            .par_iter()
            .filter_map(|&doc_id| {
                self.search_in_document_regex(doc_id, &analysis.regex)
            })
            .flatten()
            .collect();
        
        // Sort and return top results
        // ...
    }
    
    fn search_in_document_regex(&self, doc_id: u32, regex: &Regex) -> Option<Vec<SearchMatch>> {
        let file = self.file_store.get(doc_id)?;
        let content = file.as_str().ok()?;
        
        let mut matches = Vec::new();
        for (line_num, line) in content.lines().enumerate() {
            if regex.is_match(line) {
                // Create match with highlighted capture groups
                matches.push(SearchMatch {
                    // ...
                });
            }
        }
        
        Some(matches)
    }
}
```

### API Changes

#### gRPC API (proto/search.proto)

```protobuf
message SearchRequest {
  string query = 1;
  int32 max_results = 2;
  bool is_regex = 3;  // NEW: Treat query as regex pattern
}
```

#### REST API (Query Parameters)

```
GET /api/search?q=impl\s+Display&regex=true&max=50
```

Updated `SearchQuery` in `src/web/api.rs`:

```rust
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    q: String,
    #[serde(default = "default_max_results")]
    max: usize,
    #[serde(default)]
    regex: bool,  // NEW
}
```

### Performance Considerations

| Scenario | Performance Impact |
|----------|-------------------|
| Regex with 3+ char literals | Minimal - trigram filter still effective |
| Regex with only 2-char literals | Moderate - trigram filter less effective |
| Regex with no literals | High - full scan required, emit warning |
| Complex regex | Moderate - regex compilation is cached per query |

**Recommendations**:

1. **Cache compiled regex**: For repeated queries, consider an LRU cache of compiled `Regex` objects
2. **Timeout protection**: Add configurable timeout for regex search to prevent ReDoS
3. **Complexity limit**: Warn or reject extremely complex patterns
4. **Document best practices**: Guide users to include literals in their regex patterns

---

## Part 2: Path Filtering (Include/Exclude Glob)

### Design Goals

1. **Zero re-indexing**: Path filtering happens at query time, not index time
2. **Flexible patterns**: Support glob patterns like `src/**/*.rs`, `!**/test/**`
3. **Composable**: Works with both literal and regex search
4. **Fast**: Use path index for O(1) per-file filtering

### Strategy: Path Index with Glob Matching

#### Approach 1: Pre-computed Path Index (Recommended)

Add a path-to-document-id index for fast filtering:

```rust
pub struct PathIndex {
    /// Map from normalized path string to document ID
    path_to_id: HashMap<String, u32>,
    /// Document ID to path (for reverse lookup during filtering)
    id_to_path: Vec<PathBuf>,
}
```

At query time:

1. Compile glob patterns once
2. Iterate through `id_to_path` and match against patterns
3. Create a `RoaringBitmap` of matching file IDs
4. Intersect with trigram candidates

#### Glob Pattern Syntax

Support standard glob patterns with extensions:

| Pattern | Matches |
|---------|---------|
| `src/**/*.rs` | All `.rs` files under `src/` |
| `!**/test/**` | Exclude all paths containing `test/` |
| `lib/**` | All files under `lib/` |
| `*.{js,ts,tsx}` | Files with these extensions |

Use the `globset` crate for fast multi-pattern matching:

```rust
use globset::{Glob, GlobSet, GlobSetBuilder};

pub struct PathFilter {
    include: Option<GlobSet>,
    exclude: Option<GlobSet>,
}

impl PathFilter {
    pub fn new(include_patterns: &[String], exclude_patterns: &[String]) -> Result<Self> {
        let include = if include_patterns.is_empty() {
            None
        } else {
            let mut builder = GlobSetBuilder::new();
            for pattern in include_patterns {
                builder.add(Glob::new(pattern)?);
            }
            Some(builder.build()?)
        };
        
        let exclude = if exclude_patterns.is_empty() {
            None
        } else {
            let mut builder = GlobSetBuilder::new();
            for pattern in exclude_patterns {
                builder.add(Glob::new(pattern)?);
            }
            Some(builder.build()?)
        };
        
        Ok(Self { include, exclude })
    }
    
    pub fn matches(&self, path: &str) -> bool {
        // If include patterns exist, path must match at least one
        let included = match &self.include {
            Some(set) => set.is_match(path),
            None => true,  // No include patterns = include all
        };
        
        // If exclude patterns exist, path must not match any
        let excluded = match &self.exclude {
            Some(set) => set.is_match(path),
            None => false,  // No exclude patterns = exclude none
        };
        
        included && !excluded
    }
}
```

### Implementation Plan

#### 1. Add Dependency

```toml
# Cargo.toml
globset = "0.4"
```

#### 2. Add PathFilter Module

New file: `src/search/path_filter.rs`

```rust
use globset::{Glob, GlobSet, GlobSetBuilder};
use roaring::RoaringBitmap;
use anyhow::Result;

/// Filters files by path patterns
pub struct PathFilter {
    include: Option<GlobSet>,
    exclude: Option<GlobSet>,
}

impl PathFilter {
    /// Create a new path filter from include/exclude patterns
    pub fn new(include: &[String], exclude: &[String]) -> Result<Self> {
        // Implementation as above
    }
    
    /// Check if a path matches the filter criteria
    pub fn matches(&self, path: &str) -> bool {
        // Implementation as above
    }
    
    /// Filter a set of document IDs based on their paths
    pub fn filter_documents(
        &self,
        candidates: &RoaringBitmap,
        id_to_path: &[PathBuf],
    ) -> RoaringBitmap {
        let mut result = RoaringBitmap::new();
        for doc_id in candidates.iter() {
            if let Some(path) = id_to_path.get(doc_id as usize) {
                if self.matches(&path.to_string_lossy()) {
                    result.insert(doc_id);
                }
            }
        }
        result
    }
}
```

#### 3. Modify SearchEngine

```rust
impl SearchEngine {
    /// Search with path filtering
    pub fn search_with_filter(
        &self,
        query: &str,
        include_paths: &[String],
        exclude_paths: &[String],
        max_results: usize,
    ) -> Result<Vec<SearchMatch>> {
        // Build path filter
        let path_filter = PathFilter::new(include_paths, exclude_paths)?;
        
        // Get trigram candidates
        let candidates = self.trigram_index.search(query);
        
        // Apply path filter
        let filtered = path_filter.filter_documents(
            &candidates,
            &self.file_store.get_all_paths(),
        );
        
        // Search in filtered documents
        // ...
    }
}
```

### API Changes

#### gRPC API (proto/search.proto)

```protobuf
message SearchRequest {
  string query = 1;
  int32 max_results = 2;
  bool is_regex = 3;
  repeated string include_paths = 4;  // NEW: Glob patterns for paths to include
  repeated string exclude_paths = 5;  // NEW: Glob patterns for paths to exclude
}
```

#### REST API (Query Parameters)

```
GET /api/search?q=hello&include=src/**/*.rs&include=lib/**&exclude=**/test/**
```

Updated `SearchQuery`:

```rust
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    q: String,
    #[serde(default)]
    max: usize,
    #[serde(default)]
    regex: bool,
    #[serde(default)]
    include: Vec<String>,  // NEW
    #[serde(default)]
    exclude: Vec<String>,  // NEW
}
```

### Performance Considerations

| Scenario | Performance Impact |
|----------|-------------------|
| No path filter | Zero overhead |
| Simple patterns (e.g., `src/**`) | O(candidates) pattern matching |
| Complex patterns | Slightly higher, still O(candidates) |
| Many patterns | Use GlobSet for efficient multi-pattern matching |

**Optimization**: For repeated searches with the same path patterns, the `GlobSet` should be cached.

---

## Combined Example Usage

### REST API Example

Search for async functions in Rust files under `src/`, excluding tests:

```bash
curl "http://localhost:8080/api/search?\
q=async%20fn&\
regex=true&\
include=src/**/*.rs&\
exclude=**/test/**&\
exclude=**/*_test.rs&\
max=50"
```

### gRPC Example

```rust
let request = SearchRequest {
    query: r"impl\s+\w+\s+for\s+\w+".to_string(),
    max_results: 100,
    is_regex: true,
    include_paths: vec!["src/**/*.rs".to_string()],
    exclude_paths: vec!["**/test/**".to_string()],
};
```

### Search Flow with Both Features

```
Query: regex "impl\s+Display" + include "src/**" + exclude "**/test/**"
    │
    ▼
┌─────────────────────────────┐
│ 1. Parse regex              │
│    Extract literals: "impl" │
└──────────┬──────────────────┘
           │
           ▼
┌─────────────────────────────┐
│ 2. Trigram candidates       │
│    (docs containing "impl") │
└──────────┬──────────────────┘
           │
           ▼
┌─────────────────────────────┐
│ 3. Path filtering           │
│    Include: src/**          │
│    Exclude: **/test/**      │
└──────────┬──────────────────┘
           │
           ▼
┌─────────────────────────────┐
│ 4. Final candidates         │
│    (much smaller set)       │
└──────────┬──────────────────┘
           │
           ▼ (parallel)
┌─────────────────────────────┐
│ 5. Regex matching per line  │
└──────────┬──────────────────┘
           │
           ▼
┌─────────────────────────────┐
│ 6. Scored & ranked results  │
└─────────────────────────────┘
```

---

## Implementation Phases

### Phase 1: Path Filtering (Lower Risk)
1. Add `globset` dependency
2. Implement `PathFilter` module
3. Add `include`/`exclude` to REST API
4. Add `include_paths`/`exclude_paths` to gRPC API
5. Write tests
6. Update documentation

**Estimated effort**: 2-3 days

### Phase 2: Regex Support (Medium Risk)
1. Add `regex` and `regex-syntax` dependencies
2. Implement literal extraction from regex
3. Add `search_regex` method to engine
4. Add `is_regex` flag to APIs
5. Add timeout protection
6. Write comprehensive tests
7. Document best practices

**Estimated effort**: 3-5 days

### Phase 3: Optimization (Optional)
1. Add LRU cache for compiled regex patterns
2. Add LRU cache for compiled GlobSets
3. Profile and optimize hot paths
4. Add metrics for regex vs literal search performance

**Estimated effort**: 2-3 days

---

## Dependencies

### New Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `regex` | 1.10 | Regex matching |
| `regex-syntax` | 0.8 | Parsing regex for literal extraction |
| `globset` | 0.4 | Fast glob pattern matching |

### Security Considerations

1. **ReDoS Protection**: Add timeout for regex matching
2. **Pattern Complexity**: Consider limiting pattern complexity
3. **Input Validation**: Validate and sanitize all patterns

### Backward Compatibility

All changes are additive:
- Existing API calls continue to work unchanged
- New parameters have sensible defaults (regex=false, include=[], exclude=[])
- No changes to index format

---

## Summary

This design enables powerful regex and path-filtered search while preserving the core performance advantage of trigram-based candidate filtering:

1. **Regex search** extracts literals from patterns to use trigram pre-filtering, avoiding full scans when possible
2. **Path filtering** uses efficient glob matching on pre-filtered candidates, adding minimal overhead
3. **Both features** are optional and backward-compatible with existing APIs
4. **Performance** is maintained by always filtering before scanning

The implementation can be done in phases, starting with the lower-risk path filtering feature before tackling regex support.

---

## Deep Dive: How Regex and Trigrams Work Together

### The Problem with Naive Regex Search

Without optimization, regex search would be extremely slow:

```
Naive approach:
  For each of 100,000 files:
    For each of ~10,000 lines per file:
      Run regex.is_match(line)
      
Total operations: 100,000 × 10,000 = 1 BILLION regex matches 😱
```

Even if each regex match takes 1 microsecond, that's 1000 seconds (16+ minutes) for a single search!

### The Trigram Insight

**Key observation**: Most useful regex patterns contain literal substrings.

Consider the regex: `async\s+fn\s+(\w+)_handler`

This pattern matches things like `async fn get_handler`, `async  fn  post_handler`, etc.

But notice: **every match MUST contain the literal strings "async" and "handler"**. 

The `\s+` and `(\w+)` parts are variable, but "async" and "handler" are fixed!

### How Trigram Pre-filtering Works

**Step 1: Extract literals from the regex**

```
Pattern: async\s+fn\s+(\w+)_handler
              ↓ Parse regex AST ↓
Literals found: ["async", "handler"]
```

**Step 2: Find candidate documents using trigrams**

For "async":
```
Trigrams: ["asy", "syn", "ync"]

Index lookup:
  docs("asy") = {file_23, file_45, file_102, file_789, file_1024}
  docs("syn") = {file_23, file_45, file_102, file_567, file_789}
  docs("ync") = {file_23, file_45, file_102, file_789}
  
Intersection = {file_23, file_45, file_102, file_789}  ← Only 4 files contain "async"!
```

For "handler":
```
Trigrams: ["han", "and", "ndl", "dle", "ler"]

Index lookup → Intersection = {file_23, file_45, file_501, file_789, file_890}
```

Combined candidates (union): `{file_23, file_45, file_102, file_501, file_789, file_890}` = **6 files**

**Step 3: Run regex only on candidates**

```
Optimized approach:
  For each of 6 candidate files:      ← Not 100,000!
    For each line in file:
      Run regex.is_match(line)

Total: 6 files × 10,000 lines = 60,000 regex matches ✓
```

**Result**: 60,000 matches instead of 1 billion = **16,666x faster!**

### Worked Example with Real Numbers

Suppose we have a 10GB codebase:
- 100,000 source files
- Average 10,000 lines per file
- 1 billion total lines

**Query**: `impl\s+Display\s+for\s+(\w+)`

**Without trigram acceleration**:
```
1 billion lines × 1μs per regex match = 1,000 seconds = 16.7 minutes
```

**With trigram acceleration**:

1. Extract literal: `"impl"` (also "Display" and "for")
2. Query trigram index: ~500 files contain all trigrams of "impl"
3. Regex search only those files: 500 × 10,000 = 5 million lines
4. Time: 5 million × 1μs = 5 seconds

**Speedup: 200x faster** (16.7 minutes → 5 seconds)

And that's conservative! If we also use "Display" for filtering, we might get down to 50 candidate files → 0.5 seconds.

### Why Union (OR) Instead of Intersection (AND)?

When we have multiple literals like `["impl", "Display", "for"]`, we use **union** (OR) of their candidate sets, not intersection (AND). Why?

**Reason 1: Regex semantics**
The literals might come from different branches of an alternation:
```
Pattern: (impl|pub)\s+fn
Literals: ["impl", "fn"] and ["pub", "fn"]
```
A match only needs "impl" OR "pub", not both.

**Reason 2: Conservative filtering**
Using union ensures we never miss a true match. The parallel regex search will filter out false positives. Better to check a few extra files than miss valid results.

**Reason 3: Single literal is already tight**
For a single contiguous literal like "Display", the trigram index already does intersection internally:
```
"Display" → trigrams ["Dis", "isp", "spl", "pla", "lay"]
              → candidates = docs("Dis") ∩ docs("isp") ∩ ... ∩ docs("lay")
```
This is already very selective.

### What About Regex Patterns with No Literals?

Some patterns have no extractable literals:
```
Pattern: [0-9]{3}-[0-9]{4}    (phone numbers)
Pattern: \b\w{20,}\b          (long words)
Pattern: .*                   (match anything)
```

For these, we **fall back to full scan** with a warning:
```rust
tracing::warn!("Regex has no extractable literals - falling back to full scan");
```

This is unavoidable, but rare. Most useful code search patterns contain literals.

### Visual Summary

```
┌──────────────────────────────────────────────────────────────────┐
│                    REGEX + TRIGRAMS FLOW                        │
└──────────────────────────────────────────────────────────────────┘

 Regex: impl\s+Display\s+for
         ↓
 ┌───────────────────────┐
 │  Parse with           │
 │  regex-syntax crate   │
 └───────────┬───────────┘
             ↓
 ┌───────────────────────┐
 │  Extract literals:    │
 │  "impl", "Display"    │
 └───────────┬───────────┘
             ↓
 ┌───────────────────────┐
 │  For each literal:    │
 │  Extract trigrams     │──────────┐
 │  Query bitmap index   │          │
 └───────────┬───────────┘          │
             ↓                      │
 ┌───────────────────────┐          │
 │  Bitmap intersection  │          │ Existing trigram
 │  per literal          │          │ infrastructure
 └───────────┬───────────┘          │ (unchanged)
             ↓                      │
 ┌───────────────────────┐          │
 │  Union of all         │──────────┘
 │  literal candidates   │
 └───────────┬───────────┘
             ↓
             ↓ 100,000 files → ~500 candidates
             ↓
 ┌───────────────────────┐
 │  PARALLEL (rayon)     │
 │  Regex.is_match()     │          Only here do we
 │  on each candidate    │          actually run regex!
 │  file's lines         │
 └───────────┬───────────┘
             ↓
 ┌───────────────────────┐
 │  Score & rank         │
 │  Return top N         │
 └───────────────────────┘
```

---

## Deep Dive: How Path Globs Work Performantly

### The Problem with Naive Path Filtering

If we naively check glob patterns against every file on every search:

```
Naive approach:
  User searches with include="src/**/*.rs"
  For each of 100,000 files:
    Check if path matches glob pattern
    
100,000 glob matches per search = slow!
```

### Why Glob Filtering is Actually Fast

**Key insight 1: Filtering happens AFTER trigram pre-filtering**

The trigram index already reduces 100,000 files to ~500 candidates. We only need to glob-match those 500 paths, not all 100,000.

```
                    Files at each stage
                    ────────────────────
All indexed files:  100,000
After trigram:      500      (0.5% of total)
After glob filter:  150      (30% of candidates)
```

**Key insight 2: GlobSet compiles patterns once**

The `globset` crate compiles multiple patterns into a single state machine:

```rust
use globset::{Glob, GlobSetBuilder};

// Compile once (done once per query)
let mut builder = GlobSetBuilder::new();
builder.add(Glob::new("src/**/*.rs")?);
builder.add(Glob::new("lib/**/*.rs")?);
let glob_set = builder.build()?;

// Match is O(1) amortized per path
glob_set.is_match("src/search/engine.rs")  // Very fast!
```

The compiled `GlobSet` uses a finite automaton that matches in O(path_length) time, regardless of how many patterns are in the set.

**Key insight 3: Paths are short strings**

Typical path: `/home/user/code/project/src/search/engine.rs` = 48 characters

Matching a 48-character string against a compiled automaton takes ~50 nanoseconds.

### Worked Example

**Query**: Search for "SearchEngine" in Rust files under src/, excluding tests

```
include = ["src/**/*.rs"]
exclude = ["**/test/**", "**/*_test.rs"]
```

**Step 1: Trigram filtering**

```
"SearchEngine" trigrams → 500 candidate files
```

**Step 2: Compile glob patterns** (once per query, ~1ms)

```rust
let include_set = GlobSet::new(["src/**/*.rs"]);
let exclude_set = GlobSet::new(["**/test/**", "**/*_test.rs"]);
```

**Step 3: Filter candidates** (500 files × 50ns = 25μs)

```
Candidates            Path                           Include?  Exclude?  Keep?
─────────────────────────────────────────────────────────────────────────────
file_23    src/search/engine.rs                      ✓         ✗         ✓
file_45    src/search/test/engine_test.rs            ✓         ✓         ✗
file_102   lib/utils/search.rs                       ✗         ✗         ✗
file_789   src/web/api.rs                            ✓         ✗         ✓
...
```

**Result**: 500 candidates → 150 filtered candidates in 25 microseconds

**Total overhead**: 1ms (compile) + 25μs (filter) = ~1ms = negligible

### Performance Comparison

| Stage | Files | Time |
|-------|-------|------|
| Trigram filtering | 100,000 → 500 | ~1ms |
| **Glob filtering** | 500 → 150 | ~1ms |
| Parallel search | 150 files | ~50ms |
| **Total** | | ~52ms |

Without trigram filtering (glob alone):
| Stage | Files | Time |
|-------|-------|------|
| Glob filtering | 100,000 → 30,000 | ~5ms |
| Parallel search | 30,000 files | ~10,000ms |
| **Total** | | ~10 seconds |

**Trigram + Glob = 200x faster** than glob alone!

### Why Order Matters: Trigram THEN Glob

We apply filters in this order:
1. Trigram filtering (narrows to ~0.5% of files)
2. Glob filtering (narrows by another ~70%)
3. Content search (only on final candidates)

This order is optimal because:
- Trigram filtering is O(1) index lookups (bitmaps)
- Glob matching is O(path_length) per file
- Content search is O(file_size) per file

By filtering most files with O(1) operations first, we minimize the expensive O(file_size) work.

### Visual Summary

```
┌──────────────────────────────────────────────────────────────────┐
│                    PATH GLOB FILTERING FLOW                      │
└──────────────────────────────────────────────────────────────────┘

 Query: "SearchEngine" + include="src/**/*.rs" + exclude="**/test/**"
         ↓
 ┌───────────────────────────────────────────┐
 │  1. TRIGRAM INDEX                         │
 │     Extract trigrams from "SearchEngine"  │
 │     Bitmap intersection                   │
 │     100,000 files → 500 candidates        │  ← O(1) lookups
 └─────────────────────┬─────────────────────┘
                       ↓
 ┌───────────────────────────────────────────┐
 │  2. COMPILE GLOB PATTERNS (once)          │
 │     include: GlobSet(["src/**/*.rs"])     │  ← ~1ms, done once
 │     exclude: GlobSet(["**/test/**"])      │
 └─────────────────────┬─────────────────────┘
                       ↓
 ┌───────────────────────────────────────────┐
 │  3. FILTER CANDIDATES                     │
 │     For each of 500 candidates:           │
 │       path = file_store.get_path(id)      │
 │       if include.is_match(path)           │
 │         && !exclude.is_match(path):       │  ← ~50ns per path
 │         keep(id)                          │
 │     500 candidates → 150 filtered         │
 └─────────────────────┬─────────────────────┘
                       ↓
 ┌───────────────────────────────────────────┐
 │  4. PARALLEL CONTENT SEARCH               │
 │     Only 150 files searched               │  ← The expensive part
 │     Uses rayon for parallelism            │    but now minimal files
 └─────────────────────┬─────────────────────┘
                       ↓
                   Results

 Performance:
 ────────────
 Step 1 (trigram):  ~1ms     (bitmap operations)
 Step 2 (compile):  ~1ms     (one-time per query)
 Step 3 (filter):   ~25μs    (500 × 50ns)
 Step 4 (search):   ~50ms    (parallel line search on 150 files)
 ─────────────────────────
 Total:             ~52ms    ✓ Fast!
```

---

### Key Takeaways

1. **Regex + Trigrams**: Extract literal substrings from regex, use them for trigram pre-filtering, run actual regex only on the small candidate set. Most queries see 100-1000x speedup.

2. **Path Globs**: Filter is applied AFTER trigram pre-filtering, so we only glob-match a few hundred paths, not 100,000. GlobSet compiles patterns into an efficient automaton for O(path_length) matching.

3. **Composition**: Both filters stack multiplicatively:
   - Trigram: 100,000 → 500 (99.5% filtered)
   - Glob: 500 → 150 (70% of remainder filtered)  
   - Net: 100,000 → 150 (99.85% filtered)

4. **Worst Case**: Regex with no literals OR no path filters = falls back to current behavior (which is already fast due to trigram index).
