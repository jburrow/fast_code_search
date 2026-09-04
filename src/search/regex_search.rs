//! Regex search with trigram acceleration.
//!
//! This module provides regex search that uses trigram pre-filtering to avoid
//! scanning all files. It extracts literal strings from regex patterns and uses
//! them for trigram-based candidate filtering.

use anyhow::{Context, Result};
use regex::Regex;

/// Compiled-program size cap for user regexes (bytes). Patterns such as
/// `(a{1000}){1000}` would otherwise compile into hundreds of megabytes; the
/// `regex` crate rejects anything over this limit at build time, which the
/// API surfaces as a 400.
const REGEX_SIZE_LIMIT: usize = 4 * 1024 * 1024;
/// Per-thread lazy DFA cache cap (bytes).
const REGEX_DFA_SIZE_LIMIT: usize = 2 * 1024 * 1024;
use regex_syntax::hir::{Hir, HirKind, Literal};

/// Result of analyzing a regex pattern for trigram acceleration.
#[derive(Debug)]
pub struct RegexAnalysis {
    /// Compiled regex for matching
    pub regex: Regex,
    /// Extracted literal strings (flattened, for diagnostics/back-compat)
    pub literals: Vec<String>,
    /// Sound trigram constraints used to pre-filter candidate documents.
    ///
    /// The candidate set is the **intersection** of each constraint's matches;
    /// within a single constraint the literals are **alternatives** (union). Each
    /// literal is >= 3 chars. An empty vec means "no sound acceleration possible"
    /// — the caller must fall back to a full scan.
    ///
    /// This is what makes alternations correct: `hello|world` yields a single
    /// constraint `[["hello", "world"]]`, so a file containing only `world` is
    /// still a candidate (the old `best_literal()` approach required `hello`).
    /// Optional/`min==0` subexpressions contribute no constraint.
    pub constraints: Vec<Vec<String>>,
    /// Whether this regex can be accelerated (has at least one sound constraint)
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
        let regex = regex::RegexBuilder::new(pattern)
            .size_limit(REGEX_SIZE_LIMIT)
            .dfa_size_limit(REGEX_DFA_SIZE_LIMIT)
            .build()
            .with_context(|| format!("Invalid regex pattern: {}", pattern))?;

        let (literals, constraints) = match regex_syntax::parse(pattern) {
            Ok(hir) => (extract_literals_from_hir(&hir), extract_constraints(&hir)),
            Err(_) => (vec![], vec![]),
        };

        // Accelerated only when we have at least one SOUND constraint (a literal
        // that must appear, or an alternation where every branch contributes one).
        let is_accelerated = !constraints.is_empty();

        Ok(Self {
            regex,
            literals,
            constraints,
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

/// Extract sound trigram constraints from a regex HIR.
///
/// Each returned constraint is a set of alternative literals (union); the overall
/// candidate set is the intersection of all constraints. Only literals that MUST
/// appear in any matching string are used, so the pre-filter never drops a real
/// match (unlike picking a single "best" literal across alternation branches).
fn extract_constraints(hir: &Hir) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    collect_constraints(hir, &mut out);
    out
}

fn collect_constraints(hir: &Hir, out: &mut Vec<Vec<String>>) {
    match hir.kind() {
        HirKind::Concat(subs) => {
            // Merge consecutive literal children into one required literal run.
            let mut current = String::new();
            for sub in subs.iter() {
                if let HirKind::Literal(lit) = sub.kind() {
                    if let Some(s) = literal_to_string(lit) {
                        current.push_str(&s);
                    }
                } else {
                    if current.len() >= 3 {
                        out.push(vec![std::mem::take(&mut current)]);
                    }
                    current.clear();
                    collect_constraints(sub, out);
                }
            }
            if current.len() >= 3 {
                out.push(vec![current]);
            }
        }
        HirKind::Alternation(_) => {
            // Usable only if EVERY branch contributes a required literal, so that
            // the union of branch matches covers every possible match.
            if let Some(group) = alternation_constraint(hir) {
                out.push(group);
            }
        }
        HirKind::Capture(c) => collect_constraints(&c.sub, out),
        HirKind::Repetition(rep) => {
            // Only required when the subexpression must appear at least once.
            if rep.min >= 1 {
                collect_constraints(&rep.sub, out);
            }
        }
        HirKind::Literal(lit) => {
            if let Some(s) = literal_to_string(lit) {
                if s.len() >= 3 {
                    out.push(vec![s]);
                }
            }
        }
        _ => {}
    }
}

/// Build a union constraint for an alternation, or `None` if any branch lacks a
/// guaranteed >= 3-char literal (in which case the alternation cannot soundly
/// filter — a matching file might contain only that literal-less branch).
fn alternation_constraint(hir: &Hir) -> Option<Vec<String>> {
    if let HirKind::Alternation(alts) = hir.kind() {
        let mut group = Vec::with_capacity(alts.len());
        for alt in alts.iter() {
            group.push(branch_required_literal(alt)?);
        }
        if group.is_empty() {
            None
        } else {
            Some(group)
        }
    } else {
        None
    }
}

/// Return the longest literal that is guaranteed to appear in any string matching
/// `hir` (a single alternation branch), or `None` if no such >= 3-char literal exists.
fn branch_required_literal(hir: &Hir) -> Option<String> {
    fn consider(best: &mut Option<String>, candidate: &str) {
        if candidate.len() >= 3 && best.as_ref().is_none_or(|b| candidate.len() > b.len()) {
            *best = Some(candidate.to_string());
        }
    }

    match hir.kind() {
        HirKind::Literal(lit) => literal_to_string(lit).filter(|s| s.len() >= 3),
        HirKind::Concat(subs) => {
            // Longest run of consecutive (mandatory) literal children.
            let mut best: Option<String> = None;
            let mut current = String::new();
            for sub in subs.iter() {
                if let HirKind::Literal(lit) = sub.kind() {
                    if let Some(s) = literal_to_string(lit) {
                        current.push_str(&s);
                    }
                } else {
                    consider(&mut best, &current);
                    current.clear();
                }
            }
            consider(&mut best, &current);
            best
        }
        HirKind::Capture(c) => branch_required_literal(&c.sub),
        HirKind::Repetition(rep) if rep.min >= 1 => branch_required_literal(&rep.sub),
        _ => None,
    }
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
    fn test_alternation_constraint_is_union() {
        // hello|world must produce ONE constraint with BOTH literals (union),
        // so a file containing only "world" is still a candidate.
        let a = RegexAnalysis::analyze(r"hello|world").unwrap();
        assert!(a.is_accelerated);
        assert_eq!(a.constraints.len(), 1, "one alternation constraint");
        let group = &a.constraints[0];
        assert!(group.contains(&"hello".to_string()));
        assert!(group.contains(&"world".to_string()));
    }

    #[test]
    fn test_optional_prefix_not_required() {
        // (abc)?def — only "def" is required; "abc" is optional and must NOT
        // become a required constraint.
        let a = RegexAnalysis::analyze(r"(abc)?def").unwrap();
        assert!(a.is_accelerated);
        let flat: Vec<&String> = a.constraints.iter().flatten().collect();
        assert!(flat.iter().any(|l| l.as_str() == "def"));
        assert!(
            !a.constraints.iter().any(|g| g.len() == 1 && g[0] == "abc"),
            "optional abc must not be a required constraint"
        );
    }

    #[test]
    fn test_alternation_short_branch_disables_filter() {
        // (x|y)z has no >=3 literal anywhere -> no constraints -> full scan.
        let a = RegexAnalysis::analyze(r"(x|y)z").unwrap();
        assert!(!a.is_accelerated);
        assert!(a.constraints.is_empty());
    }

    #[test]
    fn test_alternation_with_one_unusable_branch_drops_group() {
        // "foobar|[0-9]+" — second branch has no required literal, so the whole
        // alternation cannot soundly filter and must be dropped (full scan).
        let a = RegexAnalysis::analyze(r"foobar|[0-9]+").unwrap();
        assert!(
            a.constraints.is_empty(),
            "alternation with a literal-less branch must not constrain; got {:?}",
            a.constraints
        );
    }

    #[test]
    fn test_concat_with_required_alternation() {
        // "(get|set)Value" -> two constraints: union{get... no, "get"/"set" are 3
        // chars} and required "Value".
        let a = RegexAnalysis::analyze(r"(get|set)Value").unwrap();
        assert!(a.is_accelerated);
        // Required "Value"
        assert!(a
            .constraints
            .iter()
            .any(|g| g.len() == 1 && g[0] == "Value"));
        // Union {get,set}
        assert!(a
            .constraints
            .iter()
            .any(|g| g.contains(&"get".to_string()) && g.contains(&"set".to_string())));
    }

    #[test]
    fn test_regex_matches() {
        let analysis = RegexAnalysis::analyze(r"fn\s+\w+").unwrap();
        assert!(analysis.regex.is_match("fn main"));
        assert!(analysis.regex.is_match("fn   test"));
        assert!(!analysis.regex.is_match("function main"));
    }
}
