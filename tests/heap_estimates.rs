//! The memory breakdown's estimates against the real heap.
//!
//! `SearchEngine::memory_breakdown` estimates each structure from lengths
//! and capacities. This test counts every allocation (its own global
//! allocator, so it lives in its own test binary with a single test) and
//! checks that the estimates for a loaded index stay close to the heap the
//! index really holds. A new structure the breakdown does not count, or an
//! estimate that drifts (roaring's own statistics were off by 2x and 8x),
//! fails here.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

struct Counting;
static LIVE: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE.fetch_add(layout.size(), Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        LIVE.fetch_add(new_size, Relaxed);
        LIVE.fetch_sub(layout.size(), Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

#[test]
fn estimates_cover_the_heap_of_a_loaded_index() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("src");
    std::fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let mut files = Vec::new();
    for i in 0..2_000 {
        let f = root.join(format!("module_{i}.py"));
        std::fs::write(
            &f,
            format!(
                "import os\nfrom module_{next} import helper_{next}\n\n\
                 class Handler{i}:\n    def handle_{i}(self, request):\n        \
                 return helper_{next}(os.path.join(request.path, \"part_{i}\"))\n\n\
                 def helper_{i}(value):\n    return value * {i}\n",
                next = (i + 7) % 2_000
            ),
        )
        .unwrap();
        // An hour old, so the load does not treat it as possibly changed
        // in the second it was saved (and re-read it).
        let hour_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3_600);
        std::fs::File::options()
            .write(true)
            .open(&f)
            .unwrap()
            .set_modified(hour_ago)
            .unwrap();
        files.push(f);
    }
    let saved = dir.path().join("index.bin");
    {
        let mut engine = fast_code_search::search::SearchEngine::new();
        engine.add_root_path(&root);
        engine.update_files(&files);
        engine.save_index(&saved, &Default::default()).unwrap();
    }

    let before = LIVE.load(Relaxed);
    let mut engine = fast_code_search::search::SearchEngine::new();
    let stale = engine.load_index(&saved).unwrap();
    assert!(stale.is_empty(), "{} files were not restored", stale.len());
    let live = LIVE.load(Relaxed).saturating_sub(before) as f64;
    let estimate = engine.memory_breakdown();
    let share = estimate.total_bytes as f64 / live;
    assert!(
        (0.75..=1.05).contains(&share),
        "estimates are {:.0}% of the {:.1} MB the loaded index holds: {estimate:?}",
        share * 100.0,
        live / 1e6
    );
}
