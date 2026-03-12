//go:build !windows
// +build !windows

package utils

import "syscall"

func getOpenFileLimit() uint64 {
	var rl syscall.Rlimit
	if err := syscall.Getrlimit(syscall.RLIMIT_NOFILE, &rl); err != nil {
		return 1024
	}
	return rl.Cur
}
