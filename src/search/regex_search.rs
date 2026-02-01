//! Regex search with trigram acceleration.
//!
//! This module provides regex search that uses trigram pre-filtering to avoid
//! scanning all files. It extracts literal strings from regex patterns and uses
//! them for trigram-based candidate filtering.

use anyhow::{Context, Result};
use regex::Regex;
use regex_syntax::hir::{Hir, HirKind, Literal};

/// Result of analyzing a regex pattern for trigram acceleration.
#[derive(Debug)]
pub struct RegexAnalysis {
    /// Compiled regex for matching
    pub regex: Regex,
    /// Extracted literal strings that can be used for trigram filtering
    pub literals: Vec<String>,
    /// Whether this regex can be accelerated (has usable literals >= 3 chars)
    pub is_accelerated: bool,
}

impl RegexAnalysis {
    /// Analyze a regex pattern and extract literals for trigram pre-filtering.
    ///
    /// # Arguments
    /// * `pattern` - The regex pattern to analyze
    ///
    /// # Returns
    /// A `RegexAnalysis` containing the compiled regex and extracted literals.
    pub fn analyze(pattern: &str) -> Result<Self> {
        let regex =
            Regex::new(pattern).with_context(|| format!("Invalid regex pattern: {}", pattern))?;

        let literals = match regex_syntax::parse(pattern) {
            Ok(hir) => extract_literals_from_hir(&hir),
            Err(_) => vec![],
        };

        // Regex is accelerated if we have at least one literal >= 3 chars
        let is_accelerated = literals.iter().any(|l| l.len() >= 3);

        Ok(Self {
            regex,
            literals,
            is_accelerated,
        })
    }

    /// Get the longest literal for use as primary trigram filter.
    pub fn best_literal(&self) -> Option<&str> {
        self.literals
            .iter()
            .filter(|l| l.len() >= 3)
            .max_by_key(|l| l.len())
            .map(|s| s.as_str())
    }
}

/// Extract literal strings from a regex HIR (High-level Intermediate Representation).
fn extract_literals_from_hir(hir: &Hir) -> Vec<String> {
    let mut literals = Vec::new();
    extract_literals_recursive(hir, &mut literals);
    literals
}

/// Recursively extract literals from HIR nodes.
fn extract_literals_recursive(hir: &Hir, literals: &mut Vec<String>) {
    match hir.kind() {
        HirKind::Literal(lit) => {
            // In regex-syntax 0.8.x, Literal is a wrapper around Box<[u8]>
            if let Some(s) = literal_to_string(lit) {
                if !s.is_empty() {
                    literals.push(s);
                }
            }
        }
        HirKind::Concat(subs) => {
            // Concatenate consecutive literals
            let mut current = String::new();
            for sub in subs.iter() {
                if let HirKind::Literal(lit) = sub.kind() {
                    if let Some(s) = literal_to_string(lit) {
                        current.push_str(&s);
                    }
                } else {
                    // Hit non-literal - save what we have and recurse
                    if current.len() >= 3 {
                        literals.push(current.clone());
                    }
                    current.clear();
                    extract_literals_recursive(sub, literals);
                }
            }
            // Don't forget trailing literal
            if current.len() >= 3 {
                literals.push(current);
            }
        }
        HirKind::Alternation(alts) => {
            // For alternation, extract from all branches
            for alt in alts.iter() {
                extract_literals_recursive(alt, literals);
            }
        }
        HirKind::Capture(capture) => {
            // Recurse into capture groups (was Group in older versions)
            extract_literals_recursive(&capture.sub, literals);
        }
        HirKind::Repetition(rep) => {
            // Recurse into repetitions (the literal might still be useful)
            extract_literals_recursive(&rep.sub, literals);
        }
        _ => {
            // Other HIR kinds (Empty, Look, Class) don't contain extractable literals
        }
    }
}

/// Convert a regex-syntax Literal to a String.
/// In regex-syntax 0.8.x, Literal is a newtype struct wrapping Box<[u8]>.
/// We access the inner bytes directly via `.0` as there's no public accessor method.
/// This is compatible with regex-syntax 0.8.x; may need adjustment for future versions.
fn literal_to_string(lit: &Literal) -> Option<String> {
    // Literal in 0.8.x is a newtype wrapper around Box<[u8]>
    // Access the bytes and try to convert to UTF-8
    std::str::from_utf8(&lit.0).ok().map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_literal() {
        let analysis = RegexAnalysis::analyze("hello").unwrap();
        assert!(analysis.is_accelerated);
        assert!(analysis.literals.contains(&"hello".to_string()));
    }

    #[test]
    fn test_literal_with_special_chars() {
        let analysis = RegexAnalysis::analyze(r"fn\s+main").unwrap();
        // Should extract "main" as a literal (fn is only 2 chars, below trigram threshold)
        assert!(analysis.literals.iter().any(|l| l.contains("main")));
    }

    #[test]
    fn test_regex_with_alternation() {
        let analysis = RegexAnalysis::analyze(r"(hello|world)").unwrap();
        assert!(analysis.is_accelerated);
        // Should have both alternatives
        assert!(analysis.literals.contains(&"hello".to_string()));
        assert!(analysis.literals.contains(&"world".to_string()));
    }

    #[test]
    fn test_no_literals() {
        let analysis = RegexAnalysis::analyze(r"[0-9]+").unwrap();
        // No extractable literals >= 3 chars
        assert!(!analysis.is_accelerated);
    }

    #[test]
    fn test_short_literal() {
        let analysis = RegexAnalysis::analyze(r"fn").unwrap();
        // "fn" is only 2 chars, not enough for a trigram
        assert!(!analysis.is_accelerated);
    }

    #[test]
    fn test_complex_pattern() {
        let analysis = RegexAnalysis::analyze(r"impl\s+Display\s+for").unwrap();
        assert!(analysis.is_accelerated);
        // Should extract "impl", "Display", "for" as literals
        assert!(analysis.literals.iter().any(|l| l.contains("impl")));
        assert!(analysis.literals.iter().any(|l| l.contains("Display")));
    }

    #[test]
    fn test_escaped_chars() {
        let analysis = RegexAnalysis::analyze(r"\.unwrap\(\)").unwrap();
        assert!(analysis.is_accelerated);
        // Should extract ".unwrap()" or parts of it
        assert!(analysis.literals.iter().any(|l| l.contains("unwrap")));
    }

    #[test]
    fn test_best_literal() {
        let analysis = RegexAnalysis::analyze(r"fn\s+handle_request").unwrap();
        let best = analysis.best_literal();
        assert!(best.is_some());
        // The longest literal should be "handle_request"
        assert!(best.unwrap().len() >= 3);
    }

    #[test]
    fn test_regex_matches() {
        let analysis = RegexAnalysis::analyze(r"fn\s+\w+").unwrap();
        assert!(analysis.regex.is_match("fn main"));
        assert!(analysis.regex.is_match("fn   test"));
        assert!(!analysis.regex.is_match("function main"));
    }
}
