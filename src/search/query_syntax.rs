//! Search query syntax: `file:`, `lang:`, `-term`, `case:`, `word:` and
//! quoted phrases, in the style of Zoekt / Sourcegraph.
//!
//! Everything is optional: a query without operators is a single term and
//! behaves exactly as before. Operators only apply to plain-text searches;
//! regex queries are passed through untouched.

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
    /// Terms that must all appear in a file (lines matching any are returned).
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
pub fn parse(raw: &str) -> ParsedQuery {
    let mut q = ParsedQuery::default();
    for token in tokenize(raw) {
        let (negated, body) = match token.strip_prefix('-') {
            // A lone "-" or a term like "-1" is not an operator.
            Some(rest) if !rest.is_empty() && !rest.chars().all(|c| c.is_ascii_digit()) => {
                (true, rest.to_string())
            }
            _ => (false, token.clone()),
        };
        if let Some(pat) = body.strip_prefix("file:") {
            if pat.is_empty() {
                continue;
            }
            let glob = file_glob(pat);
            if negated {
                q.exclude_globs.push(glob);
            } else {
                q.include_globs.push(glob);
            }
        } else if let Some(name) = body.strip_prefix("lang:") {
            if let Some(globs) = lang_globs(name) {
                if negated {
                    q.exclude_globs.extend(globs);
                } else {
                    q.include_globs.extend(globs);
                }
            }
        } else if let Some(v) = body.strip_prefix("case:") {
            q.options.case_sensitive = is_yes(v);
        } else if let Some(v) = body.strip_prefix("word:") {
            q.options.whole_word = is_yes(v);
        } else if negated {
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

/// Split on whitespace, keeping double-quoted phrases together (quotes removed).
fn tokenize(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for c in raw.chars() {
        match c {
            '"' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// `file:` argument to a glob understood by `PathFilter`.
fn file_glob(pat: &str) -> String {
    let has_glob = pat.contains(['*', '?', '[']);
    if pat.contains('/') {
        if pat.starts_with("**/") || pat.starts_with('/') {
            pat.to_string()
        } else {
            format!("**/{pat}")
        }
    } else if has_glob {
        format!("**/{pat}")
    } else {
        format!("**/*{pat}*")
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
        assert_eq!(q.include_globs, vec!["**/*.rs", "**/src/"]);
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
}
