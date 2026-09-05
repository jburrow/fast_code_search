//! Real-corpus benchmark for the keyword engine.
//!
//! Builds an index over one or more directories exactly the way the server
//! does (discovery, parallel read + trigram + symbol extraction, batched
//! merge, import resolution, finalize) and reports:
//!
//! - build time, files, bytes, throughput, resident memory after the build
//! - query latency percentiles for a fixed set of text / regex / symbol /
//!   filtered searches
//! - incremental update latency (rewrite one file, apply the change)
//! - persisted index save and reconciling-load time and size
//!
//! ```text
//! cargo run --release --example corpus_bench -- <dir> [<dir>...] \
//!     [--label NAME] [--iterations N] [--bencher] [--markdown FILE]
//! ```
//!
//! `--bencher` prints one `test <name> ... bench: <ns> ns/iter (+/- <spread>)`
//! line per metric on stdout so the output can feed the same trend tracking
//! as `cargo bench`; the human-readable table goes to stderr and, with
//! `--markdown`, to a file (CI appends it to the job summary).

use anyhow::{Context, Result};
use clap::Parser;
use fast_code_search::config::IndexerConfig;
use fast_code_search::search::engine::{
    PartialIndexedFile, PreIndexedFile, RankMode, SearchEngine, SearchLimits, SearchRankingInfo,
};
use fast_code_search::search::{
    apply_change, FileChange, FileDiscoveryConfig, FileDiscoveryIterator,
};
use rayon::prelude::*;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[command(name = "corpus_bench", about = "Real-corpus keyword engine benchmark")]
struct Args {
    /// Directories to index.
    #[arg(required = true)]
    paths: Vec<PathBuf>,
    /// Name used in the report and in bencher metric names.
    #[arg(long, default_value = "corpus")]
    label: String,
    /// Timed iterations per query (after one warm-up).
    #[arg(long, default_value_t = 20)]
    iterations: usize,
    /// Files rewritten for the incremental-update measurement.
    #[arg(long, default_value_t = 20)]
    incremental_files: usize,
    /// Emit `cargo bench`-style bencher lines on stdout.
    #[arg(long)]
    bencher: bool,
    /// Write the markdown report to this file as well as stderr.
    #[arg(long)]
    markdown: Option<PathBuf>,
    /// Skip tree-sitter symbol/import extraction (isolates trigram cost).
    #[arg(long)]
    no_symbols: bool,
}

/// One reported metric.
struct Metric {
    name: &'static str,
    /// Human-readable detail (result count, bytes, ...).
    detail: String,
    p50: Duration,
    p95: Duration,
}

fn percentile(sorted: &[Duration], pct: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let idx = ((sorted.len() - 1) as f64 * pct).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn timed<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let out = f();
    (out, start.elapsed())
}

/// Run `f` once untimed, then `iterations` times timed; returns the metric
/// and the last result.
fn measure<T>(
    name: &'static str,
    iterations: usize,
    detail: impl Fn(&T) -> String,
    mut f: impl FnMut() -> T,
) -> Metric {
    let mut last = f();
    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..iterations.max(1) {
        let (out, took) = timed(&mut f);
        samples.push(took);
        last = out;
    }
    samples.sort();
    Metric {
        name,
        detail: detail(&last),
        p50: percentile(&samples, 0.5),
        p95: percentile(&samples, 0.95),
    }
}

fn resident_bytes() -> u64 {
    use sysinfo::{Pid, ProcessesToUpdate, System};
    let pid = Pid::from_u32(std::process::id());
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    sys.process(pid).map(|p| p.memory()).unwrap_or(0)
}

fn fmt_bytes(b: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    if b as f64 >= 1024.0 * MB {
        format!("{:.2} GB", b as f64 / (1024.0 * MB))
    } else {
        format!("{:.1} MB", b as f64 / MB)
    }
}

fn fmt_dur(d: Duration) -> String {
    let us = d.as_secs_f64() * 1e6;
    if us >= 1e6 {
        format!("{:.2} s", us / 1e6)
    } else if us >= 1e3 {
        format!("{:.2} ms", us / 1e3)
    } else {
        format!("{:.0} µs", us)
    }
}

/// Build the index the way the background indexer does, synchronously.
fn build(engine: &mut SearchEngine, files: &[PathBuf], config: &IndexerConfig) -> u64 {
    let mut bytes = 0u64;
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
        bytes += pre.iter().map(|p| p.size).sum::<u64>();
        engine.index_batch(pre);
        engine.resolve_imports_incremental();
    }
    engine.resolve_imports();
    engine.finalize();
    bytes
}

fn main() -> Result<()> {
    let args = Args::parse();
    let roots: Vec<String> = args
        .paths
        .iter()
        .map(|p| {
            p.canonicalize()
                .with_context(|| format!("{} does not exist", p.display()))
                .map(|c| c.to_string_lossy().into_owned())
        })
        .collect::<Result<_>>()?;
    let config = IndexerConfig {
        paths: roots.clone(),
        enable_symbols: !args.no_symbols,
        ..Default::default()
    };

    // Discovery is part of what a user waits for, so it is timed too.
    let (files, discover_took) = timed(|| {
        let discovery = FileDiscoveryConfig {
            paths: roots.clone(),
            exclude_patterns: config.exclude_patterns.clone(),
            include_extensions: config.include_extensions.clone(),
            max_file_size: Some(config.max_file_size),
            respect_gitignore: config.respect_gitignore,
            ..Default::default()
        };
        FileDiscoveryIterator::new(&discovery).collect::<Vec<PathBuf>>()
    });

    let rss_before = resident_bytes();
    let mut engine = SearchEngine::new();
    for r in &roots {
        engine.add_root_path(Path::new(r));
    }
    let (bytes, build_took) = timed(|| build(&mut engine, &files, &config));
    let rss_after = resident_bytes();
    let stats = engine.get_stats();

    let mut metrics: Vec<Metric> = Vec::new();
    let n = args.iterations;
    let page = 50;

    // Text searches: a very common token, a moderately common one, a token
    // that appears nowhere (index-only), and a two-character query (the
    // all-documents path).
    for (name, q) in [
        ("text/common", "return"),
        ("text/identifier", "Config"),
        ("text/no_match", "xq9zv_nothing_here"),
        ("text/short", "fn"),
    ] {
        metrics.push(measure(
            name,
            n,
            |r: &Vec<_>| format!("{} results", r.len()),
            || engine.search(q, page),
        ));
    }
    metrics.push(measure(
        "text/full_rank",
        n,
        |r: &(Vec<_>, _)| format!("{} results", r.0.len()),
        || engine.search_ranked("return", page, RankMode::Full),
    ));
    metrics.push(measure(
        "text/filtered",
        n,
        |r: &Result<Vec<_>>| format!("{} results", r.as_ref().map(|v| v.len()).unwrap_or(0)),
        || engine.search_with_filter("return", "**/*.rs;**/*.py", "**/tests/**", page),
    ));

    // Regex: accelerated by a literal, case-insensitive literal, and one
    // with no usable literal (bounded full scan).
    for (name, re) in [
        ("regex/literal", r"fn\s+\w+\("),
        ("regex/case_insensitive", r"(?i)unwrap\("),
        ("regex/no_literal", r"[a-z]+_[a-z]+_[a-z]+\("),
    ] {
        metrics.push(measure(
            name,
            n,
            |r: &Result<(Vec<_>, SearchRankingInfo)>| {
                r.as_ref()
                    .map(|(v, info)| {
                        format!(
                            "{} results{}",
                            v.len(),
                            if info.truncated_by_budget {
                                " (budget)"
                            } else {
                                ""
                            }
                        )
                    })
                    .unwrap_or_else(|e| format!("error: {e}"))
            },
            || engine.search_regex_with_limits(re, "", "", SearchLimits::new(page), RankMode::Auto),
        ));
    }

    // Symbols.
    for (name, q) in [("symbol/exact", "Error"), ("symbol/prefix", "Req")] {
        metrics.push(measure(
            name,
            n,
            |r: &Result<Vec<_>>| format!("{} results", r.as_ref().map(|v| v.len()).unwrap_or(0)),
            || engine.search_symbols(q, "", "", page),
        ));
    }

    // Incremental update: rewrite a spread of small files and apply each
    // change as the watcher would; restore afterwards.
    let mut incremental: Vec<Duration> = Vec::new();
    let candidates: Vec<&PathBuf> = files
        .iter()
        .filter(|p| {
            std::fs::metadata(p)
                .map(|m| m.len() <= 64 * 1024)
                .unwrap_or(false)
        })
        .collect();
    let step = (candidates.len() / args.incremental_files.max(1)).max(1);
    for p in candidates.iter().step_by(step).take(args.incremental_files) {
        let Ok(original) = std::fs::read(p) else {
            continue;
        };
        let mut edited = original.clone();
        edited.extend_from_slice(b"\n// corpus_bench edit\n");
        if std::fs::write(p, &edited).is_err() {
            continue;
        }
        let change = FileChange::Modified((*p).clone());
        let (_, took) = timed(|| apply_change(&mut engine, &change, &config));
        incremental.push(took);
        let _ = std::fs::write(p, &original);
        apply_change(&mut engine, &change, &config);
    }
    incremental.sort();
    metrics.push(Metric {
        name: "incremental/modify_file",
        detail: format!("{} files", incremental.len()),
        p50: percentile(&incremental, 0.5),
        p95: percentile(&incremental, 0.95),
    });

    // Persistence.
    let tmp = tempfile::tempdir()?;
    let index_path = tmp.path().join("index.bin");
    let (_, save_took) = timed(|| engine.save_index(&index_path, &config));
    let index_bytes = std::fs::metadata(&index_path).map(|m| m.len()).unwrap_or(0);
    let (_, load_took) = timed(|| {
        let mut fresh = SearchEngine::new();
        fresh.load_index_with_reconciliation(&index_path, &config)
    });

    // Report.
    let mut md = String::new();
    let _ = writeln!(md, "### Corpus benchmark: {}", args.label);
    let _ = writeln!(md);
    let _ = writeln!(md, "| Build | Value |");
    let _ = writeln!(md, "|---|---|");
    let _ = writeln!(md, "| files indexed | {} |", stats.num_files);
    let _ = writeln!(md, "| bytes indexed | {} |", fmt_bytes(bytes));
    let _ = writeln!(md, "| discovery | {} |", fmt_dur(discover_took));
    let _ = writeln!(
        md,
        "| build (read + trigrams + symbols + merge) | {} ({:.0} files/s, {:.1} MB/s) |",
        fmt_dur(build_took),
        stats.num_files as f64 / build_took.as_secs_f64().max(1e-9),
        bytes as f64 / (1024.0 * 1024.0) / build_took.as_secs_f64().max(1e-9)
    );
    let _ = writeln!(md, "| trigrams | {} |", stats.num_trigrams);
    let _ = writeln!(md, "| dependency edges | {} |", stats.dependency_edges);
    let _ = writeln!(
        md,
        "| resident memory after build | {} (process before build: {}) |",
        fmt_bytes(rss_after),
        fmt_bytes(rss_before)
    );
    let _ = writeln!(
        md,
        "| index save | {} ({}) |",
        fmt_dur(save_took),
        fmt_bytes(index_bytes)
    );
    let _ = writeln!(md, "| index load (reconciling) | {} |", fmt_dur(load_took));
    let _ = writeln!(md);
    let _ = writeln!(md, "| Query | p50 | p95 | Detail |");
    let _ = writeln!(md, "|---|---|---|---|");
    for m in &metrics {
        let _ = writeln!(
            md,
            "| {} | {} | {} | {} |",
            m.name,
            fmt_dur(m.p50),
            fmt_dur(m.p95),
            m.detail
        );
    }
    eprint!("{md}");
    if let Some(path) = &args.markdown {
        std::fs::write(path, &md).with_context(|| format!("writing {}", path.display()))?;
    }

    if args.bencher {
        let line = |name: &str, value: Duration, spread: Duration| {
            println!(
                "test corpus_bench/{}/{} ... bench: {} ns/iter (+/- {})",
                args.label,
                name,
                value.as_nanos(),
                spread.as_nanos()
            );
        };
        line("index_build", build_took, Duration::ZERO);
        line("index_save", save_took, Duration::ZERO);
        line("index_load", load_took, Duration::ZERO);
        for m in &metrics {
            line(m.name, m.p50, m.p95.saturating_sub(m.p50));
        }
        // Memory as a pseudo-metric so the trend chart tracks it (bytes, not ns).
        println!(
            "test corpus_bench/{}/rss_after_build_bytes ... bench: {} ns/iter (+/- 0)",
            args.label, rss_after
        );
    }
    Ok(())
}
