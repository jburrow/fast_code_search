//! Chaos: `kill -9` the real server at random moments, restart it, and
//! check that the index comes back right.
//!
//! Each round starts `fast_code_search_server` on a generated tree (see
//! `examples/support/load_tree.rs`), does one of the things below, and
//! kills the process with SIGKILL at a random moment while it works:
//!
//! - `build`: a build from scratch, with checkpoint saves along the way;
//! - `load`: a start from the saved index, while it loads and checks the
//!   tree;
//! - `stream`: files rewritten, created and removed while the watcher
//!   applies them and saves every few hundred updates; the edits go on for
//!   a second after the kill, as they would while a server is down;
//! - `save`: the same stream, killed a moment (0 to 20 ms) after one of
//!   the periodic saves begins;
//! - `burst`: a tenth of the files rewritten at once, killed while the
//!   watcher installs them;
//! - `shutdown`: a graceful stop (SIGTERM), killed a moment after its save
//!   begins.
//!
//! Then it starts the server again, unharmed, and checks:
//!
//! - the saved index was loaded, or there was none: a kill must never leave
//!   an index the next start has to reject (the save writes a temp file,
//!   fsyncs it and renames it over the old one);
//! - no temp file of a killed save is left once the server has started;
//! - the server becomes ready, catches up with every change (nothing
//!   pending, and the changed files indexed as they are on disk), and
//!   `GET /api/check?all=true` finds nothing wrong: invariants, every
//!   file's content, and the file set against a fresh walk;
//! - it did not crash.
//!
//! ```text
//! cargo build --release --bins --example chaos
//! target/release/examples/chaos --dir /tmp/fcs-chaos --files 20000 --rounds 40 \
//!     [--seed N] [--markdown OUT] [--json OUT]
//! ```
//!
//! Any failed check fails the run (exit code 1). The report holds numbers
//! only. The seed is printed so a failing run can be repeated; the server
//! logs of a failing round are kept in `--dir`.

#[path = "support/load_tree.rs"]
mod load_tree;
#[path = "support/server.rs"]
mod server;

use anyhow::{bail, Context, Result};
use clap::Parser;
use load_tree::{generate, path_of, script};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::Serialize;
use server::{free_port, Server};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const KINDS: &[&str] = &["build", "load", "stream", "save", "burst", "shutdown"];

#[derive(Parser, Debug)]
#[command(about = "kill -9 the server at random moments and check it recovers")]
struct Args {
    /// Working directory: the generated tree, the index and the logs.
    #[arg(long, default_value = "/tmp/fcs-chaos")]
    dir: PathBuf,
    /// Files in the generated tree.
    #[arg(long, default_value_t = 20_000)]
    files: usize,
    /// Rounds (each one kill and one check), taking the kinds in turn.
    #[arg(long, default_value_t = 40)]
    rounds: usize,
    /// Seed for the kill moments (default: the time).
    #[arg(long)]
    seed: Option<u64>,
    /// Only these kinds (comma separated).
    #[arg(long, value_delimiter = ',')]
    kinds: Vec<String>,
    /// The server binary (default: next to this example's folder).
    #[arg(long)]
    server: Option<PathBuf>,
    /// Files changed per second in `stream`.
    #[arg(long, default_value_t = 300)]
    stream_rate: usize,
    /// Save the index after this many watcher updates.
    #[arg(long, default_value_t = 300)]
    save_after_updates: usize,
    /// Give up on a restart catching up after this long.
    #[arg(long, default_value_t = 600)]
    settle_seconds: u64,
    #[arg(long)]
    markdown: Option<PathBuf>,
    #[arg(long)]
    json: Option<PathBuf>,
}

/// One round: what was killed when, and what the restart found.
#[derive(Debug, Default, Clone, Serialize)]
struct Round {
    round: usize,
    kind: String,
    /// From the start of the server (or of the shutdown) to the kill.
    kill_after_ms: u64,
    /// A save's temp file was there when the process died.
    killed_mid_save: bool,
    /// The index file was there after the kill.
    index_left: bool,
    /// Files changed since the last check.
    changed: usize,
    /// The restart loaded the saved index ("loaded"), had none ("none"),
    /// or rejected it ("rejected").
    load: String,
    /// Temp files still there once the restarted server answered.
    temp_files_after_start: usize,
    ready_secs: f64,
    /// From ready until the index matched the disk.
    fresh_secs: Option<f64>,
    not_settled: bool,
    check_problems: usize,
    crashed: bool,
    failures: Vec<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let kinds: Vec<String> = if args.kinds.is_empty() {
        KINDS.iter().map(|s| s.to_string()).collect()
    } else {
        args.kinds.clone()
    };
    for k in &kinds {
        if !KINDS.contains(&k.as_str()) {
            bail!("unknown kind {k:?}; known: {}", KINDS.join(", "));
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
    let seed = args.seed.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    });
    eprintln!("seed {seed}");
    std::fs::create_dir_all(&args.dir)?;
    let tree = args.dir.join("tree");
    generate(&tree, args.files);
    let tree = tree.canonicalize()?;
    let work = args.dir.canonicalize()?.join("run");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work)?;
    let chaos = Chaos {
        args: &args,
        server,
        tree,
        work,
        rev: AtomicU64::new(1),
        created: Mutex::new(Vec::new()),
        changed: Mutex::new(Vec::new()),
    };
    chaos.remove_created_files();

    // A clean build first, to time it and have an index to start from.
    let t = Instant::now();
    let s = chaos.start(true)?;
    s.wait_settled(Duration::from_secs(3600))?;
    s.stop()?;
    let build = t.elapsed();
    let t = Instant::now();
    let s = chaos.start(false)?;
    s.wait_settled(Duration::from_secs(3600))?;
    let restart = t.elapsed();
    s.stop()?;
    eprintln!("build {build:.1?}, restart {restart:.1?}");

    let mut rng = StdRng::seed_from_u64(seed);
    let mut rounds = Vec::new();
    for round in 0..args.rounds {
        let kind = &kinds[round % kinds.len()];
        let mut r = Round {
            round,
            kind: kind.clone(),
            ..Default::default()
        };
        chaos
            .kill_round(&mut r, &mut rng, build, restart)
            .with_context(|| format!("round {round} ({kind})"))?;
        chaos
            .check_round(&mut r)
            .with_context(|| format!("checking round {round} ({kind})"))?;
        eprintln!(
            "round {round:>3} {kind:<8} killed at {:>6} ms{}  load {:<8} ready {:>5.1} s  fresh {}  problems {}{}",
            r.kill_after_ms,
            if r.killed_mid_save { " (mid save)" } else { "" },
            r.load,
            r.ready_secs,
            r.fresh_secs.map_or("-".into(), |f| format!("{f:.1} s")),
            r.check_problems,
            if r.failures.is_empty() {
                String::new()
            } else {
                format!("  FAILED: {}", r.failures.join("; "))
            }
        );
        rounds.push(r);
    }

    let table = render_markdown(&rounds, seed, args.files);
    println!("{table}");
    if let Some(p) = &args.markdown {
        std::fs::write(p, &table)?;
    }
    if let Some(p) = &args.json {
        std::fs::write(p, serde_json::to_string_pretty(&rounds)?)?;
    }
    let failed: Vec<&Round> = rounds.iter().filter(|r| !r.failures.is_empty()).collect();
    if !failed.is_empty() {
        for r in failed {
            eprintln!(
                "FAILED round {} ({}): {}",
                r.round,
                r.kind,
                r.failures.join("; ")
            );
        }
        eprintln!("seed {seed}");
        std::process::exit(1);
    }
    eprintln!("every round recovered");
    Ok(())
}

struct Chaos<'a> {
    args: &'a Args,
    server: PathBuf,
    tree: PathBuf,
    work: PathBuf,
    /// The next revision written.
    rev: AtomicU64,
    /// Files created by streams and not removed yet.
    created: Mutex<Vec<PathBuf>>,
    /// Files changed since the last check.
    changed: Mutex<Vec<PathBuf>>,
}

impl Chaos<'_> {
    fn index(&self) -> PathBuf {
        self.work.join("index.bin")
    }

    fn start(&self, cold: bool) -> Result<Server> {
        if cold {
            let _ = std::fs::remove_file(self.index());
        }
        let port = free_port()?;
        let config = format!(
            "[server]\nenable_grpc = false\nweb_address = \"127.0.0.1:{port}\"\n\n\
             [indexer]\npaths = [{tree:?}]\nindex_path = {index:?}\nwatch = true\n\
             save_after_updates = {save}\ncheckpoint_interval_files = {checkpoint}\n",
            tree = self.tree.to_string_lossy(),
            index = self.index().to_string_lossy(),
            save = self.args.save_after_updates,
            checkpoint = (self.args.files / 4).max(1),
        );
        Server::spawn(
            &self.server,
            &self.work,
            port,
            &config,
            &self.work.join("server.log"),
        )
    }

    /// Temp files of index saves in the work directory.
    fn temp_files(&self) -> usize {
        std::fs::read_dir(&self.work)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("index.bin.tmp.")
            })
            .count()
    }

    /// Start the server, do what `r.kind` says, and kill it at a random
    /// moment.
    fn kill_round(
        &self,
        r: &mut Round,
        rng: &mut StdRng,
        build: Duration,
        restart: Duration,
    ) -> Result<()> {
        let at = |rng: &mut StdRng, max: Duration| {
            Duration::from_millis(rng.random_range(0..max.as_millis().max(1) as u64))
        };
        match r.kind.as_str() {
            "build" => {
                let s = self.start(true)?;
                let wait = at(rng, build);
                self.kill_after(r, &s, s.started, wait)?;
            }
            "load" => {
                let s = self.start(false)?;
                let wait = at(rng, restart.mul_f64(1.2));
                self.kill_after(r, &s, s.started, wait)?;
            }
            "stream" => {
                let s = self.start(false)?;
                s.wait_settled(Duration::from_secs(3600))?;
                let stop = Arc::new(AtomicBool::new(false));
                let started = Instant::now();
                std::thread::scope(|scope| -> Result<()> {
                    let stop2 = stop.clone();
                    scope.spawn(move || self.stream(&stop2));
                    let wait = Duration::from_millis(500) + at(rng, Duration::from_secs(8));
                    let res = self.kill_after(r, &s, started, wait);
                    // Edits go on while the server is down.
                    std::thread::sleep(Duration::from_secs(1));
                    stop.store(true, Ordering::Release);
                    res
                })?;
            }
            "burst" => {
                let s = self.start(false)?;
                s.wait_settled(Duration::from_secs(3600))?;
                let started = Instant::now();
                self.rewrite_share(10);
                let wait = at(rng, Duration::from_secs(5));
                self.kill_after(r, &s, started, wait)?;
            }
            "save" => {
                let s = self.start(false)?;
                s.wait_settled(Duration::from_secs(3600))?;
                let stop = Arc::new(AtomicBool::new(false));
                let started = Instant::now();
                std::thread::scope(|scope| -> Result<()> {
                    let stop2 = stop.clone();
                    scope.spawn(move || self.stream(&stop2));
                    // Into the second save or later, a moment after it begins.
                    let skip = 1 + rng.random_range(0..3);
                    let res = self.kill_while_saving(r, &s, started, rng, skip);
                    std::thread::sleep(Duration::from_secs(1));
                    stop.store(true, Ordering::Release);
                    res
                })?;
            }
            "shutdown" => {
                let s = self.start(false)?;
                s.wait_settled(Duration::from_secs(3600))?;
                // Something unsaved for the shutdown to save: fewer
                // updates than start a save of their own.
                let every = (self.args.files * 2 / self.args.save_after_updates.max(1)).max(2);
                self.rewrite_share(every);
                std::thread::sleep(Duration::from_secs(2));
                s.terminate();
                self.kill_while_saving(r, &s, Instant::now(), rng, 0)?;
            }
            _ => unreachable!(),
        }
        r.index_left = self.index().exists();
        r.changed = self.changed.lock().unwrap().len();
        Ok(())
    }

    /// Kill `s` a moment (0 to 20 ms) after a save begins, past `skip`
    /// earlier saves: the moment a save's temp file appears is watched for
    /// every millisecond. Without a save within 60 s, it is killed then.
    fn kill_while_saving(
        &self,
        r: &mut Round,
        s: &Server,
        from: Instant,
        rng: &mut StdRng,
        mut skip: usize,
    ) -> Result<()> {
        let mut saving = false;
        while from.elapsed() < Duration::from_secs(60) && !s.crashed() {
            let now = self.temp_files() > 0;
            if now && !saving {
                if skip == 0 {
                    std::thread::sleep(Duration::from_millis(rng.random_range(0..=20)));
                    break;
                }
                skip -= 1;
            }
            saving = now;
            std::thread::sleep(Duration::from_millis(1));
        }
        self.kill_after(r, s, from, Duration::ZERO)
    }

    /// Kill `s` once `wait` has passed since `from`, noting whether a save
    /// was under way.
    fn kill_after(&self, r: &mut Round, s: &Server, from: Instant, wait: Duration) -> Result<()> {
        if let Some(rest) = wait.checked_sub(from.elapsed()) {
            std::thread::sleep(rest);
        }
        r.kill_after_ms = from.elapsed().as_millis() as u64;
        r.killed_mid_save = self.temp_files() > 0;
        s.kill9()
    }

    /// Start the server again and check it recovered.
    fn check_round(&self, r: &mut Round) -> Result<()> {
        let s = self.start(false)?;
        let ready = s.wait_ready(Duration::from_secs(self.args.settle_seconds));
        r.ready_secs = s.started.elapsed().as_secs_f64();
        r.temp_files_after_start = self.temp_files();
        let log = std::fs::read_to_string(self.work.join("server.log")).unwrap_or_default();
        r.load = if log.contains("Failed to load persisted index") {
            "rejected"
        } else if log.contains("Loaded persisted index") {
            "loaded"
        } else {
            "none"
        }
        .into();
        if let Err(e) = ready {
            r.crashed = s.crashed();
            r.failures.push(format!("not ready: {e}"));
        } else {
            let ready_at = Instant::now();
            // Some of the changed files, and a few others (the status
            // check needs at least one path).
            let paths = {
                let changed = self.changed.lock().unwrap();
                let step = (changed.len() / 90).max(1);
                let mut paths: Vec<PathBuf> =
                    changed.iter().step_by(step).take(90).cloned().collect();
                let every = (self.args.files / 10).max(1);
                paths.extend(
                    (0..self.args.files)
                        .step_by(every)
                        .map(|i| path_of(&self.tree, i)),
                );
                paths
            };
            let settle = Duration::from_secs(self.args.settle_seconds);
            let settled = s.wait_settled(settle).is_ok();
            let fresh = settled.then(|| s.wait_fresh(&paths, settle)).flatten();
            r.fresh_secs = fresh.map(|_| ready_at.elapsed().as_secs_f64());
            r.not_settled = fresh.is_none();
            if fresh.is_some() {
                let report = s.check()?;
                r.check_problems = report.problems();
            }
            r.crashed = s.crashed();
            if !r.crashed {
                s.stop()?;
            }
        }
        if r.load == "rejected" {
            r.failures.push("the saved index was rejected".into());
        }
        if r.temp_files_after_start > 0 {
            r.failures
                .push(format!("{} temp files left", r.temp_files_after_start));
        }
        if r.not_settled {
            r.failures.push("never matched the disk".into());
        }
        if r.check_problems > 0 {
            r.failures
                .push(format!("{} check problems", r.check_problems));
        }
        if r.crashed {
            r.failures.push("crashed".into());
        }
        if !r.failures.is_empty() {
            let _ = std::fs::copy(
                self.work.join("server.log"),
                self.work.join(format!("failed-{}.log", r.round)),
            );
        }
        self.changed.lock().unwrap().clear();
        Ok(())
    }

    fn next_rev(&self) -> u64 {
        self.rev.fetch_add(1, Ordering::Relaxed)
    }

    fn changed(&self, p: PathBuf) {
        self.changed.lock().unwrap().push(p);
    }

    /// Rewrite one file in `every`.
    fn rewrite_share(&self, every: usize) {
        let rev = self.next_rev();
        for i in (rev as usize % every..self.args.files).step_by(every) {
            let p = path_of(&self.tree, i);
            if std::fs::write(&p, script(i, rev)).is_ok() {
                self.changed(p);
            }
        }
    }

    /// Rewrite, create and remove files at `stream_rate` until `stop`.
    fn stream(&self, stop: &AtomicBool) {
        let per_tick = (self.args.stream_rate / 10).max(1);
        let t = Instant::now();
        let mut tick = 0u32;
        while !stop.load(Ordering::Acquire) {
            let rev = self.next_rev();
            for k in 0..per_tick {
                let i =
                    (rev as usize).wrapping_mul(104_729).wrapping_add(k * 7_919) % self.args.files;
                let p = path_of(&self.tree, i);
                if std::fs::write(&p, script(i, rev)).is_ok() {
                    self.changed(p);
                }
            }
            let new = path_of(&self.tree, rev as usize * 997 % self.args.files)
                .with_file_name(format!("new_{rev}.py"));
            if std::fs::write(&new, script(rev as usize, rev)).is_ok() {
                self.changed(new.clone());
                let mut created = self.created.lock().unwrap();
                created.push(new);
                if created.len() > 50 {
                    let old = created.remove(0);
                    if std::fs::remove_file(&old).is_ok() {
                        self.changed(old);
                    }
                }
            }
            tick += 1;
            if let Some(wait) = (Duration::from_millis(100) * tick).checked_sub(t.elapsed()) {
                std::thread::sleep(wait);
            }
        }
    }

    /// Files earlier runs created, so every run starts from the generated
    /// tree (rewritten revisions aside).
    fn remove_created_files(&self) {
        for dir in std::fs::read_dir(&self.tree)
            .into_iter()
            .flatten()
            .flatten()
        {
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
}

fn render_markdown(rounds: &[Round], seed: u64, files: usize) -> String {
    #[derive(Default)]
    struct Sum {
        rounds: usize,
        mid_save: usize,
        no_index: usize,
        rejected: usize,
        temp_left: usize,
        max_ready: f64,
        max_fresh: f64,
        not_settled: usize,
        problems: usize,
        crashed: usize,
        failed: usize,
    }
    let mut by_kind: BTreeMap<&str, Sum> = BTreeMap::new();
    for r in rounds {
        let s = by_kind.entry(&r.kind).or_default();
        s.rounds += 1;
        s.mid_save += usize::from(r.killed_mid_save);
        s.no_index += usize::from(r.load == "none");
        s.rejected += usize::from(r.load == "rejected");
        s.temp_left += r.temp_files_after_start;
        s.max_ready = s.max_ready.max(r.ready_secs);
        s.max_fresh = s.max_fresh.max(r.fresh_secs.unwrap_or(0.0));
        s.not_settled += usize::from(r.not_settled);
        s.problems += r.check_problems;
        s.crashed += usize::from(r.crashed);
        s.failed += usize::from(!r.failures.is_empty());
    }
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Chaos: {} rounds on {files} files, seed {seed}.\n",
        rounds.len()
    );
    out.push_str(
        "| Killed during | Rounds | Mid save | No index | Rejected | Temp files left | Max ready | Max catch-up | Not settled | Check problems | Crashes | Failed |\n\
         |---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for (kind, s) in &by_kind {
        let _ = writeln!(
            out,
            "| {kind} | {} | {} | {} | {} | {} | {:.1} s | {:.1} s | {} | {} | {} | {} |",
            s.rounds,
            s.mid_save,
            s.no_index,
            s.rejected,
            s.temp_left,
            s.max_ready,
            s.max_fresh,
            s.not_settled,
            s.problems,
            s.crashed,
            s.failed
        );
    }
    out
}
