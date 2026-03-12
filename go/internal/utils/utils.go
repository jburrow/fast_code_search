// Package utils provides helper utilities used throughout fast_code_search,
// including encoding detection, path normalisation, and system resource checks.
// It mirrors the Rust src/utils.rs module.
package utils

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"unicode/utf8"
)

// TranscodeResult describes how a file's bytes were interpreted.
type TranscodeResult struct {
	// Content is the decoded UTF-8 text.
	Content string
	// Encoding is a human-readable label for the detected encoding.
	Encoding string
	// Lossy is true when the transcoding was not lossless.
	Lossy bool
}

// ToUTF8 attempts to interpret raw bytes as UTF-8. If the bytes are not valid
// UTF-8 it falls back to Latin-1 (ISO-8859-1), which can represent any byte
// sequence. This mirrors the Rust transcode_to_utf8() function.
func ToUTF8(data []byte) TranscodeResult {
	// Fast path: already valid UTF-8.
	if utf8.Valid(data) {
		return TranscodeResult{
			Content:  string(data),
			Encoding: "utf-8",
			Lossy:    false,
		}
	}

	// Check for UTF-16 BOM.
	if len(data) >= 2 {
		if data[0] == 0xFF && data[1] == 0xFE {
			return decodeUTF16LE(data[2:])
		}
		if data[0] == 0xFE && data[1] == 0xFF {
			return decodeUTF16BE(data[2:])
		}
	}

	// Fall back to Latin-1 — every byte is a valid Unicode code point ≤ U+00FF.
	var sb strings.Builder
	sb.Grow(len(data))
	for _, b := range data {
		sb.WriteRune(rune(b))
	}
	return TranscodeResult{
		Content:  sb.String(),
		Encoding: "latin-1",
		Lossy:    true,
	}
}

func decodeUTF16LE(data []byte) TranscodeResult {
	if len(data)%2 != 0 {
		data = data[:len(data)-1]
	}
	runes := make([]rune, 0, len(data)/2)
	for i := 0; i+1 < len(data); i += 2 {
		r := rune(data[i]) | rune(data[i+1])<<8
		runes = append(runes, r)
	}
	return TranscodeResult{
		Content:  string(runes),
		Encoding: "utf-16le",
		Lossy:    false,
	}
}

func decodeUTF16BE(data []byte) TranscodeResult {
	if len(data)%2 != 0 {
		data = data[:len(data)-1]
	}
	runes := make([]rune, 0, len(data)/2)
	for i := 0; i+1 < len(data); i += 2 {
		r := rune(data[i])<<8 | rune(data[i+1])
		runes = append(runes, r)
	}
	return TranscodeResult{
		Content:  string(runes),
		Encoding: "utf-16be",
		Lossy:    false,
	}
}

// IsBinary returns true if the byte slice is likely binary (non-text) content.
// It checks the first 8 KiB for null bytes, which is a reliable heuristic.
func IsBinary(data []byte) bool {
	check := data
	if len(check) > 8192 {
		check = check[:8192]
	}
	for _, b := range check {
		if b == 0 {
			return true
		}
	}
	return false
}

// NormalizePath returns a clean, absolute, slash-separated path.
func NormalizePath(path string) string {
	abs, err := filepath.Abs(path)
	if err != nil {
		abs = path
	}
	return filepath.ToSlash(abs)
}

// SystemLimits reports OS-level resource availability.
type SystemLimits struct {
	// MaxOpenFiles is the system soft limit on open file descriptors.
	MaxOpenFiles uint64
	// AvailableMemoryBytes is a rough estimate of available physical memory.
	AvailableMemoryBytes uint64
}

// GetSystemLimits queries current OS resource limits.
func GetSystemLimits() SystemLimits {
	// Memory: use runtime stats as a cross-platform approximation.
	var ms runtime.MemStats
	runtime.ReadMemStats(&ms)
	return SystemLimits{
		MaxOpenFiles:         getOpenFileLimit(),
		AvailableMemoryBytes: ms.Sys,
	}
}

// IsTextFile returns true if the file extension suggests a source-code or
// configuration file (not binary data such as images or compiled objects).
func IsTextFile(path string) bool {
	ext := strings.ToLower(filepath.Ext(path))
	switch ext {
	case ".rs", ".go", ".py", ".js", ".ts", ".java", ".c", ".cpp", ".h", ".hpp",
		".cs", ".rb", ".php", ".sh", ".bash", ".zsh",
		".toml", ".yaml", ".yml", ".json", ".md", ".txt",
		".html", ".css", ".xml", ".sql", ".proto",
		".tsx", ".jsx", ".vue", ".svelte",
		"": // no extension — attempt
		return true
	}
	return false
}

// FileSize returns the size of a file in bytes, or -1 on error.
func FileSize(path string) int64 {
	info, err := os.Stat(path)
	if err != nil {
		return -1
	}
	return info.Size()
}
