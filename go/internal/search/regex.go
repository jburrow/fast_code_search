package search

import (
	"regexp"
	"strings"
)

// RegexAnalysis holds the results of analysing a regex pattern for trigram
// acceleration. It mirrors the Rust RegexAnalysis in src/search/regex_search.rs.
type RegexAnalysis struct {
	// Pattern is the compiled regular expression.
	Pattern *regexp.Regexp
	// LiteralSubstrings are fixed strings that any match must contain.
	// Used to pre-filter candidate documents via the trigram index.
	LiteralSubstrings []string
	// IsLiteral is true when the pattern is a plain string (no meta-chars).
	IsLiteral bool
}

// AnalyzeRegex compiles pattern and extracts literal prefixes/substrings for
// trigram pre-filtering.
func AnalyzeRegex(pattern string, caseInsensitive bool) (*RegexAnalysis, error) {
	flags := ""
	if caseInsensitive {
		flags = "(?i)"
	}
	re, err := regexp.Compile(flags + pattern)
	if err != nil {
		return nil, err
	}

	literals := extractLiterals(pattern)
	isLiteral := isPlainString(pattern)

	return &RegexAnalysis{
		Pattern:           re,
		LiteralSubstrings: literals,
		IsLiteral:         isLiteral,
	}, nil
}

// CandidateQuery returns the longest literal substring suitable for trigram
// pre-filtering. Returns empty string when no suitable literal was found.
func (ra *RegexAnalysis) CandidateQuery() string {
	best := ""
	for _, lit := range ra.LiteralSubstrings {
		if len(lit) > len(best) {
			best = lit
		}
	}
	return best
}

// isPlainString returns true when pattern contains no regex meta-characters.
func isPlainString(pattern string) bool {
	metaChars := `\.+*?()|[]{}^$`
	return !strings.ContainsAny(pattern, metaChars)
}

// extractLiterals pulls out literal byte sequences from a regex pattern.
// The approach is conservative: only clearly literal runs are returned.
func extractLiterals(pattern string) []string {
	var literals []string
	var cur strings.Builder

	flush := func() {
		s := cur.String()
		if len(s) >= 3 { // trigrams need at least 3 chars
			literals = append(literals, s)
		}
		cur.Reset()
	}

	i := 0
	for i < len(pattern) {
		c := pattern[i]
		switch c {
		case '\\':
			if i+1 < len(pattern) {
				next := pattern[i+1]
				// Escaped literal characters.
				if isLiteralEscape(next) {
					cur.WriteByte(next)
					i += 2
					continue
				}
			}
			flush()
			i++
		case '.', '+', '*', '?', '(', ')', '|', '[', ']', '{', '}', '^', '$':
			flush()
			i++
		default:
			cur.WriteByte(c)
			i++
		}
	}
	flush()
	return literals
}

func isLiteralEscape(c byte) bool {
	return c == '.' || c == '+' || c == '*' || c == '?' ||
		c == '(' || c == ')' || c == '|' || c == '[' || c == ']' ||
		c == '{' || c == '}' || c == '^' || c == '$' || c == '\\'
}
