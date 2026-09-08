//! Split out of the search engine module; see `engine/mod.rs`.

use super::*;

/// Case-insensitive substring search without heap allocation.
/// Both `haystack` and `needle` are compared using ASCII case-insensitive matching.
/// `needle` should already be lowercase for optimal performance.
///
/// Note: ASCII-only case folding. Non-ASCII characters (e.g., accented letters,
/// CJK) are compared byte-for-byte without case folding. This is acceptable for
/// code identifiers but won't handle natural language in comments.
#[inline]
pub(super) fn contains_case_insensitive(haystack: &str, needle_lower: &str) -> bool {
    if needle_lower.is_empty() {
        return true;
    }
    // Non-ASCII needle: use Unicode-aware folding so candidates surfaced by the
    // (Unicode-lowercased) trigram index are not silently dropped here. The
    // trigram index lowercases content with `to_lowercase()`, so verification
    // must fold the same way for non-ASCII text (e.g. `über` vs `ÜBER`).
    if !needle_lower.is_ascii() {
        return unicode_ci_find(haystack, needle_lower).is_some();
    }
    let needle_len = needle_lower.len();
    if haystack.len() < needle_len {
        return false;
    }

    let needle_bytes = needle_lower.as_bytes();
    let haystack_bytes = haystack.as_bytes();
    let first_needle = needle_bytes[0];
    // Also match uppercase variant of first byte for faster skip
    let first_needle_upper = if first_needle.is_ascii_lowercase() {
        first_needle - 32
    } else {
        first_needle
    };

    let mut i = 0;
    let max_start = haystack_bytes.len() - needle_len;

    while i <= max_start {
        let h = haystack_bytes[i];
        // Quick first-byte check (handles both cases)
        if h == first_needle || h == first_needle_upper {
            // Check rest of needle
            let mut matched = true;
            for j in 1..needle_len {
                let h = haystack_bytes[i + j];
                let h_lower = if h.is_ascii_uppercase() { h + 32 } else { h };
                if h_lower != needle_bytes[j] {
                    matched = false;
                    break;
                }
            }
            if matched {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// Fast byte substring search using memchr's memmem for SIMD acceleration
#[inline]
pub(super) fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.len() < needle.len() {
        return false;
    }
    memmem::find(haystack, needle).is_some()
}

/// Maximum length of content to return per match (in bytes)
const MAX_CONTENT_LENGTH: usize = 500;

/// Context to show on each side of the match
const MATCH_CONTEXT_CHARS: usize = 200;

/// Result of truncating content around a match
pub(super) struct TruncatedContent {
    pub(super) content: String,
    pub(super) match_start: usize,
    pub(super) match_end: usize,
    pub(super) was_truncated: bool,
}

/// Truncates a line around the match position, preserving context on both sides.
/// Returns the truncated content with ellipsis indicators if truncated.
#[inline]
pub(super) fn truncate_around_match(
    line: &str,
    match_start: usize,
    match_end: usize,
) -> TruncatedContent {
    // If line is short enough, return as-is
    if line.len() <= MAX_CONTENT_LENGTH {
        return TruncatedContent {
            content: line.to_string(),
            match_start,
            match_end,
            was_truncated: false,
        };
    }

    // Calculate window around the match
    let window_start = match_start.saturating_sub(MATCH_CONTEXT_CHARS);
    let window_end = (match_end + MATCH_CONTEXT_CHARS).min(line.len());

    // Find safe UTF-8 boundaries
    let safe_start = find_char_boundary_floor(line, window_start);
    let safe_end = find_char_boundary_ceil(line, window_end);

    // Build truncated string with ellipsis indicators
    // "…" (U+2026) is 3 bytes in UTF-8; pre-allocate enough space for both ellipses.
    const ELLIPSIS_BYTE_LEN: usize = '…'.len_utf8(); // 3
    let prefix_truncated = safe_start > 0;
    let suffix_truncated = safe_end < line.len();
    let extra_bytes = if prefix_truncated {
        ELLIPSIS_BYTE_LEN
    } else {
        0
    } + if suffix_truncated {
        ELLIPSIS_BYTE_LEN
    } else {
        0
    };
    let mut result = String::with_capacity(safe_end - safe_start + extra_bytes);

    if prefix_truncated {
        result.push('…');
    }
    result.push_str(&line[safe_start..safe_end]);
    if suffix_truncated {
        result.push('…');
    }

    // Adjust match positions relative to the new string.
    // "…" is 3 bytes in UTF-8, so the prefix shifts byte offsets by ELLIPSIS_BYTE_LEN.
    let offset = safe_start;
    let new_match_start = if prefix_truncated {
        match_start - offset + ELLIPSIS_BYTE_LEN
    } else {
        match_start - offset
    };
    let new_match_end = if prefix_truncated {
        match_end - offset + ELLIPSIS_BYTE_LEN
    } else {
        match_end - offset
    };

    TruncatedContent {
        content: result,
        match_start: new_match_start,
        match_end: new_match_end,
        was_truncated: true,
    }
}

/// Find the largest valid char boundary <= pos
#[inline]
pub(super) fn find_char_boundary_floor(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    let mut p = pos;
    while p > 0 && !s.is_char_boundary(p) {
        p -= 1;
    }
    p
}

/// Find the smallest valid char boundary >= pos
#[inline]
pub(super) fn find_char_boundary_ceil(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    let mut p = pos;
    while p < s.len() && !s.is_char_boundary(p) {
        p += 1;
    }
    p
}

/// Unicode-aware case-insensitive substring search.
///
/// `needle_lower` must already be Unicode-lowercased (as the query is). Folds
/// each haystack char via `char::to_lowercase()` and matches against the needle,
/// returning byte offsets into the ORIGINAL `haystack`. A match that would split
/// a haystack char whose case-fold expands to multiple chars (e.g. `ß` → `ss`) is
/// rejected so the returned offsets always fall on char boundaries. Only used on
/// the rare non-ASCII path, so its O(n·m) cost is acceptable.
pub(super) fn unicode_ci_find(haystack: &str, needle_lower: &str) -> Option<(usize, usize)> {
    let needle: Vec<char> = needle_lower.chars().collect();
    if needle.is_empty() {
        return Some((0, 0));
    }
    for (start, _) in haystack.char_indices() {
        let mut ni = 0usize;
        let mut byte_end = start;
        let mut ok = true;
        for c in haystack[start..].chars() {
            if ni == needle.len() {
                break;
            }
            for lc in c.to_lowercase() {
                if ni == needle.len() {
                    // c's fold extends past the needle end — would split a char.
                    ok = false;
                    break;
                }
                if lc != needle[ni] {
                    ok = false;
                    break;
                }
                ni += 1;
            }
            if !ok {
                break;
            }
            byte_end += c.len_utf8();
            if ni == needle.len() {
                break;
            }
        }
        if ok && ni == needle.len() {
            return Some((start, byte_end));
        }
    }
    None
}

/// Find match position using case-insensitive search
#[inline]
pub(super) fn find_match_position_case_insensitive(
    haystack: &str,
    needle_lower: &str,
) -> Option<(usize, usize)> {
    if needle_lower.is_empty() {
        return Some((0, 0));
    }
    // Non-ASCII needle: Unicode-aware match returning original-string byte offsets
    // (see contains_case_insensitive for the rationale).
    if !needle_lower.is_ascii() {
        return unicode_ci_find(haystack, needle_lower);
    }
    let needle_len = needle_lower.len();
    if haystack.len() < needle_len {
        return None;
    }

    let needle_bytes = needle_lower.as_bytes();
    let haystack_bytes = haystack.as_bytes();
    let first_needle = needle_bytes[0];
    let first_needle_upper = if first_needle.is_ascii_lowercase() {
        first_needle - 32
    } else {
        first_needle
    };

    let mut i = 0;
    let max_start = haystack_bytes.len() - needle_len;

    while i <= max_start {
        let h = haystack_bytes[i];
        if h == first_needle || h == first_needle_upper {
            let mut matched = true;
            for j in 1..needle_len {
                let h = haystack_bytes[i + j];
                let h_lower = if h.is_ascii_uppercase() { h + 32 } else { h };
                if h_lower != needle_bytes[j] {
                    matched = false;
                    break;
                }
            }
            if matched {
                return Some((i, i + needle_len));
            }
        }
        i += 1;
    }
    None
}

/// Path filter from explicit include/exclude strings plus the query's globs.
pub(super) fn merged_path_filter(
    parsed: &ParsedQuery,
    include_patterns: &str,
    exclude_patterns: &str,
) -> Result<PathFilter> {
    let mut include: Vec<&str> = include_patterns
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let mut exclude: Vec<&str> = exclude_patterns
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    include.extend(parsed.include_globs.iter().map(String::as_str));
    exclude.extend(parsed.exclude_globs.iter().map(String::as_str));
    PathFilter::from_delimited(&include.join(";"), &exclude.join(";"))
}

/// 0-based character column of `byte_offset` within `line`.
#[inline]
pub(super) fn char_column(line: &str, byte_offset: usize) -> usize {
    line[..byte_offset.min(line.len())].chars().count()
}

/// Per-document symbol lookups built lazily on the first match of a scan.
pub(super) struct SymbolLineMaps<'a> {
    pub(super) def_lines: Vec<usize>,
    pub(super) def_set: FxHashSet<usize>,
    pub(super) use_set: bool,
    pub(super) names_by_line: FxHashMap<usize, Vec<&'a str>>,
}

impl<'a> SymbolLineMaps<'a> {
    pub(super) fn build(symbols: &'a [Symbol]) -> Self {
        // The synthetic FileName symbol lives at line 0 and must NOT count as
        // a definition line, otherwise every match on the first line of every
        // file gets the definition boost.
        let def_lines: Vec<usize> = symbols
            .iter()
            .filter(|s| s.is_definition && s.symbol_type != SymbolType::FileName)
            .map(|s| s.line)
            .collect();
        // Linear Vec::contains beats a hash set for the usual handful of
        // definitions; promote to a set only for symbol-dense files.
        let use_set = def_lines.len() > 32;
        let def_set: FxHashSet<usize> = if use_set {
            def_lines.iter().copied().collect()
        } else {
            FxHashSet::default()
        };
        let names_by_line: FxHashMap<usize, Vec<&'a str>> = symbols
            .iter()
            .filter(|s| s.symbol_type != SymbolType::FileName)
            .fold(FxHashMap::default(), |mut map, s| {
                map.entry(s.line).or_default().push(s.name.as_str());
                map
            });
        Self {
            def_lines,
            def_set,
            use_set,
            names_by_line,
        }
    }

    pub(super) fn is_definition_line(&self, line: usize) -> bool {
        if self.use_set {
            self.def_set.contains(&line)
        } else {
            self.def_lines.contains(&line)
        }
    }

    pub(super) fn names_on_line(&self, line: usize) -> Option<&Vec<&'a str>> {
        self.names_by_line.get(&line)
    }
}

/// One line containing an ASCII case-insensitive hit.
pub(super) struct LineHit<'a> {
    /// 0-based line number
    pub(super) line_num: usize,
    /// The line without its terminator (`\n` / `\r\n`), like `str::lines`
    pub(super) line: &'a str,
    /// Match byte range within `line` (first occurrence on the line)
    pub(super) start: usize,
    pub(super) end: usize,
}

/// Find the lines of `content` containing `needle_lower` (ASCII, lowercase),
/// case-insensitively, scanning the whole buffer with `memchr` on the first
/// byte in both cases and only resolving line boundaries at hits. Equivalent
/// to running `find_match_position_case_insensitive` on every `lines()`
/// item, but does no per-line work on the (typically thousands of) lines
/// that do not match.
pub(super) fn ascii_ci_line_hits<'a>(content: &'a str, needle_lower: &str) -> Vec<LineHit<'a>> {
    let mut hits = Vec::new();
    let needle = needle_lower.as_bytes();
    let bytes = content.as_bytes();
    if needle.is_empty() || bytes.len() < needle.len() {
        return hits;
    }
    let first = needle[0];
    let first_upper = first.to_ascii_uppercase();
    let mut line_num = 0usize;
    let mut counted_upto = 0usize; // newlines before this offset are counted in line_num
    let mut skip_until = 0usize; // first occurrence per line only

    for pos in memchr::memchr2_iter(first, first_upper, bytes) {
        if pos < skip_until || pos + needle.len() > bytes.len() {
            continue;
        }
        let window = &bytes[pos..pos + needle.len()];
        if !window
            .iter()
            .zip(needle)
            .skip(1)
            .all(|(&h, &n)| h.to_ascii_lowercase() == n)
        {
            continue;
        }
        let line_start = memchr::memrchr(b'\n', &bytes[..pos]).map_or(0, |i| i + 1);
        let mut line_end = memchr::memchr(b'\n', &bytes[pos..]).map_or(bytes.len(), |i| pos + i);
        skip_until = line_end + 1;
        if line_end > line_start && bytes[line_end - 1] == b'\r' {
            line_end -= 1;
        }
        line_num += memchr::memchr_iter(b'\n', &bytes[counted_upto..line_start]).count();
        counted_upto = line_start;
        // Both bounds sit on newline bytes (or the buffer edges), which are
        // always char boundaries in valid UTF-8.
        let line = &content[line_start..line_end];
        // A needle containing `\n` matches across lines; the hit is reported
        // on the line where it starts, with the range clamped to that line.
        let end = (pos - line_start + needle.len()).min(line.len());
        hits.push(LineHit {
            line_num,
            line,
            start: pos - line_start,
            end,
        });
    }
    hits
}

/// Is the byte at `idx` (or the char starting there) a word character?
#[inline]
pub(super) fn is_word_char_at(bytes: &[u8], idx: usize) -> bool {
    match bytes.get(idx) {
        None => false,
        Some(&b) if b.is_ascii() => b.is_ascii_alphanumeric() || b == b'_',
        // Non-ASCII: letters/digits count as word characters.
        Some(_) => std::str::from_utf8(&bytes[idx..])
            .ok()
            .and_then(|s| s.chars().next())
            .is_some_and(|c| c.is_alphanumeric()),
    }
}

/// Byte index of the char *before* `idx` (`None` at the start).
#[inline]
pub(super) fn prev_char_start(bytes: &[u8], idx: usize) -> Option<usize> {
    if idx == 0 {
        return None;
    }
    let mut i = idx - 1;
    while i > 0 && (bytes[i] & 0xC0) == 0x80 {
        i -= 1;
    }
    Some(i)
}

/// Whole-word test for a match at `[start, end)` within `line`.
#[inline]
pub(super) fn is_whole_word(line: &str, start: usize, end: usize) -> bool {
    let b = line.as_bytes();
    let before = prev_char_start(b, start).is_some_and(|i| is_word_char_at(b, i));
    !before && !is_word_char_at(b, end)
}

/// Lines containing `needle` under `opts` (first qualifying hit per line).
///
/// - case-sensitive: SIMD `memmem` over the whole buffer
/// - case-insensitive ASCII: `memchr2` on the first byte in both cases
/// - case-insensitive non-ASCII: Unicode fold per line
///
/// With `whole_word`, hits that touch a word character on either side are
/// skipped and the scan continues within the line.
pub(super) fn line_hits<'a>(
    content: &'a str,
    needle: &str,
    needle_lower: &str,
    opts: SearchOptions,
) -> Vec<LineHit<'a>> {
    let bytes = content.as_bytes();
    let mut hits = Vec::new();
    if needle.is_empty() {
        return hits;
    }
    let push_hit = |pos: usize,
                    len: usize,
                    line_num: &mut usize,
                    counted: &mut usize,
                    skip_until: &mut usize,
                    hits: &mut Vec<LineHit<'a>>|
     -> bool {
        let line_start = memchr::memrchr(b'\n', &bytes[..pos]).map_or(0, |i| i + 1);
        let mut line_end = memchr::memchr(b'\n', &bytes[pos..]).map_or(bytes.len(), |i| pos + i);
        let raw_line_end = line_end;
        if line_end > line_start && bytes[line_end - 1] == b'\r' {
            line_end -= 1;
        }
        let line = &content[line_start..line_end];
        // A needle containing `\n` matches across lines; the hit is reported
        // on the line where it starts, with the range clamped to that line.
        let (s, e) = (pos - line_start, (pos - line_start + len).min(line.len()));
        if opts.whole_word && !is_whole_word(line, s, e) {
            return false; // keep scanning this line
        }
        *line_num += memchr::memchr_iter(b'\n', &bytes[*counted..line_start]).count();
        *counted = line_start;
        *skip_until = raw_line_end + 1;
        hits.push(LineHit {
            line_num: *line_num,
            line,
            start: s,
            end: e,
        });
        true
    };
    let (mut line_num, mut counted, mut skip_until) = (0usize, 0usize, 0usize);
    if opts.case_sensitive {
        let finder = memmem::Finder::new(needle.as_bytes());
        for pos in finder.find_iter(bytes) {
            if pos < skip_until {
                continue;
            }
            push_hit(
                pos,
                needle.len(),
                &mut line_num,
                &mut counted,
                &mut skip_until,
                &mut hits,
            );
        }
    } else if needle_lower.is_ascii() {
        let nb = needle_lower.as_bytes();
        let (first, first_upper) = (nb[0], nb[0].to_ascii_uppercase());
        for pos in memchr::memchr2_iter(first, first_upper, bytes) {
            if pos < skip_until || pos + nb.len() > bytes.len() {
                continue;
            }
            if !bytes[pos..pos + nb.len()]
                .iter()
                .zip(nb)
                .skip(1)
                .all(|(&h, &n)| h.to_ascii_lowercase() == n)
            {
                continue;
            }
            push_hit(
                pos,
                nb.len(),
                &mut line_num,
                &mut counted,
                &mut skip_until,
                &mut hits,
            );
        }
    } else {
        // Unicode fold: per line, all occurrences until one qualifies.
        let mut offset = 0usize;
        for (n, line) in content.lines().enumerate() {
            let mut from = 0usize;
            while let Some((s, e)) = unicode_ci_find(&line[from..], needle_lower) {
                let (s, e) = (from + s, from + e);
                if !opts.whole_word || is_whole_word(line, s, e) {
                    hits.push(LineHit {
                        line_num: n,
                        line,
                        start: s,
                        end: e,
                    });
                    break;
                }
                from = e;
            }
            offset += line.len() + 1;
        }
        let _ = offset;
    }
    hits
}

/// A set of terms to verify in a document: all `terms` must be present, no
/// `exclude` term may be, and lines matching any term are reported.
#[derive(Debug, Clone)]
pub struct TermSet {
    /// (original, lowercase) per term; the first is the primary (ranking) term
    pub terms: Vec<(String, String)>,
    /// (original, lowercase) per excluded term
    pub exclude: Vec<(String, String)>,
    pub opts: SearchOptions,
    /// The terms joined by one space, when there are several: a line (or
    /// file) containing the query as written outranks one that merely holds
    /// every term somewhere.
    pub phrase: Option<(String, String)>,
}

impl TermSet {
    pub(super) fn single(original: &str, lower: &str) -> Self {
        Self {
            terms: vec![(original.to_string(), lower.to_string())],
            exclude: Vec::new(),
            opts: SearchOptions::default(),
            phrase: None,
        }
    }

    pub(super) fn from_parsed(parsed: &ParsedQuery) -> Self {
        let phrase = (parsed.terms.len() > 1).then(|| {
            let original = parsed.terms.join(" ");
            let lower = original.to_lowercase();
            (original, lower)
        });
        Self {
            phrase,
            terms: parsed
                .terms
                .iter()
                .map(|t| (t.clone(), t.to_lowercase()))
                .collect(),
            exclude: parsed
                .exclude_terms
                .iter()
                .map(|t| (t.clone(), t.to_lowercase()))
                .collect(),
            opts: parsed.options,
        }
    }
}

/// Drop leading item modifiers (`pub`, `pub(crate)`, `export`, `public`,
/// `static`, `async`, `unsafe`, `const`, `default`, `extern "C"`) so the
/// start-of-line test sees the keyword a reader reads first.
pub(super) fn strip_item_modifiers(mut s: &str) -> &str {
    loop {
        let before = s.len();
        for kw in [
            "pub ",
            "export ",
            "public ",
            "private ",
            "protected ",
            "static ",
            "async ",
            "unsafe ",
            "const ",
            "default ",
            "abstract ",
            "final ",
            "override ",
        ] {
            if let Some(rest) = s.strip_prefix(kw) {
                s = rest.trim_start();
            }
        }
        if let Some(rest) = s.strip_prefix("pub(") {
            if let Some(close) = rest.find(')') {
                s = rest[close + 1..].trim_start();
            }
        }
        if let Some(rest) = s.strip_prefix("extern \"") {
            if let Some(close) = rest.find('"') {
                s = rest[close + 1..].trim_start();
            }
        }
        if s.len() == before {
            return s;
        }
    }
}

/// Inline scoring function with pre-computed values (no method call overhead, no redundant lookups)
///
/// `original_query` is the un-lowered query for exact case-sensitive match boosting.
/// `query_lower` is the lowercased query for start-of-line checks.
#[inline]
pub(super) fn calculate_score_inline(
    line: &str,
    original_query: &str,
    query_lower: &str,
    is_symbol_def: bool,
    is_src_lib: bool,
    is_test_path: bool,
    dependency_boost: f64,
) -> f64 {
    let w = &RankingWeights::DEFAULT;
    let mut score = 1.0;

    // Tests, examples, mocks and fixtures mention everything; demote them.
    if is_test_path {
        score *= w.test_path;
    }

    // Boost for exact case-sensitive matches (using the original un-lowered query)
    if line.contains(original_query) {
        score *= w.exact_case;
    }

    // Boost for symbol definitions (pre-computed)
    if is_symbol_def {
        score *= w.symbol_definition;
    }

    // Boost for primary source directories (pre-computed)
    if is_src_lib {
        score *= w.src_lib_dir;
    }

    // Boost for shorter lines (more relevant) — gentle logarithmic curve
    score *= w.line_length_factor(line.len());

    // A public definition is what a reader usually wants first: `pub fn
    // block_on` in runtime.rs over the scheduler's internal `fn block_on`.
    let trimmed = line.trim_start();
    if is_symbol_def
        && (trimmed.starts_with("pub ")
            || trimmed.starts_with("export ")
            || trimmed.starts_with("public "))
    {
        score *= w.public_definition;
    }

    // Boost for the query appearing at the start of the line, ignoring
    // visibility and other item modifiers: `pub fn block_on` starts with
    // `fn block_on` as far as a reader is concerned.
    let head = strip_item_modifiers(trimmed);
    if head.len() >= query_lower.len()
        && head.as_bytes()[..query_lower.len()].eq_ignore_ascii_case(query_lower.as_bytes())
    {
        score *= w.line_start;
    }

    // Apply pre-computed dependency boost
    score * dependency_boost
}

/// Inline regex scoring function with pre-computed values
#[inline]
pub(super) fn calculate_score_regex_inline(
    line: &str,
    regex: &Regex,
    is_symbol_def: bool,
    is_src_lib: bool,
    dependency_boost: f64,
) -> f64 {
    let w = &RankingWeights::DEFAULT;
    let mut score = 1.0;

    // Boost for symbol definitions (pre-computed)
    if is_symbol_def {
        score *= w.symbol_definition;
    }

    // Boost for primary source directories (pre-computed)
    if is_src_lib {
        score *= w.src_lib_dir;
    }

    // Boost for shorter lines (more relevant) — gentle logarithmic curve
    score *= w.line_length_factor(line.len());

    // Boost for matches at the start of the line
    let trimmed = line.trim_start();
    if let Some(m) = regex.find(trimmed) {
        if m.start() == 0 {
            score *= w.line_start;
        }
    }

    // Apply pre-computed dependency boost
    score * dependency_boost
}
