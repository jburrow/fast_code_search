package search

import (
"path/filepath"
"strings"
)

// PathFilter performs include/exclude glob matching on file paths, equivalent
// to the Rust PathFilter in src/search/path_filter.rs.
type PathFilter struct {
includePatterns []string
excludePatterns []string
}

// NewPathFilter compiles include and exclude glob patterns.
// An empty include list means "include everything".
func NewPathFilter(includes, excludes []string) *PathFilter {
return &PathFilter{
includePatterns: includes,
excludePatterns: excludes,
}
}

// Match returns true when path should be searched/indexed:
//   - It must match at least one include pattern (or includes list is empty).
//   - It must not match any exclude pattern.
func (pf *PathFilter) Match(path string) bool {
// Normalise to forward slashes for consistent glob matching.
norm := filepath.ToSlash(path)

// Check exclusions first (faster bail-out).
for _, pat := range pf.excludePatterns {
if matchGlob(pat, norm) {
return false
}
}

if len(pf.includePatterns) == 0 {
return true
}
for _, pat := range pf.includePatterns {
if matchGlob(pat, norm) {
return true
}
}
return false
}

// matchGlob matches a slash-normalised path against a glob pattern.
// It handles the ** glob (zero or more path segments) which the Go stdlib
// filepath.Match does not support.
func matchGlob(pattern, path string) bool {
pattern = filepath.ToSlash(pattern)
return globMatch(pattern, path)
}

// globMatch is a recursive glob matcher that understands **.
func globMatch(pattern, path string) bool {
// Split on first occurrence of **.
idx := strings.Index(pattern, "**")
if idx == -1 {
// No ** — use stdlib matching against the full path and also the basename.
if ok, _ := filepath.Match(pattern, path); ok {
return true
}
// Also try matching pattern against just the file name.
if ok, _ := filepath.Match(pattern, filepath.Base(path)); ok {
return true
}
return false
}

prefix := pattern[:idx]
suffix := pattern[idx+2:]

// Remove leading/trailing slashes from prefix and suffix.
prefix = strings.TrimSuffix(prefix, "/")
suffix = strings.TrimPrefix(suffix, "/")

// Verify the path starts with the literal prefix (if any).
remaining := path
if prefix != "" {
if !strings.HasPrefix(path, prefix+"/") && path != prefix {
return false
}
if path == prefix {
remaining = ""
} else {
remaining = strings.TrimPrefix(path, prefix+"/")
}
}

if suffix == "" {
// ** at end — matches everything.
return true
}

// ** can match zero or more segments. Try all sub-paths.
// First try matching suffix against the full remaining path.
if globMatch(suffix, remaining) {
return true
}

// Then skip segments one at a time.
cur := remaining
for {
slash := strings.Index(cur, "/")
if slash < 0 {
break
}
cur = cur[slash+1:]
if globMatch(suffix, cur) {
return true
}
}
return false
}
