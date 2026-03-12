//go:build windows

package utils

func getOpenFileLimit() uint64 {
	return 16384 // Windows default
}
