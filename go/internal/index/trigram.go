// Package index provides the core data structures for the trigram inverted
// index, file storage, and on-disk persistence. It is the Go equivalent of
// the Rust src/index/ module family.
package index

import (
	"encoding/binary"
	"fmt"
	"sort"

	"github.com/RoaringBitmap/roaring"
)

// Trigram represents three consecutive bytes of content used as an index key.
type Trigram [3]byte

// TrigramIndex maps every trigram found in the corpus to the set of document
// IDs (file indices) that contain it. Backed by Roaring bitmaps for compact
// storage and fast bitwise intersection, mirroring the Rust TrigramIndex.
type TrigramIndex struct {
	index map[uint32]*roaring.Bitmap // trigram key → set of doc IDs
}

// NewTrigramIndex allocates an empty TrigramIndex.
func NewTrigramIndex() *TrigramIndex {
	return &TrigramIndex{index: make(map[uint32]*roaring.Bitmap)}
}

// trigramKey packs three bytes into a uint32 for use as map key.
func trigramKey(t Trigram) uint32 {
	return uint32(t[0])<<16 | uint32(t[1])<<8 | uint32(t[2])
}

// trigramFromKey reconstructs a Trigram from a packed uint32.
func trigramFromKey(k uint32) Trigram {
	return Trigram{byte(k >> 16), byte(k >> 8), byte(k)}
}

// Add registers docID under all trigrams extracted from content.
func (ti *TrigramIndex) Add(docID uint32, content []byte) {
	seen := make(map[uint32]struct{}, len(content))
	for i := 0; i+2 < len(content); i++ {
		t := Trigram{content[i], content[i+1], content[i+2]}
		k := trigramKey(t)
		if _, ok := seen[k]; ok {
			continue
		}
		seen[k] = struct{}{}
		bm, ok := ti.index[k]
		if !ok {
			bm = roaring.New()
			ti.index[k] = bm
		}
		bm.Add(docID)
	}
}

// Remove removes docID from every bitmap that contains it.
// This is O(|index|) but removals are rare.
func (ti *TrigramIndex) Remove(docID uint32) {
	for _, bm := range ti.index {
		bm.Remove(docID)
	}
}

// Candidates returns the set of document IDs that contain ALL trigrams in
// query. If query is shorter than 3 bytes, all doc IDs are returned (no
// filtering). An empty result bitmap means no candidate matches.
func (ti *TrigramIndex) Candidates(query []byte) *roaring.Bitmap {
	trigrams := extractUniqueTrigrams(query)
	if len(trigrams) == 0 {
		// Cannot narrow down — return nil to signal "search all".
		return nil
	}

	// Sort so we start with the rarest (smallest) bitmap.
	sort.Slice(trigrams, func(i, j int) bool {
		ki, kj := trigramKey(trigrams[i]), trigramKey(trigrams[j])
		bi := ti.index[ki]
		bj := ti.index[kj]
		si, sj := uint64(0), uint64(0)
		if bi != nil {
			si = bi.GetCardinality()
		}
		if bj != nil {
			sj = bj.GetCardinality()
		}
		return si < sj
	})

	var result *roaring.Bitmap
	for _, t := range trigrams {
		k := trigramKey(t)
		bm, ok := ti.index[k]
		if !ok {
			// No doc contains this trigram → intersection is empty.
			return roaring.New()
		}
		if result == nil {
			result = bm.Clone()
		} else {
			result.And(bm)
		}
		if result.IsEmpty() {
			return result
		}
	}
	if result == nil {
		return roaring.New()
	}
	return result
}

// Stats returns some diagnostic counters.
func (ti *TrigramIndex) Stats() (numTrigrams int, totalPostings uint64) {
	for _, bm := range ti.index {
		numTrigrams++
		totalPostings += bm.GetCardinality()
	}
	return
}

// MarshalBinary serialises the index to a compact byte slice.
// Format: [numEntries:4][key:4][bitmapLen:4][bitmapBytes:N]...
func (ti *TrigramIndex) MarshalBinary() ([]byte, error) {
	// Estimate buffer size.
	buf := make([]byte, 0, len(ti.index)*20)
	tmp := make([]byte, 8)

	binary.LittleEndian.PutUint32(tmp, uint32(len(ti.index)))
	buf = append(buf, tmp[:4]...)

	for k, bm := range ti.index {
		binary.LittleEndian.PutUint32(tmp, k)
		buf = append(buf, tmp[:4]...)

		bmBytes, err := bm.ToBytes()
		if err != nil {
			return nil, fmt.Errorf("serialising bitmap for trigram %06X: %w", k, err)
		}
		binary.LittleEndian.PutUint32(tmp, uint32(len(bmBytes)))
		buf = append(buf, tmp[:4]...)
		buf = append(buf, bmBytes...)
	}
	return buf, nil
}

// UnmarshalBinary restores an index from bytes written by MarshalBinary.
func (ti *TrigramIndex) UnmarshalBinary(data []byte) error {
	if len(data) < 4 {
		return fmt.Errorf("trigram index: too short (%d bytes)", len(data))
	}
	n := int(binary.LittleEndian.Uint32(data))
	off := 4
	ti.index = make(map[uint32]*roaring.Bitmap, n)

	for i := 0; i < n; i++ {
		if off+8 > len(data) {
			return fmt.Errorf("trigram index: truncated at entry %d", i)
		}
		k := binary.LittleEndian.Uint32(data[off:])
		bLen := int(binary.LittleEndian.Uint32(data[off+4:]))
		off += 8
		if off+bLen > len(data) {
			return fmt.Errorf("trigram index: bitmap %d truncated", i)
		}
		bm := roaring.New()
		if _, err := bm.FromBuffer(data[off : off+bLen]); err != nil {
			return fmt.Errorf("trigram index: bitmap %d: %w", i, err)
		}
		ti.index[k] = bm
		off += bLen
	}
	return nil
}

// extractUniqueTrigrams returns the deduplicated set of trigrams in data.
func extractUniqueTrigrams(data []byte) []Trigram {
	seen := make(map[uint32]struct{}, len(data))
	out := make([]Trigram, 0, len(data))
	for i := 0; i+2 < len(data); i++ {
		t := Trigram{data[i], data[i+1], data[i+2]}
		k := trigramKey(t)
		if _, ok := seen[k]; !ok {
			seen[k] = struct{}{}
			out = append(out, t)
		}
	}
	return out
}

// TrigramFromKey is exported for persistence layer use.
func TrigramFromKey(k uint32) Trigram { return trigramFromKey(k) }
