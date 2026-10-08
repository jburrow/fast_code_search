//! `/mcp`: the index as a Model Context Protocol server, so a coding agent
//! can search, read and walk the import graph through one read-only
//! interface.
//!
//! Transport is MCP's streamable HTTP in its simplest form: the client
//! POSTs a JSON-RPC message and gets the JSON response back in the body
//! (no server-initiated messages, so no SSE stream and no session to keep).
//! `fcs mcp` bridges stdio-only clients to this endpoint.
//!
//! The tools call the same handlers as the REST API, so they share its
//! search permits, deadlines and limits. Their output is plain text laid
//! out for a model to read: root-relative paths, `path:line` citations,
//! sorted lists capped at a limit with a note saying what was left out.

use super::api::{self, try_read_engine, ApiError, ApiQuery};
use super::graph::{self, lookup};
use super::WebState;
use crate::dependencies::graph::{is_test_path, Direction};
use axum::{
    body::Bytes,
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Protocol revisions this server speaks, newest first. A client asking for
/// another one is answered with the newest (it may then disconnect).
const PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

const DEFAULT_LIMIT: usize = 50;
const MAX_LIMIT: usize = 500;
const MAX_SEARCH_LIMIT: usize = 100;
const DEFAULT_READ_LINES: usize = 200;
const MAX_READ_LINES: usize = 1000;

/// What every graph tool says about coverage, so an empty answer for a Go
/// file is not read as "nothing uses it".
const GRAPH_COVERAGE: &str = "Imports are resolved for Rust, Python and JavaScript/TypeScript \
     files only; other languages have no edges.";

const INSTRUCTIONS: &str = "fast_code_search keeps a trigram index and a file-level import graph \
     of the codebase in memory, so these tools answer in milliseconds. Prefer search_code over \
     grep, read_file to read source with line numbers, file_dependencies and change_impact \
     before changing a file other code relies on, import_path to see how two files are \
     connected, and dependency_overview to get oriented in an unfamiliar repository. Paths are \
     root-relative (`<root>/src/main.rs`), as every tool returns them. All tools are read-only.";

// ------------------------------------------------------------------ transport

/// `GET /mcp`: no server-to-client stream is offered.
pub async fn mcp_get() -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        [(header::ALLOW, "POST")],
        "This MCP endpoint takes JSON-RPC messages by POST and does not open an event stream.",
    )
        .into_response()
}

/// `POST /mcp`: one JSON-RPC message (or a batch) in, its response out.
pub async fn mcp_post(State(state): State<WebState>, headers: HeaderMap, body: Bytes) -> Response {
    if !origin_allowed(&headers, &state.cors_origins) {
        // The spec's DNS-rebinding guard: a web page must not be able to
        // drive a local server through the visitor's browser.
        return (StatusCode::FORBIDDEN, "Origin not allowed").into_response();
    }
    let message: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(rpc_error(Value::Null, -32700, &format!("Parse error: {e}"))),
            )
                .into_response()
        }
    };
    let reply = match message {
        Value::Array(batch) => {
            let mut out = Vec::new();
            for m in batch {
                if let Some(r) = handle(&state, m).await {
                    out.push(r);
                }
            }
            (!out.is_empty()).then_some(Value::Array(out))
        }
        m => handle(&state, m).await,
    };
    match reply {
        Some(r) => Json(r).into_response(),
        // Notifications and responses get no body.
        None => StatusCode::ACCEPTED.into_response(),
    }
}

/// No `Origin` (a non-browser client), a loopback origin, the server's own
/// origin, or one listed in `cors_origins`.
fn origin_allowed(headers: &HeaderMap, cors_origins: &[String]) -> bool {
    let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else {
        return true;
    };
    if cors_origins.iter().any(|o| o == "*" || o == origin) {
        return true;
    }
    let host = origin
        .split_once("://")
        .map_or(origin, |(_, rest)| rest)
        .trim_end_matches('/');
    let same_host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|h| h.eq_ignore_ascii_case(host));
    let name = if host.starts_with('[') {
        host.split_once(']')
            .map_or(host, |(n, _)| n)
            .trim_start_matches('[')
    } else {
        host.rsplit_once(':').map_or(host, |(n, _)| n)
    };
    same_host || matches!(name, "localhost" | "127.0.0.1" | "::1")
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn rpc_result(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

/// Answer one message; `None` for notifications and stray responses.
async fn handle(state: &WebState, message: Value) -> Option<Value> {
    let Value::Object(mut msg) = message else {
        return Some(rpc_error(Value::Null, -32600, "Invalid request"));
    };
    let id = msg.remove("id");
    let Some(method) = msg
        .get("method")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        if msg.contains_key("result") || msg.contains_key("error") {
            return None; // a response; this server never sends requests
        }
        return Some(rpc_error(
            id.unwrap_or(Value::Null),
            -32600,
            "Invalid request: no method",
        ));
    };
    let id = id?; // a notification (initialized, cancelled, …): nothing to say
    let params = msg.remove("params").unwrap_or(Value::Null);
    Some(match method.as_str() {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str);
            let version = asked
                .filter(|v| PROTOCOL_VERSIONS.contains(v))
                .unwrap_or(PROTOCOL_VERSIONS[0]);
            rpc_result(
                id,
                json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {"listChanged": false}},
                    "serverInfo": {
                        "name": "fast_code_search",
                        "title": "fast_code_search",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                    "instructions": INSTRUCTIONS,
                }),
            )
        }
        "ping" => rpc_result(id, json!({})),
        "tools/list" => rpc_result(id, json!({ "tools": tool_definitions() })),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| Value::Object(Map::new()));
            match call_tool(state, name, args).await {
                Ok(text) => rpc_result(
                    id,
                    json!({"content": [{"type": "text", "text": text}], "isError": false}),
                ),
                Err(ToolError::Unknown) => rpc_error(id, -32602, &format!("Unknown tool: {name}")),
                // Failures the model can act on (bad path, bad regex, busy
                // server) come back as a tool result, per the spec.
                Err(ToolError::Failed(text)) => rpc_result(
                    id,
                    json!({"content": [{"type": "text", "text": text}], "isError": true}),
                ),
            }
        }
        _ => rpc_error(id, -32601, &format!("Method not found: {method}")),
    })
}

// --------------------------------------------------------------------- tools

enum ToolError {
    Unknown,
    Failed(String),
}

impl From<ApiError> for ToolError {
    fn from(e: ApiError) -> Self {
        ToolError::Failed(e.message)
    }
}

impl From<String> for ToolError {
    fn from(e: String) -> Self {
        ToolError::Failed(e)
    }
}

type ToolResult = Result<String, ToolError>;

fn tool_definitions() -> Value {
    let read_only = json!({"readOnlyHint": true, "openWorldHint": false});
    let tool = |name: &str, title: &str, description: String, schema: Value| {
        json!({
            "name": name,
            "title": title,
            "description": description,
            "inputSchema": schema,
            "annotations": read_only,
        })
    };
    json!([
        tool(
            "search_code",
            "Search code",
            "Search the indexed code. Use instead of grep: it answers from an in-memory index. \
             `query` takes the query syntax: several words must all appear in a file (lines \
             holding the whole phrase rank first), \"exact phrase\", -term, file:PATTERN, \
             lang:rust, case:yes, word:yes. mode: text (default), regex (a Rust regex matched \
             per line), symbols (definitions only) or references (call sites and type mentions \
             of an identifier). Returns path:line:column with the matching line."
                .to_string(),
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "What to search for."},
                    "mode": {"type": "string", "enum": ["text", "regex", "symbols", "references"], "default": "text"},
                    "include": {"type": "string", "description": "Semicolon-separated path globs to search within, e.g. src/**/*.rs;lib/**"},
                    "exclude": {"type": "string", "description": "Semicolon-separated path globs to skip."},
                    "limit": {"type": "integer", "minimum": 1, "maximum": MAX_SEARCH_LIMIT, "default": 20},
                    "context": {"type": "integer", "minimum": 0, "maximum": 5, "default": 0, "description": "Lines of context around each match."}
                },
                "required": ["query"]
            }),
        ),
        tool(
            "read_file",
            "Read file",
            format!(
                "Read an indexed file with line numbers, so you can cite path:line. Reads \
                 {DEFAULT_READ_LINES} lines from start_line by default, at most {MAX_READ_LINES}."
            ),
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Path as search results print it (a unique suffix such as src/main.rs also works)."},
                    "start_line": {"type": "integer", "minimum": 1, "default": 1},
                    "end_line": {"type": "integer", "minimum": 1, "description": "Last line to read (inclusive)."}
                },
                "required": ["path"]
            }),
        ),
        tool(
            "file_dependencies",
            "File dependencies",
            format!(
                "The files a file imports (out) and the files that import it (in), up to \
                 `depth` imports away, with the line of each direct import. Call it before \
                 changing a file's interface, or to find where something is used at file level. \
                 {GRAPH_COVERAGE}"
            ),
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "direction": {"type": "string", "enum": ["in", "out", "both"], "default": "both"},
                    "depth": {"type": "integer", "minimum": 1, "maximum": 4, "default": 1},
                    "limit": {"type": "integer", "minimum": 1, "maximum": MAX_LIMIT, "default": DEFAULT_LIMIT, "description": "Files listed per level."}
                },
                "required": ["path"]
            }),
        ),
        tool(
            "change_impact",
            "Change impact",
            format!(
                "Every file that directly or transitively imports any of `paths`: what a \
                 change to them can break, nearest first, split into tests and other code. \
                 Call it before editing a widely used file to know which tests to run. \
                 {GRAPH_COVERAGE}"
            ),
            json!({
                "type": "object",
                "properties": {
                    "paths": {"type": "array", "items": {"type": "string"}, "minItems": 1},
                    "limit": {"type": "integer", "minimum": 1, "maximum": MAX_LIMIT, "default": DEFAULT_LIMIT, "description": "Files listed in each group."}
                },
                "required": ["paths"]
            }),
        ),
        tool(
            "import_path",
            "Import path",
            format!(
                "The shortest chain of imports from one file to another (or the other way \
                 round when there is none that way): how two parts of the code are connected. \
                 {GRAPH_COVERAGE}"
            ),
            json!({
                "type": "object",
                "properties": {
                    "from": {"type": "string"},
                    "to": {"type": "string"}
                },
                "required": ["from", "to"]
            }),
        ),
        tool(
            "dependency_overview",
            "Dependency overview",
            format!(
                "Orientation in an unfamiliar codebase: the most connected folders and how \
                 they import each other, import cycles between folders, and the hub files \
                 most of the code depends on. Optionally limited to folders under `directory`. \
                 {GRAPH_COVERAGE}"
            ),
            json!({
                "type": "object",
                "properties": {
                    "directory": {"type": "string", "description": "Only folders whose path contains this, e.g. src/search"},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 200, "default": 30}
                }
            }),
        ),
    ])
}

async fn call_tool(state: &WebState, name: &str, args: Value) -> ToolResult {
    match name {
        "search_code" => search_code(state, parse(args)?).await,
        "read_file" => read_file(state, parse(args)?).await,
        "file_dependencies" => file_dependencies(state, parse(args)?).await,
        "change_impact" => change_impact(state, parse(args)?).await,
        "import_path" => import_path(state, parse(args)?).await,
        "dependency_overview" => dependency_overview(state, parse(args)?).await,
        _ => Err(ToolError::Unknown),
    }
}

fn parse<T: DeserializeOwned>(args: Value) -> Result<T, ToolError> {
    serde_json::from_value(args).map_err(|e| ToolError::Failed(format!("Invalid arguments: {e}")))
}

/// Build a REST handler's query struct from JSON (they are all `Deserialize`).
fn query<T: DeserializeOwned>(v: Value) -> Result<ApiQuery<T>, ToolError> {
    serde_json::from_value(v)
        .map(ApiQuery)
        .map_err(|e| ToolError::Failed(format!("Invalid arguments: {e}")))
}

fn limit(asked: Option<usize>, default: usize, max: usize) -> usize {
    asked.unwrap_or(default).clamp(1, max)
}

// ------------------------------------------------------------- search_code

#[derive(serde::Deserialize)]
struct SearchArgs {
    query: String,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    include: String,
    #[serde(default)]
    exclude: String,
    limit: Option<usize>,
    #[serde(default)]
    context: usize,
}

async fn search_code(state: &WebState, a: SearchArgs) -> ToolResult {
    if a.query.trim().is_empty() {
        return Err("query is empty".to_string().into());
    }
    let mode = a.mode.as_deref().unwrap_or("text");
    let (regex, symbols, references) = match mode {
        "text" => (false, false, false),
        "regex" => (true, false, false),
        "symbols" => (false, true, false),
        "references" => (false, false, true),
        other => {
            return Err(
                format!("Unknown mode {other:?}: use text, regex, symbols or references").into(),
            )
        }
    };
    let context = a.context.min(5);
    let q = query(json!({
        "q": a.query,
        "max": limit(a.limit, 20, MAX_SEARCH_LIMIT),
        "include": a.include,
        "exclude": a.exclude,
        "regex": regex,
        "symbols": symbols,
        "references": references,
        "context": context,
    }))?;
    let Json(resp) = api::search_handler(State(state.clone()), q).await?;
    let mut out = String::new();
    if resp.results.is_empty() {
        let _ = writeln!(out, "No matches for {:?} ({mode} search).", resp.query);
        return Ok(out);
    }
    let total = match resp.total_matches {
        Some(n) => format!("{n} matches"),
        None => "Many matches (the scan stopped early)".to_string(),
    };
    let _ = writeln!(
        out,
        "{total}; showing {}{}.",
        resp.results.len(),
        if resp.has_more {
            ", more exist: narrow the query or raise limit"
        } else {
            ""
        }
    );
    for r in &resp.results {
        let kind = match r.match_type.as_ref() {
            "SYMBOL_DEFINITION" => "  [definition]",
            "SYMBOL_REFERENCE" => "  [reference]",
            _ => "",
        };
        if r.line_number == 0 {
            let _ = writeln!(out, "{}  [file name matches]", r.file_path);
            continue;
        }
        match (&r.context_lines, r.context_start_line) {
            (Some(lines), Some(start)) if context > 0 => {
                let _ = writeln!(
                    out,
                    "\n{}:{}:{}{kind}",
                    r.file_path,
                    r.line_number,
                    r.match_column + 1
                );
                for (i, line) in lines.iter().enumerate() {
                    let n = start + i;
                    let mark = if n == r.line_number { '>' } else { ' ' };
                    let _ = writeln!(out, "{mark}{n:>6}  {line}");
                }
            }
            _ => {
                let _ = writeln!(
                    out,
                    "{}:{}:{}: {}{}{kind}",
                    r.file_path,
                    r.line_number,
                    r.match_column + 1,
                    r.content.trim_end(),
                    if r.content_truncated { " …" } else { "" }
                );
            }
        }
    }
    Ok(out)
}

// --------------------------------------------------------------- read_file

#[derive(serde::Deserialize)]
struct ReadArgs {
    path: String,
    start_line: Option<usize>,
    end_line: Option<usize>,
}

async fn read_file(state: &WebState, a: ReadArgs) -> ToolResult {
    let q = query(json!({ "file": a.path }))?;
    let Json(file) = api::file_handler(State(state.clone()), q).await?;
    let lines: Vec<&str> = file.content.lines().collect();
    let total = lines.len();
    let start = a.start_line.unwrap_or(1).max(1);
    if total == 0 {
        return Ok(format!("{} is empty.", file.file));
    }
    if start > total {
        return Err(format!(
            "{} has {total} lines; start_line {start} is past the end.",
            file.file
        )
        .into());
    }
    let wanted_end = a
        .end_line
        .unwrap_or(start + DEFAULT_READ_LINES - 1)
        .max(start);
    let end = wanted_end.min(total).min(start + MAX_READ_LINES - 1);
    let mut out = format!("{} (lines {start}-{end} of {total})\n", file.file);
    for (i, line) in lines[start - 1..end].iter().enumerate() {
        let _ = writeln!(out, "{:>6}  {line}", start + i);
    }
    if end < total {
        let _ = writeln!(
            out,
            "… {} more lines; continue with start_line={}",
            total - end,
            end + 1
        );
    }
    Ok(out)
}

// ------------------------------------------------------- file_dependencies

#[derive(serde::Deserialize)]
struct DepsArgs {
    path: String,
    #[serde(default)]
    direction: Option<String>,
    depth: Option<usize>,
    limit: Option<usize>,
}

#[derive(serde::Deserialize)]
struct Node {
    path: String,
    depth: i32,
    imports: usize,
    imported_by: usize,
    test: bool,
}

#[derive(serde::Deserialize)]
struct Hidden {
    depth: i32,
    count: usize,
    folder: Option<String>,
    folders: usize,
}

#[derive(serde::Deserialize)]
struct NodeSetJson {
    file: String,
    nodes: Vec<Node>,
    hidden: Vec<Hidden>,
}

#[derive(serde::Deserialize)]
struct ImportLineJson {
    line: usize,
    target: Option<String>,
    containment: bool,
}

#[derive(serde::Deserialize)]
struct ImportsJson {
    imports: Vec<ImportLineJson>,
}

/// Re-read a handler's typed response through JSON (its fields are the API).
fn reshape<T: serde::Serialize, U: DeserializeOwned>(v: &T) -> Result<U, ToolError> {
    serde_json::to_value(v)
        .and_then(serde_json::from_value)
        .map_err(|e| ToolError::Failed(format!("Internal error: {e}")))
}

fn hidden_note(h: &Hidden) -> String {
    match &h.folder {
        Some(f) => format!(
            "… {} more (in {}/)",
            h.count,
            if f.is_empty() { "." } else { f }
        ),
        None => format!("… {} more (in {} folders)", h.count, h.folders),
    }
}

async fn file_dependencies(state: &WebState, a: DepsArgs) -> ToolResult {
    let direction = a.direction.as_deref().unwrap_or("both");
    let (show_out, show_in) = match direction {
        "out" => (true, false),
        "in" => (false, true),
        "both" => (true, true),
        other => return Err(format!("Unknown direction {other:?}: use in, out or both").into()),
    };
    let depth = a.depth.unwrap_or(1).clamp(1, 4);
    let per_level = limit(a.limit, DEFAULT_LIMIT, MAX_LIMIT);
    let q = query(json!({ "file": a.path, "depth": depth, "limit": per_level }))?;
    let Json(resp) = graph::neighborhood_handler(State(state.clone()), q).await?;
    let set: NodeSetJson = reshape(&resp)?;

    // The line of each direct import, from the file itself.
    let mut import_lines: BTreeMap<String, usize> = BTreeMap::new();
    if show_out {
        let q = query(json!({ "file": set.file }))?;
        if let Ok(Json(imports)) = graph::imports_handler(State(state.clone()), q).await {
            let imports: ImportsJson = reshape(&imports)?;
            for i in imports.imports.into_iter().filter(|i| !i.containment) {
                if let Some(t) = i.target {
                    import_lines.entry(t).or_insert(i.line);
                }
            }
        }
    }

    let me = set.nodes.iter().find(|n| n.depth == 0);
    let mut out = format!(
        "{}: imports {} files, imported by {} (direct, excluding Rust `mod` declarations).\n",
        set.file,
        me.map_or(0, |n| n.imports),
        me.map_or(0, |n| n.imported_by),
    );
    let mut section = |title: &str, sign: i32| {
        let _ = writeln!(out, "\n{title}:");
        let mut any = false;
        for d in 1..=depth as i32 {
            let level = d * sign;
            for n in set.nodes.iter().filter(|n| n.depth == level) {
                any = true;
                let mut extra = String::new();
                if level == -1 {
                    if let Some(line) = import_lines.get(&n.path) {
                        let _ = write!(extra, "  (imported at {}:{line})", set.file);
                    }
                }
                if n.test {
                    extra.push_str("  [test]");
                }
                let _ = writeln!(out, "  {d}  {}{extra}", n.path);
            }
            if let Some(h) = set.hidden.iter().find(|h| h.depth == level) {
                any = true;
                let _ = writeln!(out, "  {d}  {}", hidden_note(h));
            }
        }
        if !any {
            let _ = writeln!(out, "  (none)");
        }
    };
    if show_out {
        section("Imports (hops away, path)", -1);
    }
    if show_in {
        section("Imported by (hops away, path)", 1);
    }
    if !has_import_graph(&set.file) {
        let _ = writeln!(out, "\nNote: {GRAPH_COVERAGE}");
    }
    Ok(out)
}

fn has_import_graph(path: &str) -> bool {
    let ext = path
        .rsplit_once('.')
        .map_or("", |(_, e)| e)
        .to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "rs" | "py" | "pyi" | "pyw" | "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts"
    )
}

// ----------------------------------------------------------- change_impact

#[derive(serde::Deserialize)]
struct ImpactArgs {
    paths: Vec<String>,
    limit: Option<usize>,
}

async fn change_impact(state: &WebState, a: ImpactArgs) -> ToolResult {
    if a.paths.is_empty() {
        return Err("paths is empty".to_string().into());
    }
    let per_group = limit(a.limit, DEFAULT_LIMIT, MAX_LIMIT);
    let engine = state.engine.clone();
    tokio::task::spawn_blocking(move || -> ToolResult {
        let engine = try_read_engine(&engine).map_err(|(_, m)| ToolError::Failed(m))?;
        let index = &engine.dependency_index;
        let mut starts = Vec::new();
        for p in &a.paths {
            starts.push(lookup(&engine, p).map_err(|(_, m)| ToolError::Failed(m))?);
        }
        // Nearest distance from any of the changed files.
        let mut dist: BTreeMap<u32, usize> = BTreeMap::new();
        for &s in &starts {
            for (d, level) in index.levels(s, Direction::ImportedBy, None, false).iter().enumerate().skip(1) {
                for &n in level {
                    let e = dist.entry(n).or_insert(d);
                    *e = (*e).min(d);
                }
            }
        }
        for s in &starts {
            dist.remove(s);
        }
        let mut affected: Vec<(usize, String)> = dist
            .into_iter()
            .filter_map(|(id, d)| engine.display_path(id).map(|p| (d, p.into_owned())))
            .collect();
        affected.sort();
        let (tests, code): (Vec<_>, Vec<_>) = affected.iter().partition(|(_, p)| is_test_path(p));
        let names: Vec<String> = starts
            .iter()
            .filter_map(|&s| engine.display_path(s).map(|p| p.into_owned()))
            .collect();
        let what = if names.len() == 1 {
            names[0].clone()
        } else {
            format!("these {} files", names.len())
        };
        if affected.is_empty() {
            return Ok(format!(
                "Nothing imports {what}, so a change there affects no other indexed file. {GRAPH_COVERAGE}\n"
            ));
        }
        let max_d = affected.iter().map(|(d, _)| *d).max().unwrap_or(0);
        let mut out = format!(
            "Changing {what} can affect {} other files ({} tests, {} other code), up to {max_d} imports away.\n",
            affected.len(),
            tests.len(),
            code.len()
        );
        for (title, list) in [("Tests", &tests), ("Code", &code)] {
            if list.is_empty() {
                continue;
            }
            let _ = writeln!(out, "\n{title} (hops away, path):");
            for (d, p) in list.iter().take(per_group) {
                let _ = writeln!(out, "  {d}  {p}");
            }
            if list.len() > per_group {
                let _ = writeln!(out, "  … {} more (raise limit to see them)", list.len() - per_group);
            }
        }
        Ok(out)
    })
    .await
    .map_err(|e| ToolError::Failed(format!("Task join error: {e}")))?
}

// ------------------------------------------------------------- import_path

#[derive(serde::Deserialize)]
struct PathArgs {
    from: String,
    to: String,
}

#[derive(serde::Deserialize)]
struct PathJson {
    from: String,
    to: String,
    found: bool,
    reversed: bool,
    files: Vec<String>,
}

async fn import_path(state: &WebState, a: PathArgs) -> ToolResult {
    let q = query(json!({ "from": a.from, "to": a.to }))?;
    let Json(resp) = graph::path_handler(State(state.clone()), q).await?;
    let p: PathJson = reshape(&resp)?;
    if !p.found {
        return Ok(format!(
            "No chain of imports connects {} and {} in either direction (Rust `mod` declarations \
             not counted). {GRAPH_COVERAGE}\n",
            p.from, p.to
        ));
    }
    let mut out = String::new();
    if p.reversed {
        let _ = writeln!(
            out,
            "{} does not reach {} through imports, but the reverse holds:",
            p.from, p.to
        );
    }
    let steps = p.files.len() - 1;
    let _ = writeln!(
        out,
        "{steps} import{} from {} to {} (each file imports the next):",
        if steps == 1 { "" } else { "s" },
        p.files[0],
        p.files[p.files.len() - 1]
    );
    for (i, f) in p.files.iter().enumerate() {
        let _ = writeln!(out, "  {}{f}", if i == 0 { "" } else { "→ " });
    }
    Ok(out)
}

// ------------------------------------------------------ dependency_overview

#[derive(serde::Deserialize)]
struct OverviewArgs {
    #[serde(default)]
    directory: String,
    limit: Option<usize>,
}

#[derive(serde::Deserialize)]
struct FolderJson {
    folder: String,
    files: usize,
    imports: usize,
    imported_by: usize,
    cycle: bool,
}

#[derive(serde::Deserialize)]
struct FolderEdgeJson {
    from: String,
    to: String,
    count: usize,
}

#[derive(serde::Deserialize)]
struct ModulesJson {
    folders: Vec<FolderJson>,
    edges: Vec<FolderEdgeJson>,
}

#[derive(serde::Deserialize)]
struct FileJson {
    path: String,
    imports: usize,
    imported_by: usize,
}

#[derive(serde::Deserialize)]
struct FilesJson {
    total: usize,
    files: Vec<FileJson>,
}

async fn dependency_overview(state: &WebState, a: OverviewArgs) -> ToolResult {
    let n = limit(a.limit, 30, 200);
    let dir = a.directory.trim().trim_matches('/').to_string();
    let Json(modules) = graph::modules_handler(State(state.clone()), query(json!({}))?).await?;
    let modules: ModulesJson = reshape(&modules)?;
    let in_scope = |f: &str| dir.is_empty() || f == dir || f.contains(&dir);
    let mut folders: Vec<&FolderJson> = modules
        .folders
        .iter()
        .filter(|f| in_scope(&f.folder))
        .collect();
    if folders.is_empty() {
        return Ok(if dir.is_empty() {
            format!("The import graph is empty. {GRAPH_COVERAGE}\n")
        } else {
            format!("No indexed folder matches {dir:?}.\n")
        });
    }
    let files_total: usize = folders.iter().map(|f| f.files).sum();
    folders.sort_by(|x, y| {
        (y.imports + y.imported_by)
            .cmp(&(x.imports + x.imported_by))
            .then_with(|| x.folder.cmp(&y.folder))
    });
    let shown = |f: &str| {
        if f.is_empty() {
            "./".to_string()
        } else {
            format!("{f}/")
        }
    };

    let mut out = format!(
        "{files_total} files in the import graph, in {} folders{}. {GRAPH_COVERAGE}\n\nMost connected folders (files, imported by ←, imports →):\n",
        folders.len(),
        if dir.is_empty() { String::new() } else { format!(" matching {dir:?}") }
    );
    for f in folders.iter().take(n) {
        let _ = writeln!(
            out,
            "  {}  {} files  ←{} →{}{}",
            shown(&f.folder),
            f.files,
            f.imported_by,
            f.imports,
            if f.cycle {
                "  [in an import cycle]"
            } else {
                ""
            }
        );
    }
    if folders.len() > n {
        let _ = writeln!(out, "  … {} more folders", folders.len() - n);
    }

    let mut edges: Vec<&FolderEdgeJson> = modules
        .edges
        .iter()
        .filter(|e| in_scope(&e.from) || in_scope(&e.to))
        .collect();
    edges.sort_by(|x, y| y.count.cmp(&x.count).then_with(|| x.from.cmp(&y.from)));
    if !edges.is_empty() {
        let _ = writeln!(
            out,
            "\nHeaviest folder imports (folder → folder it imports: file-level imports):"
        );
        for e in edges.iter().take(n) {
            let _ = writeln!(out, "  {} → {}: {}", shown(&e.from), shown(&e.to), e.count);
        }
    }
    let cyclic: Vec<String> = folders
        .iter()
        .filter(|f| f.cycle)
        .map(|f| shown(&f.folder))
        .collect();
    if !cyclic.is_empty() {
        let _ = writeln!(out, "\nFolders in import cycles: {}", cyclic.join(", "));
    }

    let q = query(json!({ "q": dir, "limit": n.min(15) }))?;
    let Json(files) = graph::files_handler(State(state.clone()), q).await?;
    let files: FilesJson = reshape(&files)?;
    if !files.files.is_empty() {
        let _ = writeln!(out, "\nHub files (imported by, imports):");
        for f in &files.files {
            let _ = writeln!(out, "  {}  ←{} →{}", f.path, f.imported_by, f.imports);
        }
        if files.total > files.files.len() {
            let _ = writeln!(out, "  (top {} of {})", files.files.len(), files.total);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(origin: Option<&str>, host: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(header::HOST, host.parse().unwrap());
        if let Some(o) = origin {
            h.insert(header::ORIGIN, o.parse().unwrap());
        }
        h
    }

    #[test]
    fn origin_check_allows_local_and_listed_origins_only() {
        let none: Vec<String> = vec![];
        assert!(origin_allowed(&headers(None, "127.0.0.1:8080"), &none));
        assert!(origin_allowed(
            &headers(Some("http://localhost:3000"), "127.0.0.1:8080"),
            &none
        ));
        assert!(origin_allowed(
            &headers(Some("http://127.0.0.1:8080"), "127.0.0.1:8080"),
            &none
        ));
        assert!(origin_allowed(
            &headers(Some("http://[::1]:5173"), "[::1]:8080"),
            &none
        ));
        assert!(origin_allowed(
            &headers(Some("https://code.example"), "code.example"),
            &none
        ));
        assert!(!origin_allowed(
            &headers(Some("https://evil.example"), "127.0.0.1:8080"),
            &none
        ));
        assert!(!origin_allowed(
            &headers(Some("http://localhost.evil.example"), "127.0.0.1:8080"),
            &none
        ));
        let listed = vec!["https://ide.example".to_string()];
        assert!(origin_allowed(
            &headers(Some("https://ide.example"), "127.0.0.1:8080"),
            &listed
        ));
    }

    #[test]
    fn graph_coverage_by_extension() {
        assert!(has_import_graph("r/src/a.rs"));
        assert!(has_import_graph("r/web/App.TSX"));
        assert!(!has_import_graph("r/main.go"));
        assert!(!has_import_graph("r/Makefile"));
    }
}
