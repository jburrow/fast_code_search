//! Search query syntax: `file:`, `lang:`, `-term`, `case:`, `word:` and
//! quoted phrases, in the style of Zoekt / Sourcegraph.
//!
//! Everything is optional: a query without operators is a single term and
//! behaves exactly as before. Operators only apply to plain-text searches;
//! regex queries are passed through untouched.
//!
//! # Several terms
//!
//! `fn main` is two terms. A file matches only when it contains **every**
//! term (file-level AND); a `"quoted phrase"` is one term matched literally.
//! Within a matching file, every line containing at least one term is a
//! hit, reported once (by the earliest term that matched it). Lines are
//! ranked by how many distinct terms they contain: the line score is
//! multiplied by that count, and within a file the lines holding the most
//! terms are emitted first, so they are never cut off by the per-file match
//! cap or the query's match budget. A single-term query is unaffected.

/// Matching options that change how a term is verified in file content.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SearchOptions {
    /// Compare bytes exactly instead of case-insensitively.
    pub case_sensitive: bool,
    /// Require the match to be delimited by non-word characters.
    pub whole_word: bool,
}

/// A parsed query.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedQuery {
    /// Terms that must all appear in a file. Lines matching any are
    /// returned, those matching the most terms first (see the module docs).
    pub terms: Vec<String>,
    /// Files containing any of these are dropped.
    pub exclude_terms: Vec<String>,
    /// Path globs a file must match (from `file:` / `lang:`).
    pub include_globs: Vec<String>,
    /// Path globs that exclude a file (from `-file:` / `-lang:`).
    pub exclude_globs: Vec<String>,
    /// Matching options (from `case:` / `word:`).
    pub options: SearchOptions,
}

impl ParsedQuery {
    /// The primary term (first), or empty.
    pub fn primary(&self) -> &str {
        self.terms.first().map(String::as_str).unwrap_or("")
    }
}

/// Parse `raw` into terms, filters and options.
///
/// Tokens are whitespace-separated; `"quoted phrases"` are one token.
/// - `file:PAT`   include paths matching PAT (a bare word means `*PAT*` in
///   any directory; a pattern with `/` or a glob char is used as written)
/// - `-file:PAT`  exclude paths matching PAT
/// - `lang:NAME`  include files of that language (`rs`, `rust`, `py`, ...)
/// - `-lang:NAME` exclude them
/// - `case:yes`   case-sensitive (`case:no` forces insensitive)
/// - `word:yes`   whole-word matching
/// - `-term`      files containing `term` are dropped
///
/// Escapes: `-` negates only when followed by a letter, `_` or a quote, so
/// `->`, `-1.5` and a lone `-` are ordinary terms. Quoting makes a token
/// literal (`"file:"` is the term `file:`, `-"a b"` excludes the phrase
/// `a b`), while a quote *after* an operator's colon still belongs to the
/// operator (`file:"my dir"`). An operator without an argument (`file:`,
/// `lang:`, `case:`, `word:`) or with a language it does not know is a
/// plain term rather than a silently empty filter.
pub fn parse(raw: &str) -> ParsedQuery {
    let mut q = ParsedQuery::default();
    for token in tokenize(raw) {
        let (negated, body, quote_at) = match token.text.strip_prefix('-') {
            Some(rest) if token.negates() => {
                (true, rest.to_string(), token.quote_at.map(|i| i - 1))
            }
            _ => (false, token.text.clone(), token.quote_at),
        };
        // `prefix` is an operator only when no quote precedes its colon.
        let operator = |prefix: &str| -> Option<&str> {
            let arg = body.strip_prefix(prefix)?;
            (quote_at.is_none_or(|i| i >= prefix.len()) && !arg.is_empty()).then_some(arg)
        };
        if let Some(pat) = operator("file:") {
            let glob = file_glob(pat);
            if negated {
                q.exclude_globs.push(glob);
            } else {
                q.include_globs.push(glob);
            }
            continue;
        }
        if let Some(name) = operator("lang:") {
            if let Some(globs) = lang_globs(name) {
                if negated {
                    q.exclude_globs.extend(globs);
                } else {
                    q.include_globs.extend(globs);
                }
                continue;
            }
        }
        if let Some(v) = operator("case:") {
            q.options.case_sensitive = is_yes(v);
            continue;
        }
        if let Some(v) = operator("word:") {
            q.options.whole_word = is_yes(v);
            continue;
        }
        if negated {
            q.exclude_terms.push(body);
        } else if !body.is_empty() {
            q.terms.push(body);
        }
    }
    q
}

fn is_yes(v: &str) -> bool {
    matches!(
        v.to_ascii_lowercase().as_str(),
        "yes" | "y" | "true" | "1" | "on"
    )
}

/// One whitespace-separated token with its quotes removed.
struct Token {
    text: String,
    /// Byte offset in `text` where the first `"` stood, if any.
    quote_at: Option<usize>,
}

impl Token {
    /// Does a leading `-` negate this token? Only when a letter, `_` or a
    /// quote follows it: `-Wall` and `-"a b"` negate, `->` and `-1.5` do not.
    fn negates(&self) -> bool {
        let Some(rest) = self.text.strip_prefix('-') else {
            return false;
        };
        match self.quote_at {
            Some(0) => false, // `"-foo"` is the literal term `-foo`
            Some(1) => true,  // `-"a b"`
            _ => rest
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_'),
        }
    }
}

/// Split on whitespace, keeping double-quoted phrases together (quotes removed).
fn tokenize(raw: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote_at = None;
    let mut in_quotes = false;
    for c in raw.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                quote_at.get_or_insert(cur.len());
            }
            c if c.is_whitespace() && !in_quotes => {
                if !cur.is_empty() {
                    out.push(Token {
                        text: std::mem::take(&mut cur),
                        quote_at,
                    });
                }
                quote_at = None;
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(Token {
            text: cur,
            quote_at,
        });
    }
    out
}

/// `file:` argument to a glob understood by `PathFilter`.
///
/// A trailing `/` means "everything under this directory": `src/` becomes
/// `**/src/**` (a glob ending in `/` would only match paths that end in
/// `src/`, i.e. nothing). A pattern that is only slashes selects everything.
fn file_glob(pat: &str) -> String {
    let dir = pat.ends_with('/');
    let pat = pat.trim_end_matches('/');
    if pat.is_empty() {
        return "**".to_string();
    }
    let has_glob = pat.contains(['*', '?', '[']);
    let base = if pat.contains('/') || dir {
        if pat.starts_with("**/") || pat.starts_with('/') {
            pat.to_string()
        } else {
            format!("**/{pat}")
        }
    } else if has_glob {
        format!("**/{pat}")
    } else {
        format!("**/*{pat}*")
    };
    if dir {
        format!("{base}/**")
    } else {
        base
    }
}

/// Language name / extension to the globs that select it.
fn lang_globs(name: &str) -> Option<Vec<String>> {
    let exts: &[&str] = match name.to_ascii_lowercase().as_str() {
        "rs" | "rust" => &["rs"],
        "py" | "python" => &["py", "pyi", "pyw"],
        "js" | "javascript" => &["js", "jsx", "mjs", "cjs"],
        "ts" | "typescript" => &["ts", "tsx", "mts", "cts"],
        "go" | "golang" => &["go"],
        "c" => &["c", "h"],
        "cpp" | "c++" | "cxx" => &["cpp", "cc", "cxx", "hpp", "hxx", "hh", "inl"],
        "java" => &["java"],
        "cs" | "csharp" | "c#" => &["cs"],
        "rb" | "ruby" => &["rb", "rake", "gemspec"],
        "php" => &["php", "phtml"],
        "sh" | "bash" | "shell" => &["sh", "bash", "zsh"],
        "md" | "markdown" => &["md", "markdown"],
        "json" => &["json", "jsonc"],
        "yaml" | "yml" => &["yaml", "yml"],
        "toml" => &["toml"],
        "html" => &["html", "htm"],
        "css" => &["css", "scss"],
        other if !other.is_empty() && other.chars().all(|c| c.is_ascii_alphanumeric()) => {
            return Some(vec![format!("**/*.{other}")])
        }
        _ => return None,
    };
    Some(exts.iter().map(|e| format!("**/*.{e}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_query_is_one_term() {
        let q = parse("hello world");
        assert_eq!(q.terms, vec!["hello", "world"]);
        assert!(q.include_globs.is_empty() && q.exclude_terms.is_empty());
        assert_eq!(q.options, SearchOptions::default());
        assert_eq!(parse("\"hello world\"").terms, vec!["hello world"]);
    }

    #[test]
    fn operators_are_parsed() {
        let q = parse("needle lang:rust file:src/ -file:test case:yes word:y -haystack -lang:py");
        assert_eq!(q.terms, vec!["needle"]);
        assert_eq!(q.exclude_terms, vec!["haystack"]);
        assert_eq!(q.include_globs, vec!["**/*.rs", "**/src/**"]);
        assert_eq!(
            q.exclude_globs,
            vec!["**/*test*", "**/*.py", "**/*.pyi", "**/*.pyw"]
        );
        assert!(q.options.case_sensitive && q.options.whole_word);
        assert_eq!(parse("file:*.toml").include_globs, vec!["**/*.toml"]);
        assert_eq!(parse("lang:xyz").include_globs, vec!["**/*.xyz"]);
        // A negative number is a term, not an exclusion.
        assert_eq!(parse("-1 x").terms, vec!["-1", "x"]);
    }

    /// Review 1.7: `file:src/` selects everything under a `src` directory
    /// (the old glob `**/src/` matched only paths *ending* in `src/`).
    #[test]
    fn trailing_slash_selects_a_directory() {
        use crate::search::path_filter::PathFilter;
        assert_eq!(parse("x file:src/").include_globs, vec!["**/src/**"]);
        assert_eq!(
            parse("x file:src/search/").include_globs,
            vec!["**/src/search/**"]
        );
        assert_eq!(parse("x file:/abs/dir/").include_globs, vec!["/abs/dir/**"]);
        assert_eq!(parse("x file:///").include_globs, vec!["**"]);
        assert_eq!(parse("x -file:target/").exclude_globs, vec!["**/target/**"]);
        let filter = |q: &str| {
            let p = parse(q);
            PathFilter::from_delimited(&p.include_globs.join(";"), &p.exclude_globs.join(";"))
                .unwrap()
        };
        let src = filter("x file:src/");
        assert!(src.matches("/proj/src/main.rs"));
        assert!(src.matches("/proj/src/engine/mod.rs"));
        assert!(!src.matches("/proj/tests/main.rs"));
        assert!(!src.matches("/proj/srcs/main.rs"));
        let not_target = filter("x -file:target/");
        assert!(!not_target.matches("/proj/target/debug/a.rs"));
        assert!(not_target.matches("/proj/src/a.rs"));
    }

    /// Review 1.6: operator-looking text the user means literally is not
    /// swallowed into an empty filter or an accidental exclusion.
    #[test]
    fn escapes_keep_operator_like_text_literal() {
        // `-` negates only before a letter, `_` or a quote.
        assert_eq!(parse("-> fn").terms, vec!["->", "fn"]);
        assert!(parse("-> fn").exclude_terms.is_empty());
        assert_eq!(parse("-1.5 - -_x -Wall").terms, vec!["-1.5", "-"]);
        assert_eq!(parse("-1.5 - -_x -Wall").exclude_terms, vec!["_x", "Wall"]);
        // Quoted tokens are literal; a quote after the colon still belongs to
        // the operator.
        assert_eq!(parse("\"file:\"").terms, vec!["file:"]);
        assert_eq!(parse("\"-foo\"").terms, vec!["-foo"]);
        assert_eq!(parse("x -\"dog here\"").exclude_terms, vec!["dog here"]);
        assert_eq!(parse("file:\"my dir\"").include_globs, vec!["**/*my dir*"]);
        assert_eq!(parse("-\"file:x\"").exclude_terms, vec!["file:x"]);
        // An operator without an argument, or with an unknown language, is a term.
        assert_eq!(
            parse("file: lang: case: word:").terms,
            vec!["file:", "lang:", "case:", "word:"]
        );
        assert_eq!(parse("-file:").exclude_terms, vec!["file:"]);
        assert_eq!(parse("lang:??? x").terms, vec!["lang:???", "x"]);
        let q = parse("file: lang: case: word:");
        assert!(q.include_globs.is_empty() && q.options == SearchOptions::default());
    }
}
