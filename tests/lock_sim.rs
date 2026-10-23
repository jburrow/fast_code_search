//! The engine lock and the watcher's install steps, on simulated time.
//!
//! Three server bugs lived in timing, not logic: searches turned away at
//! the end of a big change (#171), every search stuck behind one slow
//! search and a waiting writer (#179), and the watcher falling further
//! behind for minutes under steady searching (#181). Real-thread tests
//! could only approximate them with short sleeps and loose wall-clock
//! bounds. Here the real code (`engine_lock::read_within` /
//! `read_deferring`, `engine_lock::write`, `watch_loop::install_step` with
//! `INSTALL_CHUNK`, `MAX_HOLD`, `STEP_GAP`) runs on the simulated clock of
//! `fast_code_search::sim`: searchers hold the read lock for a simulated
//! search length, the watcher installs chunks that take a simulated time,
//! and minutes of load run in about a second, the same on every run and OS.
//!
//! What is checked, per scenario:
//! - every search is answered (none waits past `SEARCH_READ_WAIT`), and no
//!   search waits longer than its hold-back + one install step;
//! - a backlog drains at half the uncontended speed or better while
//!   searches keep coming (#181);
//! - a slow search delays the writer, never the other searches (#179).
//!
//! `cargo test --release --test lock_sim -- --ignored --nocapture` prints
//! the same scenarios over a grid of `MAX_HOLD` values and hold-back rules:
//! the evidence for the values the code uses (see the book's
//! "Incremental updates" page).

use fast_code_search::search::engine_lock::{
    read_deferring, Defer, DEFER, MAX_BACKOFF, SEARCH_READ_WAIT, WRITER_MAX_BACKOFF,
};
use fast_code_search::search::watch_loop::{install_step, INSTALL_CHUNK, MAX_HOLD, STEP_GAP};
use fast_code_search::sim::{self, Sim};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

const MS: Duration = Duration::from_millis(1);

/// A small deterministic generator (SplitMix64), one per simulated thread.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in `lo..=hi`.
    fn between(&mut self, lo: Duration, hi: Duration) -> Duration {
        let span = (hi - lo).as_micros() as u64 + 1;
        lo + Duration::from_micros(self.next() % span)
    }
}

/// One kind of searcher: how long its searches take and how long it
/// pauses between them.
#[derive(Clone, Copy, Debug)]
struct Searcher {
    count: usize,
    /// When each one starts (picked at random in this range).
    start: (Duration, Duration),
    search: (Duration, Duration),
    pause: (Duration, Duration),
}

#[derive(Clone, Debug)]
struct Scenario {
    name: &'static str,
    searchers: Vec<Searcher>,
    /// Files the watcher has to install, and how long installing
    /// `INSTALL_CHUNK` of them takes with the lock held.
    backlog: usize,
    chunk_time: Duration,
    /// Simulated time to run for (searches stop then; the watcher finishes).
    run_for: Duration,
    seed: u64,
}

#[derive(Debug, Default)]
struct Outcome {
    searches: usize,
    refused: usize,
    longest_search_wait: Duration,
    /// When the backlog was installed (`None`: not within `run_for`).
    drained_at: Option<Duration>,
    longest_hold: Duration,
}

fn run(sc: &Scenario, max_hold: Duration, defer: Defer) -> Outcome {
    let s = Sim::new();
    let lock = Arc::new(RwLock::new(0usize));
    let out = Arc::new(Mutex::new(Outcome::default()));
    let stop = Arc::new(AtomicBool::new(false));
    let mut seed = sc.seed;
    for kind in &sc.searchers {
        for _ in 0..kind.count {
            seed += 1;
            let (lock, out, stop, kind) = (lock.clone(), out.clone(), stop.clone(), *kind);
            let mut rng = Rng(seed);
            s.spawn(move || {
                // Start at different moments.
                sim::sleep(rng.between(kind.start.0, kind.start.1));
                while !stop.load(Ordering::Relaxed) {
                    let asked = sim::now();
                    let guard = read_deferring(&*lock, SEARCH_READ_WAIT, defer);
                    let waited = sim::now() - asked;
                    {
                        let mut o = out.lock().unwrap();
                        o.searches += 1;
                        o.longest_search_wait = o.longest_search_wait.max(waited);
                        if guard.is_none() {
                            o.refused += 1;
                        }
                    }
                    if guard.is_some() {
                        sim::sleep(rng.between(kind.search.0, kind.search.1));
                    }
                    drop(guard);
                    sim::sleep(rng.between(kind.pause.0, kind.pause.1));
                }
            });
        }
    }
    {
        // The watcher: install steps with a gap between them, as
        // `apply_burst` does, until the backlog is in.
        let (lock, out, sc2, s2) = (lock.clone(), out.clone(), sc.clone(), s.clone());
        s.spawn(move || {
            sim::sleep(100 * MS);
            let mut left = sc2.backlog;
            while left > 0 {
                sim::sleep(STEP_GAP);
                let held = install_step(&*lock, max_hold, |n| {
                    let files = left.min(INSTALL_CHUNK);
                    sim::sleep(sc2.chunk_time.mul_f64(files as f64 / INSTALL_CHUNK as f64));
                    *n += files;
                    left -= files;
                    Some(left > 0)
                })
                .unwrap();
                let mut o = out.lock().unwrap();
                o.longest_hold = o.longest_hold.max(held);
            }
            out.lock().unwrap().drained_at = Some(s2.elapsed());
        });
    }
    {
        let (stop, run_for) = (stop.clone(), sc.run_for);
        s.spawn(move || {
            sim::sleep(run_for);
            stop.store(true, Ordering::Relaxed);
        });
    }
    s.run();
    assert_eq!(*lock.read().unwrap(), sc.backlog, "every file installed");
    let mut o = std::mem::take(&mut *out.lock().unwrap());
    if o.drained_at.is_some_and(|d| d > sc.run_for) {
        o.drained_at = None;
    }
    o
}

/// The time the backlog takes with no searches at all.
fn uncontended(sc: &Scenario) -> Duration {
    let steps = sc.backlog.div_ceil(INSTALL_CHUNK) as u32;
    sc.chunk_time * steps
}

fn scenarios() -> Vec<Scenario> {
    vec![
        // #181: 100,000 rewritten files while four people search back to
        // back, half a second a search. Installing 512 files takes 25 ms.
        Scenario {
            name: "steady half-second searches, 100k-file change",
            searchers: vec![Searcher {
                count: 4,
                start: (Duration::ZERO, 600 * MS),
                search: (400 * MS, 600 * MS),
                pause: (Duration::ZERO, 20 * MS),
            }],
            backlog: 100_000,
            chunk_time: 25 * MS,
            run_for: Duration::from_secs(600),
            seed: 1,
        },
        // #179: one 20-second regex and three people with quick searches,
        // while the watcher applies a small change.
        Scenario {
            name: "one slow search, quick searches, small change",
            searchers: vec![
                Searcher {
                    count: 1,
                    start: (Duration::ZERO, Duration::ZERO),
                    search: (20_000 * MS, 20_000 * MS),
                    pause: (Duration::from_secs(1000), Duration::from_secs(1000)),
                },
                Searcher {
                    count: 3,
                    start: (Duration::ZERO, 500 * MS),
                    search: (20 * MS, 80 * MS),
                    pause: (100 * MS, 500 * MS),
                },
            ],
            backlog: 10,
            chunk_time: 25 * MS,
            run_for: Duration::from_secs(60),
            seed: 2,
        },
        // A busy shared server: many quick searches, a few slow ones, a
        // branch switch of 30,000 files.
        Scenario {
            name: "busy server, branch switch",
            searchers: vec![
                Searcher {
                    count: 12,
                    start: (Duration::ZERO, 2000 * MS),
                    search: (5 * MS, 150 * MS),
                    pause: (50 * MS, 2000 * MS),
                },
                Searcher {
                    count: 2,
                    start: (Duration::ZERO, 5000 * MS),
                    search: (1000 * MS, 4000 * MS),
                    pause: (1000 * MS, 5000 * MS),
                },
            ],
            backlog: 30_000,
            chunk_time: 40 * MS,
            run_for: Duration::from_secs(300),
            seed: 3,
        },
    ]
}

/// The bound on any search's wait: holding back for the writer, then one
/// install step (its hold, plus the chunk that runs past it), plus the
/// longest back-off between attempts. `longest_search` is the longest the
/// searches that set the adaptive hold-back take.
fn search_wait_bound(
    defer: Defer,
    longest_search: Duration,
    max_hold: Duration,
    chunk: Duration,
) -> Duration {
    hold_back(defer, longest_search) + max_hold + chunk + MAX_BACKOFF + WRITER_MAX_BACKOFF
}

/// The longest a search holds back when the searches take up to
/// `longest_search`.
fn hold_back(defer: Defer, longest_search: Duration) -> Duration {
    match defer {
        Defer::PerSearch(d) | Defer::Window(d) => d,
        Defer::Adaptive { min, max } => (longest_search * 3 / 2).clamp(min, max),
    }
}

fn longest_search(sc: &Scenario) -> Duration {
    sc.searchers.iter().map(|k| k.search.1).max().unwrap()
}

#[test]
fn searches_are_answered_and_backlogs_drain() {
    for sc in scenarios() {
        let o = run(&sc, MAX_HOLD, DEFER);
        eprintln!("{}: {o:?}", sc.name);
        assert_eq!(o.refused, 0, "{}: searches turned away", sc.name);
        let bound = search_wait_bound(DEFER, longest_search(&sc), MAX_HOLD, sc.chunk_time);
        assert!(
            o.longest_search_wait <= bound,
            "{}: a search waited {:?}, more than {bound:?}",
            sc.name,
            o.longest_search_wait
        );
        let drained = o.drained_at.unwrap_or_else(|| {
            panic!(
                "{}: the backlog was not installed in {:?}",
                sc.name, sc.run_for
            )
        });
        // The searches' own length bounds how soon the watcher can start;
        // after that it must install at half speed or better.
        let longest = longest_search(&sc);
        let limit =
            longest + hold_back(DEFER, longest) + uncontended(&sc) * 2 + Duration::from_secs(1);
        assert!(
            drained <= limit,
            "{}: the backlog took {drained:?}, more than {limit:?} (uncontended {:?})",
            sc.name,
            uncontended(&sc)
        );
    }
}

/// #179: the quick searches never wait for the slow one, only for the
/// writer's hold-back; the writer gets in when the slow search ends.
#[test]
fn a_slow_search_delays_only_the_writer() {
    let sc = &scenarios()[1];
    let o = run(sc, MAX_HOLD, DEFER);
    assert_eq!(o.refused, 0);
    // The slow search is one in many: the hold-back follows the quick ones.
    let quick = sc.searchers[1].search.1;
    assert!(
        o.longest_search_wait
            <= hold_back(DEFER, quick) + sc.chunk_time + MAX_BACKOFF + WRITER_MAX_BACKOFF,
        "a quick search waited {:?}",
        o.longest_search_wait
    );
    let drained = o.drained_at.expect("the change was applied");
    // The slow search runs from 0 to 20 s; the writer asks at 0.1 s.
    assert!(
        drained >= Duration::from_secs(20) && drained <= Duration::from_secs(21),
        "the writer got in at {drained:?}, not when the slow search ended"
    );
}

/// Same seed, same schedule: two runs agree exactly.
#[test]
fn simulation_is_deterministic() {
    let sc = &scenarios()[2];
    let (a, b) = (run(sc, MAX_HOLD, DEFER), run(sc, MAX_HOLD, DEFER));
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
}

/// The evidence for `MAX_HOLD` and the hold-back rule (`DEFER`): each scenario over a
/// grid of values, printed as a table.
#[test]
#[ignore = "prints the tuning table; run with --ignored --nocapture"]
fn tuning_table() {
    let holds = [250 * MS, 500 * MS, 1000 * MS, 2000 * MS];
    let mut defers = Vec::new();
    for d in [Duration::ZERO, 250 * MS, 500 * MS, 1000 * MS] {
        defers.push(Defer::PerSearch(d));
        defers.push(Defer::Window(d));
    }
    defers.push(Defer::Adaptive {
        min: 250 * MS,
        max: 2000 * MS,
    });
    for sc in scenarios() {
        println!("\n{} (uncontended {:?})", sc.name, uncontended(&sc));
        println!("| MAX_HOLD | holding back | drained after | longest search wait | refused |");
        println!("|---|---|---|---|---|");
        for hold in holds {
            for &defer in &defers {
                // Five seeds: the slowest drain and the longest wait.
                let runs: Vec<Outcome> = (0..5)
                    .map(|k| {
                        let sc = Scenario {
                            seed: sc.seed * 100 + k,
                            ..sc.clone()
                        };
                        run(&sc, hold, defer)
                    })
                    .collect();
                let drained = runs
                    .iter()
                    .map(|o| o.drained_at)
                    .max_by_key(|d| d.map(|d| d.as_micros()).unwrap_or(u128::MAX));
                println!(
                    "| {:?} | {:?} | {} | {:?} | {} |",
                    hold,
                    defer,
                    drained
                        .flatten()
                        .map(|d| format!("{:.1} s", d.as_secs_f64()))
                        .unwrap_or_else(|| "not drained".into()),
                    runs.iter().map(|o| o.longest_search_wait).max().unwrap(),
                    runs.iter().map(|o| o.refused).sum::<usize>()
                );
            }
        }
    }
}
