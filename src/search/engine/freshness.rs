//! How current the index is compared with the files on disk.
//!
//! With the file watcher off, nothing written after the last scan reaches
//! the index, while `read_file` / the file view read the disk directly. The
//! types here let every client (web UI, `fcs`, MCP) say so instead of
//! silently missing a fresh edit.

use super::*;
use crate::index::persistence::mtime_secs_of;

/// When the index last matched the disk, and whether a watcher keeps it so.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IndexFreshness {
    /// A file watcher applies changes as they happen.
    pub watching: bool,
    /// When the last full scan of the indexed folders started (Unix millis);
    /// absent while the first build or reconcile is still running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synced_at_ms: Option<u64>,
    /// When the watcher last changed the index (Unix millis).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at_ms: Option<u64>,
}

impl IndexFreshness {
    /// One line warning that results may miss recent writes, or `None` while
    /// a watcher keeps the index current or no scan has completed (an engine
    /// built in-process, or the first build, which progress already reports).
    pub fn note(&self, now_ms: u64) -> Option<String> {
        if self.watching {
            return None;
        }
        let at = self.synced_at_ms?;
        Some(format!(
            "Index last synced with disk {} ({}); file watching is off, so files written \
             since then may be missing or out of date.",
            format_age(now_ms.saturating_sub(at)),
            format_utc(at)
        ))
    }

    /// One line for status output: watching state plus the last sync.
    pub fn summary(&self, now_ms: u64) -> String {
        let synced = match self.synced_at_ms {
            Some(at) => format!(
                "last synced with disk {} ({})",
                format_age(now_ms.saturating_sub(at)),
                format_utc(at)
            ),
            None => "first build still running".to_string(),
        };
        let updated = match self.updated_at_ms {
            Some(at) if self.watching => format!(
                ", last change applied {}",
                format_age(now_ms.saturating_sub(at))
            ),
            _ => String::new(),
        };
        format!(
            "index {synced}, file watching {}{updated}",
            if self.watching { "on" } else { "off" }
        )
    }
}

/// Whether one path's indexed content still matches the file on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathFreshness {
    /// Indexed, and the file's modification time and size are unchanged.
    Current,
    /// Indexed, but the file changed on disk after it was indexed.
    Stale,
    /// Indexed, but the file no longer exists.
    Deleted,
    /// On disk but not in the index: written after the last sync, or
    /// excluded by the configuration.
    NotIndexed,
    /// Neither in the index nor on disk.
    NotFound,
    /// Indexed, but what was indexed was not recorded, so it can't be compared.
    Unknown,
}

impl PathFreshness {
    /// Short explanation for people and agents.
    pub fn describe(self) -> &'static str {
        match self {
            Self::Current => "current: unchanged since it was indexed",
            Self::Stale => "stale: changed on disk after it was indexed",
            Self::Deleted => "deleted: still in the index but gone from disk",
            Self::NotIndexed => {
                "not indexed: exists on disk but is not in the index (written after the last \
                 sync, or excluded by the configuration)"
            }
            Self::NotFound => "not found: neither in the index nor under an indexed folder",
            Self::Unknown => "indexed, but its indexed version was not recorded",
        }
    }
}

impl SearchEngine {
    /// When the index last matched the disk.
    pub fn freshness(&self) -> IndexFreshness {
        IndexFreshness {
            watching: self.watching,
            synced_at_ms: self.synced_at_ms,
            updated_at_ms: self.updated_at_ms,
        }
    }

    /// Compare one path's indexed (mtime, size) with the file on disk.
    ///
    /// `path` is absolute, a display path as results print it, or relative
    /// to an indexed folder. Unlike [`Self::find_file_id`] there is no
    /// suffix fallback: a new `src/x.rs` must not match some other indexed
    /// `.../src/x.rs` and be reported current.
    pub fn path_freshness(&self, path: &str) -> PathFreshness {
        let candidates = self.disk_candidates(path);
        let id = candidates
            .iter()
            .find_map(|p| self.find_file_id_exact(p))
            .or_else(|| self.find_by_display_path(path));
        let Some(id) = id else {
            return if candidates.iter().any(|p| p.is_file()) {
                PathFreshness::NotIndexed
            } else {
                PathFreshness::NotFound
            };
        };
        let Some(file) = self.file_store.get(id) else {
            return PathFreshness::NotFound;
        };
        let (mtime, size) = self
            .indexed_meta
            .get(id as usize)
            .copied()
            .unwrap_or((0, 0));
        match std::fs::metadata(&*file.path) {
            Err(_) => PathFreshness::Deleted,
            Ok(_) if (mtime, size) == (0, 0) => PathFreshness::Unknown,
            Ok(m) if mtime_secs_of(&m) == mtime && m.len() == size => PathFreshness::Current,
            Ok(_) => PathFreshness::Stale,
        }
    }

    /// Where `path` would be on disk: itself when absolute, otherwise under
    /// each indexed folder, both as written and with a leading root-folder
    /// name removed (the display form).
    fn disk_candidates(&self, path: &str) -> Vec<PathBuf> {
        let p = Path::new(path);
        if p.is_absolute() {
            return vec![p.to_path_buf()];
        }
        let display = path.replace('\\', "/");
        let mut out = Vec::new();
        for root in &self.root_paths {
            out.push(root.join(&display));
            if let Some(rest) = root
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|name| display.strip_prefix(name))
                .and_then(|r| r.strip_prefix('/'))
            {
                out.push(root.join(rest));
            }
        }
        out
    }
}

/// Milliseconds since the Unix epoch.
pub fn unix_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn format_age(ms: u64) -> String {
    let secs = ms / 1000;
    match secs {
        0..=59 => "just now".to_string(),
        60..=3599 => format!("{} min ago", secs / 60),
        3600..=86_399 => format!("{} h {} min ago", secs / 3600, secs % 3600 / 60),
        _ => format!("{} days ago", secs / 86_400),
    }
}

fn format_utc(ms: u64) -> String {
    chrono::DateTime::from_timestamp_millis(ms as i64)
        .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| "unknown time".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_is_silent_while_watching() {
        let f = IndexFreshness {
            watching: true,
            synced_at_ms: Some(0),
            updated_at_ms: None,
        };
        assert_eq!(f.note(1_000_000), None);
    }

    #[test]
    fn note_reports_age_and_time_when_not_watching() {
        let at = 1_760_000_000_000; // 2025-10-09 08:53 UTC
        let f = IndexFreshness {
            watching: false,
            synced_at_ms: Some(at),
            updated_at_ms: None,
        };
        let note = f.note(at + 6 * 60_000).unwrap();
        assert!(note.contains("6 min ago"), "{note}");
        assert!(note.contains("2025-10-09 08:53 UTC"), "{note}");
        assert!(note.contains("watching is off"), "{note}");
        assert_eq!(IndexFreshness::default().note(at), None);
    }

    #[test]
    fn path_freshness_tracks_edits_new_files_and_deletes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let a = root.join("a.rs");
        std::fs::write(&a, "fn a() {}\n").unwrap();
        let mut engine = SearchEngine::new();
        engine.add_root_path(&root);
        engine.update_files(std::slice::from_ref(&a));
        let root_name = root.file_name().unwrap().to_str().unwrap().to_string();

        assert_eq!(engine.path_freshness("a.rs"), PathFreshness::Current);
        assert_eq!(
            engine.path_freshness(&format!("{root_name}/a.rs")),
            PathFreshness::Current
        );
        assert_eq!(
            engine.path_freshness(a.to_str().unwrap()),
            PathFreshness::Current
        );

        std::fs::write(&a, "fn a() { changed(); }\n").unwrap();
        assert_eq!(engine.path_freshness("a.rs"), PathFreshness::Stale);

        std::fs::write(root.join("b.rs"), "fn b() {}\n").unwrap();
        assert_eq!(engine.path_freshness("b.rs"), PathFreshness::NotIndexed);
        // No suffix fallback: a new file must not match an indexed one.
        assert_eq!(engine.path_freshness("x/a.rs"), PathFreshness::NotFound);

        std::fs::remove_file(&a).unwrap();
        assert_eq!(engine.path_freshness("a.rs"), PathFreshness::Deleted);
    }
}
