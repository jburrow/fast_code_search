//! File path search: the files whose *path* matches the query, reported
//! apart from content hits (the web UI's "Files" group, `fcs`, MCP).
//!
//! Content search never looks at paths (only a file-name *stem* is indexed,
//! for the synthetic filename hit), so typing `src/web/api.rs` found nothing.
//! This is a separate, cheap pass over the indexed paths: no file is opened
//! and the content results are untouched, so they still match ripgrep.
//!
//! Matching, for a plain-text query: every whitespace-separated word of the
//! text must appear in the path as shown in results (case-insensitive unless `case:yes`, `word:yes`
//! honoured, `\` read as `/`). A term holding a `/` may also match the file's
//! absolute path, so a pasted `/home/me/repo/src/main.rs` finds the file,
//! as long as the match reaches into the shown part. `file:` / `lang:` and
//! include/exclude globs filter
//! as they do for content. A regex is matched against the shown path.

use super::*;

/// One file whose path matched.
#[derive(Debug, Clone, PartialEq)]
pub struct PathMatch {
    pub file_id: u32,
    /// The path as results show it.
    pub file_path: String,
    pub score: f64,
    /// Byte ranges of `file_path` that matched (one per term).
    pub ranges: Vec<(usize, usize)>,
}

/// The best path matches and how many paths matched in all.
#[derive(Debug, Clone, Default)]
pub struct PathSearch {
    pub matches: Vec<PathMatch>,
    pub total: usize,
    /// True when the deadline stopped the scan, so `total` is a lower bound.
    pub truncated: bool,
}

/// What a path is matched against.
pub enum PathQuery<'a> {
    Terms(&'a ParsedQuery),
    /// A regex pattern (already carrying any case flag).
    Regex(&'a str),
}

/// Scores of one term's hit. Higher is better; a file name hit beats a
/// directory hit, and an exact name beats a partial one.
mod score {
    pub const EXACT_NAME: f64 = 10.0;
    pub const EXACT_STEM: f64 = 9.0;
    pub const PATH_SUFFIX: f64 = 8.0;
    pub const NAME_PREFIX: f64 = 6.0;
    pub const IN_NAME: f64 = 4.0;
    pub const SEGMENT_START: f64 = 3.0;
    pub const IN_PATH: f64 = 2.0;
}

impl SearchEngine {
    /// Files whose path matches `query`, best first, at most `limit` of
    /// them. Symbol and reference searches have no path matches.
    pub fn search_paths(
        &self,
        query: PathQuery<'_>,
        include_patterns: &str,
        exclude_patterns: &str,
        limit: usize,
        deadline: Option<std::time::Instant>,
        scope: Option<&super::GraphScope>,
    ) -> Result<PathSearch> {
        let empty = ParsedQuery::default();
        let (parsed, regex) = match query {
            PathQuery::Terms(p) => (p, None),
            PathQuery::Regex(pattern) => (&empty, Some(self.cached_regex(pattern)?)),
        };
        let path_filter = merged_path_filter(parsed, include_patterns, exclude_patterns)?;
        // Unlike content search, the text is split into words: `web api`
        // finds `src/web/api.rs`.
        let terms: Vec<PathTerm> = parsed.text.split_whitespace().map(PathTerm::new).collect();
        if regex.is_none() && terms.is_empty() {
            return Ok(PathSearch::default());
        }
        let opts = parsed.options;

        let mut out = PathSearch::default();
        for id in 0..self.file_store.len() as u32 {
            if id % 1024 == 0 && deadline.is_some_and(|d| std::time::Instant::now() >= d) {
                out.truncated = true;
                break;
            }
            if scope.is_some_and(|s| !s.allows(id)) {
                continue;
            }
            let Some(file) = self.file_store.get(id) else {
                continue;
            };
            // The root-qualified path (what globs match) and the part shown.
            let (qualified, start) = match self.file_metadata.get(id as usize) {
                Some(m) if !m.display_path.is_empty() => (
                    std::borrow::Cow::Borrowed(m.display_path.as_str()),
                    m.display_start as usize,
                ),
                _ => {
                    let (q, s) = self.qualified_display_path(&file.path);
                    (std::borrow::Cow::Owned(q), s)
                }
            };
            let shown = &qualified[start.min(qualified.len())..];
            if !path_filter.is_empty() && !path_filter.matches(&qualified) {
                continue;
            }
            let hit = match &regex {
                Some(analysis) => analysis.regex.find(shown).map(|m| {
                    let s = if m.start() >= name_start(shown) {
                        score::IN_NAME
                    } else {
                        score::IN_PATH
                    };
                    (s, vec![(m.start(), m.end())])
                }),
                None => match_terms(&terms, shown, &file.path, opts),
            };
            let Some((score, ranges)) = hit else {
                continue;
            };
            out.total += 1;
            out.matches.push(PathMatch {
                file_id: id,
                file_path: shown.to_string(),
                score,
                ranges,
            });
        }
        // Best score first, then the shorter (shallower) path, then by name.
        let cmp = |a: &PathMatch, b: &PathMatch| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.file_path.len().cmp(&b.file_path.len()))
                .then_with(|| a.file_path.cmp(&b.file_path))
        };
        if out.matches.len() > limit {
            out.matches.select_nth_unstable_by(limit, cmp);
            out.matches.truncate(limit);
        }
        out.matches.sort_by(cmp);
        Ok(out)
    }
}

/// A query term prepared for path matching.
struct PathTerm {
    text: String,
    lower: String,
    /// Holds a `/`, so it may be a pasted (absolute or partial) path.
    has_sep: bool,
}

impl PathTerm {
    fn new(term: &str) -> Self {
        let text = term.replace('\\', "/");
        let lower = fold_str(&text).into_owned();
        let has_sep = text.contains('/');
        Self {
            text,
            lower,
            has_sep,
        }
    }

    /// First hit of the term in `hay` (byte range) under `opts`.
    fn find(&self, hay: &str, opts: SearchOptions) -> Option<(usize, usize)> {
        let mut from = 0;
        while from <= hay.len() {
            let rest = &hay[from..];
            let (s, e) = if opts.case_sensitive {
                rest.find(&self.text).map(|s| (s, s + self.text.len()))?
            } else {
                find_match_position_case_insensitive(rest, &self.lower)?
            };
            let (s, e) = (from + s, from + e);
            if !opts.whole_word || is_whole_word(hay, s, e) {
                return Some((s, e));
            }
            // Retry past this hit's first character.
            from = s + hay[s..].chars().next().map_or(1, char::len_utf8);
        }
        None
    }
}

/// Byte offset where the file name starts in a `/`-separated path.
fn name_start(path: &str) -> usize {
    path.rfind('/').map_or(0, |i| i + 1)
}

/// Score and ranges when every term matches `shown` (or, for a term with a
/// `/`, the absolute path, reaching into the shown part).
fn match_terms(
    terms: &[PathTerm],
    shown: &str,
    abs: &Path,
    opts: SearchOptions,
) -> Option<(f64, Vec<(usize, usize)>)> {
    let name_at = name_start(shown);
    let name = &shown[name_at..];
    let stem_len = name.rfind('.').filter(|&i| i > 0).unwrap_or(name.len());
    let mut abs_norm: Option<String> = None;
    let mut total = 0.0;
    let mut ranges = Vec::with_capacity(terms.len());
    for term in terms {
        // A hit inside the file name ranks above one in a directory, so
        // look there first.
        if let Some((s, e)) = term.find(name, opts) {
            let s_score = if s == 0 && e == name.len() {
                score::EXACT_NAME
            } else if s == 0 && e == stem_len {
                score::EXACT_STEM
            } else if s == 0 {
                score::NAME_PREFIX
            } else {
                score::IN_NAME
            };
            total += s_score;
            ranges.push((name_at + s, name_at + e));
            continue;
        }
        if let Some((s, e)) = term.find(shown, opts) {
            let b = shown.as_bytes();
            let at_segment = s == 0 || b[s - 1] == b'/' || b[s] == b'/';
            total += if term.has_sep && at_segment && e == shown.len() {
                score::PATH_SUFFIX
            } else if at_segment {
                score::SEGMENT_START
            } else {
                score::IN_PATH
            };
            ranges.push((s, e));
            continue;
        }
        if !term.has_sep {
            return None;
        }
        // A pasted absolute (or root-qualified) path: the shown path is a
        // suffix of the absolute one, so map the hit back onto it.
        let abs = abs_norm.get_or_insert_with(|| abs.to_string_lossy().replace('\\', "/"));
        if !abs.ends_with(shown) {
            return None;
        }
        let offset = abs.len() - shown.len();
        let (s, e) = term.find(abs, opts)?;
        if e <= offset {
            return None;
        }
        total += if e == abs.len() {
            score::PATH_SUFFIX
        } else {
            score::IN_PATH
        };
        ranges.push((s.max(offset) - offset, e - offset));
    }
    Some((total / terms.len() as f64, ranges))
}
