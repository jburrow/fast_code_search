//! Allocation profiler for search hot path
//!
//! Run with: cargo run --example profile_search --release
//!
//! Outputs dhat-heap.json which can be viewed at:
//! https://nnethercote.github.io/dh_view/dh_view.html

use fast_code_search::search::SearchEngine;
use std::path::PathBuf;

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() {
    // Start heap profiler
    let _profiler = dhat::Profiler::new_heap();

    // Setup: create engine with test files
    let mut engine = SearchEngine::new();

    // Index test corpus if it exists, otherwise create synthetic data
    let test_corpus = PathBuf::from("test_corpus");
    let mut indexed_count = 0;

    if test_corpus.exists() {
        println!("Checking test_corpus...");
        for entry in walkdir::WalkDir::new(&test_corpus)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().is_file()
                    && e.path().extension().map_or(false, |ext| {
                        matches!(ext.to_str(), Some("rs" | "py" | "js" | "ts" | "tsx"))
                    })
            })
            .take(500)
        {
            if engine.index_file(entry.path()).is_ok() {
                indexed_count += 1;
            }
        }
    }

    // If no files found, create synthetic data
    if indexed_count == 0 {
        println!("Creating synthetic files for profiling...");
        let temp_dir = std::env::temp_dir().join("fcs_profile");
        let _ = std::fs::remove_dir_all(&temp_dir); // Clean up previous run
        std::fs::create_dir_all(&temp_dir).unwrap();

        for i in 0..200 {
            let subdir = temp_dir.join(format!("module_{}", i / 20));
            std::fs::create_dir_all(&subdir).unwrap();
            let path = subdir.join(format!("file_{}.rs", i));
            let content = format!(
                "// File {i}\n\
                use std::collections::HashMap;\n\
                use std::sync::Arc;\n\n\
                pub struct DataProcessor{i} {{\n\
                    data: Vec<u8>,\n\
                    cache: HashMap<String, String>,\n\
                }}\n\n\
                impl DataProcessor{i} {{\n\
                    pub fn new() -> Self {{\n\
                        Self {{ data: Vec::new(), cache: HashMap::new() }}\n\
                    }}\n\n\
                    pub fn process_data(&self, input: &str) -> String {{\n\
                        let mut result = String::new();\n\
                        for line in input.lines() {{\n\
                            if line.contains(\"pattern\") {{\n\
                                result.push_str(line);\n\
                            }}\n\
                        }}\n\
                        result\n\
                    }}\n\
                }}\n"
            );
            std::fs::write(&path, &content).unwrap();
            if engine.index_file(&path).is_ok() {
                indexed_count += 1;
            }
        }
    }

    engine.finalize();
    println!("Indexed {} files", indexed_count);

    // Profile: run searches (this is what we want to measure)
    println!("\nRunning searches (profiling allocations)...");

    for _ in 0..100 {
        // Common query
        let _ = engine.search("result", 100);

        // Case-insensitive
        let _ = engine.search("RESULT", 100);

        // Rare query
        let _ = engine.search("HashMap", 100);
    }

    println!("\nProfile complete. View dhat-heap.json at:");
    println!("https://nnethercote.github.io/dh_view/dh_view.html");

    // Profiler drops here and writes dhat-heap.json
}
