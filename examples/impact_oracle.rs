//! Impact oracle: does `fcs impact` list the files that use what a commit
//! changes, as Jedi resolves them?
//!
//! Indexes a Git repository at HEAD, then for each of its last N commits
//! that touch Python files runs the engine's impact analysis on
//! `git diff C^ HEAD -- <files C changed>` (so the new side is the indexed
//! one), and compares the files listed for each changed symbol with the
//! files where Jedi resolves a use to that symbol's definition
//! (`src/check/oracle/py_impact.py`).
//!
//! ```sh
//! pip install jedi==0.20.1
//! python3 src/check/oracle/py_impact.py REPO py-impact.jsonl
//! cargo run --release --example impact_oracle -- --repo REPO --oracle py-impact.jsonl \
//!     --commits 300 [--min-found 0.98] [--min-right 0.75] [--markdown report.md]
//! ```
//!
//! **Found**: the share of the oracle's user files that fcs lists.
//! **Right**: the share of listed files the oracle confirms (a lower bound:
//! Jedi misses dynamic uses, such as dotted paths in settings). Files the
//! diff itself changes are left out on both sides. Functions, classes and
//! constants are scored apart from members (methods, fields), which fcs
//! finds by name only.

use anyhow::{bail, Context, Result};
use clap::Parser;
use fast_code_search::config::IndexerConfig;
use fast_code_search::search::engine::{PartialIndexedFile, PreIndexedFile, SearchEngine};
use fast_code_search::search::{FileDiscoveryConfig, FileDiscoveryIterator, ImpactOptions};
use rayon::prelude::*;
use serde::Deserialize;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    repo: PathBuf,
    /// The oracle's uses (JSON lines from src/check/oracle/py_impact.py).
    #[arg(long)]
    oracle: PathBuf,
    #[arg(long, default_value_t = 300)]
    commits: usize,
    /// Fail when functions/classes found falls below this share.
    #[arg(long)]
    min_found: Option<f64>,
    /// Fail when functions/classes right falls below this share.
    #[arg(long)]
    min_right: Option<f64>,
    /// Fail when members found falls below this share.
    #[arg(long)]
    min_member_found: Option<f64>,
    /// Write the markdown report here as well as to stderr.
    #[arg(long)]
    markdown: Option<PathBuf>,
}

#[derive(Deserialize)]
struct Use {
    file: Option<String>,
    target_file: Option<String>,
    target_line: Option<usize>,
}

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()?;
    if !out.status.success() {
        bail!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[derive(Default)]
struct Score {
    symbols: usize,
    found: usize,
    truth: usize,
    listed: usize,
    right: usize,
}

impl Score {
    fn found(&self) -> f64 {
        self.found as f64 / self.truth.max(1) as f64
    }
    fn right(&self) -> f64 {
        self.right as f64 / self.listed.max(1) as f64
    }
    fn row(&self, what: &str) -> String {
        format!(
            "| {what} | {} | {}/{} = {:.1}% | {}/{} = {:.1}% |\n",
            self.symbols,
            self.found,
            self.truth,
            100.0 * self.found(),
            self.right,
            self.listed,
            100.0 * self.right()
        )
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let repo = args.repo.canonicalize()?;
    let mut users: HashMap<(String, usize), HashSet<String>> = HashMap::new();
    for line in std::fs::read_to_string(&args.oracle)
        .with_context(|| format!("reading {:?}", args.oracle))?
        .lines()
    {
        let u: Use = serde_json::from_str(line)?;
        if let (Some(f), Some(tf), Some(tl)) = (u.file, u.target_file, u.target_line) {
            users.entry((tf, tl)).or_default().insert(f);
        }
    }

    let root = repo.to_string_lossy().into_owned();
    let config = IndexerConfig {
        paths: vec![root.clone()],
        ..Default::default()
    };
    let mut engine = SearchEngine::new();
    engine.add_root_path(&repo);
    let t = Instant::now();
    let discovery = FileDiscoveryConfig {
        paths: vec![root],
        exclude_patterns: config.exclude_patterns.clone(),
        include_extensions: config.include_extensions.clone(),
        max_file_size: Some(config.max_file_size),
        respect_gitignore: config.respect_gitignore,
        ..Default::default()
    };
    let files: Vec<PathBuf> = FileDiscoveryIterator::new(&discovery).collect();
    for chunk in files.chunks(config.batch_size.max(1)) {
        let pre: Vec<PreIndexedFile> = chunk
            .par_iter()
            .filter_map(|p| {
                let (partial, _) = PartialIndexedFile::process(
                    p,
                    config.transcode_non_utf8,
                    config.max_file_size,
                )?;
                Some(PreIndexedFile::from_partial(partial, config.enable_symbols))
            })
            .collect();
        engine.index_batch(pre);
        engine.resolve_imports_incremental();
    }
    engine.resolve_imports();
    engine.finalize();
    eprintln!(
        "indexed {} files in {:.1}s",
        engine.get_stats().num_files,
        t.elapsed().as_secs_f64()
    );

    let opts = ImpactOptions {
        root: Some(repo.clone()),
        max_files: 0,
        max_lines: 1,
    };
    let label = format!("{}/", repo.file_name().unwrap().to_string_lossy());
    let log = git(
        &repo,
        &[
            "log",
            "--no-merges",
            "--format=%H",
            "-n",
            &args.commits.to_string(),
            "--",
            "*.py",
        ],
    )?;
    let (mut plain, mut members) = (Score::default(), Score::default());
    let (mut commits, mut times) = (0usize, Vec::new());
    for commit in log.lines() {
        // The oldest commit of a shallow clone has no parent here.
        let Ok(changed) = git(
            &repo,
            &[
                "diff",
                "--name-only",
                &format!("{commit}^"),
                commit,
                "--",
                "*.py",
            ],
        ) else {
            continue;
        };
        let changed: Vec<&str> = changed.lines().collect();
        if changed.is_empty() {
            continue;
        }
        let mut cmd = vec![
            "diff".to_string(),
            "--no-renames".into(),
            format!("{commit}^"),
            "HEAD".into(),
            "--".into(),
        ];
        cmd.extend(changed.iter().map(|s| s.to_string()));
        let cmd: Vec<&str> = cmd.iter().map(String::as_str).collect();
        let diff = git(&repo, &cmd)?;
        if diff.trim().is_empty() {
            continue;
        }
        let t = Instant::now();
        let Ok(report) = engine.impact(&diff, &opts) else {
            continue;
        };
        let ms = t.elapsed().as_secs_f64() * 1e3;
        times.push(ms);
        commits += 1;
        let in_diff: HashSet<&str> = report.files.iter().map(|f| f.path.as_str()).collect();
        for s in &report.symbols {
            if s.change == fast_code_search::impact::defs::Change::Removed
                || s.names.first() != Some(&s.name)
            {
                continue; // not in HEAD, or a constructor scored under its class
            }
            let Some(truth) = users.get(&(s.file.clone(), s.line - 1)) else {
                continue;
            };
            let truth: BTreeSet<&str> = truth
                .iter()
                .map(String::as_str)
                .filter(|f| !in_diff.contains(f))
                .collect();
            if truth.is_empty() {
                continue;
            }
            let listed: BTreeSet<&str> = s
                .users
                .iter()
                .map(|u| u.path.strip_prefix(&label).unwrap_or(&u.path))
                .filter(|f| !in_diff.contains(f))
                .collect();
            let score = if s.kind.is_member() {
                &mut members
            } else {
                &mut plain
            };
            score.symbols += 1;
            score.truth += truth.len();
            score.found += truth.intersection(&listed).count();
            score.listed += listed.len();
            score.right += listed.iter().filter(|f| truth.contains(*f)).count();
        }
    }
    times.sort_by(f64::total_cmp);
    let pct = |p: f64| {
        times
            .get(((times.len() as f64 - 1.0) * p) as usize)
            .copied()
            .unwrap_or(0.0)
    };
    let mut md = format!(
        "## Impact oracle: {}\n\n{commits} commits, impact analysis {:.0} ms median, {:.0} ms p95 per commit.\n\n| Symbols | Scored | Found | Right |\n|---|---|---|---|\n",
        repo.display(),
        pct(0.5),
        pct(0.95)
    );
    md.push_str(&plain.row("Functions, classes, constants"));
    md.push_str(&members.row("Methods and fields"));
    eprintln!("{md}");
    if let Some(p) = &args.markdown {
        std::fs::write(p, &md)?;
    }
    let mut failed = Vec::new();
    if args.min_found.is_some_and(|m| plain.found() < m) {
        failed.push(format!(
            "found {:.3} < {}",
            plain.found(),
            args.min_found.unwrap()
        ));
    }
    if args.min_right.is_some_and(|m| plain.right() < m) {
        failed.push(format!(
            "right {:.3} < {}",
            plain.right(),
            args.min_right.unwrap()
        ));
    }
    if args.min_member_found.is_some_and(|m| members.found() < m) {
        failed.push(format!(
            "member found {:.3} < {}",
            members.found(),
            args.min_member_found.unwrap()
        ));
    }
    if !failed.is_empty() {
        bail!("impact oracle below baseline: {}", failed.join(", "));
    }
    Ok(())
}
