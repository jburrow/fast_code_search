//! What a query's trigram lookup costs when posting lists are read from the
//! saved index instead of from the heap (design C1, "Reading posting lists
//! in place").
//!
//! Today a load deserializes every posting list onto the heap, and a query
//! intersects those. With C1 the lists stay in the mapped file and a query
//! decodes only the ones it touches. This times both for the same queries
//! on the same saved index, so the cost of decoding per query is measured
//! rather than guessed:
//!
//! ```text
//! cargo run --release --example corpus_bench -- <dir>... --keep-index idx.bin
//! cargo run --release --example posting_decode -- idx.bin [--iterations 50]
//! ```
//!
//! Queries: the corpus benchmark's text queries and every plain query in
//! `benches/rg_backtest.toml`. Only the trigram step is timed (candidates
//! for the query), not reading files or ranking.

use anyhow::{Context, Result};
use clap::Parser;
use fast_code_search::index::persistence::PersistedIndex;
use fast_code_search::index::trigram::{extract_unique_trigrams, Trigram};
use roaring::RoaringBitmap;
use rustc_hash::FxHashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
struct Args {
    /// A saved index (`corpus_bench --keep-index`, or a server's index.bin).
    index: PathBuf,
    /// Timed runs per query and way (after one warm-up).
    #[arg(long, default_value_t = 50)]
    iterations: usize,
    /// The backtest's queries file.
    #[arg(long, default_value = "benches/rg_backtest.toml")]
    queries: PathBuf,
    /// Also load the index the way the server does (reconciling against
    /// these folders, the ones it was built from) and print the load's
    /// phases, to compare with what reading posting lists costs.
    #[arg(long, num_args = 1..)]
    roots: Vec<PathBuf>,
}

/// Candidates from posting lists already on the heap: what
/// `TrigramIndex::search` does.
fn from_heap(map: &FxHashMap<Trigram, RoaringBitmap>, trigrams: &[Trigram]) -> RoaringBitmap {
    let mut lists = Vec::with_capacity(trigrams.len());
    for t in trigrams {
        match map.get(t) {
            Some(b) => lists.push(b),
            None => return RoaringBitmap::new(),
        }
    }
    lists.sort_by_key(|b| b.len());
    let mut out = lists[0].clone();
    for b in &lists[1..] {
        out &= *b;
        if out.is_empty() {
            break;
        }
    }
    out
}

/// Candidates from the mapped file: find each list in the directory,
/// decode it, intersect. Returns the candidates and the bytes decoded.
fn from_mapped(index: &PersistedIndex, trigrams: &[Trigram]) -> Result<(RoaringBitmap, usize)> {
    let mut lists = Vec::with_capacity(trigrams.len());
    let mut bytes = 0;
    for t in trigrams {
        let Some(raw) = index.trigram_index.posting_bytes(*t) else {
            return Ok((RoaringBitmap::new(), bytes));
        };
        bytes += raw.len();
        lists.push(RoaringBitmap::deserialize_unchecked_from(raw)?);
    }
    lists.sort_by_key(|b| b.len());
    let mut lists = lists.into_iter();
    let mut out = lists.next().unwrap_or_default();
    for b in lists {
        out &= b;
        if out.is_empty() {
            break;
        }
    }
    Ok((out, bytes))
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

fn time(iterations: usize, mut f: impl FnMut()) -> Duration {
    f();
    median(
        (0..iterations.max(1))
            .map(|_| {
                let started = Instant::now();
                f();
                started.elapsed()
            })
            .collect(),
    )
}

fn fmt(d: Duration) -> String {
    let us = d.as_secs_f64() * 1e6;
    if us >= 1000.0 {
        format!("{:.2} ms", us / 1000.0)
    } else {
        format!("{us:.1} µs")
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    if !args.roots.is_empty() {
        tracing_subscriber::fmt()
            .with_env_filter("fast_code_search::search::engine=info")
            .with_writer(std::io::stderr)
            .init();
        let config = fast_code_search::config::IndexerConfig {
            paths: args
                .roots
                .iter()
                .map(|p| p.canonicalize().map(|c| c.to_string_lossy().into_owned()))
                .collect::<std::io::Result<_>>()?,
            ..Default::default()
        };
        let started = Instant::now();
        let mut engine = fast_code_search::search::SearchEngine::new();
        engine.load_index_with_reconciliation(&args.index, &config)?;
        println!(
            "server-style load: {} (phases in the log line above)",
            fmt(started.elapsed())
        );
    }
    let started = Instant::now();
    let index = PersistedIndex::load(&args.index)
        .with_context(|| format!("loading {}", args.index.display()))?;
    let mapped_load = started.elapsed();
    let started = Instant::now();
    let heap = index.restore_trigram_index()?;
    let restore = started.elapsed();

    let mut queries: Vec<String> = ["return", "Config", "xq9zv_nothing_here", "fn"]
        .into_iter()
        .map(String::from)
        .collect();
    let backtest: toml::Value = toml::from_str(&std::fs::read_to_string(&args.queries)?)?;
    for q in backtest
        .get("query")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        let plain = q.get("mode").and_then(|v| v.as_str()) != Some("regex");
        if let (true, Some(text)) = (plain, q.get("q").and_then(|v| v.as_str())) {
            queries.push(text.to_string());
        }
    }

    println!(
        "{} files, {} posting lists; load (mapped, metadata decoded) {}, restoring every list onto the heap {}",
        index.files.len(),
        index.trigram_index.len(),
        fmt(mapped_load),
        fmt(restore)
    );
    println!();
    println!("| Query | Trigrams | Bytes decoded | Candidates | Heap | Mapped | Extra |");
    println!("|---|---|---|---|---|---|---|");
    let (mut heap_total, mut mapped_total) = (Duration::ZERO, Duration::ZERO);
    let mut extras = Vec::new();
    let mut counted = 0;
    for q in &queries {
        let folded = fast_code_search::search::fold::fold_str(q);
        let trigrams: Vec<Trigram> = extract_unique_trigrams(&folded).into_iter().collect();
        if trigrams.is_empty() {
            continue; // no trigram step: such a query reads every file
        }
        let (want, bytes) = from_mapped(&index, &trigrams)?;
        anyhow::ensure!(
            want == from_heap(&heap, &trigrams),
            "the two ways disagree on {q:?}"
        );
        let h = time(args.iterations, || {
            std::hint::black_box(from_heap(&heap, &trigrams));
        });
        let m = time(args.iterations, || {
            std::hint::black_box(from_mapped(&index, &trigrams).unwrap());
        });
        heap_total += h;
        mapped_total += m;
        extras.push(m.saturating_sub(h));
        counted += 1;
        println!(
            "| `{}` | {} | {} | {} | {} | {} | {} |",
            q.replace('|', "\\|").replace('`', "'"),
            trigrams.len(),
            bytes,
            want.len(),
            fmt(h),
            fmt(m),
            fmt(m.saturating_sub(h))
        );
    }
    println!();
    println!(
        "{counted} queries: heap {} in all, mapped {} in all; extra per query median {}, max {}",
        fmt(heap_total),
        fmt(mapped_total),
        fmt(median(extras.clone())),
        fmt(extras.iter().copied().max().unwrap_or_default())
    );
    Ok(())
}
