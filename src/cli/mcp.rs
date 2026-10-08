//! `fcs mcp`: a stdio bridge to the server's `/mcp` endpoint, for MCP
//! clients that only launch local commands.
//!
//! MCP's stdio transport is one JSON-RPC message per line. Each line read
//! from stdin is POSTed to `<server>/mcp`; the response body, if any, is
//! written to stdout as one line. Nothing else is printed on stdout (it is
//! the protocol channel); problems go to stderr, and a request that cannot
//! reach the server is answered with a JSON-RPC error so the client sees
//! why instead of hanging.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::time::Duration;

/// What one POST to `/mcp` produced.
pub enum Reply {
    /// A JSON-RPC response (or batch) to pass on.
    Body(String),
    /// Accepted with no body (the message was a notification).
    Empty,
}

/// Relay every line of `input` through `post`, writing replies to `output`.
/// Returns when `input` reaches end of file.
pub fn relay<R, W, P>(input: R, mut output: W, mut post: P) -> Result<()>
where
    R: BufRead,
    W: Write,
    P: FnMut(&str) -> std::result::Result<Reply, String>,
{
    for line in input.lines() {
        let line = line.context("reading stdin")?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let reply = match post(line) {
            Ok(Reply::Body(body)) => Some(one_line(&body)),
            Ok(Reply::Empty) => None,
            Err(err) => {
                eprintln!("fcs mcp: {err}");
                // Answer requests (they carry an id) so the client is not
                // left waiting; notifications need nothing.
                serde_json::from_str::<Value>(line)
                    .ok()
                    .and_then(|m| m.get("id").cloned())
                    .filter(|id| !id.is_null())
                    .map(|id| {
                        json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": {"code": -32000, "message": err},
                        })
                        .to_string()
                    })
            }
        };
        if let Some(r) = reply {
            writeln!(output, "{r}").context("writing stdout")?;
            output.flush().context("writing stdout")?;
        }
    }
    Ok(())
}

/// The stdio transport forbids embedded newlines; re-encode compactly.
fn one_line(body: &str) -> String {
    match serde_json::from_str::<Value>(body) {
        Ok(v) => v.to_string(),
        Err(_) => body.replace(['\n', '\r'], " "),
    }
}

/// Run the bridge against `server` (a base URL such as http://127.0.0.1:8080).
pub fn run(server: &str) -> Result<()> {
    let url = format!("{server}/mcp");
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(120))
        .build()
        .context("building HTTP client")?;
    eprintln!("fcs mcp: relaying MCP over stdio to {url}");
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    relay(stdin.lock(), stdout.lock(), |line| {
        let resp = client
            .post(&url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(
                reqwest::header::ACCEPT,
                "application/json, text/event-stream",
            )
            .body(line.to_string())
            .send()
            .map_err(|e| {
                format!(
                    "cannot reach the fast_code_search server at {server} ({e}); start it, or \
                     point at another with --server URL or $FCS_SERVER"
                )
            })?;
        let status = resp.status();
        let body = resp
            .text()
            .map_err(|e| format!("reading the response: {e}"))?;
        if status == reqwest::StatusCode::ACCEPTED || body.trim().is_empty() {
            return Ok(Reply::Empty);
        }
        if status.is_success()
            || serde_json::from_str::<Value>(&body).is_ok_and(|v| v.get("jsonrpc").is_some())
        {
            return Ok(Reply::Body(body));
        }
        Err(format!(
            "the server answered {status} for /mcp ({}); is it a fast_code_search \
             server new enough to serve MCP?",
            body.trim()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_relay(
        input: &str,
        mut post: impl FnMut(&str) -> std::result::Result<Reply, String>,
    ) -> String {
        let mut out = Vec::new();
        relay(input.as_bytes(), &mut out, &mut post).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn relays_each_line_and_skips_empty_replies() {
        let mut seen = Vec::new();
        let out = run_relay(
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
            |line| {
                seen.push(line.to_string());
                Ok(if line.contains("\"id\"") {
                    Reply::Body("{\n  \"jsonrpc\": \"2.0\",\n  \"id\": 1,\n  \"result\": {}\n}".into())
                } else {
                    Reply::Empty
                })
            },
        );
        assert_eq!(seen.len(), 2, "blank lines are skipped");
        assert_eq!(out, "{\"id\":1,\"jsonrpc\":\"2.0\",\"result\":{}}\n");
    }

    #[test]
    fn unreachable_server_answers_requests_with_an_error() {
        let out = run_relay(
            "{\"jsonrpc\":\"2.0\",\"id\":\"a\",\"method\":\"tools/list\"}\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
            |_| Err("down".to_string()),
        );
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 1, "the notification gets no reply: {out}");
        let v: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(v["id"], "a");
        assert_eq!(v["error"]["message"], "down");
    }
}
