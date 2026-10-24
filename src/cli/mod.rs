//! The `fcs` command-line client.
//!
//! Design (see `docs/CLI.md`):
//!
//! - **Client first.** A developer machine runs one `fast_code_search_server`
//!   (ideally at login, see `docs/RUN-AT-STARTUP.md`) that holds the index
//!   and watches the tree. `fcs` sends each query to it over the REST API,
//!   so a search costs milliseconds and never re-reads the corpus.
//! - **Offline fallback.** When no server answers and an `index_path` is
//!   known (from the configuration or `--index-path`), the on-disk index is
//!   loaded in-process and searched read-only. It is a few seconds slower
//!   and may be stale; a note says so on stderr.
//! - **Same semantics as the web UI.** The query syntax (a literal text
//!   plus `file:`, `lang:`, `case:`, `word:`), modes (regex, symbols,
//!   references) and result shape are the server's; offline searches go
//!   through the same engine calls the server uses.
//! - **Grep conventions.** `path:line:col:text` when piped (what editors'
//!   quickfix lists read), grouped and coloured on a terminal, `--json` for
//!   tools. Exit code 0 = matches, 1 = none, 2 = error.

pub mod format;
pub mod mcp;

use crate::config::Config;
use crate::web::{ErrorResponse, SearchResponse};
use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use format::{Format, Style};
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Default server when neither `--server`, `FCS_SERVER` nor a config file
/// names one (the server's default `web_address`).
pub const DEFAULT_SERVER: &str = "http://127.0.0.1:8080";

/// Subcommands `fcs` understands; anything else on the command line is a
/// search query (see [`normalize_args`]).
pub const SUBCOMMANDS: &[&str] = &[
    "search", "refs", "symbols", "impact", "status", "mcp", "check", "diagnose", "help",
];

#[derive(Parser, Debug)]
#[command(
    name = "fcs",
    version,
    about = "Search a fast_code_search index from the command line",
    long_about = "Search the code index kept by a running fast_code_search server.\n\n\
                  `fcs QUERY` is `fcs search QUERY`. The query is the web UI's: literal text, \
                  like rg -F (several words are one phrase; quotes and a leading - are text), \
                  plus file:PATTERN, -file:PATTERN, lang:rust, case:yes, word:yes. Put a query \
                  starting with - after --.\n\n\
                  Exit status: 0 = matches found, 1 = no matches, 2 = error.",
    after_help = "Examples:\n  \
                  fcs 'fn main'                 lines holding the phrase\n  \
                  fcs -- -Wall                  a query starting with -\n  \
                  fcs -e 'fn\\s+\\w+\\(' -g '*.rs'  regex, Rust files only\n  \
                  fcs refs SearchEngine         call sites and type mentions\n  \
                  fcs symbols parse_query       definitions only\n  \
                  fcs impact                    files that use what your uncommitted changes touch\n  \
                  fcs -l TODO | xargs $EDITOR   files containing TODO\n  \
                  vim -q <(fcs --format vimgrep TODO)\n  \
                  fcs status                    is the server up, what does it hold\n  \
                  fcs mcp                       MCP over stdio, for coding agents\n  \
                  fcs check                     compare fcs with ripgrep and the import resolvers\n  \
                  fcs diagnose                  what the server's diagnostic captures say"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
    #[command(flatten)]
    pub conn: ConnectionArgs,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Search file contents (the default; `fcs QUERY` works too)
    Search(SearchArgs),
    /// Find references (call sites, type mentions) of an identifier;
    /// `kind:call`, `kind:type`, `kind:value`, `kind:attribute` keep one kind
    Refs(SearchArgs),
    /// Search symbol definitions (functions, types, classes, ...) only
    Symbols(SearchArgs),
    /// The files that use each function, class, method or constant a diff
    /// changes (default: your uncommitted changes, `git diff HEAD`)
    Impact(ImpactArgs),
    /// Show whether the server is reachable and what it has indexed
    Status,
    /// Serve MCP over stdio by relaying to the server's /mcp endpoint (for
    /// agents that launch a command rather than connect to a URL)
    Mcp,
    /// Check fcs against independent tools on your own code: generated
    /// queries against ripgrep, and the import graph against Python's and
    /// TypeScript's own resolvers when they are installed
    Check(CheckArgs),
    /// Summarize the server's diagnostic captures (written when
    /// `[diagnostics] capture = true` and the server is slow or stuck)
    Diagnose(DiagnoseArgs),
}

/// `fcs diagnose`.
#[derive(Args, Debug, Clone)]
pub struct DiagnoseArgs {
    /// The captures folder (default: the configuration's `[diagnostics]`
    /// dir, else `diagnostics` next to its index)
    #[arg(value_name = "DIR")]
    pub dir: Option<PathBuf>,
}

/// `fcs check`.
#[derive(Args, Debug, Clone)]
pub struct CheckArgs {
    /// Folders to check (default: the configuration's indexed paths, else
    /// the current folder). They are indexed afresh, in memory.
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,
    /// Queries made up from the files and compared with `rg -F`
    #[arg(long, default_value_t = 200, value_name = "N")]
    pub queries: usize,
    /// Seed for the queries, to repeat a run (default: a fresh one)
    #[arg(long, value_name = "N")]
    pub seed: Option<u64>,
    /// The ripgrep binary
    #[arg(long, default_value = "rg", value_name = "PATH")]
    pub rg: String,
    /// Skip the search check
    #[arg(long)]
    pub no_search: bool,
    /// Skip the dependency graph check
    #[arg(long)]
    pub no_graph: bool,
    /// Check the running server's index instead (see --server): does it
    /// agree with itself, does indexed content match the disk, and does it
    /// hold the files a fresh walk finds. Prints counts only, so the output
    /// is safe to share; add --show-paths for example file names
    #[arg(long)]
    pub live: bool,
    /// With --live: files whose content to read and check, spread over the
    /// index (0 skips it)
    #[arg(long, default_value_t = 2000, value_name = "N", requires = "live")]
    pub content: usize,
    /// With --live: read and check every indexed file (slow on large trees)
    #[arg(long, requires = "live")]
    pub all_content: bool,
    /// With --live: skip the walk of the disk
    #[arg(long, requires = "live")]
    pub no_discovery: bool,
    /// With --live: show example file names for each kind of problem
    #[arg(long, requires = "live")]
    pub show_paths: bool,
}

/// `fcs impact`.
#[derive(Args, Debug, Clone, Default)]
pub struct ImpactArgs {
    /// Revisions to diff, as `git diff` takes them: none for uncommitted
    /// changes (against HEAD), `REV` for REV to the working tree, `A..B`
    /// or `A B` between commits (the index must hold one of the two sides)
    #[arg(value_name = "REV")]
    pub revs: Vec<String>,
    /// Only staged changes (`git diff --cached`)
    #[arg(long, conflicts_with = "diff")]
    pub staged: bool,
    /// Read the diff from this file (`-` for stdin) instead of running git
    #[arg(long, value_name = "FILE", conflicts_with = "revs")]
    pub diff: Option<PathBuf>,
    /// Files listed per changed symbol (0 = all)
    #[arg(short = 'n', long = "max-files", default_value_t = crate::search::engine::DEFAULT_IMPACT_FILES, value_name = "N")]
    pub max_files: usize,
    /// Use lines listed per file (0 = all)
    #[arg(long = "lines", default_value_t = crate::search::engine::DEFAULT_IMPACT_LINES, value_name = "N")]
    pub max_lines: usize,
    /// Folder the diff's paths are relative to (default: the Git
    /// repository's top folder)
    #[arg(long, value_name = "DIR")]
    pub root: Option<PathBuf>,
}

/// How to reach the index.
#[derive(Args, Debug, Clone, Default)]
pub struct ConnectionArgs {
    /// Server base URL (default: $FCS_SERVER, then the config's web_address,
    /// then http://127.0.0.1:8080)
    #[arg(long, global = true, value_name = "URL")]
    pub server: Option<String>,
    /// Configuration file (default: $FCS_CONFIG, ./fast_code_search.toml,
    /// ~/.config/fast_code_search/config.toml)
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// Search the on-disk index directly instead of asking the server
    #[arg(long, global = true, conflicts_with = "no_offline")]
    pub offline: bool,
    /// Never fall back to the on-disk index when the server is unreachable
    #[arg(long, global = true)]
    pub no_offline: bool,
    /// Index file for offline searches (default: the config's index_path)
    #[arg(long, global = true, value_name = "FILE")]
    pub index_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum ColorChoice {
    #[default]
    Auto,
    Always,
    Never,
}

/// How to print.
#[derive(Args, Debug, Clone, Default)]
pub struct OutputArgs {
    /// Output layout (default: grouped on a terminal, vimgrep when piped)
    #[arg(long, global = true, value_enum)]
    pub format: Option<Format>,
    /// Shorthand for --format json
    #[arg(long, global = true)]
    pub json: bool,
    /// Print every file that matches, once, best first (pages through all hits)
    #[arg(short = 'l', long, global = true)]
    pub files_with_matches: bool,
    /// Print absolute paths (needs the roots from the configuration)
    #[arg(long, global = true)]
    pub absolute: bool,
    /// Colour output: auto (terminal only, honours NO_COLOR), always, never
    #[arg(long, global = true, value_enum, default_value_t = ColorChoice::Auto)]
    pub color: ColorChoice,
    /// No summary or fallback notes on stderr
    #[arg(short, long, global = true)]
    pub quiet: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum Rank {
    #[default]
    Auto,
    Fast,
    Full,
}

/// What to search for.
#[derive(Args, Debug, Clone, Default)]
pub struct SearchArgs {
    /// The query (web UI syntax; several words are one phrase)
    #[arg(value_name = "QUERY", required = true, num_args = 1..)]
    pub query: Vec<String>,
    /// Treat the query as a regular expression
    #[arg(short = 'e', long)]
    pub regex: bool,
    /// Match case exactly
    #[arg(short = 's', long, conflicts_with = "ignore_case")]
    pub case_sensitive: bool,
    /// Ignore case (the default for plain text; with -e, makes a regex ignore it too)
    #[arg(short = 'i', long)]
    pub ignore_case: bool,
    /// Match whole words only
    #[arg(short = 'w', long)]
    pub word: bool,
    /// Maximum hits to print (1 to the server's search.max_page_size, 1000;
    /// default 50). With -l, the most files to list (default: every file)
    #[arg(short = 'n', long, value_name = "N")]
    pub max: Option<usize>,
    /// Skip the first N hits (paging; ordering is deterministic)
    #[arg(long, default_value_t = 0, value_name = "N")]
    pub offset: usize,
    /// Lines of context before and after each hit (0 to search.max_context_lines, 10)
    #[arg(short = 'C', long, default_value_t = 0, value_name = "N")]
    pub context: usize,
    /// Only files matching this glob (repeatable; `src/**/*.rs`, `*.py`, `tests`)
    #[arg(short = 'g', long = "glob", value_name = "GLOB")]
    pub globs: Vec<String>,
    /// Skip files matching this glob (repeatable)
    #[arg(long = "exclude", value_name = "GLOB")]
    pub excludes: Vec<String>,
    /// Deprecated and hidden: every search is ranked the same way. `full`
    /// still ignores a file limit (`--max-files`, search.fast_rank_files).
    #[arg(long, value_enum, default_value_t = Rank::Auto, hide = true)]
    pub rank: Rank,
    /// Stop scanning after this many milliseconds and return the best so far
    #[arg(long, default_value_t = 0, value_name = "MS")]
    pub timeout_ms: u64,
    /// Most candidate files to read, best first (default: the server's
    /// search.fast_rank_files, no limit)
    #[arg(long, value_name = "N")]
    pub max_files: Option<usize>,
    /// Only search this file and what it imports, N hops deep (1 by
    /// default; `*` for any distance). Repeatable; `-PATH` leaves them out.
    #[arg(long = "deps", value_name = "PATH[:N]", allow_hyphen_values = true)]
    pub deps: Vec<String>,
    /// Only search this file and the files that import it (as `--deps`).
    #[arg(long = "rdeps", value_name = "PATH[:N]", allow_hyphen_values = true)]
    pub rdeps: Vec<String>,
}

/// Search mode, chosen by the subcommand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Text,
    References,
    Symbols,
}

/// The parameters of one search, independent of transport.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchRequest {
    pub query: String,
    pub mode: Mode,
    pub regex: bool,
    pub case: Option<bool>,
    pub word: Option<bool>,
    pub max: usize,
    pub offset: usize,
    pub context: usize,
    pub include: String,
    pub exclude: String,
    pub rank: Rank,
    pub timeout_ms: u64,
    /// Files whose path matches to list too (0 = none).
    pub files: usize,
    /// Candidate files fast ranking may open (`None` = the server's default).
    pub max_files: Option<usize>,
    /// Graph filters, semicolon-separated `PATH[:N]` (`--deps` / `--rdeps`).
    pub deps: String,
    pub rdeps: String,
}

impl SearchRequest {
    pub fn from_args(args: &SearchArgs, mode: Mode) -> Self {
        Self {
            query: args.query.join(" "),
            mode,
            regex: args.regex,
            case: if args.case_sensitive {
                Some(true)
            } else if args.ignore_case {
                Some(false)
            } else {
                None
            },
            word: args.word.then_some(true),
            max: args.max.unwrap_or(DEFAULT_HITS).clamp(1, 1000),
            offset: args.offset,
            context: args.context.min(10),
            include: args.globs.join(";"),
            exclude: args.excludes.join(";"),
            rank: args.rank,
            timeout_ms: args.timeout_ms,
            files: 0,
            max_files: args.max_files,
            deps: args.deps.join(";"),
            rdeps: args.rdeps.join(";"),
        }
    }

    /// The `/api/search` query string parameters.
    pub fn to_params(&self) -> Vec<(&'static str, String)> {
        let mut p = vec![("q", self.query.clone()), ("max", self.max.to_string())];
        if self.offset > 0 {
            p.push(("offset", self.offset.to_string()));
        }
        if self.context > 0 {
            p.push(("context", self.context.to_string()));
        }
        if self.regex {
            p.push(("regex", "true".into()));
        }
        match self.mode {
            Mode::References => p.push(("references", "true".into())),
            Mode::Symbols => p.push(("symbols", "true".into())),
            Mode::Text => {}
        }
        if let Some(c) = self.case {
            p.push(("case", c.to_string()));
        }
        if let Some(w) = self.word {
            p.push(("word", w.to_string()));
        }
        if !self.include.is_empty() {
            p.push(("include", self.include.clone()));
        }
        if !self.exclude.is_empty() {
            p.push(("exclude", self.exclude.clone()));
        }
        if self.rank != Rank::Auto {
            p.push(("rank", format!("{:?}", self.rank).to_lowercase()));
        }
        if self.timeout_ms > 0 {
            p.push(("timeout_ms", self.timeout_ms.to_string()));
        }
        if self.files > 0 {
            p.push(("files", self.files.to_string()));
        }
        if let Some(n) = self.max_files {
            p.push(("max_files", n.to_string()));
        }
        if !self.deps.is_empty() {
            p.push(("deps", self.deps.clone()));
        }
        if !self.rdeps.is_empty() {
            p.push(("rdeps", self.rdeps.clone()));
        }
        p
    }
}

/// Make the command line clap-shaped without making users care where the
/// subcommand goes:
///
/// - `fcs QUERY` is `fcs search QUERY`: when no argument names a subcommand
///   (or help/version), `search` is inserted after the program name.
/// - `fcs -n 2 refs NAME` is `fcs refs -n 2 NAME`: a subcommand named later
///   on the line is moved to the front, so search flags may precede it.
///
/// Other arguments keep their relative order; global flags are accepted
/// anywhere. Everything after a `--` is left untouched.
pub fn normalize_args(mut args: Vec<std::ffi::OsString>) -> Vec<std::ffi::OsString> {
    if args.len() < 2 {
        return args;
    }
    let is_help_or_version = |s: &str| matches!(s, "--help" | "-h" | "--version" | "-V");
    let end = args.iter().position(|a| a == "--").unwrap_or(args.len());
    if args[1..end]
        .iter()
        .any(|a| a.to_str().is_some_and(is_help_or_version))
    {
        return args;
    }
    let sub_at = args[1..end]
        .iter()
        .position(|a| a.to_str().is_some_and(|s| SUBCOMMANDS.contains(&s)))
        .map(|i| i + 1);
    match sub_at {
        Some(1) => {}
        Some(i) => {
            let sub = args.remove(i);
            args.insert(1, sub);
        }
        None => {
            let has_value = args[1..end]
                .iter()
                .any(|a| !a.to_string_lossy().starts_with('-'));
            if has_value || end < args.len() {
                args.insert(1, "search".into());
            }
        }
    }
    args
}

/// Resolve the server base URL: `--server`, `$FCS_SERVER`, then the config's
/// `web_address`, then [`DEFAULT_SERVER`].
pub fn server_url(flag: Option<&str>, env: Option<&str>, config: Option<&Config>) -> String {
    let raw = flag
        .map(str::to_string)
        .or_else(|| env.filter(|s| !s.trim().is_empty()).map(str::to_string))
        .or_else(|| config.map(|c| c.server.web_address.clone()))
        .unwrap_or_else(|| DEFAULT_SERVER.to_string());
    let raw = raw.trim().trim_end_matches('/').to_string();
    let with_scheme = if raw.starts_with("http://") || raw.starts_with("https://") {
        raw
    } else {
        format!("http://{raw}")
    };
    // A bind address like 0.0.0.0:8080 is not connectable; use loopback.
    with_scheme
        .replace("://0.0.0.0", "://127.0.0.1")
        .replace("://[::]", "://[::1]")
}

/// Map a display path (`root/dir/file`) back to an absolute path using the
/// configured roots: the first component is a root's directory name.
pub fn absolute_path(display: &str, roots: &[String]) -> Option<PathBuf> {
    // The server shows each root under its folder name, or more folders
    // when two roots share a name (`crate::utils::root_labels`).
    let labels = crate::utils::root_labels(roots);
    let (root, rest) = roots.iter().zip(&labels).find_map(|(root, label)| {
        if label.is_empty() {
            return None;
        }
        if display == label {
            return Some((root, ""));
        }
        display
            .strip_prefix(label.as_str())
            .and_then(|r| r.strip_prefix('/'))
            .map(|rest| (root, rest))
    })?;
    let root = Path::new(root);
    Some(if rest.is_empty() {
        root.to_path_buf()
    } else {
        root.join(rest.replace('/', std::path::MAIN_SEPARATOR_STR))
    })
}

/// `abs` as a path that opens from `cwd`: relative when it is under it,
/// absolute otherwise. Roots written relative or with `..` in the
/// configuration are made absolute and tidied first, so they still match.
pub fn openable_path(abs: &Path, cwd: Option<&Path>) -> String {
    let abs = match cwd {
        Some(c) if abs.is_relative() => lexical_normalize(&c.join(abs)),
        _ => lexical_normalize(abs),
    };
    cwd.and_then(|c| abs.strip_prefix(c).ok())
        .filter(|rel| !rel.as_os_str().is_empty())
        .map(|rel| rel.to_string_lossy().into_owned())
        .unwrap_or_else(|| abs.to_string_lossy().into_owned())
}

/// Drop `.` and resolve `..` without touching the disk.
fn lexical_normalize(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// What `fcs` says on stderr when the hits printed are not all there are.
fn more_note(resp: &SearchResponse, req: &SearchRequest, files_only: bool) -> String {
    let shown = resp.results.len();
    let of = resp
        .total_matches
        .map(|t| format!(" of {t}"))
        .unwrap_or_default();
    if files_only {
        let files = resp
            .results
            .iter()
            .map(|r| r.file_path.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len();
        return format!("listed {files} files; more match (raise -n to list more)");
    }
    format!(
        "showing {shown} hits{of}; more exist: raise -n (up to 1000) or page with --offset {}",
        req.offset + shown
    )
}

/// Load the configuration named by `--config`, or the default locations.
fn load_config(flag: Option<&Path>) -> Result<Option<Config>> {
    match flag {
        Some(p) => Ok(Some(
            Config::from_file(p).with_context(|| format!("reading {}", p.display()))?,
        )),
        None => Ok(Config::from_default_locations()?.map(|(c, _)| c)),
    }
}

/// Everything a run needs, resolved from flags, environment and config.
pub struct Session {
    pub config: Option<Config>,
    pub server: String,
    pub conn: ConnectionArgs,
    pub output: OutputArgs,
    pub style: Style,
    pub format: Format,
    /// The on-disk index, loaded once for every offline search this run
    /// makes (`fcs -l` walks several pages).
    offline_engine: std::cell::OnceCell<crate::search::SearchEngine>,
}

impl Session {
    pub fn new(conn: ConnectionArgs, output: OutputArgs) -> Result<Self> {
        let config = load_config(conn.config.as_deref())?;
        let server = server_url(
            conn.server.as_deref(),
            std::env::var("FCS_SERVER").ok().as_deref(),
            config.as_ref(),
        );
        let tty = std::io::stdout().is_terminal();
        let color = match output.color {
            ColorChoice::Always => true,
            ColorChoice::Never => false,
            ColorChoice::Auto => tty && std::env::var_os("NO_COLOR").is_none(),
        };
        let format = if output.json {
            Format::Json
        } else {
            output.format.unwrap_or(if tty {
                Format::Grouped
            } else {
                Format::Vimgrep
            })
        };
        Ok(Self {
            config,
            server,
            conn,
            output,
            style: Style { color },
            format,
            offline_engine: std::cell::OnceCell::new(),
        })
    }

    fn index_path(&self) -> Option<PathBuf> {
        self.conn.index_path.clone().or_else(|| {
            self.config
                .as_ref()
                .and_then(|c| c.indexer.index_path.as_ref())
                .map(PathBuf::from)
        })
    }

    fn note(&self, msg: &str) {
        if !self.output.quiet {
            eprintln!("fcs: {msg}");
        }
    }

    fn path_mapper(&self) -> Box<dyn Fn(&str) -> String + '_> {
        let roots: Vec<String> = self
            .config
            .as_ref()
            .map(|c| c.indexer.paths.clone())
            .unwrap_or_default();
        if self.output.absolute {
            Box::new(move |p: &str| {
                absolute_path(p, &roots)
                    .map(|a| a.to_string_lossy().into_owned())
                    .unwrap_or_else(|| p.to_string())
            })
        } else if self.format == Format::Vimgrep || self.output.files_with_matches {
            // Output meant for an editor or `xargs`: paths that open from
            // here, like grep's and rg's (relative to the current folder
            // when the file is under it, else absolute). Without the
            // configuration's roots, the server's paths are all there is.
            let cwd = std::env::current_dir().ok();
            Box::new(move |p: &str| match absolute_path(p, &roots) {
                Some(abs) => openable_path(&abs, cwd.as_deref()),
                None => p.to_string(),
            })
        } else {
            Box::new(|p: &str| p.to_string())
        }
    }

    /// Every file the search matches (`fcs -l`), walking the results a
    /// server page at a time; at most `max_files` files when given.
    fn search_all_files(
        &self,
        req: &SearchRequest,
        max_files: Option<usize>,
    ) -> Result<SearchResponse> {
        let mut page_req = req.clone();
        page_req.max = FILES_ONLY_PAGE;
        let mut first = self.search(&page_req)?;
        let mut seen: std::collections::HashSet<String> =
            first.results.iter().map(|r| r.file_path.clone()).collect();
        let enough = |n: usize| max_files.is_some_and(|m| n >= m);
        while first.has_more && !first.results.is_empty() && !enough(seen.len()) {
            page_req.offset = req.offset + first.results.len();
            let next = match self.search(&page_req) {
                Ok(r) => r,
                // The server's deepest offset (search.max_page_offset):
                // stop there, still saying more exist.
                Err(e) => {
                    self.note(&format!("stopped listing files: {e:#}"));
                    break;
                }
            };
            for r in &next.results {
                seen.insert(r.file_path.clone());
            }
            first.has_more = next.has_more;
            first.total_matches = next.total_matches;
            for l in next.limits_hit {
                if !first.limits_hit.iter().any(|h| h.limit == l.limit) {
                    first.limits_hit.push(l);
                }
            }
            if next.results.is_empty() {
                break;
            }
            first.results.extend(next.results);
        }
        if let Some(m) = max_files {
            // Keep the hits of the first `m` files, in rank order.
            let mut kept: Vec<String> = Vec::new();
            first.results.retain(|r| {
                if kept.contains(&r.file_path) {
                    return true;
                }
                if kept.len() < m {
                    kept.push(r.file_path.clone());
                    return true;
                }
                first.has_more = true;
                false
            });
        }
        first.total_results = first.results.len();
        Ok(first)
    }

    /// Run a search against the server, or offline, and return the response.
    pub fn search(&self, req: &SearchRequest) -> Result<SearchResponse> {
        if self.conn.offline {
            return self.search_offline(req);
        }
        match self.search_server(req) {
            Ok(resp) => Ok(resp),
            Err(ServerError::Unreachable(err)) => {
                if !self.conn.no_offline && self.index_path().is_some() {
                    self.note(&format!(
                        "server not reachable at {} ({err}); searching the on-disk index (may be stale)",
                        self.server
                    ));
                    self.search_offline(req)
                } else {
                    Err(unreachable_error(&self.server, &err))
                }
            }
            Err(ServerError::Other(e)) => Err(e),
        }
    }

    fn client(&self, timeout_ms: u64) -> Result<reqwest::blocking::Client> {
        let total = Duration::from_millis(timeout_ms.max(1)).max(Duration::from_secs(35));
        reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(total)
            .build()
            .context("building HTTP client")
    }

    fn search_server(
        &self,
        req: &SearchRequest,
    ) -> std::result::Result<SearchResponse, ServerError> {
        let client = self.client(req.timeout_ms).map_err(ServerError::Other)?;
        let url = format!("{}/api/search", self.server);
        let params = req.to_params();
        let mut attempt = 0;
        loop {
            attempt += 1;
            let resp = client.get(&url).query(&params).send();
            let resp = match resp {
                Ok(r) => r,
                Err(e)
                    if e.is_connect() || e.is_timeout() && attempt == 1 && req.timeout_ms == 0 =>
                {
                    return Err(ServerError::Unreachable(e.to_string()));
                }
                Err(e) => return Err(ServerError::Other(anyhow!("request failed: {e}"))),
            };
            let status = resp.status();
            if status.as_u16() == 503 && attempt < 3 {
                // Index being updated: the server says how long to wait.
                let wait = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(1);
                std::thread::sleep(Duration::from_secs(wait.min(5)));
                continue;
            }
            let body = resp
                .text()
                .map_err(|e| ServerError::Other(anyhow!("reading response: {e}")))?;
            if !status.is_success() {
                let msg = serde_json::from_str::<ErrorResponse>(&body)
                    .map(|e| e.error)
                    .unwrap_or_else(|_| body.trim().to_string());
                return Err(ServerError::Other(anyhow!(
                    "server answered {status}: {msg}"
                )));
            }
            return serde_json::from_str(&body)
                .map_err(|e| ServerError::Other(anyhow!("unexpected response from {url}: {e}")));
        }
    }

    /// Load the persisted index in-process and run the same engine calls
    /// the server does. Read-only: nothing is saved back.
    /// Load the on-disk index for offline searches.
    fn load_offline_engine(&self) -> Result<crate::search::SearchEngine> {
        use crate::search::SearchEngine;
        let path = self.index_path().ok_or_else(|| {
            anyhow!(
                "no index file to search offline: set index_path in the configuration or pass --index-path"
            )
        })?;
        if !path.exists() {
            bail!("index file not found: {}", path.display());
        }
        let started = std::time::Instant::now();
        let mut engine = SearchEngine::new();
        let stale = match &self.config {
            Some(c) => {
                let mut indexer = c.indexer.clone();
                indexer.canonicalize_paths();
                engine
                    .load_index_with_reconciliation(&path, &indexer)
                    .with_context(|| format!("loading {}", path.display()))?
                    .stale_files
            }
            None => engine
                .load_index(&path)
                .with_context(|| format!("loading {}", path.display()))?,
        };
        if !stale.is_empty() {
            engine.update_files(&stale);
        }
        engine.finalize();
        self.note(&format!(
            "loaded {} files from {} in {:.1} s{}",
            engine.get_stats().num_files,
            path.display(),
            started.elapsed().as_secs_f64(),
            if stale.is_empty() {
                String::new()
            } else {
                format!(" ({} changed files re-read)", stale.len())
            }
        ));

        Ok(engine)
    }

    /// The on-disk index, loaded on first use.
    fn offline_engine(&self) -> Result<&crate::search::SearchEngine> {
        match self.offline_engine.get() {
            Some(e) => Ok(e),
            None => {
                let e = self.load_offline_engine()?;
                Ok(self.offline_engine.get_or_init(|| e))
            }
        }
    }

    fn search_offline(&self, req: &SearchRequest) -> Result<SearchResponse> {
        use crate::search::RankMode;

        let engine = self.offline_engine()?;

        let mode = match req.mode {
            Mode::References => crate::search::SearchMode::References,
            Mode::Symbols => crate::search::SearchMode::Symbols,
            Mode::Text if req.regex => crate::search::SearchMode::Regex,
            Mode::Text => crate::search::SearchMode::Text,
        };
        let request = crate::search::SearchRequest {
            query: req.query.clone(),
            mode,
            case_sensitive: req.case,
            whole_word: req.word,
            include: req.include.clone(),
            exclude: req.exclude.clone(),
            max_results: req.max,
            offset: req.offset,
            rank: match req.rank {
                Rank::Auto => RankMode::Auto,
                Rank::Fast => RankMode::Fast,
                Rank::Full => RankMode::Full,
            },
            timeout: (req.timeout_ms > 0).then(|| Duration::from_millis(req.timeout_ms)),
            max_files: req.max_files,
            path_matches: req.files,
            deps: crate::web::split_graph_param(&req.deps),
            rdeps: crate::web::split_graph_param(&req.rdeps),
        };
        let settings = self
            .config
            .as_ref()
            .map(crate::search::SearchSettings::from_config)
            .unwrap_or_default();
        let request = crate::search::SearchRequest {
            path_matches: request.path_matches.min(settings.max_path_matches),
            ..request
        };
        let limits = request.limits(&settings, None).map_err(|e| anyhow!(e))?;
        let search_started = std::time::Instant::now();
        let outcome = engine.run(&request, limits).map_err(|e| anyhow!(e))?;
        let has_more = outcome.has_more(req.offset);
        let info = outcome.info;
        let (files, total_files) = match outcome.paths {
            Some(found) => (
                crate::web::file_matches_to_json(found.matches),
                Some(found.total),
            ),
            None => (Vec::new(), None),
        };
        let results = crate::web::results_to_json(
            engine,
            outcome.matches,
            req.context,
            outcome.scope.as_deref(),
        );
        let total_results = results.len();
        Ok(SearchResponse {
            results,
            files,
            total_files,
            index: None,
            query: req.query.clone(),
            total_results,
            has_more,
            offset: req.offset,
            total_matches: info.total_matches,
            truncated_by_budget: info.truncated_by_budget,
            limits_hit: crate::web::LimitHitJson::all(&info),
            elapsed_ms: search_started.elapsed().as_secs_f64() * 1000.0,
            rank_mode: Some(format!("{:?}", info.mode).to_lowercase()),
            total_candidates: Some(info.total_candidates),
            candidates_searched: Some(info.candidates_searched),
            max_files_allowed: None,
            scope: outcome
                .scope
                .as_deref()
                .map(crate::web::ScopeJson::from_scope),
        })
    }

    /// Run an impact analysis on the server, or offline.
    pub fn impact(
        &self,
        req: &crate::web::DiffImpactRequest,
    ) -> Result<crate::search::ImpactReport> {
        let offline = |s: &Self| -> Result<crate::search::ImpactReport> {
            s.offline_engine()?
                .impact(&req.diff, &req.options())
                .map_err(|e| anyhow!(e))
        };
        if self.conn.offline {
            return offline(self);
        }
        let client = self.client(0)?;
        let url = format!("{}/api/impact", self.server);
        let mut attempt = 0;
        loop {
            attempt += 1;
            let resp = match client.post(&url).json(req).send() {
                Ok(r) => r,
                Err(e) if e.is_connect() => {
                    if !self.conn.no_offline && self.index_path().is_some() {
                        self.note(&format!(
                            "server not reachable at {} ({e}); using the on-disk index (may be stale)",
                            self.server
                        ));
                        return offline(self);
                    }
                    return Err(unreachable_error(&self.server, &e.to_string()));
                }
                Err(e) => bail!("request failed: {e}"),
            };
            let status = resp.status();
            if status.as_u16() == 503 && attempt < 3 {
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
            let body = resp.text().context("reading response")?;
            if !status.is_success() {
                let msg = serde_json::from_str::<ErrorResponse>(&body)
                    .map(|e| e.error)
                    .unwrap_or_else(|_| body.trim().to_string());
                if status.as_u16() == 404 || status.as_u16() == 405 {
                    bail!("the server at {} has no impact analysis (older than fcs; restart it with this version)", self.server);
                }
                bail!("server answered {status}: {msg}");
            }
            return serde_json::from_str(&body)
                .map_err(|e| anyhow!("unexpected response from {url}: {e}"));
        }
    }

    /// `fcs status`: server health and index size, or the offline picture.
    pub fn status(&self) -> Result<String> {
        let mut out = String::new();
        if let Some(c) = &self.config {
            out.push_str(&format!(
                "config: {} path(s), index_path = {}\n",
                c.indexer.paths.len(),
                c.indexer.index_path.as_deref().unwrap_or("(none)")
            ));
        } else {
            out.push_str("config: none found (defaults)\n");
        }
        out.push_str(&format!("server: {}\n", self.server));
        let client = self.client(0)?;
        let health = client.get(format!("{}/api/health", self.server)).send();
        match health {
            Ok(r) if r.status().is_success() => {
                let h: serde_json::Value = r.json().unwrap_or_default();
                out.push_str(&format!(
                    "  reachable, version {}\n",
                    h["version"].as_str().unwrap_or("?")
                ));
                if let Ok(r) = client.get(format!("{}/api/ready", self.server)).send() {
                    let v: serde_json::Value = r.json().unwrap_or_default();
                    out.push_str(&format!(
                        "  status {}, ready = {}\n",
                        v["status"].as_str().unwrap_or("?"),
                        v["ready"].as_bool().unwrap_or(false)
                    ));
                }
                if let Ok(r) = client.get(format!("{}/api/stats", self.server)).send() {
                    let v: serde_json::Value = r.json().unwrap_or_default();
                    out.push_str(&format!(
                        "  {} files, {} trigrams, {} dependency edges, {} MB of content\n",
                        v["num_files"].as_u64().unwrap_or(0),
                        v["num_trigrams"].as_u64().unwrap_or(0),
                        v["dependency_edges"].as_u64().unwrap_or(0),
                        v["total_content_bytes"].as_u64().unwrap_or(0) / 1_000_000
                    ));
                }
                let freshness = client
                    .get(format!("{}/api/status", self.server))
                    .send()
                    .ok()
                    .and_then(|r| r.json::<serde_json::Value>().ok())
                    .and_then(|v| {
                        serde_json::from_value::<crate::search::IndexFreshness>(v["index"].clone())
                            .ok()
                    });
                if let Some(f) = freshness {
                    out.push_str(&format!("  {}\n", f.summary(crate::search::unix_now_ms())));
                }
            }
            Ok(r) => out.push_str(&format!("  answered {}\n", r.status())),
            Err(e) => {
                out.push_str(&format!("  not reachable ({e})\n"));
                match self.index_path() {
                    Some(p) if p.exists() => out.push_str(&format!(
                        "  offline searches will use {}\n",
                        p.display()
                    )),
                    _ => out.push_str(
                        "  no index file for offline searches; start the server (docs/RUN-AT-STARTUP.md)\n",
                    ),
                }
            }
        }
        Ok(out)
    }
}

enum ServerError {
    Unreachable(String),
    Other(anyhow::Error),
}

fn unreachable_error(server: &str, err: &str) -> anyhow::Error {
    anyhow!(
        "cannot reach the search server at {server} ({err}).\n  \
         Start it (fast_code_search_server --config <file>; see docs/RUN-AT-STARTUP.md to run it at login),\n  \
         point at another with --server URL or $FCS_SERVER,\n  \
         or search the on-disk index with --offline --index-path <file>."
    )
}

/// Run the CLI with `args` (including the program name); returns the exit
/// code (0 matches, 1 no matches, 2 error).
pub fn main_with_args(args: Vec<std::ffi::OsString>) -> i32 {
    let cli = match Cli::try_parse_from(normalize_args(args)) {
        Ok(c) => c,
        Err(e) => {
            // clap prints help/version itself with exit 0; usage errors exit 2.
            let code = if e.use_stderr() { 2 } else { 0 };
            let _ = e.print();
            return code;
        }
    };
    match run(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("fcs: error: {e:#}");
            2
        }
    }
}

/// `fcs check --live`: the server's own check of its index
/// (`/api/check`). Exit 1 when it found problems.
fn check_live(session: &Session, args: &CheckArgs) -> Result<i32> {
    session.note(&format!(
        "checking the index of the server at {}…",
        session.server
    ));
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(2))
        // Reading every file of a large index takes a while.
        .timeout(Duration::from_secs(3600))
        .build()
        .context("building HTTP client")?;
    let mut url = format!(
        "{}/api/check?content={}&discovery={}&examples={}",
        session.server,
        args.content,
        !args.no_discovery,
        if args.show_paths { 20 } else { 0 }
    );
    if args.all_content {
        url.push_str("&all=true");
    }
    let resp = client
        .get(&url)
        .send()
        .with_context(|| format!("server not reachable at {}", session.server))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        anyhow::bail!("the server answered {status}: {body}");
    }
    let report: crate::search::integrity::IntegrityReport =
        resp.json().context("reading the check report")?;
    let text = match session.format {
        Format::Json => serde_json::to_string_pretty(&report)? + "\n",
        _ => report.render(args.show_paths),
    };
    print!("{text}");
    Ok(i32::from(report.problems() > 0))
}

/// `fcs check`: see [`crate::check::run_check`].
fn check(session: &Session, args: &CheckArgs) -> Result<i32> {
    if args.live {
        return check_live(session, args);
    }
    let mut indexer = session
        .config
        .as_ref()
        .map(|c| c.indexer.clone())
        .unwrap_or_default();
    if !args.paths.is_empty() {
        indexer.paths = args
            .paths
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
    } else if indexer.paths.is_empty() {
        indexer.paths = vec![".".into()];
    }
    let opts = crate::check::CheckOptions {
        queries: if args.no_search { 0 } else { args.queries },
        seed: args.seed,
        rg: args.rg.clone(),
        graph: !args.no_graph,
        ..Default::default()
    };
    let mut stdout = std::io::stdout().lock();
    crate::check::run_check(&indexer, &opts, &mut stdout, &|m| session.note(m))
}

/// `fcs diagnose`: summarize the captures in a folder. Exit 1 when there
/// are none.
fn diagnose(session: &Session, args: &DiagnoseArgs) -> Result<i32> {
    use crate::diagnostics::capture;
    let dir = match &args.dir {
        Some(d) => d.clone(),
        None => {
            let config = session.config.clone().unwrap_or_default();
            capture::capture_dir(&config.diagnostics, config.indexer.index_path.as_deref())
        }
    };
    let (captures, bad) = capture::load_captures(&dir);
    print!("{}", capture::summarize(&dir, &captures, &bad));
    Ok(if captures.is_empty() { 1 } else { 0 })
}

fn run(cli: Cli) -> Result<i32> {
    let session = Session::new(cli.conn, cli.output)?;
    let (args, mode) = match cli.command {
        Command::Status => {
            print!("{}", session.status()?);
            return Ok(0);
        }
        Command::Mcp => {
            mcp::run(&session.server)?;
            return Ok(0);
        }
        Command::Check(a) => return check(&session, &a),
        Command::Diagnose(a) => return diagnose(&session, &a),
        Command::Impact(a) => return impact(&session, &a),
        Command::Search(a) => (a, Mode::Text),
        Command::Refs(a) => (a, Mode::References),
        Command::Symbols(a) => (a, Mode::Symbols),
    };
    let mut req = SearchRequest::from_args(&args, mode);
    if req.query.trim().is_empty() {
        bail!("empty query");
    }
    // On a terminal, files whose path matches are listed above the hits;
    // piped output stays grep-shaped.
    if session.format == Format::Grouped && !session.output.files_with_matches {
        req.files = GROUPED_FILE_MATCHES;
    }
    let resp = if session.output.files_with_matches && session.format != Format::Json {
        // Like `rg -l`: every file that matches, not the files of the first
        // page of hits (which listed 50 of 120 files without a word).
        session.search_all_files(&req, args.max)?
    } else {
        session.search(&req)?
    };
    let mapper = session.path_mapper();
    let text = format::render(
        &resp,
        session.format,
        &session.style,
        session.output.files_with_matches,
        &*mapper,
    );
    let mut stdout = std::io::stdout().lock();
    if let Err(e) = stdout
        .write_all(text.as_bytes())
        .and_then(|_| stdout.flush())
    {
        // A closed pipe (`fcs … | head`) is not an error worth reporting.
        if e.kind() == std::io::ErrorKind::BrokenPipe {
            return Ok(0);
        }
        return Err(e.into());
    }
    if session.format == Format::Grouped && !session.output.quiet {
        eprintln!("{}", format::summary(&resp));
    }
    // A capped result must never pass for a complete one.
    for hit in &resp.limits_hit {
        session.note(&format!("results capped: {}", hit.detail));
    }
    // Nor a page for the whole result, piped or not.
    if resp.has_more && session.format != Format::Json {
        session.note(&more_note(&resp, &req, session.output.files_with_matches));
    }
    // Without a file watcher the server misses writes after its last scan.
    if let Some(note) = resp
        .index
        .as_ref()
        .and_then(|f| f.note(crate::search::unix_now_ms()))
    {
        session.note(&note);
    }
    Ok(if resp.results.is_empty() && resp.files.is_empty() {
        1
    } else {
        0
    })
}

/// Run `git` in the current folder and return its stdout.
fn git(args: &[&str]) -> Result<String> {
    let out = std::process::Command::new("git")
        .args(args)
        .output()
        .context("running git (pass the diff with --diff FILE when git is not available)")?;
    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The diff `fcs impact` analyses, and the folder its paths are relative to.
fn impact_diff(args: &ImpactArgs) -> Result<(String, Option<PathBuf>)> {
    let toplevel = || {
        git(&["rev-parse", "--show-toplevel"])
            .ok()
            .map(|s| PathBuf::from(s.trim()))
    };
    if let Some(file) = &args.diff {
        let text = if file.as_os_str() == "-" {
            let mut s = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut s)
                .context("reading the diff from stdin")?;
            s
        } else {
            std::fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?
        };
        return Ok((text, args.root.clone().or_else(toplevel)));
    }
    let root = args.root.clone().or_else(toplevel).ok_or_else(|| {
        anyhow!(
            "not in a Git repository: run fcs impact inside one, or pass the diff with --diff FILE"
        )
    })?;
    let root_s = root.to_string_lossy().into_owned();
    // Plain unified diff with repository-relative paths, whatever the
    // user's git configuration says.
    let mut cmd: Vec<&str> = vec![
        "-C",
        &root_s,
        "diff",
        "--no-color",
        "--no-ext-diff",
        "--no-renames",
        "--src-prefix=a/",
        "--dst-prefix=b/",
    ];
    if args.staged {
        cmd.push("--cached");
    }
    if args.revs.is_empty() && !args.staged {
        cmd.push("HEAD");
    }
    cmd.extend(args.revs.iter().map(String::as_str));
    Ok((git(&cmd)?, Some(root)))
}

/// `fcs impact`: exit 0 when something uses the changed code, 1 when
/// nothing does (or nothing changed), 2 on error.
fn impact(session: &Session, args: &ImpactArgs) -> Result<i32> {
    let (diff, root) = impact_diff(args)?;
    if diff.trim().is_empty() {
        session.note("no changes to analyse");
        return Ok(1);
    }
    let req = crate::web::DiffImpactRequest {
        diff,
        root: root.map(|r| r.to_string_lossy().into_owned()),
        max_files: Some(args.max_files),
        max_lines: Some(args.max_lines),
    };
    let report = session.impact(&req)?;
    let mapper = session.path_mapper();
    let text = match session.format {
        Format::Json => serde_json::to_string_pretty(&report)? + "\n",
        Format::Vimgrep => crate::impact::report::vimgrep(&report, &*mapper),
        Format::Grouped => crate::impact::report::grouped(&report, &*mapper, session.style.color),
    };
    let mut stdout = std::io::stdout().lock();
    if let Err(e) = stdout
        .write_all(text.as_bytes())
        .and_then(|_| stdout.flush())
    {
        if e.kind() == std::io::ErrorKind::BrokenPipe {
            return Ok(0);
        }
        return Err(e.into());
    }
    if session.format != Format::Json {
        // Piped output is only use lines: the caps and gaps go to stderr.
        if session.format == Format::Vimgrep {
            for n in crate::impact::report::caps(&report) {
                session.note(&n);
            }
        }
        session.note(&crate::impact::report::summary(&report));
    }
    Ok(if report.total_files == 0 { 1 } else { 0 })
}

/// Path matches listed above the hits in grouped output.
const GROUPED_FILE_MATCHES: usize = 5;

/// Hits printed when `-n` is not given.
const DEFAULT_HITS: usize = 50;

/// Page size `fcs -l` walks the results with (the server's largest).
const FILES_ONLY_PAGE: usize = 1000;

#[cfg(test)]
mod tests {
    use super::*;

    fn os(args: &[&str]) -> Vec<std::ffi::OsString> {
        args.iter().map(|a| a.into()).collect()
    }

    #[test]
    fn bare_query_becomes_search_subcommand() {
        let a = normalize_args(os(&["fcs", "fn main"]));
        assert_eq!(a, os(&["fcs", "search", "fn main"]));
        let a = normalize_args(os(&["fcs", "-n", "20", "foo"]));
        assert_eq!(a, os(&["fcs", "search", "-n", "20", "foo"]));
        // Subcommands, help and version are left alone.
        assert_eq!(
            normalize_args(os(&["fcs", "status"])),
            os(&["fcs", "status"])
        );
        assert_eq!(
            normalize_args(os(&["fcs", "refs", "x"])),
            os(&["fcs", "refs", "x"])
        );
        assert_eq!(
            normalize_args(os(&["fcs", "--help"])),
            os(&["fcs", "--help"])
        );
        assert_eq!(normalize_args(os(&["fcs"])), os(&["fcs"]));
        // A subcommand named after some flags moves to the front.
        assert_eq!(
            normalize_args(os(&["fcs", "--absolute", "-n", "2", "refs", "x"])),
            os(&["fcs", "refs", "--absolute", "-n", "2", "x"])
        );
        assert_eq!(
            normalize_args(os(&["fcs", "-q", "status"])),
            os(&["fcs", "status", "-q"])
        );
        // `--` protects a query that looks like a flag or a subcommand.
        assert_eq!(
            normalize_args(os(&["fcs", "--", "-status"])),
            os(&["fcs", "search", "--", "-status"])
        );
    }

    #[test]
    fn parses_search_flags_into_a_request() {
        let cli = Cli::try_parse_from(normalize_args(os(&[
            "fcs",
            "-e",
            "-s",
            "-w",
            "-n",
            "10",
            "-C",
            "2",
            "-g",
            "*.rs",
            "-g",
            "src/**",
            "--exclude",
            "tests",
            "--rank",
            "full",
            "--offset",
            "10",
            "fn",
            "main",
        ])))
        .unwrap();
        let Command::Search(args) = cli.command else {
            panic!("expected search");
        };
        let req = SearchRequest::from_args(&args, Mode::Text);
        assert_eq!(req.query, "fn main");
        assert!(req.regex);
        assert_eq!(req.case, Some(true));
        assert_eq!(req.word, Some(true));
        assert_eq!(req.max, 10);
        assert_eq!(req.context, 2);
        assert_eq!(req.include, "*.rs;src/**");
        assert_eq!(req.exclude, "tests");
        assert_eq!(req.rank, Rank::Full);
        let params = req.to_params();
        assert!(params.contains(&("regex", "true".to_string())));
        assert!(params.contains(&("case", "true".to_string())));
        assert!(params.contains(&("rank", "full".to_string())));
        assert!(params.contains(&("offset", "10".to_string())));
        assert!(!params.iter().any(|(k, _)| *k == "references"));
    }

    #[test]
    fn refs_and_symbols_set_the_mode() {
        let cli = Cli::try_parse_from(os(&["fcs", "refs", "SearchEngine"])).unwrap();
        assert!(matches!(cli.command, Command::Refs(_)));
        let cli = Cli::try_parse_from(os(&["fcs", "symbols", "--json", "x"])).unwrap();
        assert!(cli.output.json);
        let Command::Symbols(args) = cli.command else {
            panic!()
        };
        let req = SearchRequest::from_args(&args, Mode::Symbols);
        assert!(req.to_params().contains(&("symbols", "true".to_string())));
    }

    #[test]
    fn impact_takes_revisions_or_a_diff_file() {
        let cli =
            Cli::try_parse_from(normalize_args(os(&["fcs", "--json", "impact", "main"]))).unwrap();
        let Command::Impact(a) = cli.command else {
            panic!()
        };
        assert_eq!(a.revs, ["main"]);
        assert_eq!(a.max_files, crate::search::engine::DEFAULT_IMPACT_FILES);
        assert!(cli.output.json);
        let cli = Cli::try_parse_from(os(&[
            "fcs", "impact", "--diff", "-", "-n", "0", "--lines", "1",
        ]))
        .unwrap();
        let Command::Impact(a) = cli.command else {
            panic!()
        };
        assert_eq!(a.diff.as_deref(), Some(Path::new("-")));
        assert_eq!((a.max_files, a.max_lines), (0, 1));
        // A diff file and revisions are two different inputs.
        assert!(Cli::try_parse_from(os(&["fcs", "impact", "--diff", "x", "HEAD"])).is_err());
    }

    #[test]
    fn global_flags_are_accepted_before_and_after_the_query() {
        let cli = Cli::try_parse_from(normalize_args(os(&[
            "fcs",
            "--server",
            "http://h:1",
            "q",
            "--offline",
            "--color",
            "never",
        ])))
        .unwrap();
        assert_eq!(cli.conn.server.as_deref(), Some("http://h:1"));
        assert!(cli.conn.offline);
        assert_eq!(cli.output.color, ColorChoice::Never);
    }

    #[test]
    fn server_url_precedence_and_normalisation() {
        let mut cfg = Config::default();
        cfg.server.web_address = "0.0.0.0:9000".into();
        assert_eq!(
            server_url(Some("http://a:1/"), Some("http://b:2"), Some(&cfg)),
            "http://a:1"
        );
        assert_eq!(server_url(None, Some("b:2"), Some(&cfg)), "http://b:2");
        assert_eq!(
            server_url(None, Some("  "), Some(&cfg)),
            "http://127.0.0.1:9000"
        );
        assert_eq!(server_url(None, None, None), DEFAULT_SERVER);
    }

    #[test]
    #[cfg(unix)]
    fn openable_paths_tidy_relative_and_dotted_roots() {
        let cwd = Path::new("/w/c/proj/src");
        let dotted = Path::new("/w/v/../c/proj/src/lib.rs");
        assert_eq!(openable_path(dotted, Some(cwd)), "lib.rs");
        let relative = Path::new("../../../v/../c/proj/src/lib.rs");
        assert_eq!(openable_path(relative, Some(cwd)), "lib.rs");
        let outside = Path::new("/w/other/x.rs");
        assert_eq!(openable_path(outside, Some(cwd)), "/w/other/x.rs");
    }

    #[test]
    fn absolute_paths_use_the_root_directory_name() {
        let roots = vec!["/home/me/work/proj".to_string(), "/srv/other".to_string()];
        assert_eq!(
            absolute_path("proj/src/main.rs", &roots),
            Some(
                PathBuf::from("/home/me/work/proj")
                    .join("src")
                    .join("main.rs")
            )
        );
        assert_eq!(
            absolute_path("other", &roots),
            Some(PathBuf::from("/srv/other"))
        );
        assert_eq!(absolute_path("unknown/x.rs", &roots), None);
    }
}
