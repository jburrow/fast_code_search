//! Manual test to demonstrate index loading performance
//!
//! Run with: cargo run --release --example test_index_loading

use fast_code_search::config::IndexerConfig;
use fast_code_search::search::SearchEngine;
use std::time::Instant;
use tempfile::TempDir;

fn generate_test_files(temp_dir: &TempDir, num_files: usize) {
    println!("Generating {} test files...", num_files);
    for i in 0..num_files {
        let subdir = temp_dir.path().join(format!("module_{}", i / 10));
        std::fs::create_dir_all(&subdir).unwrap();

        let file_path = subdir.join(format!("file_{}.rs", i));
        let content = format!(
            "// File {}\n\
             use std::collections::HashMap;\n\
             \n\
             pub fn process_data_{}(input: &str) -> Result<String, Error> {{\n\
             \tlet mut result = String::new();\n\
             \tfor line in input.lines() {{\n\
             \t\tif line.contains(\"pattern\") {{\n\
             \t\t\tresult.push_str(line);\n\
             \t\t}}\n\
             \t}}\n\
             \tOk(result)\n\
             }}\n\
             \n\
             pub struct DataProcessor{} {{\n\
             \tdata: Vec<u8>,\n\
             \tcache: HashMap<String, String>,\n\
             }}\n",
            i, i, i
        );
        std::fs::write(&file_path, content).unwrap();
    }
    println!("Files generated successfully");
}

fn main() {
    // Test with different file counts
    for num_files in [100, 500, 1000] {
        println!("\n{}", "=".repeat(60));
        println!("Testing with {} files", num_files);
        println!("{}", "=".repeat(60));

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        generate_test_files(&temp_dir, num_files);

        // Create and populate search engine
        println!("\nIndexing files...");
        let start = Instant::now();
        let mut engine = SearchEngine::new();

        // Index all files
        for entry in walkdir::WalkDir::new(temp_dir.path())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
        {
            let _ = engine.index_file(entry.path());
        }
        engine.finalize();

        let index_time = start.elapsed();
        println!("Initial indexing took: {:?}", index_time);
        println!("Files indexed: {}", engine.file_store.len());
        println!("Trigrams: {}", engine.trigram_index.num_trigrams());

        // Save index
        let index_path = temp_dir.path().join("index.bin");
        let config = IndexerConfig {
            paths: vec![temp_dir.path().to_string_lossy().to_string()],
            ..Default::default()
        };

        println!("\nSaving index to disk...");
        let start = Instant::now();
        engine
            .save_index(&index_path, &config)
            .expect("Failed to save index");
        let save_time = start.elapsed();
        println!("Save took: {:?}", save_time);

        // Get file size
        let file_size = std::fs::metadata(&index_path).unwrap().len();
        println!("Index file size: {} KB", file_size / 1024);

        // Load index - this is what we optimized!
        println!("\nLoading index from disk (OPTIMIZED)...");
        let start = Instant::now();
        let mut engine2 = SearchEngine::new();
        let stale_files = engine2
            .load_index(&index_path)
            .expect("Failed to load index");
        let load_time = start.elapsed();

        println!("Load took: {:?}", load_time);
        println!("Files loaded: {}", engine2.file_store.len());
        println!("Stale files: {}", stale_files.len());
        println!("Trigrams: {}", engine2.trigram_index.num_trigrams());

        // Calculate speedup vs naive sequential approach
        println!("\nPerformance Summary:");
        println!("  Load time: {:?}", load_time);
        println!(
            "  Speedup vs initial index: {:.2}x faster",
            index_time.as_secs_f64() / load_time.as_secs_f64()
        );

        // Test a search to ensure it works
        let results = engine2.search("process_data", 10);
        println!(
            "  Search test: found {} results for 'process_data'",
            results.len()
        );
    }

    println!("\n{}", "=".repeat(60));
    println!("Performance test complete!");
    println!("{}", "=".repeat(60));
}
