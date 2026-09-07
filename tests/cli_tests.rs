//! End-to-end tests for the `fcs` command-line client: against a live
//! router (server mode), against a saved index (offline mode), and the
//! failure paths (no server, bad regex), checking grep's exit codes.

use anyhow::Result;
use fast_code_search::config::IndexerConfig;
use fast_code_search::search::{create_progress_broadcaster, IndexingProgress, SearchEngine};
use fast_code_search::web::{create_router_with_options, AppState, RouterOptions};
use std::process::Command;
use std::sync::{Arc, RwLock};
use tempfile::TempDir;
use tokio::net::TcpListener;

const RUST_FILE: &str = "fn main() {\n    helper_target(1);\n}\n\nfn helper_target(x: i32) -> i32 {\n    x + 1\n}\n// TODO: cli_marker_token here\n";
const PY_FILE: &str = "def py_thing():\n    return helper_target()\n";

fn fcs() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_fcs"));
    // Keep the machine's own configuration out of the tests.
    c.env_remove("FCS_CONFIG")
        .env_remove("FCS_SERVER")
        .env("NO_COLOR", "1");
    c
}

fn corpus() -> Result<(TempDir, SearchEngine)> {
    let temp = TempDir::new()?;
    let root = temp.path().join("proj");
    std::fs::create_dir_all(root.join("src"))?;
    std::fs::write(root.join("src/main.rs"), RUST_FILE)?;
    std::fs::write(root.join("src/util.py"), PY_FILE)?;
    let mut engine = SearchEngine::new();
    engine.add_root_path(&root);
    engine.index_file(root.join("src/main.rs"))?;
    engine.index_file(root.join("src/util.py"))?;
    engine.finalize();
    Ok((temp, engine))
}

async fn serve(engine: SearchEngine) -> Result<String> {
    let engine: AppState = Arc::new(RwLock::new(engine));
    let progress = Arc::new(RwLock::new(IndexingProgress::default()));
    let progress_tx = create_progress_broadcaster();
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let router = create_router_with_options(
        engine,
        progress,
        progress_tx,
        None,
        &RouterOptions::default(),
    );
    tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("HTTP server failed");
    });
    Ok(format!("http://{addr}"))
}

fn run(cmd: &mut Command) -> (i32, String, String) {
    let out = cmd.output().expect("run fcs");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_against_server_uses_grep_exit_codes_and_vimgrep_output() -> Result<()> {
    let (_temp, engine) = corpus()?;
    let url = serve(engine).await?;

    let (code, out, err) = run(fcs().args(["--server", &url, "helper_target"]));
    assert_eq!(code, 0, "stderr: {err}");
    // Piped stdout: path:line:col:text, one per hit.
    assert!(
        out.contains("proj/src/main.rs:2:5:    helper_target(1);"),
        "{out}"
    );
    assert!(
        out.contains("proj/src/main.rs:5:4:fn helper_target(x: i32) -> i32 {"),
        "{out}"
    );
    assert!(out.contains("proj/src/util.py:2:"), "{out}");

    let (code, out, _) = run(fcs().args(["--server", &url, "no_such_token_anywhere"]));
    assert_eq!(code, 1);
    assert!(out.is_empty());

    let (code, _, err) = run(fcs().args(["--server", &url, "-e", "[unclosed"]));
    assert_eq!(code, 2);
    assert!(err.contains("Invalid regex"), "{err}");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn modes_flags_and_formats() -> Result<()> {
    let (_temp, engine) = corpus()?;
    let url = serve(engine).await?;

    // References: the two call sites, not the definition.
    let (code, out, _) = run(fcs().args(["--server", &url, "refs", "helper_target"]));
    assert_eq!(code, 0);
    assert!(out.contains("main.rs:2:"), "{out}");
    assert!(out.contains("util.py:2:"), "{out}");
    assert!(!out.contains("main.rs:5:"), "{out}");

    // Symbols: the definition only.
    let (code, out, _) = run(fcs().args(["--server", &url, "symbols", "helper_target"]));
    assert_eq!(code, 0);
    assert!(out.contains("main.rs:5:"), "{out}");
    assert!(!out.contains("main.rs:2:"), "{out}");

    // Glob filter and files-only.
    let (code, out, _) = run(fcs().args(["--server", &url, "-l", "-g", "*.py", "helper_target"]));
    assert_eq!(code, 0);
    assert_eq!(out.trim(), "proj/src/util.py");

    // Regex + JSON.
    let (code, out, _) = run(fcs().args(["--server", &url, "--json", "-e", r"fn\s+\w+\("]));
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out)?;
    assert!(
        v["results"]
            .as_array()
            .map(|a| a.len() >= 2)
            .unwrap_or(false),
        "{out}"
    );

    // Context and grouped format.
    let (code, out, _) = run(fcs().args([
        "--server",
        &url,
        "--format",
        "grouped",
        "-C",
        "1",
        "cli_marker_token",
    ]));
    assert_eq!(code, 0);
    assert!(out.starts_with("proj/src/main.rs\n"), "{out}");
    assert!(
        out.contains("    8: // TODO: cli_marker_token here"),
        "{out}"
    );
    assert!(out.contains("    7  }"), "{out}");
    Ok(())
}

#[test]
fn offline_search_reads_the_saved_index() -> Result<()> {
    let (temp, engine) = corpus()?;
    let root = temp.path().join("proj");
    let index_path = temp.path().join("index.fcsidx");
    let config = IndexerConfig {
        paths: vec![root.to_string_lossy().to_string()],
        index_path: Some(index_path.to_string_lossy().to_string()),
        ..Default::default()
    };
    engine.save_index(&index_path, &config)?;
    let cfg_path = temp.path().join("fcs.toml");
    std::fs::write(
        &cfg_path,
        format!(
            "[indexer]\npaths = [{:?}]\nindex_path = {:?}\n",
            root.to_string_lossy(),
            index_path.to_string_lossy()
        ),
    )?;

    // Explicit offline, index from the config.
    let (code, out, err) = run(fcs().args([
        "--offline",
        "--config",
        cfg_path.to_str().unwrap(),
        "helper_target",
    ]));
    assert_eq!(code, 0, "stderr: {err}");
    assert!(out.contains("proj/src/main.rs:5:"), "{out}");
    assert!(err.contains("loaded 2 files"), "{err}");

    // Absolute paths map the root name back to the configured directory.
    let (code, out, _) = run(fcs().args([
        "--offline",
        "--absolute",
        "--config",
        cfg_path.to_str().unwrap(),
        "-l",
        "py_thing",
    ]));
    assert_eq!(code, 0);
    assert_eq!(
        out.trim(),
        root.join("src").join("util.py").to_string_lossy()
    );

    // Unreachable server + configured index: falls back with a note.
    let (code, out, err) = run(fcs().args([
        "--server",
        "http://127.0.0.1:1",
        "--config",
        cfg_path.to_str().unwrap(),
        "py_thing",
    ]));
    assert_eq!(code, 0, "stderr: {err}");
    assert!(err.contains("not reachable"), "{err}");
    assert!(out.contains("util.py:1:"), "{out}");

    // --no-offline turns that into an error with instructions.
    let (code, _, err) = run(fcs().args([
        "--server",
        "http://127.0.0.1:1",
        "--no-offline",
        "--config",
        cfg_path.to_str().unwrap(),
        "py_thing",
    ]));
    assert_eq!(code, 2);
    assert!(err.contains("cannot reach the search server"), "{err}");
    assert!(err.contains("RUN-AT-STARTUP"), "{err}");
    Ok(())
}

#[test]
fn no_server_and_no_index_is_an_error_and_status_reports_it() -> Result<()> {
    let temp = TempDir::new()?;
    let cfg_path = temp.path().join("fcs.toml");
    std::fs::write(&cfg_path, "[indexer]\npaths = []\n")?;
    let (code, _, err) = run(fcs().args([
        "--server",
        "http://127.0.0.1:1",
        "--config",
        cfg_path.to_str().unwrap(),
        "anything",
    ]));
    assert_eq!(code, 2);
    assert!(err.contains("cannot reach the search server"), "{err}");

    let (code, out, _) = run(fcs().args([
        "--server",
        "http://127.0.0.1:1",
        "--config",
        cfg_path.to_str().unwrap(),
        "status",
    ]));
    assert_eq!(code, 0);
    assert!(out.contains("not reachable"), "{out}");
    Ok(())
}

#[test]
fn help_and_usage_errors() {
    let (code, out, _) = run(fcs().arg("--help"));
    assert_eq!(code, 0);
    assert!(out.contains("fcs QUERY"), "{out}");
    let (code, _, err) = run(fcs().arg("--no-such-flag"));
    assert_eq!(code, 2);
    assert!(err.contains("unexpected argument"), "{err}");
    let (code, _, _) = run(&mut fcs());
    assert_eq!(code, 2, "no query is a usage error");
}
