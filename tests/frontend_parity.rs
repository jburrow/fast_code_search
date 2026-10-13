//! Front-end parity: the same search through the web API, the gRPC service
//! and offline `fcs` returns the same hits in the same order.
//!
//! The three used to carry their own copies of the request handling and
//! drifted apart; they now share `search::request`. This test keeps it that
//! way. (MCP's `search_code` calls the web handler, so it is covered by the
//! web leg.)

use anyhow::Result;
use fast_code_search::config::IndexerConfig;
use fast_code_search::search::{create_progress_broadcaster, IndexingProgress, SearchEngine};
use fast_code_search::server::{
    create_server_with_engine,
    search_proto::{code_search_client::CodeSearchClient, SearchRequest},
};
use fast_code_search::web::{create_router, AppState};
use std::process::Command;
use std::sync::{Arc, RwLock};
use tempfile::TempDir;
use tokio::net::TcpListener;
use tonic::transport::Server;

const FILES: &[(&str, &str)] = &[
    (
        "src/main.rs",
        "use crate::util::Parser;\n\nfn main() {\n    let p = Parser::new();\n    parse_input(&p);\n    // TODO parse later\n}\n\nfn parse_input(p: &Parser) -> usize {\n    p.parse(\"Parse me\")\n}\n",
    ),
    (
        "src/util.rs",
        "pub struct Parser;\n\nimpl Parser {\n    pub fn new() -> Self { Parser }\n    pub fn parse(&self, s: &str) -> usize { s.len() }\n}\n// parser, PARSER, parsers\n",
    ),
    (
        "app/models.py",
        "class Parser:\n    def parse(self, text):\n        return text.split()\n\n\ndef parse_input(text):\n    return Parser().parse(text)\n",
    ),
    (
        "web/index.ts",
        "import { parseInput } from \"./parse\";\n\nexport function parseInput(s: string) {\n  return s.trim(); // parse\n}\n",
    ),
];

/// One request, as each front end spells it.
struct Case {
    name: &'static str,
    query: &'static str,
    /// "text", "regex", "symbols" or "refs".
    mode: &'static str,
    case_sensitive: bool,
    word: bool,
    glob: Option<&'static str>,
}

const CASES: &[Case] = &[
    Case {
        name: "plain text",
        query: "parse",
        mode: "text",
        case_sensitive: false,
        word: false,
        glob: None,
    },
    Case {
        name: "phrase",
        query: "parse later",
        mode: "text",
        case_sensitive: false,
        word: false,
        glob: None,
    },
    Case {
        name: "match case",
        query: "Parse",
        mode: "text",
        case_sensitive: true,
        word: false,
        glob: None,
    },
    Case {
        name: "whole word",
        query: "parse",
        mode: "text",
        case_sensitive: false,
        word: true,
        glob: None,
    },
    Case {
        name: "glob filter",
        query: "parse",
        mode: "text",
        case_sensitive: false,
        word: false,
        glob: Some("*.py"),
    },
    Case {
        name: "regex",
        query: r"fn\s+\w+",
        mode: "regex",
        case_sensitive: false,
        word: false,
        glob: None,
    },
    Case {
        name: "regex, match case",
        query: r"[Pp]arse\w*",
        mode: "regex",
        case_sensitive: true,
        word: false,
        glob: None,
    },
    Case {
        name: "query syntax",
        query: "lang:rust parse",
        mode: "text",
        case_sensitive: false,
        word: false,
        glob: None,
    },
    Case {
        name: "symbols",
        query: "parse",
        mode: "symbols",
        case_sensitive: false,
        word: false,
        glob: None,
    },
    Case {
        name: "references",
        query: "Parser",
        mode: "refs",
        case_sensitive: false,
        word: false,
        glob: None,
    },
];

type Hit = (String, u64, u64);

fn hits_from_json(v: &serde_json::Value) -> Vec<Hit> {
    v["results"]
        .as_array()
        .expect("results array")
        .iter()
        .map(|r| {
            (
                r["file_path"].as_str().unwrap().to_string(),
                r["line_number"].as_u64().unwrap(),
                r["line_match_start"].as_u64().unwrap_or(0),
            )
        })
        .collect()
}

async fn web_hits(url: &str, c: &Case) -> Result<Vec<Hit>> {
    let mut q: Vec<(&str, String)> = vec![("q", c.query.to_string()), ("max", "200".into())];
    match c.mode {
        "regex" => q.push(("regex", "true".into())),
        "symbols" => q.push(("symbols", "true".into())),
        "refs" => q.push(("references", "true".into())),
        _ => {}
    }
    if c.case_sensitive {
        q.push(("case", "true".into()));
    }
    if c.word {
        q.push(("word", "true".into()));
    }
    if let Some(g) = c.glob {
        q.push(("include", g.into()));
    }
    let resp = reqwest::Client::new()
        .get(format!("{url}/api/search"))
        .query(&q)
        .send()
        .await?;
    assert!(resp.status().is_success(), "{}: {}", c.name, resp.status());
    Ok(hits_from_json(&resp.json().await?))
}

async fn grpc_hits(url: &str, c: &Case) -> Result<Vec<Hit>> {
    let mut client = CodeSearchClient::connect(url.to_string()).await?;
    let req = SearchRequest {
        query: c.query.to_string(),
        max_results: 200,
        include_paths: c.glob.map(|g| vec![g.to_string()]).unwrap_or_default(),
        is_regex: c.mode == "regex",
        symbols_only: c.mode == "symbols",
        references: c.mode == "refs",
        case_sensitive: c.case_sensitive,
        whole_word: c.word,
        ..Default::default()
    };
    let mut stream = client.search(req).await?.into_inner();
    let mut hits = Vec::new();
    while let Some(r) = stream.message().await? {
        hits.push((r.file_path, r.line_number as u64, r.line_match_start as u64));
    }
    Ok(hits)
}

fn cli_hits(config: &str, c: &Case) -> Vec<Hit> {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_fcs"));
    cmd.env_remove("FCS_CONFIG")
        .env_remove("FCS_SERVER")
        .env("NO_COLOR", "1")
        .args(["--offline", "--config", config, "--json", "-n", "200"]);
    match c.mode {
        "symbols" => {
            cmd.arg("symbols");
        }
        "refs" => {
            cmd.arg("refs");
        }
        _ => {}
    }
    if c.mode == "regex" {
        cmd.arg("-e");
    }
    if c.case_sensitive {
        cmd.arg("-s");
    }
    if c.word {
        cmd.arg("-w");
    }
    if let Some(g) = c.glob {
        cmd.args(["-g", g]);
    }
    cmd.arg("--").arg(c.query);
    let out = cmd.output().expect("run fcs");
    let stdout = String::from_utf8_lossy(&out.stdout);
    if stdout.trim().is_empty() {
        return Vec::new();
    }
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "{}: fcs output is not JSON ({e}): {stdout}\nstderr: {}",
            c.name,
            String::from_utf8_lossy(&out.stderr)
        )
    });
    hits_from_json(&v)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_grpc_and_offline_cli_agree() -> Result<()> {
    let temp = TempDir::new()?;
    let root = temp.path().join("proj");
    let mut engine = SearchEngine::new();
    engine.add_root_path(&root);
    for (rel, text) in FILES {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(&path, text)?;
    }
    for (rel, _) in FILES {
        engine.index_file(root.join(rel))?;
    }
    engine.resolve_imports();
    engine.finalize();

    // Offline fcs reads a saved copy of the same index.
    let index_path = temp.path().join("index.fcsidx");
    let indexer = IndexerConfig {
        paths: vec![root.to_string_lossy().to_string()],
        index_path: Some(index_path.to_string_lossy().to_string()),
        ..Default::default()
    };
    engine.save_index(&index_path, &indexer)?;
    let cfg_path = temp.path().join("fcs.toml");
    std::fs::write(
        &cfg_path,
        format!(
            "[indexer]\npaths = [{:?}]\nindex_path = {:?}\n",
            root.to_string_lossy(),
            index_path.to_string_lossy()
        ),
    )?;

    let engine: AppState = Arc::new(RwLock::new(engine));
    let grpc_listener = TcpListener::bind("127.0.0.1:0").await?;
    let grpc_url = format!("http://{}", grpc_listener.local_addr()?);
    let grpc_service = create_server_with_engine(engine.clone());
    tokio::spawn(async move {
        Server::builder()
            .add_service(grpc_service)
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(
                grpc_listener,
            ))
            .await
            .expect("gRPC server failed");
    });
    let http_listener = TcpListener::bind("127.0.0.1:0").await?;
    let web_url = format!("http://{}", http_listener.local_addr()?);
    let router = create_router(
        engine,
        Arc::new(RwLock::new(IndexingProgress::default())),
        create_progress_broadcaster(),
        None,
    );
    tokio::spawn(async move {
        axum::serve(http_listener, router)
            .await
            .expect("HTTP server failed");
    });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let config = cfg_path.to_str().unwrap();
    for c in CASES {
        let web = web_hits(&web_url, c).await?;
        assert!(!web.is_empty(), "{}: the web API found nothing", c.name);
        let grpc = grpc_hits(&grpc_url, c).await?;
        assert_eq!(grpc, web, "{}: gRPC differs from the web API", c.name);
        let cli = tokio::task::block_in_place(|| cli_hits(config, c));
        assert_eq!(cli, web, "{}: offline fcs differs from the web API", c.name);
    }
    Ok(())
}
