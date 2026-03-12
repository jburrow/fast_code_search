package search

import (
	"fmt"
	"log/slog"
	"math"
	"runtime"
	"sort"
	"strings"
	"sync"

	"github.com/jburrow/fast_code_search/internal/config"
	"github.com/jburrow/fast_code_search/internal/dependencies"
	"github.com/jburrow/fast_code_search/internal/index"
	"github.com/jburrow/fast_code_search/internal/symbols"
)

// MatchType categorises what kind of match was found.
type MatchType int

const (
	MatchText            MatchType = 0
	MatchSymbolDef       MatchType = 1
	MatchSymbolReference MatchType = 2
)

// SearchMatch is a single result returned by the search engine.
type SearchMatch struct {
	// FilePath is the canonical slash-separated absolute path.
	FilePath string
	// Content is the matching line (possibly truncated).
	Content string
	// LineNumber is 1-based.
	LineNumber int
	// Score is the relevance score (higher = better).
	Score float64
	// MatchType distinguishes text vs symbol matches.
	MatchType MatchType
	// MatchStart/End are byte offsets within Content.
	MatchStart int
	MatchEnd   int
	// ContentTruncated is true when the line was trimmed to MaxLineLen.
	ContentTruncated bool
}

// SearchOptions parameterises a single search request.
type SearchOptions struct {
	// Query is the literal string or regex pattern.
	Query string
	// MaxResults caps the number of returned matches.
	MaxResults int
	// IsRegex treats Query as a regular expression.
	IsRegex bool
	// CaseInsensitive enables case-insensitive matching.
	CaseInsensitive bool
	// SymbolsOnly restricts results to symbol definition/reference lines.
	SymbolsOnly bool
	// IncludePatterns and ExcludePatterns filter results by path.
	IncludePatterns []string
	ExcludePatterns []string
}

// IndexingStats reports current index size metrics.
type IndexingStats struct {
	FilesIndexed  int
	TotalBytes    int64
	NumTrigrams   int
	TotalPostings uint64
	NumSymbols    int
}

// Engine is the central search engine, equivalent to Rust's SearchEngine.
// It holds the trigram index, file store, symbol cache, and dependency graph
// and provides thread-safe search and indexing methods.
type Engine struct {
	mu sync.RWMutex

	fileStore   *index.FileStore
	trigramIdx  *index.TrigramIndex
	persistence *index.PersistenceManager
	symExtractor *symbols.Extractor
	depGraph    *dependencies.Graph

	// symbolsByFile maps docID → extracted symbols.
	symbolsByFile map[uint32][]symbols.Symbol

	cfg *config.Config
}

// NewEngine creates an empty Engine using the provided configuration.
func NewEngine(cfg *config.Config) *Engine {
	e := &Engine{
		fileStore:     index.NewFileStore(),
		trigramIdx:    index.NewTrigramIndex(),
		symExtractor:  symbols.NewExtractor(),
		depGraph:      dependencies.NewGraph(),
		symbolsByFile: make(map[uint32][]symbols.Symbol),
		cfg:           cfg,
	}
	if cfg.Indexer.PersistIndex && cfg.Indexer.IndexPath != "" {
		e.persistence = index.NewPersistenceManager(cfg.Indexer.IndexPath)
	}
	return e
}

// IndexFile adds a single file to the search index. It is safe to call
// concurrently — a file-level mutex per docID is not needed because the
// FileStore handles concurrency internally.
func (e *Engine) IndexFile(path string) error {
	docID, err := e.fileStore.AddFile(path, e.cfg.Indexer.MaxFileSizeBytes)
	if err != nil {
		return fmt.Errorf("indexing %q: %w", path, err)
	}

	mf := e.fileStore.Get(docID)
	if mf == nil || mf.IsBinary {
		return nil
	}

	e.mu.Lock()
	defer e.mu.Unlock()

	// Add trigrams.
	e.trigramIdx.Add(docID, []byte(mf.Content))

	// Extract symbols if enabled.
	if e.cfg.Indexer.EnableSymbols {
		syms, imports := e.symExtractor.Extract(path, mf.Content)
		e.symbolsByFile[docID] = syms

		if e.cfg.Indexer.EnableDependencies {
			for _, imp := range imports {
				e.depGraph.AddImport(path, imp)
			}
		}
	}
	return nil
}

// IndexBatch indexes multiple files in parallel using up to numWorkers
// goroutines. Errors are logged and do not abort the batch.
func (e *Engine) IndexBatch(paths []string, numWorkers int) int {
	if numWorkers <= 0 {
		numWorkers = runtime.NumCPU()
	}

	type job struct{ path string }
	jobs := make(chan job, len(paths))
	for _, p := range paths {
		jobs <- job{p}
	}
	close(jobs)

	var wg sync.WaitGroup
	var mu sync.Mutex
	indexed := 0

	for i := 0; i < numWorkers; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for j := range jobs {
				if err := e.IndexFile(j.path); err != nil {
					slog.Warn("index error", "path", j.path, "err", err)
					continue
				}
				mu.Lock()
				indexed++
				mu.Unlock()
			}
		}()
	}
	wg.Wait()
	return indexed
}

// Search executes a keyword or regex search and returns up to opts.MaxResults
// ranked matches. It mirrors the Rust search() / search_ranked() methods.
func (e *Engine) Search(opts SearchOptions) ([]SearchMatch, error) {
	if opts.MaxResults <= 0 {
		opts.MaxResults = 100
	}

	e.mu.RLock()
	defer e.mu.RUnlock()

	// Build path filter.
	filter := NewPathFilter(opts.IncludePatterns, opts.ExcludePatterns)

	if opts.IsRegex {
		return e.searchRegex(opts, filter)
	}
	return e.searchKeyword(opts, filter)
}

// SearchSymbols returns matches where the query names a known symbol.
func (e *Engine) SearchSymbols(query string, maxResults int) []SearchMatch {
	if maxResults <= 0 {
		maxResults = 100
	}

	e.mu.RLock()
	defer e.mu.RUnlock()

	queryLower := strings.ToLower(query)
	var results []SearchMatch

	for docID, syms := range e.symbolsByFile {
		mf := e.fileStore.Get(docID)
		if mf == nil {
			continue
		}
		for _, sym := range syms {
			if !strings.Contains(strings.ToLower(sym.Name), queryLower) {
				continue
			}
			content := sym.Name
			if sym.Signature != "" {
				content = sym.Signature
			}
			score := 3.0 // symbol definition boost (mirrors Rust 3x)
			if strings.EqualFold(sym.Name, query) {
				score += 2.0 // exact match boost
			}
			results = append(results, SearchMatch{
				FilePath:   mf.Path,
				Content:    content,
				LineNumber: sym.Line,
				Score:      score,
				MatchType:  MatchSymbolDef,
			})
		}
	}

	sort.Slice(results, func(i, j int) bool {
		return results[i].Score > results[j].Score
	})
	if len(results) > maxResults {
		results = results[:maxResults]
	}
	return results
}

// SaveIndex persists the current index to disk.
func (e *Engine) SaveIndex(configFingerprint string) error {
	if e.persistence == nil {
		return fmt.Errorf("persistence not configured")
	}
	e.mu.RLock()
	defer e.mu.RUnlock()
	return e.persistence.Save(e.fileStore, e.trigramIdx, configFingerprint)
}

// LoadIndex restores a previously saved index from disk.
func (e *Engine) LoadIndex() error {
	if e.persistence == nil {
		return fmt.Errorf("persistence not configured")
	}
	lr, err := e.persistence.Load()
	if err != nil {
		return err
	}
	e.mu.Lock()
	defer e.mu.Unlock()
	return index.RestoreInto(lr, e.fileStore, e.trigramIdx)
}

// Stats returns current index statistics.
func (e *Engine) Stats() IndexingStats {
	e.mu.RLock()
	defer e.mu.RUnlock()

	numTrigrams, totalPostings := e.trigramIdx.Stats()
	totalBytes := int64(0)
	allFiles := e.fileStore.AllFiles()
	for _, mf := range allFiles {
		totalBytes += mf.Size
	}

	numSymbols := 0
	for _, syms := range e.symbolsByFile {
		numSymbols += len(syms)
	}

	return IndexingStats{
		FilesIndexed:  e.fileStore.Count(),
		TotalBytes:    totalBytes,
		NumTrigrams:   numTrigrams,
		TotalPostings: totalPostings,
		NumSymbols:    numSymbols,
	}
}

// --- internal helpers -------------------------------------------------------

const maxLineLen = 512

func (e *Engine) searchKeyword(opts SearchOptions, filter *PathFilter) ([]SearchMatch, error) {
	query := opts.Query
	if opts.CaseInsensitive {
		query = strings.ToLower(query)
	}

	// Use trigram index to get candidates only for case-sensitive queries.
	// Case-insensitive queries cannot use the trigram index (which was built
	// on the original-case content).
	var candidates []uint32
	if opts.CaseInsensitive {
		// Search all documents.
		allFiles := e.fileStore.AllFiles()
		candidates = make([]uint32, len(allFiles))
		for i := range allFiles {
			candidates[i] = uint32(i)
		}
	} else {
		candidateBM := e.trigramIdx.Candidates([]byte(query))
		if candidateBM == nil {
			// Short query — search all docs.
			allFiles := e.fileStore.AllFiles()
			candidates = make([]uint32, len(allFiles))
			for i := range allFiles {
				candidates[i] = uint32(i)
			}
		} else {
			iter := candidateBM.Iterator()
			for iter.HasNext() {
				candidates = append(candidates, iter.Next())
			}
		}
	}

	// Parallel search over candidates.
	type partial struct {
		matches []SearchMatch
	}
	numWorkers := runtime.NumCPU()
	chunkSize := (len(candidates) + numWorkers - 1) / numWorkers
	resultCh := make(chan partial, numWorkers)
	var wg sync.WaitGroup

	for start := 0; start < len(candidates); start += chunkSize {
		end := start + chunkSize
		if end > len(candidates) {
			end = len(candidates)
		}
		chunk := candidates[start:end]
		wg.Add(1)
		go func(chunk []uint32) {
			defer wg.Done()
			var local []SearchMatch
			for _, docID := range chunk {
				mf := e.fileStore.Get(docID)
				if mf == nil || mf.IsBinary || mf.Content == "" {
					continue
				}
				if !filter.Match(mf.Path) {
					continue
				}
				matches := findKeywordMatches(mf, query, opts, docID, e.symbolsByFile[docID], e.depGraph)
				local = append(local, matches...)
			}
			resultCh <- partial{local}
		}(chunk)
	}

	go func() {
		wg.Wait()
		close(resultCh)
	}()

	var all []SearchMatch
	for p := range resultCh {
		all = append(all, p.matches...)
	}

	sort.Slice(all, func(i, j int) bool { return all[i].Score > all[j].Score })
	if len(all) > opts.MaxResults {
		all = all[:opts.MaxResults]
	}
	return all, nil
}

func (e *Engine) searchRegex(opts SearchOptions, filter *PathFilter) ([]SearchMatch, error) {
	analysis, err := AnalyzeRegex(opts.Query, opts.CaseInsensitive)
	if err != nil {
		return nil, fmt.Errorf("invalid regex %q: %w", opts.Query, err)
	}

	// Use longest literal for trigram pre-filtering.
	candidate := analysis.CandidateQuery()
	var candidateIDs []uint32
	if len(candidate) >= 3 {
		bm := e.trigramIdx.Candidates([]byte(candidate))
		if bm != nil {
			iter := bm.Iterator()
			for iter.HasNext() {
				candidateIDs = append(candidateIDs, iter.Next())
			}
		}
	}
	if len(candidateIDs) == 0 {
		allFiles := e.fileStore.AllFiles()
		candidateIDs = make([]uint32, len(allFiles))
		for i := range allFiles {
			candidateIDs[i] = uint32(i)
		}
	}

	var results []SearchMatch
	for _, docID := range candidateIDs {
		mf := e.fileStore.Get(docID)
		if mf == nil || mf.IsBinary || mf.Content == "" {
			continue
		}
		if !filter.Match(mf.Path) {
			continue
		}
		lines := strings.Split(mf.Content, "\n")
		for lineIdx, line := range lines {
			loc := analysis.Pattern.FindStringIndex(line)
			if loc == nil {
				continue
			}
			truncated := false
			content := line
			if len(content) > maxLineLen {
				content = content[:maxLineLen]
				truncated = true
			}
			score := calculateScore(mf.Path, line, e.symbolsByFile[docID], e.depGraph, lineIdx+1)
			results = append(results, SearchMatch{
				FilePath:         mf.Path,
				Content:          content,
				LineNumber:       lineIdx + 1,
				Score:            score,
				MatchType:        MatchText,
				MatchStart:       loc[0],
				MatchEnd:         loc[1],
				ContentTruncated: truncated,
			})
		}
	}

	sort.Slice(results, func(i, j int) bool { return results[i].Score > results[j].Score })
	if len(results) > opts.MaxResults {
		results = results[:opts.MaxResults]
	}
	return results, nil
}

// findKeywordMatches searches a single file for keyword matches.
func findKeywordMatches(
	mf *index.MappedFile,
	query string,
	opts SearchOptions,
	docID uint32,
	syms []symbols.Symbol,
	depGraph *dependencies.Graph,
) []SearchMatch {
	lines := strings.Split(mf.Content, "\n")
	var results []SearchMatch

	for lineIdx, line := range lines {
		searchLine := line
		if opts.CaseInsensitive {
			searchLine = strings.ToLower(line)
		}

		idx := strings.Index(searchLine, query)
		if idx < 0 {
			continue
		}

		// Symbol-only filter: skip lines not containing a symbol.
		if opts.SymbolsOnly && !lineHasSymbol(lineIdx+1, syms) {
			continue
		}

		truncated := false
		content := line
		if len(content) > maxLineLen {
			content = content[:maxLineLen]
			truncated = true
		}

		score := calculateScore(mf.Path, line, syms, depGraph, lineIdx+1)

		// Exact-match boost.
		if strings.EqualFold(line, query) || strings.Contains(line, query) {
			score *= 2.0
		}

		end := idx + len(query)
		if end > len(content) {
			end = len(content)
		}
		results = append(results, SearchMatch{
			FilePath:         mf.Path,
			Content:          content,
			LineNumber:       lineIdx + 1,
			Score:            score,
			MatchType:        matchTypeForLine(lineIdx+1, syms),
			MatchStart:       idx,
			MatchEnd:         end,
			ContentTruncated: truncated,
		})
	}
	return results
}

// calculateScore computes a relevance score for a match, applying the same
// heuristics as the Rust scoring model:
//   - Symbol definitions: 3× boost
//   - src/ or lib/ directory: 1.5× boost
//   - Import count (log-scaled): up to 1.0 additive bonus
func calculateScore(
	filePath string,
	line string,
	syms []symbols.Symbol,
	depGraph *dependencies.Graph,
	lineNumber int,
) float64 {
	score := 1.0

	// Symbol definition boost.
	for _, sym := range syms {
		if sym.Line == lineNumber {
			score *= 3.0
			break
		}
	}

	// Source directory boost.
	lp := strings.ToLower(filePath)
	if strings.Contains(lp, "/src/") || strings.Contains(lp, "/lib/") {
		score *= 1.5
	}

	// Import count boost (PageRank-style).
	importCount := depGraph.DependentCount(filePath)
	if importCount > 0 {
		score += math.Log1p(float64(importCount))
	}

	return score
}

func lineHasSymbol(lineNumber int, syms []symbols.Symbol) bool {
	for _, sym := range syms {
		if sym.Line == lineNumber {
			return true
		}
	}
	return false
}

func matchTypeForLine(lineNumber int, syms []symbols.Symbol) MatchType {
	for _, sym := range syms {
		if sym.Line == lineNumber {
			return MatchSymbolDef
		}
	}
	return MatchText
}
