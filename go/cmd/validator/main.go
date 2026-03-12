// fast_code_search_validator is a self-test binary that builds a small in-memory
// corpus, indexes it, and verifies that all inserted needles are findable.
// It mirrors the Rust fast_code_search_validator binary in src/bin/.
package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/jburrow/fast_code_search/internal/config"
	"github.com/jburrow/fast_code_search/internal/search"
)

func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, "FAIL:", err)
		os.Exit(1)
	}
	fmt.Println("PASS: all validator checks passed")
}

func run() error {
	// ── Build synthetic corpus ────────────────────────────────────────────────
	dir, err := os.MkdirTemp("", "fcs_validator_*")
	if err != nil {
		return fmt.Errorf("creating temp dir: %w", err)
	}
	defer os.RemoveAll(dir)

	type testFile struct {
		name   string
		needle string
		body   string
	}

	cases := []testFile{
		{
			name:   "rust_example.rs",
			needle: "fn find_needle_rust",
			body: `/// Example Rust function.
fn find_needle_rust(haystack: &str) -> bool {
    haystack.contains("needle")
}`,
		},
		{
			name:   "python_example.py",
			needle: "def find_needle_python",
			body: `# Example Python function
def find_needle_python(haystack):
    return "needle" in haystack
`,
		},
		{
			name:   "go_example.go",
			needle: "func FindNeedleGo",
			body: `package example

// FindNeedleGo searches for needle.
func FindNeedleGo(haystack string) bool {
    return strings.Contains(haystack, "needle")
}
`,
		},
		{
			name:   "java_example.java",
			needle: "public boolean findNeedleJava",
			body: `public class Example {
    public boolean findNeedleJava(String haystack) {
        return haystack.contains("needle");
    }
}`,
		},
	}

	for _, tc := range cases {
		path := filepath.Join(dir, tc.name)
		if err := os.WriteFile(path, []byte(tc.body), 0o644); err != nil {
			return fmt.Errorf("writing %s: %w", tc.name, err)
		}
	}

	// ── Index the corpus ──────────────────────────────────────────────────────
	cfg := config.DefaultConfig()
	cfg.Indexer.Paths = []string{dir}
	cfg.Indexer.PersistIndex = false
	cfg.Indexer.EnableSymbols = true

	engine := search.NewEngine(cfg)
	for _, tc := range cases {
		path := filepath.Join(dir, tc.name)
		if err := engine.IndexFile(path); err != nil {
			return fmt.Errorf("indexing %s: %w", tc.name, err)
		}
	}

	stats := engine.Stats()
	fmt.Printf("Indexed %d files, %d trigrams, %d symbols\n",
		stats.FilesIndexed, stats.NumTrigrams, stats.NumSymbols)

	// ── Verify all needles are findable ───────────────────────────────────────
	for _, tc := range cases {
		matches, err := engine.Search(search.SearchOptions{
			Query:      tc.needle,
			MaxResults: 10,
		})
		if err != nil {
			return fmt.Errorf("searching %q: %w", tc.needle, err)
		}
		found := false
		for _, m := range matches {
			if strings.Contains(m.Content, tc.needle) {
				found = true
				break
			}
		}
		if !found {
			return fmt.Errorf("needle %q not found in search results (got %d matches)", tc.needle, len(matches))
		}
		fmt.Printf("  ✓ found %q\n", tc.needle)
	}

	// ── Regex search test ─────────────────────────────────────────────────────
	reMatches, err := engine.Search(search.SearchOptions{
		Query:      `find_needle_\w+`,
		MaxResults: 20,
		IsRegex:    true,
	})
	if err != nil {
		return fmt.Errorf("regex search: %w", err)
	}
	if len(reMatches) == 0 {
		return fmt.Errorf("regex search returned no results")
	}
	fmt.Printf("  ✓ regex search found %d matches\n", len(reMatches))

	// ── Symbol search test ────────────────────────────────────────────────────
	symMatches := engine.SearchSymbols("find_needle", 20)
	if len(symMatches) == 0 {
		return fmt.Errorf("symbol search returned no results")
	}
	fmt.Printf("  ✓ symbol search found %d matches\n", len(symMatches))

	return nil
}
