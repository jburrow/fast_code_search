package index

import (
	"fmt"
	"os"
	"sync"

	"github.com/jburrow/fast_code_search/internal/utils"
)

// MappedFile holds file content either as an OS memory mapping or a heap copy.
type MappedFile struct {
	// Path is the canonical slash-separated absolute path.
	Path    string
	// Content is the decoded UTF-8 text.
	Content string
	// Size is the byte size of the original file.
	Size int64
	// IsBinary records whether the file was detected as non-text.
	IsBinary bool
}

// FileStore manages the corpus of indexed files. Each file is assigned a
// monotonically increasing uint32 document ID (docID). This mirrors the Rust
// FileStore / LazyFileStore types.
type FileStore struct {
	mu    sync.RWMutex
	files []*MappedFile       // indexed by docID
	paths map[string]uint32   // canonical path → docID (deduplication)
}

// NewFileStore allocates an empty FileStore.
func NewFileStore() *FileStore {
	return &FileStore{
		paths: make(map[string]uint32),
	}
}

// AddFile reads path from disk, decodes its content to UTF-8, and assigns it
// a docID. If the file has already been indexed its existing docID is returned.
// maxBytes is the maximum number of bytes read (0 = unlimited).
func (fs *FileStore) AddFile(path string, maxBytes int64) (uint32, error) {
	canonical := utils.NormalizePath(path)

	fs.mu.Lock()
	defer fs.mu.Unlock()

	if id, ok := fs.paths[canonical]; ok {
		return id, nil
	}

	data, err := readFileCapped(path, maxBytes)
	if err != nil {
		return 0, fmt.Errorf("reading %q: %w", path, err)
	}

	mf := &MappedFile{Path: canonical, Size: int64(len(data))}
	if utils.IsBinary(data) {
		mf.IsBinary = true
		mf.Content = ""
	} else {
		result := utils.ToUTF8(data)
		mf.Content = result.Content
	}

	docID := uint32(len(fs.files))
	fs.files = append(fs.files, mf)
	fs.paths[canonical] = docID
	return docID, nil
}

// UpdateFile re-reads a file and updates its content in place.
// Returns (docID, true) when the file was changed, (docID, false) otherwise.
func (fs *FileStore) UpdateFile(path string, maxBytes int64) (uint32, bool, error) {
	canonical := utils.NormalizePath(path)

	data, err := readFileCapped(path, maxBytes)
	if err != nil {
		return 0, false, fmt.Errorf("reading %q: %w", path, err)
	}

	fs.mu.Lock()
	defer fs.mu.Unlock()

	id, exists := fs.paths[canonical]
	var mf *MappedFile
	if exists {
		mf = fs.files[id]
	} else {
		mf = &MappedFile{Path: canonical}
		id = uint32(len(fs.files))
		fs.files = append(fs.files, mf)
		fs.paths[canonical] = id
	}

	newContent := ""
	isBinary := utils.IsBinary(data)
	if !isBinary {
		newContent = utils.ToUTF8(data).Content
	}

	changed := mf.Content != newContent || mf.IsBinary != isBinary
	mf.Content = newContent
	mf.IsBinary = isBinary
	mf.Size = int64(len(data))
	return id, changed, nil
}

// Get returns the MappedFile for docID, or nil if out of range.
func (fs *FileStore) Get(docID uint32) *MappedFile {
	fs.mu.RLock()
	defer fs.mu.RUnlock()
	if int(docID) >= len(fs.files) {
		return nil
	}
	return fs.files[docID]
}

// Count returns the number of indexed files.
func (fs *FileStore) Count() int {
	fs.mu.RLock()
	defer fs.mu.RUnlock()
	return len(fs.files)
}

// DocIDForPath resolves a canonical path to its docID. Returns (id, true) when found.
func (fs *FileStore) DocIDForPath(path string) (uint32, bool) {
	canonical := utils.NormalizePath(path)
	fs.mu.RLock()
	defer fs.mu.RUnlock()
	id, ok := fs.paths[canonical]
	return id, ok
}

// AllFiles returns a snapshot of all MappedFiles.
func (fs *FileStore) AllFiles() []*MappedFile {
	fs.mu.RLock()
	defer fs.mu.RUnlock()
	out := make([]*MappedFile, len(fs.files))
	copy(out, fs.files)
	return out
}

// readFileCapped reads at most maxBytes from path. If maxBytes ≤ 0 the whole
// file is read.
func readFileCapped(path string, maxBytes int64) ([]byte, error) {
	if maxBytes <= 0 {
		return os.ReadFile(path)
	}
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()

	buf := make([]byte, maxBytes)
	n, err := f.Read(buf)
	if err != nil && n == 0 {
		return nil, err
	}
	return buf[:n], nil
}
