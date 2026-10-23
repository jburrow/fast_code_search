//! The contract walk: one search, every surface, walked to the end.
//!
//! For each query the engine's exhaustive ordered list is taken once (pages
//! of 1000, which all rank the same sample) and checked against a scan of
//! the files on disk. Then every surface is walked the way a person or an
//! agent walks it, under the shipped defaults, and must show exactly that
//! list, in that order, with honest counts:
//!
//! - REST `/api/search`, at several page sizes and at random offsets (page
//!   *k* at offset *o* is items *o..o+k* of the list);
//! - gRPC `Search`, paged by `offset`;
//! - MCP `search_code` over HTTP, following "call again with offset=N" to
//!   the end, and the same call through `fcs mcp` on stdio;
//! - `fcs` against the server, piped (vimgrep), paged with `--offset`;
//!   `fcs -l` (every file, best first); plain piped `fcs` (50 hits and a
//!   note that more exist); and offline `fcs` on the saved index;
//! - the web page in headless Chromium (when Playwright is installed):
//!   LOAD MORE and "Show N more hits in this file" pressed until they are
//!   gone, then every group and hit on the page compared.
//!
//! The corpus is generated to reach the limits a small fixture never does:
//! more hits than a page, than MCP's default, than the 1000-hit list; a
//! file with more hits than the per-file cap; file-name hits; a line longer
//! than the content window; on Linux, two files whose names differ only in
//! case.
//!
//! The browser leg needs `node` and the `playwright` package (a local
//! `npm i -D playwright`, or a global one on `NODE_PATH`) with its Chromium;
//! without them it prints a note and is skipped, unless
//! `FCS_REQUIRE_BROWSER=1` is set (CI's Linux test job sets it).

#[path = "support/pygen.rs"]
mod pygen;

use anyhow::{bail, Context, Result};
use fast_code_search::config::IndexerConfig;
use fast_code_search::search::{
    create_progress_broadcaster, run_background_indexer, BackgroundIndexerConfig, IndexingProgress,
    SearchEngine, SearchMode, SearchRequest, SearchSettings,
};
use fast_code_search::server::{
    create_server_with_engine,
    search_proto::{code_search_client::CodeSearchClient, SearchRequest as GrpcRequest},
};
use fast_code_search::web::{create_router, AppState};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};
use tokio::net::TcpListener;
use tonic::transport::Server;

/// The indexed folder's name; with the shipped defaults it starts every
/// displayed path.
const ROOT: &str = "corpus";

/// One hit as every surface can name it: display path and line (0 for a
/// file-name hit).
type Hit = (String, usize);

/// One hit of the exhaustive list, with what the contract checks.
#[derive(Debug, Clone)]
struct Item {
    path: String,
    line: usize,
    score: f64,
    file_hits: u32,
    file_hits_capped: bool,
}

/// What a query is and how each surface spells its mode.
#[derive(Debug, Clone)]
struct Query {
    name: &'static str,
    text: String,
    mode: SearchMode,
    /// The right answer where the generator knows it (references), as
    /// (display path, line).
    key: Option<BTreeSet<Hit>>,
}

impl Query {
    fn mode_name(&self) -> &'static str {
        match self.mode {
            SearchMode::Text => "text",
            SearchMode::Regex => "regex",
            SearchMode::Symbols => "symbols",
            SearchMode::References => "references",
        }
    }
}

/// The Python part of the corpus and its answer key.
fn python() -> pygen::Codebase {
    pygen::generate(&pygen::Options {
        packages: 20,
        ..pygen::Options::new(11)
    })
}

fn queries() -> Vec<Query> {
    // References: the generated name used most, whose every use the
    // generator recorded.
    let cb = python();
    let (name, uses) = cb
        .refs
        .iter()
        .max_by_key(|(n, r)| (r.len(), std::cmp::Reverse((*n).clone())))
        .unwrap();
    let key = uses
        .iter()
        .map(|(p, l)| (format!("{ROOT}/py/{p}"), *l))
        .collect();
    vec![
        Query {
            name: "text, many files and a capped file",
            text: "needle".into(),
            mode: SearchMode::Text,
            key: None,
        },
        Query {
            name: "regex",
            text: r"needle_\d+".into(),
            mode: SearchMode::Regex,
            key: None,
        },
        Query {
            name: "symbols",
            text: "fn_".into(),
            mode: SearchMode::Symbols,
            key: None,
        },
        Query {
            name: "references",
            text: name.clone(),
            mode: SearchMode::References,
            key: Some(key),
        },
    ]
}

/// Write the corpus under `root`.
fn write_corpus(root: &Path) {
    // Python with imports, symbols and references.
    python().write(&root.join("py"));

    let mut rng = pygen::Rng::new(7);
    let write = |rel: &str, text: String| {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    };
    // Many files with a few hits each, in several spellings.
    for i in 0..240 {
        let mut text = String::new();
        for j in 0..rng.range(0, 14) {
            match rng.below(5) {
                0 => text.push_str(&format!("let needle_{j} = {i};\n")),
                1 => text.push_str("A Needle in a haystack\n"),
                2 => text.push_str("NEEDLE NEEDLE twice on one line\n"),
                _ => text.push_str("hay hay hay\n"),
            }
        }
        write(&format!("notes/n{i:03}.txt"), text);
    }
    // One file with more hits than the per-file cap (1000).
    let mut hot = String::new();
    for i in 0..1500 {
        hot.push_str(&format!("needle_{i} in the hot file\n"));
    }
    write("hot/many.txt", hot);
    // A line longer than the 500-byte content window.
    write(
        "hot/long.txt",
        format!("{} needle at the end\n", "x".repeat(900)),
    );
    // File-name hits.
    write("notes/needle_notes.md", "no match in here\n".to_string());
    // Two names that differ only in case (one file on case-insensitive
    // file systems, so Linux only).
    if cfg!(target_os = "linux") {
        write("web/Foo.ts", "const needle = 1;\n".to_string());
        write("web/foo.ts", "const needle = 2;\n".to_string());
    }
}

/// The lines a text query should find, from the files on disk: (display
/// path, line). Case-insensitive, as plain text searches are by default.
fn disk_hits(root: &Path, needle: &str, regex: bool) -> BTreeSet<Hit> {
    let re = if regex {
        regex::Regex::new(needle).unwrap()
    } else {
        regex::RegexBuilder::new(&regex::escape(needle))
            .case_insensitive(true)
            .build()
            .unwrap()
    };
    let mut out = BTreeSet::new();
    for entry in walkdir::WalkDir::new(root).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
        for (i, line) in text.lines().enumerate() {
            if re.is_match(line) {
                out.insert((format!("{ROOT}/{rel}"), i + 1));
            }
        }
    }
    out
}

/// The exhaustive ordered list: the engine's pages of 1000.
fn exhaustive(engine: &SearchEngine, q: &Query) -> Result<(Vec<Item>, Vec<String>)> {
    let settings = SearchSettings::default();
    let mut items = Vec::new();
    let mut limits_hit = BTreeSet::new();
    let mut offset = 0;
    loop {
        let req = SearchRequest {
            query: q.text.clone(),
            mode: q.mode,
            max_results: 1000,
            offset,
            ..Default::default()
        };
        let limits = req.limits(&settings, None).map_err(anyhow::Error::msg)?;
        let out = engine.run(&req, limits).map_err(anyhow::Error::msg)?;
        for l in &out.info.limits_hit {
            limits_hit.insert(format!("{l:?}"));
        }
        let n = out.matches.len();
        items.extend(out.matches.iter().map(|m| Item {
            path: m.file_path.clone(),
            line: m.line_number,
            score: m.score,
            file_hits: m.file_hits,
            file_hits_capped: m.file_hits_capped,
        }));
        if n == 0 || !out.has_more(offset) {
            break;
        }
        offset += n;
    }
    Ok((items, limits_hit.into_iter().collect()))
}

/// The contract on the list itself: no hit twice, each file's hits
/// together, files by best hit, a file's hits by score, honest per-file
/// counts.
fn check_list(q: &Query, items: &[Item]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen = BTreeSet::new();
    let mut files: Vec<&str> = Vec::new();
    let mut per_file: HashMap<&str, usize> = HashMap::new();
    for (i, it) in items.iter().enumerate() {
        if !seen.insert((&it.path, it.line)) {
            problems.push(format!(
                "{}: {}:{} is listed twice",
                q.name, it.path, it.line
            ));
        }
        if files.last() != Some(&it.path.as_str()) {
            if files.contains(&it.path.as_str()) {
                problems.push(format!(
                    "{}: the hits of {} are not together (again at #{i})",
                    q.name, it.path
                ));
            }
            files.push(&it.path);
        } else if it.score > items[i - 1].score + 1e-9 {
            problems.push(format!(
                "{}: in {}, line {} (score {}) comes after line {} (score {})",
                q.name,
                it.path,
                it.line,
                it.score,
                items[i - 1].line,
                items[i - 1].score
            ));
        }
        *per_file.entry(&it.path).or_default() += 1;
    }
    // Files by best hit: each file's first hit scores no higher than the
    // previous file's first hit.
    let firsts: Vec<&Item> = files
        .iter()
        .map(|f| items.iter().find(|it| it.path == *f).unwrap())
        .collect();
    for w in firsts.windows(2) {
        if w[1].score > w[0].score + 1e-9 {
            problems.push(format!(
                "{}: {} (best {}) is listed after {} (best {})",
                q.name, w[1].path, w[1].score, w[0].path, w[0].score
            ));
        }
    }
    for it in items {
        let n = per_file[it.path.as_str()];
        if it.file_hits as usize != n && !it.file_hits_capped {
            problems.push(format!(
                "{}: {} says {} hits in the file, the list has {n}",
                q.name, it.path, it.file_hits
            ));
        }
    }
    problems
}

fn hits(items: &[Item]) -> Vec<Hit> {
    items.iter().map(|i| (i.path.clone(), i.line)).collect()
}

/// Compare a surface's walk with the list; a short description of the
/// first difference.
fn same(surface: &str, q: &Query, got: &[Hit], want: &[Hit]) -> Option<String> {
    if got == want {
        return None;
    }
    let first = got
        .iter()
        .zip(want)
        .position(|(a, b)| a != b)
        .unwrap_or(got.len().min(want.len()));
    Some(format!(
        "{} via {surface}: {} hits, the list has {}; first difference at #{first}: {:?} vs {:?}",
        q.name,
        got.len(),
        want.len(),
        got.get(first),
        want.get(first)
    ))
}

struct Servers {
    web: String,
    grpc: String,
    config: PathBuf,
    root: PathBuf,
    engine: AppState,
    _temp: tempfile::TempDir,
}

async fn start() -> Result<Servers> {
    let temp = tempfile::tempdir()?;
    let base = temp.path().canonicalize()?;
    let root = base.join(ROOT);
    write_corpus(&root);
    let index_path = base.join("index.fcsidx");
    let indexer = IndexerConfig {
        paths: vec![root.to_string_lossy().into_owned()],
        index_path: Some(index_path.to_string_lossy().into_owned()),
        watch: false,
        ..Default::default()
    };
    let engine: AppState = Arc::new(RwLock::new(SearchEngine::new()));
    {
        let engine = engine.clone();
        let indexer = indexer.clone();
        tokio::task::spawn_blocking(move || {
            run_background_indexer(BackgroundIndexerConfig {
                indexer_config: indexer,
                engine,
                progress: Arc::new(RwLock::new(IndexingProgress::default())),
                progress_tx: create_progress_broadcaster(),
                shutdown: Arc::new(AtomicBool::new(false)),
            })
        })
        .await?;
    }
    engine
        .read()
        .unwrap()
        .save_index(&index_path, &indexer)
        .context("saving the index")?;
    let config = base.join("fcs.toml");
    std::fs::write(
        &config,
        format!(
            "[indexer]\npaths = [{:?}]\nindex_path = {:?}\n",
            root.to_string_lossy(),
            index_path.to_string_lossy()
        ),
    )?;

    let grpc_listener = TcpListener::bind("127.0.0.1:0").await?;
    let grpc = format!("http://{}", grpc_listener.local_addr()?);
    let service = create_server_with_engine(engine.clone());
    tokio::spawn(async move {
        Server::builder()
            .add_service(service)
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(
                grpc_listener,
            ))
            .await
            .expect("gRPC server failed");
    });
    let http_listener = TcpListener::bind("127.0.0.1:0").await?;
    let web = format!("http://{}", http_listener.local_addr()?);
    let router = create_router(
        engine.clone(),
        Arc::new(RwLock::new(IndexingProgress::default())),
        create_progress_broadcaster(),
        None,
    );
    tokio::spawn(async move {
        axum::serve(http_listener, router)
            .await
            .expect("HTTP server failed");
    });
    Ok(Servers {
        web,
        grpc,
        config,
        root,
        engine,
        _temp: temp,
    })
}

/// REST, one page.
async fn rest_page(s: &Servers, q: &Query, offset: usize, max: Option<usize>) -> Result<Value> {
    let mut params: Vec<(&str, String)> =
        vec![("q", q.text.clone()), ("offset", offset.to_string())];
    if let Some(m) = max {
        params.push(("max", m.to_string()));
    }
    match q.mode {
        SearchMode::Regex => params.push(("regex", "true".into())),
        SearchMode::Symbols => params.push(("symbols", "true".into())),
        SearchMode::References => params.push(("references", "true".into())),
        SearchMode::Text => {}
    }
    let resp = reqwest::Client::new()
        .get(format!("{}/api/search", s.web))
        .query(&params)
        .send()
        .await?;
    if !resp.status().is_success() {
        bail!("{}: HTTP {}", q.name, resp.status());
    }
    Ok(resp.json().await?)
}

fn json_hits(v: &Value) -> Vec<Hit> {
    v["results"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|r| {
                    (
                        r["file_path"].as_str().unwrap_or_default().to_string(),
                        r["line_number"].as_u64().unwrap_or(0) as usize,
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// REST walked with LOAD MORE's offsets at page size `max` (None: the
/// server's default page). Checks each page's slice, total and has_more.
async fn rest_walk(
    s: &Servers,
    q: &Query,
    max: Option<usize>,
    want: &[Hit],
    problems: &mut Vec<String>,
) -> Result<Vec<Hit>> {
    let mut all = Vec::new();
    loop {
        let offset = all.len();
        let page = rest_page(s, q, offset, max).await?;
        let got = json_hits(&page);
        let end = (offset + got.len()).min(want.len());
        if got[..] != want[offset.min(want.len())..end] {
            problems.push(format!(
                "{} via REST max={max:?}: page at offset {offset} is not the list's slice",
                q.name
            ));
        }
        if let Some(total) = page["total_matches"].as_u64() {
            if total as usize != want.len() {
                problems.push(format!(
                    "{} via REST max={max:?}: page at {offset} says {total} matches, the list has {}",
                    q.name,
                    want.len()
                ));
            }
        }
        let has_more = page["has_more"].as_bool().unwrap_or(false);
        if has_more != (offset + got.len() < want.len()) {
            problems.push(format!(
                "{} via REST max={max:?}: has_more={has_more} at {offset}+{}, of {}",
                q.name,
                got.len(),
                want.len()
            ));
        }
        all.extend(got.iter().cloned());
        if got.is_empty() || !has_more || all.len() > want.len() * 2 + 10 {
            return Ok(all);
        }
    }
}

async fn grpc_walk(s: &Servers, q: &Query, page: i32) -> Result<Vec<Hit>> {
    let mut client = CodeSearchClient::connect(s.grpc.clone()).await?;
    let mut all = Vec::new();
    loop {
        let req = GrpcRequest {
            query: q.text.clone(),
            max_results: page,
            offset: all.len() as i32,
            is_regex: matches!(q.mode, SearchMode::Regex),
            symbols_only: matches!(q.mode, SearchMode::Symbols),
            references: matches!(q.mode, SearchMode::References),
            ..Default::default()
        };
        let mut stream = client.search(req).await?.into_inner();
        let mut got = Vec::new();
        while let Some(r) = stream.message().await? {
            got.push((r.file_path, r.line_number as usize));
        }
        let n = got.len();
        all.extend(got);
        // gRPC has no has_more: a short page is the last.
        if n < page as usize {
            return Ok(all);
        }
    }
}

fn mcp_request(q: &Query, offset: usize, limit: usize, id: usize) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {
            "name": "search_code",
            "arguments": {
                "query": &q.text,
                "mode": q.mode_name(),
                "offset": offset,
                "limit": limit,
            }
        }
    })
}

/// The hits in an MCP `search_code` text, and the offset it says to pass
/// next.
fn mcp_hits(text: &str) -> (Vec<Hit>, Option<usize>) {
    let mut hits = Vec::new();
    let mut next = None;
    for line in text.lines() {
        if let Some(i) = line.find("call again with offset=") {
            let rest = &line[i + "call again with offset=".len()..];
            next = rest
                .trim_end_matches('.')
                .split(|c: char| !c.is_ascii_digit())
                .next()
                .and_then(|n| n.parse().ok());
        }
        if let Some(path) = line.strip_suffix("  [file name matches]") {
            hits.push((path.to_string(), 0));
            continue;
        }
        // path:line:col: text
        if let Some(hit) = path_line(line) {
            hits.push(hit);
        }
    }
    (hits, next)
}

async fn mcp_post(s: &Servers, body: &Value) -> Result<String> {
    let v: Value = reqwest::Client::new()
        .post(format!("{}/mcp", s.web))
        .json(body)
        .send()
        .await?
        .json()
        .await?;
    v["result"]["content"][0]["text"]
        .as_str()
        .map(str::to_string)
        .with_context(|| format!("MCP answered {v}"))
}

/// MCP over HTTP: start at offset 0 with the default limit and follow the
/// offsets the result names until it names none.
async fn mcp_walk(s: &Servers, q: &Query, limit: usize) -> Result<(Vec<Hit>, String)> {
    let mut all = Vec::new();
    let mut offset = 0;
    let mut first_text = String::new();
    for id in 1.. {
        let text = mcp_post(s, &mcp_request(q, offset, limit, id)).await?;
        if id == 1 {
            first_text = text.clone();
        }
        let (hits, next) = mcp_hits(&text);
        all.extend(hits);
        match next {
            Some(n) if n > offset => offset = n,
            _ => break,
        }
        if id > 10_000 {
            bail!("MCP never stopped naming a next offset");
        }
    }
    Ok((all, first_text))
}

fn fcs(s: &Servers) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_fcs"));
    cmd.env_remove("FCS_CONFIG")
        .env_remove("FCS_SERVER")
        .env("NO_COLOR", "1")
        .args(["--server", &s.web]);
    cmd
}

fn fcs_mode_args(cmd: &mut Command, q: &Query) {
    match q.mode {
        SearchMode::Symbols => {
            cmd.arg("symbols");
        }
        SearchMode::References => {
            cmd.arg("refs");
        }
        SearchMode::Regex => {
            cmd.arg("-e");
        }
        SearchMode::Text => {}
    }
}

/// `path:line:col:text` lines.
fn vimgrep_hits(out: &str) -> Vec<Hit> {
    out.lines().filter_map(path_line).collect()
}

/// The path and line of a `path:line:col:text` line. The path is what comes
/// before the first `:line:col:`, so a Windows drive letter (`C:\...`) or a
/// colon in the text does not cut it short.
fn path_line(l: &str) -> Option<Hit> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"^(.*?):(\d+):(\d+):").unwrap());
    let c = re.captures(l)?;
    Some((c[1].to_string(), c[2].parse().ok()?))
}

/// `fcs` piped, paged with `--offset` like a script would.
fn fcs_walk(s: &Servers, q: &Query, page: usize, offline: bool) -> Result<Vec<Hit>> {
    let mut all = Vec::new();
    loop {
        let mut cmd = if offline {
            let mut c = Command::new(env!("CARGO_BIN_EXE_fcs"));
            c.env_remove("FCS_CONFIG")
                .env_remove("FCS_SERVER")
                .env("NO_COLOR", "1")
                .args(["--offline", "--config"])
                .arg(&s.config);
            c
        } else {
            fcs(s)
        };
        cmd.args(["--format", "vimgrep", "-q"]);
        fcs_mode_args(&mut cmd, q);
        cmd.args(["-n", &page.to_string(), "--offset", &all.len().to_string()])
            .arg("--")
            .arg(&q.text);
        let out = cmd.output()?;
        // With the configuration's roots (offline), fcs prints paths that
        // open from the current folder, like rg: absolute here.
        let got: Vec<Hit> = vimgrep_hits(&String::from_utf8_lossy(&out.stdout))
            .into_iter()
            .map(|(p, l)| (display_path(&s.root, &p), l))
            .collect();
        let n = got.len();
        all.extend(got);
        if n < page {
            return Ok(all);
        }
    }
}

/// `path` as results show it: an absolute path under `root` becomes
/// `<root name>/rest`. Compared as text with `/` separators and without
/// Windows' `\\?\` prefix, which `canonicalize` adds and fcs does not print.
fn display_path(root: &Path, path: &str) -> String {
    let norm = |p: &str| p.replace('\\', "/").trim_start_matches("//?/").to_string();
    let (root, path) = (norm(&root.to_string_lossy()), norm(path));
    match path.strip_prefix(&format!("{root}/")) {
        Some(rest) => format!("{ROOT}/{rest}"),
        None => path,
    }
}

/// What the web page showed (scripts/e2e/web_walk.cjs).
fn browser_walk(s: &Servers, q: &Query) -> Result<Option<Value>> {
    let node_path = {
        let global = Command::new("npm")
            .args(["root", "-g"])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
        match std::env::var("NODE_PATH") {
            Ok(p) if !p.is_empty() => format!("{p}:{global}"),
            _ => global,
        }
    };
    let have = Command::new("node")
        .args(["-e", "require('playwright')"])
        .env("NODE_PATH", &node_path)
        .output()
        .is_ok_and(|o| o.status.success());
    if !have {
        if std::env::var_os("FCS_REQUIRE_BROWSER").is_some() {
            bail!("FCS_REQUIRE_BROWSER is set but node with playwright is missing");
        }
        eprintln!("node with playwright not found; skipping the web page leg");
        return Ok(None);
    }
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/e2e/web_walk.cjs");
    let mut cmd = Command::new("node");
    cmd.arg(script).arg(&s.web).arg(&q.text);
    if !matches!(q.mode, SearchMode::Text) {
        cmd.arg(q.mode_name());
    }
    let out = cmd.env("NODE_PATH", &node_path).output()?;
    if !out.status.success() {
        bail!(
            "the web walk failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(Some(serde_json::from_slice(&out.stdout)?))
}

/// What the page should show for `items`: files in order of their best
/// hit, each with all its hits in list order.
fn page_groups(items: &[Item]) -> Vec<(String, Vec<usize>)> {
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
    for it in items {
        match groups.last_mut() {
            Some((p, lines)) if *p == it.path => lines.push(it.line),
            _ => groups.push((it.path.clone(), vec![it.line])),
        }
    }
    groups
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_surface_shows_the_whole_list_in_order() -> Result<()> {
    let s = start().await?;
    let mut problems: Vec<String> = Vec::new();

    for q in &queries() {
        let (items, limits) = {
            let engine = s.engine.read().unwrap();
            exhaustive(&engine, q)?
        };
        eprintln!(
            "{}: {} hits in {} files; limits {:?}",
            q.name,
            items.len(),
            page_groups(&items).len(),
            limits
        );
        if items.len() <= 50 && matches!(q.mode, SearchMode::Text | SearchMode::Regex) {
            problems.push(format!(
                "{}: only {} hits; the corpus should give more than a page",
                q.name,
                items.len()
            ));
        }
        problems.extend(check_list(q, &items));
        let want = hits(&items);

        // The list against the disk, for the queries a line scan answers.
        if matches!(q.mode, SearchMode::Text | SearchMode::Regex) {
            let disk = disk_hits(&s.root, &q.text, matches!(q.mode, SearchMode::Regex));
            let listed: BTreeSet<Hit> = want.iter().filter(|h| h.1 > 0).cloned().collect();
            let capped: BTreeSet<&str> = items
                .iter()
                .filter(|i| i.file_hits_capped)
                .map(|i| i.path.as_str())
                .collect();
            let missing: Vec<&Hit> = disk
                .difference(&listed)
                .filter(|h| !capped.contains(h.0.as_str()))
                .collect();
            let extra: Vec<&Hit> = listed.difference(&disk).collect();
            if !missing.is_empty() || !extra.is_empty() {
                problems.push(format!(
                    "{}: the list and the disk differ: missing {:?}, extra {:?}",
                    q.name,
                    &missing[..missing.len().min(5)],
                    &extra[..extra.len().min(5)]
                ));
            }
            if !capped.is_empty() && limits.is_empty() {
                problems.push(format!(
                    "{}: {capped:?} hold more hits than shown, but no limit is named",
                    q.name
                ));
            }
        }

        // REST at the UI's default page, odd sizes and the largest.
        for max in [None, Some(7), Some(333), Some(1000)] {
            let got = rest_walk(&s, q, max, &want, &mut problems).await?;
            problems.extend(same(&format!("REST max={max:?}"), q, &got, &want));
        }
        // Random slices.
        let mut rng = pygen::Rng::new(q.text.len() as u64);
        if let Some(key) = &q.key {
            let listed: BTreeSet<Hit> = want.iter().cloned().collect();
            if &listed != key {
                problems.push(format!(
                    "{}: the list and the generator's key differ: missing {:?}, extra {:?}",
                    q.name,
                    key.difference(&listed).take(5).collect::<Vec<_>>(),
                    listed.difference(key).take(5).collect::<Vec<_>>()
                ));
            }
        }
        for _ in 0..12 {
            let offset = rng.below(want.len() + 5);
            let max = rng.range(1, 400);
            let page = rest_page(&s, q, offset, Some(max)).await?;
            let got = json_hits(&page);
            let end = (offset + max).min(want.len());
            let slice = &want[offset.min(want.len())..end];
            if got != slice {
                problems.push(format!(
                    "{}: REST offset={offset} max={max} is not the list's slice ({} vs {} hits)",
                    q.name,
                    got.len(),
                    slice.len()
                ));
            }
        }

        let got = grpc_walk(&s, q, 50).await?;
        problems.extend(same("gRPC", q, &got, &want));

        let (got, first) = mcp_walk(&s, q, 50).await?;
        problems.extend(same("MCP", q, &got, &want));
        if want.len() > 50 && !first.contains("more exist") {
            problems.push(format!(
                "{}: MCP's first page does not say more exist:\n{first}",
                q.name
            ));
        }
        // The same first call through `fcs mcp` on stdio.
        let stdio = tokio::task::block_in_place(|| -> Result<String> {
            let mut child = fcs(&s)
                .arg("mcp")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()?;
            let mut stdin = child.stdin.take().unwrap();
            writeln!(stdin, "{}", mcp_request(q, 0, 50, 1))?;
            drop(stdin);
            let mut line = String::new();
            BufReader::new(child.stdout.take().unwrap()).read_line(&mut line)?;
            child.wait()?;
            let v: Value = serde_json::from_str(&line)?;
            Ok(v["result"]["content"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .to_string())
        })?;
        if strip_timing(&stdio) != strip_timing(&first) {
            problems.push(format!(
                "{}: `fcs mcp` on stdio answered differently from /mcp",
                q.name
            ));
        }

        // fcs, piped.
        for (page, offline) in [(1000, false), (73, false), (1000, true)] {
            let got = tokio::task::block_in_place(|| fcs_walk(&s, q, page, offline))?;
            problems.extend(same(
                &format!("fcs -n {page}{}", if offline { " --offline" } else { "" }),
                q,
                &got,
                &want,
            ));
        }
        let (plain, note) = tokio::task::block_in_place(|| -> Result<(Vec<Hit>, String)> {
            let mut cmd = fcs(&s);
            fcs_mode_args(&mut cmd, q);
            let out = cmd.arg("--").arg(&q.text).output()?;
            Ok((
                vimgrep_hits(&String::from_utf8_lossy(&out.stdout)),
                String::from_utf8_lossy(&out.stderr).to_string(),
            ))
        })?;
        problems.extend(same(
            "plain piped fcs (first 50)",
            q,
            &plain,
            &want[..want.len().min(50)],
        ));
        if want.len() > 50 && !note.to_lowercase().contains("more") {
            problems.push(format!(
                "{}: piped fcs stopped at 50 hits without saying more exist: {note:?}",
                q.name
            ));
        }
        if matches!(q.mode, SearchMode::Text | SearchMode::Regex) {
            let files = tokio::task::block_in_place(|| -> Result<Vec<String>> {
                let mut cmd = fcs(&s);
                fcs_mode_args(&mut cmd, q);
                let out = cmd.args(["-l", "-q", "--"]).arg(&q.text).output()?;
                Ok(String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .map(str::to_string)
                    .collect())
            })?;
            let want_files: Vec<String> = page_groups(&items).into_iter().map(|g| g.0).collect();
            if files != want_files {
                problems.push(format!(
                    "{}: fcs -l listed {} files, the list has {} (first: {:?} vs {:?})",
                    q.name,
                    files.len(),
                    want_files.len(),
                    files.first(),
                    want_files.first()
                ));
            }
        }

        // The web page.
        if let Some(shown) = tokio::task::block_in_place(|| browser_walk(&s, q))? {
            eprintln!("{}: the web page showed {}", q.name, shown["count"]);
            if let Some(errors) = shown["errors"].as_array().filter(|e| !e.is_empty()) {
                problems.push(format!("{}: the page threw: {errors:?}", q.name));
            }
            let groups: Vec<(String, Vec<usize>)> = shown["groups"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|g| {
                    (
                        g["path"].as_str().unwrap_or_default().to_string(),
                        g["lines"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_u64)
                            .map(|l| l as usize)
                            .collect(),
                    )
                })
                .collect();
            let want_groups = page_groups(&items);
            if groups != want_groups {
                let first = groups
                    .iter()
                    .zip(&want_groups)
                    .position(|(a, b)| a != b)
                    .unwrap_or(groups.len().min(want_groups.len()));
                problems.push(format!(
                    "{}: the web page shows {} groups, the list has {}; first difference at group {first}: {:?} vs {:?}",
                    q.name,
                    groups.len(),
                    want_groups.len(),
                    groups.get(first).map(|g| (&g.0, g.1.len())),
                    want_groups.get(first).map(|g| (&g.0, g.1.len()))
                ));
            }
            // Each badge counts the hits its group shows (all of them, now).
            let counts: BTreeMap<String, String> = shown["groups"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|g| {
                    (
                        g["path"].as_str().unwrap_or_default().to_string(),
                        g["badge"].as_str().unwrap_or_default().to_string(),
                    )
                })
                .collect();
            for (path, lines) in &want_groups {
                let n = lines.len();
                let want_badge = format!("{n} hit{}", if n == 1 { "" } else { "s" });
                if let Some(badge) = counts.get(path) {
                    if badge.replace(',', "") != want_badge {
                        problems.push(format!(
                            "{}: {path}'s badge says {badge:?} with {n} hits shown",
                            q.name
                        ));
                    }
                }
            }
        }
    }

    assert!(
        problems.is_empty(),
        "{} contract violations:\n  {}",
        problems.len(),
        problems.join("\n  ")
    );
    Ok(())
}

/// MCP text without the parts that differ between two calls (timings).
fn strip_timing(text: &str) -> String {
    text.lines()
        .filter(|l| !l.contains(" ms"))
        .collect::<Vec<_>>()
        .join("\n")
}
