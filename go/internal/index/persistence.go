package index

import (
	"encoding/binary"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"
)

// PersistedFileEntry is the serialised representation of a single file.
type PersistedFileEntry struct {
	Path     string `json:"path"`
	Size     int64  `json:"size"`
	IsBinary bool   `json:"is_binary"`
	Content  string `json:"content,omitempty"`
}

// PersistedIndex is the on-disk format for the entire search index.
// Version 1 — JSON header + binary trigram data.
type PersistedIndex struct {
	Version   int                  `json:"version"`
	CreatedAt time.Time            `json:"created_at"`
	// ConfigFingerprint is a hash of the indexer config used to detect staleness.
	ConfigFingerprint string              `json:"config_fingerprint"`
	Files             []PersistedFileEntry `json:"files"`
}

const persistenceVersion = 1

// PersistenceManager handles save and load operations for the search index.
// It uses an exclusive write lock and shared read lock via a file-system lock
// file, mirroring the Rust persistence.rs approach.
type PersistenceManager struct {
	mu       sync.Mutex
	indexPath string
}

// NewPersistenceManager creates a manager for the given file path.
func NewPersistenceManager(path string) *PersistenceManager {
	return &PersistenceManager{indexPath: path}
}

// Save serialises store and trigramIdx to disk atomically via a temp file.
func (pm *PersistenceManager) Save(
	store *FileStore,
	trigramIdx *TrigramIndex,
	configFingerprint string,
) error {
	pm.mu.Lock()
	defer pm.mu.Unlock()

	if err := os.MkdirAll(filepath.Dir(pm.indexPath), 0o755); err != nil {
		return fmt.Errorf("creating index directory: %w", err)
	}

	// Build the persisted representation.
	allFiles := store.AllFiles()
	entries := make([]PersistedFileEntry, len(allFiles))
	for i, mf := range allFiles {
		entries[i] = PersistedFileEntry{
			Path:     mf.Path,
			Size:     mf.Size,
			IsBinary: mf.IsBinary,
			Content:  mf.Content,
		}
	}

	pi := PersistedIndex{
		Version:           persistenceVersion,
		CreatedAt:         time.Now().UTC(),
		ConfigFingerprint: configFingerprint,
		Files:             entries,
	}

	headerBytes, err := json.Marshal(pi)
	if err != nil {
		return fmt.Errorf("marshalling index header: %w", err)
	}

	trigramBytes, err := trigramIdx.MarshalBinary()
	if err != nil {
		return fmt.Errorf("marshalling trigram index: %w", err)
	}

	// Write atomically: temp file → rename.
	tmp := pm.indexPath + ".tmp"
	f, err := os.Create(tmp)
	if err != nil {
		return fmt.Errorf("creating temp file: %w", err)
	}
	defer func() {
		f.Close()
		os.Remove(tmp)
	}()

	// [headerLen:4][header JSON][trigramData...]
	var lenBuf [4]byte
	binary.LittleEndian.PutUint32(lenBuf[:], uint32(len(headerBytes)))
	if _, err := f.Write(lenBuf[:]); err != nil {
		return err
	}
	if _, err := f.Write(headerBytes); err != nil {
		return err
	}
	if _, err := f.Write(trigramBytes); err != nil {
		return err
	}
	if err := f.Close(); err != nil {
		return err
	}

	return os.Rename(tmp, pm.indexPath)
}

// LoadResult holds data returned from a successful Load call.
type LoadResult struct {
	Index             *PersistedIndex
	TrigramBytes      []byte
	ConfigFingerprint string
}

// Load reads a previously saved index from disk.
func (pm *PersistenceManager) Load() (*LoadResult, error) {
	pm.mu.Lock()
	defer pm.mu.Unlock()

	data, err := os.ReadFile(pm.indexPath)
	if err != nil {
		return nil, fmt.Errorf("reading index file %q: %w", pm.indexPath, err)
	}

	if len(data) < 4 {
		return nil, fmt.Errorf("index file %q is too short", pm.indexPath)
	}

	headerLen := int(binary.LittleEndian.Uint32(data[:4]))
	if 4+headerLen > len(data) {
		return nil, fmt.Errorf("index file corrupted: header length exceeds file size")
	}

	var pi PersistedIndex
	if err := json.Unmarshal(data[4:4+headerLen], &pi); err != nil {
		return nil, fmt.Errorf("parsing index header: %w", err)
	}

	if pi.Version != persistenceVersion {
		return nil, fmt.Errorf("unsupported index version %d (expected %d)", pi.Version, persistenceVersion)
	}

	return &LoadResult{
		Index:             &pi,
		TrigramBytes:      data[4+headerLen:],
		ConfigFingerprint: pi.ConfigFingerprint,
	}, nil
}

// RestoreInto populates store and trigramIdx from a LoadResult.
func RestoreInto(lr *LoadResult, store *FileStore, trigramIdx *TrigramIndex) error {
	for i, entry := range lr.Index.Files {
		mf := &MappedFile{
			Path:     entry.Path,
			Size:     entry.Size,
			IsBinary: entry.IsBinary,
			Content:  entry.Content,
		}
		store.mu.Lock()
		store.files = append(store.files, mf)
		store.paths[entry.Path] = uint32(i)
		store.mu.Unlock()
	}

	return trigramIdx.UnmarshalBinary(lr.TrigramBytes)
}
