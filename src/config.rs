//! Configuration management for fast_code_search
//!
//! Supports loading configuration from TOML files with CLI overrides.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::utils::normalize_path_for_comparison;

/// Telemetry / OpenTelemetry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TelemetryConfig {
    /// Enable OpenTelemetry trace export (default: false)
    /// Can be overridden by env var FCS_TRACING_ENABLED or OTEL_SDK_DISABLED
    #[serde(default)]
    pub enabled: bool,

    /// OTLP exporter endpoint (default: http://localhost:4317)
    /// Can be overridden by env var OTEL_EXPORTER_OTLP_ENDPOINT
    #[serde(default = "default_otlp_endpoint")]
    pub otlp_endpoint: String,

    /// Service name reported to the collector (default: fast_code_search)
    /// Can be overridden by env var OTEL_SERVICE_NAME
    #[serde(default = "default_service_name")]
    pub service_name: String,
}

fn default_otlp_endpoint() -> String {
    "http://localhost:4317".to_string()
}

fn default_service_name() -> String {
    "fast_code_search".to_string()
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            otlp_endpoint: default_otlp_endpoint(),
            service_name: default_service_name(),
        }
    }
}

impl TelemetryConfig {
    /// Apply environment variable overrides.
    /// Env vars take precedence over TOML config values.
    pub fn with_env_overrides(mut self) -> Self {
        // FCS_TRACING_ENABLED overrides the TOML value (project-specific switch)
        if let Ok(val) = std::env::var("FCS_TRACING_ENABLED") {
            self.enabled = val.eq_ignore_ascii_case("true") || val == "1";
        }
        // OTEL_SDK_DISABLED=true is the standard kill-switch and is FINAL:
        // nothing else may re-enable export after it.
        if let Ok(val) = std::env::var("OTEL_SDK_DISABLED") {
            if val.eq_ignore_ascii_case("true") {
                self.enabled = false;
            }
        }
        if let Ok(val) = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
            if !val.is_empty() {
                self.otlp_endpoint = val;
            }
        }
        if let Ok(val) = std::env::var("OTEL_SERVICE_NAME") {
            if !val.is_empty() {
                self.service_name = val;
            }
        }
        self
    }
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,

    #[serde(default)]
    pub indexer: IndexerConfig,

    #[serde(default)]
    pub telemetry: TelemetryConfig,

    #[serde(default)]
    pub search: SearchConfig,

    #[serde(default)]
    pub mcp: McpConfig,

    #[serde(default)]
    pub diagnostics: DiagnosticsConfig,
}

/// Self-diagnosis (`[diagnostics]`): when on, the server notices when it is
/// slow or stuck and writes what it was doing to a file you can look at
/// later (`fcs diagnose`). Off by default. Thresholds are measured; see the
/// book's "Diagnostic captures" page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticsConfig {
    /// Write diagnostic captures (default: false).
    #[serde(default)]
    pub capture: bool,
    /// Where captures go. Empty (the default): a `diagnostics` folder next
    /// to the index, or the user cache folder when there is no index file.
    #[serde(default)]
    pub dir: String,
    /// A search slower than this, from the request to the reply, is captured.
    #[serde(default = "default_diag_slow_search_secs")]
    pub slow_search_secs: f64,
    /// A search that waited longer than this for the index lock is captured.
    #[serde(default = "default_diag_lock_wait_secs")]
    pub lock_wait_secs: f64,
    /// Work that makes no progress for this long is a hang: the watcher with
    /// changes to apply, the index build, a search that has not finished,
    /// or the index lock never free. The capture says what every thread is
    /// doing.
    #[serde(default = "default_diag_stall_secs")]
    pub stall_secs: f64,
    /// The watcher still has changes to apply after this long without a
    /// break: it is falling behind.
    #[serde(default = "default_diag_backlog_secs")]
    pub backlog_secs: f64,
    /// Seconds before the same kind of capture is written again.
    #[serde(default = "default_diag_min_interval_secs")]
    pub min_interval_secs: f64,
    /// Captures kept; the oldest are deleted.
    #[serde(default = "default_diag_max_files")]
    pub max_files: usize,
    /// Megabytes of captures kept; the oldest are deleted.
    #[serde(default = "default_diag_max_total_mb")]
    pub max_total_mb: u64,
    /// Log lines kept for each capture.
    #[serde(default = "default_diag_log_lines")]
    pub log_lines: usize,
    /// On a hang, record every thread's stack with `gdb` (or `eu-stack`)
    /// when one is installed. It pauses the server for a second or two.
    #[serde(default = "default_true")]
    pub thread_stacks: bool,
    /// Keep query text, file paths and quoted text in captures. Off by
    /// default: captures then say how long a query was, not what it was.
    #[serde(default)]
    pub include_queries: bool,
}

impl Default for DiagnosticsConfig {
    fn default() -> Self {
        Self {
            capture: false,
            dir: String::new(),
            slow_search_secs: default_diag_slow_search_secs(),
            lock_wait_secs: default_diag_lock_wait_secs(),
            stall_secs: default_diag_stall_secs(),
            backlog_secs: default_diag_backlog_secs(),
            min_interval_secs: default_diag_min_interval_secs(),
            max_files: default_diag_max_files(),
            max_total_mb: default_diag_max_total_mb(),
            log_lines: default_diag_log_lines(),
            thread_stacks: true,
            include_queries: false,
        }
    }
}

fn default_diag_slow_search_secs() -> f64 {
    5.0
}
fn default_diag_lock_wait_secs() -> f64 {
    2.0
}
fn default_diag_stall_secs() -> f64 {
    120.0
}
fn default_diag_backlog_secs() -> f64 {
    300.0
}
fn default_diag_min_interval_secs() -> f64 {
    300.0
}
fn default_diag_max_files() -> usize {
    50
}
fn default_diag_max_total_mb() -> u64 {
    100
}
fn default_diag_log_lines() -> usize {
    300
}

/// Search limits (`[search]`), applied to every front end: web UI, REST,
/// gRPC, `fcs` and MCP. Defaults are measured; see the book's "Limits" page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchConfig {
    /// Most candidate files a search reads (best first) before it stops,
    /// unless the request asks for more. 0 (the default) = no cap: read
    /// until the match budget is spent or the deadline passes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast_rank_files: Option<usize>,

    /// Most candidate files a request may ask for (`max_files` on
    /// `/api/search`, the web UI's SEARCH MORE FILES). 0 = no limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_fast_rank_files: Option<usize>,

    /// Deepest result offset a client may page to. 0 = no limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_page_offset: Option<usize>,

    /// Matches collected to rank a page, at least.
    #[serde(default = "default_min_match_budget")]
    pub min_match_budget: usize,

    /// Matches collected per result of the page end (`offset + page`).
    #[serde(default = "default_match_budget_multiplier")]
    pub match_budget_multiplier: usize,

    /// Most matches collected for a page (deeper pages still get enough to
    /// reach them).
    #[serde(default = "default_max_match_budget")]
    pub max_match_budget: usize,

    /// Most hits one file contributes (raised to the page end).
    #[serde(default = "default_per_file_matches")]
    pub per_file_matches: usize,

    /// Page size when a request does not give one.
    #[serde(default = "default_page_size")]
    pub default_page_size: usize,

    /// Largest page one request may ask for (the web UI's LOAD MORE and MCP
    /// follow it).
    #[serde(default = "default_max_page_size")]
    pub max_page_size: usize,

    /// Most path matches (FILES box) one request may ask for.
    #[serde(default = "default_max_path_matches")]
    pub max_path_matches: usize,

    /// Longest deadline a request may ask for, in seconds (the server's
    /// `request_timeout_secs` still applies).
    #[serde(default = "default_max_timeout_secs")]
    pub max_timeout_secs: u64,

    /// Most context lines a request may ask for around each hit.
    #[serde(default = "default_max_context_lines")]
    pub max_context_lines: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            fast_rank_files: None,
            max_fast_rank_files: None,
            max_page_offset: None,
            min_match_budget: default_min_match_budget(),
            match_budget_multiplier: default_match_budget_multiplier(),
            max_match_budget: default_max_match_budget(),
            per_file_matches: default_per_file_matches(),
            default_page_size: default_page_size(),
            max_page_size: default_max_page_size(),
            max_path_matches: default_max_path_matches(),
            max_timeout_secs: default_max_timeout_secs(),
            max_context_lines: default_max_context_lines(),
        }
    }
}

fn default_min_match_budget() -> usize {
    crate::search::BudgetRule::DEFAULT.min
}
fn default_match_budget_multiplier() -> usize {
    crate::search::BudgetRule::DEFAULT.multiplier
}
fn default_max_match_budget() -> usize {
    crate::search::BudgetRule::DEFAULT.max
}
fn default_per_file_matches() -> usize {
    crate::search::BudgetRule::DEFAULT.per_file
}
fn default_page_size() -> usize {
    crate::search::SearchSettings::default().default_page_size
}
fn default_max_page_size() -> usize {
    crate::search::SearchSettings::default().max_page_size
}
fn default_max_path_matches() -> usize {
    crate::search::SearchSettings::default().max_path_matches
}
fn default_max_timeout_secs() -> u64 {
    crate::search::SearchSettings::default()
        .max_timeout
        .as_secs()
}
fn default_max_context_lines() -> usize {
    crate::search::SearchSettings::default().max_context_lines
}

/// MCP tool limits (`[mcp]`, for `/mcp` and `fcs mcp`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpConfig {
    /// Hits `search_code` returns when the agent gives no `limit`.
    #[serde(default = "default_mcp_limit")]
    pub default_limit: usize,
    /// Most hits one `search_code` call may ask for (at most
    /// `search.max_page_size`); agents page further with `offset`.
    #[serde(default = "default_mcp_max_limit")]
    pub max_limit: usize,
    /// Files whose path matches, listed above `search_code`'s hits.
    #[serde(default = "default_mcp_file_matches")]
    pub file_matches: usize,
    /// Most context lines `search_code` may ask for around each hit.
    #[serde(default = "default_mcp_max_context_lines")]
    pub max_context_lines: usize,
    /// Lines `read_file` returns when the agent gives no `end_line`.
    #[serde(default = "default_mcp_read_file_lines")]
    pub read_file_lines: usize,
    /// Most lines one `read_file` call returns.
    #[serde(default = "default_mcp_max_read_file_lines")]
    pub max_read_file_lines: usize,
    /// Files the graph tools list per level or group by default.
    #[serde(default = "default_mcp_graph_limit")]
    pub graph_default_limit: usize,
    /// Most files the graph tools may list per level or group.
    #[serde(default = "default_mcp_graph_max_limit")]
    pub graph_max_limit: usize,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            default_limit: default_mcp_limit(),
            max_limit: default_mcp_max_limit(),
            file_matches: default_mcp_file_matches(),
            max_context_lines: default_mcp_max_context_lines(),
            read_file_lines: default_mcp_read_file_lines(),
            max_read_file_lines: default_mcp_max_read_file_lines(),
            graph_default_limit: default_mcp_graph_limit(),
            graph_max_limit: default_mcp_graph_max_limit(),
        }
    }
}

fn default_mcp_limit() -> usize {
    50
}
fn default_mcp_max_limit() -> usize {
    1000
}
fn default_mcp_file_matches() -> usize {
    20
}
fn default_mcp_max_context_lines() -> usize {
    5
}
fn default_mcp_read_file_lines() -> usize {
    200
}
fn default_mcp_max_read_file_lines() -> usize {
    1000
}
fn default_mcp_graph_limit() -> usize {
    50
}
fn default_mcp_graph_max_limit() -> usize {
    500
}

/// Server-related configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    /// Address to bind the gRPC server to
    #[serde(default = "default_address")]
    pub address: String,

    /// Start the gRPC API. When false, `address` is ignored and only the web
    /// UI / REST API is served.
    #[serde(default = "default_true")]
    pub enable_grpc: bool,

    /// Address to bind the HTTP/Web UI server to
    #[serde(default = "default_web_address")]
    pub web_address: String,

    /// Enable the web UI and REST API
    #[serde(default = "default_enable_web_ui")]
    pub enable_web_ui: bool,

    /// Origins allowed to call the REST API cross-origin (CORS).
    ///
    /// Empty (the default) sends no CORS headers, so only same-origin pages
    /// (the embedded web UI) can read API responses. List explicit origins
    /// such as `"http://localhost:3000"`, or `"*"` to allow any origin. The
    /// API serves full source files, so keep this tight.
    #[serde(default)]
    pub cors_origins: Vec<String>,

    /// Maximum searches executing at once on the REST API; further requests
    /// get 503 + Retry-After instead of queueing on the blocking pool.
    #[serde(default = "default_max_concurrent_searches")]
    pub max_concurrent_searches: usize,

    /// Per-request timeout for the REST and gRPC servers, in seconds.
    #[serde(default = "default_request_timeout_secs")]
    pub request_timeout_secs: u64,

    /// Deprecated spelling of `search.fast_rank_files` (still read; the
    /// `[search]` value wins).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast_rank_files: Option<usize>,

    /// Deprecated spelling of `search.max_fast_rank_files`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_fast_rank_files: Option<usize>,

    /// Deprecated spelling of `search.max_page_offset`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_page_offset: Option<usize>,

    /// Serve static UI files from this directory instead of the embedded assets.
    ///
    /// When set, the web server reads HTML/CSS/JS files directly from disk on
    /// every request so that UI changes are visible without recompiling the
    /// server.  Leave unset (the default) for production deployments where the
    /// embedded assets should be used.
    ///
    /// Example: `static_dir = "static"`
    #[serde(default)]
    pub static_dir: Option<String>,

    /// Web UI theme for people who haven't picked one in their browser:
    /// `"bold"` (the default), `"corporate"` or `"corporate-dark"`.
    #[serde(default)]
    pub default_theme: UiTheme,

    /// Show the theme picker on a person's first visit to the web UI. Set
    /// false on a shared instance to use `default_theme` without asking;
    /// people can still change theme from the header.
    #[serde(default = "default_true")]
    pub theme_picker: bool,
}

/// A web UI theme (the token blocks in `static/app.css`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UiTheme {
    /// Yellow and black, square corners, capital labels.
    #[default]
    Bold,
    /// White and grey with one blue accent, rounded corners, sentence case.
    Corporate,
    /// Corporate on dark surfaces.
    CorporateDark,
}

/// Indexer-related configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexerConfig {
    /// Paths to index on startup
    #[serde(default)]
    pub paths: Vec<String>,

    /// File extensions to include (empty means all text files)
    #[serde(default)]
    pub include_extensions: Vec<String>,

    /// Glob-like patterns to exclude (matched as path substrings during discovery)
    #[serde(default = "default_exclude_patterns")]
    pub exclude_patterns: Vec<String>,

    /// Honour `.gitignore` / `.ignore` files under the indexed paths (default true).
    #[serde(default = "default_true")]
    pub respect_gitignore: bool,

    /// Maximum file size to index in bytes (default 10MB)
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,

    /// Path to persistent index storage (if set, index will be saved/loaded)
    #[serde(default)]
    pub index_path: Option<String>,

    /// Enable file watcher for incremental indexing (default true)
    #[serde(default = "default_true")]
    pub watch: bool,

    /// Save index after initial build completes (default: true if index_path is set)
    #[serde(default = "default_true")]
    pub save_after_build: bool,

    /// Save index after N file updates (0 = disabled, off by default)
    /// When enabled, the index is periodically saved after this many files are updated
    #[serde(default)]
    pub save_after_updates: usize,

    /// Save a checkpoint to disk every N files during the initial index build (0 = disabled).
    /// If interrupted before completion, the next run will resume from the checkpoint,
    /// re-indexing only the files that were not yet committed. Recommended value for
    /// very large repos: 20000. Has no effect if `index_path` is not set.
    #[serde(default)]
    pub checkpoint_interval_files: usize,

    /// Exact file paths to permanently exclude from indexing.
    /// Use this to skip files that cause crashes or other issues.
    /// After a crash, check `fcs_last_processed.txt` in the working directory
    /// to identify the offending file, then add its absolute path here.
    ///
    /// Example:
    ///   exclude_files = ["/repo/src/generated/huge_file.rs"]
    #[serde(default)]
    pub exclude_files: Vec<String>,

    /// Enable encoding detection for non-UTF-8 text files (default: true).
    /// When enabled, files in encodings like Latin-1, Shift-JIS, UTF-16 etc.
    /// are automatically transcoded to UTF-8 for indexing.
    /// Disable this if you only work with UTF-8 codebases for slightly faster indexing.
    #[serde(default = "default_true")]
    pub transcode_non_utf8: bool,

    /// Number of files to process per indexing batch (default: 500).
    ///
    /// During initial indexing each batch loads all files into memory in
    /// parallel, so peak RAM scales with `batch_size × average_file_size × ~4`.
    /// Increase this on machines with abundant RAM to reduce engine lock
    /// contention; decrease it on memory-constrained machines.
    /// The v0.6.0 default was 2000 which caused OOM on large codebases.
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Enable tree-sitter symbol extraction (default: true).
    /// When enabled, function/class/struct definitions are parsed from source files
    /// and used to boost search relevance scores for symbol matches.
    /// Disable this to reduce memory usage and indexing time at the cost of
    /// reduced search relevance for symbol-based queries.
    #[serde(default = "default_true")]
    pub enable_symbols: bool,

    /// Start result paths with the indexed folder's own name (default: true).
    /// Indexing `/code/myapp` shows `myapp/src/main.rs`; set this to false to
    /// show `src/main.rs`. Only takes effect when a single path is indexed:
    /// with several, the folder name is what tells their files apart.
    /// Display only: `file:` filters and the saved index are unaffected.
    #[serde(default = "default_true")]
    pub show_root_name: bool,

    /// Load `index_path` but never write it. Not a config key: the server
    /// sets it when another server holds the index lock, so the two cannot
    /// overwrite each other's index.
    #[serde(skip)]
    pub index_read_only: bool,
}

// Both servers bind to loopback by default: there is no authentication, the
// REST API serves full file contents and the gRPC Index RPC indexes paths on
// request. Exposing them on a network is an explicit choice (`0.0.0.0:…`).
fn default_address() -> String {
    "127.0.0.1:50051".to_string()
}

fn default_web_address() -> String {
    "127.0.0.1:8080".to_string()
}

fn default_enable_web_ui() -> bool {
    true
}

fn default_max_concurrent_searches() -> usize {
    64
}

fn default_request_timeout_secs() -> u64 {
    30
}

fn default_exclude_patterns() -> Vec<String> {
    vec![
        "**/node_modules/**".to_string(),
        "**/target/**".to_string(),
        "**/.git/**".to_string(),
        "**/build/**".to_string(),
        "**/dist/**".to_string(),
        "**/__pycache__/**".to_string(),
        "**/venv/**".to_string(),
        "**/.venv/**".to_string(),
    ]
}

fn default_max_file_size() -> u64 {
    10 * 1024 * 1024 // 10MB
}

fn default_batch_size() -> usize {
    500
}

fn default_true() -> bool {
    true
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            address: default_address(),
            enable_grpc: true,
            web_address: default_web_address(),
            enable_web_ui: default_enable_web_ui(),
            cors_origins: Vec::new(),
            max_concurrent_searches: default_max_concurrent_searches(),
            request_timeout_secs: default_request_timeout_secs(),
            fast_rank_files: None,
            max_fast_rank_files: None,
            max_page_offset: None,
            static_dir: None,
            default_theme: UiTheme::default(),
            theme_picker: true,
        }
    }
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            include_extensions: Vec::new(),
            exclude_patterns: default_exclude_patterns(),
            respect_gitignore: true,
            max_file_size: default_max_file_size(),
            index_path: None,
            watch: true,
            save_after_build: true,
            save_after_updates: 0,        // Disabled by default
            checkpoint_interval_files: 0, // Disabled by default
            exclude_files: Vec::new(),
            transcode_non_utf8: true,
            batch_size: default_batch_size(),
            enable_symbols: true,
            show_root_name: true,
            index_read_only: false,
        }
    }
}

impl IndexerConfig {
    /// Generate a fingerprint of the indexer configuration
    /// Used to detect config changes that require re-indexing
    pub fn fingerprint(&self) -> String {
        // Normalize and sort paths for consistent fingerprinting
        let mut sorted_paths: Vec<_> = self
            .paths
            .iter()
            .map(|p| normalize_path_for_comparison(p))
            .collect();
        sorted_paths.sort();

        // Sort extensions
        let mut sorted_exts = self.include_extensions.to_vec();
        sorted_exts.sort();

        // Sort exclude patterns
        let mut sorted_excludes = self.exclude_patterns.to_vec();
        sorted_excludes.sort();

        // Sort excluded files
        let mut sorted_excluded_files = self.exclude_files.to_vec();
        sorted_excluded_files.sort();

        // Create a deterministic string representation
        let config_str = format!(
            "paths:{:?}|exts:{:?}|excludes:{:?}|max_size:{}|exclude_files:{:?}|transcode_non_utf8:{}",
            sorted_paths, sorted_exts, sorted_excludes, self.max_file_size, sorted_excluded_files, self.transcode_non_utf8
        );

        // Generate MD5 hash
        // FxHasher is a documented, deterministic algorithm (unlike
        // DefaultHasher), so the fingerprint is stable across runs and builds.
        use std::hash::Hasher as _;
        let mut h = rustc_hash::FxHasher::default();
        h.write(config_str.as_bytes());
        format!("{:016x}{}", h.finish(), SYMBOL_SCHEMA_TAG)
    }

    /// Check if a file path is explicitly excluded via `exclude_files`.
    pub fn is_file_excluded(&self, path: &std::path::Path) -> bool {
        if self.exclude_files.is_empty() {
            return false;
        }
        let path_str = path.to_string_lossy().replace('\\', "/");
        self.exclude_files.iter().any(|excluded| {
            let excluded_normalized = excluded.replace('\\', "/");
            path_str == excluded_normalized
        })
    }

    /// Canonicalize every configured index path that exists on disk.
    ///
    /// The file store keys files by canonical path; the watcher reports
    /// events under whatever form the root was configured in (relative,
    /// symlinked, `\\?\`-prefixed on Windows). Canonicalizing the roots once
    /// makes exact-path lookups hit on every event instead of falling back to
    /// an O(n) suffix scan. Non-existent paths are left as written so the
    /// usual "path does not exist" warning still names what the user typed.
    pub fn canonicalize_paths(&mut self) {
        for p in &mut self.paths {
            if let Ok(canonical) = std::path::Path::new(p.as_str()).canonicalize() {
                *p = canonical.to_string_lossy().to_string();
            }
        }
    }
}

impl Config {
    /// Load configuration from a file
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let mut config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;
        config.normalize_paths(path.parent());

        Ok(config)
    }

    /// Expand `~` and resolve relative paths in every path-valued setting.
    ///
    /// Relative paths resolve against `base_dir` (the config file's
    /// directory) rather than the process's working directory, so a config
    /// behaves the same no matter where the server is started from.
    /// Extensions in `include_extensions` lose a leading dot (`".rs"` and
    /// `"rs"` both work).
    pub fn normalize_paths(&mut self, base_dir: Option<&Path>) {
        for p in &mut self.indexer.paths {
            *p = expand_path(p, base_dir);
        }
        if let Some(p) = &mut self.indexer.index_path {
            *p = expand_path(p, base_dir);
        }
        if let Some(p) = &mut self.server.static_dir {
            *p = expand_path(p, base_dir);
        }
        if !self.diagnostics.dir.is_empty() {
            self.diagnostics.dir = expand_path(&self.diagnostics.dir, base_dir);
        }
        for e in &mut self.indexer.include_extensions {
            *e = e.trim_start_matches('.').to_string();
        }
    }

    /// Try to load configuration from default locations
    ///
    /// Search order:
    /// 1. FCS_CONFIG environment variable
    /// 2. ./fast_code_search.toml (current directory)
    /// 3. ~/.config/fast_code_search/config.toml (user config)
    pub fn from_default_locations() -> Result<Option<(Self, PathBuf)>> {
        // Check environment variable first
        if let Ok(env_path) = std::env::var("FCS_CONFIG") {
            let path = PathBuf::from(expand_path(&env_path, None));
            // An explicitly named file that is missing is an error, not a
            // silent fall-through to the defaults.
            anyhow::ensure!(
                path.exists(),
                "FCS_CONFIG points to a missing file: {}",
                path.display()
            );
            let config = Self::from_file(&path)?;
            return Ok(Some((config, path)));
        }

        // Check current directory
        let local_path = PathBuf::from("fast_code_search.toml");
        if local_path.exists() {
            let config = Self::from_file(&local_path)?;
            return Ok(Some((config, local_path)));
        }

        // Check user config directory
        if let Some(config_dir) = dirs::config_dir() {
            let user_path = config_dir.join("fast_code_search").join("config.toml");
            if user_path.exists() {
                let config = Self::from_file(&user_path)?;
                return Ok(Some((config, user_path)));
            }
        }

        Ok(None)
    }

    /// Generate a template configuration file
    pub fn generate_template() -> String {
        Self::generate_template_with_index_path("~/.local/share/fast_code_search/index.fcsidx")
    }

    /// The template with `index_path` set to `index_path`.
    pub fn generate_template_with_index_path(index_path: &str) -> String {
        CONFIG_TEMPLATE.replace("__INDEX_PATH__", index_path)
    }

    /// Write template config to the specified path
    pub fn write_template(path: &Path) -> Result<()> {
        // Name the index after the config file so two configs generated
        // side by side do not share (and overwrite) one index.
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("index");
        let template = Self::generate_template_with_index_path(&format!(
            "~/.local/share/fast_code_search/{stem}.fcsidx"
        ));

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory: {}", parent.display())
            })?;
        }

        std::fs::write(path, template)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;

        Ok(())
    }

    /// Merge CLI overrides into the configuration
    pub fn with_overrides(
        mut self,
        address: Option<String>,
        web_address: Option<String>,
        extra_paths: Vec<String>,
    ) -> Self {
        if let Some(addr) = address {
            self.server.address = addr;
        }
        if let Some(addr) = web_address {
            self.server.web_address = addr;
        }

        // Append extra paths from CLI (relative to the working directory,
        // which is what a shell user expects).
        self.indexer
            .paths
            .extend(extra_paths.iter().map(|p| expand_path(p, None)));
        self.indexer.canonicalize_paths();

        self
    }

    /// Check the configuration before anything binds or indexes.
    ///
    /// Hard errors (unparseable addresses, zero limits) are returned; soft
    /// problems (no paths at all, an index directory that
    /// does not exist yet) come back as warnings so the caller can log them
    /// and continue.
    pub fn validate(&self) -> Result<Vec<String>> {
        use std::net::SocketAddr;
        let mut warnings = Vec::new();

        if self.server.enable_grpc {
            self.server.address.parse::<SocketAddr>().with_context(|| {
                format!("server.address is not host:port: {}", self.server.address)
            })?;
        }
        if !self.server.enable_grpc && !self.server.enable_web_ui {
            warnings.push(
                "server.enable_grpc and server.enable_web_ui are both false: \
                 the index is built but nothing can search it"
                    .to_string(),
            );
        }
        if self.server.enable_web_ui {
            self.server
                .web_address
                .parse::<SocketAddr>()
                .with_context(|| {
                    format!(
                        "server.web_address is not host:port: {}",
                        self.server.web_address
                    )
                })?;
            if self.server.enable_grpc && self.server.web_address == self.server.address {
                anyhow::bail!(
                    "server.address and server.web_address are both {}",
                    self.server.address
                );
            }
        }
        if self.server.max_concurrent_searches == 0 {
            anyhow::bail!("server.max_concurrent_searches must be at least 1");
        }
        if self.server.request_timeout_secs == 0 {
            anyhow::bail!("server.request_timeout_secs must be at least 1");
        }
        if self.indexer.batch_size == 0 {
            anyhow::bail!("indexer.batch_size must be at least 1");
        }
        self.validate_limits(&mut warnings)?;
        let d = &self.diagnostics;
        for (key, v) in [
            ("slow_search_secs", d.slow_search_secs),
            ("lock_wait_secs", d.lock_wait_secs),
            ("stall_secs", d.stall_secs),
            ("backlog_secs", d.backlog_secs),
        ] {
            anyhow::ensure!(
                v.is_finite() && v > 0.0,
                "diagnostics.{key} must be a number of seconds above 0 (got {v})"
            );
        }
        anyhow::ensure!(
            d.min_interval_secs.is_finite() && d.min_interval_secs >= 0.0,
            "diagnostics.min_interval_secs must be 0 or more (got {})",
            d.min_interval_secs
        );
        if let Some(index_path) = &self.indexer.index_path {
            let p = Path::new(index_path);
            if let Some(parent) = p.parent() {
                if !parent.as_os_str().is_empty() && !parent.is_dir() {
                    warnings.push(format!(
                        "indexer.index_path directory does not exist yet and will be \
                         created on first save: {}",
                        parent.display()
                    ));
                }
            }
        }
        if self.indexer.paths.is_empty() {
            warnings.push("indexer.paths is empty: nothing will be indexed".to_string());
        }
        for p in &self.indexer.paths {
            if !Path::new(p).exists() {
                warnings.push(format!("indexer.paths entry does not exist (yet): {p}"));
            }
        }
        if self.indexer.save_after_updates > 0 && self.indexer.index_path.is_none() {
            warnings.push(
                "indexer.save_after_updates is set but indexer.index_path is not".to_string(),
            );
        }
        Ok(warnings)
    }

    /// `[search]` and `[mcp]` values that contradict each other are errors
    /// naming the key, not silent clamps.
    fn validate_limits(&self, warnings: &mut Vec<String>) -> Result<()> {
        let (s, m) = (&self.search, &self.mcp);
        for (key, v) in [
            ("search.min_match_budget", s.min_match_budget),
            ("search.match_budget_multiplier", s.match_budget_multiplier),
            ("search.max_match_budget", s.max_match_budget),
            ("search.per_file_matches", s.per_file_matches),
            ("search.default_page_size", s.default_page_size),
            ("search.max_page_size", s.max_page_size),
            ("mcp.default_limit", m.default_limit),
            ("mcp.max_limit", m.max_limit),
            ("mcp.read_file_lines", m.read_file_lines),
            ("mcp.max_read_file_lines", m.max_read_file_lines),
            ("mcp.graph_default_limit", m.graph_default_limit),
            ("mcp.graph_max_limit", m.graph_max_limit),
        ] {
            anyhow::ensure!(v >= 1, "{key} must be at least 1");
        }
        if s.max_timeout_secs == 0 {
            anyhow::bail!("search.max_timeout_secs must be at least 1");
        }
        let pairs = [
            (
                "search.min_match_budget",
                s.min_match_budget,
                "search.max_match_budget",
                s.max_match_budget,
            ),
            (
                "search.default_page_size",
                s.default_page_size,
                "search.max_page_size",
                s.max_page_size,
            ),
            (
                "mcp.default_limit",
                m.default_limit,
                "mcp.max_limit",
                m.max_limit,
            ),
            (
                "mcp.max_limit",
                m.max_limit,
                "search.max_page_size",
                s.max_page_size,
            ),
            (
                "mcp.read_file_lines",
                m.read_file_lines,
                "mcp.max_read_file_lines",
                m.max_read_file_lines,
            ),
            (
                "mcp.graph_default_limit",
                m.graph_default_limit,
                "mcp.graph_max_limit",
                m.graph_max_limit,
            ),
            (
                "mcp.file_matches",
                m.file_matches,
                "search.max_path_matches",
                s.max_path_matches,
            ),
        ];
        for (a, av, b, bv) in pairs {
            anyhow::ensure!(av <= bv, "{a} ({av}) must not exceed {b} ({bv})");
        }
        for (key, set) in [
            ("fast_rank_files", self.server.fast_rank_files.is_some()),
            (
                "max_fast_rank_files",
                self.server.max_fast_rank_files.is_some(),
            ),
            ("max_page_offset", self.server.max_page_offset.is_some()),
        ] {
            if set {
                warnings.push(format!(
                    "server.{key} has moved to search.{key}; the [server] value is still used \
                     unless [search] sets one"
                ));
            }
        }
        Ok(())
    }
}

/// What `--init` writes: every key with its default and an explanation.
/// `__INDEX_PATH__` is filled in by [`Config::generate_template_with_index_path`].
const CONFIG_TEMPLATE: &str = r#"# Fast Code Search Configuration
# Generated template - customize as needed

[server]
# Address to bind the gRPC server to.
# Loopback by default: the server has no authentication and serves full file
# contents. Use "0.0.0.0:50051" only on a trusted network.
address = "127.0.0.1:50051"

# Start the gRPC API (default: true). Set false if you only use the web UI,
# REST API and fcs CLI. If the gRPC port is busy at startup the server logs
# an error and keeps running without gRPC rather than exiting.
enable_grpc = true

# Address to bind the HTTP/Web UI server to (same caveat as above)
web_address = "127.0.0.1:8080"

# Enable the web UI and REST API (default: true)
enable_web_ui = true

# Origins allowed to call the REST API from another site (CORS).
# Empty (default) = same-origin only, which is all the embedded UI needs.
# Example: ["http://localhost:3000"], or ["*"] to allow any origin
cors_origins = []

# Searches executing at once on the REST API; extra requests get 503 + Retry-After
max_concurrent_searches = 64

# Per-request timeout (seconds) for both servers
request_timeout_secs = 30

# Serve static UI files from a directory on disk instead of embedded assets.
# When set, the web server reads HTML/CSS/JS files from this path on every
# request so that UI edits are visible without recompiling the server.
# Useful during development. Unset (the default) = the assets embedded in the
# binary; TOML has no "unset" value, so this one key stays commented out.
# static_dir = "static"

# Web UI theme for people who haven't picked one: "bold", "corporate" or
# "corporate-dark". Each browser remembers its own choice.
default_theme = "bold"

# Ask people to pick a theme on their first visit. Set false on a shared
# server to use default_theme without asking (they can still change it from
# the palette button in the header).
theme_picker = true

[search]
# Search limits for every front end (web UI, REST, gRPC, fcs, MCP). The
# defaults were measured on a 372k-file corpus; see the "Limits" page of the
# book. Every limit that cuts a search is reported with the result.

# A search reads candidate files best first (by a per-file score that needs
# no reads) until it has collected enough matches to rank. This caps how
# many files it reads unless the request asks for more. 0 = no cap.
fast_rank_files = 0

# Most files a request may ask for (`max_files` on /api/search, the web UI's
# SEARCH MORE FILES). 0 = no limit; set one on a shared server.
max_fast_rank_files = 0

# Matches collected to rank a page. Every page ending before result 1000
# collects multiplier x 1000, between min and max; every page from there on
# collects max. One budget per range keeps offset pages consistent. A page
# is the best of what was collected, so a small budget gives a poor page 1
# for common words. Each match costs about 230 bytes while the search runs.
min_match_budget = 32768
match_budget_multiplier = 8
max_match_budget = 100000

# Most hits one file contributes to pages before result 1000 (its best, by
# score). 1000 leaves out nothing those pages could show.
per_file_matches = 1000

# Page size when a request gives none, and the largest it may ask for (the
# web UI's LOAD MORE and MCP follow max_page_size).
default_page_size = 50
max_page_size = 1000

# Deepest result offset a client may page to. Deep pages cost a scan of that
# many matches. 0 = no limit.
max_page_offset = 100000

# Most path matches (the FILES box) one request may ask for.
max_path_matches = 100

# Longest deadline a request may ask for, in seconds (request_timeout_secs
# still bounds every search).
max_timeout_secs = 30

# Most context lines around each hit.
max_context_lines = 10

[mcp]
# Hits search_code returns by default, and the most one call may ask for
# (at most search.max_page_size; agents page further with `offset`).
default_limit = 50
max_limit = 1000

# Files whose path matches the query, listed above search_code's hits.
file_matches = 20

# Most context lines search_code may ask for.
max_context_lines = 5

# read_file: lines returned by default, and the most one call returns.
read_file_lines = 200
max_read_file_lines = 1000

# Graph tools: files listed per level or group by default, and at most.
graph_default_limit = 50
graph_max_limit = 500

[indexer]
# Paths to index on startup
# Add your project directories here
paths = [
    # "C:/code/my-project",
    # "C:/code/another-project",
    # "/home/user/projects/my-app",
]

# Honour .gitignore / .ignore files under the indexed paths (default: true)
respect_gitignore = true

# File extensions to include (empty = all text files)
# Example: ["rs", "py", "js", "ts", "go", "c", "cpp", "h", "java"]
include_extensions = []

# Patterns to exclude from indexing
exclude_patterns = [
    "**/node_modules/**",
    "**/target/**",
    "**/.git/**",
    "**/build/**",
    "**/dist/**",
    "**/__pycache__/**",
    "**/venv/**",
    "**/.venv/**",
]

# Exact file paths to permanently exclude from indexing.
# Useful for files that cause crashes or are too large/noisy to be useful.
# After a crash, check fcs_last_processed.txt to identify the offending file.
# Example: ["/repo/src/generated/huge_file.rs"]
exclude_files = []

# Maximum file size to index in bytes (default: 10MB)
max_file_size = 10485760

# Enable encoding detection for non-UTF-8 text files (default: true)
# When enabled, files in Latin-1, Shift-JIS, UTF-16 etc. are transcoded to UTF-8.
# Disable for UTF-8-only codebases for slightly faster indexing.
transcode_non_utf8 = true

# Path to persistent index storage.
# The index is saved here and loaded on restart, so a restart takes seconds
# and only changed files are re-read. Remove the line to keep the index in
# memory only: it is then rebuilt from scratch on every start, and the server
# logs a warning saying so. Use one file per config. The directory is created
# on first save; `~` is expanded and relative paths resolve against this file.
index_path = "__INDEX_PATH__"

# Save index after initial build completes (default: true)
# Only effective when index_path is set
save_after_build = true

# Save index after N file updates via watcher (default: 0 = disabled)
# When enabled, the index is periodically saved after this many watcher-triggered updates.
# Useful for long-running servers to persist incremental changes.
save_after_updates = 0

# Checkpoint every N files during the initial index build (default: 0 = disabled)
# If the process is killed mid-build, the next run resumes from the checkpoint.
# Recommended for very large repos: 20000. Has no effect if index_path is not set.
checkpoint_interval_files = 0

# Enable file watcher for incremental indexing (default: true)
# Changes to indexed files are detected and re-indexed automatically.
# Set to false to index once at startup (search then misses later edits
# until a restart; responses say how old the index is).
watch = true

# Enable tree-sitter symbol extraction (default: true)
# When enabled, function/class/struct definitions are parsed from source files
# and used to boost search relevance for symbol matches.
# Disable to reduce memory usage and indexing time at the cost of reduced relevance.
enable_symbols = true

# Start result paths with the indexed folder's name (default: true).
# Indexing /code/myapp shows myapp/src/main.rs; false shows src/main.rs.
# Only applies when a single path is indexed. Display only: file: filters
# and the saved index are unaffected.
show_root_name = true

# Files read and indexed per batch during the initial build (default: 500).
# Peak RAM scales with batch_size x average file size x ~4: lower it on small
# machines, raise it on large ones to reduce lock contention.
batch_size = 500

[telemetry]
# Enable OpenTelemetry trace export (default: false)
# Set to true to enable OTLP export (console logging is always active)
# Env overrides: OTEL_SDK_DISABLED=true, FCS_TRACING_ENABLED=true
enabled = false

# OTLP gRPC exporter endpoint (default: http://localhost:4317)
# Env override: OTEL_EXPORTER_OTLP_ENDPOINT
otlp_endpoint = "http://localhost:4317"

# Service name reported to the collector
# Env override: OTEL_SERVICE_NAME
service_name = "fast_code_search"

[diagnostics]
# Self-diagnosis: when the server is slow or stuck, write what it was doing
# to a JSON file you can look at later with `fcs diagnose`. Nothing leaves
# the machine. Off by default.
capture = false

# Where captures go. Empty: a `diagnostics` folder next to index_path.
dir = ""

# What counts as slow or stuck, in seconds. The defaults sit well above what
# was measured on a healthy server under heavy load (see the book's
# "Diagnostic captures" page).
slow_search_secs = 5.0
lock_wait_secs = 2.0
stall_secs = 120.0
backlog_secs = 300.0

# Seconds before the same kind of capture is written again, and how many
# captures (and megabytes) are kept; the oldest are deleted.
min_interval_secs = 300.0
max_files = 50
max_total_mb = 100

# Log lines kept in each capture.
log_lines = 300

# On a hang, record every thread's stack with gdb or eu-stack if installed
# (pauses the server for a second or two).
thread_stacks = true

# Keep query text, file paths and quoted text in captures. Off: captures say
# how long a query was, not what it was.
include_queries = false
"#;

/// Version tag of the symbol/reference extraction (tags queries, reference
/// kinds), appended to the configuration fingerprint. A persisted index
/// whose fingerprint lacks the current tag has its symbols and references
/// re-extracted from file content on load instead of restored, so a change
/// to what is captured (say, TypeScript call sites) reaches an existing
/// index without a full rebuild. Bump when extraction changes.
pub const SYMBOL_SCHEMA_TAG: &str = "|sym5";

/// Whether an index saved with `fingerprint` used the current extraction.
pub fn symbols_schema_current(fingerprint: &str) -> bool {
    fingerprint.ends_with(SYMBOL_SCHEMA_TAG)
}

/// Expand a leading `~` to the home directory and, when `base_dir` is given,
/// resolve a relative path against it. Paths that do not exist are left as
/// written (after expansion) so error messages still name what the user typed.
pub fn expand_path(raw: &str, base_dir: Option<&Path>) -> String {
    let expanded: PathBuf = if raw == "~" {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from(raw))
    } else if let Some(rest) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\")) {
        match dirs::home_dir() {
            Some(home) => home.join(rest),
            None => PathBuf::from(raw),
        }
    } else {
        PathBuf::from(raw)
    };
    let resolved = match base_dir {
        Some(base) if expanded.is_relative() && !base.as_os_str().is_empty() => {
            base.join(&expanded)
        }
        _ => expanded,
    };
    resolved.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Roadmap 4.5: a misspelled key is an error, not a silently ignored
    /// default; the generated template must parse; validate() catches bad
    /// addresses and zero limits and warns about missing paths.
    #[test]
    fn test_unknown_key_rejected_and_template_parses() {
        let err = toml::from_str::<Config>("[indexer]\nexlude_patterns = [\"x\"]\n")
            .expect_err("typo must be rejected");
        assert!(err.to_string().contains("exlude_patterns"), "{err}");

        let template = Config::generate_template();
        let cfg: Config = toml::from_str(&template).expect("template must parse");
        assert_eq!(cfg.server.address, "127.0.0.1:50051");
    }

    #[test]
    fn test_show_root_name_defaults_on() {
        let cfg: Config = toml::from_str("[indexer]\n").unwrap();
        assert!(cfg.indexer.show_root_name);
        let cfg: Config = toml::from_str("[indexer]\nshow_root_name = false\n").unwrap();
        assert!(!cfg.indexer.show_root_name);
    }

    #[test]
    fn test_validate_reports_errors_and_warnings() {
        let mut cfg = Config::default();
        let warnings = cfg.validate().unwrap();
        assert!(warnings
            .iter()
            .any(|w| w.contains("indexer.paths is empty")));

        cfg.server.address = "not-an-address".to_string();
        assert!(cfg.validate().is_err());
        cfg.server.address = "127.0.0.1:1".to_string();
        cfg.server.web_address = "127.0.0.1:1".to_string();
        assert!(cfg.validate().is_err(), "same port twice");
        cfg.server.web_address = "127.0.0.1:2".to_string();
        cfg.indexer.batch_size = 0;
        assert!(cfg.validate().is_err());
        cfg.indexer.batch_size = 10;
        cfg.indexer.index_path = Some("/definitely/not/here/index.bin".to_string());
        let warnings = cfg.validate().unwrap();
        assert!(warnings.iter().any(|w| w.contains("created on first save")));
        cfg.indexer.index_path = None;
        cfg.indexer.paths = vec!["/no/such/dir".to_string()];
        let warnings = cfg.validate().unwrap();
        assert!(warnings.iter().any(|w| w.contains("does not exist")));
    }

    #[test]
    fn test_validate_grpc_disabled() {
        let mut cfg = Config::default();
        cfg.server.enable_grpc = false;
        cfg.server.address = "not-an-address".to_string();
        assert!(cfg.validate().is_ok(), "address is ignored without gRPC");
        cfg.server.address = cfg.server.web_address.clone();
        assert!(cfg.validate().is_ok(), "no port clash without gRPC");
        cfg.server.enable_web_ui = false;
        let warnings = cfg.validate().unwrap();
        assert!(warnings.iter().any(|w| w.contains("nothing can search it")));
    }

    #[test]
    fn test_with_overrides_sets_web_address() {
        let cfg = Config::default().with_overrides(
            Some("127.0.0.1:9000".to_string()),
            Some("127.0.0.1:9001".to_string()),
            vec![],
        );
        assert_eq!(cfg.server.address, "127.0.0.1:9000");
        assert_eq!(cfg.server.web_address, "127.0.0.1:9001");
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.server.address, "127.0.0.1:50051");
        assert!(config.indexer.paths.is_empty());
        assert!(!config.indexer.exclude_patterns.is_empty());
    }

    #[test]
    fn test_parse_minimal_config() {
        let toml = r#"
[server]
address = "127.0.0.1:8080"

[indexer]
paths = ["/code/project"]
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.server.address, "127.0.0.1:8080");
        assert_eq!(config.indexer.paths, vec!["/code/project"]);
    }

    #[test]
    fn test_ui_theme_names() {
        let toml = "[server]\ndefault_theme = \"corporate-dark\"\ntheme_picker = false\n";
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.server.default_theme, UiTheme::CorporateDark);
        assert!(!config.server.theme_picker);
        assert!(toml::from_str::<Config>("[server]\ndefault_theme = \"dark\"\n").is_err());
        let defaults = Config::default();
        assert_eq!(defaults.server.default_theme, UiTheme::Bold);
        assert!(defaults.server.theme_picker);
    }

    #[test]
    fn test_generate_template() {
        let template = Config::generate_template();
        assert!(template.contains("[server]"));
        assert!(template.contains("[indexer]"));
        assert!(template.contains("paths"));
    }

    /// `--init` must scaffold every key, so a new option cannot be added to
    /// the structs without also being documented in the template.
    #[test]
    fn test_template_lists_every_key() {
        let template = Config::generate_template();
        let mut full = Config::default();
        full.indexer.index_path = Some(String::new());
        full.server.static_dir = Some(String::new());
        let value = toml::Value::try_from(&full).unwrap();
        for (section, table) in value.as_table().unwrap() {
            assert!(template.contains(&format!("[{section}]")), "[{section}]");
            for key in table.as_table().unwrap().keys() {
                // static_dir has no TOML "unset" value, so it is the one key
                // that is documented commented out.
                let line = if key == "static_dir" {
                    format!("\n# {key} = ")
                } else {
                    format!("\n{key} = ")
                };
                assert!(
                    template.contains(&line),
                    "template is missing {section}.{key}"
                );
            }
        }
        let parsed: Config = toml::from_str(&template).unwrap();
        assert_eq!(
            parsed.indexer.index_path.as_deref(),
            Some("~/.local/share/fast_code_search/index.fcsidx")
        );
    }

    /// `[search]` and `[mcp]` set the limits; the old `[server]` spellings
    /// still work (with a warning) unless `[search]` sets the same key.
    #[test]
    fn test_search_limits_from_config() {
        use crate::search::SearchSettings;
        let d = SearchSettings::from_config(&Config::default());
        assert_eq!(d, SearchSettings::default());
        assert_eq!(d.fast_rank_files, usize::MAX);
        assert_eq!(d.budget.min, 32_768);

        let c: Config = toml::from_str(
            "[server]\nfast_rank_files = 3000\nmax_page_offset = 500\n\
             [search]\nmax_page_offset = 900\nmin_match_budget = 4096\nmax_page_size = 2000\n\
             default_page_size = 25\n[mcp]\nmax_limit = 2000\n",
        )
        .unwrap();
        let s = SearchSettings::from_config(&c);
        assert_eq!(s.fast_rank_files, 3000, "legacy [server] key still read");
        assert_eq!(s.max_offset, 900, "[search] wins over [server]");
        assert_eq!(s.budget.min, 4096);
        assert_eq!(s.max_page_size, 2000);
        assert_eq!(s.page_size(0), 25);
        assert_eq!(c.mcp.max_limit, 2000);
        let warnings = c.validate().unwrap();
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("server.fast_rank_files")),
            "{warnings:?}"
        );
        assert!(warnings
            .iter()
            .any(|w| w.contains("server.max_page_offset")));

        for (bad, needle) in [
            (
                "[search]\nmin_match_budget = 0\n",
                "search.min_match_budget",
            ),
            (
                "[search]\nmin_match_budget = 9000\nmax_match_budget = 100\n",
                "search.max_match_budget",
            ),
            (
                "[search]\ndefault_page_size = 5000\n",
                "search.max_page_size",
            ),
            ("[mcp]\nmax_limit = 5000\n", "search.max_page_size"),
            (
                "[mcp]\ndefault_limit = 600\nmax_limit = 500\n",
                "mcp.max_limit",
            ),
        ] {
            let c: Config = toml::from_str(bad).unwrap();
            let err = c.validate().unwrap_err().to_string();
            assert!(err.contains(needle), "{bad}: {err}");
        }
    }

    #[test]
    fn test_write_template_names_index_after_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("work.toml");
        Config::write_template(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("index_path = \"~/.local/share/fast_code_search/work.fcsidx\""));
    }
}

#[cfg(test)]
mod path_tests {
    use super::*;

    #[test]
    fn test_fingerprint_carries_the_symbol_schema_tag() {
        let fp = IndexerConfig::default().fingerprint();
        assert!(symbols_schema_current(&fp), "{fp}");
        assert!(!symbols_schema_current("0123456789abcdef"));
        assert!(!symbols_schema_current("0123456789abcdef|sym1"));
    }

    #[test]
    fn test_expand_path_tilde_and_relative() {
        let home = dirs::home_dir().expect("home dir");
        assert_eq!(
            expand_path("~/code", None),
            home.join("code").to_string_lossy()
        );
        assert_eq!(expand_path("~", None), home.to_string_lossy());

        let base = Path::new("/etc/fcs");
        assert_eq!(
            expand_path("repos", Some(base)),
            base.join("repos").to_string_lossy()
        );
        assert_eq!(expand_path("/abs/repos", Some(base)), "/abs/repos");
        assert_eq!(
            expand_path("~/x", Some(base)),
            home.join("x").to_string_lossy()
        );
        assert_eq!(expand_path("plain", None), "plain");
    }

    #[test]
    fn test_from_file_resolves_paths_against_config_dir_and_strips_dots() {
        let temp = tempfile::TempDir::new().unwrap();
        let cfg_path = temp.path().join("fcs.toml");
        std::fs::write(
            &cfg_path,
            r#"
[indexer]
paths = ["repos", "~/elsewhere"]
index_path = "cache/index.bin"
include_extensions = [".rs", "py"]
"#,
        )
        .unwrap();
        let config = Config::from_file(&cfg_path).unwrap();
        assert_eq!(
            config.indexer.paths[0],
            temp.path().join("repos").to_string_lossy()
        );
        assert!(config.indexer.paths[1]
            .starts_with(dirs::home_dir().unwrap().to_string_lossy().as_ref()));
        assert_eq!(
            config.indexer.index_path.as_deref(),
            Some(
                temp.path()
                    .join("cache/index.bin")
                    .to_string_lossy()
                    .as_ref()
            )
        );
        assert_eq!(config.indexer.include_extensions, vec!["rs", "py"]);
    }
}
