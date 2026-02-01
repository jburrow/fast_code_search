//! Criterion benchmarks for search performance
//!
//! Run with: cargo bench
//! View HTML report: target/criterion/report/index.html
//!
//! These benchmarks measure CPU cycles for various search scenarios to help
//! quantify optimization improvements.
//!
//! Optimized for fast iteration:
//! - Uses smaller corpus sizes (50-200 files vs 100-1000)
//! - Shorter measurement times (3s vs 10s)
//! - Reuses engines across related benchmarks where possible

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use fast_code_search::search::SearchEngine;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;
use tempfile::TempDir;

/// Default corpus size - small enough for fast iteration, large enough for meaningful results
const DEFAULT_NUM_FILES: usize = 100;
const DEFAULT_LINES_PER_FILE: usize = 50;

/// Cached engine for benchmarks that don't need varying corpus sizes
static CACHED_ENGINE: OnceLock<(SearchEngine, TempDir)> = OnceLock::new();

fn get_or_create_engine() -> &'static (SearchEngine, TempDir) {
    CACHED_ENGINE.get_or_init(|| setup_engine_with_files(DEFAULT_NUM_FILES, DEFAULT_LINES_PER_FILE))
}

/// Generate synthetic source code for benchmarking
fn generate_source_files(num_files: usize, lines_per_file: usize) -> Vec<(PathBuf, String)> {
    let mut files = Vec::with_capacity(num_files);

    for i in 0..num_files {
        let mut content = String::with_capacity(lines_per_file * 60);

        // Add a file header
        content.push_str(&format!("// File {} - Generated for benchmarking\n", i));
        content.push_str("use std::collections::HashMap;\n");
        content.push_str("use std::sync::Arc;\n\n");

        // Generate some function definitions
        for j in 0..lines_per_file / 10 {
            content.push_str(&format!(
                "pub fn process_data_{}_{i}(input: &str) -> Result<String, Error> {{\n",
                j
            ));
            content.push_str("    let mut result = String::new();\n");
            content.push_str("    for line in input.lines() {\n");
            content.push_str("        if line.contains(\"pattern\") {\n");
            content.push_str("            result.push_str(line);\n");
            content.push_str("        }\n");
            content.push_str("    }\n");
            content.push_str("    Ok(result)\n");
            content.push_str("}\n\n");
        }

        // Add some struct definitions
        content.push_str(&format!(
            "pub struct DataProcessor{} {{\n    data: Vec<u8>,\n    cache: HashMap<String, String>,\n}}\n\n",
            i
        ));

        // Add impl block
        content.push_str(&format!("impl DataProcessor{} {{\n", i));
        content.push_str("    pub fn new() -> Self {\n");
        content.push_str("        Self { data: Vec::new(), cache: HashMap::new() }\n");
        content.push_str("    }\n");
        content.push_str("    \n");
        content.push_str("    pub fn search_internal(&self, query: &str) -> Option<&str> {\n");
        content.push_str("        self.cache.get(query).map(|s| s.as_str())\n");
        content.push_str("    }\n");
        content.push_str("}\n");

        let path = PathBuf::from(format!("src/module_{}/processor_{}.rs", i / 10, i));
        files.push((path, content));
    }

    files
}

/// Create a temp directory with source files and populate a search engine
fn setup_engine_with_files(num_files: usize, lines_per_file: usize) -> (SearchEngine, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let files = generate_source_files(num_files, lines_per_file);

    // Write files to temp directory
    for (rel_path, content) in &files {
        let full_path = temp_dir.path().join(rel_path);
        std::fs::create_dir_all(full_path.parent().unwrap()).unwrap();
        std::fs::write(&full_path, content).unwrap();
    }

    // Index the files
    let mut engine = SearchEngine::new();
    for (rel_path, _) in &files {
        let full_path = temp_dir.path().join(rel_path);
        let _ = engine.index_file(&full_path);
    }
    engine.finalize();

    (engine, temp_dir)
}

/// Benchmark basic text search with varying corpus sizes
fn bench_text_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_search");
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(50);

    // Test different corpus sizes - reduced range for faster iteration
    for num_files in [50, 100, 200] {
        let (engine, _temp_dir) = setup_engine_with_files(num_files, DEFAULT_LINES_PER_FILE);
        let total_lines = num_files * DEFAULT_LINES_PER_FILE;

        group.throughput(Throughput::Elements(total_lines as u64));

        // Common query - appears in most files
        group.bench_with_input(
            BenchmarkId::new("common_query", num_files),
            &engine,
            |b, engine| {
                b.iter(|| black_box(engine.search(black_box("result"), 100)));
            },
        );

        // Rare query - appears in few files
        group.bench_with_input(
            BenchmarkId::new("rare_query", num_files),
            &engine,
            |b, engine| {
                b.iter(|| black_box(engine.search(black_box("DataProcessor0"), 100)));
            },
        );

        // No matches
        group.bench_with_input(
            BenchmarkId::new("no_match", num_files),
            &engine,
            |b, engine| {
                b.iter(|| black_box(engine.search(black_box("xyznonexistent"), 100)));
            },
        );
    }

    group.finish();
}

/// Benchmark regex search - uses cached engine for speed
fn bench_regex_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("regex_search");
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(50);

    let (engine, _temp_dir) = get_or_create_engine();

    // Simple literal regex (should use trigram acceleration)
    group.bench_function("simple_literal", |b| {
        b.iter(|| black_box(engine.search_regex(black_box("process_data"), "", "", 100)));
    });

    // Regex with alternation
    group.bench_function("alternation", |b| {
        b.iter(|| black_box(engine.search_regex(black_box("String|Vec|HashMap"), "", "", 100)));
    });

    // Regex with character class
    group.bench_function("char_class", |b| {
        b.iter(|| black_box(engine.search_regex(black_box("process_[a-z]+"), "", "", 100)));
    });

    // Complex regex (no trigram acceleration possible)
    group.bench_function("no_literal", |b| {
        b.iter(|| black_box(engine.search_regex(black_box(".*data.*"), "", "", 100)));
    });

    group.finish();
}

/// Benchmark filtered search (with path patterns) - uses cached engine
fn bench_filtered_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("filtered_search");
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(50);

    let (engine, _temp_dir) = get_or_create_engine();

    // No filter (baseline)
    group.bench_function("no_filter", |b| {
        b.iter(|| black_box(engine.search_with_filter(black_box("result"), "", "", 100)));
    });

    // Include filter
    group.bench_function("include_filter", |b| {
        b.iter(|| {
            black_box(engine.search_with_filter(black_box("result"), "**/module_1/**", "", 100))
        });
    });

    // Exclude filter
    group.bench_function("exclude_filter", |b| {
        b.iter(|| {
            black_box(engine.search_with_filter(
                black_box("result"),
                "",
                "**/module_0/**;**/module_2/**",
                100,
            ))
        });
    });

    // Both filters
    group.bench_function("include_and_exclude", |b| {
        b.iter(|| {
            black_box(engine.search_with_filter(
                black_box("result"),
                "**/*.rs",
                "**/module_4/**",
                100,
            ))
        });
    });

    group.finish();
}

/// Benchmark case sensitivity impact - uses cached engine
fn bench_case_sensitivity(c: &mut Criterion) {
    let mut group = c.benchmark_group("case_sensitivity");
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(50);

    let (engine, _temp_dir) = get_or_create_engine();

    // Lowercase query (common case)
    group.bench_function("lowercase", |b| {
        b.iter(|| black_box(engine.search(black_box("result"), 100)));
    });

    // Uppercase query (needs case folding)
    group.bench_function("uppercase", |b| {
        b.iter(|| black_box(engine.search(black_box("RESULT"), 100)));
    });

    // Mixed case
    group.bench_function("mixed_case", |b| {
        b.iter(|| black_box(engine.search(black_box("HashMap"), 100)));
    });

    group.finish();
}

/// Benchmark result limit impact - uses cached engine
fn bench_result_limits(c: &mut Criterion) {
    let mut group = c.benchmark_group("result_limits");
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(50);

    let (engine, _temp_dir) = get_or_create_engine();

    // Fewer limit variations for faster benchmarks
    for limit in [10, 100, 500] {
        group.bench_with_input(BenchmarkId::new("limit", limit), &limit, |b, &limit| {
            b.iter(|| black_box(engine.search(black_box("result"), limit)));
        });
    }

    group.finish();
}

/// Benchmark query length impact - uses cached engine
fn bench_query_length(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_length");
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(50);

    let (engine, _temp_dir) = get_or_create_engine();

    // Reduced set of query lengths
    let queries = [
        ("short_2", "fn"),
        ("medium_8", "process_"),
        ("long_16", "process_data_0_0"),
    ];

    for (name, query) in queries {
        group.bench_function(name, |b| {
            b.iter(|| black_box(engine.search(black_box(query), 100)));
        });
    }

    group.finish();
}

/// Benchmark indexing performance
fn bench_indexing(c: &mut Criterion) {
    let mut group = c.benchmark_group("indexing");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(10); // Fewer samples since indexing is I/O bound

    // Smaller file counts for faster iteration
    for num_files in [25, 50, 100] {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let files = generate_source_files(num_files, DEFAULT_LINES_PER_FILE);
        let total_bytes: usize = files.iter().map(|(_, c)| c.len()).sum();

        // Write files to temp directory
        for (rel_path, content) in &files {
            let full_path = temp_dir.path().join(rel_path);
            std::fs::create_dir_all(full_path.parent().unwrap()).unwrap();
            std::fs::write(&full_path, content).unwrap();
        }

        let file_paths: Vec<_> = files
            .iter()
            .map(|(rel_path, _)| temp_dir.path().join(rel_path))
            .collect();

        group.throughput(Throughput::Bytes(total_bytes as u64));

        group.bench_with_input(
            BenchmarkId::new("index_files", num_files),
            &file_paths,
            |b, file_paths| {
                b.iter(|| {
                    let mut engine = SearchEngine::new();
                    for path in file_paths {
                        let _ = engine.index_file(path);
                    }
                    engine.finalize();
                    black_box(engine)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark import resolution strategies
fn bench_import_resolution(c: &mut Criterion) {
    let mut group = c.benchmark_group("import_resolution");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(10);

    for num_files in [50, 100] {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let files = generate_source_files_with_imports(num_files, DEFAULT_LINES_PER_FILE);

        // Write files to temp directory
        for (rel_path, content) in &files {
            let full_path = temp_dir.path().join(rel_path);
            std::fs::create_dir_all(full_path.parent().unwrap()).unwrap();
            std::fs::write(&full_path, content).unwrap();
        }

        let file_paths: Vec<_> = files
            .iter()
            .map(|(rel_path, _)| temp_dir.path().join(rel_path))
            .collect();

        // Benchmark: Index all, then resolve imports at end (old approach)
        group.bench_with_input(
            BenchmarkId::new("batch_resolve", num_files),
            &file_paths,
            |b, file_paths| {
                b.iter(|| {
                    let mut engine = SearchEngine::new();
                    for path in file_paths {
                        let _ = engine.index_file(path);
                    }
                    engine.resolve_imports();
                    engine.finalize();
                    black_box(engine)
                });
            },
        );

        // Benchmark: Incremental resolution every 10 files (simulates per-batch)
        // This is closer to the real-world usage with BATCH_SIZE=500
        group.bench_with_input(
            BenchmarkId::new("incremental_every_10", num_files),
            &file_paths,
            |b, file_paths| {
                b.iter(|| {
                    let mut engine = SearchEngine::new();
                    for (i, path) in file_paths.iter().enumerate() {
                        let _ = engine.index_file(path);
                        // Resolve every 10 files (simulates batch processing)
                        if (i + 1) % 10 == 0 {
                            engine.resolve_imports_incremental();
                        }
                    }
                    engine.resolve_imports(); // Resolve any remaining
                    engine.finalize();
                    black_box(engine)
                });
            },
        );
    }

    group.finish();
}

/// Generate source files with import statements for testing import resolution
fn generate_source_files_with_imports(
    num_files: usize,
    lines_per_file: usize,
) -> Vec<(PathBuf, String)> {
    let mut files = Vec::with_capacity(num_files);

    for i in 0..num_files {
        let mut content = String::with_capacity(lines_per_file * 60);

        // Add imports that reference other files in the codebase
        if i > 0 {
            // Reference earlier files (which will be indexed before this one)
            let import_target = i - 1;
            content.push_str(&format!(
                "use crate::module_{}::processor_{}::DataProcessor{};\n",
                import_target / 10,
                import_target,
                import_target
            ));
        }
        if i + 1 < num_files {
            // Reference later files (which may not be indexed yet)
            let import_target = i + 1;
            content.push_str(&format!(
                "use crate::module_{}::processor_{}::DataProcessor{};\n",
                import_target / 10,
                import_target,
                import_target
            ));
        }

        content.push_str("\nuse std::collections::HashMap;\n");
        content.push_str("use std::sync::Arc;\n\n");

        // Generate some function definitions
        for j in 0..lines_per_file / 10 {
            content.push_str(&format!(
                "pub fn process_data_{}_{i}(input: &str) -> Result<String, Error> {{\n",
                j
            ));
            content.push_str("    let mut result = String::new();\n");
            content.push_str("    for line in input.lines() {\n");
            content.push_str("        if line.contains(\"pattern\") {\n");
            content.push_str("            result.push_str(line);\n");
            content.push_str("        }\n");
            content.push_str("    }\n");
            content.push_str("    Ok(result)\n");
            content.push_str("}\n\n");
        }

        // Add struct definition
        content.push_str(&format!(
            "pub struct DataProcessor{} {{\n    data: Vec<u8>,\n    cache: HashMap<String, String>,\n}}\n\n",
            i
        ));

        let path = PathBuf::from(format!("src/module_{}/processor_{}.rs", i / 10, i));
        files.push((path, content));
    }

    files
}

criterion_group!(
    benches,
    bench_text_search,
    bench_regex_search,
    bench_filtered_search,
    bench_case_sensitivity,
    bench_result_limits,
    bench_query_length,
    bench_indexing,
    bench_import_resolution,
);

criterion_main!(benches);
