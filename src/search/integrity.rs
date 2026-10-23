//! The live-server check behind `/api/check` and `fcs check --server`, and
//! the background scrub.
//!
//! Three questions, each answered with counts first and examples second:
//!
//! - **Invariants:** does the index agree with itself
//!   ([`SearchEngine::check_invariants`])?
//! - **Content:** for indexed files, is what was indexed still on disk, and
//!   is every trigram of it in the index ([`SearchEngine::check_file`])?
//! - **Discovery:** does the set of indexed files match what a fresh walk
//!   of the disk, with the same settings, would index?
//!
//! Changes the watcher has yet to apply look like differences for a moment,
//! so differences are checked a second time once the watcher has caught up
//! (or after a few seconds), and only those that remain are reported.
//!
//! The engine lock is taken for reads only, in short pieces: the
//! invariants under one read lock (about a second on a million files),
//! file checks in small groups, the disk walk under none.

use crate::config::IndexerConfig;
use crate::search::engine::{FileCheck, InvariantReport};
use crate::search::{engine_lock, FileDiscoveryIterator, PartialIndexedFile, SearchEngine};
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Files checked per read-lock hold.
const FILES_PER_HOLD: usize = 256;

/// How long a second look waits for the watcher to catch up.
const SETTLE: Duration = Duration::from_secs(10);

/// What to check.
#[derive(Debug, Clone)]
pub struct IntegrityOptions {
    /// Files whose content to check (`None`: all of them). Spread evenly
    /// over the index.
    pub content_files: Option<usize>,
    /// Walk the disk and compare the file set.
    pub discovery: bool,
    /// Examples kept per kind of difference (they name files).
    pub examples: usize,
}

impl Default for IntegrityOptions {
    fn default() -> Self {
        Self {
            content_files: Some(2_000),
            discovery: true,
            examples: 5,
        }
    }
}

/// The result: counts by kind, and a few example paths per kind.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IntegrityReport {
    /// Live files in the index.
    pub files: usize,
    /// Index invariant violations by kind (see [`InvariantReport`]).
    pub invariants: BTreeMap<String, usize>,
    /// Files whose content was checked.
    pub content_checked: usize,
    /// Content checks that did not come back current, by kind: `changed`
    /// and `gone` (on disk but not in the index yet: a lost change if it
    /// lasts), `missing_trigrams` (a search would skip the file),
    /// `not_indexable`, `unreadable`.
    pub content: BTreeMap<String, usize>,
    /// Files on disk the index should hold (`None`: discovery not run).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_disk: Option<usize>,
    /// Discovery differences by kind: `missing` (on disk and indexable, not
    /// in the index), `extra` (in the index, but gone or excluded now),
    /// `unreadable` (on disk, can't be read).
    pub discovery: BTreeMap<String, usize>,
    /// Changes the watcher still had to apply when the check ended.
    pub pending_changes: usize,
    /// The startup build was still running (differences are expected).
    pub building: bool,
    /// How long the check took.
    pub elapsed_ms: u64,
    /// File watching is on (without it, changes since the last scan show
    /// up here as differences until the next restart).
    pub watching: bool,
    /// What the background scrub has found since the server started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scrub: Option<ScrubStats>,
    /// Example paths (display form) per kind, `kind` prefixed by its group
    /// (`invariants.path_map`, `content.changed`, `discovery.missing`).
    pub examples: BTreeMap<String, Vec<String>>,
}

impl IntegrityReport {
    /// Differences of every kind (invariants, content, discovery), not
    /// counting unreadable files, which are reported but are not the index's
    /// fault.
    pub fn problems(&self) -> usize {
        self.invariants.values().sum::<usize>()
            + self
                .content
                .iter()
                .chain(&self.discovery)
                .filter(|(k, _)| k.as_str() != "unreadable" && k.as_str() != "not_indexable")
                .map(|(_, n)| n)
                .sum::<usize>()
    }

    /// The report as text: numbers only, unless `with_paths`.
    pub fn render(&self, with_paths: bool) -> String {
        let mut out = String::new();
        let line = |out: &mut String, s: String| {
            out.push_str(&s);
            out.push('\n');
        };
        line(
            &mut out,
            format!(
                "index: {} files{}{}",
                self.files,
                if self.building {
                    ", first build still running"
                } else {
                    ""
                },
                if self.pending_changes > 0 {
                    format!(", {} changes still to apply", self.pending_changes)
                } else {
                    String::new()
                }
            ),
        );
        let section =
            |out: &mut String, title: &str, group: &str, counts: &BTreeMap<String, usize>| {
                if counts.is_empty() {
                    line(out, format!("{title}: ok"));
                    return;
                }
                line(out, format!("{title}:"));
                for (kind, n) in counts {
                    line(out, format!("  {kind}: {n}"));
                    if with_paths {
                        for e in self
                            .examples
                            .get(&format!("{group}.{kind}"))
                            .into_iter()
                            .flatten()
                        {
                            line(out, format!("    {e}"));
                        }
                    }
                }
            };
        section(&mut out, "invariants", "invariants", &self.invariants);
        section(
            &mut out,
            &format!("content ({} files read)", self.content_checked),
            "content",
            &self.content,
        );
        match self.on_disk {
            Some(n) => section(
                &mut out,
                &format!("discovery ({n} files on disk to index)"),
                "discovery",
                &self.discovery,
            ),
            None => line(&mut out, "discovery: not run".into()),
        }
        if let Some(s) = &self.scrub {
            let drift: u64 = s.drift.values().sum();
            line(
                &mut out,
                format!(
                    "background scrub: {} files checked ({} full passes), {} out of step{}; \
                     {} consistency checks, last {}",
                    s.files_checked,
                    s.passes,
                    drift,
                    if drift > 0 {
                        format!(
                            " ({}), all queued to index again",
                            s.drift
                                .iter()
                                .map(|(k, n)| format!("{k} {n}"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    } else {
                        String::new()
                    },
                    s.invariant_checks,
                    if s.invariant_checks == 0 {
                        "not run yet".to_string()
                    } else if s.invariant_violations.is_empty() {
                        "clean".to_string()
                    } else {
                        format!(
                            "{} violations",
                            s.invariant_violations.values().sum::<usize>()
                        )
                    }
                ),
            );
        }
        if !self.watching && self.problems() > 0 {
            line(
                &mut out,
                "note: file watching is off, so files changed since the last scan show up \
                 as changed, gone, missing or extra until the server rescans them"
                    .into(),
            );
        }
        line(
            &mut out,
            format!(
                "{} in {:.1} s",
                match self.problems() {
                    0 => "no problems found".to_string(),
                    n => format!("{n} problems found"),
                },
                self.elapsed_ms as f64 / 1000.0
            ),
        );
        out
    }

    fn add(&mut self, group: &str, kind: &str, example: String, max_examples: usize) {
        let counts = match group {
            "content" => &mut self.content,
            "discovery" => &mut self.discovery,
            _ => &mut self.invariants,
        };
        *counts.entry(kind.to_string()).or_default() += 1;
        let examples = self.examples.entry(format!("{group}.{kind}")).or_default();
        if examples.len() < max_examples {
            examples.push(example);
        }
    }
}

/// The name a [`FileCheck`] is counted under; `None` for a current file.
pub fn file_check_kind(check: FileCheck) -> Option<&'static str> {
    match check {
        FileCheck::Current => None,
        FileCheck::Changed => Some("changed"),
        FileCheck::Gone => Some("gone"),
        FileCheck::Unreadable => Some("unreadable"),
        FileCheck::NotIndexable => Some("not_indexable"),
        FileCheck::MissingTrigrams(_) => Some("missing_trigrams"),
    }
}

/// Run the check against a live engine.
pub fn check(
    engine: &Arc<RwLock<SearchEngine>>,
    config: &IndexerConfig,
    opts: &IntegrityOptions,
) -> IntegrityReport {
    let started = Instant::now();
    let mut report = IntegrityReport::default();

    // Invariants, under one read lock.
    let (invariants, ids) = {
        let e = read(engine);
        (e.check_invariants(), e.live_file_ids())
    };
    report.files = invariants.files;
    absorb_invariants(&mut report, &invariants, opts.examples);

    // Content of an even spread of files, a group at a time.
    let picked: Vec<u32> = match opts.content_files {
        None => ids,
        Some(n) if n >= ids.len() => ids,
        Some(0) => Vec::new(),
        Some(n) => {
            let step = ids.len() as f64 / n as f64;
            (0..n).map(|i| ids[(i as f64 * step) as usize]).collect()
        }
    };
    report.content_checked = picked.len();
    let mut differing = check_files(engine, &picked);
    if !differing.is_empty() {
        settle(engine);
        let again: Vec<u32> = differing.iter().map(|(id, _, _)| *id).collect();
        differing = check_files(engine, &again);
    }
    for (_, path, check) in differing {
        if let Some(kind) = file_check_kind(check) {
            report.add("content", kind, path, opts.examples);
        }
    }

    if opts.discovery {
        let mut diff = discovery_diff(engine, config);
        if !diff.missing.is_empty() || !diff.extra.is_empty() {
            settle(engine);
            diff = discovery_diff(engine, config);
        }
        report.on_disk = Some(diff.on_disk);
        let e = read(engine);
        for (kind, paths) in [
            ("missing", &diff.missing),
            ("extra", &diff.extra),
            ("unreadable", &diff.unreadable),
        ] {
            for p in paths {
                report.add("discovery", kind, e.make_display_path(p), opts.examples);
            }
        }
    }

    {
        let e = read(engine);
        report.pending_changes = e.freshness().pending_changes;
        report.building = e.building;
        report.watching = e.watching;
        let scrub = e
            .watch_backlog
            .scrub
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        report.scrub = scrub.enabled.then_some(scrub);
    }
    report.elapsed_ms = started.elapsed().as_millis() as u64;
    report
}

fn absorb_invariants(report: &mut IntegrityReport, inv: &InvariantReport, max_examples: usize) {
    for (kind, n) in &inv.counts {
        report.invariants.insert(kind.clone(), *n);
        let examples = inv.examples.get(kind).cloned().unwrap_or_default();
        report.examples.insert(
            format!("invariants.{kind}"),
            examples.into_iter().take(max_examples).collect(),
        );
    }
}

fn read(engine: &Arc<RwLock<SearchEngine>>) -> std::sync::RwLockReadGuard<'_, SearchEngine> {
    engine.read().unwrap_or_else(|p| p.into_inner())
}

/// Check `ids`, a group per read lock; the ones not current, with their
/// display paths.
pub fn check_files(
    engine: &Arc<RwLock<SearchEngine>>,
    ids: &[u32],
) -> Vec<(u32, String, FileCheck)> {
    use rayon::prelude::*;
    let mut out = Vec::new();
    for group in ids.chunks(FILES_PER_HOLD) {
        let e = read(engine);
        let found: Vec<_> = group
            .par_iter()
            .filter_map(|&id| {
                let check = e.check_file(id)?;
                let path = e.get_file_path(id)?;
                (check != FileCheck::Current).then_some((id, path, check))
            })
            .collect();
        out.extend(found);
    }
    out
}

/// Wait (up to [`SETTLE`]) until the watcher has nothing left to apply.
fn settle(engine: &Arc<RwLock<SearchEngine>>) {
    let deadline = Instant::now() + SETTLE;
    // A change written just now may not even be queued yet.
    std::thread::sleep(Duration::from_millis(500));
    while Instant::now() < deadline {
        let pending = engine_lock::read_within(engine, Duration::from_secs(1))
            .map(|e| e.freshness().pending_changes)
            .unwrap_or(1);
        if pending == 0 {
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// How the indexed file set differs from a fresh walk of the disk.
#[derive(Debug, Default)]
pub struct DiscoveryDiff {
    /// Files the walk finds that the indexer would index.
    pub on_disk: usize,
    /// On disk and indexable, not in the index.
    pub missing: Vec<PathBuf>,
    /// In the index, but not found by the walk (gone, or excluded now).
    pub extra: Vec<PathBuf>,
    /// On disk, not in the index, and can't be read.
    pub unreadable: Vec<PathBuf>,
}

/// Walk the configured paths (no lock held) and compare with the index.
pub fn discovery_diff(engine: &Arc<RwLock<SearchEngine>>, config: &IndexerConfig) -> DiscoveryDiff {
    use rayon::prelude::*;
    let roots: Vec<String> = config
        .paths
        .iter()
        .filter_map(|p| std::fs::canonicalize(crate::config::expand_path(p, None)).ok())
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let walked: HashSet<PathBuf> =
        FileDiscoveryIterator::new(&config.discovery_config(roots.clone()))
            .filter(|p| !config.is_file_excluded(p))
            .map(|p| p.canonicalize().unwrap_or(p))
            .collect();
    let indexed: HashSet<PathBuf> = {
        let e = read(engine);
        roots
            .iter()
            .flat_map(|r| e.file_ids_under(std::path::Path::new(r)))
            .filter_map(|id| e.file_store.get_path(id).map(|p| p.to_path_buf()))
            .collect()
    };
    // Discovered but not indexed: read it to see whether the indexer would
    // index it (it skips binary content and, without transcoding, non-UTF-8).
    let candidates: Vec<&PathBuf> = walked.difference(&indexed).collect();
    let classified: Vec<(PathBuf, Option<bool>)> = candidates
        .par_iter()
        .map(|p| {
            let state = match PartialIndexedFile::process_checked(
                p,
                config.transcode_non_utf8,
                config.max_file_size,
            ) {
                Ok(Some(_)) => Some(true),
                Ok(None) => Some(false),
                Err(_) => None,
            };
            ((*p).clone(), state)
        })
        .collect();
    let mut diff = DiscoveryDiff::default();
    let mut not_indexable = 0;
    for (path, state) in classified {
        match state {
            Some(true) => diff.missing.push(path),
            Some(false) => not_indexable += 1,
            None => diff.unreadable.push(path),
        }
    }
    diff.on_disk = walked.len() - not_indexable;
    diff.extra = indexed.difference(&walked).cloned().collect();
    diff.missing.sort();
    diff.extra.sort();
    diff.unreadable.sort();
    diff
}

/// Settings for [`run_scrub`] (`[diagnostics]` scrub keys).
#[derive(Debug, Clone)]
pub struct ScrubConfig {
    /// Files read and checked per minute (0: no content checks).
    pub files_per_minute: usize,
    /// How often the invariants are checked (`None`: never).
    pub invariants_every: Option<Duration>,
    /// How long a file must stay out of step before it counts as drift
    /// (a change being applied settles well within the default minute).
    pub suspect_for: Duration,
}

/// How long a file that looks out of step must stay so before it counts:
/// a change being applied, or about to be, settles well within this.
pub const SUSPECT_FOR: Duration = Duration::from_secs(60);

/// What the background scrub has checked and found since the server
/// started. Drift is the index disagreeing with the disk for longer than a
/// change takes to apply: a lost change. Each one is logged and queued for
/// the watcher to index again.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ScrubStats {
    /// The scrub runs.
    pub enabled: bool,
    /// Files read and checked.
    pub files_checked: u64,
    /// Passes over the whole index completed.
    pub passes: u64,
    /// Lasting differences found, by kind (`changed`, `gone`,
    /// `missing_trigrams`, `not_indexable`).
    pub drift: BTreeMap<String, u64>,
    /// Files queued for the watcher to index again.
    pub requeued: u64,
    /// Invariant checks run.
    pub invariant_checks: u64,
    /// What the last invariant check found, by kind (empty: clean).
    pub invariant_violations: BTreeMap<String, usize>,
    /// How long the last invariant check took (it holds a read lock).
    pub last_invariant_check_ms: u64,
}

/// Check the index a little at a time until `shutdown`: `files_per_minute`
/// files a minute, round the index, and the invariants every
/// `invariants_every`. Runs on its own thread; holds the engine's read lock
/// for one small group of files at a time.
pub fn run_scrub(
    engine: Arc<RwLock<SearchEngine>>,
    cfg: ScrubConfig,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
) {
    use std::sync::atomic::Ordering;
    let stats = read(&engine).watch_backlog.scrub.clone();
    stats.lock().unwrap_or_else(|p| p.into_inner()).enabled = true;
    let tick = Duration::from_secs(1);
    let mut ids: Vec<u32> = Vec::new();
    let mut cursor = 0usize;
    let mut budget = 0f64;
    let mut suspects: std::collections::HashMap<u32, (Instant, FileCheck)> = Default::default();
    let mut last_invariants = Instant::now();
    while !shutdown.load(Ordering::Acquire) {
        std::thread::sleep(tick);
        let (building, watching) = {
            let e = read(&engine);
            (e.building, e.watching)
        };
        if building {
            continue;
        }
        if let Some(every) = cfg.invariants_every {
            if last_invariants.elapsed() >= every {
                last_invariants = Instant::now();
                let started = Instant::now();
                let report = read(&engine).check_invariants();
                let mut s = stats.lock().unwrap_or_else(|p| p.into_inner());
                s.invariant_checks += 1;
                s.last_invariant_check_ms = started.elapsed().as_millis() as u64;
                s.invariant_violations = report.counts.clone();
                if !report.is_clean() {
                    tracing::warn!("Index scrub: {report}");
                }
            }
        }
        if cfg.files_per_minute == 0 {
            continue;
        }
        budget += cfg.files_per_minute as f64 / 60.0;
        let n = budget as usize;
        if n == 0 {
            continue;
        }
        budget -= n as f64;
        if cursor >= ids.len() {
            if !ids.is_empty() {
                stats.lock().unwrap_or_else(|p| p.into_inner()).passes += 1;
            }
            ids = read(&engine).live_file_ids();
            cursor = 0;
        }
        let batch: Vec<u32> = ids[cursor..(cursor + n).min(ids.len())].to_vec();
        cursor += batch.len();
        // Files that looked out of step a while ago, checked again.
        let due: Vec<u32> = suspects
            .iter()
            .filter(|(_, (since, _))| since.elapsed() >= cfg.suspect_for)
            .map(|(id, _)| *id)
            .collect();
        for id in &due {
            suspects.remove(id);
        }
        stats
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .files_checked += (batch.len() + due.len()) as u64;
        for (id, path, check) in check_files(&engine, &due) {
            if watching && check != FileCheck::Unreadable {
                record_drift(&engine, &stats, id, &path, check);
            }
        }
        for (id, path, check) in check_files(&engine, &batch) {
            match check {
                // A search would skip this file: fix it now.
                FileCheck::MissingTrigrams(_) => record_drift(&engine, &stats, id, &path, check),
                // Retried and reported by the read-failure handling.
                FileCheck::Unreadable => {}
                // Without a watcher, changes since the last scan are
                // expected (and searches say so).
                _ if !watching => {}
                _ => {
                    suspects.entry(id).or_insert((Instant::now(), check));
                }
            }
        }
    }
}

/// Count a lasting difference, log it and queue the file for the watcher.
fn record_drift(
    engine: &Arc<RwLock<SearchEngine>>,
    stats: &Arc<std::sync::Mutex<ScrubStats>>,
    id: u32,
    path: &str,
    check: FileCheck,
) {
    let Some(kind) = file_check_kind(check) else {
        return;
    };
    tracing::warn!(
        file = path,
        kind,
        "Index scrub: the index is out of step with this file; indexing it again"
    );
    let e = read(engine);
    let mut s = stats.lock().unwrap_or_else(|p| p.into_inner());
    *s.drift.entry(kind.to_string()).or_default() += 1;
    if let Some(p) = e.file_store.get_path(id) {
        e.watch_backlog
            .requeue
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(p.to_path_buf());
        s.requeued += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::{create_progress_broadcaster, run_background_indexer};
    use crate::search::{BackgroundIndexerConfig, IndexingProgress};

    fn build(config: &IndexerConfig) -> Arc<RwLock<SearchEngine>> {
        let engine = Arc::new(RwLock::new(SearchEngine::new()));
        run_background_indexer(BackgroundIndexerConfig {
            indexer_config: config.clone(),
            engine: engine.clone(),
            progress: Arc::new(RwLock::new(IndexingProgress::default())),
            progress_tx: create_progress_broadcaster(),
            shutdown: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        });
        engine
    }

    /// Clean after a build; then, with nothing watching, a changed, a
    /// deleted and a new file are each reported under their kind, and the
    /// text output names no file unless asked to.
    #[test]
    fn check_reports_each_kind_and_hides_paths_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        for i in 0..20 {
            std::fs::write(root.join(format!("m{i}.py")), format!("x = {i}\n")).unwrap();
        }
        std::fs::write(root.join("blob.py"), b"\x00\x01binary").unwrap();
        let config = IndexerConfig {
            paths: vec![root.to_string_lossy().to_string()],
            watch: false,
            ..Default::default()
        };
        let engine = build(&config);
        let opts = IntegrityOptions {
            content_files: None,
            ..Default::default()
        };
        let report = check(&engine, &config, &opts);
        assert_eq!(report.problems(), 0, "{}", report.render(true));
        assert_eq!((report.files, report.on_disk), (20, Some(20)));
        assert_eq!(report.content_checked, 20);

        std::fs::write(root.join("m1.py"), "x = 'a longer line'\n").unwrap();
        std::fs::remove_file(root.join("m2.py")).unwrap();
        std::fs::write(root.join("new.py"), "y = 1\n").unwrap();
        let report = check(&engine, &config, &opts);
        assert_eq!(report.content.get("changed"), Some(&1));
        assert_eq!(report.content.get("gone"), Some(&1));
        assert_eq!(report.discovery.get("missing"), Some(&1));
        assert_eq!(report.discovery.get("extra"), Some(&1));
        assert_eq!(report.problems(), 4);
        let text = report.render(false);
        assert!(
            !text.contains("m1.py") && !text.contains("new.py"),
            "{text}"
        );
        assert!(text.contains("file watching is off"), "{text}");
        let text = report.render(true);
        assert!(text.contains("m1.py") && text.contains("new.py"), "{text}");

        // A sample spreads over the index.
        let sampled = check(
            &engine,
            &config,
            &IntegrityOptions {
                content_files: Some(5),
                discovery: false,
                examples: 5,
            },
        );
        assert_eq!((sampled.content_checked, sampled.on_disk), (5, None));
    }
}
