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
//! while a writer waits, about as long as a search runs ([`DEFER`]), and
//! then go ahead whether or not it got in: a search waits for a writer's
//! short hold and a typical search's length, never for one slow search.
//! While a writer waits, searches see the index as it was; the
//! watcher reports the changes it has not applied yet (see
//! `IndexFreshness::pending_changes`).
//!
//! Time is read and slept through [`crate::sim`], so the protocol can be run
//! on simulated time (`tests/lock_sim.rs`): searches of any length, for
//! minutes, the same schedule on every run and OS.

use crate::sim;
use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockError};
use std::time::{Duration, Instant};
use tracing::error;

/// How long a search waits for a writer (a watcher batch, a checkpoint)
/// before it is answered "index is being updated, try again".
pub const SEARCH_READ_WAIT: Duration = Duration::from_secs(5);

/// The shortest a new search holds back while a writer waits, so that the
/// running searches finish and the writer gets in (see [`DEFER`]).
///
/// Measured with `examples/watch_load.rs --mixed` (four threads searching
/// back to back): with no holding back the watcher did not get in once in
/// 30 s.
pub const READER_DEFER: Duration = Duration::from_millis(250);

/// The longest a new search holds back for a waiting writer.
pub const MAX_READER_DEFER: Duration = Duration::from_secs(2);

/// The holding-back rule searches use: a little longer than recent
/// searches took, between [`READER_DEFER`] and [`MAX_READER_DEFER`].
///
/// Measured with `tests/lock_sim.rs` (the tuning table on the book's
/// "Incremental updates" page): a fixed hold-back only works for searches
/// shorter than it. With four people running half-second searches back to
/// back, a 100,000-file change that takes 5 s to install with no searches
/// took 62 s with a fixed 250 ms, and 10 s once searches held back as long
/// as a search runs.
pub const DEFER: Defer = Defer::Adaptive {
    min: READER_DEFER,
    max: MAX_READER_DEFER,
};

/// How many recent searches [`Defer::Adaptive`] looks at.
const RECENT: usize = 32;

/// The longest sleep between attempts; the first ones are much shorter.
pub const MAX_BACKOFF: Duration = Duration::from_millis(10);

/// The longest a waiting writer sleeps between attempts.
pub const WRITER_MAX_BACKOFF: Duration = Duration::from_millis(2);

/// Writers waiting in [`write()`], and since when. One for the process: in
/// the server there is one engine, and in tests that run several, a writer
/// on one only makes a search on another hold back for a while.
static GATE: WriterGate = WriterGate::new();

/// Writers waiting for the lock, and since when the first of them has.
/// Also how long recent searches held the lock, for [`Defer::Adaptive`].
pub(crate) struct WriterGate {
    waiting: AtomicUsize,
    since: Mutex<Option<Instant>>,
    recent: Mutex<Recent>,
}

/// The last [`RECENT`] read holds, oldest overwritten first.
struct Recent {
    holds: [Duration; RECENT],
    len: usize,
    next: usize,
}

impl WriterGate {
    pub(crate) const fn new() -> Self {
        Self {
            waiting: AtomicUsize::new(0),
            since: Mutex::new(None),
            recent: Mutex::new(Recent {
                holds: [Duration::ZERO; RECENT],
                len: 0,
                next: 0,
            }),
        }
    }

    fn record_hold(&self, held: Duration) {
        let mut r = self.recent.lock().unwrap_or_else(|p| p.into_inner());
        let i = r.next;
        r.holds[i] = held;
        r.next = (i + 1) % RECENT;
        r.len = (r.len + 1).min(RECENT);
    }

    /// The 90th percentile of the recent read holds (`None`: none yet).
    fn typical_hold(&self) -> Option<Duration> {
        let r = self.recent.lock().unwrap_or_else(|p| p.into_inner());
        if r.len == 0 {
            return None;
        }
        let mut holds = r.holds[..r.len].to_vec();
        holds.sort_unstable();
        Some(holds[(r.len * 9 / 10).min(r.len - 1)])
    }

    fn enter(&self) {
        let mut since = self.since.lock().unwrap_or_else(|p| p.into_inner());
        if self.waiting.fetch_add(1, Ordering::AcqRel) == 0 {
            *since = Some(sim::now());
        }
    }

    fn leave(&self) {
        let mut since = self.since.lock().unwrap_or_else(|p| p.into_inner());
        if self.waiting.fetch_sub(1, Ordering::AcqRel) == 1 {
            *since = None;
        }
    }

    /// When the writers now waiting started to (`None`: none waits).
    fn waiting_since(&self) -> Option<Instant> {
        if self.waiting.load(Ordering::Acquire) == 0 {
            return None;
        }
        *self.since.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// How a new search holds back while a writer waits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Defer {
    /// Every search that arrives while a writer waits holds back for up to
    /// this long from its own arrival (the policy until 0.18).
    PerSearch(Duration),
    /// Searches hold back only until this long after the writer started
    /// waiting: long enough for the running searches to finish, after
    /// which holding back cannot help (a slow search is keeping the writer
    /// out) and searches go ahead at once.
    Window(Duration),
    /// Like `PerSearch`, for one and a half times the 90th percentile of
    /// how long recent searches held the lock, between `min` and `max`.
    /// The writer is waiting for the searches already running; a search
    /// that holds back for less than they take gets in before they end, and
    /// the writer then waits for it too. With four people searching back to
    /// back that can go on for seconds (`tests/lock_sim.rs`: 1.0 × the 90th
    /// percentile drained a backlog in up to 22 s, 1.5 × in up to 11 s,
    /// where installing alone takes 5 s). Before any search has finished it
    /// is `min`.
    Adaptive { min: Duration, max: Duration },
}

impl Defer {
    /// How long a search arriving now holds back from its arrival, for
    /// the per-search rules.
    fn per_search(self, gate: &WriterGate) -> Option<Duration> {
        match self {
            Defer::PerSearch(d) => Some(d),
            Defer::Window(_) => None,
            Defer::Adaptive { min, max } => Some(
                gate.typical_hold()
                    .map_or(min, |d| d * 3 / 2)
                    .clamp(min, max),
            ),
        }
    }
}

/// A read lock on the engine. Derefs to the engine; when dropped, it
/// records how long it was held, for [`Defer::Adaptive`].
pub struct ReadGuard<'a, T> {
    guard: RwLockReadGuard<'a, T>,
    since: Instant,
}

impl<T> Deref for ReadGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.guard
    }
}

impl<T> Drop for ReadGuard<'_, T> {
    fn drop(&mut self) {
        let held = sim::now() - self.since;
        sim::with_gate(&GATE, |g| g.record_hold(held));
    }
}

/// A read lock, waiting up to `wait` for a writer to finish. `None` when the
/// lock is still held after that. A poisoned lock is recovered: every
/// writer is wrapped in `catch_unwind`, so poison only means some thread
/// panicked, not that the index is unusable.
///
/// While a writer is waiting for running searches to finish, this first
/// holds back (see [`DEFER`]) so the writer gets in.
pub fn read_within<T>(engine: &RwLock<T>, wait: Duration) -> Option<ReadGuard<'_, T>> {
    read_deferring(engine, wait, DEFER)
}

/// [`read_within`] with another holding-back rule than [`DEFER`]: for the
/// lock simulation (`tests/lock_sim.rs`) to compare them.
pub fn read_deferring<T>(
    engine: &RwLock<T>,
    wait: Duration,
    defer: Defer,
) -> Option<ReadGuard<'_, T>> {
    let started = sim::now();
    let deadline = started + wait;
    // Fixed at arrival, so a search's hold-back does not grow while it waits.
    let hold_back = sim::with_gate(&GATE, |g| defer.per_search(g));
    let mut backoff = Duration::from_micros(200);
    loop {
        let now = sim::now();
        let deferring = sim::with_gate(&GATE, |g| g.waiting_since()).is_some_and(|since| {
            match (defer, hold_back) {
                (Defer::Window(d), _) => now < since + d && now < deadline,
                (_, Some(d)) => now < started + d.min(wait),
                (_, None) => false,
            }
        });
        if !deferring {
            let guard = match engine.try_read() {
                Ok(guard) => Some(guard),
                Err(TryLockError::Poisoned(poisoned)) => {
                    error!("Search engine lock was poisoned; recovering for read");
                    Some(poisoned.into_inner())
                }
                Err(TryLockError::WouldBlock) => None,
            };
            if let Some(guard) = guard {
                let now = sim::now();
                crate::diagnostics::capture::lock_waited(now - started, true);
                return Some(ReadGuard { guard, since: now });
            }
        }
        let now = sim::now();
        if now >= deadline {
            crate::diagnostics::capture::lock_waited(now - started, false);
            return None;
        }
        sim::sleep(backoff.min(deadline - now));
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

/// The write lock, recovering a poisoned lock.
///
/// Taken in a moment when no search holds the lock, rather than by queueing
/// for it (see the module notes): searches keep being answered while this
/// waits. Writers keep each hold short and pause between holds (see
/// [`crate::search::watch_loop::apply_burst`]).
pub fn write<T>(engine: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    struct Waiting;
    impl Drop for Waiting {
        fn drop(&mut self) {
            sim::with_gate(&GATE, WriterGate::leave);
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
                    sim::with_gate(&GATE, WriterGate::enter);
                    waiting = Some(Waiting);
                }
                sim::sleep(backoff);
                backoff = (backoff * 2).min(WRITER_MAX_BACKOFF);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{self, Sim};
    use std::sync::{Arc, Mutex};

    const MS: Duration = Duration::from_millis(1);

    /// A writer waiting for a slow search used to stop every new search
    /// until the slow one ended: the lock turns readers away while a writer
    /// queues. Now new searches go ahead after `READER_DEFER`, and the
    /// writer gets in when the slow search ends.
    #[test]
    fn a_waiting_writer_does_not_stop_new_searches() {
        let s = Sim::new();
        let lock = Arc::new(RwLock::new(0u32));
        let log = Arc::new(Mutex::new(Vec::new()));
        {
            // A 10 s search from t = 0.
            let lock = Arc::clone(&lock);
            s.spawn(move || {
                let _g = read_within(&*lock, SEARCH_READ_WAIT).unwrap();
                sim::sleep(Duration::from_secs(10));
            });
        }
        {
            // The watcher, from t = 1 s.
            let (lock, log, s2) = (Arc::clone(&lock), Arc::clone(&log), Arc::clone(&s));
            s.spawn(move || {
                sim::sleep(Duration::from_secs(1));
                let mut g = write(&*lock);
                *g += 1;
                log.lock().unwrap().push(("writer in", s2.elapsed()));
                sim::sleep(100 * MS);
            });
        }
        {
            // A search at t = 2 s: it holds back for READER_DEFER, then goes
            // ahead while the slow search keeps the writer out.
            let (lock, log, s2) = (Arc::clone(&lock), Arc::clone(&log), Arc::clone(&s));
            s.spawn(move || {
                sim::sleep(Duration::from_secs(2));
                let g = read_within(&*lock, SEARCH_READ_WAIT).expect("search answered");
                assert_eq!(
                    *g, 0,
                    "the writer cannot be in while a search holds the lock"
                );
                log.lock().unwrap().push(("search in", s2.elapsed()));
            });
        }
        s.run();
        let log = log.lock().unwrap().clone();
        assert_eq!(log[0].0, "search in");
        let waited = log[0].1 - Duration::from_secs(2);
        assert!(
            waited >= READER_DEFER && waited <= READER_DEFER + MAX_BACKOFF,
            "a search waited {waited:?} behind a writer that was waiting for another search"
        );
        assert_eq!(log[1].0, "writer in");
        assert!(
            log[1].1 >= Duration::from_secs(10)
                && log[1].1 <= Duration::from_secs(10) + WRITER_MAX_BACKOFF,
            "the writer got in at {:?}, not when the slow search ended",
            log[1].1
        );
        assert_eq!(*lock.read().unwrap(), 1);
    }

    /// After searches of 600 ms, a new one holds back 1.5 × 600 ms while a
    /// writer waits; one slow search among them does not raise that.
    #[test]
    fn hold_back_follows_recent_searches() {
        let s = Sim::new();
        let lock = Arc::new(RwLock::new(()));
        let log = Arc::new(Mutex::new(Vec::new()));
        let at = |ms: u64| Duration::from_millis(ms);
        {
            // 19 searches of 600 ms and one of 30 s, back to back: done at
            // 41.4 s.
            let lock = Arc::clone(&lock);
            s.spawn(move || {
                for k in 0..20 {
                    let _g = read_within(&*lock, SEARCH_READ_WAIT).unwrap();
                    sim::sleep(if k == 4 {
                        Duration::from_secs(30)
                    } else {
                        600 * MS
                    });
                }
            });
        }
        {
            // A 5 s search from 41.5 s keeps the writer out.
            let lock = Arc::clone(&lock);
            s.spawn(move || {
                sim::sleep(at(41_500));
                let _g = read_within(&*lock, SEARCH_READ_WAIT).unwrap();
                sim::sleep(Duration::from_secs(5));
            });
        }
        {
            // The writer, from 42 s.
            let lock = Arc::clone(&lock);
            s.spawn(move || {
                sim::sleep(at(42_000));
                let _g = write(&*lock);
            });
        }
        {
            let (lock, log, s2) = (Arc::clone(&lock), Arc::clone(&log), Arc::clone(&s));
            s.spawn(move || {
                sim::sleep(at(42_500));
                let _g = read_within(&*lock, SEARCH_READ_WAIT).unwrap();
                log.lock().unwrap().push(s2.elapsed());
            });
        }
        s.run();
        let waited = log.lock().unwrap()[0] - at(42_500);
        assert!(
            waited >= 900 * MS && waited <= 900 * MS + MAX_BACKOFF,
            "held back {waited:?}, not 1.5 × 600 ms"
        );
    }

    /// A search that arrives while the writer holds the lock waits for it
    /// and is then answered; one that would wait past its limit is not.
    #[test]
    fn a_search_waits_out_a_short_write() {
        let s = Sim::new();
        let lock = Arc::new(RwLock::new(()));
        let waited = Arc::new(Mutex::new(Vec::new()));
        {
            let lock = Arc::clone(&lock);
            s.spawn(move || {
                let _g = write(&*lock);
                sim::sleep(Duration::from_secs(2));
            });
        }
        for (start, limit) in [(100, SEARCH_READ_WAIT), (200, Duration::from_secs(1))] {
            let (lock, waited, s2) = (Arc::clone(&lock), Arc::clone(&waited), Arc::clone(&s));
            s.spawn(move || {
                sim::sleep(start * MS);
                let answered = read_within(&*lock, limit).is_some();
                waited.lock().unwrap().push((answered, s2.elapsed()));
            });
        }
        s.run();
        let waited = waited.lock().unwrap().clone();
        // The one with a 1 s limit gives up at 1.2 s; the other is answered
        // when the write ends at 2 s (within one back-off).
        assert_eq!(waited[0], (false, Duration::from_millis(1200)));
        assert!(waited[1].0);
        assert!(
            waited[1].1 >= Duration::from_secs(2)
                && waited[1].1 <= Duration::from_secs(2) + MAX_BACKOFF
        );
    }
}
