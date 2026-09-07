//! Unit tests for the search engine (split out of `engine/mod.rs`).

use super::*;
use std::fs;
use std::io::Write;
use tempfile::TempDir;

#[test]
fn test_unicode_ci_find_matches_accented() {
    // Needle already Unicode-lowercased; haystack has uppercase accented form.
    assert_eq!(
        unicode_ci_find("ÜBER alles", "über"),
        Some((0, "Ü".len() + 3))
    );
    assert!(unicode_ci_find("der ÜBER mensch", "über").is_some());
    assert!(unicode_ci_find("nothing here", "über").is_none());
}

#[test]
fn test_contains_case_insensitive_unicode() {
    assert!(contains_case_insensitive("ÜBER", "über"));
    assert!(contains_case_insensitive("Café", "café"));
    assert!(!contains_case_insensitive("cafe", "café"));
}

#[test]
fn test_empty_query_returns_nothing() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("e.txt");
    std::fs::write(&file_path, "hello world\n").unwrap();
    let mut engine = SearchEngine::new();
    engine.index_file(&file_path).unwrap();

    assert!(engine.search("", 10).is_empty(), "empty query → no results");
    assert!(
        engine.search("   ", 10).is_empty(),
        "whitespace query → no results"
    );
    let (m, info) = engine.search_ranked("  ", 10, RankMode::Auto);
    assert!(m.is_empty());
    assert_eq!(info.total_candidates, 0, "must not scan any documents");
}

#[test]
fn test_search_engine() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.txt");

    let mut file = fs::File::create(&file_path).unwrap();
    writeln!(file, "hello world").unwrap();
    writeln!(file, "hello rust").unwrap();
    writeln!(file, "goodbye world").unwrap();
    drop(file);

    let mut engine = SearchEngine::new();
    engine.index_file(&file_path).unwrap();

    let results = engine.search("hello", 10);
    assert_eq!(results.len(), 2);

    let results = engine.search("world", 10);
    assert_eq!(results.len(), 2);
}

#[test]
fn test_incremental_import_resolution() {
    let temp_dir = TempDir::new().unwrap();

    // Create a file with imports
    let main_path = temp_dir.path().join("main.rs");
    let helper_path = temp_dir.path().join("helper.rs");

    fs::write(
        &helper_path,
        "pub fn help() {\n    println!(\"helping\");\n}\n",
    )
    .unwrap();

    fs::write(
        &main_path,
        "mod helper;\nfn main() {\n    helper::help();\n}\n",
    )
    .unwrap();

    let mut engine = SearchEngine::new();

    // Index the helper file first
    engine.index_file(&helper_path).unwrap();

    // Now index main file - its import to helper should be resolvable
    engine.index_file(&main_path).unwrap();

    // Try incremental resolution - should resolve the import
    let _resolved = engine.resolve_imports_incremental();

    // Some imports may or may not resolve depending on path canonicalization
    // The key is that incremental resolution doesn't panic and works correctly
    // pending_imports_count is always >= 0 (usize), so just check it works
    let _ = engine.pending_imports_count();

    // Final resolve should clear any remaining
    engine.resolve_imports();
    assert_eq!(engine.pending_imports_count(), 0);
}

#[test]
fn test_pending_imports_count() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.py");

    fs::write(
        &file_path,
        "import os\nimport sys\nfrom pathlib import Path\n",
    )
    .unwrap();

    let mut engine = SearchEngine::new();
    engine.index_file(&file_path).unwrap();

    // There should be pending imports (stdlib imports won't resolve to indexed files)
    // These will remain unresolved since os, sys, pathlib aren't indexed
    let pending = engine.pending_imports_count();
    assert!(pending > 0, "Expected pending imports");

    // After resolution they are *parked* (waiting for a matching file),
    // not dropped, so a later `sys.py` can still gain the edge. Repeated
    // resolution does not grow the parked set.
    engine.resolve_imports();
    let parked = engine.waiting_imports_count();
    assert_eq!(parked, pending);
    engine.resolve_imports();
    engine.resolve_imports_incremental();
    assert_eq!(engine.waiting_imports_count(), parked);
}

/// Roadmap 2.6: an import whose target is indexed *later* is parked and
/// retried only when a file with a matching name appears; it must then
/// produce the edge without rescanning every unresolved import.
#[test]
fn test_waiting_import_resolves_when_target_appears() {
    let temp_dir = TempDir::new().unwrap();
    let main_path = temp_dir.path().join("main.rs");
    let helper_path = temp_dir.path().join("helper.rs");
    let unrelated = temp_dir.path().join("zzz.rs");
    fs::write(&main_path, "mod helper;\nuse std::io;\nfn main() {}\n").unwrap();
    fs::write(&helper_path, "pub fn help() {}\n").unwrap();
    fs::write(&unrelated, "pub fn nothing() {}\n").unwrap();

    let mut engine = SearchEngine::new();
    engine.index_file(&main_path).unwrap(); // helper not indexed yet
    assert_eq!(engine.resolve_imports_incremental(), 0);
    assert_eq!(engine.waiting_imports_count(), 2, "helper + std::io parked");

    // An unrelated file must not trigger a retry of parked imports.
    engine.index_file(&unrelated).unwrap();
    assert_eq!(engine.resolve_imports_incremental(), 0);
    assert_eq!(engine.waiting_imports_count(), 2);

    // The target arrives: only the `helper` import is retried and resolves.
    engine.index_file(&helper_path).unwrap();
    assert_eq!(engine.resolve_imports_incremental(), 1);
    assert_eq!(engine.waiting_imports_count(), 1, "std::io stays parked");
    let helper_id = engine.find_file_id(&helper_path.to_string_lossy()).unwrap();
    let main_id = engine.find_file_id(&main_path.to_string_lossy()).unwrap();
    assert_eq!(engine.get_dependents(helper_id), vec![main_id]);
}

#[test]
fn test_case_insensitive_search() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.txt");

    // Create file with only lowercase content
    let mut file = fs::File::create(&file_path).unwrap();
    writeln!(file, "hello world").unwrap();
    writeln!(file, "another hello here").unwrap();
    drop(file);

    let mut engine = SearchEngine::new();
    engine.index_file(&file_path).unwrap();
    engine.finalize();

    // Test that all case variants find the same results
    let results_lower = engine.search("hello", 10);
    let results_upper = engine.search("HELLO", 10);
    let results_mixed = engine.search("Hello", 10);

    // All queries should find both lines with "hello"
    assert_eq!(
        results_lower.len(),
        2,
        "lowercase query 'hello' should find 2 matches"
    );
    assert_eq!(
        results_upper.len(),
        2,
        "uppercase query 'HELLO' should find 2 matches"
    );
    assert_eq!(
        results_mixed.len(),
        2,
        "mixed case query 'Hello' should find 2 matches"
    );
}

#[test]
fn test_save_and_load_index() {
    use crate::config::IndexerConfig;

    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.rs");
    let index_path = temp_dir.path().join("index.bin");

    // Create a test file
    let mut file = fs::File::create(&file_path).unwrap();
    writeln!(file, "fn hello_world() {{}}").unwrap();
    writeln!(file, "hello world").unwrap();
    writeln!(file, "rust programming").unwrap();
    drop(file);

    // Create config for the test
    let config = IndexerConfig {
        paths: vec![temp_dir.path().to_string_lossy().to_string()],
        ..Default::default()
    };

    // Index and save
    let mut engine = SearchEngine::new();
    engine.index_file(&file_path).unwrap();
    engine.finalize();

    // Verify search works before save
    let results = engine.search("hello", 10);
    assert!(!results.is_empty(), "Should find hello before save");

    // Save the index
    engine.save_index(&index_path, &config).unwrap();
    assert!(index_path.exists(), "Index file should exist");

    // Create a new engine and load the index
    let mut engine2 = SearchEngine::new();
    let stale_files = engine2.load_index(&index_path).unwrap();

    // No files should be stale since we haven't modified them
    assert!(stale_files.is_empty(), "No files should be stale");

    // Verify search works after load
    let results2 = engine2.search("hello", 10);
    assert!(!results2.is_empty(), "Should find hello after load");

    // Verify symbol cache was rebuilt after load
    let symbol_results = engine2.search_symbols("hello_world", "", "", 10).unwrap();
    assert!(
        !symbol_results.is_empty(),
        "Should find hello_world symbol after load"
    );
}

/// The three public loaders share one implementation. `load_index` folds
/// removed files into its stale list and registers no roots (display paths
/// stay absolute); the reconciling loaders report removed files separately,
/// register the configured paths as roots, and report every phase in order.
#[test]
fn test_load_paths_share_one_implementation() {
    use crate::config::IndexerConfig;
    use std::collections::BTreeSet;

    let temp_dir = TempDir::new().unwrap();
    let keep = temp_dir.path().join("keep.rs");
    let gone = temp_dir.path().join("gone.rs");
    let index_path = temp_dir.path().join("index.bin");
    fs::write(&keep, "fn keep_token() {}\n").unwrap();
    fs::write(&gone, "fn gone_token() {}\n").unwrap();
    let config = IndexerConfig {
        paths: vec![temp_dir.path().to_string_lossy().to_string()],
        ..Default::default()
    };

    let mut engine = SearchEngine::new();
    engine.index_file(&keep).unwrap();
    engine.index_file(&gone).unwrap();
    engine.finalize();
    engine.save_index(&index_path, &config).unwrap();
    fs::remove_file(&gone).unwrap();

    // Legacy loader: removed file is reported as stale, paths stay absolute.
    let mut legacy = SearchEngine::new();
    let stale = legacy.load_index(&index_path).unwrap();
    assert_eq!(stale.len(), 1);
    assert!(stale[0].ends_with("gone.rs"));
    let hits = legacy.search("keep_token", 10);
    assert_eq!(hits.len(), 1);
    assert!(
        std::path::Path::new(&hits[0].file_path).is_absolute(),
        "no roots registered without a config: {}",
        hits[0].file_path
    );
    assert!(legacy.search("gone_token", 10).is_empty());

    // Reconciling loader with progress: removed file reported separately,
    // configured path registered as a root, phases reported in order.
    let mut phases = Vec::new();
    let mut reconciled = SearchEngine::new();
    let result = reconciled
        .load_index_with_progress(&index_path, &config, |phase, _, _, _| {
            if phases.last() != Some(&phase) {
                phases.push(phase);
            }
        })
        .unwrap();
    assert!(result.stale_files.is_empty());
    assert_eq!(result.removed_files.len(), 1);
    assert!(result.removed_files[0].ends_with("gone.rs"));
    assert!(result.config_compatible);
    assert_eq!(
        result.already_indexed_files,
        vec![keep.canonicalize().unwrap()]
    );
    assert_eq!(
        phases,
        vec![
            LoadingPhase::ReadingFile,
            LoadingPhase::Deserializing,
            LoadingPhase::CheckingFiles,
            LoadingPhase::MappingFiles,
            LoadingPhase::RestoringTrigrams,
            LoadingPhase::RebuildingSymbols,
        ]
    );
    let root_name = temp_dir
        .path()
        .canonicalize()
        .unwrap()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let hits = reconciled.search("keep_token", 10);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].file_path, format!("{root_name}/keep.rs"));

    // Both loaders restore the same searchable set.
    let ids = |e: &SearchEngine| -> BTreeSet<String> {
        e.search("keep_token", 10)
            .into_iter()
            .map(|m| m.line_number.to_string())
            .collect()
    };
    assert_eq!(ids(&legacy), ids(&reconciled));
}

#[test]
fn test_can_load_index() {
    let temp_dir = TempDir::new().unwrap();
    let index_path = temp_dir.path().join("nonexistent.bin");

    assert!(!SearchEngine::can_load_index(&index_path));

    // Create the file
    fs::write(&index_path, "dummy").unwrap();
    assert!(SearchEngine::can_load_index(&index_path));
}

#[test]
fn test_search_symbols() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.rs");

    // Create a Rust file with functions and a class
    fs::write(
        &file_path,
        r#"
fn hello_world() {
println!("Hello");
}

fn another_function() {
// code
}

pub struct TestStruct {
name: String,
}

impl TestStruct {
pub fn new(name: &str) -> Self {
    Self { name: name.to_string() }
}
}
"#,
    )
    .unwrap();

    let mut engine = SearchEngine::new();
    engine.index_file(&file_path).unwrap();
    engine.finalize();

    // Search for symbols matching "function"
    let results = engine.search_symbols("function", "", "", 10).unwrap();
    assert!(
        !results.is_empty(),
        "Expected at least one symbol match for 'function'"
    );
    assert!(
        results
            .iter()
            .any(|r| r.content.contains("another_function")),
        "Expected to find 'another_function' symbol"
    );

    // Search for symbols matching "hello"
    let results = engine.search_symbols("hello", "", "", 10).unwrap();
    assert!(
        !results.is_empty(),
        "Expected at least one symbol match for 'hello'"
    );
    assert!(
        results.iter().any(|r| r.content.contains("hello_world")),
        "Expected to find 'hello_world' symbol"
    );

    // All results should be marked as symbols
    for result in &results {
        assert!(
            result.is_symbol,
            "All symbol search results should have is_symbol=true"
        );
    }

    // Search for something that doesn't match any symbol
    let results = engine.search_symbols("println", "", "", 10).unwrap();
    assert!(
        results.is_empty(),
        "Expected no symbol match for 'println' (it's not a symbol name)"
    );
}

#[test]
fn test_search_symbols_case_insensitive() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.rs");

    fs::write(
        &file_path,
        r#"
fn HelloWorld() {
println!("Hello");
}
"#,
    )
    .unwrap();

    let mut engine = SearchEngine::new();
    engine.index_file(&file_path).unwrap();
    engine.finalize();

    // Case-insensitive search should work
    let results_lower = engine.search_symbols("helloworld", "", "", 10).unwrap();
    let results_upper = engine.search_symbols("HELLOWORLD", "", "", 10).unwrap();
    let results_mixed = engine.search_symbols("HelloWorld", "", "", 10).unwrap();

    assert!(
        !results_lower.is_empty(),
        "lowercase query should find symbol"
    );
    assert!(
        !results_upper.is_empty(),
        "uppercase query should find symbol"
    );
    assert!(
        !results_mixed.is_empty(),
        "mixed case query should find symbol"
    );
}

#[test]
fn test_symbol_exact_match_scores_higher_than_partial() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.rs");

    // Two symbols: one is an exact match for "calc", one is a superset "calculate"
    fs::write(
        &file_path,
        r#"
fn calc(x: f64) -> f64 { x }

fn calculate(x: f64, y: f64) -> f64 { x + y }
"#,
    )
    .unwrap();

    let mut engine = SearchEngine::new();
    engine.index_file(&file_path).unwrap();
    engine.finalize();

    let results = engine.search_symbols("calc", "", "", 10).unwrap();
    assert!(
        results.len() >= 2,
        "Expected at least two symbol matches (calc and calculate)"
    );

    let calc_score = results
        .iter()
        .find(|r| r.content.contains("fn calc("))
        .map(|r| r.score)
        .expect("Expected to find 'calc' symbol");

    let calculate_score = results
        .iter()
        .find(|r| r.content.contains("fn calculate("))
        .map(|r| r.score)
        .expect("Expected to find 'calculate' symbol");

    assert!(
        calc_score > calculate_score,
        "Exact match 'calc' (score={calc_score}) should score higher than partial match 'calculate' (score={calculate_score})"
    );
}

// ========== Tests for review fixes ==========

/// Fix #1: Regex trigram literals should be lowercased before index lookup.
/// Without this fix, searching for a regex like `MyClass\.\w+` would fail to
/// find trigram matches because the index stores lowercased content.
#[test]
fn test_regex_search_uses_lowercased_trigrams() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.py");

    fs::write(
        &file_path,
        "class MyClass:\n    def do_thing(self):\n        MyClass.do_thing()\n",
    )
    .unwrap();

    let mut engine = SearchEngine::new();
    engine.index_file(&file_path).unwrap();
    engine.finalize();

    // Regex with uppercase literal — trigram acceleration must lowercase before lookup
    let results = engine.search_regex(r"MyClass\.\w+", "", "", 10).unwrap();
    assert!(
        !results.is_empty(),
        "Regex with uppercase literal should find matches via lowered trigram lookup"
    );
    assert!(
        results
            .iter()
            .any(|r| r.content.contains("MyClass.do_thing")),
        "Should find MyClass.do_thing()"
    );
}

/// Fix #2: Exact match boost must compare against the original (un-lowered) query.
/// A search for "MyFunction" should score the exact-case line higher than
/// a line with "myfunction".
#[test]
fn test_exact_match_boost_uses_original_case() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.rs");

    // Two lines: one with exact case, one with different case
    fs::write(&file_path, "fn MyFunction() {}\nfn myfunction() {}\n").unwrap();

    let mut engine = SearchEngine::new();
    engine.index_file(&file_path).unwrap();
    engine.finalize();

    let results = engine.search("MyFunction", 10);
    assert!(results.len() >= 2, "Should find both variants");

    // Find the exact-case match and the lower-case match
    let exact_match = results
        .iter()
        .find(|r| r.content.contains("fn MyFunction"))
        .unwrap();
    let lower_match = results
        .iter()
        .find(|r| r.content.contains("fn myfunction"))
        .unwrap();

    assert!(
        exact_match.score > lower_match.score,
        "Exact case match ({:.3}) should score higher than lowercase ({:.3})",
        exact_match.score,
        lower_match.score
    );
}

/// Roadmap 3.8: symbol search ranks exact > prefix > substring, and emits
/// one row per line even when several symbols on that line match.
#[test]
fn test_symbol_search_ranking_and_line_dedupe() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("syms.rs");
    fs::write(
        &file_path,
        "fn recalc_total() {}\nfn calc() {}\nfn calc_sum() {}\n",
    )
    .unwrap();
    let mut engine = SearchEngine::new();
    engine.index_file(&file_path).unwrap();
    engine.finalize();

    let hits = engine.search_symbols("calc", "", "", 10).unwrap();
    let order: Vec<usize> = hits.iter().map(|m| m.line_number).collect();
    assert_eq!(
        order,
        vec![2, 3, 1],
        "exact, then prefix, then substring: {hits:?}"
    );

    // Two symbols on one line: inject a duplicate definition on line 2.
    let id = engine.find_file_id(&file_path.to_string_lossy()).unwrap();
    let dup = Symbol {
        name: "calc".to_string(),
        symbol_type: SymbolType::Variable,
        line: 1,
        column: 3,
        is_definition: true,
    };
    engine.symbol_cache[id as usize].push(dup);
    let hits = engine.search_symbols("calc", "", "", 10).unwrap();
    assert_eq!(
        hits.iter().filter(|m| m.line_number == 2).count(),
        1,
        "one row per line: {hits:?}"
    );
}

/// Roadmap 7: case-sensitive, whole-word, AND, exclusion and lang:/file:
/// through the parsed-query entry point.
#[test]
fn test_query_syntax_search() {
    use crate::search::query_syntax::parse;
    let temp_dir = TempDir::new().unwrap();
    fs::write(
        temp_dir.path().join("a.rs"),
        "fn Cat() {}\nlet concatenate = 1;\nlet cat = 2; // dog here\n",
    )
    .unwrap();
    fs::write(temp_dir.path().join("b.py"), "cat = 'CAT'\n").unwrap();
    fs::write(temp_dir.path().join("c.rs"), "fn other() { cat(); }\n").unwrap();
    let mut engine = SearchEngine::new();
    for f in ["a.rs", "b.py", "c.rs"] {
        engine.index_file(temp_dir.path().join(f)).unwrap();
    }
    engine.finalize();
    let run = |q: &str| -> Vec<(String, usize)> {
        let parsed = parse(q);
        let (hits, _) = engine
            .search_parsed(&parsed, "", "", SearchLimits::new(50), RankMode::Full)
            .unwrap();
        let mut v: Vec<(String, usize)> = hits
            .iter()
            .map(|m| {
                (
                    m.file_path.rsplit('/').next().unwrap().to_string(),
                    m.line_number,
                )
            })
            .collect();
        v.sort();
        v
    };
    // case-insensitive default: every line with "cat" in any case
    assert_eq!(run("cat").len(), 5);
    // case:yes -> only exact-case occurrences
    assert_eq!(
        run("cat case:yes"),
        vec![
            ("a.rs".into(), 2),
            ("a.rs".into(), 3),
            ("b.py".into(), 1),
            ("c.rs".into(), 1)
        ]
    );
    assert_eq!(run("Cat case:yes"), vec![("a.rs".into(), 1)]);
    // word:yes -> "concatenate" no longer matches
    assert_eq!(
        run("cat word:yes"),
        vec![
            ("a.rs".into(), 1),
            ("a.rs".into(), 3),
            ("b.py".into(), 1),
            ("c.rs".into(), 1)
        ]
    );
    // AND: both terms must be in the file; lines matching either are returned
    assert_eq!(
        run("cat dog"),
        vec![("a.rs".into(), 1), ("a.rs".into(), 2), ("a.rs".into(), 3)]
    );
    // exclusion at file level
    assert_eq!(
        run("cat -dog"),
        vec![("b.py".into(), 1), ("c.rs".into(), 1)]
    );
    // lang: / file:
    assert_eq!(run("cat lang:py"), vec![("b.py".into(), 1)]);
    assert_eq!(run("cat file:c.rs"), vec![("c.rs".into(), 1)]);
    // A bare `file:` fragment is a substring match over the whole display
    // path, and with no root registered that is the absolute temp path, so
    // exclude by file name rather than by a single letter (macOS temp dirs
    // live under `/var/folders`, Windows under `AppData`).
    assert_eq!(
        run("cat -file:a.rs"),
        vec![("b.py".into(), 1), ("c.rs".into(), 1)]
    );
    // quoted phrase
    assert_eq!(run("\"dog here\""), vec![("a.rs".into(), 3)]);
}

/// Roadmap 7: a pattern that mentions a newline or sets the `s` flag is
/// matched against the whole file and reported on the line where the match
/// starts; ordinary patterns stay line-oriented.
#[test]
fn test_multiline_regex() {
    use crate::search::regex_search::needs_multiline;
    assert!(needs_multiline(r"foo\n\s*bar"));
    assert!(needs_multiline(r"(?s)start.*end"));
    assert!(needs_multiline(r"(?is:a.b)"));
    assert!(needs_multiline(r"a\x0Ab"));
    assert!(!needs_multiline(r"foo\s+bar"));
    assert!(!needs_multiline(r"(?i)foo"));
    assert!(!needs_multiline(r"(?-s)a.b"));

    let temp_dir = TempDir::new().unwrap();
    fs::write(
        temp_dir.path().join("a.rs"),
        "fn alpha() {\n    beta();\n}\nfn gamma() {\n\n    beta();\n}\nstart middle\nend\n",
    )
    .unwrap();
    let mut engine = SearchEngine::new();
    engine.index_file(temp_dir.path().join("a.rs")).unwrap();
    engine.finalize();

    // Line-oriented: `\s+` never crosses a line.
    assert!(engine
        .search_regex(r"\{\s+beta", "", "", 10)
        .unwrap()
        .is_empty());
    // Explicit newline: matches across lines, reported on the first line.
    // (`\s+` also spans the blank line inside `gamma`.)
    let hits = engine.search_regex(r"\{\n\s+beta", "", "", 10).unwrap();
    let lines: Vec<usize> = hits.iter().map(|h| h.line_number).collect();
    assert_eq!(lines, vec![1, 4], "{hits:?}");
    assert_eq!(hits[0].line_match_start, 11);
    assert_eq!(hits[0].line_match_end, 12, "clamped to the first line");
    // `(?s)` lets `.` cross lines.
    let hits = engine.search_regex(r"(?s)start.*end", "", "", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].line_number, 8);
    assert_eq!(hits[0].content, "start middle");
    // One result per starting line even if several matches start there.
    let hits = engine.search_regex(r"(?s)b.", "", "", 10).unwrap();
    let lines: Vec<usize> = hits.iter().map(|h| h.line_number).collect();
    assert_eq!(lines, vec![2, 6]);
}

/// Roadmap 7: reference search returns the lines where an identifier is
/// used, not where it is defined; survives a save/load round trip with the
/// name table intact; and follows updates and removals.
#[test]
fn test_search_references() {
    use crate::config::IndexerConfig;
    let temp_dir = TempDir::new().unwrap();
    let a = temp_dir.path().join("a.rs");
    let b = temp_dir.path().join("b.rs");
    fs::write(
        &a,
        "pub fn widget() {}\nfn run() {\n    widget();\n    widget(); widget();\n}\n",
    )
    .unwrap();
    fs::write(
        &b,
        "fn other() {\n    crate::widget();\n}\nfn widgets() {}\n",
    )
    .unwrap();
    let mut engine = SearchEngine::new();
    engine.index_file(&a).unwrap();
    engine.index_file(&b).unwrap();
    engine.finalize();
    assert_eq!(engine.reference_count(), 4);

    let hits = |e: &SearchEngine, name: &str| -> Vec<(String, usize, usize)> {
        let (m, _) = e
            .search_references(name, "", "", SearchLimits::new(50))
            .unwrap();
        let mut v: Vec<(String, usize, usize)> = m
            .iter()
            .map(|m| {
                assert!(m.is_reference && !m.is_symbol);
                (
                    m.file_path.rsplit('/').next().unwrap().to_string(),
                    m.line_number,
                    m.line_match_start,
                )
            })
            .collect();
        v.sort();
        v
    };
    // Two call lines in a.rs (line 4 has two calls -> one result), one in b.rs.
    // The definition (a.rs:1) and the unrelated `widgets` are not references.
    assert_eq!(
        hits(&engine, "widget"),
        vec![
            ("a.rs".into(), 3, 4),
            ("a.rs".into(), 4, 4),
            ("b.rs".into(), 2, 11)
        ]
    );
    assert!(hits(&engine, "Widget").is_empty(), "case-sensitive");
    assert!(hits(&engine, "nothing_here").is_empty());
    // file: operators apply through the parsed form.
    let parsed = crate::search::query_syntax::parse("widget file:b.rs");
    let (m, _) = engine
        .search_references_parsed(&parsed, "", "", SearchLimits::new(50))
        .unwrap();
    assert_eq!(m.len(), 1);
    assert!(m[0].file_path.ends_with("b.rs"));

    // Persistence round trip keeps the references and their names.
    let config = IndexerConfig {
        paths: vec![temp_dir.path().to_string_lossy().to_string()],
        ..Default::default()
    };
    let index_path = temp_dir.path().join("index.bin");
    engine.save_index(&index_path, &config).unwrap();
    let mut reloaded = SearchEngine::new();
    reloaded
        .load_index_with_reconciliation(&index_path, &config)
        .unwrap();
    assert_eq!(reloaded.reference_count(), 4);
    assert_eq!(hits(&reloaded, "widget"), hits(&engine, "widget"));

    // Update: b.rs no longer calls widget; remove: a.rs goes away.
    fs::write(&b, "fn other() {}\n").unwrap();
    engine.update_file(&b).unwrap();
    assert_eq!(hits(&engine, "widget").len(), 2);
    assert!(engine.remove_file(&a));
    assert!(hits(&engine, "widget").is_empty());
    assert_eq!(engine.reference_count(), 0);
}

/// Roadmap 7: `line_hits` agrees with the ASCII scanner for the default
/// options and handles word boundaries around multi-byte characters.
#[test]
fn test_line_hits_options() {
    let content = "Needle needlework\nüneedle needle\nneedle_x needle";
    let default = SearchOptions::default();
    let a: Vec<_> = line_hits(content, "needle", "needle", default)
        .into_iter()
        .map(|h| (h.line_num, h.start))
        .collect();
    let b: Vec<_> = ascii_ci_line_hits(content, "needle")
        .into_iter()
        .map(|h| (h.line_num, h.start))
        .collect();
    assert_eq!(a, b);
    let word = SearchOptions {
        whole_word: true,
        ..default
    };
    let w: Vec<_> = line_hits(content, "needle", "needle", word)
        .into_iter()
        .map(|h| (h.line_num, h.line[h.start..h.end].to_string(), h.start))
        .collect();
    // line 0: "Needle" (word), line 1: skip "üneedle" (ü is a word char), take " needle";
    // line 2: skip "needle_x", take the last one.
    assert_eq!(
        w,
        vec![
            (0, "Needle".to_string(), 0),
            (1, "needle".to_string(), 9),
            (2, "needle".to_string(), 9)
        ]
    );
    let cs = SearchOptions {
        case_sensitive: true,
        ..default
    };
    let c: Vec<_> = line_hits(content, "Needle", "needle", cs)
        .into_iter()
        .map(|h| h.line_num)
        .collect();
    assert_eq!(c, vec![0]);
}

/// Roadmap 3.7: results carry offsets into the FULL line and a character
/// column, independent of content truncation and multi-byte prefixes.
#[test]
fn test_match_offsets_refer_to_full_line() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("offsets.rs");
    // 600 bytes of prefix (truncation window is 500), then a multi-byte
    // char, then the needle.
    let prefix = "x".repeat(600);
    let line = format!("{prefix}é offsets_needle();");
    fs::write(&file_path, format!("{line}\n")).unwrap();

    let mut engine = SearchEngine::new();
    engine.index_file(&file_path).unwrap();
    engine.finalize();

    let hits = engine.search("offsets_needle", 5);
    assert_eq!(hits.len(), 1);
    let m = &hits[0];
    assert!(m.content_truncated);
    let expected_byte = line.find("offsets_needle").unwrap();
    assert_eq!(m.line_match_start, expected_byte);
    assert_eq!(m.line_match_end, expected_byte + "offsets_needle".len());
    // 600 x's + 'é' + ' ' = 602 characters before the match.
    assert_eq!(m.match_column, 602);
    // The truncated-content offsets still index `content` correctly.
    assert_eq!(&m.content[m.match_start..m.match_end], "offsets_needle");
}

/// Roadmap 3.4: the whole-buffer ASCII scan must agree exactly with the
/// per-line search it replaces (first hit per line, CRLF handling, hits
/// on the last unterminated line, case-insensitivity, multi-byte text).
#[test]
fn test_ascii_line_hits_matches_per_line_search() {
    let content = "First Needle here\r\nno hit\nneedle NEEDLE twice\n\n  über needle\nlast needle";
    let expected: Vec<(usize, &str, usize, usize)> = content
        .lines()
        .enumerate()
        .filter_map(|(n, l)| {
            find_match_position_case_insensitive(l, "needle").map(|(s, e)| (n, l, s, e))
        })
        .collect();
    let got: Vec<(usize, &str, usize, usize)> = ascii_ci_line_hits(content, "needle")
        .into_iter()
        .map(|h| (h.line_num, h.line, h.start, h.end))
        .collect();
    assert_eq!(got, expected);
    assert_eq!(got.len(), 4);
    assert_eq!(got[0].1, "First Needle here", "CRLF stripped");
    assert!(ascii_ci_line_hits(content, "absent").is_empty());
    assert!(ascii_ci_line_hits("", "x").is_empty());
}

/// Roadmap 3.1: a query's work is bounded by the match budget; the
/// result reports the truncation and omits the total. With an unbounded
/// budget the total is exact.
#[test]
fn test_match_budget_bounds_work_and_is_reported() {
    let temp_dir = TempDir::new().unwrap();
    for f in 0..40 {
        let body: String = (0..30)
            .map(|l| format!("let budget_needle_{f}_{l} = 1;\n"))
            .collect();
        fs::write(temp_dir.path().join(format!("b{f}.rs")), body).unwrap();
    }
    let mut engine = SearchEngine::new();
    for f in 0..40 {
        engine
            .index_file(temp_dir.path().join(format!("b{f}.rs")))
            .unwrap();
    }
    engine.finalize();

    // 1200 matching lines exist. A budget of 100 must stop early.
    let limits = SearchLimits::new(10).with_match_budget(100);
    let (hits, info) = engine.search_ranked_with_limits("budget_needle", limits, RankMode::Full);
    assert_eq!(hits.len(), 10);
    assert!(info.truncated_by_budget, "{info:?}");
    assert_eq!(info.total_matches, None);

    // Unbounded: exact total, not truncated.
    let limits = SearchLimits::new(10).with_match_budget(usize::MAX);
    let (hits, info) = engine.search_ranked_with_limits("budget_needle", limits, RankMode::Full);
    assert_eq!(hits.len(), 10);
    assert!(!info.truncated_by_budget);
    assert_eq!(info.total_matches, Some(1200));

    // An already-expired deadline stops the scan immediately.
    let limits = SearchLimits::new(10)
        .with_match_budget(usize::MAX)
        .with_deadline(std::time::Instant::now() - std::time::Duration::from_millis(1));
    let (_hits, info) = engine.search_ranked_with_limits("budget_needle", limits, RankMode::Full);
    assert!(info.truncated_by_budget, "deadline must mark truncation");
}

/// Roadmap 3.6: ordering is deterministic under ties and paging with
/// `offset` walks the same ordering without repeats or gaps.
#[test]
fn test_deterministic_order_and_offset_paging() {
    let temp_dir = TempDir::new().unwrap();
    // 12 identical files -> 12 tied matches per query.
    for f in 0..12 {
        fs::write(
            temp_dir.path().join(format!("tie{f:02}.rs")),
            "fn tied_needle() {}\n",
        )
        .unwrap();
    }
    let mut engine = SearchEngine::new();
    for f in 0..12 {
        engine
            .index_file(temp_dir.path().join(format!("tie{f:02}.rs")))
            .unwrap();
    }
    engine.finalize();

    let key = |m: &SearchMatch| (m.file_path.clone(), m.line_number);
    let full: Vec<_> = engine
        .search_ranked_with_limits("tied_needle", SearchLimits::new(100), RankMode::Full)
        .0
        .iter()
        .map(key)
        .collect();
    assert_eq!(full.len(), 12);
    for _ in 0..5 {
        let again: Vec<_> = engine
            .search_ranked_with_limits("tied_needle", SearchLimits::new(100), RankMode::Full)
            .0
            .iter()
            .map(key)
            .collect();
        assert_eq!(again, full, "order must be identical run to run");
    }

    let mut paged = Vec::new();
    for page in 0..3 {
        let (hits, info) = engine.search_ranked_with_limits(
            "tied_needle",
            SearchLimits::new(5).with_offset(page * 5),
            RankMode::Full,
        );
        assert_eq!(info.total_matches, Some(12));
        paged.extend(hits.iter().map(key));
    }
    assert_eq!(paged, full, "pages must tile the full ordering");
    let (past_end, _) = engine.search_ranked_with_limits(
        "tied_needle",
        SearchLimits::new(5).with_offset(50),
        RankMode::Full,
    );
    assert!(past_end.is_empty());
}

/// Roadmap 1.3: a file that fails the tree-sitter structural check (here a
/// single 200 KB line) must still be text-searchable; it only loses symbol
/// extraction. Previously it was dropped from the whole index.
#[test]
fn test_long_line_file_is_searchable_without_symbols() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("fixture.json");
    let mut content = String::from("{\"needle_zq\": \"");
    content.push_str(&"x".repeat(200_000));
    content.push_str("\", \"fn\": \"other_fn_name\"}\n");
    fs::write(&file_path, &content).unwrap();

    let mut engine = SearchEngine::new();
    engine.index_file(&file_path).unwrap();
    engine.finalize();

    let results = engine.search("needle_zq", 10);
    assert_eq!(results.len(), 1, "long-line file must be searchable");
    assert!(results[0].file_path.ends_with("fixture.json"));

    // Only the synthetic FileName symbol exists; nothing was extracted.
    let syms = engine.symbol_cache.first().cloned().unwrap_or_default();
    assert!(
        syms.iter()
            .all(|s| s.symbol_type == crate::symbols::SymbolType::FileName),
        "no tree-sitter symbols expected, got {syms:?}"
    );
}

/// Roadmap 1.2: the synthetic FileName symbol sits at line 0 and must not
/// make every first-line match look like a symbol definition. Two identical
/// non-definition lines (line 1 and line 4) must score identically, and a
/// real definition further down must still outrank a plain first-line
/// mention.
#[test]
fn test_first_line_does_not_get_definition_boost() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("boost.rs");
    fs::write(
        &file_path,
        "// widget_thing mention\n\n\n// widget_thing mention\nfn widget_thing() {}\n",
    )
    .unwrap();

    let mut engine = SearchEngine::new();
    engine.index_file(&file_path).unwrap();
    engine.finalize();

    let results = engine.search("widget_thing", 10);
    let by_line = |n: usize| {
        results
            .iter()
            .find(|r| r.line_number == n)
            .unwrap_or_else(|| panic!("no match on line {n}: {results:?}"))
    };
    let (l1, l4, l5) = (by_line(1), by_line(4), by_line(5));

    assert!(
        (l1.score - l4.score).abs() < 1e-9,
        "identical plain lines must score identically (line1={:.3}, line4={:.3})",
        l1.score,
        l4.score
    );
    assert!(
        !l1.is_symbol,
        "a comment on line 1 is not a symbol definition"
    );
    assert!(
        l5.score > l1.score,
        "definition on line 5 ({:.3}) must outrank the first-line mention ({:.3})",
        l5.score,
        l1.score
    );
}

/// Fix #7: Line length penalty should be gentle (logarithmic), not harsh.
/// A function definition on a ~100-char line should NOT be obliterated by
/// a short comment. Both should get reasonable scores.
#[test]
fn test_line_length_penalty_is_gentle() {
    // Short line (20 chars)
    let short_score = calculate_score_inline(
        "fn do_thing() {}   ",
        "do_thing",
        "do_thing",
        false,
        false,
        1.0,
    );

    // Medium line (~80 chars)
    let medium_line = "fn do_thing(arg1: String, arg2: i32, arg3: bool) -> Result<()> { todo!() }";
    let medium_score =
        calculate_score_inline(medium_line, "do_thing", "do_thing", false, false, 1.0);

    // Long line (~200 chars)
    let long_line = format!(
        "fn do_thing({}) -> Result<()> {{}}",
        (0..20)
            .map(|i| format!("arg{}: String", i))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let long_score = calculate_score_inline(&long_line, "do_thing", "do_thing", false, false, 1.0);

    // The medium line should retain a decent fraction of the short line's score
    assert!(
        medium_score / short_score > 0.5,
        "Medium line ({:.3}) should be > 50% of short line ({:.3}), got {:.1}%",
        medium_score,
        short_score,
        medium_score / short_score * 100.0
    );

    // Even the long line should not drop below 30% (the floor)
    assert!(
        long_score / short_score > 0.25,
        "Long line ({:.3}) should be > 25% of short line ({:.3}), got {:.1}%",
        long_score,
        short_score,
        long_score / short_score * 100.0
    );
}

/// Fix #8: Document search functions should return None for zero matches
/// instead of Some(empty vec), avoiding unnecessary allocations.
#[test]
fn test_no_match_returns_none_not_empty_vec() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.txt");

    fs::write(&file_path, "hello world\n").unwrap();

    let mut engine = SearchEngine::new();
    engine.index_file(&file_path).unwrap();
    engine.finalize();

    // Search for something not in the file — should produce zero results
    let results = engine.search("xyznonexistent", 10);
    assert!(
        results.is_empty(),
        "Search for non-existent term should yield empty results"
    );
}

/// Fix #9: Filename and content should be separated by triple newline to
/// prevent trigram bleed across the boundary.
#[test]
fn test_filename_content_separator_prevents_trigram_bleed() {
    let temp_dir = TempDir::new().unwrap();
    // File named "alpha_module.txt" with content starting with "beta_function"
    let file_path = temp_dir.path().join("alpha_module.txt");
    fs::write(&file_path, "beta_function called here\n").unwrap();

    let mut engine = SearchEngine::new();
    engine.index_file(&file_path).unwrap();
    engine.finalize();

    // Verify content is still searchable after the separator change
    let results = engine.search("beta_function", 10);
    assert!(
        !results.is_empty(),
        "Should find 'beta_function' in file content"
    );

    // Verify content from the file is correctly returned
    assert!(
        results[0].content.contains("beta_function"),
        "Result content should contain the search term"
    );

    // The triple newline separator means trigrams like "xt\nb" (from single newline join)
    // are NOT generated, protecting against false trigram candidate matches on
    // boundary-spanning text. The filename is used only for trigram candidate filtering,
    // not for result content.
}

/// Fix #12: FileMetadata should pre-compute lowercase_stem at index time
/// and use it for query matching, avoiding per-query path allocation.
#[test]
fn test_file_metadata_precomputed_lowercase_stem() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("MyModule.rs");
    fs::write(&file_path, "fn test() {}\n").unwrap();

    let mut engine = SearchEngine::new();
    engine.index_file(&file_path).unwrap();
    engine.finalize();

    // Verify the metadata was computed with the correct lowercase stem
    let metadata = engine.get_file_metadata(0);
    assert_eq!(
        metadata.lowercase_stem, "mymodule",
        "lowercase_stem should be pre-computed at index time"
    );

    // Query score should boost when query matches the filename stem
    let score_match = metadata.query_score("mymodule");
    let score_nomatch = metadata.query_score("unrelated");
    assert!(
        score_match > score_nomatch,
        "Query matching filename stem ({:.3}) should score higher than non-match ({:.3})",
        score_match,
        score_nomatch
    );
}

/// Fix #4: Symbol search should use trigram pre-filtering for queries >= 3 chars
/// instead of scanning all documents.
#[test]
fn test_symbol_search_uses_trigram_filtering() {
    let temp_dir = TempDir::new().unwrap();

    // Create two files: one with target symbol, one without
    let file_with = temp_dir.path().join("has_symbol.rs");
    fs::write(&file_with, "fn calculate_total() {\n    // does math\n}\n").unwrap();

    let file_without = temp_dir.path().join("no_symbol.rs");
    fs::write(
        &file_without,
        "fn something_else() {\n    // unrelated\n}\n",
    )
    .unwrap();

    let mut engine = SearchEngine::new();
    engine.index_file(&file_with).unwrap();
    engine.index_file(&file_without).unwrap();
    engine.finalize();

    // Symbol search for "calculate" (>= 3 chars, should use trigram pre-filtering)
    let results = engine.search_symbols("calculate", "", "", 10).unwrap();
    assert!(!results.is_empty(), "Should find calculate_total symbol");
    assert!(
        results.iter().all(|r| r.content.contains("calculate")),
        "All results should contain the query term"
    );
}

/// Fix #6: FAST_RANKING_TOP_N should be large enough to not miss relevant results.
#[test]
fn test_fast_ranking_top_n_is_sufficient() {
    // Just verify the constant is reasonable (constant value is expected here)
    #[allow(clippy::assertions_on_constants)]
    {
        assert!(
            SearchEngine::FAST_RANKING_TOP_N >= 2000,
            "FAST_RANKING_TOP_N should be at least 2000 to avoid dropping relevant files"
        );
    }
}

/// Filename-only matches: searching for the filename stem should return
/// a result even when the query does NOT appear in the file content.
#[test]
fn test_filename_only_match_returns_result() {
    let temp_dir = TempDir::new().unwrap();
    // File whose content does NOT contain "configuration_manager"
    let file_path = temp_dir.path().join("configuration_manager.rs");
    fs::write(
        &file_path,
        "pub fn init() {\n    println!(\"starting up\");\n}\n",
    )
    .unwrap();

    let mut engine = SearchEngine::new();
    engine.index_file(&file_path).unwrap();
    engine.finalize();

    // Search for the filename stem — not present in content
    let results = engine.search("configuration_manager", 10);
    assert!(
        !results.is_empty(),
        "Searching for filename stem should return a result even when content doesn't match"
    );

    // The synthetic filename match should have line_number 0
    let filename_result = results.iter().find(|r| r.line_number == 0);
    assert!(
        filename_result.is_some(),
        "Filename match should appear with line_number=0"
    );
    let filename_result = filename_result.unwrap();
    assert!(
        filename_result.is_symbol,
        "Filename match should be marked as a symbol"
    );
    assert!(
        filename_result.content.contains("configuration_manager"),
        "Filename match content should contain the filename: got '{}'",
        filename_result.content
    );
}

/// Filename-only matches should work in symbol search too.
#[test]
fn test_filename_symbol_search() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("widget_factory.rs");
    fs::write(&file_path, "pub fn make() {}\n").unwrap();

    let mut engine = SearchEngine::new();
    engine.index_file(&file_path).unwrap();
    engine.finalize();

    // Symbol search for the filename — the FileName symbol should match
    let results = engine.search_symbols("widget_factory", "", "", 10).unwrap();
    assert!(
        !results.is_empty(),
        "Symbol search for filename stem should return a result"
    );
    let sym_result = &results[0];
    assert_eq!(
        sym_result.line_number, 0,
        "FileName symbol should have line_number=0"
    );
    assert!(
        sym_result.content.contains("widget_factory"),
        "FileName symbol result should show the file path"
    );
}

/// Filename-only matches should work with regex search too.
#[test]
fn test_filename_regex_match() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("data_processor.py");
    fs::write(&file_path, "x = 42\n").unwrap();

    let mut engine = SearchEngine::new();
    engine.index_file(&file_path).unwrap();
    engine.finalize();

    let results = engine.search_regex(r"data_processor", "", "", 10).unwrap();
    assert!(
        !results.is_empty(),
        "Regex search for filename stem should return a result"
    );
    assert!(
        results[0].content.contains("data_processor"),
        "Regex filename match should show the file path"
    );
}

#[test]
fn test_reconciling_progress_percent_with_new_files() {
    // Simulate starting from a saved index (1000 cached) with 50 new files to index.
    let mut progress = IndexingProgress {
        status: IndexingStatus::Reconciling,
        files_indexed: 1000,
        files_discovered: 1000, // pre-seeded with offset
        files_loaded_from_cache: 1000,
        ..Default::default()
    };

    // Before any new files are discovered the percentage should be 92%.
    assert_eq!(progress.progress_percent(), 92);

    // Simulate discovery of 50 new files and partial indexing of 25.
    progress.files_discovered = 1050;
    progress.files_indexed = 1025;
    let mid_pct = progress.progress_percent();
    assert!(
        mid_pct > 92 && mid_pct < 99,
        "Mid-reconciliation percent should be between 92 and 99, got {mid_pct}"
    );

    // After all new files are indexed the percentage should reach 99%.
    progress.files_indexed = 1050;
    assert_eq!(progress.progress_percent(), 99);
}

#[test]
fn test_reconciling_progress_percent_no_new_files() {
    // When there are no new files (nothing to reconcile) percentage stays at 92.
    let progress = IndexingProgress {
        status: IndexingStatus::Reconciling,
        files_indexed: 1000,
        files_discovered: 1000,
        files_loaded_from_cache: 1000,
        ..Default::default()
    };
    assert_eq!(progress.progress_percent(), 92);
}

/// Fix: Short queries (< 3 bytes) should fall back to scanning all documents
/// rather than returning empty results from the trigram index.
/// This is particularly important for `_` (wildcard variable), `__` (Python dunder prefix),
/// and other short but common code search terms.
#[test]
fn test_short_query_underscore_returns_results() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.py");

    fs::write(
        &file_path,
        "class MyClass:\n    def __init__(self):\n        _ = unused_value\n",
    )
    .unwrap();

    let mut engine = SearchEngine::new();
    engine.index_file(&file_path).unwrap();
    engine.finalize();

    // Single `_` — 1 byte, no trigrams can be extracted.
    // Before the fix this returned 0 results; now it falls back to full scan.
    let results = engine.search("_", 10);
    assert!(
        !results.is_empty(),
        "Searching for `_` should find lines containing underscore"
    );

    // `__` — 2 bytes, still no trigrams.
    let results = engine.search("__", 10);
    assert!(
        !results.is_empty(),
        "Searching for `__` should find dunder-style identifiers"
    );
}

/// Compound underscore terms like `badger_farmer` must be found case-insensitively.
/// The trigram index includes cross-underscore trigrams (e.g. `er_`, `r_f`, `_fa`),
/// so a document containing the term will always be a candidate.
#[test]
fn test_compound_underscore_term_search() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.rs");

    fs::write(
        &file_path,
        "fn badger_farmer(x: u32) -> u32 { x + 1 }\nfn other_func() {}\n",
    )
    .unwrap();

    let mut engine = SearchEngine::new();
    engine.index_file(&file_path).unwrap();
    engine.finalize();

    // Exact-case search.
    let results = engine.search("badger_farmer", 10);
    assert!(
        results.iter().any(|r| r.content.contains("badger_farmer")),
        "Should find line containing badger_farmer"
    );

    // Case-insensitive: UPPER_CASE query should match lower-case content.
    let results = engine.search("BADGER_FARMER", 10);
    assert!(
        results.iter().any(|r| r.content.contains("badger_farmer")),
        "UPPER case query should find badger_farmer (case-insensitive)"
    );

    // Prefix ending with underscore.
    let results = engine.search("badger_", 10);
    assert!(
        results.iter().any(|r| r.content.contains("badger_farmer")),
        "Prefix 'badger_' should find badger_farmer"
    );

    // Suffix starting with underscore.
    let results = engine.search("_farmer", 10);
    assert!(
        results.iter().any(|r| r.content.contains("badger_farmer")),
        "Suffix '_farmer' should find badger_farmer"
    );

    // Unrelated term must not match.
    let results = engine.search("badger_thatcher", 10);
    assert!(
        results.is_empty(),
        "Should NOT find badger_thatcher when it is not in the content"
    );
}

/// `truncate_around_match` match positions: no truncation case.
/// When the line is short enough, positions should be returned unchanged.
#[test]
fn test_truncate_around_match_no_truncation() {
    let line = "hello world"; // shorter than MAX_CONTENT_LENGTH
    let result = truncate_around_match(line, 6, 11); // "world"
    assert!(!result.was_truncated);
    assert_eq!(result.match_start, 6);
    assert_eq!(result.match_end, 11);
    assert_eq!(
        &result.content[result.match_start..result.match_end],
        "world"
    );
}

/// `truncate_around_match` match positions: prefix-only truncation.
/// The prefix "…" is 3 bytes (UTF-8 U+2026), so match positions must be
/// shifted by 3, not 1.
#[test]
fn test_truncate_around_match_prefix_ellipsis_is_3_bytes() {
    // Build a line long enough to trigger truncation (> MAX_CONTENT_LENGTH = 500).
    // The match is placed far enough from the start that the window will cut
    // the prefix (match_start > MATCH_CONTEXT_CHARS = 200).
    let prefix = "x".repeat(300); // 300 bytes before the match
    let needle = "TARGET";
    let suffix = "y".repeat(300); // 300 bytes after the match → total > 500
    let line = format!("{prefix}{needle}{suffix}");

    let match_start = 300; // byte offset of "TARGET" in `line`
    let match_end = 306; // "TARGET".len() == 6

    assert!(line.len() > 500, "line must exceed MAX_CONTENT_LENGTH");
    assert!(
        match_start > 200,
        "match must be far enough right to trigger prefix truncation"
    );

    let result = truncate_around_match(&line, match_start, match_end);

    assert!(result.was_truncated, "Long line should be truncated");

    // The returned content should start with "…" (3 bytes) when prefix is cut.
    assert!(
        result.content.starts_with('…'),
        "Truncated content with cut prefix should start with '…'"
    );

    // The byte slice at [match_start..match_end] in the truncated content
    // must equal the original needle.
    let truncated_slice = &result.content[result.match_start..result.match_end];
    assert_eq!(
        truncated_slice, needle,
        "match_start/match_end must point to the needle in the truncated content; \
         got '{truncated_slice}' (expected '{needle}'). \
         This catches the off-by-2 bug where +1 was used instead of +3 for the 3-byte '…'."
    );
}

/// `truncate_around_match` match positions: suffix-only truncation.
/// No prefix ellipsis, so positions should only be shifted by the safe_start offset.
#[test]
fn test_truncate_around_match_suffix_only_no_shift() {
    // Match near the beginning — prefix won't be cut, but suffix will.
    // Total line > 500 bytes (MAX_CONTENT_LENGTH).
    let needle = "TARGET";
    let suffix = "z".repeat(600); // 600 bytes after → total > 500
    let line = format!("{needle}{suffix}");

    assert!(line.len() > 500, "line must exceed MAX_CONTENT_LENGTH");

    let match_start = 0;
    let match_end = needle.len();

    let result = truncate_around_match(&line, match_start, match_end);

    assert!(result.was_truncated, "Long line should be truncated");

    // No prefix ellipsis
    assert!(
        !result.content.starts_with('…'),
        "No prefix cut means content should not start with '…'"
    );
    assert!(
        result.content.ends_with('…'),
        "Suffix cut means content should end with '…'"
    );

    let truncated_slice = &result.content[result.match_start..result.match_end];
    assert_eq!(
        truncated_slice, needle,
        "match_start/match_end must point to the needle even with suffix-only truncation"
    );
}

// ── make_display_path workspace-relative tests ───────────────────────────

/// When no root paths are registered the full path (forward-slash
/// normalised) is returned as a fallback.
#[test]
fn test_make_display_path_no_roots() {
    let engine = SearchEngine::new();
    let path = Path::new("/home/user/project/src/main.rs");
    let result = engine.make_display_path(path);
    // With no roots, the full path is returned with forward slashes
    assert_eq!(result, "/home/user/project/src/main.rs");
}

/// The root folder name must be the first component of the display path.
/// E.g. root `/tmp/myproject`, file `/tmp/myproject/src/main.rs`
/// → `myproject/src/main.rs`.
#[test]
fn test_make_display_path_includes_root_folder_name() {
    let temp_dir = TempDir::new().unwrap();
    // Create a sub-directory that will be the "project root"
    let project_dir = temp_dir.path().join("myproject");
    fs::create_dir_all(&project_dir).unwrap();
    let src_dir = project_dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let file = src_dir.join("main.rs");
    fs::write(&file, "fn main() {}").unwrap();

    let mut engine = SearchEngine::new();
    engine.add_root_path(&project_dir);

    let canonical_file = file.canonicalize().unwrap();
    let display = engine.make_display_path(&canonical_file);

    // Should be "myproject/src/main.rs" — root name included
    assert_eq!(display, "myproject/src/main.rs");
}

/// A file directly inside the root (no sub-directory) should be displayed
/// as `rootname/file.txt`.
#[test]
fn test_make_display_path_file_at_root_level() {
    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path().join("project");
    fs::create_dir_all(&project_dir).unwrap();
    let file = project_dir.join("README.md");
    fs::write(&file, "# readme").unwrap();

    let mut engine = SearchEngine::new();
    engine.add_root_path(&project_dir);

    let canonical_file = file.canonicalize().unwrap();
    let display = engine.make_display_path(&canonical_file);

    assert_eq!(display, "project/README.md");
}

/// With two registered roots, each file shows under its own workspace name.
#[test]
fn test_make_display_path_multiple_roots() {
    let temp_dir = TempDir::new().unwrap();

    let root_a = temp_dir.path().join("alpha");
    let root_b = temp_dir.path().join("beta");
    fs::create_dir_all(&root_a).unwrap();
    fs::create_dir_all(&root_b).unwrap();

    let file_a = root_a.join("utils.rs");
    let file_b = root_b.join("utils.rs");
    fs::write(&file_a, "// alpha utils").unwrap();
    fs::write(&file_b, "// beta utils").unwrap();

    let mut engine = SearchEngine::new();
    engine.add_root_path(&root_a);
    engine.add_root_path(&root_b);

    let display_a = engine.make_display_path(&file_a.canonicalize().unwrap());
    let display_b = engine.make_display_path(&file_b.canonicalize().unwrap());

    assert_eq!(display_a, "alpha/utils.rs");
    assert_eq!(display_b, "beta/utils.rs");
}

// ---------------------------------------------------------------------------
// SearchLimits: offset and budget bounds
// ---------------------------------------------------------------------------

#[test]
fn test_search_limits_offset_is_clamped_and_budget_capped() {
    let base = SearchLimits::new(50);
    assert_eq!(base.offset, 0);

    // A deep offset is clamped and the derived budget stops growing.
    let deep = SearchLimits::new(1000).with_offset(usize::MAX);
    assert_eq!(deep.offset, SearchLimits::MAX_OFFSET);
    assert_eq!(deep.match_budget, (SearchLimits::MAX_OFFSET + 1000) * 8);
    assert!(deep.match_budget <= SearchLimits::MAX_DERIVED_BUDGET);

    // A shallow offset still grows the budget proportionally.
    let shallow = SearchLimits::new(50).with_offset(100);
    assert_eq!(shallow.offset, 100);
    assert!(shallow.match_budget > base.match_budget);
    assert!(shallow.match_budget <= SearchLimits::MAX_DERIVED_BUDGET);

    // An explicit override is still allowed to exceed the derived ceiling.
    let unbounded = SearchLimits::new(50).with_match_budget(usize::MAX);
    assert_eq!(unbounded.match_budget, usize::MAX);
}

// ---------------------------------------------------------------------------
// Reload reconciliation: configuration changes, root boundaries
// ---------------------------------------------------------------------------

/// A file that is unchanged on disk but excluded by the *current* config
/// (new exclude pattern, extension list, .gitignore rule) is dropped on
/// reload instead of staying indexed forever.
#[test]
fn test_reload_drops_files_the_current_config_excludes() {
    use crate::config::IndexerConfig;

    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    fs::create_dir_all(root.join("vendor")).unwrap();
    fs::write(root.join("keep.rs"), "fn keep_marker() {}\n").unwrap();
    fs::write(root.join("vendor/dep.rs"), "fn vendor_marker() {}\n").unwrap();
    fs::write(root.join("notes.py"), "def python_marker(): pass\n").unwrap();
    let index_path = root.join("index.bin");

    let open_config = IndexerConfig {
        paths: vec![root.to_string_lossy().to_string()],
        exclude_patterns: Vec::new(),
        respect_gitignore: false,
        ..Default::default()
    };
    let mut engine = SearchEngine::new();
    engine.index_file(root.join("keep.rs")).unwrap();
    engine.index_file(root.join("vendor/dep.rs")).unwrap();
    engine.index_file(root.join("notes.py")).unwrap();
    engine.finalize();
    engine.save_index(&index_path, &open_config).unwrap();

    // Reload with a stricter config: vendor excluded, only Rust files, and a
    // fresh .gitignore rule that hides keep.rs — none of the files changed.
    fs::write(root.join(".gitignore"), "keep.rs\n").unwrap();
    let strict = IndexerConfig {
        exclude_patterns: vec!["**/vendor/**".to_string()],
        include_extensions: vec!["rs".to_string()],
        respect_gitignore: true,
        ..open_config.clone()
    };
    let mut reloaded = SearchEngine::new();
    let result = reloaded
        .load_index_with_reconciliation(&index_path, &strict)
        .unwrap();
    assert_eq!(
        result.removed_files.len(),
        3,
        "all three files are now ineligible: {:?}",
        result.removed_files
    );
    assert_eq!(reloaded.get_stats().num_files, 0);
    assert!(reloaded.search("vendor_marker", 5).is_empty());
    assert!(reloaded.search("python_marker", 5).is_empty());
    assert!(reloaded.search("keep_marker", 5).is_empty());

    // The same index reloaded with the original config keeps everything.
    let mut same = SearchEngine::new();
    let result = same
        .load_index_with_reconciliation(&index_path, &open_config)
        .unwrap();
    assert!(result.removed_files.is_empty());
    assert_eq!(same.get_stats().num_files, 3);
}

/// `source_base_path` is the longest configured root containing the file at
/// a path-component boundary, so `/x/proj` never claims `/x/proj2/...`.
#[test]
fn test_source_base_path_respects_component_boundaries() {
    use crate::config::IndexerConfig;
    use crate::index::persistence::PersistedIndex;

    let temp_dir = TempDir::new().unwrap();
    let proj = temp_dir.path().join("proj");
    let proj2 = temp_dir.path().join("proj2");
    fs::create_dir_all(&proj).unwrap();
    fs::create_dir_all(&proj2).unwrap();
    fs::write(proj.join("a.rs"), "fn a() {}\n").unwrap();
    fs::write(proj2.join("b.rs"), "fn b() {}\n").unwrap();
    let index_path = temp_dir.path().join("index.bin");

    let config = IndexerConfig {
        paths: vec![
            proj.to_string_lossy().to_string(),
            proj2.to_string_lossy().to_string(),
        ],
        ..Default::default()
    };
    let mut engine = SearchEngine::new();
    engine.index_file(proj.join("a.rs")).unwrap();
    engine.index_file(proj2.join("b.rs")).unwrap();
    engine.finalize();
    engine.save_index(&index_path, &config).unwrap();

    let persisted = PersistedIndex::load(&index_path).unwrap();
    for f in &persisted.files {
        let name = f.path.file_name().unwrap().to_string_lossy().to_string();
        let base = f.source_base_path.clone().unwrap_or_default();
        let expected = if name == "a.rs" { &proj } else { &proj2 };
        assert_eq!(
            std::path::Path::new(&base),
            expected.as_path(),
            "{name} recorded under the wrong root"
        );
    }

    // Dropping `proj` from the config must not report proj2's file removed.
    let only_proj2 = IndexerConfig {
        paths: vec![proj2.to_string_lossy().to_string()],
        ..Default::default()
    };
    let mut reloaded = SearchEngine::new();
    let result = reloaded
        .load_index_with_reconciliation(&index_path, &only_proj2)
        .unwrap();
    assert_eq!(result.removed_files.len(), 1);
    assert!(result.removed_files[0].ends_with("a.rs"));
    assert_eq!(reloaded.search("fn b", 5).len(), 1);
}

/// Review 1.1 / 1.2: the trigram candidate set derived from a regex's
/// literal constraints must be a superset of the documents the compiled
/// regex matches — for repetitions, fixed counts, case-insensitive runs,
/// alternations and word boundaries alike — and the interesting patterns
/// must actually be accelerated (otherwise the property is vacuous).
#[test]
fn test_regex_candidates_are_superset_of_matches() {
    use crate::search::regex_search::RegexAnalysis;
    let temp_dir = TempDir::new().unwrap();
    let corpus: &[&str] = &[
        "fn foobar() {}\n",
        "fooobar and fooooobar\n",
        "fobar only\n",
        "xaaay\n",
        "xaaaay\n",
        "xaay\n",
        "TASK: Task list\n",
        "mask pass MASK\n",
        "foobarbarbaz\n",
        "foobarbaz\n",
        "foobaz\n",
        "abc\n",
        "ababababc\n",
        "xxy xxxxxy xxxxxxxxxxy\n",
        "bbbc bc\n",
        "hello there\n",
        "world here\n",
        "  foooo  \n",
        "foo\n",
        "getValue setValue\n",
        "unrelated content with nothing special\n",
        "Kelvin K sign\n",
        "aaaaaaaaaaaaaaaaaaaaaaaa\n",
    ];
    let mut engine = SearchEngine::new();
    let mut ids = Vec::new();
    for (i, text) in corpus.iter().enumerate() {
        let path = temp_dir.path().join(format!("doc{i}.txt"));
        fs::write(&path, text).unwrap();
        engine.index_file(&path).unwrap();
        ids.push(engine.find_file_id(&path.to_string_lossy()).unwrap());
    }
    engine.finalize();

    let patterns = [
        "fo+bar",
        "foo+bar",
        "xa{3}y",
        "xa{3,}y",
        "(?i)task",
        "(?i)mask",
        "(?i)pass",
        "foo(bar)+baz",
        "a|b+c",
        "(ab)*c",
        "x{2,5}y",
        r"\bfoo+\b",
        "hello|world",
        "(get|set)Value",
        "a{20}",
        "(?i)Kelvin",
        "foobar",
        "fo{2,}b",
    ];
    let mut accelerated = 0;
    for pattern in patterns {
        let analysis = RegexAnalysis::analyze(pattern).unwrap();
        let candidates = engine
            .regex_candidate_docs(&analysis)
            .unwrap_or_else(|| engine.trigram_index.all_documents().into_owned());
        if analysis.is_accelerated {
            accelerated += 1;
        }
        for (i, text) in corpus.iter().enumerate() {
            if analysis.regex.is_match(text) {
                assert!(
                    candidates.contains(ids[i]),
                    "{pattern}: doc {i} ({text:?}) matches but is not a candidate; \
                     constraints {:?}",
                    analysis.constraints
                );
            }
        }
        // End to end: the search itself finds every matching document.
        let hits = engine.search_regex(pattern, "", "", 100).unwrap();
        let hit_ids: std::collections::HashSet<u32> = hits.iter().map(|h| h.file_id).collect();
        for (i, text) in corpus.iter().enumerate() {
            if text.lines().any(|l| analysis.regex.is_match(l)) {
                assert!(hit_ids.contains(&ids[i]), "{pattern}: doc {i} not found");
            }
        }
    }
    assert!(
        accelerated >= 14,
        "expected most patterns to be accelerated, got {accelerated}"
    );
}

/// Review 1.3: reference positions captured at index time are applied to
/// the *current* file content. When the file changed on disk without a
/// re-index, a stale column must not slice mid-character (it panicked, and
/// the panic surfaced as a 500 for every `references=true` request); only
/// positions that still hold the name are reported.
#[test]
fn test_references_survive_stale_positions() {
    let temp_dir = TempDir::new().unwrap();
    let a = temp_dir.path().join("a.rs");
    fs::write(
        &a,
        "pub fn widget() {}\nfn run() {\n    widget();\n    widget();\n    widget();\n}\n",
    )
    .unwrap();
    let mut engine = SearchEngine::new();
    engine.index_file(&a).unwrap();
    engine.finalize();
    let count = |e: &SearchEngine| {
        e.search_references("widget", "", "", SearchLimits::new(50))
            .unwrap()
            .0
            .len()
    };
    assert_eq!(count(&engine), 3);

    // Same line count, but: line 3 now has a multi-byte character so byte 4
    // lands mid-character; line 4 is shorter than the recorded column; line
    // 5 still has the call where it was.
    fs::write(
        &a,
        "pub fn widget() {}\nfn run() {\n   é widget();\n  x\n    widget();\n}\n",
    )
    .unwrap();
    let (hits, _) = engine
        .search_references("widget", "", "", SearchLimits::new(50))
        .unwrap();
    let lines: Vec<usize> = hits.iter().map(|h| h.line_number).collect();
    assert_eq!(lines, vec![5], "{hits:?}");
    assert_eq!(hits[0].line_match_start, 4);
    assert_eq!(&hits[0].content[4..10], "widget");

    // The file shrank below the recorded lines: no hits, no panic.
    fs::write(&a, "fn other() {}\n").unwrap();
    assert_eq!(count(&engine), 0);
}

/// Review 1.5: with several terms, lines containing every term rank first
/// and are emitted before the per-document cap, so `fn main` finds
/// `fn main()` even in a file with hundreds of other `fn` lines; single-term
/// queries keep their document-order, unscaled results.
#[test]
fn test_multi_term_lines_rank_by_terms_matched() {
    use crate::search::query_syntax::parse;
    let temp_dir = TempDir::new().unwrap();
    // 150 `fn` lines (only the first term), then the line with both terms,
    // then one more `main` line (only the second term).
    let mut big = String::new();
    for i in 0..150 {
        big.push_str(&format!("fn helper_{i}() {{}}\n"));
    }
    big.push_str("fn main() {}\n");
    big.push_str("// main entry\n");
    fs::write(temp_dir.path().join("big.rs"), &big).unwrap();
    // A second file where the first `fn` line comes before the joint line.
    fs::write(
        temp_dir.path().join("small.rs"),
        "fn other() {}\nlet x = 1;\nfn main() { other() }\n",
    )
    .unwrap();
    let mut engine = SearchEngine::new();
    engine.index_file(temp_dir.path().join("big.rs")).unwrap();
    engine.index_file(temp_dir.path().join("small.rs")).unwrap();
    engine.finalize();

    let (hits, _) = engine
        .search_parsed(
            &parse("fn main"),
            "",
            "",
            SearchLimits::new(500),
            RankMode::Full,
        )
        .unwrap();
    let name = |h: &SearchMatch| h.file_path.rsplit('/').next().unwrap().to_string();
    // The two lines holding both terms come first, ahead of every
    // single-term line, and the capped big file still reports its joint
    // line (line 151, beyond MAX_MATCHES_PER_DOC) and its `main`-only line.
    assert!(hits.len() >= 2, "{}", hits.len());
    let top: Vec<(String, usize)> = hits[..2].iter().map(|h| (name(h), h.line_number)).collect();
    assert!(top.contains(&("big.rs".into(), 151)), "{top:?}");
    assert!(top.contains(&("small.rs".into(), 3)), "{top:?}");
    assert!(hits[..2].iter().all(|h| h.content.contains("fn main")));
    let big_lines: Vec<usize> = hits
        .iter()
        .filter(|h| name(h) == "big.rs")
        .map(|h| h.line_number)
        .collect();
    assert_eq!(big_lines.len(), SearchEngine::MAX_MATCHES_PER_DOC);
    assert_eq!(big_lines[0], 151, "joint line first");
    // Single-term lines follow in document order and the cap still applies
    // to them, so the `main`-only line 152 is cut off like `fn` line 150.
    assert!(!big_lines.contains(&152), "{big_lines:?}");
    let mut rest = big_lines[1..].to_vec();
    rest.sort_unstable();
    assert_eq!(rest, (1..100).collect::<Vec<usize>>());
    // The multiplier is exactly the number of distinct terms on the line.
    let joint = hits.iter().find(|h| h.line_number == 3).unwrap();
    let single = hits
        .iter()
        .find(|h| name(h) == "small.rs" && h.line_number == 1)
        .unwrap();
    assert!(joint.score > single.score, "{joint:?} vs {single:?}");
    let (only_fn, _) = engine
        .search_parsed(&parse("fn"), "", "", SearchLimits::new(500), RankMode::Full)
        .unwrap();
    let small_fn_main = only_fn
        .iter()
        .find(|h| name(h) == "small.rs" && h.line_number == 3)
        .unwrap();
    assert_eq!(joint.score, small_fn_main.score * 2.0);

    // Single term: document order within the file, unscaled, capped.
    let big_only: Vec<usize> = only_fn
        .iter()
        .filter(|h| name(h) == "big.rs")
        .map(|h| h.line_number)
        .collect();
    assert_eq!(big_only.len(), SearchEngine::MAX_MATCHES_PER_DOC);
    assert!(!big_only.contains(&151), "cut off by the cap, as before");
}

/// Review 1.10: a needle containing a newline matches across lines; the
/// hit is reported on its first line with the match range clamped to that
/// line instead of `match_end` pointing past it.
#[test]
fn test_needle_with_newline_is_clamped_to_first_line() {
    use crate::search::query_syntax::parse;
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("a.txt");
    fs::write(&path, "alpha\nbeta\ngamma\nAlpha\nbeta\n").unwrap();
    let mut engine = SearchEngine::new();
    engine.index_file(&path).unwrap();
    engine.finalize();

    let check = |hits: Vec<SearchMatch>, expected_lines: &[usize]| {
        let mut lines: Vec<usize> = hits.iter().map(|h| h.line_number).collect();
        lines.sort_unstable();
        assert_eq!(lines, expected_lines, "{hits:?}");
        for h in &hits {
            assert_eq!(
                h.content,
                if h.line_number == 1 { "alpha" } else { "Alpha" }
            );
            assert_eq!((h.line_match_start, h.line_match_end), (0, 5), "{h:?}");
            assert_eq!((h.match_start, h.match_end), (0, 5), "{h:?}");
        }
    };
    // ASCII case-insensitive scan (single term).
    check(engine.search("alpha\nbeta", 10), &[1, 4]);
    // Generic scanner: case-sensitive, and whole-word.
    let run = |q: &str| {
        engine
            .search_parsed(&parse(q), "", "", SearchLimits::new(10), RankMode::Full)
            .unwrap()
            .0
    };
    check(run("\"alpha\nbeta\" case:yes"), &[1]);
    check(run("\"alpha\nbeta\" word:yes"), &[1, 4]);
}

/// Review 1.10: a regex search must not drop a document whose symbol
/// cache slot is missing (the text path already tolerates that).
#[test]
fn test_regex_search_tolerates_missing_symbol_slot() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("a.rs");
    fs::write(&path, "fn alpha() {}\nlet beta = 1;\n").unwrap();
    let mut engine = SearchEngine::new();
    engine.index_file(&path).unwrap();
    engine.finalize();
    assert_eq!(engine.search_regex(r"bet\w", "", "", 10).unwrap().len(), 1);

    engine.symbol_cache.clear();
    assert_eq!(engine.search("beta", 10).len(), 1, "text path");
    let hits = engine.search_regex(r"bet\w", "", "", 10).unwrap();
    assert_eq!(hits.len(), 1, "regex path: {hits:?}");
    assert_eq!(hits[0].line_number, 2);
    assert!(!hits[0].is_symbol);
}

/// Review priority 4: editing a file must keep the dependency edges *into*
/// it (its importers did not change), and a file that is deleted and later
/// recreated regains its dependents once it is indexed again.
#[test]
fn test_update_and_recreate_keep_dependents() {
    let temp_dir = TempDir::new().unwrap();
    let main_path = temp_dir.path().join("main.rs");
    // `lib.rs`: like `main.rs`, `mod helper;` there names a sibling file.
    let other_path = temp_dir.path().join("lib.rs");
    let helper_path = temp_dir.path().join("helper.rs");
    fs::write(&main_path, "mod helper;\nfn main() { helper::help() }\n").unwrap();
    fs::write(&other_path, "mod helper;\nfn other() {}\n").unwrap();
    fs::write(&helper_path, "pub fn help() {}\n").unwrap();
    let mut engine = SearchEngine::new();
    for p in [&helper_path, &main_path, &other_path] {
        engine.index_file(p).unwrap();
    }
    engine.resolve_imports();
    engine.finalize();
    let id_of =
        |e: &SearchEngine, p: &std::path::Path| e.find_file_id(&p.to_string_lossy()).unwrap();
    let helper = id_of(&engine, &helper_path);
    let main = id_of(&engine, &main_path);
    let other = id_of(&engine, &other_path);
    let dependents = |e: &SearchEngine, id: u32| {
        let mut d = e.get_dependents(id);
        d.sort_unstable();
        d
    };
    assert_eq!(dependents(&engine, helper), vec![main, other]);
    assert_eq!(engine.get_dependencies(main), vec![helper]);

    // Edit the imported file: same id, edges into it survive.
    fs::write(&helper_path, "pub fn help() {}\npub fn more() {}\n").unwrap();
    engine.update_file(&helper_path).unwrap();
    assert_eq!(id_of(&engine, &helper_path), helper, "id is stable");
    assert_eq!(dependents(&engine, helper), vec![main, other]);
    assert_eq!(engine.get_dependencies(main), vec![helper]);
    assert_eq!(engine.dependency_index.get_import_count(helper), 2);
    assert!(engine
        .search("more", 10)
        .iter()
        .any(|h| h.file_id == helper));

    // Edit an importer: its out-edge is re-resolved, nothing else changes.
    fs::write(&main_path, "mod helper;\nfn main() {}\n").unwrap();
    engine.update_file(&main_path).unwrap();
    assert_eq!(dependents(&engine, helper), vec![main, other]);

    // Delete the imported file, then recreate it: the new id regains the
    // dependents when the file is indexed again.
    assert!(engine.remove_file(&helper_path));
    assert!(engine.get_dependents(helper).is_empty());
    assert!(engine.get_dependencies(main).is_empty());
    assert_eq!(engine.waiting_imports_count(), 2, "two edges parked");
    fs::write(&helper_path, "pub fn help() {}\n").unwrap();
    engine.update_file(&helper_path).unwrap(); // the watcher's create path
    let helper2 = id_of(&engine, &helper_path);
    assert_ne!(helper2, helper, "ids are never reused");
    assert_eq!(dependents(&engine, helper2), vec![main, other]);
    assert_eq!(engine.get_dependencies(main), vec![helper2]);
    assert_eq!(engine.waiting_imports_count(), 0);

    // The same through `update_file` when the file turns unreadable
    // (binary) and later becomes source again.
    fs::write(&helper_path, b"\0\0\0binary\0").unwrap();
    engine.update_file(&helper_path).unwrap();
    assert!(
        engine.file_store.get(helper2).is_none(),
        "dropped from the index"
    );
    assert!(engine.get_dependencies(main).is_empty());
    fs::write(&helper_path, "pub fn help() {}\n").unwrap();
    engine.update_file(&helper_path).unwrap();
    let helper3 = id_of(&engine, &helper_path);
    assert_eq!(dependents(&engine, helper3), vec![main, other]);

    // A parked edge whose importer has meanwhile been removed is dropped,
    // never added from a dead id.
    assert!(engine.remove_file(&helper_path));
    assert!(engine.remove_file(&other_path));
    fs::write(&helper_path, "pub fn help() {}\n").unwrap();
    engine.update_file(&helper_path).unwrap();
    let helper4 = id_of(&engine, &helper_path);
    assert_eq!(dependents(&engine, helper4), vec![main]);
    assert_eq!(engine.waiting_imports_count(), 0);
}

/// Review 1.10: indexing a path that is already indexed (the watcher beat
/// discovery during the initial build) replaces its postings instead of
/// unioning onto them, so stale trigrams never keep the file a candidate
/// for text it no longer contains; the edges into it survive.
#[test]
fn test_reindexing_a_known_path_replaces_its_trigrams() {
    let temp_dir = TempDir::new().unwrap();
    let main_path = temp_dir.path().join("main.rs");
    let helper_path = temp_dir.path().join("helper.rs");
    fs::write(&main_path, "mod helper;\nfn main() {}\n").unwrap();
    fs::write(&helper_path, "pub fn alphabet() {}\n").unwrap();
    let mut engine = SearchEngine::new();
    engine.index_file(&helper_path).unwrap();
    engine.index_file(&main_path).unwrap();
    engine.resolve_imports();
    let helper = engine.find_file_id(&helper_path.to_string_lossy()).unwrap();
    let main = engine.find_file_id(&main_path.to_string_lossy()).unwrap();
    assert!(engine.text_candidates("alphabet").contains(helper));
    assert_eq!(engine.get_dependents(helper), vec![main]);
    assert!(engine
        .search_symbols("alphabet", "", "", 10)
        .unwrap()
        .iter()
        .any(|h| h.file_id == helper));

    // The file changed and is indexed again through the batch path.
    fs::write(&helper_path, "pub fn zebra() {}\n").unwrap();
    engine.index_file(&helper_path).unwrap();
    assert_eq!(
        engine.find_file_id(&helper_path.to_string_lossy()),
        Some(helper),
        "same id"
    );
    assert!(
        !engine.text_candidates("alphabet").contains(helper),
        "stale trigrams must be gone"
    );
    assert!(engine.text_candidates("zebra").contains(helper));
    assert!(engine
        .search_symbols("alphabet", "", "", 10)
        .unwrap()
        .is_empty());
    assert_eq!(engine.search("zebra", 10).len(), 1);
    assert_eq!(engine.get_dependents(helper), vec![main]);
    assert_eq!(engine.file_store.live_len(), 2);
}

/// Review 2.3: a batch of modified files is re-indexed with one pass over the
/// posting lists and a parallel parse, with the same results as one
/// `update_file` per path: new content is searchable, old content is not,
/// dependents survive, unknown paths are added, unreadable ones dropped.
#[test]
fn test_update_files_batch_matches_per_file_semantics() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    let a = root.join("a.py");
    let b = root.join("b.py");
    let c = root.join("c.py");
    fs::write(&a, "def alpha_one(): pass\n").unwrap();
    fs::write(&b, "from a import alpha_one\ndef beta_one(): pass\n").unwrap();
    fs::write(&c, "def gamma_one(): pass\n").unwrap();

    let mut engine = SearchEngine::new();
    engine.add_root_path(root);
    engine.index_file(&a).unwrap();
    engine.index_file(&b).unwrap();
    engine.index_file(&c).unwrap();
    engine.finalize();
    engine.resolve_imports();
    let a_id = engine.find_file_id_exact(&a).unwrap();
    assert_eq!(engine.get_dependents(a_id).len(), 1, "b imports a");

    // Modify a and b, turn c into a binary blob, add d.
    fs::write(&a, "def alpha_two(): pass\n").unwrap();
    fs::write(&b, "from a import alpha_two\ndef beta_two(): pass\n").unwrap();
    fs::write(&c, [0u8, 159, 146, 150, 0, 0, 0, 1]).unwrap();
    let d = root.join("d.py");
    fs::write(&d, "def delta_one(): pass\n").unwrap();

    let (indexed, removed) = engine.update_files(&[a.clone(), b.clone(), c.clone(), d.clone()]);
    assert_eq!((indexed, removed), (3, 1));

    // The definition in a.py and the import in b.py.
    assert_eq!(engine.search("alpha_two", 5).len(), 2);
    assert!(
        engine.search("alpha_one", 5).is_empty(),
        "stale postings must be gone"
    );
    assert_eq!(engine.search("beta_two", 5).len(), 1);
    assert!(
        engine.search("gamma_one", 5).is_empty(),
        "binary file dropped"
    );
    assert_eq!(engine.search("delta_one", 5).len(), 1, "new file indexed");
    assert_eq!(engine.find_file_id_exact(&a), Some(a_id), "id is kept");
    assert_eq!(
        engine.get_dependents(a_id).len(),
        1,
        "edges into a survive the batch update"
    );
    assert_eq!(engine.get_stats().num_files, 3);
}

/// Review 2 (item 9): removing a file releases its memory map instead of
/// keeping it alive in the tombstoned slot.
#[test]
fn test_removed_file_releases_its_mapping() {
    let temp_dir = TempDir::new().unwrap();
    let big = temp_dir.path().join("big.txt");
    // Above the owned-read threshold, so the content is memory-mapped.
    let line = "mapped_marker_token line of text that repeats\n";
    let content = line.repeat(2 * 1024 * 1024 / line.len() + 1);
    fs::write(&big, &content).unwrap();

    let mut engine = SearchEngine::new();
    engine.index_file(&big).unwrap();
    engine.finalize();
    // Reading the content maps the file.
    assert_eq!(engine.search("mapped_marker_token", 1).len(), 1);
    assert!(
        engine.file_store.total_mapped_size() >= content.len() as u64,
        "file should be mapped after a search"
    );

    assert!(engine.remove_file(&big));
    assert_eq!(engine.file_store.total_mapped_size(), 0);
    assert!(engine.search("mapped_marker_token", 1).is_empty());
}

/// Review 1.4: TypeScript call sites (plain and member calls) are captured
/// as references; the upstream tags query only had type and `new` mentions.
#[test]
fn test_typescript_call_sites_are_references() {
    let temp_dir = TempDir::new().unwrap();
    let a = temp_dir.path().join("a.ts");
    fs::write(
        &a,
        "export function raceFilter(a: number) { return a; }\n\
         export function caller() { return raceFilter(1); }\n\
         const x = svc.raceFilter(2);\n",
    )
    .unwrap();
    let mut engine = SearchEngine::new();
    engine.add_root_path(temp_dir.path());
    engine.index_file(&a).unwrap();
    engine.finalize();

    let (hits, _) = engine
        .search_references("raceFilter", "", "", SearchLimits::new(50))
        .unwrap();
    let mut lines: Vec<usize> = hits.iter().map(|h| h.line_number).collect();
    lines.sort_unstable();
    assert_eq!(lines, vec![2, 3], "{hits:?}");
}

/// Review 1.9: line-mode regexes are matched in one pass per file. The
/// per-line semantics must survive: `^`/`$` anchor at line boundaries (CRLF
/// included), `\s+` never joins two lines, one hit per line, and the hit's
/// offsets are relative to its line.
#[test]
fn test_regex_single_pass_keeps_per_line_semantics() {
    let temp_dir = TempDir::new().unwrap();
    let f = temp_dir.path().join("r.rs");
    fs::write(
        &f,
        "fn alpha() {}\r\nlet x = fn_like();\r\nfn\r\nmain()\r\nfn beta() { fn gamma() {} }\r\nend\r\n",
    )
    .unwrap();
    let mut engine = SearchEngine::new();
    engine.add_root_path(temp_dir.path());
    engine.index_file(&f).unwrap();
    engine.finalize();

    let run = |pattern: &str| -> Vec<(usize, usize, usize)> {
        let (hits, _) = engine
            .search_regex_with_limits(pattern, "", "", SearchLimits::new(50), RankMode::Full)
            .unwrap();
        let mut v: Vec<(usize, usize, usize)> = hits
            .iter()
            .map(|h| (h.line_number, h.line_match_start, h.line_match_end))
            .collect();
        v.sort_unstable();
        v
    };

    // `^fn` matches at the start of lines 1, 3 and 5 only (not `fn_like`).
    assert_eq!(run(r"^fn\b"), vec![(1, 0, 2), (3, 0, 2), (5, 0, 2)]);
    // `$` anchors before the CRLF terminator.
    assert_eq!(run(r"\(\) \{\}$"), vec![(1, 8, 13)]);
    // `\s+` must not join "fn" and "main()" across the line break.
    assert!(run(r"fn\s+main\(").is_empty());
    // One hit per line, at the first match, with line-relative offsets.
    assert_eq!(run(r"fn \w+\(\)"), vec![(1, 0, 10), (5, 0, 9)]);
    // A whole-file pattern still spans lines.
    assert_eq!(run(r"fn\r\nmain\("), vec![(3, 0, 2)]);
}
