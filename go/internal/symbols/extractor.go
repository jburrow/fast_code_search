// Package symbols provides regex-based symbol extraction for 12+ programming
// languages. It is the Go equivalent of the Rust src/symbols/extractor.rs.
// A full tree-sitter implementation could be substituted via CGO bindings, but
// the regex approach avoids the CGO dependency while still identifying
// functions, classes, types, and import statements.
package symbols

import (
	"path/filepath"
	"regexp"
	"strings"
)

// SymbolKind classifies what kind of code entity a Symbol represents.
type SymbolKind int

const (
	SymbolFunction  SymbolKind = iota // fn, func, def, sub
	SymbolClass                       // class, struct, interface, impl
	SymbolType                        // type alias, enum, typedef
	SymbolConstant                    // const, let (module-level)
	SymbolMethod                      // method inside a class
)

// Symbol represents an extracted code symbol (function, class, type, etc.).
type Symbol struct {
	// Name is the identifier (e.g. "MyFunc").
	Name string
	// Kind classifies the symbol.
	Kind SymbolKind
	// Line is 1-based line number in the source file.
	Line int
	// Signature is the full declaration line, if available.
	Signature string
}

// langPatterns holds per-language regex patterns for symbol extraction.
type langPatterns struct {
	functions  *regexp.Regexp
	classes    *regexp.Regexp
	types      *regexp.Regexp
	imports    *regexp.Regexp
}

// Extractor performs symbol extraction across multiple languages.
type Extractor struct {
	patterns map[string]*langPatterns // extension → patterns
}

// NewExtractor creates an Extractor with built-in language patterns.
func NewExtractor() *Extractor {
	e := &Extractor{patterns: make(map[string]*langPatterns)}
	e.registerAll()
	return e
}

// Extract returns the symbols and import paths found in content for the given
// file path. lineNumbers are 1-based. Import paths are raw strings (the
// argument to import/require/use).
func (e *Extractor) Extract(filePath, content string) ([]Symbol, []string) {
	ext := strings.ToLower(filepath.Ext(filePath))
	pat, ok := e.patterns[ext]
	if !ok {
		return nil, nil
	}

	lines := strings.Split(content, "\n")
	var syms []Symbol
	var imports []string

	for lineIdx, line := range lines {
		lineNum := lineIdx + 1
		trimmed := strings.TrimSpace(line)

		// Functions / methods.
		if pat.functions != nil {
			if m := pat.functions.FindStringSubmatch(trimmed); m != nil && len(m) > 1 {
				name := m[1]
				syms = append(syms, Symbol{
					Name:      name,
					Kind:      SymbolFunction,
					Line:      lineNum,
					Signature: trimToMaxLen(trimmed, 120),
				})
			}
		}

		// Classes / structs / interfaces.
		if pat.classes != nil {
			if m := pat.classes.FindStringSubmatch(trimmed); m != nil && len(m) > 1 {
				syms = append(syms, Symbol{
					Name:      m[1],
					Kind:      SymbolClass,
					Line:      lineNum,
					Signature: trimToMaxLen(trimmed, 120),
				})
			}
		}

		// Type aliases / enums.
		if pat.types != nil {
			if m := pat.types.FindStringSubmatch(trimmed); m != nil && len(m) > 1 {
				syms = append(syms, Symbol{
					Name:      m[1],
					Kind:      SymbolType,
					Line:      lineNum,
					Signature: trimToMaxLen(trimmed, 120),
				})
			}
		}

		// Imports.
		if pat.imports != nil {
			if m := pat.imports.FindStringSubmatch(trimmed); m != nil && len(m) > 1 {
				imp := strings.Trim(m[1], `"'`)
				if imp != "" {
					imports = append(imports, imp)
				}
			}
		}
	}

	return syms, imports
}

func trimToMaxLen(s string, max int) string {
	if len(s) <= max {
		return s
	}
	return s[:max]
}

// registerAll populates the patterns map for all supported languages.
func (e *Extractor) registerAll() {
	// Rust
	e.patterns[".rs"] = &langPatterns{
		functions: regexp.MustCompile(`^(?:pub\s+)?(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)`),
		classes:   regexp.MustCompile(`^(?:pub\s+)?(?:struct|enum|trait|impl(?:\s+\S+\s+for)?)\s+([A-Za-z_][A-Za-z0-9_]*)`),
		types:     regexp.MustCompile(`^(?:pub\s+)?type\s+([A-Za-z_][A-Za-z0-9_]*)`),
		imports:   regexp.MustCompile(`^use\s+([\w::{},\s*]+)`),
	}

	// Go
	e.patterns[".go"] = &langPatterns{
		functions: regexp.MustCompile(`^func\s+(?:\([^)]*\)\s+)?([A-Za-z_][A-Za-z0-9_]*)`),
		classes:   regexp.MustCompile(`^type\s+([A-Za-z_][A-Za-z0-9_]*)\s+(?:struct|interface)`),
		types:     regexp.MustCompile(`^type\s+([A-Za-z_][A-Za-z0-9_]*)\s+`),
		imports:   regexp.MustCompile(`"([^"]+)"`),
	}

	// Python
	e.patterns[".py"] = &langPatterns{
		functions: regexp.MustCompile(`^(?:async\s+)?def\s+([A-Za-z_][A-Za-z0-9_]*)`),
		classes:   regexp.MustCompile(`^class\s+([A-Za-z_][A-Za-z0-9_]*)`),
		imports:   regexp.MustCompile(`^(?:import|from)\s+([\w.]+)`),
	}

	// JavaScript / TypeScript
	jsTS := &langPatterns{
		functions: regexp.MustCompile(`^(?:export\s+)?(?:async\s+)?(?:function\s+([A-Za-z_$][A-Za-z0-9_$]*)|(?:const|let|var)\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*=\s*(?:async\s+)?(?:function|\())`),
		classes:   regexp.MustCompile(`^(?:export\s+)?(?:abstract\s+)?class\s+([A-Za-z_$][A-Za-z0-9_$]*)`),
		types:     regexp.MustCompile(`^(?:export\s+)?(?:interface|type)\s+([A-Za-z_$][A-Za-z0-9_$]*)`),
		imports:   regexp.MustCompile(`(?:import|require)\s*\(?["']([^"']+)["']`),
	}
	e.patterns[".js"] = jsTS
	e.patterns[".ts"] = jsTS
	e.patterns[".jsx"] = jsTS
	e.patterns[".tsx"] = jsTS

	// Java
	e.patterns[".java"] = &langPatterns{
		functions: regexp.MustCompile(`^(?:public|private|protected|static|final|synchronized|\s)*\s+\w[\w<>\[\]]*\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(`),
		classes:   regexp.MustCompile(`^(?:public|private|protected|abstract|final|\s)*\s+(?:class|interface|enum)\s+([A-Za-z_][A-Za-z0-9_]*)`),
		imports:   regexp.MustCompile(`^import\s+([\w.]+)`),
	}

	// C / C++
	cpp := &langPatterns{
		functions: regexp.MustCompile(`^(?:(?:static|inline|extern|virtual|override)\s+)*[\w:*&<>]+\s+([A-Za-z_][A-Za-z0-9_:]*)\s*\(`),
		classes:   regexp.MustCompile(`^(?:class|struct|enum(?:\s+class)?|union)\s+([A-Za-z_][A-Za-z0-9_]*)`),
		types:     regexp.MustCompile(`^typedef\s+\S+\s+([A-Za-z_][A-Za-z0-9_]*)`),
		imports:   regexp.MustCompile(`^#include\s+[<"]([^>"]+)[>"]`),
	}
	e.patterns[".c"] = cpp
	e.patterns[".cpp"] = cpp
	e.patterns[".cc"] = cpp
	e.patterns[".cxx"] = cpp
	e.patterns[".h"] = cpp
	e.patterns[".hpp"] = cpp

	// C#
	e.patterns[".cs"] = &langPatterns{
		functions: regexp.MustCompile(`^(?:public|private|protected|internal|static|virtual|override|async|\s)*\s+[\w<>\[\]?]+\s+([A-Za-z_][A-Za-z0-9_]*)\s*[\(<]`),
		classes:   regexp.MustCompile(`^(?:public|private|protected|internal|abstract|sealed|static|\s)*\s+(?:class|interface|enum|struct|record)\s+([A-Za-z_][A-Za-z0-9_]*)`),
		imports:   regexp.MustCompile(`^using\s+([\w.]+)`),
	}

	// Ruby
	e.patterns[".rb"] = &langPatterns{
		functions: regexp.MustCompile(`^(?:def\s+)([A-Za-z_][A-Za-z0-9_?!]*)`),
		classes:   regexp.MustCompile(`^(?:class|module)\s+([A-Za-z_][A-Za-z0-9_:]*)`),
		imports:   regexp.MustCompile(`^(?:require|require_relative)\s+['"]([^'"]+)['"]`),
	}

	// PHP
	e.patterns[".php"] = &langPatterns{
		functions: regexp.MustCompile(`^(?:public|private|protected|static|abstract|\s)*\s*function\s+([A-Za-z_][A-Za-z0-9_]*)`),
		classes:   regexp.MustCompile(`^(?:abstract|final|\s)*\s*(?:class|interface|trait)\s+([A-Za-z_][A-Za-z0-9_]*)`),
		imports:   regexp.MustCompile(`^(?:use|require|include)\s+['"]?([A-Za-z_\\][A-Za-z0-9_\\]*)`),
	}

	// Bash / Shell
	sh := &langPatterns{
		functions: regexp.MustCompile(`^([A-Za-z_][A-Za-z0-9_]*)\s*\(\s*\)`),
	}
	e.patterns[".sh"] = sh
	e.patterns[".bash"] = sh
	e.patterns[".zsh"] = sh
}
