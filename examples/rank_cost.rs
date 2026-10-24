//! What ranking costs: times page 1 of a search under the default match
//! budget (read files until 32,768 matches, then rank) against stopping as
//! soon as page 1 is full (what an unranked "first matches" mode would do),
//! and checks how much of the exhaustive top page each one returns.
//!
//! ```text
//! cargo run --release --example rank_cost -- <dir> [<dir>...] [--iterations N] [--page N]
//! ```

use clap::Parser;
use fast_code_search::config::IndexerConfig;
use fast_code_search::search::engine::{
    PartialIndexedFile, PreIndexedFile, RankMode, SearchEngine, SearchLimits,
};
use fast_code_search::search::{FileDiscoveryConfig, FileDiscoveryIterator};
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Parser)]
struct Args {
    paths: Vec<PathBuf>,
    #[arg(long, default_value_t = 15)]
    iterations: usize,
    #[arg(long, default_value_t = 50)]
    page: usize,
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

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v[v.len() / 2]
}

fn main() {
    let args = Args::parse();
    let roots: Vec<String> = args
        .paths
        .iter()
        .map(|p| p.canonicalize().unwrap().to_string_lossy().into_owned())
        .collect();
    let config = IndexerConfig {
        paths: roots.clone(),
        ..Default::default()
    };
    let discovery = FileDiscoveryConfig {
        paths: roots.clone(),
        exclude_patterns: config.exclude_patterns.clone(),
        include_extensions: config.include_extensions.clone(),
        max_file_size: Some(config.max_file_size),
        respect_gitignore: config.respect_gitignore,
        ..Default::default()
    };
    let files: Vec<PathBuf> = FileDiscoveryIterator::new(&discovery).collect();
    let mut engine = SearchEngine::new();
    for r in &roots {
        engine.add_root_path(Path::new(r));
    }
    let t = Instant::now();
    build(&mut engine, &files, &config);
    eprintln!("indexed {} files in {:.1?}", files.len(), t.elapsed());

    let page = args.page;
    let key =
        |m: &fast_code_search::search::engine::SearchMatch| (m.file_path.clone(), m.line_number);
    println!("query\tcandidates\tmatches_total\tranked_ms\tfirst_ms\tranked_top_overlap\tfirst_top_overlap");
    for q in [
        "return",
        "self",
        "import",
        "Config",
        "async fn",
        "HttpResponse",
        "QuerySet",
        "unwrap()",
        "def test_",
        "TODO",
        "fn",
        "xq9zv_nothing",
    ] {
        let exhaustive = SearchLimits::new(page).with_match_budget(usize::MAX);
        let (top, info) = engine.search_ranked_with_limits(q, exhaustive, RankMode::Full);
        let top: HashSet<_> = top.iter().map(key).collect();
        let total = info.total_matches.unwrap_or(0);
        let run = |limits: SearchLimits| {
            let (r, info) = engine.search_ranked_with_limits(q, limits.clone(), RankMode::Full);
            let times: Vec<Duration> = (0..args.iterations)
                .map(|_| {
                    let t = Instant::now();
                    let _ = engine.search_ranked_with_limits(q, limits.clone(), RankMode::Full);
                    t.elapsed()
                })
                .collect();
            let overlap = r.iter().filter(|m| top.contains(&key(m))).count();
            (median(times), info.candidates_searched, overlap, r.len())
        };
        let (rt, rc, ro, rn) = run(SearchLimits::new(page));
        // Stop as soon as the page is full: the budget is the page.
        let (ft, fc, fo, fnn) = run(SearchLimits::new(page).with_match_budget(page));
        let _ = (rc, fc);
        println!(
            "{q}\t{}\t{total}\t{:.2}\t{:.2}\t{ro}/{rn}\t{fo}/{fnn}",
            info.total_candidates,
            rt.as_secs_f64() * 1e3,
            ft.as_secs_f64() * 1e3,
        );
    }
}
