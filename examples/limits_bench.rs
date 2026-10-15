//! Evidence for the search limits: what each limit costs and what it hides.
//!
//! Indexes a corpus (or loads a saved index), then for each query:
//!
//! - runs it exhaustively (every hit, no budget) to get the reference ranking
//!   and the true counts (hits, files, files over the per-file cap);
//! - runs it under each setting being judged (ranking mode, fast-ranking file
//!   cap, match budget, page size) and reports latency, how many of the
//!   reference top-K appear on the page, whether the reference best hit is on
//!   the page, and which limits were reported.
//!
//! ```text
//! cargo run --release --example limits_bench -- <dir>... [--index FILE] \
//!     [--queries FILE] [--json OUT] [--iterations N]
//! ```

use anyhow::Result;
use clap::Parser;
use fast_code_search::config::IndexerConfig;
use fast_code_search::search::engine::{
    PartialIndexedFile, PreIndexedFile, RankMode, SearchEngine, SearchLimits, SearchMatch,
    SearchRankingInfo,
};
use fast_code_search::search::{parse_query, FileDiscoveryConfig, FileDiscoveryIterator};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
struct Args {
    #[arg(required = true)]
    paths: Vec<PathBuf>,
    /// Load this index if it exists, else build and save it here.
    #[arg(long)]
    index: Option<PathBuf>,
    /// One query per line (default: a built-in list).
    #[arg(long)]
    queries: Option<PathBuf>,
    /// Timed runs per setting (median reported).
    #[arg(long, default_value_t = 3)]
    iterations: usize,
    /// Write every row as JSON lines here.
    #[arg(long)]
    json: Option<PathBuf>,
    /// Skip the exhaustive reference when it would exceed this many hits
    /// (it is still counted).
    #[arg(long, default_value_t = 3_000_000)]
    max_reference_hits: usize,
    /// Only run these experiments (comma list: modes,files,budget,page).
    #[arg(long, default_value = "modes,files,budget,page")]
    only: String,
}

fn build(engine: &mut SearchEngine, files: &[PathBuf], config: &IndexerConfig) {
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
}

type Key = (u32, usize);

fn key(m: &SearchMatch) -> Key {
    (m.file_id, m.line_number)
}

struct Reference {
    hits: usize,
    files: usize,
    files_over_100: usize,
    max_file_hits: usize,
    /// Reference page order (rank order with each file's hits together).
    order: Vec<Key>,
    took: Duration,
}

fn run(
    engine: &SearchEngine,
    q: &str,
    limits: SearchLimits,
    mode: RankMode,
) -> (Vec<SearchMatch>, SearchRankingInfo) {
    engine
        .search_parsed(&parse_query(q), "", "", limits, mode)
        .expect("search")
}

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v[v.len() / 2]
}

fn ms(d: Duration) -> f64 {
    (d.as_secs_f64() * 1e5).round() / 100.0
}

const DEFAULT_QUERIES: &[&str] = &[
    // very common tokens
    "return", "static", "struct", "error", "NULL", "self", "include", "the",
    // common identifiers
    "mutex_lock", "kmalloc", "unwrap", "Config", "printf", "std::vector",
    // medium
    "spin_lock_irqsave", "EXPORT_SYMBOL_GPL", "SmallVector", "ctx.Done()",
    // rare strings made of common trigrams
    "return -EBUSY;", "if (ret < 0) {", "get_user_pages_fast", "handle_error",
    "TODO(", "deadbeef", "LLVM_DEBUG(dbgs() <<", "fn main",
];

fn main() -> Result<()> {
    let args = Args::parse();
    let roots: Vec<String> = args
        .paths
        .iter()
        .map(|p| p.canonicalize().map(|c| c.to_string_lossy().into_owned()))
        .collect::<std::io::Result<_>>()?;
    let config = IndexerConfig {
        paths: roots.clone(),
        ..Default::default()
    };
    let mut engine = SearchEngine::new();
    for r in &roots {
        engine.add_root_path(Path::new(r));
    }
    let t = Instant::now();
    let loaded = match &args.index {
        Some(p) if p.exists() => {
            engine.load_index_with_reconciliation(p, &config)?;
            engine.finalize();
            true
        }
        _ => false,
    };
    if !loaded {
        let discovery = FileDiscoveryConfig {
            paths: roots.clone(),
            exclude_patterns: config.exclude_patterns.clone(),
            include_extensions: config.include_extensions.clone(),
            max_file_size: Some(config.max_file_size),
            respect_gitignore: config.respect_gitignore,
            ..Default::default()
        };
        let files: Vec<PathBuf> = FileDiscoveryIterator::new(&discovery).collect();
        build(&mut engine, &files, &config);
        if let Some(p) = &args.index {
            engine.save_index(p, &config)?;
        }
    }
    let stats = engine.get_stats();
    eprintln!(
        "index: {} files, {} ({}) in {:.1}s, threads {}",
        stats.num_files,
        stats.total_size,
        if loaded { "loaded" } else { "built" },
        t.elapsed().as_secs_f64(),
        rayon::current_num_threads()
    );

    let queries: Vec<String> = match &args.queries {
        Some(p) => std::fs::read_to_string(p)?
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(String::from)
            .collect(),
        None => DEFAULT_QUERIES.iter().map(|s| s.to_string()).collect(),
    };
    let only: HashSet<&str> = args.only.split(',').collect();
    let mut out = args
        .json
        .as_ref()
        .map(std::fs::File::create)
        .transpose()?;
    let mut emit = |row: serde_json::Value| {
        println!("{row}");
        if let Some(f) = out.as_mut() {
            use std::io::Write;
            let _ = writeln!(f, "{row}");
        }
    };

    for q in &queries {
        // Reference: every hit, full ranking, no budget or per-file cap.
        let t = Instant::now();
        let (all, info) = run(&engine, q, SearchLimits::exhaustive(), RankMode::Full);
        let took = t.elapsed();
        let mut per_file: HashMap<u32, usize> = HashMap::new();
        for m in &all {
            *per_file.entry(m.file_id).or_default() += 1;
        }
        let reference = Reference {
            hits: all.len(),
            files: per_file.len(),
            files_over_100: per_file.values().filter(|&&n| n > 100).count(),
            max_file_hits: per_file.values().copied().max().unwrap_or(0),
            order: all.iter().map(key).collect(),
            took,
        };
        drop(all);
        emit(serde_json::json!({
            "query": q, "kind": "reference",
            "candidates": info.total_candidates,
            "hits": reference.hits, "files": reference.files,
            "files_over_100": reference.files_over_100,
            "max_file_hits": reference.max_file_hits,
            "ms": ms(reference.took),
        }));
        let reference_ok = reference.hits <= args.max_reference_hits;

        let measure = |label: &str,
                       value: serde_json::Value,
                       limits: SearchLimits,
                       mode: RankMode|
         -> serde_json::Value {
            let mut times = Vec::new();
            let mut last = None;
            for _ in 0..args.iterations.max(1) {
                let t = Instant::now();
                let r = run(&engine, q, limits.clone(), mode);
                times.push(t.elapsed());
                last = Some(r);
            }
            let (page, info) = last.unwrap();
            let k = limits.max_results.min(reference.order.len());
            let truth: HashSet<Key> = reference.order[..k].iter().copied().collect();
            let got: HashSet<Key> = page.iter().map(key).collect();
            let overlap = truth.intersection(&got).count();
            let best_found = reference
                .order
                .first()
                .is_none_or(|b| page.first().map(key) == Some(*b));
            let page_files: HashSet<u32> = page.iter().map(|m| m.file_id).collect();
            serde_json::json!({
                "query": q, "kind": label, "value": value,
                "mode": format!("{:?}", info.mode).to_lowercase(),
                "ms": ms(median(times)),
                "page": page.len(), "page_files": page_files.len(),
                "files_opened": info.candidates_searched,
                "candidates": info.total_candidates,
                "top_k_overlap": if reference_ok { serde_json::json!(overlap as f64 / k.max(1) as f64) } else { serde_json::Value::Null },
                "best_first": best_found,
                "limits": info.limits_hit.iter().map(|l| l.as_str()).collect::<Vec<_>>(),
            })
        };

        let page = 50;
        if only.contains("modes") {
            for (name, mode, files) in [
                ("auto", RankMode::Auto, 2000usize),
                ("full", RankMode::Full, 0),
                ("fast_2000", RankMode::Fast, 2000),
                // The proposal: metadata order, no file cap, stop at the budget.
                ("fast_all", RankMode::Fast, 0),
            ] {
                let l = SearchLimits::new(page).with_max_files(files);
                emit(measure("mode", serde_json::json!(name), l, mode));
            }
        }
        if only.contains("files") {
            for n in [500usize, 1000, 2000, 5000, 10_000, 20_000, 50_000, 0] {
                let l = SearchLimits::new(page).with_max_files(n);
                emit(measure("fast_files", serde_json::json!(n), l, RankMode::Fast));
            }
        }
        if only.contains("budget") {
            for b in [128usize, 512, 2048, 8192, 32_768, 100_000] {
                for (name, mode) in [("full", RankMode::Full), ("fast_all", RankMode::Fast)] {
                    let l = SearchLimits::new(page)
                        .with_max_files(0)
                        .with_match_budget(b);
                    emit(measure(
                        &format!("budget_{name}"),
                        serde_json::json!(b),
                        l,
                        mode,
                    ));
                }
            }
        }
        if only.contains("page") {
            for p in [20usize, 50, 100, 500, 1000] {
                let l = SearchLimits::new(p);
                emit(measure("page_auto", serde_json::json!(p), l, RankMode::Auto));
                let l = SearchLimits::new(p).with_max_files(0);
                emit(measure("page_fast_all", serde_json::json!(p), l, RankMode::Fast));
            }
        }
    }
    Ok(())
}
