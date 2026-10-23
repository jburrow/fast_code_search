//! Diagnostic captures: the server notices when it is slow or stuck and
//! writes what it was doing to a JSON file, to look at later with
//! `fcs diagnose`.
//!
//! Off unless `[diagnostics] capture = true`. While off, the hooks cost one
//! atomic load. While on:
//!
//! - every search registers while it runs ([`SearchProbe`]); one slower
//!   than `slow_search_secs` is captured with its phase timings (waiting for
//!   the index lock, searching, building the reply);
//! - a search that waits longer than `lock_wait_secs` for the index lock is
//!   captured ([`lock_waited`], called by `engine_lock::read_within`);
//! - a watchdog thread ([`start_watchdog`]) samples the server once a
//!   second and captures a hang: the watcher with changes to apply and no
//!   progress, the index build reporting no progress, a search still
//!   running, or the index lock never free, for `stall_secs`. Hang captures
//!   say what every thread is doing. It also captures a watcher that has
//!   had changes to apply for `backlog_secs` without a break.
//!
//! Each capture holds the trigger, the search (if any), the searches
//! running at the time, the latest sample of the index and watcher, the
//! last watcher bursts, process figures and the last log lines. Query text,
//! paths and quoted text are replaced unless `include_queries = true`.
//! Captures of one kind are at least `min_interval_secs` apart, and the
//! oldest are deleted past `max_files` / `max_total_mb`.

use crate::config::DiagnosticsConfig;
use crate::search::{SearchEngine, SearchRankingInfo, SharedIndexingProgress};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Version of the capture file layout.
pub const FORMAT: u32 = 1;

/// Capture files are named `fcs-diag-<time>-<kind>.json`.
pub const FILE_PREFIX: &str = "fcs-diag-";

/// What set a capture off.
pub mod kind {
    /// A search took longer than `slow_search_secs`.
    pub const SLOW_SEARCH: &str = "slow_search";
    /// A search waited longer than `lock_wait_secs` for the index lock.
    pub const LOCK_WAIT: &str = "lock_wait";
    /// The watcher had changes to apply and applied none for `stall_secs`.
    pub const WATCHER_STALLED: &str = "watcher_stalled";
    /// The index build reported no progress for `stall_secs`.
    pub const INDEXER_STALLED: &str = "indexer_stalled";
    /// A search has been running for `stall_secs`.
    pub const SEARCH_STUCK: &str = "search_stuck";
    /// The index lock was never free for `stall_secs`.
    pub const LOCK_HELD: &str = "lock_held";
    /// The watcher has had changes to apply for `backlog_secs` straight.
    pub const BACKLOG: &str = "backlog";
}

/// Most bytes of thread stacks kept in one capture.
const MAX_STACKS_BYTES: usize = 2 << 20;

/// How long the stack tool may take before it is stopped.
const STACKS_TIMEOUT: Duration = Duration::from_secs(60);

/// Watcher bursts remembered for captures.
const RECENT_BURSTS: usize = 20;

static RECORDER: OnceLock<Recorder> = OnceLock::new();

/// Watcher progress: events planned, files parsed and installed. Always
/// counted (one atomic add); the watchdog reads it.
static WATCH_PROGRESS: AtomicU64 = AtomicU64::new(0);

/// Log lines kept for captures; 0 while captures are off.
static LOG_CAPACITY: AtomicUsize = AtomicUsize::new(0);
static LOG_RING: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

/// `[diagnostics]` resolved: durations, and the folder to write to.
#[derive(Debug, Clone)]
pub struct Settings {
    pub dir: PathBuf,
    pub slow_search: Duration,
    pub lock_wait: Duration,
    pub stall: Duration,
    pub backlog: Duration,
    pub min_interval: Duration,
    pub max_files: usize,
    pub max_bytes: u64,
    pub log_lines: usize,
    pub thread_stacks: bool,
    pub include_queries: bool,
}

impl Settings {
    /// The settings when `capture = true`, else `None`.
    pub fn from_config(c: &DiagnosticsConfig, index_path: Option<&str>) -> Option<Self> {
        c.capture.then(|| Self::resolve(c, index_path))
    }

    /// The settings whether or not captures are on (`fcs diagnose` needs
    /// the folder either way).
    pub fn resolve(c: &DiagnosticsConfig, index_path: Option<&str>) -> Self {
        let secs = |s: f64| Duration::from_secs_f64(s.max(0.0));
        Self {
            dir: capture_dir(c, index_path),
            slow_search: secs(c.slow_search_secs),
            lock_wait: secs(c.lock_wait_secs),
            stall: secs(c.stall_secs),
            backlog: secs(c.backlog_secs),
            min_interval: secs(c.min_interval_secs),
            max_files: c.max_files.max(1),
            max_bytes: c.max_total_mb.max(1) << 20,
            log_lines: c.log_lines,
            thread_stacks: c.thread_stacks,
            include_queries: c.include_queries,
        }
    }
}

/// Where captures go: `dir`, else a `diagnostics` folder next to the index
/// file, else the user cache folder.
pub fn capture_dir(c: &DiagnosticsConfig, index_path: Option<&str>) -> PathBuf {
    if !c.dir.is_empty() {
        return PathBuf::from(&c.dir);
    }
    if let Some(parent) = index_path
        .filter(|p| !p.is_empty())
        .and_then(|p| Path::new(p).parent())
        .filter(|p| !p.as_os_str().is_empty())
    {
        return parent.join("diagnostics");
    }
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("fast_code_search")
        .join("diagnostics")
}

/// Turn captures on for this process. Errors if the folder cannot be
/// created; a second call is ignored.
pub fn init(settings: Settings) -> std::io::Result<()> {
    std::fs::create_dir_all(&settings.dir)?;
    LOG_CAPACITY.store(settings.log_lines, Ordering::Release);
    let _ = RECORDER.set(Recorder::new(settings));
    Ok(())
}

/// Whether captures are on.
pub fn enabled() -> bool {
    RECORDER.get().is_some()
}

/// Captures written so far and where, for `/api/diagnostics`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureStatus {
    pub dir: String,
    pub written: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<String>,
}

/// `None` while captures are off.
pub fn status() -> Option<CaptureStatus> {
    let r = RECORDER.get()?;
    Some(CaptureStatus {
        dir: r.settings.dir.display().to_string(),
        written: r.written.load(Ordering::Acquire),
        last: lock(&r.last_file).clone(),
    })
}

/// Count watcher progress (events planned, files parsed or installed).
pub fn watcher_progress(n: usize) {
    WATCH_PROGRESS.fetch_add(n as u64, Ordering::Relaxed);
}

/// Remember a watcher burst for later captures.
pub fn record_burst(
    events: usize,
    indexed: usize,
    removed: usize,
    steps: usize,
    longest_hold: Duration,
    elapsed: Duration,
) {
    let Some(r) = RECORDER.get() else { return };
    let mut bursts = lock(&r.bursts);
    if bursts.len() == RECENT_BURSTS {
        bursts.pop_front();
    }
    bursts.push_back(BurstSample {
        ended_unix_ms: unix_ms(),
        events,
        indexed,
        removed,
        steps,
        longest_hold_ms: ms(longest_hold),
        elapsed_ms: ms(elapsed),
    });
}

/// A search waited `waited` for the index lock (`got`: and then had it),
/// not counting the time it held back for a waiting writer (at most
/// `engine_lock::MAX_READER_DEFER`): that is by design, and with long
/// searches it alone can pass the threshold. Called by
/// `engine_lock::read_within`.
pub fn lock_waited(waited: Duration, got: bool) {
    let Some(r) = RECORDER.get() else { return };
    if waited < r.settings.lock_wait {
        return;
    }
    let summary = if got {
        format!("a search waited {} for the index lock", secs_text(waited))
    } else {
        format!(
            "a search waited {} for the index lock and was turned away",
            secs_text(waited)
        )
    };
    r.capture_in_background(Trigger::new(
        kind::LOCK_WAIT,
        summary,
        waited,
        r.settings.lock_wait,
    ));
}

/// A search while it runs: registered so a hang can name it, and captured
/// when it ends if it was slow. Inert while captures are off.
pub struct SearchProbe(Option<Probe>);

struct Probe {
    id: u64,
    front_end: &'static str,
    mode: String,
    query: String,
    started: Instant,
    locked: Option<Instant>,
    searched: Option<(Instant, SearchFigures)>,
}

#[derive(Debug, Clone, Default)]
struct SearchFigures {
    total_candidates: usize,
    candidates_searched: usize,
    matches: usize,
    limits_hit: Vec<String>,
}

impl SearchProbe {
    /// A search from `front_end` ("web", "grpc", ...) begins.
    pub fn start(front_end: &'static str, mode: &str, query: &str) -> Self {
        let Some(r) = RECORDER.get() else {
            return Self(None);
        };
        let id = r.next_search.fetch_add(1, Ordering::Relaxed);
        lock(&r.active).insert(
            id,
            ActiveSearch {
                front_end,
                mode: mode.to_string(),
                query: query.to_string(),
                started: Instant::now(),
                locked: false,
            },
        );
        Self(Some(Probe {
            id,
            front_end,
            mode: mode.to_string(),
            query: query.to_string(),
            started: Instant::now(),
            locked: None,
            searched: None,
        }))
    }

    /// The search has the index lock.
    pub fn locked(&mut self) {
        let Some(p) = &mut self.0 else { return };
        p.locked = Some(Instant::now());
        if let Some(r) = RECORDER.get() {
            if let Some(a) = lock(&r.active).get_mut(&p.id) {
                a.locked = true;
            }
        }
    }

    /// The engine answered; what is left is building the reply.
    pub fn searched(&mut self, info: &SearchRankingInfo, matches: usize) {
        let Some(p) = &mut self.0 else { return };
        p.searched = Some((
            Instant::now(),
            SearchFigures {
                total_candidates: info.total_candidates,
                candidates_searched: info.candidates_searched,
                matches,
                limits_hit: info.limits_hit.iter().map(|l| format!("{l:?}")).collect(),
            },
        ));
    }
}

impl Drop for SearchProbe {
    fn drop(&mut self) {
        let Some(p) = self.0.take() else { return };
        let Some(r) = RECORDER.get() else { return };
        lock(&r.active).remove(&p.id);
        let ended = Instant::now();
        let elapsed = ended - p.started;
        if elapsed < r.settings.slow_search {
            return;
        }
        let locked = p.locked.unwrap_or(ended);
        let (searched_at, figures) = p.searched.clone().unwrap_or((ended, Default::default()));
        let sample = SearchSample {
            front_end: p.front_end.to_string(),
            mode: p.mode.clone(),
            query: r.redact_query(&p.query),
            query_chars: p.query.chars().count(),
            elapsed_ms: ms(elapsed),
            lock_wait_ms: ms(locked - p.started),
            search_ms: ms(searched_at.saturating_duration_since(locked)),
            reply_ms: ms(ended.saturating_duration_since(searched_at)),
            finished: p.searched.is_some(),
            total_candidates: figures.total_candidates,
            candidates_searched: figures.candidates_searched,
            matches: figures.matches,
            limits_hit: figures.limits_hit,
        };
        let mut trigger = Trigger::new(
            kind::SLOW_SEARCH,
            format!("a {} search took {}", p.mode, secs_text(elapsed)),
            elapsed,
            r.settings.slow_search,
        );
        trigger.search = Some(sample);
        r.capture_in_background(trigger);
    }
}

/// What the watchdog watches.
pub struct Watched {
    pub engine: Arc<RwLock<SearchEngine>>,
    pub progress: Option<SharedIndexingProgress>,
    pub shutdown: Arc<AtomicBool>,
}

/// Start the watchdog thread, if captures are on.
pub fn start_watchdog(watched: Watched) -> Option<std::thread::JoinHandle<()>> {
    let r = RECORDER.get()?;
    std::thread::Builder::new()
        .name("fcs-diag-watchdog".into())
        .spawn(move || {
            let mut dog = Watchdog::new(Instant::now());
            while !watched.shutdown.load(Ordering::Acquire) {
                let now = Instant::now();
                let sample = sample_state(&watched, r);
                for trigger in dog.check(now, &sample, &r.settings, &r.oldest_search()) {
                    r.capture(trigger);
                }
                *lock(&r.state) = Some((now, sample));
                std::thread::sleep(Duration::from_secs(1));
            }
        })
        .ok()
}

/// One look at the server, taken by the watchdog each second.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StateSample {
    /// Seconds between the sample and the capture.
    #[serde(default)]
    pub age_secs: f64,
    /// False when the index lock was not free within 200 ms; the index
    /// fields are then from the last sample that got it.
    pub index_lock_free: bool,
    pub files: Option<usize>,
    pub building: Option<bool>,
    pub watching: Option<bool>,
    /// Changes the watcher has seen and not applied.
    pub pending_changes: usize,
    /// Watcher progress counter (events planned, files parsed and
    /// installed): unchanged between captures means no progress.
    pub watcher_progress: u64,
    /// The index build's progress, as `/api/status` reports it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexing: Option<serde_json::Value>,
}

fn sample_state(w: &Watched, r: &Recorder) -> StateSample {
    let previous = lock(&r.state).as_ref().map(|(_, s)| s.clone());
    let mut s = StateSample {
        watcher_progress: WATCH_PROGRESS.load(Ordering::Relaxed),
        ..Default::default()
    };
    // Not `engine_lock::read_within`: the watchdog's own wait is not a
    // search's, and must not be captured as one.
    let deadline = Instant::now() + Duration::from_millis(200);
    let guard = loop {
        match w.engine.try_read() {
            Ok(g) => break Some(g),
            Err(std::sync::TryLockError::Poisoned(p)) => break Some(p.into_inner()),
            Err(std::sync::TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(5))
            }
            Err(std::sync::TryLockError::WouldBlock) => break None,
        }
    };
    match guard {
        Some(engine) => {
            s.index_lock_free = true;
            s.files = Some(engine.file_store.live_len());
            s.building = Some(engine.building);
            s.watching = Some(engine.watching);
            s.pending_changes = engine.watch_backlog.pending();
        }
        None => {
            if let Some(p) = previous {
                s.files = p.files;
                s.building = p.building;
                s.watching = p.watching;
                s.pending_changes = p.pending_changes;
            }
        }
    }
    if let Some(progress) = &w.progress {
        if let Ok(p) = progress.try_read() {
            use crate::search::IndexingStatus as S;
            if !matches!(p.status, S::Idle | S::Completed) {
                s.indexing = serde_json::to_value(&*p).ok();
            }
        }
    }
    s
}

/// A backlog the watcher applies as much as within this long counts as kept
/// up with, for the `backlog` capture. Measured with `examples/scoreboard.rs`
/// (`stream`: 200 files a second on 200,000 files): the watcher kept 400 to
/// 1,250 changes pending for ten minutes, each a few seconds' work, and the
/// capture fired after 300 s for a backlog that was never more than seconds
/// old.
const KEEPING_UP: Duration = Duration::from_secs(10);

/// The watchdog's memory between samples: since when each condition has
/// held, and which have been captured this time round.
#[derive(Debug)]
struct Watchdog {
    lock_busy_since: Option<Instant>,
    pending_since: Option<Instant>,
    /// Watcher progress over the last [`KEEPING_UP`], oldest first.
    recent_progress: std::collections::VecDeque<(Instant, u64)>,
    watcher_progress: u64,
    watcher_moved: Instant,
    indexing: Option<String>,
    indexing_moved: Instant,
    fired: HashSet<&'static str>,
    stuck_searches: HashSet<u64>,
}

impl Watchdog {
    fn new(now: Instant) -> Self {
        Self {
            lock_busy_since: None,
            pending_since: None,
            recent_progress: std::collections::VecDeque::new(),
            watcher_progress: 0,
            watcher_moved: now,
            indexing: None,
            indexing_moved: now,
            fired: HashSet::new(),
            stuck_searches: HashSet::new(),
        }
    }

    /// Fire `kind` once while `on` holds; re-arm when it stops holding.
    fn once(&mut self, kind: &'static str, on: bool) -> bool {
        if !on {
            self.fired.remove(kind);
            return false;
        }
        self.fired.insert(kind)
    }

    /// The captures `s` calls for.
    fn check(
        &mut self,
        now: Instant,
        s: &StateSample,
        cfg: &Settings,
        oldest: &Option<(u64, Duration, String)>,
    ) -> Vec<Trigger> {
        let mut out = Vec::new();

        let busy = (!s.index_lock_free).then(|| *self.lock_busy_since.get_or_insert(now));
        if s.index_lock_free {
            self.lock_busy_since = None;
        }
        let busy_for = busy.map_or(Duration::ZERO, |b| now - b);
        if self.once(kind::LOCK_HELD, busy_for >= cfg.stall) {
            out.push(Trigger::new(
                kind::LOCK_HELD,
                format!(
                    "the index lock has not been free for {}",
                    secs_text(busy_for)
                ),
                busy_for,
                cfg.stall,
            ));
        }

        self.recent_progress.push_back((now, s.watcher_progress));
        while self
            .recent_progress
            .front()
            .is_some_and(|&(t, _)| now.saturating_duration_since(t) > KEEPING_UP)
        {
            self.recent_progress.pop_front();
        }
        let applied_lately = self
            .recent_progress
            .front()
            .map_or(0, |&(_, p)| s.watcher_progress.saturating_sub(p));
        // A steady stream of changes never brings the backlog to zero; a
        // watcher that applies as much as is pending within `KEEPING_UP`
        // is keeping up with it, not falling behind.
        if s.pending_changes == 0 || s.pending_changes as u64 <= applied_lately {
            self.pending_since = None;
        }
        let pending_for = if s.pending_changes > 0 {
            now - *self.pending_since.get_or_insert(now)
        } else {
            Duration::ZERO
        };
        if s.watcher_progress != self.watcher_progress || s.pending_changes == 0 {
            self.watcher_progress = s.watcher_progress;
            self.watcher_moved = now;
        }
        let still_for = now - self.watcher_moved;
        if self.once(kind::WATCHER_STALLED, still_for >= cfg.stall) {
            out.push(Trigger::new(
                kind::WATCHER_STALLED,
                format!(
                    "the watcher has {} changes to apply and has made no progress for {}",
                    s.pending_changes,
                    secs_text(still_for)
                ),
                still_for,
                cfg.stall,
            ));
        }
        if self.once(kind::BACKLOG, pending_for >= cfg.backlog) {
            out.push(Trigger::new(
                kind::BACKLOG,
                format!(
                    "the watcher has had changes to apply for {} ({} now)",
                    secs_text(pending_for),
                    s.pending_changes
                ),
                pending_for,
                cfg.backlog,
            ));
        }

        let indexing = s.indexing.as_ref().map(|v| v.to_string());
        if indexing.is_none() || indexing != self.indexing {
            self.indexing = indexing;
            self.indexing_moved = now;
        }
        let idle_for = now - self.indexing_moved;
        if self.once(kind::INDEXER_STALLED, idle_for >= cfg.stall) {
            out.push(Trigger::new(
                kind::INDEXER_STALLED,
                format!(
                    "the index build has reported no progress for {}",
                    secs_text(idle_for)
                ),
                idle_for,
                cfg.stall,
            ));
        }

        if let Some((id, age, mode)) = oldest {
            if *age >= cfg.stall && self.stuck_searches.insert(*id) {
                out.push(Trigger::new(
                    kind::SEARCH_STUCK,
                    format!("a {mode} search has been running for {}", secs_text(*age)),
                    *age,
                    cfg.stall,
                ));
            }
        }
        out
    }
}

/// What set a capture off.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    pub kind: String,
    pub summary: String,
    pub observed_ms: u64,
    pub threshold_ms: u64,
    #[serde(skip)]
    search: Option<SearchSample>,
}

impl Trigger {
    fn new(kind: &'static str, summary: String, observed: Duration, threshold: Duration) -> Self {
        Self {
            kind: kind.to_string(),
            summary,
            observed_ms: ms(observed),
            threshold_ms: ms(threshold),
            search: None,
        }
    }

    /// Hangs are worth the thread stacks; slowness is not.
    fn wants_stacks(&self) -> bool {
        matches!(
            self.kind.as_str(),
            kind::WATCHER_STALLED | kind::INDEXER_STALLED | kind::SEARCH_STUCK | kind::LOCK_HELD
        )
    }
}

/// The search a `slow_search` capture is about.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchSample {
    pub front_end: String,
    pub mode: String,
    /// The query, or `<redacted>` unless `include_queries` is on.
    pub query: String,
    pub query_chars: usize,
    pub elapsed_ms: u64,
    pub lock_wait_ms: u64,
    pub search_ms: u64,
    /// Building the reply (context lines, JSON).
    pub reply_ms: u64,
    /// False when the search failed or was turned away.
    pub finished: bool,
    pub total_candidates: usize,
    pub candidates_searched: usize,
    pub matches: usize,
    pub limits_hit: Vec<String>,
}

/// A search running when the capture was taken.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveSample {
    pub front_end: String,
    pub mode: String,
    pub query: String,
    pub running_ms: u64,
    pub has_lock: bool,
}

/// A watcher burst, as `watch_loop` reports it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BurstSample {
    pub ended_unix_ms: u64,
    pub events: usize,
    pub indexed: usize,
    pub removed: usize,
    pub steps: usize,
    pub longest_hold_ms: u64,
    pub elapsed_ms: u64,
}

/// Process figures at the time of a capture.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProcessSample {
    pub pid: u32,
    pub uptime_secs: u64,
    pub cpus: usize,
    pub threads: Option<usize>,
    pub rss_bytes: Option<u64>,
}

/// One thread, as the OS reports it (Linux).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadInfo {
    pub tid: u64,
    pub name: String,
    /// R running, S sleeping, D waiting on the disk, ...
    pub state: String,
    /// The kernel function a sleeping thread waits in.
    pub wchan: String,
}

/// What every thread was doing, for hang captures.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThreadsReport {
    pub threads: Vec<ThreadInfo>,
    /// Every thread's stack from `gdb` or `eu-stack`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stacks: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stacks_tool: Option<String>,
    /// Why there are no stacks, when there are none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// One capture file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capture {
    pub format: u32,
    pub version: String,
    pub captured_at: String,
    pub unix_ms: u64,
    pub trigger: Trigger,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<SearchSample>,
    pub active_searches: Vec<ActiveSample>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<StateSample>,
    pub recent_bursts: Vec<BurstSample>,
    pub process: ProcessSample,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threads: Option<ThreadsReport>,
    pub log: Vec<String>,
    /// True when query text, paths and quoted text were replaced.
    pub redacted: bool,
}

struct ActiveSearch {
    front_end: &'static str,
    mode: String,
    query: String,
    started: Instant,
    locked: bool,
}

struct Recorder {
    settings: Settings,
    last_by_kind: Mutex<HashMap<String, Instant>>,
    next_search: AtomicU64,
    active: Mutex<HashMap<u64, ActiveSearch>>,
    bursts: Mutex<VecDeque<BurstSample>>,
    state: Mutex<Option<(Instant, StateSample)>>,
    written: AtomicUsize,
    last_file: Mutex<Option<String>>,
}

impl Recorder {
    fn new(settings: Settings) -> Self {
        Self {
            settings,
            last_by_kind: Mutex::new(HashMap::new()),
            next_search: AtomicU64::new(0),
            active: Mutex::new(HashMap::new()),
            bursts: Mutex::new(VecDeque::new()),
            state: Mutex::new(None),
            written: AtomicUsize::new(0),
            last_file: Mutex::new(None),
        }
    }

    /// The longest-running search: (id, running for, mode).
    fn oldest_search(&self) -> Option<(u64, Duration, String)> {
        lock(&self.active)
            .iter()
            .max_by_key(|(_, a)| a.started.elapsed())
            .map(|(id, a)| (*id, a.started.elapsed(), a.mode.clone()))
    }

    /// Claim the right to capture `kind` now; false within
    /// `min_interval` of the last one.
    fn claim(&self, kind: &str) -> bool {
        let mut last = lock(&self.last_by_kind);
        let now = Instant::now();
        if let Some(t) = last.get(kind) {
            if now.duration_since(*t) < self.settings.min_interval {
                return false;
            }
        }
        last.insert(kind.to_string(), now);
        true
    }

    /// Capture on a thread of its own, so the caller (a search) is not
    /// held up writing the file.
    fn capture_in_background(&'static self, trigger: Trigger) {
        if !self.claim(&trigger.kind) {
            return;
        }
        let _ = std::thread::Builder::new()
            .name("fcs-diag-capture".into())
            .spawn(move || self.write(trigger));
    }

    /// Capture now, unless one of this kind was taken recently.
    fn capture(&self, trigger: Trigger) -> Option<PathBuf> {
        if !self.claim(&trigger.kind) {
            return None;
        }
        self.write(trigger)
    }

    fn write(&self, mut trigger: Trigger) -> Option<PathBuf> {
        let search = trigger.search.take();
        let stacks = trigger.wants_stacks();
        let capture = self.build(trigger, search, stacks);
        match write_capture(&self.settings, &capture) {
            Ok(path) => {
                self.written.fetch_add(1, Ordering::AcqRel);
                *lock(&self.last_file) = Some(path.display().to_string());
                tracing::warn!(
                    "Diagnostic capture ({}): {}; written to {}",
                    capture.trigger.kind,
                    capture.trigger.summary,
                    path.display()
                );
                Some(path)
            }
            Err(e) => {
                tracing::warn!(error = %e, "Could not write a diagnostic capture");
                None
            }
        }
    }

    fn build(&self, trigger: Trigger, search: Option<SearchSample>, stacks: bool) -> Capture {
        let now = Instant::now();
        let active_searches = lock(&self.active)
            .values()
            .map(|a| ActiveSample {
                front_end: a.front_end.to_string(),
                mode: a.mode.clone(),
                query: self.redact_query(&a.query),
                running_ms: ms(now.saturating_duration_since(a.started)),
                has_lock: a.locked,
            })
            .collect();
        let state = lock(&self.state).as_ref().map(|(at, s)| StateSample {
            age_secs: now.saturating_duration_since(*at).as_secs_f64(),
            indexing: s.indexing.as_ref().map(|v| self.redact_json(v)),
            ..s.clone()
        });
        let threads = stacks.then(|| thread_report(self.settings.thread_stacks));
        Capture {
            format: FORMAT,
            version: env!("CARGO_PKG_VERSION").to_string(),
            captured_at: chrono::Utc::now().to_rfc3339(),
            unix_ms: unix_ms(),
            trigger,
            search,
            active_searches,
            state,
            recent_bursts: lock(&self.bursts).iter().cloned().collect(),
            process: process_sample(),
            threads,
            log: log_lines()
                .into_iter()
                .map(|l| {
                    // "<time> <level> <text>": only the text can hold names.
                    let mut parts = l.splitn(3, ' ');
                    match (parts.next(), parts.next(), parts.next()) {
                        (Some(time), Some(level), Some(text)) => {
                            format!("{time} {level} {}", self.redact_text(text))
                        }
                        _ => self.redact_text(&l),
                    }
                })
                .collect(),
            redacted: !self.settings.include_queries,
        }
    }

    fn redact_query(&self, q: &str) -> String {
        if self.settings.include_queries {
            q.to_string()
        } else {
            "<redacted>".to_string()
        }
    }

    fn redact_text(&self, s: &str) -> String {
        if self.settings.include_queries {
            s.to_string()
        } else {
            redact(s)
        }
    }

    fn redact_json(&self, v: &serde_json::Value) -> serde_json::Value {
        match v {
            serde_json::Value::String(s) => serde_json::Value::String(self.redact_text(s)),
            serde_json::Value::Array(a) => a.iter().map(|v| self.redact_json(v)).collect(),
            serde_json::Value::Object(o) => o
                .iter()
                .map(|(k, v)| (k.clone(), self.redact_json(v)))
                .collect(),
            other => other.clone(),
        }
    }
}

/// Field names whose values are always replaced.
const SECRET_FIELDS: &[&str] = &["query", "q", "pattern", "path", "file", "dir", "root"];

/// `s` with quoted text, paths and file names, and the values of
/// fields such as `query=` and `path=` replaced, so a capture shows what happened without
/// what was searched or which files were involved.
pub fn redact(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        if !word.is_empty() {
            out.push_str(&redact_word(word));
            word.clear();
        }
    };
    while let Some(c) = chars.next() {
        match c {
            '"' | '`' => {
                flush(&mut word, &mut out);
                // Up to the closing quote (escaped quotes included).
                let mut quoted = String::new();
                let mut escaped = false;
                for d in chars.by_ref() {
                    if escaped {
                        escaped = false;
                    } else if d == '\\' {
                        escaped = true;
                    } else if d == c {
                        break;
                    }
                    quoted.push(d);
                }
                if is_measurement(&quoted) {
                    out.push(c);
                    out.push_str(&quoted);
                    out.push(c);
                } else {
                    out.push_str("<text>");
                }
            }
            c if c.is_whitespace() || c == ',' || c == '(' || c == ')' => {
                flush(&mut word, &mut out);
                out.push(c);
            }
            c => word.push(c),
        }
    }
    flush(&mut word, &mut out);
    out
}

/// A number, a version or a size (`0.18.1`, `1.25`, `672.67 KB`): safe to
/// keep, and useful.
fn is_measurement(s: &str) -> bool {
    s.chars().any(|c| c.is_ascii_digit())
        && s.chars().filter(|c| c.is_alphabetic()).count() <= 3
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | ' ' | '%' | ':' | '-'))
}

fn redact_word(w: &str) -> String {
    if let Some((key, value)) = w.split_once('=') {
        if SECRET_FIELDS.contains(&key) && !value.is_empty() {
            return format!("{key}=<redacted>");
        }
        return format!("{key}={}", redact_word(value));
    }
    if w.contains('/') || w.contains('\\') || looks_like_file_name(w) {
        return "<path>".to_string();
    }
    w.to_string()
}

/// `main.rs`, `index.d.ts`: a name, a dot, and a short extension that is
/// not a number (`1.5 s` stays).
fn looks_like_file_name(w: &str) -> bool {
    let w = w.trim_end_matches([':', ';', '.']);
    let Some((stem, ext)) = w.rsplit_once('.') else {
        return false;
    };
    !stem.is_empty()
        && (1..=6).contains(&ext.len())
        && ext.chars().all(|c| c.is_ascii_alphanumeric())
        && !ext.chars().all(|c| c.is_ascii_digit())
        && !stem.chars().all(|c| c.is_ascii_digit() || c == '.')
}

fn write_capture(settings: &Settings, capture: &Capture) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(&settings.dir)?;
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ");
    let mut path = settings.dir.join(format!(
        "{FILE_PREFIX}{stamp}-{}.json",
        capture.trigger.kind
    ));
    let mut n = 1;
    while path.exists() {
        n += 1;
        path = settings.dir.join(format!(
            "{FILE_PREFIX}{stamp}-{}-{n}.json",
            capture.trigger.kind
        ));
    }
    let json = serde_json::to_vec_pretty(capture).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, &path)?;
    prune(settings);
    Ok(path)
}

/// Delete the oldest captures past `max_files` or `max_bytes`.
fn prune(settings: &Settings) {
    let mut files = capture_files(&settings.dir);
    // Names start with the time, so name order is age order.
    files.sort();
    let sizes: Vec<u64> = files
        .iter()
        .map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
        .collect();
    let mut total: u64 = sizes.iter().sum();
    let mut count = files.len();
    for (path, size) in files.iter().zip(sizes) {
        if count <= settings.max_files && total <= settings.max_bytes {
            break;
        }
        // Never delete the only (newest) capture.
        if count == 1 {
            break;
        }
        if std::fs::remove_file(path).is_ok() {
            count -= 1;
            total = total.saturating_sub(size);
        }
    }
}

/// The capture files in `dir`.
pub fn capture_files(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(FILE_PREFIX) && n.ends_with(".json"))
        })
        .collect()
}

fn process_sample() -> ProcessSample {
    let mut p = ProcessSample {
        pid: std::process::id(),
        uptime_secs: super::get_uptime_secs(),
        cpus: std::thread::available_parallelism().map_or(1, |n| n.get()),
        ..Default::default()
    };
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            let mut parts = line.split_whitespace();
            match parts.next() {
                Some("VmRSS:") => {
                    p.rss_bytes = parts
                        .next()
                        .and_then(|v| v.parse::<u64>().ok())
                        .map(|kb| kb * 1024)
                }
                Some("Threads:") => p.threads = parts.next().and_then(|v| v.parse().ok()),
                _ => {}
            }
        }
    }
    p
}

/// The process's threads and, if `stacks` and a tool is installed, every
/// thread's stack.
pub fn thread_report(stacks: bool) -> ThreadsReport {
    let mut report = ThreadsReport {
        threads: proc_threads(),
        ..Default::default()
    };
    if !stacks {
        report.note = Some("thread_stacks is off".into());
        return report;
    }
    match collect_stacks() {
        Ok((tool, text)) => {
            report.stacks_tool = Some(tool);
            report.stacks = Some(text);
        }
        Err(why) => report.note = Some(why),
    }
    report
}

fn proc_threads() -> Vec<ThreadInfo> {
    let mut out: Vec<ThreadInfo> = std::fs::read_dir("/proc/self/task")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let tid: u64 = e.file_name().to_str()?.parse().ok()?;
            let dir = e.path();
            let read = |f: &str| {
                std::fs::read_to_string(dir.join(f))
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default()
            };
            let stat = read("stat");
            // "tid (name) S ...": the state follows the last ')'.
            let state = stat
                .rsplit_once(')')
                .and_then(|(_, rest)| rest.split_whitespace().next())
                .unwrap_or("?")
                .to_string();
            Some(ThreadInfo {
                tid,
                name: read("comm"),
                state,
                wchan: read("wchan"),
            })
        })
        .collect();
    out.sort_by_key(|t| t.tid);
    out
}

fn on_path(tool: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|d| d.join(tool).is_file()))
}

fn collect_stacks() -> Result<(String, String), String> {
    if !cfg!(target_os = "linux") {
        return Err("thread stacks are recorded on Linux only".into());
    }
    let pid = std::process::id().to_string();
    let (tool, args): (&str, Vec<String>) = if on_path("gdb") {
        (
            "gdb",
            [
                "-p",
                &pid,
                "-batch",
                "-nx",
                "-q",
                "-ex",
                "set pagination off",
                "-ex",
                "set print frame-arguments none",
                "-ex",
                "thread apply all bt",
            ]
            .map(String::from)
            .to_vec(),
        )
    } else if on_path("eu-stack") {
        ("eu-stack", vec!["-p".into(), pid])
    } else {
        return Err("install gdb or eu-stack (elfutils) to record thread stacks".into());
    };
    allow_tracing_by_children();
    let mut child = std::process::Command::new(tool)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run {tool}: {e}"))?;
    let mut stdout = child.stdout.take().expect("piped");
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stdout, &mut buf);
        buf
    });
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() > STACKS_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "{tool} did not finish within {}s",
                    STACKS_TIMEOUT.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(format!("{tool}: {e}")),
        }
    }
    let mut text = String::from_utf8_lossy(&reader.join().unwrap_or_default()).into_owned();
    if text.trim().is_empty() {
        let mut err = String::new();
        if let Some(mut e) = child.stderr.take() {
            let _ = std::io::Read::read_to_string(&mut e, &mut err);
        }
        return Err(format!(
            "{tool} printed no stacks: {}",
            err.lines().last().unwrap_or("no output").trim()
        ));
    }
    if text.len() > MAX_STACKS_BYTES {
        let mut cut = MAX_STACKS_BYTES;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
        text.push_str("\n... (cut)\n");
    }
    Ok((tool.to_string(), text))
}

/// Under Yama's `ptrace_scope = 1` (Ubuntu's default) a process may only be
/// traced by its ancestors; let the stack tool we start trace us.
fn allow_tracing_by_children() {
    #[cfg(target_os = "linux")]
    // SAFETY: prctl with PR_SET_PTRACER takes plain integers and changes
    // only who may trace this process.
    unsafe {
        libc::prctl(libc::PR_SET_PTRACER, libc::PR_SET_PTRACER_ANY, 0, 0, 0);
    }
}

/// The log lines kept for captures, oldest first.
fn log_lines() -> Vec<String> {
    lock(&LOG_RING).iter().cloned().collect()
}

/// A tracing layer that keeps the last log lines for captures (only while
/// captures are on).
pub struct LogRingLayer;

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for LogRingLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let cap = LOG_CAPACITY.load(Ordering::Relaxed);
        if cap == 0 {
            return;
        }
        let mut fields = LineVisitor::default();
        event.record(&mut fields);
        let meta = event.metadata();
        let line = format!(
            "{} {} {}{}",
            chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ"),
            meta.level(),
            fields.message,
            fields.rest
        );
        let mut ring = lock(&LOG_RING);
        while ring.len() >= cap {
            ring.pop_front();
        }
        ring.push_back(line);
    }
}

#[derive(Default)]
struct LineVisitor {
    message: String,
    rest: String,
}

impl tracing::field::Visit for LineVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message.push_str(value);
        } else {
            let _ = write!(self.rest, " {}={value:?}", field.name());
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
        } else {
            let _ = write!(self.rest, " {}={value:?}", field.name());
        }
    }
}

/// A capture file that could not be read, and why.
pub type Unreadable = (PathBuf, String);

/// Read the captures in `dir`, oldest first, with the files that could not
/// be read.
pub fn load_captures(dir: &Path) -> (Vec<(PathBuf, Capture)>, Vec<Unreadable>) {
    let mut files = capture_files(dir);
    files.sort();
    let mut ok = Vec::new();
    let mut bad = Vec::new();
    for path in files {
        match std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|b| serde_json::from_slice::<Capture>(&b).map_err(|e| e.to_string()))
        {
            Ok(c) => ok.push((path, c)),
            Err(e) => bad.push((path, e)),
        }
    }
    (ok, bad)
}

/// `fcs diagnose`: what the captures in a folder say.
pub fn summarize(dir: &Path, captures: &[(PathBuf, Capture)], bad: &[Unreadable]) -> String {
    let mut out = String::new();
    if captures.is_empty() {
        let _ = writeln!(out, "No diagnostic captures in {}.", dir.display());
        if bad.is_empty() {
            let _ = writeln!(
                out,
                "Captures are written when [diagnostics] capture = true and the server is slow or stuck."
            );
        }
    } else {
        let first = &captures[0].1;
        let last = &captures[captures.len() - 1].1;
        let _ = writeln!(
            out,
            "{} capture{} in {}, {} to {}",
            captures.len(),
            if captures.len() == 1 { "" } else { "s" },
            dir.display(),
            short_time(&first.captured_at),
            short_time(&last.captured_at)
        );
        let mut by_kind: Vec<(String, usize)> = Vec::new();
        for (_, c) in captures {
            match by_kind.iter_mut().find(|(k, _)| *k == c.trigger.kind) {
                Some((_, n)) => *n += 1,
                None => by_kind.push((c.trigger.kind.clone(), 1)),
            }
        }
        out.push('\n');
        for (k, n) in &by_kind {
            let _ = writeln!(out, "  {k:<16} {n}");
        }
        if let Some((_, slowest)) = captures
            .iter()
            .filter_map(|(_, c)| c.search.as_ref().map(|s| (c, s)))
            .max_by_key(|(_, s)| s.elapsed_ms)
        {
            let _ = writeln!(
                out,
                "\nSlowest search: {} ({} search via {}): waited {} for the index, searched {}, built the reply in {}",
                secs_ms(slowest.elapsed_ms),
                slowest.mode,
                slowest.front_end,
                secs_ms(slowest.lock_wait_ms),
                secs_ms(slowest.search_ms),
                secs_ms(slowest.reply_ms)
            );
        }
        let _ = writeln!(out, "\nNewest first:");
        for (path, c) in captures.iter().rev() {
            let _ = writeln!(
                out,
                "\n{}  {}  v{}\n  {}",
                short_time(&c.captured_at),
                c.trigger.kind,
                c.version,
                c.trigger.summary
            );
            if let Some(s) = &c.state {
                let _ = writeln!(
                    out,
                    "  index: {} files, {} changes to apply{}{}",
                    s.files.map_or("?".into(), |n| n.to_string()),
                    s.pending_changes,
                    if s.building == Some(true) {
                        ", building"
                    } else {
                        ""
                    },
                    if s.index_lock_free { "" } else { ", lock busy" }
                );
            }
            if !c.active_searches.is_empty() {
                let longest = c
                    .active_searches
                    .iter()
                    .map(|a| a.running_ms)
                    .max()
                    .unwrap_or(0);
                let _ = writeln!(
                    out,
                    "  {} searches running, the longest for {}",
                    c.active_searches.len(),
                    secs_ms(longest)
                );
            }
            if let Some(b) = c.recent_bursts.last() {
                let _ = writeln!(
                    out,
                    "  last watcher burst: {} events, {} indexed, {} removed, {} steps, longest hold {}, took {}",
                    b.events,
                    b.indexed,
                    b.removed,
                    b.steps,
                    secs_ms(b.longest_hold_ms),
                    secs_ms(b.elapsed_ms)
                );
            }
            if let Some(t) = &c.threads {
                for line in thread_summary(t) {
                    let _ = writeln!(out, "  {line}");
                }
            }
            let _ = writeln!(out, "  file: {}", path.display());
        }
    }
    for (path, why) in bad {
        let _ = writeln!(out, "\nCould not read {}: {why}", path.display());
    }
    out
}

/// One line per fcs thread: where its stack is in fcs's own code.
fn thread_summary(t: &ThreadsReport) -> Vec<String> {
    let mut lines = Vec::new();
    let waiting = t.threads.iter().filter(|t| t.state == "D").count();
    let running = t.threads.iter().filter(|t| t.state == "R").count();
    lines.push(format!(
        "threads: {} ({running} running, {waiting} waiting on the disk)",
        t.threads.len()
    ));
    let Some(stacks) = &t.stacks else {
        if let Some(n) = &t.note {
            lines.push(format!("no stacks: {n}"));
        }
        return lines;
    };
    // gdb: "Thread N (Thread 0x.. (LWP tid) "name"):" then "#k  ... in fn (...) at file:line".
    let mut current: Option<String> = None;
    let mut found = false;
    for line in stacks.lines() {
        let l = line.trim();
        if l.starts_with("Thread ") || l.starts_with("TID ") {
            current = Some(
                l.rsplit_once('"')
                    .and_then(|(head, _)| head.rsplit_once('"'))
                    .map(|(_, name)| name.to_string())
                    .unwrap_or_else(|| l.trim_end_matches(':').to_string()),
            );
            found = false;
        } else if !found && l.contains("fast_code_search::") {
            if let Some(name) = &current {
                let func = l
                    .split(" in ")
                    .nth(1)
                    .unwrap_or(l)
                    .split(" (")
                    .next()
                    .unwrap_or(l);
                lines.push(format!("  {name}: {func}"));
                found = true;
            }
        }
    }
    lines
}

fn short_time(rfc3339: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|_| rfc3339.to_string())
}

fn secs_ms(ms: u64) -> String {
    secs_text(Duration::from_millis(ms))
}

fn secs_text(d: Duration) -> String {
    let s = d.as_secs_f64();
    if s < 1.0 {
        format!("{} ms", d.as_millis())
    } else if s < 10.0 {
        format!("{s:.1} s")
    } else {
        format!("{s:.0} s")
    }
}

fn ms(d: Duration) -> u64 {
    d.as_millis().min(u64::MAX as u128) as u64
}

fn unix_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, ms)
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(dir: &Path) -> Settings {
        let mut s = Settings::resolve(&DiagnosticsConfig::default(), None);
        s.dir = dir.to_path_buf();
        s.thread_stacks = false;
        s
    }

    fn sample(pending: usize, progress: u64) -> StateSample {
        StateSample {
            index_lock_free: true,
            pending_changes: pending,
            watcher_progress: progress,
            ..Default::default()
        }
    }

    #[test]
    fn redaction_keeps_the_shape_not_the_text() {
        assert_eq!(
            redact(r#"Search for "secret thing" in src/billing/api.rs took 1.5 s"#),
            "Search for <text> in <path> took 1.5 s"
        );
        assert_eq!(
            redact("query=Invoice files=12"),
            "query=<redacted> files=12"
        );
        assert_eq!(
            redact("indexed=512 elapsed_ms=40"),
            "indexed=512 elapsed_ms=40"
        );
        assert_eq!(
            redact("Loaded config.toml, 3 paths"),
            "Loaded <path>, 3 paths"
        );
        assert_eq!(redact(r"C:\work\repo"), "<path>");
        assert_eq!(redact("v0.18.1"), "v0.18.1");
        assert_eq!(
            redact(r#"version="0.18.1" size="672.67 KB" name="billing""#),
            r#"version="0.18.1" size="672.67 KB" name=<text>"#
        );
    }

    /// A watcher with work and no progress for `stall` is a hang; one
    /// that keeps applying is not, however long it takes, until `backlog`.
    #[test]
    fn watchdog_tells_a_stalled_watcher_from_a_busy_one() {
        let mut cfg = settings(Path::new("."));
        cfg.stall = Duration::from_secs(10);
        cfg.backlog = Duration::from_secs(30);
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        let mut dog = Watchdog::new(t0);

        // Busy: progress every second.
        for s in 0..25 {
            let fired = dog.check(at(s), &sample(1000, s), &cfg, &None);
            assert!(fired.is_empty(), "second {s}: {fired:?}");
        }
        // Still busy at 30 s: the backlog fires, once.
        let fired = dog.check(at(30), &sample(1000, 30), &cfg, &None);
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].kind, kind::BACKLOG);
        assert!(dog.check(at(31), &sample(1000, 31), &cfg, &None).is_empty());

        // Stuck at progress 31 from 31 s: a stall at 41 s, once.
        for s in 32..41 {
            assert!(dog.check(at(s), &sample(1000, 31), &cfg, &None).is_empty());
        }
        let fired = dog.check(at(41), &sample(1000, 31), &cfg, &None);
        assert_eq!(fired.len(), 1, "{fired:?}");
        assert_eq!(fired[0].kind, kind::WATCHER_STALLED);
        assert!(dog.check(at(42), &sample(1000, 31), &cfg, &None).is_empty());

        // Caught up: everything re-arms.
        assert!(dog.check(at(43), &sample(0, 31), &cfg, &None).is_empty());
        assert!(dog.fired.is_empty());
    }

    /// A steady stream of changes keeps a few hundred pending for ever;
    /// the watcher applying them as fast as they come is not a backlog.
    #[test]
    fn watchdog_lets_a_watcher_keep_up_with_a_stream() {
        let mut cfg = settings(Path::new("."));
        cfg.backlog = Duration::from_secs(300);
        let t0 = Instant::now();
        let mut dog = Watchdog::new(t0);
        for s in 0..600 {
            // 200 changes a second applied, 400 to 1,200 pending.
            let pending = 400 + (s as usize * 37) % 800;
            let fired = dog.check(
                t0 + Duration::from_secs(s),
                &sample(pending, 200 * s),
                &cfg,
                &None,
            );
            assert!(fired.is_empty(), "second {s}: {fired:?}");
        }
        // The stream outgrows the watcher: 5,000 pending, 100 a second
        // applied. That is a backlog, 300 s on.
        let mut fired = Vec::new();
        for s in 600..901 {
            fired.extend(dog.check(
                t0 + Duration::from_secs(s),
                &sample(5000, 120_000 + 100 * (s - 600)),
                &cfg,
                &None,
            ));
        }
        assert_eq!(fired.len(), 1, "{fired:?}");
        assert_eq!(fired[0].kind, kind::BACKLOG);
    }

    #[test]
    fn watchdog_reports_a_busy_lock_and_a_long_search() {
        let mut cfg = settings(Path::new("."));
        cfg.stall = Duration::from_secs(5);
        let t0 = Instant::now();
        let mut dog = Watchdog::new(t0);
        let busy = StateSample::default();
        for s in 0..5 {
            assert!(dog
                .check(t0 + Duration::from_secs(s), &busy, &cfg, &None)
                .is_empty());
        }
        let fired = dog.check(t0 + Duration::from_secs(5), &busy, &cfg, &None);
        assert_eq!(fired[0].kind, kind::LOCK_HELD);

        let long = Some((7, Duration::from_secs(6), "regex".to_string()));
        let fired = dog.check(t0 + Duration::from_secs(6), &sample(0, 0), &cfg, &long);
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].kind, kind::SEARCH_STUCK);
        // The same search is reported once.
        assert!(dog
            .check(t0 + Duration::from_secs(7), &sample(0, 0), &cfg, &long)
            .is_empty());
    }

    #[test]
    fn captures_are_rate_limited_capped_and_readable() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = settings(dir.path());
        s.max_files = 3;
        s.min_interval = Duration::from_secs(3600);
        let r = Recorder::new(s);
        lock(&LOG_RING).push_back("INFO Indexed src/secret/a.rs".into());
        let trigger = |k: &'static str| {
            Trigger::new(
                k,
                "test".into(),
                Duration::from_secs(9),
                Duration::from_secs(5),
            )
        };

        let first = r.capture(trigger(kind::SLOW_SEARCH)).unwrap();
        assert!(
            r.capture(trigger(kind::SLOW_SEARCH)).is_none(),
            "rate limit"
        );
        for k in [kind::LOCK_WAIT, kind::BACKLOG, kind::LOCK_HELD] {
            std::thread::sleep(Duration::from_millis(5));
            r.capture(trigger(k)).unwrap();
        }
        let files = capture_files(dir.path());
        assert_eq!(files.len(), 3, "{files:?}");
        assert!(!first.exists(), "the oldest went first");

        let (caps, bad) = load_captures(dir.path());
        assert!(bad.is_empty(), "{bad:?}");
        assert_eq!(caps.len(), 3);
        let held = &caps[2].1;
        assert_eq!(held.trigger.kind, kind::LOCK_HELD);
        assert!(held.redacted);
        assert!(held.threads.is_some(), "a hang lists the threads");
        assert!(
            held.log.iter().all(|l| !l.contains("secret")),
            "{:?}",
            held.log
        );
        let text = summarize(dir.path(), &caps, &bad);
        assert!(text.contains("3 captures"), "{text}");
        assert!(text.contains("lock_held"), "{text}");
    }

    /// `fcs diagnose` names each thread's innermost fcs frame.
    #[test]
    fn thread_summary_names_where_each_thread_is() {
        let report = ThreadsReport {
            threads: vec![ThreadInfo {
                tid: 1,
                name: "fcs-watch-0".into(),
                state: "S".into(),
                wchan: String::new(),
            }],
            stacks: Some(
                "Thread 2 (Thread 0x7f (LWP 12) \"fcs-watch-0\"):\n\
                 #0  0x1 in syscall ()\n\
                 #1  0x2 in std::sys::sync::rwlock::futex::RwLock::write_contended ()\n\
                 #2  0x3 in fast_code_search::search::engine_lock::write ()\n\
                 #3  0x4 in fast_code_search::search::watch_loop::install ()\n\
                 \nThread 1 (Thread 0x7e (LWP 11) \"tokio-runtime-w\"):\n\
                 #0  0x5 in epoll_wait ()\n"
                    .into(),
            ),
            ..Default::default()
        };
        let lines = thread_summary(&report);
        assert_eq!(
            lines,
            [
                "threads: 1 (0 running, 0 waiting on the disk)",
                "  fcs-watch-0: fast_code_search::search::engine_lock::write",
            ]
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn threads_are_listed_by_name() {
        let report = thread_report(false);
        assert!(!report.threads.is_empty());
        assert!(report.threads.iter().all(|t| !t.state.is_empty()));
    }
}
