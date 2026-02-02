//! Criterion benchmarks for index persistence and loading
//!
//! Run with: cargo bench --bench persistence_benchmark
//! View HTML report: target/criterion/report/index.html
//!
//! These benchmarks measure the performance of saving and loading indexes
//! from disk, focusing on the optimizations for parallel deserialization.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use fast_code_search::search::SearchEngine;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;

/// Generate synthetic source files for benchmarking
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

/// Benchmark saving an index to disk
fn bench_save_index(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_save");
    group.measurement_time(Duration::from_secs(5));

    for num_files in [100, 500, 1000] {
        let (engine, temp_dir) = setup_engine_with_files(num_files, 50);
        let index_path = temp_dir.path().join("index.bin");

        // Create a minimal config
        let config = fast_code_search::config::IndexerConfig {
            paths: vec![temp_dir.path().to_string_lossy().to_string()],
            ..Default::default()
        };

        group.throughput(Throughput::Elements(num_files as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_files),
            &num_files,
            |b, _| {
                b.iter(|| {
                    engine
                        .save_index(black_box(&index_path), black_box(&config))
                        .expect("Failed to save index");
                });
            },
        );
    }

    group.finish();
}

/// Benchmark loading an index from disk
fn bench_load_index(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_load");
    group.measurement_time(Duration::from_secs(5));

    for num_files in [100, 500, 1000] {
        // Setup: create and save an index
        let (engine, temp_dir) = setup_engine_with_files(num_files, 50);
        let index_path = temp_dir.path().join("index.bin");

        let config = fast_code_search::config::IndexerConfig {
            paths: vec![temp_dir.path().to_string_lossy().to_string()],
            ..Default::default()
        };

        engine
            .save_index(&index_path, &config)
            .expect("Failed to save index");

        group.throughput(Throughput::Elements(num_files as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_files),
            &num_files,
            |b, _| {
                b.iter(|| {
                    let mut engine = SearchEngine::new();
                    engine
                        .load_index(black_box(&index_path))
                        .expect("Failed to load index");
                });
            },
        );
    }

    group.finish();
}

/// Benchmark the trigram deserialization specifically
fn bench_trigram_deserialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("trigram_deserialization");
    group.measurement_time(Duration::from_secs(5));

    for num_files in [100, 500, 1000] {
        // Setup: create and save an index
        let (engine, temp_dir) = setup_engine_with_files(num_files, 50);
        let index_path = temp_dir.path().join("index.bin");

        let config = fast_code_search::config::IndexerConfig {
            paths: vec![temp_dir.path().to_string_lossy().to_string()],
            ..Default::default()
        };

        engine
            .save_index(&index_path, &config)
            .expect("Failed to save index");

        // Load the persisted index
        use fast_code_search::index::PersistedIndex;
        let persisted = PersistedIndex::load(&index_path).expect("Failed to load persisted index");

        group.throughput(Throughput::Elements(
            persisted.trigram_index.trigram_to_docs.len() as u64,
        ));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_files),
            &num_files,
            |b, _| {
                b.iter(|| {
                    persisted
                        .restore_trigram_index()
                        .expect("Failed to restore trigram index");
                });
            },
        );
    }

    group.finish();
}

/// Benchmark file staleness checking
fn bench_file_staleness_check(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_staleness_check");
    group.measurement_time(Duration::from_secs(5));

    for num_files in [100, 500, 1000] {
        // Setup: create and save an index
        let (engine, temp_dir) = setup_engine_with_files(num_files, 50);
        let index_path = temp_dir.path().join("index.bin");

        let config = fast_code_search::config::IndexerConfig {
            paths: vec![temp_dir.path().to_string_lossy().to_string()],
            ..Default::default()
        };

        engine
            .save_index(&index_path, &config)
            .expect("Failed to save index");

        // Load the persisted index
        use fast_code_search::index::PersistedIndex;
        let persisted = PersistedIndex::load(&index_path).expect("Failed to load persisted index");

        group.throughput(Throughput::Elements(num_files as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_files),
            &num_files,
            |b, _| {
                b.iter(|| {
                    use fast_code_search::index::persistence::batch_check_files;
                    batch_check_files(black_box(&persisted.files), &[]);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_save_index,
    bench_load_index,
    bench_trigram_deserialization,
    bench_file_staleness_check
);
criterion_main!(benches);
