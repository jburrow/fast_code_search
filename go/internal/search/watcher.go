package search

import (
	"log/slog"
	"path/filepath"
	"time"

	"github.com/fsnotify/fsnotify"
	"github.com/jburrow/fast_code_search/internal/utils"
)

// FileChangeKind describes the type of a filesystem event.
type FileChangeKind int

const (
	FileModified FileChangeKind = iota
	FileDeleted
	FileRenamed
)

// FileChange represents a single filesystem event after debouncing.
type FileChange struct {
	Path string
	Kind FileChangeKind
}

// WatcherConfig parameterises the file watcher.
type WatcherConfig struct {
	// Paths is the list of root directories to watch.
	Paths []string
	// Filter limits which change events are forwarded.
	Filter *PathFilter
	// DebounceDuration groups rapid events into a single notification.
	DebounceDuration time.Duration
}

// FileWatcher wraps fsnotify with debouncing, equivalent to the Rust FileWatcher.
type FileWatcher struct {
	cfg     WatcherConfig
	watcher *fsnotify.Watcher
	changes chan FileChange
	done    chan struct{}
}

// NewFileWatcher creates and starts a FileWatcher.
func NewFileWatcher(cfg WatcherConfig) (*FileWatcher, error) {
	w, err := fsnotify.NewWatcher()
	if err != nil {
		return nil, err
	}

	for _, p := range cfg.Paths {
		if err := w.Add(p); err != nil {
			slog.Warn("watcher: failed to watch path", "path", p, "err", err)
		}
	}

	fw := &FileWatcher{
		cfg:     cfg,
		watcher: w,
		changes: make(chan FileChange, 256),
		done:    make(chan struct{}),
	}
	go fw.run()
	return fw, nil
}

// Changes returns the channel of debounced filesystem events.
func (fw *FileWatcher) Changes() <-chan FileChange {
	return fw.changes
}

// Close stops the watcher and releases resources.
func (fw *FileWatcher) Close() error {
	close(fw.done)
	return fw.watcher.Close()
}

func (fw *FileWatcher) run() {
	pending := make(map[string]FileChangeKind)
	ticker := time.NewTicker(fw.cfg.DebounceDuration)
	defer ticker.Stop()

	flush := func() {
		for path, kind := range pending {
			canonical := utils.NormalizePath(path)
			ext := filepath.Ext(canonical)
			_ = ext // already filtered by IsTextFile in discovery
			if fw.cfg.Filter != nil && !fw.cfg.Filter.Match(path) {
				continue
			}
			select {
			case fw.changes <- FileChange{Path: canonical, Kind: kind}:
			default:
			}
		}
		pending = make(map[string]FileChangeKind)
	}

	for {
		select {
		case <-fw.done:
			return

		case event, ok := <-fw.watcher.Events:
			if !ok {
				return
			}
			var kind FileChangeKind
			switch {
			case event.Has(fsnotify.Remove):
				kind = FileDeleted
			case event.Has(fsnotify.Rename):
				kind = FileRenamed
			default:
				kind = FileModified
			}
			pending[event.Name] = kind

		case err, ok := <-fw.watcher.Errors:
			if !ok {
				return
			}
			slog.Warn("watcher error", "err", err)

		case <-ticker.C:
			if len(pending) > 0 {
				flush()
			}
		}
	}
}
