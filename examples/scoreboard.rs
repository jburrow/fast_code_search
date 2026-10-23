//! The scale scoreboard: the real server binary, over HTTP, under named
//! load scenarios, with the same numbers reported for each.
//!
//! `watch_load` drives the engine in-process; this starts
//! `fast_code_search_server` on a generated tree (see
//! `examples/support/load_tree.rs`), searches it over HTTP from several
//! threads the way people do, changes the tree the way a branch switch, a
//! formatter or a build does, and reports per scenario:
//!
//! - searches answered, turned away (503), refused while busy (429), not
//!   connected after the server first answered, and other errors;
//! - search latency (p50, p99, max), as the client sees it;
//! - for scenarios that change files: how long until the index matches the
//!   disk again after the changes stop, and the largest backlog seen;
//! - for start-up scenarios: how long until the server answers at all, and
//!   until it is ready;
//! - peak memory and CPU time of the server process (Linux);
//! - diagnostic captures written (`[diagnostics] capture = true`), crashes,
//!   and the problems `GET /api/check?all=true` finds at the end.
//!
//! ```text
//! cargo build --release --bins --example scoreboard
//! target/release/examples/scoreboard --dir /tmp/fcs-score --files 200000 \
//!     [--scenarios cold-build,branch-switch] [--stream-seconds 600] \
//!     [--targets benches/scoreboard/targets.toml] [--markdown OUT] [--json OUT]
//! ```
//!
//! With `--targets`, a number over its target fails the run (exit code 1).
//! The targets and the measurements behind them are on the book's
//! "Scale scoreboard" page. The report holds numbers only, never paths or
//! queries, so a run on a private tree is safe to share.

#[path = "support/load_tree.rs"]
mod load_tree;

use anyhow::{bail, Context, Result};
use clap::Parser;
use fast_code_search::search::integrity::IntegrityReport;
use load_tree::{generate, path_of, script, MIXED};
use rayon::prelude::*;
use serde::Serialize;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SCENARIOS: &[&str] = &[
    "cold-build",
    "warm-restart",
    "branch-switch",
    "stream",
    "slow-mix",
    "save-under-load",
    "overflow",
];

#[derive(Parser, Debug)]
struct Args {
    /// Where the generated tree and the run's files live.
    #[arg(long)]
    dir: PathBuf,
    /// Number of generated scripts.
    #[arg(long, default_value_t = 200_000)]
    files: usize,
    /// Comma-separated scenarios (default: all, in order).
    #[arg(long, value_delimiter = ',')]
    scenarios: Vec<String>,
    /// The server binary (default: next to this example's folder).
    #[arg(long)]
    server: Option<PathBuf>,
    /// Threads searching back to back.
    #[arg(long, default_value_t = 4)]
    searchers: usize,
    /// How long the `stream` scenario changes files.
    #[arg(long, default_value_t = 600)]
    stream_seconds: u64,
    /// Files changed per second in `stream`.
    #[arg(long, default_value_t = 200)]
    stream_rate: usize,
    /// How long `slow-mix` and `save-under-load` run.
    #[arg(long, default_value_t = 120)]
    mix_seconds: u64,
    /// After changes stop, how long the index may take to match the disk
    /// before the scenario is reported as not settled.
    #[arg(long, default_value_t = 600)]
    settle_seconds: u64,
    /// Fail when a number is over its target in this file.
    #[arg(long)]
    targets: Option<PathBuf>,
    /// Write the scoreboard as a Markdown table here.
    #[arg(long)]
    markdown: Option<PathBuf>,
    /// Write every number (and the backlog curves) as JSON here.
    #[arg(long)]
    json: Option<PathBuf>,
}

/// One scenario's numbers. Everything a target can name is a field here.
#[derive(Debug, Default, Clone, Serialize)]
struct Score {
    scenario: String,
    files: usize,
    searches: u64,
    answered: u64,
    /// Answered while the index was still being built or loaded (the
    /// answer says so).
    incomplete: u64,
    /// 503: "index is being updated", or not ready.
    turned_away: u64,
    /// 429: too many searches at once.
    busy: u64,
    /// No connection after the server had answered once.
    refused: u64,
    errors: u64,
    p50_ms: f64,
    p99_ms: f64,
    max_ms: f64,
    /// Seconds from start until the server answered any request.
    #[serde(skip_serializing_if = "Option::is_none")]
    listen_secs: Option<f64>,
    /// Seconds from start until `/api/ready` said ready.
    #[serde(skip_serializing_if = "Option::is_none")]
    ready_secs: Option<f64>,
    /// Files the scenario changed.
    #[serde(skip_serializing_if = "Option::is_none")]
    changed: Option<usize>,
    /// Seconds from the last change until the index matched the disk
    /// (`None` with `changed` set: it did not within `--settle-seconds`).
    #[serde(skip_serializing_if = "Option::is_none")]
    fresh_secs: Option<f64>,
    /// The scenario changed files and the index never caught up.
    not_settled: u64,
    /// The most changes the server reported waiting to be applied.
    #[serde(skip_serializing_if = "Option::is_none")]
    max_pending: Option<usize>,
    /// "The watched paths will be rescanned" lines in the server log.
    rescans: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    peak_rss_mb: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cpu_secs: Option<f64>,
    captures: u64,
    /// Captures other than `slow_search`: a stall, a long lock wait or
    /// hold, a backlog. A slow search alone can be the machine's CPUs
    /// being shared by too many searches (`slow-mix` does that on purpose).
    stall_captures: u64,
    /// Each capture's kind and the number that set it off.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    capture_kinds: Vec<String>,
    crashes: u64,
    /// Problems `GET /api/check?all=true` found once the index had settled.
    check_problems: u64,
    elapsed_secs: f64,
    /// (seconds since the scenario started, changes pending), every second.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    backlog: Vec<(f64, usize)>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let scenarios: Vec<String> = if args.scenarios.is_empty() {
        SCENARIOS.iter().map(|s| s.to_string()).collect()
    } else {
        args.scenarios.clone()
    };
    for s in &scenarios {
        if !SCENARIOS.contains(&s.as_str()) {
            bail!("unknown scenario {s:?}; known: {}", SCENARIOS.join(", "));
        }
    }
    let server = match &args.server {
        Some(p) => p.clone(),
        None => {
            let exe = std::env::current_exe()?;
            let bin_dir = exe.parent().and_then(Path::parent).context("exe path")?;
            bin_dir.join(format!(
                "fast_code_search_server{}",
                std::env::consts::EXE_SUFFIX
            ))
        }
    };
    if !server.exists() {
        bail!(
            "no server binary at {} (cargo build --release --bins, or --server)",
            server.display()
        );
    }
    std::fs::create_dir_all(&args.dir)?;
    let tree = args.dir.join("tree");
    generate(&tree, args.files);
    let tree = tree.canonicalize()?;
    remove_created_files(&tree);
    let run = Run {
        args: &args,
        server,
        tree,
        work: args.dir.canonicalize()?.join("run"),
        rev: AtomicU64::new(unix_secs() * 1000),
    };

    let mut scores = Vec::new();
    for name in &scenarios {
        eprintln!("== {name}");
        let score = run.scenario(name)?;
        eprintln!("{}", render_markdown(std::slice::from_ref(&score)));
        scores.push(score);
    }
    let table = render_markdown(&scores);
    println!("{table}");
    if let Some(p) = &args.markdown {
        std::fs::write(p, &table)?;
    }
    if let Some(p) = &args.json {
        std::fs::write(p, serde_json::to_string_pretty(&scores)?)?;
    }
    if let Some(p) = &args.targets {
        let misses = check_targets(&scores, p, args.files)?;
        if !misses.is_empty() {
            for m in &misses {
                eprintln!("OVER TARGET: {m}");
            }
            if let Some(md) = &args.markdown {
                let mut s = std::fs::read_to_string(md)?;
                s.push_str("\n**Over target:**\n\n");
                for m in &misses {
                    let _ = writeln!(s, "- {m}");
                }
                std::fs::write(md, s)?;
            }
            std::process::exit(1);
        }
        eprintln!("every number within its target");
    }
    Ok(())
}

fn unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Files `stream` created in an earlier run, so every run starts from the
/// generated tree (rewritten revisions aside).
fn remove_created_files(tree: &Path) {
    for dir in std::fs::read_dir(tree).into_iter().flatten().flatten() {
        for f in std::fs::read_dir(dir.path())
            .into_iter()
            .flatten()
            .flatten()
        {
            if f.file_name().to_string_lossy().starts_with("new_") {
                let _ = std::fs::remove_file(f.path());
            }
        }
    }
}

struct Run<'a> {
    args: &'a Args,
    server: PathBuf,
    tree: PathBuf,
    work: PathBuf,
    /// The next revision written; unique across runs.
    rev: AtomicU64,
}

/// What a scenario does to the server's configuration.
#[derive(Default)]
struct Setup {
    /// Delete the saved index first.
    cold: bool,
    save_after_updates: usize,
}

impl Run<'_> {
    fn scenario(&self, name: &str) -> Result<Score> {
        let started = Instant::now();
        let mut score = Score {
            scenario: name.to_string(),
            files: self.args.files,
            ..Default::default()
        };
        if name != "cold-build" && !self.work.join("index.bin").exists() {
            // Every other scenario starts from a saved index.
            eprintln!("building the index first");
            let s = self.start(&Setup {
                cold: true,
                ..Default::default()
            })?;
            s.wait_settled(Duration::from_secs(3600))?;
            s.stop()?;
        }
        let setup = Setup {
            cold: name == "cold-build",
            save_after_updates: if name == "save-under-load" { 5_000 } else { 0 },
        };
        let server = self.start(&setup)?;
        let searchers = if name == "slow-mix" {
            self.args.searchers * 2
        } else {
            self.args.searchers
        };
        let load = Load::start(&server.base, searchers, name == "slow-mix");
        let sampler = Sampler::start(server.pid);

        match name {
            "cold-build" | "warm-restart" => {
                let (listen, ready) = server.wait_listening_and_ready(Duration::from_secs(3600))?;
                score.listen_secs = Some(listen.as_secs_f64());
                score.ready_secs = Some(ready.as_secs_f64());
                std::thread::sleep(Duration::from_secs(5));
            }
            _ => {
                // The restart's own check of the tree runs first: changes
                // that overlap it are another scenario (see the book page).
                server.wait_settled(Duration::from_secs(3600))?;
                std::thread::sleep(Duration::from_secs(3));
                let t0 = Instant::now();
                let backlog = Backlog::start(&server.base, t0);
                let sample = match name {
                    "branch-switch" => self.rewrite_share(5),
                    "overflow" => self.rewrite_share(2),
                    "stream" => self.stream(
                        self.args.stream_rate,
                        Duration::from_secs(self.args.stream_seconds),
                    ),
                    "slow-mix" => self.stream(50, Duration::from_secs(self.args.mix_seconds)),
                    "save-under-load" => {
                        self.stream(500, Duration::from_secs(self.args.mix_seconds))
                    }
                    _ => unreachable!(),
                };
                score.changed = Some(sample.changed);
                let fresh =
                    server.wait_fresh(&sample.paths, Duration::from_secs(self.args.settle_seconds));
                score.fresh_secs = fresh.map(|d| d.as_secs_f64());
                score.not_settled = u64::from(fresh.is_none());
                // A few more seconds of searching on the settled index.
                std::thread::sleep(Duration::from_secs(2));
                let curve = backlog.stop();
                score.max_pending = curve.iter().map(|&(_, n)| n).max();
                score.backlog = curve;
            }
        }

        let outcome = load.stop();
        score.searches = outcome.searches;
        score.answered = outcome.answered;
        score.incomplete = outcome.incomplete;
        score.turned_away = outcome.turned_away;
        score.busy = outcome.busy;
        score.refused = outcome.refused;
        score.errors = outcome.errors;
        score.p50_ms = outcome.percentile_ms(0.50);
        score.p99_ms = outcome.percentile_ms(0.99);
        score.max_ms = outcome.percentile_ms(1.0);

        score.check_problems = match server.check() {
            Ok(r) => {
                if r.problems() > 0 {
                    eprintln!("check:\n{}", r.render(false));
                }
                r.problems() as u64
            }
            Err(e) => {
                eprintln!("check failed: {e:#}");
                1
            }
        };
        let resources = sampler.stop();
        score.peak_rss_mb = resources.peak_rss_mb;
        score.cpu_secs = resources.cpu_secs;
        let crashed = server.crashed();
        score.crashes = u64::from(crashed);
        if !crashed {
            server.stop()?;
        }
        let log = std::fs::read_to_string(self.work.join("server.log")).unwrap_or_default();
        score.rescans = log.matches("will be rescanned").count() as u64;
        let _ = std::fs::rename(
            self.work.join("server.log"),
            self.work.join(format!("server-{name}.log")),
        );
        // Captures are written on threads of their own; the server has
        // stopped, so they are all on disk.
        let (captures, unreadable) =
            fast_code_search::diagnostics::capture::load_captures(&self.work.join("captures"));
        score.captures = (captures.len() + unreadable.len()) as u64;
        score.stall_captures = unreadable.len() as u64
            + captures
                .iter()
                .filter(|(_, c)| c.trigger.kind != "slow_search")
                .count() as u64;
        score.capture_kinds = captures
            .iter()
            .map(|(_, c)| format!("{} {} ms", c.trigger.kind, c.trigger.observed_ms))
            .collect();
        if !captures.is_empty() {
            // Kept for a look: the next scenario starts with an empty folder.
            let keep = self.work.join(format!("captures-{name}"));
            let _ = std::fs::remove_dir_all(&keep);
            let _ = std::fs::rename(self.work.join("captures"), &keep);
        }
        score.elapsed_secs = started.elapsed().as_secs_f64();
        Ok(score)
    }

    /// Start the server on a fresh port with captures on.
    fn start(&self, setup: &Setup) -> Result<Server> {
        std::fs::create_dir_all(&self.work)?;
        let index = self.work.join("index.bin");
        if setup.cold {
            let _ = std::fs::remove_file(&index);
        }
        let captures = self.work.join("captures");
        let _ = std::fs::remove_dir_all(&captures);
        std::fs::create_dir_all(&captures)?;
        let port = free_port()?;
        let config = format!(
            "[server]\nenable_grpc = false\nweb_address = \"127.0.0.1:{port}\"\n\n\
             [indexer]\npaths = [{tree:?}]\nindex_path = {index:?}\nwatch = true\n\
             save_after_updates = {save}\n\n\
             [diagnostics]\ncapture = true\ndir = {captures:?}\n",
            tree = self.tree.to_string_lossy(),
            index = index.to_string_lossy(),
            captures = captures.to_string_lossy(),
            save = setup.save_after_updates,
        );
        let config_path = self.work.join("config.toml");
        std::fs::write(&config_path, config)?;
        let log = std::fs::File::create(self.work.join("server.log"))?;
        let child = Command::new(&self.server)
            .arg("--config")
            .arg(&config_path)
            .current_dir(&self.work)
            .env("RUST_LOG", "info")
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()
            .with_context(|| format!("starting {}", self.server.display()))?;
        Ok(Server {
            pid: child.id(),
            child: Mutex::new(child),
            base: format!("http://127.0.0.1:{port}"),
            started: Instant::now(),
            http: client(Duration::from_secs(600)),
        })
    }

    fn next_rev(&self) -> u64 {
        self.rev.fetch_add(1, Ordering::Relaxed)
    }

    /// Rewrite one file in `every` at once, as a branch switch does.
    fn rewrite_share(&self, every: usize) -> Changed {
        let rev = self.next_rev();
        let picked: Vec<usize> = (0..self.args.files)
            .filter(|i| (i + rev as usize).is_multiple_of(every))
            .collect();
        picked.par_iter().for_each(|&i| {
            let _ = std::fs::write(path_of(&self.tree, i), script(i, rev));
        });
        Changed {
            changed: picked.len(),
            paths: sample(&picked, 100)
                .into_iter()
                .map(|i| path_of(&self.tree, i))
                .collect(),
        }
    }

    /// Change `rate` files a second for `how_long`, in ticks of 100 ms:
    /// rewrites, plus one new file and the removal of an older new file
    /// each tick.
    fn stream(&self, rate: usize, how_long: Duration) -> Changed {
        let t = Instant::now();
        let per_tick = (rate / 10).max(1);
        let mut changed = 0;
        let mut last = Vec::new();
        let mut created = std::collections::VecDeque::new();
        let mut tick = 0usize;
        while t.elapsed() < how_long {
            let rev = self.next_rev();
            last.clear();
            for k in 0..per_tick {
                let i =
                    (rev as usize).wrapping_mul(104_729).wrapping_add(k * 7_919) % self.args.files;
                if std::fs::write(path_of(&self.tree, i), script(i, rev)).is_ok() {
                    changed += 1;
                    last.push(path_of(&self.tree, i));
                }
            }
            let new = path_of(&self.tree, tick * 997 % self.args.files)
                .with_file_name(format!("new_{rev}.py"));
            if std::fs::write(&new, script(tick, rev)).is_ok() {
                changed += 1;
                created.push_back(new.clone());
                last.push(new);
            }
            if created.len() > 50 {
                if let Some(old) = created.pop_front() {
                    let _ = std::fs::remove_file(old);
                    changed += 1;
                }
            }
            tick += 1;
            let next = Duration::from_millis(100) * tick as u32;
            if let Some(wait) = next.checked_sub(t.elapsed()) {
                std::thread::sleep(wait);
            }
        }
        Changed {
            changed,
            paths: last.into_iter().take(100).collect(),
        }
    }
}

/// Files a scenario changed, and some of the last ones to watch for.
struct Changed {
    changed: usize,
    paths: Vec<PathBuf>,
}

fn sample(items: &[usize], n: usize) -> Vec<usize> {
    let step = (items.len() / n).max(1);
    items.iter().step_by(step).take(n).copied().collect()
}

fn free_port() -> Result<u16> {
    let l = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(l.local_addr()?.port())
}

fn client(timeout: Duration) -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(5))
        .build()
        .unwrap()
}

struct Server {
    pid: u32,
    child: Mutex<Child>,
    base: String,
    started: Instant,
    http: reqwest::blocking::Client,
}

impl Server {
    fn get(&self, path: &str) -> Result<reqwest::blocking::Response> {
        Ok(self.http.get(format!("{}{path}", self.base)).send()?)
    }

    fn crashed(&self) -> bool {
        matches!(self.child.lock().unwrap().try_wait(), Ok(Some(_)))
    }

    /// (until it answered anything, until it was ready), from the start.
    fn wait_listening_and_ready(&self, limit: Duration) -> Result<(Duration, Duration)> {
        let mut listening = None;
        loop {
            if self.crashed() {
                bail!("the server exited while starting");
            }
            if let Ok(r) = self.get("/api/ready") {
                let now = self.started.elapsed();
                listening.get_or_insert(now);
                if r.status().is_success() {
                    return Ok((listening.unwrap(), now));
                }
            }
            if self.started.elapsed() > limit {
                bail!("the server was not ready in {limit:?}");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn wait_ready(&self, limit: Duration) -> Result<()> {
        self.wait_listening_and_ready(limit).map(|_| ())
    }

    /// Ready, and the build or the restart's check of the tree finished.
    fn wait_settled(&self, limit: Duration) -> Result<()> {
        self.wait_ready(limit)?;
        while self.started.elapsed() < limit {
            let indexing = self
                .get("/api/status")
                .ok()
                .filter(|r| r.status().is_success())
                .and_then(|r| r.json::<serde_json::Value>().ok())
                .map(|v| v["is_indexing"].as_bool().unwrap_or(true));
            if indexing == Some(false) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        bail!("the server was still indexing after {limit:?}")
    }

    /// Seconds until no change is pending and every path in `paths` is
    /// indexed as it is on disk, polled every 250 ms.
    fn wait_fresh(&self, paths: &[PathBuf], limit: Duration) -> Option<Duration> {
        let t = Instant::now();
        let wanted: Vec<String> = paths
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        let query = wanted.join(";");
        while t.elapsed() < limit {
            let fresh = self
                .http
                .get(format!("{}/api/status", self.base))
                .query(&[("paths", query.as_str())])
                .send()
                .ok()
                .filter(|r| r.status().is_success())
                .and_then(|r| r.json::<serde_json::Value>().ok())
                .is_some_and(|v| {
                    let index = &v["index"];
                    let pending = index["pending_changes"].as_u64().unwrap_or(0);
                    let current = v["paths"].as_array().is_some_and(|ps| {
                        ps.len() == wanted.len()
                            && ps.iter().all(|p| {
                                let state = p["state"].as_str().unwrap_or("");
                                // A file a stream removed again is fine.
                                state == "current" || state == "not_found"
                            })
                    });
                    !index.is_null() && pending == 0 && current
                });
            if fresh {
                return Some(t.elapsed());
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        None
    }

    fn check(&self) -> Result<IntegrityReport> {
        // One check at a time; the scrub's own does not use this lock, but
        // be patient with a 429 anyway.
        for _ in 0..30 {
            let r = self.get("/api/check?all=true&examples=0")?;
            if r.status().as_u16() == 429 {
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
            if !r.status().is_success() {
                bail!("check answered {}", r.status());
            }
            return Ok(r.json()?);
        }
        bail!("a check was always running")
    }

    /// SIGTERM (a graceful stop that saves the index), then wait.
    fn stop(&self) -> Result<()> {
        let mut child = self.child.lock().unwrap();
        #[cfg(unix)]
        unsafe {
            libc::kill(self.pid as i32, libc::SIGTERM);
        }
        #[cfg(not(unix))]
        let _ = child.kill();
        let t = Instant::now();
        loop {
            if let Some(status) = child.try_wait()? {
                if !status.success() {
                    eprintln!("the server exited with {status}");
                }
                return Ok(());
            }
            if t.elapsed() > Duration::from_secs(300) {
                let _ = child.kill();
                bail!("the server did not stop within 300 s of SIGTERM");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Ok(mut c) = self.child.lock() {
            if matches!(c.try_wait(), Ok(None)) {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
    }
}

/// Threads searching back to back over HTTP, with `MIXED` queries, plus
/// (with `sweeper`) one regex that reads every file, every 3 s.
struct Load {
    stop: Arc<AtomicBool>,
    threads: Vec<std::thread::JoinHandle<()>>,
    outcome: Arc<Mutex<Outcome>>,
}

#[derive(Default)]
struct Outcome {
    searches: u64,
    answered: u64,
    incomplete: u64,
    turned_away: u64,
    busy: u64,
    refused: u64,
    errors: u64,
    latencies: Vec<Duration>,
}

impl Outcome {
    fn percentile_ms(&self, p: f64) -> f64 {
        let mut l = self.latencies.clone();
        if l.is_empty() {
            return 0.0;
        }
        l.sort();
        l[((l.len() - 1) as f64 * p).round() as usize].as_secs_f64() * 1000.0
    }
}

impl Load {
    fn start(base: &str, searchers: usize, sweeper: bool) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let outcome = Arc::new(Mutex::new(Outcome::default()));
        // Connection errors count as refused only once the server has
        // answered (before that it is starting; see `listen_secs`).
        let answered_once = Arc::new(AtomicBool::new(false));
        let mut threads = Vec::new();
        for t in 0..searchers + usize::from(sweeper) {
            let is_sweeper = t == searchers;
            let (stop, outcome, answered_once, base) = (
                stop.clone(),
                outcome.clone(),
                answered_once.clone(),
                base.to_string(),
            );
            threads.push(std::thread::spawn(move || {
                let http = client(Duration::from_secs(60));
                let mut n = t;
                while !stop.load(Ordering::Relaxed) {
                    n += 1;
                    let (regex, text) = if is_sweeper {
                        // No trigram to narrow it down and no match: every
                        // file is read.
                        (true, r"[qxz]{4}\d")
                    } else {
                        MIXED[n % MIXED.len()]
                    };
                    let t0 = Instant::now();
                    let r = http
                        .get(format!("{base}/api/search"))
                        .query(&[("q", text), ("max", "50"), ("regex", &regex.to_string())])
                        .send()
                        .and_then(|r| {
                            let status = r.status();
                            r.bytes().map(|body| (status, incomplete(&body)))
                        });
                    let took = t0.elapsed();
                    let answered = matches!(&r, Ok((s, false)) if s.as_u16() == 200);
                    let mut o = outcome.lock().unwrap();
                    match r {
                        Ok((status, incomplete)) => {
                            answered_once.store(true, Ordering::Relaxed);
                            o.searches += 1;
                            match status.as_u16() {
                                200 => {
                                    o.answered += 1;
                                    o.incomplete += u64::from(incomplete);
                                    o.latencies.push(took);
                                }
                                503 => o.turned_away += 1,
                                429 => o.busy += 1,
                                _ => o.errors += 1,
                            }
                        }
                        Err(e) if e.is_connect() => {
                            if answered_once.load(Ordering::Relaxed) {
                                o.searches += 1;
                                o.refused += 1;
                            }
                        }
                        Err(_) => {
                            o.searches += 1;
                            o.errors += 1;
                        }
                    }
                    drop(o);
                    if is_sweeper {
                        std::thread::sleep(Duration::from_secs(3));
                    } else if !answered {
                        // A client retrying after a short pause (also after
                        // an answer from an index still loading, which can
                        // come back in a fraction of a millisecond).
                        std::thread::sleep(Duration::from_millis(250));
                    }
                }
            }));
        }
        Self {
            stop,
            threads,
            outcome,
        }
    }

    fn stop(self) -> Outcome {
        self.stop.store(true, Ordering::Relaxed);
        for t in self.threads {
            let _ = t.join();
        }
        std::mem::take(&mut *self.outcome.lock().unwrap())
    }
}

/// Whether a search answer says the index was still being built or loaded
/// (its `indexing` limit): what it found is real, but not everything.
fn incomplete(body: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            v["limits_hit"]
                .as_array()
                .map(|ls| ls.iter().any(|l| l["limit"] == "indexing"))
        })
        .unwrap_or(false)
}

/// The server's pending changes, every second.
struct Backlog {
    stop: Arc<AtomicBool>,
    thread: std::thread::JoinHandle<Vec<(f64, usize)>>,
}

impl Backlog {
    fn start(base: &str, t0: Instant) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let (stop2, base) = (stop.clone(), base.to_string());
        let thread = std::thread::spawn(move || {
            let http = client(Duration::from_secs(10));
            let mut curve = Vec::new();
            while !stop2.load(Ordering::Relaxed) {
                if let Some(v) = http
                    .get(format!("{base}/api/status"))
                    .send()
                    .ok()
                    .filter(|r| r.status().is_success())
                    .and_then(|r| r.json::<serde_json::Value>().ok())
                {
                    let pending = v["index"]["pending_changes"].as_u64().unwrap_or(0);
                    curve.push((t0.elapsed().as_secs_f64(), pending as usize));
                }
                std::thread::sleep(Duration::from_secs(1));
            }
            curve
        });
        Self { stop, thread }
    }

    fn stop(self) -> Vec<(f64, usize)> {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.join().unwrap_or_default()
    }
}

/// Peak memory and CPU time of the server process (Linux: from `/proc`).
struct Sampler {
    stop: Arc<AtomicBool>,
    thread: std::thread::JoinHandle<Resources>,
}

#[derive(Default)]
struct Resources {
    peak_rss_mb: Option<f64>,
    cpu_secs: Option<f64>,
}

impl Sampler {
    fn start(pid: u32) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        let thread = std::thread::spawn(move || {
            let mut last = Resources::default();
            while !stop2.load(Ordering::Relaxed) {
                if let Some(r) = read_resources(pid) {
                    last = r;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            read_resources(pid).unwrap_or(last)
        });
        Self { stop, thread }
    }

    fn stop(self) -> Resources {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.join().unwrap_or_default()
    }
}

#[cfg(target_os = "linux")]
fn read_resources(pid: u32) -> Option<Resources> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let hwm_kb: f64 = status
        .lines()
        .find(|l| l.starts_with("VmHWM:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // Fields after the command name (which may hold spaces): utime and
    // stime are the 12th and 13th.
    let rest = &stat[stat.rfind(')')? + 2..];
    let fields: Vec<&str> = rest.split_whitespace().collect();
    let ticks: f64 = fields.get(11)?.parse::<f64>().ok()? + fields.get(12)?.parse::<f64>().ok()?;
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as f64;
    Some(Resources {
        peak_rss_mb: Some(hwm_kb / 1024.0),
        cpu_secs: Some(ticks / hz),
    })
}

#[cfg(not(target_os = "linux"))]
fn read_resources(_pid: u32) -> Option<Resources> {
    None
}

/// Number formats for the table.
fn ms(v: f64) -> String {
    if v >= 1000.0 {
        format!("{:.2} s", v / 1000.0)
    } else {
        format!("{v:.0} ms")
    }
}

fn opt_secs(v: Option<f64>) -> String {
    v.map(|s| format!("{s:.1} s")).unwrap_or_else(|| "–".into())
}

fn render_markdown(scores: &[Score]) -> String {
    let mut out = String::new();
    let files = scores.first().map(|s| s.files).unwrap_or(0);
    let _ = writeln!(out, "Scoreboard, {files} files\n");
    out.push_str(
        "| Scenario | Searches | Incomplete | Turned away | Busy | Refused | Errors | p50 | p99 | Max | Listening | Ready | Changed | Fresh after | Max pending | Rescans | Peak RSS | CPU | Captures | Crashes | Check problems |\n",
    );
    out.push_str(
        "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for s in scores {
        let fresh = match (s.changed, s.fresh_secs) {
            (Some(_), None) => "not settled".to_string(),
            (_, f) => opt_secs(f),
        };
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            s.scenario,
            s.searches,
            s.incomplete,
            s.turned_away,
            s.busy,
            s.refused,
            s.errors,
            ms(s.p50_ms),
            ms(s.p99_ms),
            ms(s.max_ms),
            opt_secs(s.listen_secs),
            opt_secs(s.ready_secs),
            s.changed.map(|c| c.to_string()).unwrap_or_else(|| "–".into()),
            fresh,
            s.max_pending.map(|c| c.to_string()).unwrap_or_else(|| "–".into()),
            s.rescans,
            s.peak_rss_mb.map(|m| format!("{m:.0} MB")).unwrap_or_else(|| "–".into()),
            opt_secs(s.cpu_secs),
            s.captures,
            s.crashes,
            s.check_problems,
        );
    }
    out
}

/// Numbers over their targets. The file has `files = N`, an `[every]`
/// table that applies to all scenarios, and a table per scenario; each key
/// names a `Score` field and gives its largest allowed value. Targets that
/// depend on the tree's size (times, memory) apply only when the run has
/// `files = N`; the zero targets apply to every run.
fn check_targets(scores: &[Score], path: &Path, files: usize) -> Result<Vec<String>> {
    let text = std::fs::read_to_string(path)?;
    let doc: toml::Table = toml::from_str(&text)?;
    let sized = doc.get("files").and_then(|v| v.as_integer()) == Some(files as i64);
    if !sized {
        eprintln!(
            "note: {} holds targets for {:?} files; checking only the zero targets",
            path.display(),
            doc.get("files")
        );
    }
    let mut misses = Vec::new();
    for s in scores {
        let value = serde_json::to_value(s)?;
        let mut targets: Vec<(String, f64)> = Vec::new();
        for table in [doc.get("every"), doc.get(&s.scenario)]
            .into_iter()
            .flatten()
        {
            for (k, v) in table.as_table().into_iter().flatten() {
                let Some(limit) = v.as_float().or_else(|| v.as_integer().map(|i| i as f64)) else {
                    continue;
                };
                if limit == 0.0 || sized {
                    targets.retain(|(key, _)| key != k);
                    targets.push((k.clone(), limit));
                }
            }
        }
        for (k, limit) in targets {
            match value.get(&k).and_then(|v| v.as_f64()) {
                Some(v) if v > limit => {
                    misses.push(format!("{}: {k} = {v} (target {limit})", s.scenario));
                }
                Some(_) => {}
                None if k == "fresh_secs" && s.changed.is_some() => {
                    misses.push(format!("{}: the index never matched the disk", s.scenario));
                }
                None => {}
            }
        }
    }
    Ok(misses)
}
