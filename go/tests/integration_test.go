package tests

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/jburrow/fast_code_search/internal/config"
	"github.com/jburrow/fast_code_search/internal/index"
	"github.com/jburrow/fast_code_search/internal/search"
	"github.com/jburrow/fast_code_search/internal/symbols"
)

// ── helpers ──────────────────────────────────────────────────────────────────

func setupEngine(t *testing.T) (*search.Engine, string) {
	t.Helper()
	dir := t.TempDir()
	cfg := config.DefaultConfig()
	cfg.Indexer.Paths = []string{dir}
	cfg.Indexer.PersistIndex = false
	cfg.Indexer.EnableSymbols = true
	return search.NewEngine(cfg), dir
}

func writeFile(t *testing.T, dir, name, content string) string {
	t.Helper()
	path := filepath.Join(dir, name)
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatalf("writing %s: %v", name, err)
	}
	return path
}

// ── trigram index ─────────────────────────────────────────────────────────────

func TestTrigramIndex_AddAndCandidates(t *testing.T) {
	ti := index.NewTrigramIndex()
	ti.Add(0, []byte("hello world"))
	ti.Add(1, []byte("foo bar baz"))

	bm := ti.Candidates([]byte("hello"))
	if bm == nil || !bm.Contains(0) {
		t.Error("expected doc 0 to be a candidate for 'hello'")
	}
	if bm.Contains(1) {
		t.Error("doc 1 should not be a candidate for 'hello'")
	}
}

func TestTrigramIndex_ShortQuery(t *testing.T) {
	ti := index.NewTrigramIndex()
	ti.Add(0, []byte("ab"))
	// Query shorter than 3 bytes → nil (search all).
	bm := ti.Candidates([]byte("ab"))
	if bm != nil {
		t.Errorf("expected nil for short query, got %v", bm)
	}
}

func TestTrigramIndex_MarshalUnmarshal(t *testing.T) {
	ti := index.NewTrigramIndex()
	ti.Add(0, []byte("hello world"))
	ti.Add(1, []byte("foo bar baz"))

	data, err := ti.MarshalBinary()
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	ti2 := index.NewTrigramIndex()
	if err := ti2.UnmarshalBinary(data); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}

	bm := ti2.Candidates([]byte("hello"))
	if bm == nil || !bm.Contains(0) {
		t.Error("expected doc 0 after round-trip")
	}
}

// ── search engine ─────────────────────────────────────────────────────────────

func TestEngine_BasicKeywordSearch(t *testing.T) {
	engine, dir := setupEngine(t)
	writeFile(t, dir, "main.go", `package main
func hello() string { return "hello world" }
`)
	_ = engine.IndexFile(filepath.Join(dir, "main.go"))

	matches, err := engine.Search(search.SearchOptions{Query: "hello world", MaxResults: 10})
	if err != nil {
		t.Fatalf("search: %v", err)
	}
	if len(matches) == 0 {
		t.Fatal("expected at least one match")
	}
	if !strings.Contains(matches[0].Content, "hello world") {
		t.Errorf("unexpected content: %q", matches[0].Content)
	}
}

func TestEngine_RegexSearch(t *testing.T) {
	engine, dir := setupEngine(t)
	writeFile(t, dir, "lib.py", `def calculate_sum(a, b):
    return a + b

def calculate_product(a, b):
    return a * b
`)
	_ = engine.IndexFile(filepath.Join(dir, "lib.py"))

	matches, err := engine.Search(search.SearchOptions{
		Query:      `calculate_\w+`,
		MaxResults: 10,
		IsRegex:    true,
	})
	if err != nil {
		t.Fatalf("regex search: %v", err)
	}
	if len(matches) < 2 {
		t.Errorf("expected ≥2 regex matches, got %d", len(matches))
	}
}

func TestEngine_CaseInsensitiveSearch(t *testing.T) {
	engine, dir := setupEngine(t)
	writeFile(t, dir, "util.rs", `pub fn ParseJSON(input: &str) {}`)
	_ = engine.IndexFile(filepath.Join(dir, "util.rs"))

	matches, err := engine.Search(search.SearchOptions{
		Query:           "parsejson",
		MaxResults:      10,
		CaseInsensitive: true,
	})
	if err != nil {
		t.Fatalf("case-insensitive search: %v", err)
	}
	if len(matches) == 0 {
		t.Error("expected at least one case-insensitive match")
	}
}

func TestEngine_SymbolSearch(t *testing.T) {
	engine, dir := setupEngine(t)
	writeFile(t, dir, "api.go", `package api
func GetUser(id int) User { return User{} }
func CreateUser(u User) error { return nil }
`)
	_ = engine.IndexFile(filepath.Join(dir, "api.go"))

	matches := engine.SearchSymbols("GetUser", 10)
	if len(matches) == 0 {
		t.Fatal("expected symbol match for GetUser")
	}
	if matches[0].MatchType != search.MatchSymbolDef {
		t.Errorf("expected MatchSymbolDef, got %v", matches[0].MatchType)
	}
}

func TestEngine_PathFilter(t *testing.T) {
	engine, dir := setupEngine(t)
	writeFile(t, dir, "include.go", `package main // needle`)
	writeFile(t, dir, "exclude.go", `package main // needle`)
	_ = engine.IndexFile(filepath.Join(dir, "include.go"))
	_ = engine.IndexFile(filepath.Join(dir, "exclude.go"))

	matches, err := engine.Search(search.SearchOptions{
		Query:           "needle",
		MaxResults:      10,
		IncludePatterns: []string{"**/include.go"},
	})
	if err != nil {
		t.Fatalf("filtered search: %v", err)
	}
	for _, m := range matches {
		if strings.Contains(m.FilePath, "exclude.go") {
			t.Errorf("excluded file appeared in results: %s", m.FilePath)
		}
	}
}

func TestEngine_Batch(t *testing.T) {
	engine, dir := setupEngine(t)
	paths := make([]string, 5)
	for i := range paths {
		paths[i] = writeFile(t, dir, strings.Repeat("f", i+1)+".go",
			`package p // unique_needle_`+strings.Repeat("x", i))
	}

	n := engine.IndexBatch(paths, 2)
	if n != 5 {
		t.Errorf("expected 5 indexed, got %d", n)
	}
}

func TestEngine_Persistence(t *testing.T) {
	dir := t.TempDir()
	cfg := config.DefaultConfig()
	cfg.Indexer.Paths = []string{dir}
	cfg.Indexer.PersistIndex = true
	cfg.Indexer.IndexPath = filepath.Join(dir, "index.bin")
	cfg.Indexer.EnableSymbols = false

	engine := search.NewEngine(cfg)
	p := writeFile(t, dir, "src.rs", `fn persist_test_needle() {}`)
	_ = engine.IndexFile(p)

	if err := engine.SaveIndex("test-fingerprint"); err != nil {
		t.Fatalf("save: %v", err)
	}

	engine2 := search.NewEngine(cfg)
	if err := engine2.LoadIndex(); err != nil {
		t.Fatalf("load: %v", err)
	}
	if engine2.Stats().FilesIndexed == 0 {
		t.Error("expected files after loading persisted index")
	}

	matches, err := engine2.Search(search.SearchOptions{Query: "persist_test_needle", MaxResults: 5})
	if err != nil {
		t.Fatalf("search after load: %v", err)
	}
	if len(matches) == 0 {
		t.Error("expected match after loading persisted index")
	}
}

// ── symbol extractor ──────────────────────────────────────────────────────────

func TestSymbolExtractor_Go(t *testing.T) {
	ext := symbols.NewExtractor()
	syms, imports := ext.Extract("pkg.go", `package pkg
import "fmt"
func Greet(name string) string {
    return fmt.Sprintf("Hello, %s", name)
}
type User struct { Name string }
`)
	if len(syms) == 0 {
		t.Fatal("expected symbols in Go file")
	}
	found := false
	for _, s := range syms {
		if s.Name == "Greet" {
			found = true
		}
	}
	if !found {
		t.Errorf("expected Greet symbol; got %v", syms)
	}
	if len(imports) == 0 {
		t.Error("expected at least one import")
	}
}

func TestSymbolExtractor_Rust(t *testing.T) {
	ext := symbols.NewExtractor()
	syms, _ := ext.Extract("lib.rs", `pub fn public_func() {}
fn private_func() {}
pub struct MyStruct {}
pub trait MyTrait {}
`)
	names := make(map[string]bool)
	for _, s := range syms {
		names[s.Name] = true
	}
	for _, want := range []string{"public_func", "private_func", "MyStruct", "MyTrait"} {
		if !names[want] {
			t.Errorf("expected symbol %q, got %v", want, syms)
		}
	}
}

func TestSymbolExtractor_Python(t *testing.T) {
	ext := symbols.NewExtractor()
	syms, _ := ext.Extract("script.py", `class DataProcessor:
    def process(self, data):
        pass

async def async_worker():
    pass
`)
	names := make(map[string]bool)
	for _, s := range syms {
		names[s.Name] = true
	}
	for _, want := range []string{"DataProcessor", "process", "async_worker"} {
		if !names[want] {
			t.Errorf("missing symbol %q", want)
		}
	}
}

// ── path filter ───────────────────────────────────────────────────────────────

func TestPathFilter_IncludeExclude(t *testing.T) {
	f := search.NewPathFilter(
		[]string{"**/*.go"},
		[]string{"**/vendor/**"},
	)
	if !f.Match("src/main.go") {
		t.Error("src/main.go should match")
	}
	if f.Match("src/vendor/dep/lib.go") {
		t.Error("vendor path should be excluded")
	}
	if f.Match("src/main.py") {
		t.Error(".py should not match *.go pattern")
	}
}

// ── stats ─────────────────────────────────────────────────────────────────────

func TestEngine_Stats(t *testing.T) {
	engine, dir := setupEngine(t)
	writeFile(t, dir, "a.go", `package a`)
	writeFile(t, dir, "b.rs", `fn main() {}`)
	engine.IndexBatch([]string{
		filepath.Join(dir, "a.go"),
		filepath.Join(dir, "b.rs"),
	}, 2)

	stats := engine.Stats()
	if stats.FilesIndexed < 2 {
		t.Errorf("expected ≥2 files indexed, got %d", stats.FilesIndexed)
	}
	if stats.NumTrigrams == 0 {
		t.Error("expected >0 trigrams")
	}
}
