package search

import (
	"os"
	"path/filepath"
	"strings"

	"github.com/jburrow/fast_code_search/internal/utils"
)

// FileDiscoveryConfig controls how files are enumerated.
type FileDiscoveryConfig struct {
	// Paths is the list of roots to walk.
	Paths []string
	// Filter applies include/exclude patterns.
	Filter *PathFilter
	// MaxFileSizeBytes skips files larger than this (0 = unlimited).
	MaxFileSizeBytes int64
	// FollowSymlinks enables following symbolic links.
	FollowSymlinks bool
}

// DiscoverFiles walks all configured paths and returns matching file paths.
// Results are deduplicated by canonical path.
func DiscoverFiles(cfg FileDiscoveryConfig) ([]string, error) {
	seen := make(map[string]struct{})
	var results []string

	for _, root := range cfg.Paths {
		err := filepath.Walk(root, func(path string, info os.FileInfo, err error) error {
			if err != nil {
				return nil // skip unreadable directories
			}
			if info.IsDir() {
				// Skip hidden directories.
				if strings.HasPrefix(info.Name(), ".") && info.Name() != "." {
					return filepath.SkipDir
				}
				return nil
			}

			// Skip symlinks unless configured.
			if !cfg.FollowSymlinks && (info.Mode()&os.ModeSymlink != 0) {
				return nil
			}

			// Size check.
			if cfg.MaxFileSizeBytes > 0 && info.Size() > cfg.MaxFileSizeBytes {
				return nil
			}

			// Extension / text file check.
			if !utils.IsTextFile(path) {
				return nil
			}

			// Pattern filter.
			if cfg.Filter != nil && !cfg.Filter.Match(path) {
				return nil
			}

			canonical := utils.NormalizePath(path)
			if _, dup := seen[canonical]; dup {
				return nil
			}
			seen[canonical] = struct{}{}
			results = append(results, path)
			return nil
		})
		if err != nil {
			return nil, err
		}
	}
	return results, nil
}
