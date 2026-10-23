//! Diagnostic captures, end to end: a slow search, a search kept waiting
//! for the index lock, and a lock that is never free each write a capture
//! that `fcs diagnose` can read. One test: captures are switched on once
//! per process.

use fast_code_search::config::DiagnosticsConfig;
use fast_code_search::diagnostics::capture::{self, kind, SearchProbe, Watched};
use fast_code_search::search::{engine_lock, SearchEngine};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

#[test]
fn slow_searches_lock_waits_and_hangs_are_captured() {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = capture::Settings::resolve(&DiagnosticsConfig::default(), None);
    settings.dir = dir.path().to_path_buf();
    settings.slow_search = Duration::from_millis(200);
    settings.lock_wait = Duration::from_millis(100);
    settings.stall = Duration::from_secs(2);
    settings.min_interval = Duration::ZERO;
    capture::init(settings).unwrap();

    let engine = Arc::new(RwLock::new(SearchEngine::new()));
    let stop = Arc::new(AtomicBool::new(false));
    capture::start_watchdog(Watched {
        engine: engine.clone(),
        progress: None,
        shutdown: stop.clone(),
    })
    .expect("captures are on");

    // A search that takes 300 ms.
    {
        let mut probe = SearchProbe::start("test", "text", "secret needle");
        probe.locked();
        std::thread::sleep(Duration::from_millis(300));
    }

    // A search that waits 400 ms for a writer.
    let writer = engine.write().unwrap();
    let reader = {
        let engine = engine.clone();
        std::thread::spawn(move || {
            engine_lock::read_within(&engine, Duration::from_secs(5)).is_some()
        })
    };
    std::thread::sleep(Duration::from_millis(400));
    drop(writer);
    assert!(reader.join().unwrap());

    // A writer that never lets go (for longer than `stall`).
    let writer = engine.write().unwrap();
    std::thread::sleep(Duration::from_secs(4));
    drop(writer);

    let wanted = [kind::SLOW_SEARCH, kind::LOCK_WAIT, kind::LOCK_HELD];
    let deadline = Instant::now() + Duration::from_secs(120);
    let captures = loop {
        let (captures, bad) = capture::load_captures(dir.path());
        assert!(bad.is_empty(), "{bad:?}");
        let have = |k: &str| captures.iter().any(|(_, c)| c.trigger.kind == k);
        if wanted.iter().all(|k| have(k)) {
            break captures;
        }
        assert!(
            Instant::now() < deadline,
            "missing captures; have {:?}",
            captures
                .iter()
                .map(|(_, c)| c.trigger.kind.clone())
                .collect::<Vec<_>>()
        );
        std::thread::sleep(Duration::from_millis(200));
    };
    stop.store(true, Ordering::Release);

    let get = |k: &str| {
        &captures
            .iter()
            .find(|(_, c)| c.trigger.kind == k)
            .unwrap()
            .1
    };
    let slow = get(kind::SLOW_SEARCH);
    let search = slow.search.as_ref().expect("the slow search");
    assert!(search.elapsed_ms >= 300, "{search:?}");
    assert_eq!(search.query, "<redacted>");
    assert_eq!(search.query_chars, "secret needle".len());

    let waited = get(kind::LOCK_WAIT);
    assert!(waited.trigger.observed_ms >= 300, "{:?}", waited.trigger);

    let held = get(kind::LOCK_HELD);
    let threads = held.threads.as_ref().expect("a hang lists the threads");
    if cfg!(target_os = "linux") {
        assert!(!threads.threads.is_empty());
        assert!(threads.stacks.is_some() || threads.note.is_some());
    }

    for path in capture::capture_files(dir.path()) {
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            !text.contains("needle"),
            "{} holds the query",
            path.display()
        );
    }
    let summary = capture::summarize(dir.path(), &captures, &[]);
    eprintln!("{summary}");
    assert!(summary.contains("Slowest search"), "{summary}");
    assert!(summary.contains(kind::LOCK_HELD), "{summary}");
}
