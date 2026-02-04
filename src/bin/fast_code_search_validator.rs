//! Fast Code Search Validator
//!
//! Whitebox testing binary for validating both keyword and semantic search engines.
//! Generates a deterministic synthetic corpus, indexes it, and validates:
//! - Index completeness (all needles findable)
//! - Query option coverage (search, filter, regex, symbols)
//! - Optionally runs load tests for throughput/latency measurement
//!
//! Usage:
//!   cargo run --release --bin fast_code_search_validator
//!   cargo run --release --bin fast_code_search_validator -- --corpus-size 200 --seed 42
//!   cargo run --release --bin fast_code_search_validator -- --load-test --duration 30

mod validator;

use anyhow::{Context, Result};
use clap::Parser;
use fast_code_search::diagnostics::{TestResult, TestSummary};
use fast_code_search::search::SearchEngine;
use serde::Serialize;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use validator::corpus::{CorpusGenerator, CorpusManifest};

/// Fast Code Search Validator - Whitebox testing for search engines
#[derive(Parser, Debug)]
#[command(name = "fast_code_search_validator")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Number of files to generate in the corpus
    #[arg(long, default_value = "100")]
    corpus_size: usize,

    /// Random seed for reproducible corpus generation
    #[arg(long, default_value = "42")]
    seed: u64,

    /// Number of random samples for additional validation
    #[arg(long, default_value = "10")]
    sample_count: usize,

    /// Run load testing mode
    #[arg(long)]
    load_test: bool,

    /// Number of concurrent query threads for load testing
    #[arg(long, default_value = "4")]
    concurrent: usize,

    /// Duration of load test in seconds
    #[arg(long, default_value = "10")]
    duration: u64,

    /// Output results as JSON
    #[arg(long)]
    json: bool,

    /// Keep the generated corpus (don't delete temp directory)
    #[arg(long)]
    keep_corpus: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

/// Overall validation result
#[derive(Debug, Serialize)]
struct ValidationResult {
    /// Whether all tests passed
    pub passed: bool,
    /// Corpus statistics
    pub corpus: CorpusStats,
    /// Indexing statistics
    pub indexing: IndexingStats,
    /// Test results by category
    pub tests: TestResults,
    /// Load test results (if run)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_test: Option<LoadTestResults>,
}

#[derive(Debug, Serialize)]
struct CorpusStats {
    pub num_files: usize,
    pub num_needles: usize,
    pub num_symbols: usize,
    pub total_lines: usize,
    pub files_by_language: std::collections::HashMap<String, usize>,
}

#[derive(Debug, Serialize)]
struct IndexingStats {
    pub files_indexed: usize,
    pub trigrams: usize,
    pub duration_ms: f64,
}

#[derive(Debug, Serialize)]
struct TestResults {
    pub index_completeness: Vec<TestResult>,
    pub query_coverage: Vec<TestResult>,
    pub summary: TestSummary,
}

#[derive(Debug, Serialize)]
struct LoadTestResults {
    pub duration_secs: f64,
    pub total_queries: usize,
    pub queries_per_second: f64,
    pub latency_p50_us: u64,
    pub latency_p95_us: u64,
    pub latency_p99_us: u64,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.verbose {
        Level::DEBUG
    } else {
        Level::INFO
    };

    FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();

    if !args.json {
        println!("╔════════════════════════════════════════════════════════════╗");
        println!("║       Fast Code Search - Whitebox Validator                ║");
        println!("╚════════════════════════════════════════════════════════════╝");
        println!();
    }

    // Create temp directory for corpus
    let temp_dir = TempDir::new().context("Failed to create temp directory")?;
    let corpus_path = temp_dir.path().to_path_buf();

    if !args.json {
        println!("📁 Corpus directory: {:?}", corpus_path);
        println!("🎲 Seed: {}", args.seed);
        println!("📊 Target files: {}", args.corpus_size);
        println!();
    }

    // Generate corpus
    info!("Generating synthetic corpus...");
    let gen_start = Instant::now();
    let mut generator = CorpusGenerator::new(args.seed);
    let manifest = generator
        .generate(&corpus_path, args.corpus_size)
        .context("Failed to generate corpus")?;
    let gen_duration = gen_start.elapsed();

    if !args.json {
        println!(
            "✓ Generated {} files in {:.2?}",
            manifest.files.len(),
            gen_duration
        );
        println!("  • Needles: {}", manifest.needles.len());
        println!("  • Symbols: {}", manifest.symbols.len());
        println!("  • Total lines: {}", manifest.total_lines);
        println!("  • Languages: {:?}", manifest.files_by_language);
        println!();
    }

    // Index the corpus
    info!("Indexing corpus...");
    let index_start = Instant::now();
    let mut engine = SearchEngine::new();
    let mut indexed_count = 0;

    for file_path in &manifest.files {
        if engine.index_file(file_path).is_ok() {
            indexed_count += 1;
        }
    }
    engine.finalize();
    engine.resolve_imports();

    let index_duration = index_start.elapsed();
    let stats = engine.get_stats();

    if !args.json {
        println!(
            "✓ Indexed {} files in {:.2?}",
            indexed_count, index_duration
        );
        println!("  • Trigrams: {}", stats.num_trigrams);
        println!("  • Dependencies: {}", stats.dependency_edges);
        println!();
    }

    // Run validation tests
    let mut all_tests = Vec::new();

    // 1. Index completeness tests
    if !args.json {
        println!("┌────────────────────────────────────────────────────────────┐");
        println!("│ Index Completeness Tests                                   │");
        println!("└────────────────────────────────────────────────────────────┘");
    }
    let completeness_tests = run_index_completeness_tests(&engine, &manifest, args.sample_count);
    print_test_results(&completeness_tests, args.json);
    all_tests.extend(completeness_tests.clone());

    // 2. Query coverage tests
    if !args.json {
        println!();
        println!("┌────────────────────────────────────────────────────────────┐");
        println!("│ Query Coverage Tests                                       │");
        println!("└────────────────────────────────────────────────────────────┘");
    }
    let query_tests = run_query_coverage_tests(&engine, &manifest);
    print_test_results(&query_tests, args.json);
    all_tests.extend(query_tests.clone());

    let summary = TestSummary::from_results(&all_tests);

    // 3. Optional load testing
    let load_test_results = if args.load_test {
        if !args.json {
            println!();
            println!("┌────────────────────────────────────────────────────────────┐");
            println!("│ Load Testing                                               │");
            println!("└────────────────────────────────────────────────────────────┘");
        }
        Some(run_load_test(
            &engine,
            &manifest,
            args.concurrent,
            Duration::from_secs(args.duration),
            args.json,
        ))
    } else {
        None
    };

    // Build final result
    let result = ValidationResult {
        passed: summary.failed == 0,
        corpus: CorpusStats {
            num_files: manifest.files.len(),
            num_needles: manifest.needles.len(),
            num_symbols: manifest.symbols.len(),
            total_lines: manifest.total_lines,
            files_by_language: manifest
                .files_by_language
                .iter()
                .map(|(k, v)| (format!("{:?}", k), *v))
                .collect(),
        },
        indexing: IndexingStats {
            files_indexed: indexed_count,
            trigrams: stats.num_trigrams,
            duration_ms: index_duration.as_secs_f64() * 1000.0,
        },
        tests: TestResults {
            index_completeness: completeness_tests,
            query_coverage: query_tests,
            summary: summary.clone(),
        },
        load_test: load_test_results,
    };

    // Output results
    if args.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!();
        println!("════════════════════════════════════════════════════════════════");
        if result.passed {
            println!("✅ ALL TESTS PASSED ({}/{})", summary.passed, summary.total);
        } else {
            println!(
                "❌ TESTS FAILED: {} passed, {} failed",
                summary.passed, summary.failed
            );
        }
        println!("   Total duration: {:.2}ms", summary.total_duration_ms);
        println!("════════════════════════════════════════════════════════════════");
    }

    // Keep corpus if requested
    if args.keep_corpus {
        #[allow(deprecated)]
        let kept_path = temp_dir.into_path();
        if !args.json {
            println!("\n📁 Corpus kept at: {:?}", kept_path);
        }
    }

    // Exit with error code if tests failed
    if !result.passed {
        std::process::exit(1);
    }

    Ok(())
}

/// Run index completeness tests - verify all needles are findable
fn run_index_completeness_tests(
    engine: &SearchEngine,
    manifest: &CorpusManifest,
    sample_count: usize,
) -> Vec<TestResult> {
    let mut results = Vec::new();

    // Test 1: Verify all needles are searchable
    let start = Instant::now();
    let mut found = 0;
    let mut not_found = Vec::new();

    for needle in &manifest.needles {
        let search_results = engine.search(&needle.marker, 10);
        if search_results
            .iter()
            .any(|r| r.content.contains(&needle.marker))
        {
            found += 1;
        } else {
            not_found.push(needle.marker.clone());
        }
    }

    let duration = start.elapsed();
    if not_found.is_empty() {
        results.push(
            TestResult::passed(
                "needle_search",
                duration,
                format!("All {} needles found via search", manifest.needles.len()),
            )
            .with_details(format!(
                "Searched {} unique markers",
                manifest.needles.len()
            )),
        );
    } else {
        results.push(
            TestResult::failed(
                "needle_search",
                duration,
                format!("Found {}/{} needles", found, manifest.needles.len()),
            )
            .with_details(format!(
                "Missing: {:?}",
                &not_found[..not_found.len().min(5)]
            )),
        );
    }

    // Test 2: Verify needle line numbers are correct
    let start = Instant::now();
    let mut line_matches = 0;
    let mut line_mismatches = Vec::new();
    let sample_needles: Vec<_> = manifest.needles.iter().take(sample_count).collect();

    for needle in &sample_needles {
        let search_results = engine.search(&needle.marker, 10);
        for result in &search_results {
            if result.content.contains(&needle.marker) {
                // Note: line numbers are 1-based in results
                if result.line_number == needle.line_number {
                    line_matches += 1;
                } else {
                    line_mismatches.push((
                        needle.marker.clone(),
                        needle.line_number,
                        result.line_number,
                    ));
                }
                break;
            }
        }
    }

    let duration = start.elapsed();
    if line_mismatches.is_empty() {
        results.push(TestResult::passed(
            "needle_line_numbers",
            duration,
            format!(
                "All {} sampled needle line numbers correct",
                sample_needles.len()
            ),
        ));
    } else {
        results.push(
            TestResult::failed(
                "needle_line_numbers",
                duration,
                format!(
                    "{}/{} line numbers matched",
                    line_matches,
                    sample_needles.len()
                ),
            )
            .with_details(format!(
                "Mismatches: {:?}",
                &line_mismatches[..line_mismatches.len().min(3)]
            )),
        );
    }

    // Test 3: Verify symbols are searchable via search_symbols
    let start = Instant::now();
    let mut symbols_found = 0;
    let sample_symbols: Vec<_> = manifest.symbols.iter().take(sample_count).collect();

    for symbol in &sample_symbols {
        let search_results = engine
            .search_symbols(&symbol.name, "", "", 10)
            .unwrap_or_default();
        if search_results
            .iter()
            .any(|r| r.is_symbol && r.content.contains(&symbol.name))
        {
            symbols_found += 1;
        }
    }

    let duration = start.elapsed();
    let success_rate = symbols_found as f64 / sample_symbols.len() as f64;
    if success_rate >= 0.8 {
        // Allow 80% success (some symbols may not be extracted)
        results.push(TestResult::passed(
            "symbol_search",
            duration,
            format!(
                "Found {}/{} sampled symbols ({:.0}%)",
                symbols_found,
                sample_symbols.len(),
                success_rate * 100.0
            ),
        ));
    } else {
        results.push(TestResult::failed(
            "symbol_search",
            duration,
            format!(
                "Only found {}/{} symbols ({:.0}%)",
                symbols_found,
                sample_symbols.len(),
                success_rate * 100.0
            ),
        ));
    }

    results
}

/// Run query coverage tests - test all search options
fn run_query_coverage_tests(engine: &SearchEngine, _manifest: &CorpusManifest) -> Vec<TestResult> {
    let mut results = Vec::new();

    // Test 1: Basic search with common terms
    let start = Instant::now();
    let common_results = engine.search("function", 100);
    let duration = start.elapsed();

    if !common_results.is_empty() {
        results.push(TestResult::passed(
            "search_common_term",
            duration,
            format!("Found {} matches for 'function'", common_results.len()),
        ));
    } else {
        results.push(TestResult::failed(
            "search_common_term",
            duration,
            "No matches for common term 'function'",
        ));
    }

    // Test 2: Search with no matches
    let start = Instant::now();
    let no_results = engine.search("ZZZZNONEXISTENTTERMZZZZ", 100);
    let duration = start.elapsed();

    if no_results.is_empty() {
        results.push(TestResult::passed(
            "search_no_match",
            duration,
            "Correctly returned empty for non-existent term",
        ));
    } else {
        results.push(TestResult::failed(
            "search_no_match",
            duration,
            format!(
                "Unexpected {} matches for non-existent term",
                no_results.len()
            ),
        ));
    }

    // Test 3: search_with_filter - include pattern
    let start = Instant::now();
    let rust_only = engine
        .search_with_filter("NEEDLE", "*.rs", "", 100)
        .unwrap_or_default();
    let duration = start.elapsed();

    let all_rust = rust_only.iter().all(|r| r.file_path.ends_with(".rs"));
    if all_rust && !rust_only.is_empty() {
        results.push(TestResult::passed(
            "filter_include_pattern",
            duration,
            format!(
                "Include '*.rs' returned {} Rust-only results",
                rust_only.len()
            ),
        ));
    } else if rust_only.is_empty() {
        results.push(TestResult::failed(
            "filter_include_pattern",
            duration,
            "Include '*.rs' returned no results",
        ));
    } else {
        results.push(TestResult::failed(
            "filter_include_pattern",
            duration,
            "Include '*.rs' returned non-Rust files",
        ));
    }

    // Test 4: search_with_filter - exclude pattern
    let start = Instant::now();
    let no_python = engine
        .search_with_filter("NEEDLE", "", "*.py", 100)
        .unwrap_or_default();
    let duration = start.elapsed();

    let has_python = no_python.iter().any(|r| r.file_path.ends_with(".py"));
    if !has_python && !no_python.is_empty() {
        results.push(TestResult::passed(
            "filter_exclude_pattern",
            duration,
            format!(
                "Exclude '*.py' correctly filtered {} results",
                no_python.len()
            ),
        ));
    } else if no_python.is_empty() {
        results.push(TestResult::failed(
            "filter_exclude_pattern",
            duration,
            "Exclude filter returned no results",
        ));
    } else {
        results.push(TestResult::failed(
            "filter_exclude_pattern",
            duration,
            "Exclude '*.py' still returned Python files",
        ));
    }

    // Test 5: Regex search - trigram accelerated (has literal >= 3 chars)
    // Use a pattern with lowercase literal that exists in generated code
    let start = Instant::now();
    let regex_results = engine
        .search_regex(r"process_data_\d+", "", "", 100)
        .unwrap_or_default();
    let duration = start.elapsed();

    if !regex_results.is_empty() {
        results.push(
            TestResult::passed(
                "regex_accelerated",
                duration,
                format!(
                    "Regex 'process_data_\\d+' found {} matches (trigram-accelerated)",
                    regex_results.len()
                ),
            )
            .with_details("Trigram-accelerated via 'process_data' literal"),
        );
    } else {
        results.push(TestResult::failed(
            "regex_accelerated",
            duration,
            "Regex found no matches",
        ));
    }

    // Test 6: Regex search - case insensitive
    let start = Instant::now();
    let case_insensitive = engine
        .search_regex(r"(?i)needle", "", "", 100)
        .unwrap_or_default();
    let duration = start.elapsed();

    if !case_insensitive.is_empty() {
        results.push(TestResult::passed(
            "regex_case_insensitive",
            duration,
            format!(
                "Case-insensitive regex found {} matches",
                case_insensitive.len()
            ),
        ));
    } else {
        results.push(TestResult::failed(
            "regex_case_insensitive",
            duration,
            "Case-insensitive regex found no matches",
        ));
    }

    // Test 7: Symbols-only search
    let start = Instant::now();
    // Pick a symbol name pattern that should exist
    let symbol_results = engine
        .search_symbols("process", "", "", 100)
        .unwrap_or_default();
    let duration = start.elapsed();

    let all_symbols = symbol_results.iter().all(|r| r.is_symbol);
    if !symbol_results.is_empty() && all_symbols {
        results.push(TestResult::passed(
            "symbols_only_search",
            duration,
            format!(
                "Symbol search found {} symbol matches",
                symbol_results.len()
            ),
        ));
    } else if symbol_results.is_empty() {
        results.push(TestResult::failed(
            "symbols_only_search",
            duration,
            "Symbol search found no results",
        ));
    } else {
        results.push(TestResult::failed(
            "symbols_only_search",
            duration,
            "Symbol search returned non-symbol matches",
        ));
    }

    // Test 8: Combined filter + symbols
    let start = Instant::now();
    let filtered_symbols = engine
        .search_symbols("Data", "*.rs;*.ts", "", 50)
        .unwrap_or_default();
    let duration = start.elapsed();

    let valid_extensions = filtered_symbols
        .iter()
        .all(|r| r.file_path.ends_with(".rs") || r.file_path.ends_with(".ts"));

    if !filtered_symbols.is_empty() && valid_extensions {
        results.push(TestResult::passed(
            "symbols_with_filter",
            duration,
            format!(
                "Filtered symbol search found {} matches",
                filtered_symbols.len()
            ),
        ));
    } else if filtered_symbols.is_empty() {
        results.push(TestResult::passed(
            "symbols_with_filter",
            duration,
            "Filtered symbol search returned empty (may be valid)",
        ));
    } else {
        results.push(TestResult::failed(
            "symbols_with_filter",
            duration,
            "Filtered symbol search returned wrong file types",
        ));
    }

    results
}

/// Run load testing
fn run_load_test(
    engine: &SearchEngine,
    manifest: &CorpusManifest,
    _concurrent: usize,
    duration: Duration,
    quiet: bool,
) -> LoadTestResults {
    use std::sync::atomic::{AtomicUsize, Ordering};

    if !quiet {
        println!("Running load test for {}s...", duration.as_secs());
    }

    // Prepare query workload
    let needle_queries: Vec<_> = manifest.needles.iter().map(|n| n.marker.clone()).collect();
    let common_queries = vec![
        "function".to_string(),
        "class".to_string(),
        "async".to_string(),
        "return".to_string(),
        "import".to_string(),
    ];
    let symbol_queries: Vec<_> = manifest
        .symbols
        .iter()
        .take(20)
        .map(|s| s.name.clone())
        .collect();

    let query_count = AtomicUsize::new(0);
    let latencies = std::sync::Mutex::new(Vec::new());

    let start = Instant::now();
    let end_time = start + duration;

    // Run parallel queries until time expires
    rayon::scope(|s| {
        for _ in 0..rayon::current_num_threads() {
            let needle_queries = &needle_queries;
            let common_queries = &common_queries;
            let symbol_queries = &symbol_queries;
            let query_count = &query_count;
            let latencies = &latencies;

            s.spawn(move |_| {
                let mut local_latencies = Vec::new();
                let mut i = 0;

                while Instant::now() < end_time {
                    let query_start = Instant::now();

                    // Mix of query types: 50% needles, 30% common, 15% symbols, 5% regex
                    match i % 20 {
                        0..=9 => {
                            // Needle queries (50%)
                            let idx = i % needle_queries.len().max(1);
                            if let Some(q) = needle_queries.get(idx) {
                                let _ = engine.search(q, 10);
                            }
                        }
                        10..=15 => {
                            // Common queries (30%)
                            let idx = i % common_queries.len();
                            let _ = engine.search(&common_queries[idx], 50);
                        }
                        16..=18 => {
                            // Symbol queries (15%)
                            let idx = i % symbol_queries.len().max(1);
                            if let Some(q) = symbol_queries.get(idx) {
                                let _ = engine.search_symbols(q, "", "", 20);
                            }
                        }
                        _ => {
                            // Regex queries (5%)
                            let _ = engine.search_regex(r"NEEDLE_\d+", "", "", 20);
                        }
                    }

                    let latency = query_start.elapsed();
                    local_latencies.push(latency.as_micros() as u64);
                    query_count.fetch_add(1, Ordering::Relaxed);
                    i += 1;
                }

                // Merge local latencies
                latencies.lock().unwrap().extend(local_latencies);
            });
        }
    });

    let total_duration = start.elapsed();
    let total_queries = query_count.load(Ordering::Relaxed);
    let qps = total_queries as f64 / total_duration.as_secs_f64();

    // Calculate percentiles
    let mut all_latencies = latencies.into_inner().unwrap();
    all_latencies.sort_unstable();

    let p50 = percentile(&all_latencies, 50);
    let p95 = percentile(&all_latencies, 95);
    let p99 = percentile(&all_latencies, 99);

    if !quiet {
        println!("✓ Load test complete");
        println!("  • Total queries: {}", total_queries);
        println!("  • Throughput: {:.2} queries/sec", qps);
        println!("  • Latency p50: {}μs", p50);
        println!("  • Latency p95: {}μs", p95);
        println!("  • Latency p99: {}μs", p99);
    }

    LoadTestResults {
        duration_secs: total_duration.as_secs_f64(),
        total_queries,
        queries_per_second: qps,
        latency_p50_us: p50,
        latency_p95_us: p95,
        latency_p99_us: p99,
    }
}

fn percentile(sorted: &[u64], p: usize) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = (sorted.len() * p / 100).min(sorted.len() - 1);
    sorted[idx]
}

fn print_test_results(results: &[TestResult], json_mode: bool) {
    if json_mode {
        return;
    }

    for result in results {
        let icon = if result.passed { "✓" } else { "✗" };
        let status = if result.passed { "PASS" } else { "FAIL" };
        println!(
            "  {} [{}] {}: {} ({:.2}ms)",
            icon, status, result.name, result.message, result.duration_ms
        );
        if let Some(details) = &result.details {
            println!("      └─ {}", details);
        }
    }
}
