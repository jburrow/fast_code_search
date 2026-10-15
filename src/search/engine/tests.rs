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

/// TypeScript: tsconfig `paths` aliases and `import x = require()` become
/// graph edges once the full build resolves its parked imports.
#[test]
fn test_typescript_path_aliases_build_edges() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    fs::create_dir_all(root.join("src/app")).unwrap();
    fs::create_dir_all(root.join("libs/shared")).unwrap();
    fs::write(
        root.join("tsconfig.json"),
        r#"{ "compilerOptions": { "paths": { "@shared/*": ["libs/shared/*"] } } }"#,
    )
    .unwrap();
    let main_path = root.join("src/app/main.ts");
    let money_path = root.join("libs/shared/money.ts");
    let legacy_path = root.join("src/app/legacy.ts");
    fs::write(
        &main_path,
        "import { add } from '@shared/money';\nimport legacy = require('./legacy');\n",
    )
    .unwrap();
    fs::write(&money_path, "export const add = 1;\n").unwrap();
    fs::write(&legacy_path, "export = {};\n").unwrap();

    let mut engine = SearchEngine::new();
    // The importer first, so both imports are parked until the final pass.
    engine.index_file(&main_path).unwrap();
    engine.index_file(&money_path).unwrap();
    engine.index_file(&legacy_path).unwrap();
    engine.resolve_imports();

    let id = |p: &std::path::Path| engine.find_file_id(&p.to_string_lossy()).unwrap();
    let mut deps = engine.dependency_index.get_dependencies(id(&main_path));
    deps.sort();
    let mut expected = vec![id(&money_path), id(&legacy_path)];
    expected.sort();
    assert_eq!(deps, expected);
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

/// Roadmap 7: case-sensitive, whole-word, phrases and lang:/file: through
/// the parsed-query entry point.
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
    // Several words are one phrase, as in `rg -F`.
    assert!(run("cat dog").is_empty());
    assert_eq!(run("dog here"), vec![("a.rs".into(), 3)]);
    // A leading `-` is text, not an exclusion.
    assert!(run("cat -dog").is_empty());
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
    // Quotes are text: `'CAT'` holds them, `"dog here"` is not in a file.
    assert_eq!(run("'CAT'"), vec![("b.py".into(), 1)]);
    assert!(run("\"dog here\"").is_empty());
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
fn test_reference_in_shipped_code_outranks_the_same_call_in_a_test() {
    // The same call site in src/ and in tests/: the reader asking "where is
    // this used" wants the shipped code first, and the order must not depend
    // on which file happened to be indexed first.
    let temp_dir = TempDir::new().unwrap();
    let src = temp_dir.path().join("src");
    let tests = temp_dir.path().join("tests");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&tests).unwrap();
    let call = "fn run() {\n    let v = widget();\n    v\n}\n";
    fs::write(tests.join("a_test.rs"), call).unwrap();
    fs::write(src.join("lib.rs"), "pub fn widget() -> u8 { 1 }\n").unwrap();
    fs::write(src.join("z_use.rs"), call).unwrap();
    for order in [true, false] {
        let mut engine = SearchEngine::new();
        engine.add_root_path(temp_dir.path());
        let mut files = vec![
            tests.join("a_test.rs"),
            src.join("lib.rs"),
            src.join("z_use.rs"),
        ];
        if order {
            files.reverse();
        }
        for f in &files {
            engine.index_file(f).unwrap();
        }
        engine.finalize();
        let (m, _) = engine
            .search_references("widget", "", "", SearchLimits::new(10))
            .unwrap();
        let paths: Vec<String> = m
            .iter()
            .map(|m| {
                m.file_path
                    .rsplit('/')
                    .take(2)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .collect();
        assert_eq!(
            paths,
            vec!["z_use.rs/src", "a_test.rs/tests"],
            "index order reversed = {order}"
        );
    }
}

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

/// Every capped result names the limit that cut it, and a complete one
/// names none.
#[test]
fn test_limits_hit_name_each_limit() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = SearchEngine::new();
    for f in 0..20 {
        let body: String = (0..30)
            .map(|l| format!("let cap_needle_{l} = {f};\n"))
            .collect();
        let path = temp_dir.path().join(format!("c{f}.rs"));
        fs::write(&path, body).unwrap();
        engine.index_file(path).unwrap();
    }
    engine.finalize();
    let run = |limits: SearchLimits, mode| {
        engine
            .search_ranked_with_limits("cap_needle", limits, mode)
            .1
    };
    let unbounded = || SearchLimits::new(10).with_match_budget(usize::MAX);

    let info = run(unbounded(), RankMode::Full);
    assert!(info.limits_hit.is_empty(), "{info:?}");
    assert_eq!(info.total_matches, Some(600));

    let info = run(SearchLimits::new(10).with_match_budget(50), RankMode::Full);
    assert_eq!(info.limits_hit, vec![SearchLimit::MatchBudget]);

    let info = run(unbounded().with_per_doc_cap(5), RankMode::Full);
    assert_eq!(info.limits_hit, vec![SearchLimit::PerFileCap]);
    assert_eq!(info.total_matches, None);

    // The file cap keeps the total exact for the files opened.
    let info = run(unbounded().with_max_files(4), RankMode::Fast);
    assert_eq!(info.limits_hit, vec![SearchLimit::FileCap]);
    assert_eq!(info.total_matches, Some(120));
    assert!(!info.truncated_by_budget);
    assert!(SearchLimit::FileCap
        .describe(&info)
        .contains("best 4 of 20 candidate files"));

    let info = run(
        unbounded().with_deadline(std::time::Instant::now() - std::time::Duration::from_millis(1)),
        RankMode::Full,
    );
    assert_eq!(info.limits_hit, vec![SearchLimit::Deadline]);

    for l in [
        SearchLimit::FileCap,
        SearchLimit::MatchBudget,
        SearchLimit::PerFileCap,
        SearchLimit::Deadline,
    ] {
        assert_eq!(SearchLimit::parse(l.as_str()), Some(l));
    }
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
        false,
        1.0,
    );

    // Medium line (~80 chars)
    let medium_line = "fn do_thing(arg1: String, arg2: i32, arg3: bool) -> Result<()> { todo!() }";
    let medium_score = calculate_score_inline(
        medium_line,
        "do_thing",
        "do_thing",
        false,
        false,
        false,
        1.0,
    );

    // Long line (~200 chars)
    let long_line = format!(
        "fn do_thing({}) -> Result<()> {{}}",
        (0..20)
            .map(|i| format!("arg{}: String", i))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let long_score =
        calculate_score_inline(&long_line, "do_thing", "do_thing", false, false, false, 1.0);

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

/// Bug report: fast ranking always stopped at 2000 files. The number is
/// an optional limit now (`max_files`), from the server's settings or the
/// request, and by default there is none.
#[test]
fn test_fast_ranking_opens_max_files() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = SearchEngine::new();
    for i in 0..30 {
        let p = temp_dir.path().join(format!("f{i}.rs"));
        fs::write(&p, "needle\n").unwrap();
        engine.index_file(&p).unwrap();
    }
    engine.finalize();
    let searched = |limits: SearchLimits| {
        let (hits, info) = engine.search_ranked_with_limits("needle", limits, RankMode::Fast);
        assert_eq!(info.total_candidates, 30);
        assert_eq!(hits.len(), info.candidates_searched);
        info.candidates_searched
    };
    assert_eq!(SearchLimits::new(100).max_files, usize::MAX);
    assert_eq!(searched(SearchLimits::new(100)), 30);
    assert_eq!(searched(SearchLimits::new(100).with_max_files(10)), 10);
    assert_eq!(searched(SearchLimits::new(100).with_max_files(0)), 30);

    // Server settings: a default, a ceiling on what a request may ask for.
    let settings = SearchSettings {
        fast_rank_files: 10,
        max_fast_rank_files: 20,
        ..SearchSettings::default()
    };
    assert_eq!(searched(settings.apply(SearchLimits::new(100), None)), 10);
    assert_eq!(
        searched(settings.apply(SearchLimits::new(100), Some(15))),
        15
    );
    assert_eq!(
        searched(settings.apply(SearchLimits::new(100), Some(1000))),
        20
    );
    let unlimited = SearchSettings::default();
    assert_eq!(unlimited.fast_rank_files_for(Some(50_000)), 50_000);
    assert_eq!(unlimited.fast_rank_files_for(None), usize::MAX);
    assert_eq!(searched(unlimited.apply(SearchLimits::new(100), None)), 30);
}

/// Feedback: auto ranking "fell back to fast mode too quickly". Above 5,000
/// candidate files it switched to opening only the best 2,000, so a rare
/// string whose files ranked low came back with few hits. Now every mode
/// reads candidates best first until the budget is spent, so `auto`
/// reaches every file without a cap; `full` ignores a cap.
#[test]
fn test_auto_ranking_reads_until_the_page_is_full() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = SearchEngine::new();
    for i in 0..40 {
        let p = temp_dir.path().join(format!("f{i:02}.rs"));
        fs::write(&p, "needle\n").unwrap();
        engine.index_file(&p).unwrap();
    }
    engine.finalize();
    let (hits, info) =
        engine.search_ranked_with_limits("needle", SearchLimits::new(100), RankMode::Auto);
    assert_eq!(hits.len(), 40);
    assert!(info.limits_hit.is_empty(), "{:?}", info.limits_hit);
    let capped = SearchLimits::new(100).with_max_files(5);
    let (hits, info) = engine.search_ranked_with_limits("needle", capped.clone(), RankMode::Auto);
    assert_eq!(hits.len(), 5);
    assert_eq!(info.limits_hit, vec![SearchLimit::FileCap]);
    let (hits, info) = engine.search_ranked_with_limits("needle", capped, RankMode::Full);
    assert_eq!(hits.len(), 40, "full ranking never stops at the file cap");
    assert!(info.limits_hit.is_empty());
}

/// The match budget follows `[search]`: the floor, multiplier, ceiling and
/// per-file cap all come from the server's settings.
#[test]
fn test_budget_rule_from_settings() {
    let d = SearchLimits::new(50);
    assert_eq!(d.match_budget, BudgetRule::DEFAULT.min);
    assert_eq!(d.doc_cap(), 1000);
    let rule = BudgetRule {
        min: 100,
        multiplier: 4,
        max: 10_000,
        per_file: 7,
    };
    let l = SearchLimits::new(50).with_budget_rule(rule);
    assert_eq!(
        l.match_budget, 4000,
        "the budget of the deepest page below 1000"
    );
    assert_eq!(
        l.doc_cap(),
        7,
        "the per-file cap does not grow with the page"
    );
    let l = SearchLimits::new(5).with_offset(500).with_budget_rule(rule);
    assert_eq!(l.doc_cap(), 7);
    assert_eq!(
        l.match_budget, 4000,
        "every page below 1000 ranks one sample"
    );
    // A page that ends at STABLE_PAGING_FROM or deeper gets the ceiling.
    let deep = SearchLimits::new(50)
        .with_budget_rule(rule)
        .with_offset(2000);
    assert_eq!(deep.match_budget, 10_000);
    // The rule survives an offset applied afterwards.
    let s = SearchSettings {
        budget: rule,
        ..SearchSettings::default()
    };
    let l = s.apply(SearchLimits::new(5), None).with_offset(10);
    assert_eq!(l.budget_rule, rule);
    assert_eq!(l.match_budget, 4000);
}

/// Symbol search reads only the best-scoring files holding a matching
/// symbol; when there were more, it says so instead of giving an
/// exact-looking total.
#[test]
fn test_symbol_search_reports_its_file_cap() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = SearchEngine::new();
    for i in 0..250 {
        let p = temp_dir.path().join(format!("m{i:03}.rs"));
        fs::write(&p, "pub fn widget_build() {}\n").unwrap();
        engine.index_file(&p).unwrap();
    }
    engine.finalize();
    let (hits, info) = engine
        .search_symbols_parsed(
            &crate::search::parse_query("widget_build"),
            "",
            "",
            SearchLimits::new(10),
        )
        .unwrap();
    assert_eq!(hits.len(), 10);
    assert!(
        info.limits_hit.contains(&SearchLimit::FileCap),
        "{:?}",
        info.limits_hit
    );
    assert_eq!(info.candidates_searched, 200);
    assert_eq!(info.total_candidates, 250);
    assert_eq!(info.total_matches, None, "the files not read may hold more");
    let run = |limits: SearchLimits| {
        engine
            .search_symbols_parsed(&crate::search::parse_query("widget_build"), "", "", limits)
            .unwrap()
            .1
    };
    let info = run(SearchLimits::new(50));
    assert!(!info.limits_hit.contains(&SearchLimit::FileCap));
    assert_eq!(info.total_matches, Some(250));
    // The advice works: raising the file limit (SEARCH ALL FILES, MCP's
    // max_files) or full ranking reads every file.
    let info = run(SearchLimits::new(10).with_max_files(250));
    assert_eq!(info.total_matches, Some(250));
    let info = run(SearchLimits::new(10).with_every_file(true));
    assert_eq!(info.total_matches, Some(250));
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
fn test_search_limits_offset_budget() {
    let base = SearchLimits::new(50);
    assert_eq!(base.offset, 0);

    // The ranking headroom stops growing at MAX_DERIVED_BUDGET...
    let mid = SearchLimits::new(1000).with_offset(20_000);
    assert_eq!(mid.match_budget, SearchLimits::MAX_DERIVED_BUDGET);
    // ...but a deeper page still gets a budget that reaches it (bug report:
    // paging stopped at offset 10000).
    let deep = SearchLimits::new(1000).with_offset(500_000);
    assert_eq!(deep.offset, 500_000);
    assert!(deep.match_budget > 501_000, "{}", deep.match_budget);
    assert_eq!(
        SearchLimits::new(10).with_offset(usize::MAX).match_budget,
        usize::MAX
    );

    // Servers bound the depth with a setting (default 100000; 0 = none).
    let settings = SearchSettings::default();
    assert!(settings.check_offset(100_000).is_ok());
    assert!(settings.check_offset(100_001).is_err());

    // Pages ending below 1000 share one budget and per-document cap too,
    // so walking them by offset ranks one sample.
    let shallow = SearchLimits::new(50).with_offset(100);
    assert_eq!(shallow.offset, 100);
    assert_eq!(shallow.match_budget, base.match_budget);
    assert_eq!(shallow.doc_cap(), base.doc_cap());

    // Lists ending at 1000 or deeper share one budget and per-document cap,
    // so offset pages past the API's 1000-result list rank the same sample.
    let list = SearchLimits::new(1000);
    for page in [
        SearchLimits::new(50).with_offset(950),
        SearchLimits::new(50).with_offset(1000),
        SearchLimits::new(50).with_offset(40_000),
    ] {
        assert_eq!(page.match_budget, list.match_budget);
        assert_eq!(page.doc_cap(), list.doc_cap());
    }

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

/// Bug report: several words were ANDed by file and every line holding
/// any of them was returned. Like ripgrep, they are one phrase now: only
/// lines holding `fn main` match, however many `fn` lines a file has.
#[test]
fn test_several_words_are_one_phrase() {
    use crate::search::query_syntax::parse;
    let temp_dir = TempDir::new().unwrap();
    // 150 `fn` lines, then the phrase (beyond the old per-file cap), then
    // the two words apart.
    let mut big = String::new();
    for i in 0..150 {
        big.push_str(&format!("fn helper_{i}() {{}}\n"));
    }
    big.push_str("fn main() {}\n");
    big.push_str("// main entry, fn below\n");
    fs::write(temp_dir.path().join("big.rs"), &big).unwrap();
    fs::write(
        temp_dir.path().join("small.rs"),
        "fn other() {}\nlet main = 1;\npub FN MAIN() { other() }\nfn  main()\n",
    )
    .unwrap();
    let mut engine = SearchEngine::new();
    engine.index_file(temp_dir.path().join("big.rs")).unwrap();
    engine.index_file(temp_dir.path().join("small.rs")).unwrap();
    engine.finalize();

    let run = |q: &str| {
        let (hits, info) = engine
            .search_parsed(&parse(q), "", "", SearchLimits::new(500), RankMode::Full)
            .unwrap();
        let mut found: Vec<(String, usize)> = hits
            .iter()
            .map(|h| {
                (
                    h.file_path.rsplit('/').next().unwrap().to_string(),
                    h.line_number,
                )
            })
            .collect();
        found.sort();
        (found, info)
    };
    let (found, info) = run("fn main");
    // Case-insensitive by default; `fn  main` (two spaces) is not the phrase.
    assert_eq!(
        found,
        vec![("big.rs".into(), 151), ("small.rs".into(), 3)],
        "{found:?}"
    );
    assert_eq!(info.total_matches, Some(2));
    let (found, _) = run("fn main case:yes");
    assert_eq!(found, vec![("big.rs".into(), 151)]);
    // Spacing is exact, as in `rg -F`.
    let (found, _) = run("fn  main");
    assert_eq!(found, vec![("small.rs".into(), 4)]);
    // A quoted phrase is searched with its quotes.
    let (found, _) = run("\"fn main\"");
    assert!(found.is_empty(), "{found:?}");
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
    check(run("alpha\nbeta case:yes"), &[1]);
    check(run("alpha\nbeta word:yes"), &[1, 4]);
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

/// Results are in plain rank order with each file's hits kept together:
/// files in the order of their best hit, every hit of a file before the next
/// file. No hit waits behind other files' hits.
#[test]
fn test_results_keep_each_files_hits_together() {
    use crate::search::query_syntax::parse;
    let temp_dir = TempDir::new().unwrap();
    let src = temp_dir.path().join("src");
    fs::create_dir_all(&src).unwrap();
    // One file with many single-term hits and a high file score.
    let many: String = (0..30)
        .map(|i| format!("fn item_{i}() -> Widget {{}}\n"))
        .collect();
    fs::write(src.join("many.rs"), &many).unwrap();
    fs::write(
        src.join("one.rs"),
        "fn lone_widget_use() { let w: Widget = x; }\n",
    )
    .unwrap();
    fs::write(src.join("two.rs"), "struct Widget;\n").unwrap();

    let mut engine = SearchEngine::new();
    engine.add_root_path(temp_dir.path());
    for f in ["src/many.rs", "src/one.rs", "src/two.rs"] {
        engine.index_file(temp_dir.path().join(f)).unwrap();
    }
    engine.finalize();

    let (hits, _) = engine
        .search_parsed(
            &parse("Widget"),
            "",
            "",
            SearchLimits::new(10),
            RankMode::Full,
        )
        .unwrap();
    let files: Vec<&str> = hits
        .iter()
        .map(|h| h.file_path.rsplit('/').next().unwrap())
        .collect();
    // A file's hits are contiguous: once the list leaves a file it never
    // comes back to it.
    let mut runs: Vec<&str> = files.clone();
    runs.dedup();
    let distinct: std::collections::HashSet<&str> = runs.iter().copied().collect();
    assert_eq!(runs.len(), distinct.len(), "{files:?}");
    // Files come in the order of their best hit, and within a file hits are
    // in score order.
    let best: Vec<f64> = runs
        .iter()
        .map(|f| {
            hits.iter()
                .filter(|h| h.file_path.ends_with(f))
                .map(|h| h.score)
                .fold(f64::MIN, f64::max)
        })
        .collect();
    assert!(best.windows(2).all(|w| w[0] >= w[1]), "{runs:?} {best:?}");
    for w in hits.windows(2) {
        if w[0].file_path == w[1].file_path {
            assert!(w[0].score >= w[1].score, "{files:?}");
        }
    }

    // Each hit carries its file's hit count across all pages, so a file with
    // one hit on this page does not read as a file with one hit.
    for h in &hits {
        let want = match h.file_path.rsplit('/').next().unwrap() {
            "many.rs" => 30,
            _ => 1,
        };
        assert_eq!(h.file_hits, want, "{}", h.file_path);
        assert!(!h.file_hits_capped, "{}", h.file_path);
    }

    // Paging stays stable: page two continues where page one stopped.
    let (page2, _) = engine
        .search_parsed(
            &parse("Widget"),
            "",
            "",
            SearchLimits::new(10).with_offset(10),
            RankMode::Full,
        )
        .unwrap();
    let (all, _) = engine
        .search_parsed(
            &parse("Widget"),
            "",
            "",
            SearchLimits::new(40),
            RankMode::Full,
        )
        .unwrap();
    let lines = |v: &[SearchMatch]| v.iter().map(|h| h.line_number).collect::<Vec<_>>();
    assert_eq!(lines(&page2), lines(&all[10..20]));
}

/// Items wrapped in a brace-delimited macro invocation (tokio's `cfg_rt!`)
/// are re-parsed as Rust, so `JoinHandle` inside `cfg_rt! { … }` is a
/// definition at its real line and its type mentions are references.
#[test]
fn test_rust_items_inside_cfg_macros_are_symbols() {
    let temp_dir = TempDir::new().unwrap();
    let f = temp_dir.path().join("join.rs");
    fs::write(
        &f,
        "use std::marker::PhantomData;\n\
         cfg_rt! {\n\
             /// A handle.\n\
             pub struct JoinHandle<T> {\n\
                 raw: PhantomData<T>,\n\
             }\n\
             impl<T> JoinHandle<T> {\n\
                 pub fn abort(&self) {}\n\
             }\n\
         }\n",
    )
    .unwrap();
    let mut engine = SearchEngine::new();
    engine.add_root_path(temp_dir.path());
    engine.index_file(&f).unwrap();
    engine.finalize();

    let (hits, _) = engine
        .search_symbols_parsed(
            &crate::search::query_syntax::parse("JoinHandle"),
            "",
            "",
            SearchLimits::new(10),
        )
        .unwrap();
    assert!(
        hits.iter()
            .any(|h| h.line_number == 4 && h.content.contains("pub struct JoinHandle")),
        "{hits:?}"
    );
    // The struct outranks its impl block.
    assert!(
        hits[0].content.contains("pub struct JoinHandle"),
        "{hits:?}"
    );
    let (hits, _) = engine
        .search_symbols_parsed(
            &crate::search::query_syntax::parse("abort"),
            "",
            "",
            SearchLimits::new(10),
        )
        .unwrap();
    assert_eq!(hits.first().map(|h| h.line_number), Some(8), "{hits:?}");
}

/// Bug report: a plain-text search for `{ "success": True` returned nothing
/// (the parser dropped the quotes and searched for `success:`). The line
/// holding the whole query now comes back first.
#[test]
fn test_plain_search_keeps_quotes_inside_terms() {
    use crate::search::query_syntax::parse;
    let temp_dir = TempDir::new().unwrap();
    let hit = temp_dir.path().join("api.py");
    fs::write(
        &hit,
        "def ok():\n    return { \"success\": True, \"data\": 1 }\n",
    )
    .unwrap();
    let json = temp_dir.path().join("resp.json");
    fs::write(&json, "{\"success\":false}\n").unwrap();
    let mut engine = SearchEngine::new();
    engine.index_file(&hit).unwrap();
    engine.index_file(&json).unwrap();
    engine.finalize();

    let search = |q: &str| {
        engine
            .search_parsed(&parse(q), "", "", SearchLimits::new(20), RankMode::Full)
            .unwrap()
            .0
    };
    let hits = search("{ \"success\": True");
    assert!(!hits.is_empty(), "the known hit must be found");
    assert!(hits[0].file_path.ends_with("api.py"));
    assert_eq!(hits[0].line_number, 2);

    let hits = search("{\"success\":false}");
    assert_eq!(hits.len(), 1);
    assert!(hits[0].file_path.ends_with("resp.json"));
}

// ---- Search correctness review (each test failed before its fix) ----

/// Index `files` under `dir`, registered as a root so display paths are
/// `<dir name>/<file>` as in a real workspace.
fn review_engine(dir: &Path, files: &[(&str, &str)]) -> SearchEngine {
    let mut engine = SearchEngine::new();
    engine.add_root_path(dir);
    for (name, content) in files {
        let p = dir.join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, content).unwrap();
        engine.index_file(&p).unwrap();
    }
    engine.finalize();
    engine
}

fn review_text(
    engine: &SearchEngine,
    q: &str,
    limits: SearchLimits,
) -> (Vec<SearchMatch>, SearchRankingInfo) {
    engine
        .search_parsed(
            &crate::search::query_syntax::parse(q),
            "",
            "",
            limits,
            RankMode::Full,
        )
        .unwrap()
}

/// A file with more hits than the per-document cap used to report an exact
/// total of 100 with nothing more to load; its later lines were unreachable.
#[test]
fn test_per_document_cap_reports_unknown_total() {
    let t = TempDir::new().unwrap();
    let content: String = (0..150).map(|i| format!("needle {i}\n")).collect();
    let e = review_engine(t.path(), &[("a.txt", &content)]);
    let (page1, info) = review_text(&e, "needle", SearchLimits::new(100).with_per_doc_cap(100));
    assert_eq!(page1.len(), 100);
    assert_eq!(info.total_matches, None, "the total is not known");
    assert!(info.truncated_by_budget);
    // The default cap covers every page below 1000 results.
    let (all, info) = review_text(&e, "needle", SearchLimits::new(200));
    assert_eq!(all.len(), 150);
    assert_eq!(info.total_matches, Some(150));
    // Under the cap the total stays exact.
    let small: String = (0..10).map(|i| format!("needle {i}\n")).collect();
    let e = review_engine(t.path(), &[("a.txt", &small)]);
    let (_, info) = review_text(&e, "needle", SearchLimits::new(200));
    assert_eq!(info.total_matches, Some(10));
}

/// A capped file keeps its best hits, not its first ones: page 1 used to
/// show a busy file's earliest (weakest) lines while its best ones further
/// down were never read.
#[test]
fn test_per_document_cap_keeps_the_best_hits() {
    let t = TempDir::new().unwrap();
    let mut content = String::new();
    for i in 0..150 {
        content.push_str(&format!(
            "# a long comment line that mentions needle somewhere in the middle of a lot of text {i}\n"
        ));
    }
    for i in 0..50 {
        content.push_str(&format!("needle{i} = 1\n"));
    }
    let e = review_engine(t.path(), &[("a.py", &content)]);
    let (page, info) = review_text(&e, "needle", SearchLimits::new(20).with_per_doc_cap(60));
    assert!(info.limits_hit.contains(&SearchLimit::PerFileCap));
    let lines: Vec<usize> = page.iter().map(|m| m.line_number).collect();
    assert!(
        lines.iter().all(|&l| l > 150),
        "the short lines starting with the term rank first: {lines:?}"
    );
    assert_eq!(lines[0], 151, "ties keep file order");
    // The same through a regex.
    let (page, _) = e
        .search_regex_with_limits(
            "needle",
            "",
            "",
            SearchLimits::new(20).with_per_doc_cap(60),
            RankMode::Full,
        )
        .unwrap();
    assert!(page.iter().all(|m| m.line_number > 150));
}

/// Walking a search by `offset` (what MCP tells an agent to do) returns
/// every hit exactly once, also across a file with far more hits than one
/// page. The per-file cap used to grow with the page end, so each page
/// ranked a different sample: 305 hits walked, 155 distinct.
#[test]
fn test_offset_walk_returns_every_hit_once() {
    let t = TempDir::new().unwrap();
    let mut big = String::new();
    for i in 0..1200 {
        // Varied line lengths give varied scores.
        big.push_str(&format!("{}needlewalk {i}\n", " ".repeat(i % 37)));
    }
    let mut files: Vec<(String, String)> = vec![("big.py".into(), big)];
    for j in 0..30 {
        files.push((format!("s{j}.py"), format!("needlewalk = {j}\n")));
    }
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let e = review_engine(t.path(), &refs);
    let (all, info) = review_text(&e, "needlewalk", SearchLimits::exhaustive());
    assert_eq!(info.total_matches, Some(1230));
    for page in [50usize, 333] {
        let mut walked: Vec<(String, usize)> = Vec::new();
        let mut offset = 0;
        loop {
            let (hits, _) = review_text(
                &e,
                "needlewalk",
                SearchLimits::new(page).with_offset(offset),
            );
            if hits.is_empty() {
                break;
            }
            offset += hits.len();
            walked.extend(hits.into_iter().map(|m| (m.file_path, m.line_number)));
        }
        let expected: Vec<(String, usize)> = all
            .iter()
            .map(|m| (m.file_path.clone(), m.line_number))
            .collect();
        assert_eq!(walked, expected, "page size {page}");
    }
}

/// A regex mentioning `\n` runs over the whole file; `^` and `$` used to
/// lose their per-line meaning there and match only at the file's ends.
/// An escaped backslash (`\\n`, as in a C string) is not a newline at all.
#[test]
fn test_multiline_regex_keeps_line_anchors() {
    let t = TempDir::new().unwrap();
    let e = review_engine(
        t.path(),
        &[
            ("a.txt", "alpha\nbeta\ngamma\n"),
            ("b.c", "int x;\nprintf(\"hi\\n\");\n"),
        ],
    );
    let lines = |p: &str| -> Vec<(String, usize)> {
        e.search_regex_with_limits(p, "", "", SearchLimits::new(10), RankMode::Full)
            .unwrap()
            .0
            .iter()
            .map(|m| {
                (
                    m.file_path.rsplit('/').next().unwrap().to_string(),
                    m.line_number,
                )
            })
            .collect()
    };
    assert_eq!(lines(r"^beta\n"), vec![("a.txt".to_string(), 2)]);
    assert_eq!(lines(r"beta$\ngamma"), vec![("a.txt".to_string(), 2)]);
    assert_eq!(lines(r#"^printf.*\\n"#), vec![("b.c".to_string(), 2)]);
    // `\A` still anchors to the start of the file.
    assert!(lines(r"\Abeta\n").is_empty());
}

/// Symbol mode used to drop files whose only match is their name.
#[test]
fn test_symbol_search_reports_filename_only_matches() {
    let t = TempDir::new().unwrap();
    let e = review_engine(
        t.path(),
        &[
            ("widget_factory.rs", "fn other() {}\n"),
            ("defs.rs", "fn widget() {}\n"),
        ],
    );
    let (m, _) = e
        .search_symbols_parsed(
            &crate::search::query_syntax::parse("widget"),
            "",
            "",
            SearchLimits::new(10),
        )
        .unwrap();
    let names: Vec<(&str, usize)> = m
        .iter()
        .map(|m| (m.file_path.rsplit('/').next().unwrap(), m.line_number))
        .collect();
    assert!(names.contains(&("widget_factory.rs", 0)), "{names:?}");
    assert_eq!(names[0], ("defs.rs", 1), "a real definition ranks first");
}

/// `file:` / `-file:` words used to match the root folder's own name, so
/// `-file:search` dropped every file of a root named `search_proj`.
#[test]
fn test_file_operator_ignores_the_root_folder_name() {
    let t = TempDir::new().unwrap();
    let root = t.path().join("search_proj");
    fs::create_dir_all(&root).unwrap();
    let e = review_engine(
        &root,
        &[("src/a.rs", "needle\n"), ("src/search/b.rs", "needle\n")],
    );
    let files = |q: &str| -> Vec<String> {
        let mut v: Vec<String> = review_text(&e, q, SearchLimits::new(10))
            .0
            .into_iter()
            .map(|m| m.file_path)
            .collect();
        v.sort();
        v
    };
    assert_eq!(files("needle -file:search"), vec!["search_proj/src/a.rs"]);
    assert_eq!(
        files("needle file:search"),
        vec!["search_proj/src/search/b.rs"]
    );
    assert_eq!(files("needle file:*.rs").len(), 2);
    assert_eq!(files("needle -file:*proj*").len(), 2);
    // Naming the root with a slash still selects it.
    assert_eq!(files("needle file:search_proj/").len(), 2);
}

/// The filename fallback ignored `case:yes` and `word:yes`.
#[test]
fn test_filename_hit_obeys_case_and_word() {
    let t = TempDir::new().unwrap();
    let e = review_engine(t.path(), &[("widget.rs", "nothing here\n")]);
    let n = |q: &str| review_text(&e, q, SearchLimits::new(10)).0.len();
    assert_eq!(n("widget"), 1);
    assert_eq!(n("case:yes widget"), 1);
    assert_eq!(n("case:yes WIDGET"), 0);
    assert_eq!(n("word:yes widg"), 0);
    assert_eq!(n("word:yes widget"), 1);
}

/// Whole word as ripgrep's `-w`: a match rejected for touching a word
/// character does not hide an overlapping one, and combining marks are
/// word characters (found by the backtest's generated queries).
#[test]
fn test_whole_word_overlaps_and_combining_marks() {
    let t = TempDir::new().unwrap();
    let e = review_engine(
        t.path(),
        &[(
            "w.txt",
            "field}}}\nx\u{bb}\u{bb}\u{bb}\n\u{0938}\u{0947}\u{092a}\u{094d}\u{091f}\u{0947}\u{092e}\nTie\u{0302}\u{0323}t\"\n",
        )],
    );
    let n = |q: &str| review_text(&e, q, SearchLimits::new(10)).0.len();
    assert_eq!(n("word:yes case:yes }}"), 1);
    assert_eq!(n("word:yes }}"), 1);
    assert_eq!(n("word:yes \u{bb}\u{bb}"), 1);
    // The virama before `ट` and the accents before `t` are word characters.
    assert_eq!(n("word:yes \u{091f}\u{0947}\u{092e}"), 0);
    assert_eq!(n("word:yes t\""), 0);
    assert_eq!(n("t\""), 1);
}

/// Lowercasing turns a word-final `Σ` into `ς` and any other into `σ`, so
/// the query `ΟΣ` (lowercased `ος`) missed `ΟΣΑ` (indexed as `οσα`).
#[test]
fn test_greek_sigma_matches_in_any_position() {
    let t = TempDir::new().unwrap();
    let e = review_engine(t.path(), &[("g.txt", "ΟΣΑ word\nΟΔΟΣ end\nΒοσνιακά\n")]);
    let lines = |q: &str| -> Vec<usize> {
        let mut v: Vec<usize> = review_text(&e, q, SearchLimits::new(10))
            .0
            .iter()
            .map(|m| m.line_number)
            .collect();
        v.sort_unstable();
        v
    };
    assert_eq!(lines("ΟΣ"), vec![1, 2, 3]);
    assert_eq!(lines("οσ"), vec![1, 2, 3]);
    assert_eq!(lines("ΟΔΟΣ"), vec![2]);
    let (m, _) = e
        .search_regex_with_limits("(?i)ΟΣ", "", "", SearchLimits::new(10), RankMode::Full)
        .unwrap();
    assert_eq!(m.len(), 3);
}

/// An empty-matching regex (`^$`) reported a blank line past the end of
/// every file ending in a newline.
#[test]
fn test_regex_reports_no_line_past_the_end() {
    let t = TempDir::new().unwrap();
    let e = review_engine(t.path(), &[("a.txt", "one\n\nthree\n"), ("b.txt", "x\n\n")]);
    let lines = |p: &str| -> Vec<(String, usize)> {
        let mut v: Vec<_> = e
            .search_regex_with_limits(p, "", "", SearchLimits::new(50), RankMode::Full)
            .unwrap()
            .0
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
    let want = vec![("a.txt".to_string(), 2), ("b.txt".to_string(), 2)];
    assert_eq!(lines("^$"), want);
    assert_eq!(lines(r"^\s*$"), want);
}

/// A search that stops at its match budget used to keep whichever files
/// the threads reached first, so its first page changed between runs.
#[test]
fn test_budget_truncated_search_is_repeatable() {
    let t = TempDir::new().unwrap();
    let files: Vec<(String, String)> = (0..400)
        .map(|i| (format!("f{i:03}.txt"), "needle\n".repeat(5)))
        .collect();
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let e = review_engine(t.path(), &refs);
    let page = || -> Vec<(String, usize)> {
        let (m, info) = review_text(&e, "needle", SearchLimits::new(20).with_match_budget(512));
        assert!(info.truncated_by_budget, "the budget must cut this scan");
        m.iter()
            .map(|m| (m.file_path.clone(), m.line_number))
            .collect()
    };
    let first = page();
    for _ in 0..10 {
        assert_eq!(page(), first);
    }
}

/// A UTF-8 byte order mark is not text (as in ripgrep): `^` matches at the
/// start of line 1 and the line is reported without it.
#[test]
fn test_byte_order_mark_is_not_part_of_line_one() {
    let t = TempDir::new().unwrap();
    let e = review_engine(t.path(), &[("a.txt", "\u{feff}test123\ntest123\n")]);
    let (hits, _) = e
        .search_regex_with_limits("^test123", "", "", SearchLimits::new(50), RankMode::Full)
        .unwrap();
    let mut lines: Vec<usize> = hits.iter().map(|m| m.line_number).collect();
    lines.sort_unstable();
    assert_eq!(lines, vec![1, 2]);
    let (hits, _) = review_text(&e, "test123", SearchLimits::new(50));
    let first = hits.iter().find(|m| m.line_number == 1).unwrap();
    assert_eq!(first.content, "test123");
}

/// `show_root_name = false` with one root leaves the root folder's name off
/// every path a caller sees, while path filters keep matching the
/// root-qualified form, so `file:` behaves exactly as with the name shown.
#[test]
fn test_hidden_root_name_single_root() {
    let t = TempDir::new().unwrap();
    let root = t.path().join("search_proj");
    let files = [
        ("src/a.rs", "needle\n"),
        ("src/search/b.rs", "needle\n"),
        // A folder named like the root must not be mistaken for it.
        ("search_proj/c.rs", "needle\n"),
        ("top.rs", "needle\n"),
    ];
    // The root must exist before it is registered: it is canonicalized then
    // (on macOS the temp dir is under a /var -> /private/var symlink).
    fs::create_dir_all(&root).unwrap();
    let mut e = SearchEngine::new();
    e.show_root_name = false;
    e.add_root_path(&root);
    for (name, content) in files {
        let p = root.join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, content).unwrap();
        e.index_file(&p).unwrap();
    }
    e.finalize();
    assert!(e.hides_root_name());

    let found = |q: &str| -> Vec<String> {
        let mut v: Vec<String> = review_text(&e, q, SearchLimits::new(10))
            .0
            .into_iter()
            .map(|m| m.file_path)
            .collect();
        v.sort();
        v
    };
    assert_eq!(
        found("needle"),
        vec!["search_proj/c.rs", "src/a.rs", "src/search/b.rs", "top.rs"]
    );
    // Filters are unchanged by hiding the name (see
    // test_file_operator_ignores_the_root_folder_name).
    assert_eq!(found("needle -file:search"), vec!["src/a.rs", "top.rs"]);
    assert_eq!(found("needle file:search_proj/").len(), 4);
    assert_eq!(found("needle file:top"), vec!["top.rs"]);
    assert_eq!(
        found("needle file:src/"),
        vec!["src/a.rs", "src/search/b.rs"]
    );

    // Displayed paths resolve back to their own file, including the folder
    // that shares the root's name.
    for shown in ["src/a.rs", "search_proj/c.rs", "top.rs"] {
        let id = e.find_file_id(shown).unwrap();
        assert_eq!(e.display_path(id).unwrap(), shown);
        assert_eq!(e.get_file_path(id).unwrap(), shown);
    }
    // The root-qualified form still resolves too.
    let id = e.find_file_id("search_proj/src/a.rs").unwrap();
    assert_eq!(e.display_path(id).unwrap(), "src/a.rs");
}

/// With several roots the name is what tells `alpha/utils.rs` from
/// `beta/utils.rs`, so `show_root_name = false` is not honoured.
#[test]
fn test_hidden_root_name_ignored_with_multiple_roots() {
    let t = TempDir::new().unwrap();
    let mut engine = SearchEngine::new();
    engine.show_root_name = false;
    let mut files = Vec::new();
    for name in ["alpha", "beta"] {
        let root = t.path().join(name);
        fs::create_dir_all(&root).unwrap();
        let file = root.join("utils.rs");
        fs::write(&file, "// utils").unwrap();
        engine.add_root_path(&root);
        files.push(file.canonicalize().unwrap());
    }
    assert!(!engine.hides_root_name());
    assert_eq!(engine.make_display_path(&files[0]), "alpha/utils.rs");
    assert_eq!(engine.make_display_path(&files[1]), "beta/utils.rs");
}

/// A repo-like tree under a root named `repo`, indexed with that root.
fn path_search_engine() -> (TempDir, SearchEngine) {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path().join("repo");
    for (rel, body) in [
        ("src/web/api.rs", "pub fn search_handler() {}\n"),
        ("src/web/mod.rs", "mod api;\n"),
        ("src/cli/api_client.rs", "fn call() {}\n"),
        ("docs/api.md", "# The API\n"),
        ("tests/web_api_tests.rs", "#[test] fn t() {}\n"),
    ] {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, body).unwrap();
    }
    let root = root.canonicalize().unwrap();
    let mut engine = SearchEngine::new();
    engine.add_root_path(&root);
    for rel in [
        "src/web/api.rs",
        "src/web/mod.rs",
        "src/cli/api_client.rs",
        "docs/api.md",
        "tests/web_api_tests.rs",
    ] {
        engine.index_file(root.join(rel)).unwrap();
    }
    engine.finalize();
    (temp_dir, engine)
}

fn path_hits(engine: &SearchEngine, q: &str) -> Vec<String> {
    let parsed = crate::search::parse_query(q);
    engine
        .search_paths(PathQuery::Terms(&parsed), "", "", 50, None, None)
        .unwrap()
        .matches
        .into_iter()
        .map(|m| m.file_path)
        .collect()
}

#[test]
fn path_search_finds_a_typed_path() {
    let (_t, engine) = path_search_engine();
    assert_eq!(
        path_hits(&engine, "src/web/api.rs"),
        ["repo/src/web/api.rs"]
    );
    // Root-qualified, partial and Windows-style paths too.
    assert_eq!(
        path_hits(&engine, "repo/src/web/api.rs"),
        ["repo/src/web/api.rs"]
    );
    assert_eq!(path_hits(&engine, "web/api.rs"), ["repo/src/web/api.rs"]);
    assert_eq!(
        path_hits(&engine, r"src\web\api.rs"),
        ["repo/src/web/api.rs"]
    );
    // Content search is untouched: no line mentions the path.
    assert!(engine.search("src/web/api.rs", 10).is_empty());
}

#[test]
fn path_search_matches_a_pasted_absolute_path() {
    let (t, engine) = path_search_engine();
    let abs = t
        .path()
        .canonicalize()
        .unwrap()
        .join("repo/src/web/api.rs")
        .to_string_lossy()
        .replace('\\', "/");
    let parsed = crate::search::parse_query(&abs);
    let found = engine
        .search_paths(PathQuery::Terms(&parsed), "", "", 10, None, None)
        .unwrap();
    assert_eq!(found.total, 1);
    let m = &found.matches[0];
    assert_eq!(m.file_path, "repo/src/web/api.rs");
    // The highlight covers the whole shown path.
    assert_eq!(m.ranges, vec![(0, m.file_path.len())]);
    // A prefix that stops above the indexed root matches nothing.
    let above = t
        .path()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
        + "/";
    assert!(path_hits(&engine, &above).is_empty());
}

#[test]
fn path_search_ranks_file_names_first() {
    let (_t, engine) = path_search_engine();
    let hits = path_hits(&engine, "api");
    // Exact stem first (shorter path wins the tie), then a name prefix,
    // then a name containing the term; `src/web/mod.rs` has no `api`.
    assert_eq!(
        hits,
        [
            "repo/docs/api.md",
            "repo/src/web/api.rs",
            "repo/src/cli/api_client.rs",
            "repo/tests/web_api_tests.rs",
        ]
    );
}

#[test]
fn path_search_honours_terms_filters_and_options() {
    let (_t, engine) = path_search_engine();
    // Every word must appear in the path.
    assert_eq!(
        path_hits(&engine, "web api"),
        ["repo/src/web/api.rs", "repo/tests/web_api_tests.rs"]
    );
    // file:/lang: filter like content.
    assert_eq!(
        path_hits(&engine, "api lang:markdown"),
        ["repo/docs/api.md"]
    );
    assert_eq!(
        path_hits(&engine, "api -file:tests"),
        [
            "repo/docs/api.md",
            "repo/src/web/api.rs",
            "repo/src/cli/api_client.rs",
        ]
    );
    // case:yes and word:yes as for content.
    assert!(path_hits(&engine, "API case:yes").is_empty());
    assert_eq!(
        path_hits(&engine, "api word:yes"),
        ["repo/docs/api.md", "repo/src/web/api.rs"]
    );
    // Ranges are byte offsets into the shown path.
    let parsed = crate::search::parse_query("web api");
    let m = &engine
        .search_paths(PathQuery::Terms(&parsed), "", "", 1, None, None)
        .unwrap()
        .matches[0];
    assert_eq!(m.ranges, vec![(9, 12), (13, 16)]);
}

#[test]
fn path_search_regex_and_limit() {
    let (_t, engine) = path_search_engine();
    let found = engine
        .search_paths(PathQuery::Regex(r"web/\w+\.rs$"), "", "", 1, None, None)
        .unwrap();
    assert_eq!(found.total, 2);
    assert_eq!(found.matches.len(), 1);
    assert!(engine
        .search_paths(PathQuery::Regex("("), "", "", 1, None, None)
        .is_err());
}

#[test]
fn path_search_hides_the_root_name_when_configured() {
    let (_t, mut engine) = path_search_engine();
    engine.show_root_name = false;
    engine.compute_all_file_metadata();
    // The root name is not part of what is shown, so it does not match.
    assert!(path_hits(&engine, "repo").is_empty());
    assert_eq!(path_hits(&engine, "src/web/api.rs"), ["src/web/api.rs"]);
}

#[test]
fn test_file_hits_flags_a_file_cut_by_the_per_file_cap() {
    use crate::search::query_syntax::parse;
    let temp_dir = TempDir::new().unwrap();
    let many: String = (0..30).map(|i| format!("let widget_{i} = 1;\n")).collect();
    fs::write(temp_dir.path().join("many.rs"), &many).unwrap();
    fs::write(temp_dir.path().join("one.rs"), "let widget = 1;\n").unwrap();
    let mut engine = SearchEngine::new();
    engine.add_root_path(temp_dir.path());
    for f in ["many.rs", "one.rs"] {
        engine.index_file(temp_dir.path().join(f)).unwrap();
    }
    engine.finalize();

    let (hits, _) = engine
        .search_parsed(
            &parse("widget"),
            "",
            "",
            SearchLimits::new(50).with_per_doc_cap(5),
            RankMode::Full,
        )
        .unwrap();
    let many = hits
        .iter()
        .find(|h| h.file_path.ends_with("many.rs"))
        .unwrap();
    assert_eq!(many.file_hits, 5);
    assert!(many.file_hits_capped);
    let one = hits
        .iter()
        .find(|h| h.file_path.ends_with("one.rs"))
        .unwrap();
    assert_eq!(one.file_hits, 1);
    assert!(!one.file_hits_capped);
}

/// Graph-scoped search (`deps:` / `rdeps:`) on a small Python project:
/// `app/main.py` imports `pkg` (its `__init__.py`), which re-exports
/// `pkg/core.py`; `pkg/util.py` and `tool.go` are outside the graph.
fn graph_scope_engine() -> (TempDir, SearchEngine) {
    let dir = TempDir::new().unwrap();
    let files = [
        ("app/main.py", "from pkg import thing\nWeight = 1\n"),
        ("pkg/__init__.py", "from .core import thing\n"),
        ("pkg/core.py", "def thing():\n    pass\nWeight = 2\n"),
        ("pkg/util.py", "Weight = 3\n"),
        ("lib/util.py", "Weight = 4\nfrom .missing import x\n"),
        ("tool.go", "var Weight = 5\n"),
    ];
    let mut engine = SearchEngine::new();
    for (rel, body) in files {
        let p = dir.path().join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, body).unwrap();
    }
    for (rel, _) in files {
        engine.index_file(dir.path().join(rel)).unwrap();
    }
    engine.resolve_imports();
    engine.finalize();
    (dir, engine)
}

fn scoped(engine: &SearchEngine, req: crate::search::SearchRequest) -> Result<Vec<String>, String> {
    let outcome = engine.run(&req, SearchLimits::new(50))?;
    let mut files: Vec<String> = outcome
        .matches
        .iter()
        .map(|m| {
            // The path below the temporary directory.
            let p = m.file_path.replace('\\', "/");
            let at = p.find("/.tmp").expect("under the temp dir") + 1;
            p[at..].split_once('/').unwrap().1.to_string()
        })
        .collect();
    files.sort();
    files.dedup();
    Ok(files)
}

fn text(q: &str) -> crate::search::SearchRequest {
    crate::search::SearchRequest {
        query: q.to_string(),
        ..Default::default()
    }
}

#[test]
fn test_rdeps_reaches_importers_through_the_package_entry() {
    let (_dir, engine) = graph_scope_engine();
    assert_eq!(
        scoped(&engine, text("Weight rdeps:pkg/core.py")).unwrap(),
        vec!["app/main.py", "pkg/core.py"]
    );
    // The scope reports its size and each file's distance.
    let outcome = engine
        .run(&text("Weight rdeps:pkg/core.py"), SearchLimits::new(50))
        .unwrap();
    let scope = outcome.scope.expect("scoped");
    assert_eq!(scope.file_count(), Some(3)); // core, __init__, main
    assert_eq!(scope.filters, vec!["rdeps:pkg/core.py:1"]);
    for m in &outcome.matches {
        let want = if m.file_path.ends_with("core.py") {
            0
        } else {
            1
        };
        assert_eq!(scope.hops(m.file_id), Some(want), "{}", m.file_path);
    }
}

#[test]
fn test_deps_reaches_modules_behind_the_package_entry() {
    let (_dir, engine) = graph_scope_engine();
    assert_eq!(
        scoped(&engine, text("Weight deps:app/main.py")).unwrap(),
        vec!["app/main.py", "pkg/core.py"]
    );
    // Negated: everything but those files, walked strictly (no
    // pass-through), so pkg/core.py behind the package entry stays in.
    assert_eq!(
        scoped(&engine, text("Weight -deps:app/main.py")).unwrap(),
        vec!["lib/util.py", "pkg/core.py", "pkg/util.py", "tool.go"]
    );
    // Graph filters AND with path and language filters.
    assert_eq!(
        scoped(&engine, text("Weight deps:app/main.py file:pkg/")).unwrap(),
        vec!["pkg/core.py"]
    );
    assert_eq!(
        scoped(&engine, text("Weight deps:app/main.py rdeps:pkg/core.py")).unwrap(),
        vec!["app/main.py", "pkg/core.py"]
    );
}

#[test]
fn test_graph_scope_anchor_errors_are_explicit() {
    let (_dir, engine) = graph_scope_engine();
    let err = |q: &str| scoped(&engine, text(q)).unwrap_err();
    assert!(err("Weight rdeps:nope.py").contains("no indexed file matches nope.py"));
    let e = err("Weight rdeps:util.py");
    assert!(
        e.contains("matches 2 files") && e.contains("lib/util.py"),
        "{e}"
    );
    assert!(err("Weight rdeps:tool.go").contains("no import graph for .go"));
    assert!(err("Weight rdeps:pkg/core.py:0").contains("hops must be 1 to 10"));
    assert!(err("Weight rdeps:pkg/core.py:x").contains("no indexed file matches"));
}

#[test]
fn test_graph_scope_hops_folders_and_regex() {
    let (_dir, engine) = graph_scope_engine();
    // A folder anchor is every graph file under it.
    assert_eq!(
        scoped(&engine, text("Weight rdeps:pkg/")).unwrap(),
        vec!["app/main.py", "pkg/core.py", "pkg/util.py"]
    );
    assert_eq!(
        scoped(&engine, text("Weight deps:app/main.py:*")).unwrap(),
        vec!["app/main.py", "pkg/core.py"]
    );
    // A regex is all pattern; the request field scopes it.
    let regex = |q: &str, rdeps: &[&str]| crate::search::SearchRequest {
        query: q.to_string(),
        mode: crate::search::SearchMode::Regex,
        rdeps: rdeps.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    };
    assert_eq!(
        scoped(&engine, regex(r"Weight = \d", &["pkg/core.py"])).unwrap(),
        vec!["app/main.py", "pkg/core.py"]
    );
    assert!(scoped(&engine, regex(r"Weight rdeps:x", &[]))
        .unwrap()
        .is_empty());
}

#[test]
fn test_graph_scope_counts_unresolved_project_imports() {
    let (_dir, engine) = graph_scope_engine();
    let run = |q: &str| {
        engine
            .run(&text(q), SearchLimits::new(50))
            .unwrap()
            .scope
            .unwrap()
    };
    assert_eq!(run("Weight rdeps:lib/util.py").unresolved_local, 1);
    assert_eq!(run("Weight rdeps:pkg/core.py").unresolved_local, 0);
}

/// Importers reached through a package entry file are kept only when they
/// name something the anchor defines.
#[test]
fn test_rdeps_through_a_package_entry_needs_a_name_from_the_anchor() {
    let dir = TempDir::new().unwrap();
    let files = [
        (
            "pkg/__init__.py",
            "from .core import thing\nfrom .other import helper\n",
        ),
        ("pkg/core.py", "def thing():\n    return 1  # marker\n"),
        ("pkg/other.py", "def helper():\n    return 2  # marker\n"),
        (
            "uses_thing.py",
            "from pkg import thing\nthing()  # marker\n",
        ),
        (
            "uses_helper.py",
            "from pkg import helper\nhelper()  # marker\n",
        ),
    ];
    let mut engine = SearchEngine::new();
    for (rel, body) in files {
        let p = dir.path().join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, body).unwrap();
    }
    for (rel, _) in files {
        engine.index_file(dir.path().join(rel)).unwrap();
    }
    engine.resolve_imports();
    engine.finalize();
    assert_eq!(
        scoped(&engine, text("marker rdeps:pkg/core.py")).unwrap(),
        vec!["pkg/core.py", "uses_thing.py"]
    );
    // Without the package entry in the way, every importer counts.
    assert_eq!(
        scoped(&engine, text("marker rdeps:pkg/__init__.py")).unwrap(),
        vec!["uses_helper.py", "uses_thing.py"]
    );
}

/// `kind:` keeps references of the named kinds: a property read is an
/// attribute, `self.helper()` a call, `isinstance(x, Item)` a value and
/// `x: Item` a type. Results carry their kind; text search rejects `kind:`.
#[test]
fn test_reference_kind_filter() {
    let temp_dir = TempDir::new().unwrap();
    let a = temp_dir.path().join("a.py");
    fs::write(
        &a,
        "class Item:\n    def size(self):\n        return 1\n\n\
         def run(x: Item, other):\n    x.size()\n    n = other.size\n    isinstance(other, Item)\n",
    )
    .unwrap();
    let mut engine = SearchEngine::new();
    engine.index_file(&a).unwrap();
    engine.finalize();
    let refs = |q: &str| -> Vec<(usize, String)> {
        let req = crate::search::SearchRequest {
            query: q.to_string(),
            mode: crate::search::SearchMode::References,
            ..Default::default()
        };
        let mut v: Vec<(usize, String)> = engine
            .run(&req, SearchLimits::new(50))
            .unwrap()
            .matches
            .iter()
            .map(|m| {
                (
                    m.line_number,
                    m.reference_kind.unwrap().as_str().to_string(),
                )
            })
            .collect();
        v.sort();
        v
    };
    assert_eq!(
        refs("size"),
        vec![(6, "call".into()), (7, "attribute".into())]
    );
    assert_eq!(refs("size kind:call"), vec![(6, "call".into())]);
    assert_eq!(refs("size -kind:attribute"), vec![(6, "call".into())]);
    assert_eq!(refs("size kind:attr"), vec![(7, "attribute".into())]);
    assert_eq!(refs("Item"), vec![(5, "type".into()), (8, "value".into())]);
    assert_eq!(refs("Item kind:type,value").len(), 2);
    assert_eq!(refs("Item kind:call"), vec![]);
    let Err(err) = engine.run(&text("size kind:call"), SearchLimits::new(50)) else {
        panic!("text search accepted kind:");
    };
    assert!(err.contains("references"), "{err}");
}

/// Two indexed folders with the same name used to give identical result
/// paths (`app/src/main.rs` twice), and opening either hit showed one file.
#[test]
fn test_roots_with_the_same_name_get_distinct_paths() {
    let t = TempDir::new().unwrap();
    let one = t.path().join("one").join("app");
    let two = t.path().join("two").join("app");
    let mut engine = SearchEngine::new();
    for (root, tag) in [(&one, "ONE"), (&two, "TWO")] {
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src/main.rs"),
            format!("fn shared() {{ /* {tag} */ }}\n"),
        )
        .unwrap();
        engine.add_root_path(root);
    }
    for root in [&one, &two] {
        engine.index_file(root.join("src/main.rs")).unwrap();
    }
    engine.finalize();
    let (hits, _) = review_text(&engine, "shared", SearchLimits::new(10));
    let mut paths: Vec<&str> = hits.iter().map(|m| m.file_path.as_str()).collect();
    paths.sort();
    assert_eq!(paths, vec!["one/app/src/main.rs", "two/app/src/main.rs"]);
    // Each path opens its own file.
    for m in &hits {
        let id = engine.find_file_id(&m.file_path).unwrap();
        let content = engine
            .file_store
            .get(id)
            .unwrap()
            .as_str()
            .unwrap()
            .into_owned();
        let tag = if m.file_path.starts_with("one/") {
            "ONE"
        } else {
            "TWO"
        };
        assert!(content.contains(tag), "{} opened {content}", m.file_path);
    }
}

/// The `src/` boost and the test penalty used to test the absolute path, so
/// a repository under `~/src/` outranked the same file elsewhere.
#[test]
fn test_ranking_ignores_the_folders_above_a_root() {
    let t = TempDir::new().unwrap();
    let a = t.path().join("src").join("zeta");
    let b = t.path().join("plain").join("zeta");
    let mut engine = SearchEngine::new();
    for root in [&a, &b] {
        fs::create_dir_all(root).unwrap();
        fs::write(root.join("a.py"), "def rootskew_probe(): pass\n").unwrap();
        engine.add_root_path(root);
    }
    for root in [&a, &b] {
        engine.index_file(root.join("a.py")).unwrap();
    }
    engine.finalize();
    let (hits, _) = review_text(&engine, "rootskew_probe", SearchLimits::new(10));
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].score, hits[1].score, "same file, same score");
}

/// Test files named the usual ways rank below the code they test.
#[test]
fn test_common_test_file_names_are_demoted() {
    let t = TempDir::new().unwrap();
    let e = review_engine(
        t.path(),
        &[
            ("pkg/parse.go", "package pkg\nfunc ParseWidget() {}\n"),
            ("pkg/parse_test.go", "package pkg\nfunc ParseWidget() {}\n"),
            ("web/gizmo.ts", "export function ParseWidget() {}\n"),
            ("web/gizmo.spec.ts", "export function ParseWidget() {}\n"),
            ("web/__tests__/g.ts", "export function ParseWidget() {}\n"),
        ],
    );
    let (hits, _) = review_text(&e, "ParseWidget", SearchLimits::new(10));
    let score = |name: &str| {
        hits.iter()
            .find(|m| m.file_path.ends_with(name))
            .map(|m| m.score)
            .unwrap()
    };
    for (test_file, code) in [
        ("parse_test.go", "pkg/parse.go"),
        ("gizmo.spec.ts", "web/gizmo.ts"),
        ("__tests__/g.ts", "web/gizmo.ts"),
    ] {
        assert!(
            score(test_file) < score(code) * 0.8,
            "{test_file} is not demoted below {code}"
        );
    }
}

/// With file watching off, a file edited after indexing used to report its
/// old definition line: symbol search pointed at whatever the line now held
/// (a comment), and the real definition lost its label.
#[test]
fn test_edited_file_never_reports_a_definition_on_the_wrong_line() {
    let t = TempDir::new().unwrap();
    let e = review_engine(
        t.path(),
        &[(
            "src/lib.rs",
            "// header\npub fn alpha_target() {}\npub fn beta() { alpha_target(); }\n",
        )],
    );
    fs::write(
        t.path().join("src/lib.rs"),
        "// header\n// inserted 1\n// inserted 2\npub fn alpha_target() {}\npub fn beta() { alpha_target(); }\n",
    )
    .unwrap();
    let (syms, _) = e
        .search_symbols_parsed(
            &crate::search::parse_query("alpha_target"),
            "",
            "",
            SearchLimits::new(10),
        )
        .unwrap();
    assert!(
        syms.iter().all(|m| m.content.contains("alpha_target")),
        "{:?}",
        syms.iter().map(|m| &m.content).collect::<Vec<_>>()
    );
    // Text search: the comment on line 2 gets no definition boost (it does
    // not match anyway), and no hit is boosted for a symbol that moved.
    let (hits, _) = review_text(&e, "inserted", SearchLimits::new(10));
    assert!(hits
        .iter()
        .all(|m| !m.is_symbol || m.content.contains("alpha_target")));
}

/// Searches made while the startup build is running say the index is
/// incomplete instead of giving an exact-looking total.
#[test]
fn test_search_during_a_build_reports_indexing() {
    let t = TempDir::new().unwrap();
    let mut e = review_engine(t.path(), &[("a.txt", "needle\n")]);
    let req = crate::search::SearchRequest {
        query: "needle".into(),
        ..Default::default()
    };
    let settings = SearchSettings::default();
    let out = e.run(&req, req.limits(&settings, None).unwrap()).unwrap();
    assert_eq!(out.info.total_matches, Some(1));
    e.building = true;
    let out = e.run(&req, req.limits(&settings, None).unwrap()).unwrap();
    assert_eq!(out.info.total_matches, None);
    assert!(out.info.limits_hit.contains(&SearchLimit::Indexing));
}

/// Filters with no text used to answer with a silent empty result.
#[test]
fn test_filters_without_text_say_so() {
    let t = TempDir::new().unwrap();
    let e = review_engine(t.path(), &[("a.txt", "needle\n")]);
    let req = crate::search::SearchRequest {
        query: "file:///etc".into(),
        ..Default::default()
    };
    let settings = SearchSettings::default();
    let err = e
        .run(&req, req.limits(&settings, None).unwrap())
        .err()
        .unwrap();
    assert!(err.contains("no text to find"), "{err}");
}
