//! The server binary under test, started and stopped by the load tools
//! (`scoreboard`, `chaos`), and the HTTP calls they make to it.

#![allow(dead_code)]

use anyhow::{bail, Context, Result};
use fast_code_search::search::integrity::IntegrityReport;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub fn free_port() -> Result<u16> {
    let l = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(l.local_addr()?.port())
}

pub fn client(timeout: Duration) -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(5))
        .build()
        .unwrap()
}

/// A running server.
pub struct Server {
    pub pid: u32,
    child: Mutex<Child>,
    /// `http://127.0.0.1:<port>`.
    pub base: String,
    pub started: Instant,
    http: reqwest::blocking::Client,
}

impl Server {
    /// Start `binary` with `config` (written to `work/config.toml`; it must
    /// listen on `port`), in `work`, logging to `log`.
    pub fn spawn(
        binary: &Path,
        work: &Path,
        port: u16,
        config: &str,
        log: &Path,
    ) -> Result<Server> {
        let config_path = work.join("config.toml");
        std::fs::write(&config_path, config)?;
        let log = std::fs::File::create(log)?;
        let child = Command::new(binary)
            .arg("--config")
            .arg(&config_path)
            .current_dir(work)
            .env("RUST_LOG", "info")
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()
            .with_context(|| format!("starting {}", binary.display()))?;
        Ok(Server {
            pid: child.id(),
            child: Mutex::new(child),
            base: format!("http://127.0.0.1:{port}"),
            started: Instant::now(),
            http: client(Duration::from_secs(600)),
        })
    }

    /// `kill -9`, then wait for the process to go.
    pub fn kill9(&self) -> Result<()> {
        let mut child = self.child.lock().unwrap();
        child.kill()?;
        child.wait()?;
        Ok(())
    }

    /// SIGTERM without waiting (see [`Self::stop`]).
    pub fn terminate(&self) {
        #[cfg(unix)]
        unsafe {
            libc::kill(self.pid as i32, libc::SIGTERM);
        }
        #[cfg(not(unix))]
        let _ = self.child.lock().unwrap().kill();
    }

    pub fn get(&self, path: &str) -> Result<reqwest::blocking::Response> {
        Ok(self.http.get(format!("{}{path}", self.base)).send()?)
    }

    pub fn crashed(&self) -> bool {
        matches!(self.child.lock().unwrap().try_wait(), Ok(Some(_)))
    }

    /// (until it answered anything, until it was ready), from the start.
    pub fn wait_listening_and_ready(&self, limit: Duration) -> Result<(Duration, Duration)> {
        let mut listening = None;
        loop {
            if self.crashed() {
                bail!("the server exited while starting");
            }
            if let Ok(r) = self.get("/api/ready") {
                let now = self.started.elapsed();
                listening.get_or_insert(now);
                if r.status().is_success() {
                    return Ok((listening.unwrap(), now));
                }
            }
            if self.started.elapsed() > limit {
                bail!("the server was not ready in {limit:?}");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    pub fn wait_ready(&self, limit: Duration) -> Result<()> {
        self.wait_listening_and_ready(limit).map(|_| ())
    }

    /// Ready, and the build or the restart's check of the tree finished.
    pub fn wait_settled(&self, limit: Duration) -> Result<()> {
        self.wait_ready(limit)?;
        while self.started.elapsed() < limit {
            let indexing = self
                .get("/api/status")
                .ok()
                .filter(|r| r.status().is_success())
                .and_then(|r| r.json::<serde_json::Value>().ok())
                .map(|v| v["is_indexing"].as_bool().unwrap_or(true));
            if indexing == Some(false) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        bail!("the server was still indexing after {limit:?}")
    }

    /// Seconds until no change is pending and every path in `paths` is
    /// indexed as it is on disk, polled every 250 ms.
    pub fn wait_fresh(&self, paths: &[PathBuf], limit: Duration) -> Option<Duration> {
        let t = Instant::now();
        let wanted: Vec<String> = paths
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        let query = wanted.join(";");
        while t.elapsed() < limit {
            let fresh = self
                .http
                .get(format!("{}/api/status", self.base))
                .query(&[("paths", query.as_str())])
                .send()
                .ok()
                .filter(|r| r.status().is_success())
                .and_then(|r| r.json::<serde_json::Value>().ok())
                .is_some_and(|v| {
                    let index = &v["index"];
                    let pending = index["pending_changes"].as_u64().unwrap_or(0);
                    let current = v["paths"].as_array().is_some_and(|ps| {
                        ps.len() == wanted.len()
                            && ps.iter().all(|p| {
                                let state = p["state"].as_str().unwrap_or("");
                                // A file a stream removed again is fine.
                                state == "current" || state == "not_found"
                            })
                    });
                    !index.is_null() && pending == 0 && current
                });
            if fresh {
                return Some(t.elapsed());
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        None
    }

    pub fn check(&self) -> Result<IntegrityReport> {
        // One check at a time; the scrub's own does not use this lock, but
        // be patient with a 429 anyway.
        for _ in 0..30 {
            let r = self.get("/api/check?all=true&examples=0")?;
            if r.status().as_u16() == 429 {
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
            if !r.status().is_success() {
                bail!("check answered {}", r.status());
            }
            return Ok(r.json()?);
        }
        bail!("a check was always running")
    }

    /// SIGTERM (a graceful stop that saves the index), then wait.
    pub fn stop(&self) -> Result<()> {
        let mut child = self.child.lock().unwrap();
        #[cfg(unix)]
        unsafe {
            libc::kill(self.pid as i32, libc::SIGTERM);
        }
        #[cfg(not(unix))]
        let _ = child.kill();
        let t = Instant::now();
        loop {
            if let Some(status) = child.try_wait()? {
                if !status.success() {
                    eprintln!("the server exited with {status}");
                }
                return Ok(());
            }
            if t.elapsed() > Duration::from_secs(300) {
                let _ = child.kill();
                bail!("the server did not stop within 300 s of SIGTERM");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Ok(mut c) = self.child.lock() {
            if matches!(c.try_wait(), Ok(None)) {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
    }
}
