//! What a `rdeps:` filter adds to a search, on a saved index: the same
//! query (one that matches nothing) with and without `rdeps:FILE:1` and
//! `rdeps:FILE:*`, for 18 files spread over the index.
//!
//! ```sh
//! cargo run --release --example rdeps_timing -- INDEX ROOT...
//! ```
use fast_code_search::config::IndexerConfig;
use fast_code_search::search::engine::SearchEngine;
use fast_code_search::search::{SearchMode, SearchRequest, SearchSettings};
use std::path::Path;
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let index = &args[0];
    let roots: Vec<String> = args[1..].to_vec();
    let config = IndexerConfig {
        paths: roots.clone(),
        ..Default::default()
    };
    let mut engine = SearchEngine::new();
    for r in &roots {
        engine.add_root_path(Path::new(r));
    }
    let t = Instant::now();
    engine.load_index_with_reconciliation(Path::new(index), &config)?;
    engine.finalize();
    let n = engine.get_stats().num_files;
    eprintln!("loaded {n} files in {:.0}s", t.elapsed().as_secs_f64());
    let mut anchors = Vec::new();
    let mut id = 7u32;
    while anchors.len() < 40 && (id as usize) < n {
        if !engine.dependency_index.get_dependents(id).is_empty() {
            if let Some(p) = engine.display_path(id) {
                anchors.push(p.into_owned());
            }
        }
        id += 7919;
    }
    let settings = SearchSettings::default();
    let mut times = Vec::new();
    for a in &anchors {
        for hops in ["1", "*"] {
            let req = SearchRequest {
                query: "zzqqxxnotthere".into(),
                mode: SearchMode::Text,
                max_results: 20,
                rdeps: vec![format!("{a}:{hops}")],
                ..Default::default()
            };
            let limits = req.limits(&settings, None).map_err(anyhow::Error::msg)?;
            let t = Instant::now();
            let r = engine.run(&req, limits);
            times.push(t.elapsed().as_secs_f64() * 1e3);
            if let Err(e) = r {
                eprintln!("{a}: {e}");
            }
        }
    }
    let mut plain = Vec::new();
    for _ in 0..20 {
        let req = SearchRequest {
            query: "zzqqxxnotthere".into(),
            mode: SearchMode::Text,
            max_results: 20,
            ..Default::default()
        };
        let limits = req.limits(&settings, None).map_err(anyhow::Error::msg)?;
        let t = Instant::now();
        let _ = engine.run(&req, limits);
        plain.push(t.elapsed().as_secs_f64() * 1e3);
    }
    plain.sort_by(f64::total_cmp);
    println!("no scope: median {:.1} ms", plain[10]);
    times.sort_by(f64::total_cmp);
    let p = |q: f64| times[((times.len() - 1) as f64 * q) as usize];
    println!(
        "{} queries: median {:.1} ms, p95 {:.1} ms, max {:.1} ms",
        times.len(),
        p(0.5),
        p(0.95),
        p(1.0)
    );
    Ok(())
}
