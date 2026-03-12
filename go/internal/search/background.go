package search

import (
	"context"
	"log/slog"
	"sync"
	"time"

	"github.com/jburrow/fast_code_search/internal/config"
)

// IndexingProgress reports incremental background-indexer progress.
type IndexingProgress struct {
	FilesIndexed int
	TotalFiles   int
	Done         bool
	Error        error
}

// BackgroundIndexer discovers and indexes files incrementally, and optionally
// watches for filesystem changes. It mirrors the Rust background_indexer.rs.
type BackgroundIndexer struct {
	engine   *Engine
	cfg      *config.Config
	progress chan IndexingProgress
	watcher  *FileWatcher
	mu       sync.Mutex
	running  bool
}

// NewBackgroundIndexer creates an indexer that will write to engine.
func NewBackgroundIndexer(engine *Engine, cfg *config.Config) *BackgroundIndexer {
	return &BackgroundIndexer{
		engine:   engine,
		cfg:      cfg,
		progress: make(chan IndexingProgress, 64),
	}
}

// Progress returns a channel that emits incremental progress events.
func (bi *BackgroundIndexer) Progress() <-chan IndexingProgress {
	return bi.progress
}

// Run starts background indexing and blocks until ctx is cancelled.
// It first does a full initial index pass, then (optionally) watches for
// changes. Equivalent to the Rust run_background_indexer() function.
func (bi *BackgroundIndexer) Run(ctx context.Context) error {
	bi.mu.Lock()
	if bi.running {
		bi.mu.Unlock()
		return nil
	}
	bi.running = true
	bi.mu.Unlock()

	filter := NewPathFilter(
		bi.cfg.Indexer.IncludePatterns,
		bi.cfg.Indexer.ExcludePatterns,
	)

	// ── Initial full scan ────────────────────────────────────────────────────
	slog.Info("background indexer: starting initial scan", "paths", bi.cfg.Indexer.Paths)
	files, err := DiscoverFiles(FileDiscoveryConfig{
		Paths:            bi.cfg.Indexer.Paths,
		Filter:           filter,
		MaxFileSizeBytes: bi.cfg.Indexer.MaxFileSizeBytes,
	})
	if err != nil {
		bi.emit(IndexingProgress{Error: err, Done: true})
		return err
	}

	total := len(files)
	slog.Info("background indexer: discovered files", "count", total)

	batchSize := bi.cfg.Indexer.BatchSize
	if batchSize <= 0 {
		batchSize = 200
	}
	indexed := 0

	for start := 0; start < total; start += batchSize {
		select {
		case <-ctx.Done():
			bi.emit(IndexingProgress{FilesIndexed: indexed, TotalFiles: total, Done: true})
			return ctx.Err()
		default:
		}

		end := start + batchSize
		if end > total {
			end = total
		}
		n := bi.engine.IndexBatch(files[start:end], bi.cfg.Indexer.NumWorkers)
		indexed += n
		bi.emit(IndexingProgress{FilesIndexed: indexed, TotalFiles: total})
	}

	// Persist after initial scan.
	if bi.cfg.Indexer.PersistIndex {
		if err := bi.engine.SaveIndex(""); err != nil {
			slog.Warn("background indexer: failed to persist index", "err", err)
		}
	}

	slog.Info("background indexer: initial scan complete", "files_indexed", indexed)
	bi.emit(IndexingProgress{FilesIndexed: indexed, TotalFiles: total, Done: true})

	// ── Watch for changes ────────────────────────────────────────────────────
	if !bi.cfg.Indexer.WatchForChanges {
		return nil
	}

	fw, err := NewFileWatcher(WatcherConfig{
		Paths:            bi.cfg.Indexer.Paths,
		Filter:           filter,
		DebounceDuration: bi.cfg.Indexer.DebounceDuration,
	})
	if err != nil {
		slog.Warn("background indexer: could not start watcher", "err", err)
		return nil
	}
	defer fw.Close()

	slog.Info("background indexer: watching for changes")
	saveTicker := time.NewTicker(30 * time.Second)
	defer saveTicker.Stop()

	for {
		select {
		case <-ctx.Done():
			return ctx.Err()

		case change := <-fw.Changes():
			switch change.Kind {
			case FileDeleted, FileRenamed:
				// Re-index or remove — for simplicity we re-index if file exists.
				_ = bi.engine.IndexFile(change.Path)
			default:
				if err := bi.engine.IndexFile(change.Path); err != nil {
					slog.Warn("watcher: failed to re-index", "path", change.Path, "err", err)
				}
			}

		case <-saveTicker.C:
			if bi.cfg.Indexer.PersistIndex {
				if err := bi.engine.SaveIndex(""); err != nil {
					slog.Warn("background indexer: periodic save failed", "err", err)
				}
			}
		}
	}
}

func (bi *BackgroundIndexer) emit(p IndexingProgress) {
	select {
	case bi.progress <- p:
	default:
	}
}
