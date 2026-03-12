// Package dependencies tracks the import graph between files, enabling
// PageRank-style importance weighting during search. It mirrors the Rust
// src/dependencies/mod.rs module.
package dependencies

import (
	"sync"
)

// Graph is a bidirectional import graph: file A imports file B means
// B has A as a dependent, giving B a higher relevance score.
type Graph struct {
	mu sync.RWMutex
	// dependents maps a file path to the set of files that import it.
	dependents map[string]map[string]struct{}
	// dependencies maps a file path to the set of files it imports.
	dependencies map[string]map[string]struct{}
}

// NewGraph allocates an empty dependency graph.
func NewGraph() *Graph {
	return &Graph{
		dependents:   make(map[string]map[string]struct{}),
		dependencies: make(map[string]map[string]struct{}),
	}
}

// AddImport records that fromPath imports toPath (raw import string).
func (g *Graph) AddImport(fromPath, toPath string) {
	g.mu.Lock()
	defer g.mu.Unlock()

	if _, ok := g.dependencies[fromPath]; !ok {
		g.dependencies[fromPath] = make(map[string]struct{})
	}
	g.dependencies[fromPath][toPath] = struct{}{}

	if _, ok := g.dependents[toPath]; !ok {
		g.dependents[toPath] = make(map[string]struct{})
	}
	g.dependents[toPath][fromPath] = struct{}{}
}

// DependentCount returns how many other files import filePath.
// Used as a proxy for "importance" in relevance scoring.
func (g *Graph) DependentCount(filePath string) int {
	g.mu.RLock()
	defer g.mu.RUnlock()
	return len(g.dependents[filePath])
}

// Dependents returns the set of files that import filePath.
func (g *Graph) Dependents(filePath string) []string {
	g.mu.RLock()
	defer g.mu.RUnlock()
	deps := g.dependents[filePath]
	out := make([]string, 0, len(deps))
	for p := range deps {
		out = append(out, p)
	}
	return out
}

// Dependencies returns the set of files imported by filePath.
func (g *Graph) Dependencies(filePath string) []string {
	g.mu.RLock()
	defer g.mu.RUnlock()
	deps := g.dependencies[filePath]
	out := make([]string, 0, len(deps))
	for p := range deps {
		out = append(out, p)
	}
	return out
}
