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
//! - **Same semantics as the web UI.** The query syntax (`file:`, `lang:`,
//!   `-term`, `"phrase"`, `case:`, `word:`), modes (regex, symbols,
//!   references) and result shape are the server's; offline searches go
//!   through the same engine calls the server uses.
//! - **Grep conventions.** `path:line:col:text` when piped (what editors'
//!   quickfix lists read), grouped and coloured on a terminal, `--json` for
//!   tools. Exit code 0 = matches, 1 = none, 2 = error.

pub mod format;

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
pub const SUBCOMMANDS: &[&str] = &["search", "refs", "symbols", "status", "help"];

#[derive(Parser, Debug)]
#[command(
    name = "fcs",
    version,
    about = "Search a fast_code_search index from the command line",
    long_about = "Search the code index kept by a running fast_code_search server.\n\n\
                  `fcs QUERY` is `fcs search QUERY`. The query syntax is the web UI's: several \
                  words must all appear in a file (lines with the phrase rank first), \
                  \"quoted phrase\", -term, file:PATTERN, -file:PATTERN, lang:rust, \
                  case:yes, word:yes.\n\n\
                  Exit status: 0 = matches found, 1 = no matches, 2 = error.",
    after_help = "Examples:\n  \
                  fcs 'fn main'                 lines holding both words, phrase first\n  \
                  fcs -e 'fn\\s+\\w+\\(' -g '*.rs'  regex, Rust files only\n  \
                  fcs refs SearchEngine         call sites and type mentions\n  \
                  fcs symbols parse_query       definitions only\n  \
                  fcs -l TODO | xargs $EDITOR   files containing TODO\n  \
                  vim -q <(fcs --format vimgrep TODO)\n  \
                  fcs status                    is the server up, what does it hold"
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
    /// Find references (call sites, type mentions) of an identifier
    Refs(SearchArgs),
    /// Search symbol definitions (functions, types, classes, ...) only
    Symbols(SearchArgs),
    /// Show whether the server is reachable and what it has indexed
    Status,
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
    /// Print only the files that match, one per line
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
    /// The query (web UI syntax; quote it if it has spaces)
    #[arg(value_name = "QUERY", required = true, num_args = 1..)]
    pub query: Vec<String>,
    /// Treat the query as a regular expression
    #[arg(short = 'e', long)]
    pub regex: bool,
    /// Match case exactly
    #[arg(short = 's', long, conflicts_with = "ignore_case")]
    pub case_sensitive: bool,
    /// Ignore case (the default for plain text; useful to override case:yes)
    #[arg(short = 'i', long)]
    pub ignore_case: bool,
    /// Match whole words only
    #[arg(short = 'w', long)]
    pub word: bool,
    /// Maximum hits to print (1-1000)
    #[arg(short = 'n', long, default_value_t = 50, value_name = "N")]
    pub max: usize,
    /// Skip the first N hits (paging; ordering is deterministic)
    #[arg(long, default_value_t = 0, value_name = "N")]
    pub offset: usize,
    /// Lines of context before and after each hit (0-10)
    #[arg(short = 'C', long, default_value_t = 0, value_name = "N")]
    pub context: usize,
    /// Only files matching this glob (repeatable; `src/**/*.rs`, `*.py`, `tests`)
    #[arg(short = 'g', long = "glob", value_name = "GLOB")]
    pub globs: Vec<String>,
    /// Skip files matching this glob (repeatable)
    #[arg(long = "exclude", value_name = "GLOB")]
    pub excludes: Vec<String>,
    /// Ranking: auto, fast (metadata only, big candidate sets), full
    #[arg(long, value_enum, default_value_t = Rank::Auto)]
    pub rank: Rank,
    /// Stop scanning after this many milliseconds and return the best so far
    #[arg(long, default_value_t = 0, value_name = "MS")]
    pub timeout_ms: u64,
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
            max: args.max.clamp(1, 1000),
            offset: args.offset,
            context: args.context.min(10),
            include: args.globs.join(";"),
            exclude: args.excludes.join(";"),
            rank: args.rank,
            timeout_ms: args.timeout_ms,
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
    let mut parts = display.splitn(2, '/');
    let first = parts.next()?;
    let rest = parts.next().unwrap_or("");
    let root = roots.iter().map(Path::new).find(|r| {
        r.file_name()
            .map(|n| n.to_string_lossy() == first)
            .unwrap_or(false)
    })?;
    Some(if rest.is_empty() {
        root.to_path_buf()
    } else {
        root.join(rest.replace('/', std::path::MAIN_SEPARATOR_STR))
    })
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
        if self.output.absolute {
            let roots: Vec<String> = self
                .config
                .as_ref()
                .map(|c| c.indexer.paths.clone())
                .unwrap_or_default();
            Box::new(move |p: &str| {
                absolute_path(p, &roots)
                    .map(|a| a.to_string_lossy().into_owned())
                    .unwrap_or_else(|| p.to_string())
            })
        } else {
            Box::new(|p: &str| p.to_string())
        }
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
    fn search_offline(&self, req: &SearchRequest) -> Result<SearchResponse> {
        use crate::search::{RankMode, SearchEngine, SearchLimits};

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

        let mut parsed = crate::search::parse_query(&req.query);
        if let Some(c) = req.case {
            parsed.options.case_sensitive = c;
        }
        if let Some(w) = req.word {
            parsed.options.whole_word = w;
        }
        let mut limits = SearchLimits::new(req.max).with_offset(req.offset);
        if req.timeout_ms > 0 {
            limits = limits.with_timeout(Duration::from_millis(req.timeout_ms.min(30_000)));
        }
        let rank_mode = match req.rank {
            Rank::Auto => RankMode::Auto,
            Rank::Fast => RankMode::Fast,
            Rank::Full => RankMode::Full,
        };
        let search_started = std::time::Instant::now();
        let (matches, info) = match req.mode {
            Mode::References => {
                engine.search_references_parsed(&parsed, &req.include, &req.exclude, limits)?
            }
            Mode::Symbols => {
                engine.search_symbols_parsed(&parsed, &req.include, &req.exclude, limits)?
            }
            Mode::Text if req.regex => engine.search_regex_with_limits(
                &req.query,
                &req.include,
                &req.exclude,
                limits,
                rank_mode,
            )?,
            Mode::Text => {
                engine.search_parsed(&parsed, &req.include, &req.exclude, limits, rank_mode)?
            }
        };
        let results = crate::web::results_to_json(&engine, matches, req.context);
        let total_results = results.len();
        Ok(SearchResponse {
            results,
            query: req.query.clone(),
            total_results,
            has_more: match info.total_matches {
                Some(t) => req.offset + total_results < t,
                None => true,
            },
            offset: req.offset,
            total_matches: info.total_matches,
            truncated_by_budget: info.truncated_by_budget,
            elapsed_ms: search_started.elapsed().as_secs_f64() * 1000.0,
            rank_mode: Some(format!("{:?}", info.mode).to_lowercase()),
            total_candidates: Some(info.total_candidates),
            candidates_searched: Some(info.candidates_searched),
        })
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

fn run(cli: Cli) -> Result<i32> {
    let session = Session::new(cli.conn, cli.output)?;
    let (args, mode) = match cli.command {
        Command::Status => {
            print!("{}", session.status()?);
            return Ok(0);
        }
        Command::Search(a) => (a, Mode::Text),
        Command::Refs(a) => (a, Mode::References),
        Command::Symbols(a) => (a, Mode::Symbols),
    };
    let req = SearchRequest::from_args(&args, mode);
    if req.query.trim().is_empty() {
        bail!("empty query");
    }
    let resp = session.search(&req)?;
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
    Ok(if resp.results.is_empty() { 1 } else { 0 })
}

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
