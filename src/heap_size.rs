//! Estimated heap bytes of the index's structures, for the memory
//! breakdown (`SearchEngine::memory_breakdown`).
//!
//! The figures are estimates from lengths and capacities, not allocator
//! measurements: a hash table costs its capacity times key plus value plus
//! one control byte, a `Vec` its capacity times the element, a path or
//! string its capacity. They leave out allocator overhead and
//! fragmentation, so they sum to somewhat less than the process's resident
//! memory; the breakdown reports that gap as well.

use std::collections::{HashMap, HashSet};
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Give memory the allocator holds but no longer uses back to the system.
///
/// A build allocates and frees far more than the index it leaves behind
/// (file contents, per-batch trigram sets, merge buffers). glibc keeps
/// those freed pages in its arenas, so resident memory stays high until
/// the process exits. `malloc_trim` returns them: measured after a build,
/// tokio + Django went from 135 MB to 107 MB resident in 8 ms, and the
/// Linux kernel tree (85,000 files) from 793 MB to 709 MB in 36 ms. It
/// does not touch live allocations, so callers need no lock; run it after
/// the write lock is released. glibc only.
///
/// The binaries do not use the system allocator: each declares mimalloc
/// (its v2 line) as the global allocator. Queries allocate heavily (result
/// lists, decoded posting lists, file contents, per-thread buffers), and
/// with glibc a process that had built the 302,000-file real corpus
/// answered a common word in 161 ms against 77 ms in a fresh process with
/// the same index loaded, for the same instructions. A copy of the index
/// loaded into the building process was just as slow, so it was the
/// allocator's state, not the index's layout. With mimalloc the building
/// process answers in 72 to 87 ms (jemalloc measured the same, but does not
/// build for Windows; mimalloc v3 held 50% more memory on tokio + Django).
/// This call then also has mimalloc return the calling thread's unused
/// memory, and glibc trim what it still holds for the library's users who
/// keep the system allocator.
pub fn return_freed_memory() {
    // SAFETY: mi_collect only frees mimalloc's unused memory (a no-op
    // when it is not the global allocator); safe from any thread.
    unsafe {
        libmimalloc_sys::mi_collect(true);
    }
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        // SAFETY: malloc_trim only releases free memory; it is safe to call
        // from any thread at any time.
        unsafe {
            libc::malloc_trim(0);
        }
    }
}

/// Heap bytes of a hash map's table (keys and values inline, not what they
/// own).
pub fn map<K, V, S>(m: &HashMap<K, V, S>) -> u64 {
    (m.capacity() * (size_of::<K>() + size_of::<V>() + 1)) as u64
}

/// Heap bytes of a hash set's table.
pub fn set<K, S>(s: &HashSet<K, S>) -> u64 {
    (s.capacity() * (size_of::<K>() + 1)) as u64
}

/// Heap bytes of a vector's buffer (elements inline).
pub fn vec<T>(v: &Vec<T>) -> u64 {
    (v.capacity() * size_of::<T>()) as u64
}

/// Heap bytes of a string.
pub fn string(s: &String) -> u64 {
    s.capacity() as u64
}

/// Heap bytes of a path buffer.
pub fn path_buf(p: &PathBuf) -> u64 {
    p.capacity() as u64
}

/// Heap bytes of a shared path, counted once per allocation by whoever owns
/// it: the text plus the two reference counts.
pub fn arc_path(p: &Arc<Path>) -> u64 {
    (p.as_os_str().len() + 2 * size_of::<usize>()) as u64
}

/// Heap bytes of a Roaring bitmap: its containers' payload plus each
/// container's header (key and store, 40 bytes).
///
/// roaring 0.11's `statistics()` misstates two of the three payloads, so
/// they are corrected here: an array container's bytes are its capacity
/// times 4, but its values are `u16` (2 bytes); a bitset container's
/// "bytes" are its 65,536 bits, which take 8 KiB.
pub fn bitmap(b: &roaring::RoaringBitmap) -> u64 {
    const BITSET_BYTES: u64 = 65_536 / 8;
    let s = b.statistics();
    s.n_bytes_array_containers / 2
        + s.n_bytes_run_containers
        + u64::from(s.n_bitset_containers) * BITSET_BYTES
        + u64::from(s.n_containers) * 40
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmap_counts_real_container_bytes() {
        // One array container of 1,000 values: 2 bytes each.
        let small: roaring::RoaringBitmap = (0..2_000).step_by(2).collect();
        let bytes = bitmap(&small);
        assert!((2_000..=2_000 * 2 + 40 + 64).contains(&bytes), "{bytes}");
        // One bitset container (more than 4,096 values): 8 KiB.
        let dense: roaring::RoaringBitmap = (0..60_000).step_by(2).collect();
        assert_eq!(bitmap(&dense), 8_192 + 40);
    }
}
