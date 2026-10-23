//! Simulated time for the lock protocol and the watcher's step policy.
//!
//! What decides whether a search is answered while the watcher applies a
//! large change is timing: how long a search holds the index, how long a
//! writer waits for a moment when none does, how long it then stays in
//! ([`crate::search::engine_lock`], [`crate::search::watch_loop`]). Testing
//! that with real threads and real sleeps is slow and flaky, and a test
//! can't make a search take exactly 600 ms or run for four minutes.
//!
//! The lock code reads the time and sleeps through [`now`] and [`sleep`].
//! Outside a simulation they are `Instant::now` and `thread::sleep`. On a
//! thread started by [`Sim::spawn`] they use the simulation's clock: a
//! thread runs only while every other one sleeps, one at a time, and the
//! clock jumps to the next wake-up. So the real lock code runs, the
//! schedule is the same on every run and every OS (it never depends on how
//! a platform's `RwLock` wakes waiters), and four simulated minutes take
//! about a second. A simulated search "works" by holding its guard across a
//! [`sleep`] of its length.
//!
//! The rule for code run in a simulation: it may block only in [`sleep`]
//! (and on locks taken with `try_*`, which never block). A blocking lock
//! held across a sleep by another simulated thread would stop everything.

use crate::search::engine_lock::WriterGate;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

thread_local! {
    static CURRENT: RefCell<Option<(Arc<Sim>, usize)>> = const { RefCell::new(None) };
}

/// The time: the simulation's on a simulated thread, the real one otherwise.
pub fn now() -> Instant {
    CURRENT.with(|c| match &*c.borrow() {
        Some((sim, _)) => sim.now(),
        None => Instant::now(),
    })
}

/// Sleep: let the other simulated threads run for `d` of simulated time,
/// or a real sleep outside a simulation.
pub fn sleep(d: Duration) {
    let current = CURRENT.with(|c| c.borrow().clone());
    match current {
        Some((sim, id)) => sim.sleep(id, d),
        None => std::thread::sleep(d),
    }
}

/// Run `f` with the writers waiting for the engine lock: the simulation's
/// own on a simulated thread (so simulations running side by side, or a
/// real writer elsewhere in the process, don't see each other), the
/// process's otherwise.
pub(crate) fn with_gate<R>(global: &WriterGate, f: impl FnOnce(&WriterGate) -> R) -> R {
    let current = CURRENT.with(|c| c.borrow().as_ref().map(|(sim, _)| Arc::clone(sim)));
    match current {
        Some(sim) => f(&sim.gate),
        None => f(global),
    }
}

/// A simulation: threads that take turns on a simulated clock.
pub struct Sim {
    base: Instant,
    state: Mutex<State>,
    turn: Condvar,
    gate: WriterGate,
}

#[derive(Default)]
struct State {
    now: Duration,
    /// Sleeping threads by (wake-up time, order of the call): the earliest
    /// runs next, and of two waking at once the one that slept first.
    queue: BTreeMap<(Duration, u64), usize>,
    seq: u64,
    /// The thread whose turn it is (`None` before [`Sim::run`] and after
    /// the last thread ends).
    running: Option<usize>,
    started: bool,
    next_id: usize,
    live: usize,
}

impl Sim {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            base: Instant::now(),
            state: Mutex::new(State::default()),
            turn: Condvar::new(),
            gate: WriterGate::new(),
        })
    }

    /// The simulated time since the simulation started.
    pub fn elapsed(&self) -> Duration {
        self.lock().now
    }

    fn now(&self) -> Instant {
        self.base + self.elapsed()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Start `f` on a simulated thread. It first runs when [`Sim::run`]
    /// has started and every thread before it has slept or ended (from a
    /// simulated thread: at the current simulated time).
    pub fn spawn<R: Send + 'static>(
        self: &Arc<Self>,
        f: impl FnOnce() -> R + Send + 'static,
    ) -> std::thread::JoinHandle<R> {
        let id = {
            let mut st = self.lock();
            let id = st.next_id;
            st.next_id += 1;
            st.live += 1;
            let at = (st.now, st.seq);
            st.seq += 1;
            st.queue.insert(at, id);
            id
        };
        let sim = Arc::clone(self);
        std::thread::spawn(move || {
            CURRENT.with(|c| *c.borrow_mut() = Some((Arc::clone(&sim), id)));
            // Ends the thread's turn however `f` ends, a panic included, so
            // the others keep running and the panic reaches `join`.
            struct Exit(Arc<Sim>);
            impl Drop for Exit {
                fn drop(&mut self) {
                    let mut st = self.0.lock();
                    st.live -= 1;
                    self.0.dispatch(&mut st);
                }
            }
            sim.wait_turn(id);
            let _exit = Exit(Arc::clone(&sim));
            f()
        })
    }

    /// Run the spawned threads until every one has ended.
    pub fn run(self: &Arc<Self>) {
        let mut st = self.lock();
        st.started = true;
        if st.running.is_none() {
            self.dispatch(&mut st);
        }
        while st.live > 0 {
            st = self.turn.wait(st).unwrap_or_else(|p| p.into_inner());
        }
    }

    fn sleep(&self, id: usize, d: Duration) {
        {
            let mut st = self.lock();
            debug_assert_eq!(st.running, Some(id), "a simulated thread slept out of turn");
            let at = (st.now + d, st.seq);
            st.seq += 1;
            st.queue.insert(at, id);
            self.dispatch(&mut st);
        }
        self.wait_turn(id);
    }

    /// Give the turn to the earliest sleeper, moving the clock to it.
    fn dispatch(&self, st: &mut State) {
        st.running = None;
        if st.started {
            if let Some(((at, _), next)) = st.queue.pop_first() {
                st.now = st.now.max(at);
                st.running = Some(next);
            }
        }
        self.turn.notify_all();
    }

    fn wait_turn(&self, id: usize) {
        let mut st = self.lock();
        while st.running != Some(id) {
            st = self.turn.wait(st).unwrap_or_else(|p| p.into_inner());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    /// Threads run one at a time in wake-up order, and simulated time does
    /// not pass in real time.
    #[test]
    fn threads_take_turns_in_simulated_time() {
        let sim = Sim::new();
        let log = Arc::new(StdMutex::new(Vec::new()));
        for (name, step) in [("a", 300u64), ("b", 200), ("c", 500)] {
            let log = Arc::clone(&log);
            let sim2 = Arc::clone(&sim);
            sim.spawn(move || {
                for _ in 0..3 {
                    sleep(Duration::from_secs(step));
                    log.lock().unwrap().push((sim2.elapsed().as_secs(), name));
                }
            });
        }
        let started = Instant::now();
        sim.run();
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(sim.elapsed(), Duration::from_secs(1500));
        let log = log.lock().unwrap().clone();
        assert_eq!(
            log,
            vec![
                (200, "b"),
                (300, "a"),
                (400, "b"),
                (500, "c"),
                (600, "a"),
                (600, "b"),
                (900, "a"),
                (1000, "c"),
                (1500, "c"),
            ]
        );
    }

    #[test]
    fn real_time_outside_a_simulation() {
        let before = now();
        sleep(Duration::from_millis(5));
        assert!(now() - before >= Duration::from_millis(5));
    }
}
