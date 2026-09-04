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

        let constraints = match regex_syntax::parse(pattern) {
            Ok(hir) => extract_constraints(&hir),
            Err(_) => vec![],
        };

        // Accelerated only when we have at least one SOUND constraint (a literal
        // that must appear, or an alternation where every branch contributes one).
        let is_accelerated = !constraints.is_empty();

        Ok(Self {
            regex,
            constraints,
            is_accelerated,
        })
    }
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

/// Text that MUST appear, contiguously, wherever `hir` matches — or `None`.
///
/// Covers plain literals, single-character case-insensitive classes (what
/// `(?i)needle` compiles to: `[Nn][Ee]…`, returned lowercase, which is what
/// the lowercased trigram index needs), and the first mandatory copy of a
/// repetition (`c+` in `abc+`), so `(?i)needle` and `abc+` are accelerated
/// instead of falling back to a full scan.
fn mandatory_text(hir: &Hir) -> Option<String> {
    match hir.kind() {
        HirKind::Literal(lit) => literal_to_string(lit),
        HirKind::Class(class) => single_char_class_lower(class).map(|c| c.to_string()),
        HirKind::Capture(c) => mandatory_text(&c.sub),
        HirKind::Repetition(rep) if rep.min >= 1 => mandatory_text(&rep.sub),
        HirKind::Concat(subs) => {
            // Only when EVERY child has mandatory text is the concatenation
            // itself a contiguous mandatory string.
            let mut s = String::new();
            for sub in subs.iter() {
                s.push_str(&mandatory_text(sub)?);
            }
            Some(s)
        }
        _ => None,
    }
}

/// If `class` matches exactly the case variants of one character, that
/// character in lowercase.
fn single_char_class_lower(class: &regex_syntax::hir::Class) -> Option<char> {
    use regex_syntax::hir::Class;
    let mut lower: Option<char> = None;
    let mut consider = |c: char| -> bool {
        let mut it = c.to_lowercase();
        let Some(l) = it.next() else {
            return false;
        };
        if it.next().is_some() {
            return false; // multi-char lowercase mapping: not a simple case pair
        }
        match lower {
            None => {
                lower = Some(l);
                true
            }
            Some(existing) => existing == l,
        }
    };
    match class {
        Class::Unicode(u) => {
            for r in u.ranges() {
                if r.start() != r.end() || !consider(r.start()) {
                    return None;
                }
            }
        }
        Class::Bytes(b) => {
            for r in b.ranges() {
                if r.start() != r.end() || !consider(r.start() as char) {
                    return None;
                }
            }
        }
    }
    lower
}

fn collect_constraints(hir: &Hir, out: &mut Vec<Vec<String>>) {
    match hir.kind() {
        HirKind::Concat(subs) => {
            // Merge consecutive mandatory-text children into one required run.
            let mut current = String::new();
            for sub in subs.iter() {
                if let Some(s) = mandatory_text(sub) {
                    current.push_str(&s);
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
        HirKind::Literal(_) | HirKind::Class(_) => {
            if let Some(s) = mandatory_text(hir) {
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
        HirKind::Literal(_) | HirKind::Class(_) => mandatory_text(hir).filter(|s| s.len() >= 3),
        HirKind::Concat(subs) => {
            // Longest run of consecutive mandatory-text children.
            let mut best: Option<String> = None;
            let mut current = String::new();
            for sub in subs.iter() {
                if let Some(s) = mandatory_text(sub) {
                    current.push_str(&s);
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

    fn constraints(pattern: &str) -> Vec<Vec<String>> {
        RegexAnalysis::analyze(pattern).unwrap().constraints
    }

    #[test]
    fn test_simple_literal() {
        assert_eq!(constraints("hello"), vec![vec!["hello".to_string()]]);
        assert!(RegexAnalysis::analyze("hello").unwrap().is_accelerated);
    }

    #[test]
    fn test_literal_with_special_chars() {
        // `fn\s+main` -> "main" is required (and "fn" is too short)
        assert_eq!(constraints(r"fn\s+main"), vec![vec!["main".to_string()]]);
    }

    #[test]
    fn test_regex_with_alternation() {
        let c = constraints("hello|world");
        assert_eq!(c, vec![vec!["hello".to_string(), "world".to_string()]]);
    }

    #[test]
    fn test_no_literals() {
        let analysis = RegexAnalysis::analyze(r"\d+\s*\w+").unwrap();
        assert!(analysis.constraints.is_empty());
        assert!(!analysis.is_accelerated);
    }

    #[test]
    fn test_short_literal() {
        // "ab" is too short for a trigram
        assert!(constraints("ab").is_empty());
    }

    #[test]
    fn test_complex_pattern() {
        // Required runs: "impl", "Display", "for" (with their spaces merged)
        let c = constraints(r"impl\s+Display\s+for\s+\w+");
        let flat: Vec<String> = c.into_iter().flatten().collect();
        assert!(flat.iter().any(|l| l.contains("impl")), "{flat:?}");
        assert!(flat.iter().any(|l| l.contains("Display")), "{flat:?}");
    }

    #[test]
    fn test_escaped_chars() {
        let c = constraints(r"\.unwrap\(\)");
        let flat: Vec<String> = c.into_iter().flatten().collect();
        assert!(flat.iter().any(|l| l.contains(".unwrap()")), "{flat:?}");
    }

    /// Roadmap 3.3: `(?i)literal` compiles to per-character case classes;
    /// they are lowered to a lowercase literal so the search is accelerated.
    #[test]
    fn test_case_insensitive_literal_is_accelerated() {
        assert_eq!(constraints("(?i)needle"), vec![vec!["needle".to_string()]]);
        assert_eq!(
            constraints("(?i)Hello World"),
            vec![vec!["hello world".to_string()]]
        );
        assert_eq!(constraints("[Nn]eedle"), vec![vec!["needle".to_string()]]);
        // A real class is not a literal.
        assert!(constraints("[a-z]eedle")
            .iter()
            .flatten()
            .all(|l| l == "eedle"));
    }

    /// Roadmap 3.3: the first mandatory copy of a repetition is contiguous
    /// with its neighbours, so `abc+` requires "abc"; `abc*` only "ab".
    #[test]
    fn test_repetition_prefix_is_required() {
        assert_eq!(constraints("abc+"), vec![vec!["abc".to_string()]]);
        assert_eq!(constraints("abc{2,}"), vec![vec!["abc".to_string()]]);
        assert!(constraints("abc*").is_empty(), "only 'ab' is mandatory");
        assert_eq!(constraints("(abc)+d"), vec![vec!["abcd".to_string()]]);
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
