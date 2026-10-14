//! Search query syntax for plain-text searches: the text to find, plus
//! `file:`, `lang:`, `deps:`, `rdeps:`, `case:` and `word:` operators in the
//! style of Zoekt / Sourcegraph.
//!
//! Filters of different kinds narrow each other (`file:src/ lang:rs` is the
//! Rust files under `src/`); repeating one kind widens it (`lang:rs lang:py`
//! is either language).
//!
//! # The text is a literal
//!
//! Like `rg -F`, everything that is not an operator is searched for exactly
//! as typed: `fn main` is the phrase `fn main` (one space), `"success":`
//! keeps its quotes and `-Wall` keeps its `-`. The text is trimmed at both
//! ends; spacing inside it is kept. When operators stand between pieces of
//! text (`fn file:src/ main`), the pieces are joined by one space.
//!
//! An operator is a whole whitespace-separated token with an argument
//! (`file:` alone is text). To search for operator-looking text such as
//! `file:x`, use a regex. Regex queries are passed through untouched.

/// Matching options that change how the text is verified in file content.
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
    /// The literal text to find (empty when the query is only operators).
    pub text: String,
    /// Path globs a file must match, one of them (from `file:`).
    pub include_globs: Vec<String>,
    /// Language globs a file must also match, one of them (from `lang:`).
    pub lang_globs: Vec<String>,
    /// Path globs that exclude a file (from `-file:` / `-lang:`).
    pub exclude_globs: Vec<String>,
    /// Import-graph filters (from `deps:` / `rdeps:` and their negations).
    pub graph: Vec<GraphFilter>,
    /// Matching options (from `case:` / `word:`).
    pub options: SearchOptions,
}

/// Which way a graph filter walks the imports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphDirection {
    /// `deps:` — the file and what it imports.
    Deps,
    /// `rdeps:` — the file and what imports it.
    Rdeps,
}

impl GraphDirection {
    /// The operator, without the colon.
    pub fn name(self) -> &'static str {
        match self {
            Self::Deps => "deps",
            Self::Rdeps => "rdeps",
        }
    }
}

/// A `deps:` / `rdeps:` filter as typed. The argument is `PATH`,
/// `PATH:N` (hops) or `PATH:*`; the engine resolves it against the index,
/// since only it knows whether a trailing `:N` is part of a file name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GraphFilter {
    pub direction: GraphDirection,
    /// `-deps:` / `-rdeps:`: leave those files out.
    pub negate: bool,
    pub arg: String,
}

impl GraphFilter {
    /// The filter as it would be typed.
    pub fn display(&self) -> String {
        let dash = if self.negate { "-" } else { "" };
        format!("{dash}{}:{}", self.direction.name(), self.arg)
    }
}

impl ParsedQuery {
    /// The text to find.
    pub fn primary(&self) -> &str {
        &self.text
    }
}

/// Parse `raw` into the literal text, filters and options.
///
/// - `file:PAT`   include paths matching PAT (a bare word means `*PAT*` in
///   any directory; a pattern with `/` or a glob char is used as written;
///   `file:"my dir"` for a pattern with spaces)
/// - `-file:PAT`  exclude paths matching PAT
/// - `lang:NAME`  include files of that language (`rs`, `rust`, `py`, ...)
/// - `-lang:NAME` exclude them
/// - `deps:PATH[:N|:*]`  only PATH and the files it imports (N hops, 1 by
///   default, `*` for any distance); `rdeps:` the files importing it;
///   `-deps:` / `-rdeps:` leave them out
/// - `case:yes`   case-sensitive (`case:no` forces insensitive)
/// - `word:yes`   whole-word matching
///
/// Any other token is text, quotes and a leading `-` included. An operator
/// without an argument, or `lang:` with a name it does not know, is text.
pub fn parse(raw: &str) -> ParsedQuery {
    let mut q = ParsedQuery::default();
    // Byte ranges of `raw` holding text, one per run of text tokens.
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut run_open = false;
    for token in tokenize(raw) {
        if apply_operator(&mut q, &token) {
            run_open = false;
            continue;
        }
        match runs.last_mut() {
            Some(run) if run_open => run.1 = token.end,
            _ => runs.push((token.start, token.end)),
        }
        run_open = true;
    }
    q.text = runs
        .iter()
        .map(|&(s, e)| &raw[s..e])
        .collect::<Vec<_>>()
        .join(" ");
    q
}

/// If `token` is an operator, record it in `q` and return true.
fn apply_operator(q: &mut ParsedQuery, token: &Token) -> bool {
    let Some(arg) = token.arg.as_deref() else {
        return false;
    };
    match token.op {
        "file:" => q.include_globs.push(file_glob(arg)),
        "-file:" => q.exclude_globs.push(file_glob(arg)),
        "lang:" | "-lang:" => {
            let Some(globs) = lang_globs(arg) else {
                return false;
            };
            if token.op == "lang:" {
                q.lang_globs.extend(globs);
            } else {
                q.exclude_globs.extend(globs);
            }
        }
        "deps:" | "-deps:" | "rdeps:" | "-rdeps:" => q.graph.push(GraphFilter {
            direction: if token.op.ends_with("rdeps:") {
                GraphDirection::Rdeps
            } else {
                GraphDirection::Deps
            },
            negate: token.op.starts_with('-'),
            arg: arg.to_string(),
        }),
        "case:" => q.options.case_sensitive = is_yes(arg),
        "word:" => q.options.whole_word = is_yes(arg),
        _ => return false,
    }
    true
}

fn is_yes(v: &str) -> bool {
    matches!(
        v.to_ascii_lowercase().as_str(),
        "yes" | "y" | "true" | "1" | "on"
    )
}

/// Operator prefixes, longest first where one is a suffix of another.
const OPERATORS: &[&str] = &[
    "-file:", "file:", "-lang:", "lang:", "-rdeps:", "rdeps:", "-deps:", "deps:", "case:", "word:",
];

/// One whitespace-separated token: `raw[start..end]`. When it starts with
/// an operator prefix, `op` is that prefix and `arg` its (unquoted)
/// argument, or `None` when there is none.
struct Token {
    start: usize,
    end: usize,
    op: &'static str,
    arg: Option<String>,
}

/// Split on whitespace. An operator's argument may be quoted to hold
/// spaces (`file:"my dir"`): the quote must close before whitespace or the
/// end of the query, or it is an ordinary character. Quotes anywhere else
/// are text.
fn tokenize(raw: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        let rest = &raw[i..];
        let Some(c) = rest.chars().next() else { break };
        if c.is_whitespace() {
            i += c.len_utf8();
            continue;
        }
        let start = i;
        let op = OPERATORS
            .iter()
            .copied()
            .find(|op| rest.starts_with(op))
            .unwrap_or("");
        let after_op = start + op.len();
        // A quoted operator argument: `file:"my dir"`.
        if !op.is_empty() && raw[after_op..].starts_with('"') {
            let body = &raw[after_op + 1..];
            if let Some(close) = body.find('"') {
                let end = after_op + 1 + close + 1;
                let closes = raw[end..].chars().next().is_none_or(char::is_whitespace);
                if closes && close > 0 {
                    out.push(Token {
                        start,
                        end,
                        op,
                        arg: Some(body[..close].to_string()),
                    });
                    i = end;
                    continue;
                }
            }
        }
        let end = raw[start..]
            .find(char::is_whitespace)
            .map_or(raw.len(), |n| start + n);
        let arg = &raw[after_op..end];
        out.push(Token {
            start,
            end,
            op,
            arg: (!op.is_empty() && !arg.is_empty()).then(|| arg.to_string()),
        });
        i = end;
    }
    out
}

/// `file:` argument to a glob understood by `PathFilter`.
///
/// A trailing `/` means "everything under this directory": `src/` becomes
/// `**/src/**` (a glob ending in `/` would only match paths that end in
/// `src/`, i.e. nothing). A pattern that is only slashes selects everything.
///
/// Display paths start with the indexed root's own name
/// (`fast_code_search/src/main.rs`), so a pattern without a `/` must not be
/// matched against that first component: `file:search` would select every
/// file of a root named `fast_code_search`, and `-file:test` would drop
/// every file of a root named `test-harness`. Such patterns get a leading
/// `*/`, which consumes the root name (`*` crosses `/` in these globs, so
/// what follows still matches any directory or file name below it).
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
        format!("*/**/{pat}")
    } else {
        format!("*/**/*{pat}*")
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
    fn several_words_are_one_phrase() {
        let q = parse("hello world");
        assert_eq!(q.text, "hello world");
        assert!(q.include_globs.is_empty() && q.exclude_globs.is_empty());
        assert_eq!(q.options, SearchOptions::default());
        // Spacing inside the text is kept, the ends are trimmed.
        assert_eq!(parse("  fn  main\t( ").text, "fn  main\t(");
    }

    #[test]
    fn operators_are_parsed() {
        let q = parse("needle lang:rust file:src/ -file:test case:yes word:y -lang:py");
        assert_eq!(q.text, "needle");
        assert_eq!(q.include_globs, vec!["**/src/**"]);
        assert_eq!(q.lang_globs, vec!["**/*.rs"]);
        assert_eq!(
            q.exclude_globs,
            vec!["*/**/*test*", "**/*.py", "**/*.pyi", "**/*.pyw"]
        );
        assert!(q.options.case_sensitive && q.options.whole_word);
        assert_eq!(parse("file:*.toml").include_globs, vec!["*/**/*.toml"]);
        assert_eq!(parse("lang:xyz").lang_globs, vec!["**/*.xyz"]);
        assert_eq!(parse("lang:xyz").text, "");
        // Operators between pieces of text join them with one space.
        assert_eq!(parse("fn  file:src/  main").text, "fn main");
        assert_eq!(parse("case:yes fn main").text, "fn main");
    }

    /// `file:src/` selects everything under a `src` directory.
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

    /// Bug report: quotes and a leading `-` were query syntax (a phrase,
    /// an exclusion), so `"fn main"`, `-Wall` and `-v` never found the text
    /// as typed. Like ripgrep, they are now part of the text.
    #[test]
    fn quotes_and_dashes_are_text() {
        assert_eq!(parse("-Wall").text, "-Wall");
        assert_eq!(parse("-Wall gcc").text, "-Wall gcc");
        assert_eq!(parse("-> fn").text, "-> fn");
        assert_eq!(parse("-1.5").text, "-1.5");
        assert_eq!(parse("\"fn main\"").text, "\"fn main\"");
        assert_eq!(parse("{ \"success\": True").text, "{ \"success\": True");
        assert_eq!(parse("\"key\":\"value\"").text, "\"key\":\"value\"");
        assert_eq!(parse("x = \"\"").text, "x = \"\"");
        assert_eq!(parse("\"file:x\"").text, "\"file:x\"");
        assert!(parse("\"file:x\"").include_globs.is_empty());
        assert_eq!(parse("-\"a b\"").text, "-\"a b\"");
    }

    #[test]
    fn operators_need_an_argument() {
        // An operator without an argument, or with an unknown language, is text.
        let q = parse("file: lang: case: word:");
        assert_eq!(q.text, "file: lang: case: word:");
        assert!(q.include_globs.is_empty() && q.options == SearchOptions::default());
        assert_eq!(parse("-file:").text, "-file:");
        assert_eq!(parse("lang:??? x").text, "lang:??? x");
        // A quoted argument holds spaces; an unclosed quote is literal.
        assert_eq!(
            parse("x file:\"my dir\"").include_globs,
            vec!["*/**/*my dir*"]
        );
        assert_eq!(parse("x file:\"my dir\"").text, "x");
        assert_eq!(parse("file:\"my dir").include_globs, vec!["*/**/*\"my*"]);
        assert_eq!(parse("file:\"my dir").text, "dir");
        // Operator-looking text inside a token is text.
        assert_eq!(parse("url=file:x").text, "url=file:x");
    }

    #[test]
    fn graph_filters_are_parsed() {
        let q = parse("Weight rdeps:src/search/ranking.rs:2 -deps:\"my dir/a.py\" deps:x.ts");
        assert_eq!(q.text, "Weight");
        let f = |direction, negate, arg: &str| GraphFilter {
            direction,
            negate,
            arg: arg.to_string(),
        };
        assert_eq!(
            q.graph,
            vec![
                f(GraphDirection::Rdeps, false, "src/search/ranking.rs:2"),
                f(GraphDirection::Deps, true, "my dir/a.py"),
                f(GraphDirection::Deps, false, "x.ts"),
            ]
        );
        assert_eq!(q.graph[1].display(), "-deps:my dir/a.py");
        assert_eq!(parse("deps:").text, "deps:");
        assert!(parse("url=deps:x").graph.is_empty());
    }

    /// `file:` and `lang:` narrow each other; each widens itself.
    #[test]
    fn file_and_lang_both_apply() {
        use crate::search::path_filter::PathFilter;
        let filter = |q: &str| {
            let p = parse(q);
            PathFilter::from_delimited(&p.include_globs.join(";"), &p.exclude_globs.join(";"))
                .unwrap()
                .and_include(&p.lang_globs)
                .unwrap()
        };
        let src_rs = filter("x file:src/ lang:rs");
        assert!(src_rs.matches("/proj/src/main.rs"));
        assert!(!src_rs.matches("/proj/src/app.py"));
        assert!(!src_rs.matches("/proj/tests/main.rs"));
        let rs_or_py = filter("x lang:rs lang:py");
        assert!(rs_or_py.matches("/proj/a.rs") && rs_or_py.matches("/proj/a.py"));
        assert!(!rs_or_py.matches("/proj/a.go"));
    }

    #[test]
    fn non_ascii_text_is_kept() {
        assert_eq!(parse("café  ΟΣ file:x").text, "café  ΟΣ");
    }
}
