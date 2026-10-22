//! Taking the engine lock without stalling the server.
//!
//! The engine sits behind a `std::sync::RwLock`. Searches used to give up
//! the moment a writer held it and answer "index is being updated, try
//! again"; with the watcher applying a stream of changes to a large tree
//! that was most of the time. Now the watcher holds the write lock only in
//! short steps, and a search waits a short while for it ([`read_within`])
//! instead of failing, so it is answered rather than turned away.
//!
//! Writers also never *queue* on the lock ([`write()`]). A queued writer makes
//! the lock turn every new reader away until the writer has been granted
//! the lock, and it is granted only once every running search has finished.
//! One slow search (a regex over a million files) and a one-file watcher
//! step therefore used to stop every other search for as long as the slow
//! one ran, and past [`SEARCH_READ_WAIT`] they were turned away. Now a
//! writer takes the lock in a moment when no search holds it, trying every
//! couple of milliseconds. New searches make such a moment by holding back
//! for up to [`READER_DEFER`] while a writer waits, and then go ahead
//! whether or not it got in: a search waits for a writer's short hold,
//! never for other searches. While a writer waits, searches see the index as it was; the
//! watcher reports the changes it has not applied yet (see
//! `IndexFreshness::pending_changes`).

use crate::search::SearchEngine;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockError};
use std::time::{Duration, Instant};
use tracing::error;

/// How long a search waits for a writer (a watcher batch, a checkpoint)
/// before it is answered "index is being updated, try again".
pub const SEARCH_READ_WAIT: Duration = Duration::from_secs(5);

/// How long a new search holds back while a writer waits, so that the
/// running searches finish and the writer gets in. A search that runs
/// longer than this does not hold up the searches that arrive meanwhile.
///
/// Measured with `examples/watch_load.rs --mixed` (four threads searching
/// back to back): with no holding back the watcher did not get in once in
/// 30 s; with 250 ms it kept up with 100,000 rewritten files on a
/// million-file tree while the slowest search took 1.4 s.
pub const READER_DEFER: Duration = Duration::from_millis(250);

/// The longest sleep between attempts; the first ones are much shorter.
const MAX_BACKOFF: Duration = Duration::from_millis(10);

/// The longest a waiting writer sleeps between attempts.
const WRITER_MAX_BACKOFF: Duration = Duration::from_millis(2);

/// Writers waiting in [`write()`]. One count for the process: in the server
/// there is one engine, and in tests that run several, a writer on one only
/// makes a search on another hold back for [`READER_DEFER`].
static WRITERS_WAITING: AtomicUsize = AtomicUsize::new(0);

/// A read lock, waiting up to `wait` for a writer to finish. `None` when the
/// lock is still held after that. A poisoned lock is recovered: every
/// writer is wrapped in `catch_unwind`, so poison only means some thread
/// panicked, not that the index is unusable.
///
/// While a writer is waiting for running searches to finish, this first
/// holds back for up to [`READER_DEFER`] so the writer gets in.
pub fn read_within(
    engine: &RwLock<SearchEngine>,
    wait: Duration,
) -> Option<RwLockReadGuard<'_, SearchEngine>> {
    let started = Instant::now();
    let deadline = started + wait;
    let defer_until = started + READER_DEFER.min(wait);
    let mut backoff = Duration::from_micros(200);
    loop {
        let deferring = WRITERS_WAITING.load(Ordering::Acquire) > 0 && Instant::now() < defer_until;
        if !deferring {
            match engine.try_read() {
                Ok(guard) => return Some(guard),
                Err(TryLockError::Poisoned(poisoned)) => {
                    error!("Search engine lock was poisoned; recovering for read");
                    return Some(poisoned.into_inner());
                }
                Err(TryLockError::WouldBlock) => {}
            }
        }
        let now = Instant::now();
        if now >= deadline {
            return None;
        }
        std::thread::sleep(backoff.min(deadline - now));
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

/// The write lock, recovering a poisoned lock.
///
/// Taken in a moment when no search holds the lock, rather than by queueing
/// for it (see the module notes): searches keep being answered while this
/// waits. Writers keep each hold short and pause between holds (see
/// [`crate::search::watch_loop::apply_burst`]).
pub fn write(engine: &RwLock<SearchEngine>) -> RwLockWriteGuard<'_, SearchEngine> {
    struct Waiting;
    impl Drop for Waiting {
        fn drop(&mut self) {
            WRITERS_WAITING.fetch_sub(1, Ordering::AcqRel);
        }
    }
    let mut waiting = None;
    let mut backoff = Duration::from_micros(50);
    loop {
        match engine.try_write() {
            Ok(guard) => return guard,
            Err(TryLockError::Poisoned(poisoned)) => {
                error!("Search engine lock was poisoned; recovering for write");
                return poisoned.into_inner();
            }
            Err(TryLockError::WouldBlock) => {
                if waiting.is_none() {
                    WRITERS_WAITING.fetch_add(1, Ordering::AcqRel);
                    waiting = Some(Waiting);
                }
                std::thread::sleep(backoff);
                backoff = (backoff * 2).min(WRITER_MAX_BACKOFF);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// A writer waiting for a slow search used to stop every new search
    /// until the slow one ended: the lock turns readers away while a writer
    /// queues. Now new searches go ahead after `READER_DEFER`.
    #[test]
    fn a_waiting_writer_does_not_stop_new_searches() {
        let engine = Arc::new(RwLock::new(SearchEngine::new()));
        let slow = engine.read().unwrap();
        let writer = {
            let engine = Arc::clone(&engine);
            std::thread::spawn(move || {
                let mut guard = write(&engine);
                guard.watching = true;
            })
        };
        while WRITERS_WAITING.load(Ordering::Acquire) == 0 {
            std::thread::sleep(Duration::from_millis(1));
        }

        // The search held back to let the writer in, then went ahead when
        // the long search kept the writer out.
        let started = Instant::now();
        let search = read_within(&engine, SEARCH_READ_WAIT).expect("search answered");
        let waited = started.elapsed();
        assert!(
            waited >= READER_DEFER && waited < READER_DEFER + Duration::from_millis(500),
            "a search waited {waited:?} behind a writer that was waiting for another search"
        );
        assert!(
            !search.watching,
            "the writer cannot have run while searches hold the lock"
        );
        drop(search);
        drop(slow);

        writer.join().unwrap();
        assert!(
            engine.read().unwrap().watching,
            "the writer got in once searches finished"
        );
    }

    /// A search that arrives while the writer holds the lock waits for it
    /// and is then answered.
    #[test]
    fn a_search_waits_out_a_short_write() {
        let engine = Arc::new(RwLock::new(SearchEngine::new()));
        let guard = write(&engine);
        let reader = {
            let engine = Arc::clone(&engine);
            std::thread::spawn(move || read_within(&engine, SEARCH_READ_WAIT).is_some())
        };
        std::thread::sleep(Duration::from_millis(50));
        drop(guard);
        assert!(reader.join().unwrap());
        assert!(read_within(&engine, Duration::ZERO).is_some());
    }
}
