//! Free space and free inodes on the filesystems the server reads and writes.
//!
//! A filesystem can run out of inodes while `df -h` still shows free space;
//! writes then fail with "No space left on device", which reads like a full
//! disk. These checks report bytes and inodes separately so the log says
//! which one ran out.

use std::path::{Path, PathBuf};

/// Free and total bytes and inodes of one filesystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FsSpace {
    pub bytes_avail: u64,
    pub bytes_total: u64,
    /// `None` where the platform or filesystem does not report inodes
    /// (Windows; btrfs and some FUSE filesystems report a total of 0).
    pub inodes_avail: Option<u64>,
    pub inodes_total: Option<u64>,
}

/// Fewer free inodes than this is reported as nearly exhausted.
const MIN_FREE_INODES: u64 = 10_000;
/// Fewer free bytes than this is reported as nearly full.
const MIN_FREE_BYTES: u64 = 256 * 1024 * 1024;

impl FsSpace {
    /// Space on the filesystem holding `path`.
    #[cfg(unix)]
    pub fn of(path: &Path) -> std::io::Result<Self> {
        use std::os::unix::ffi::OsStrExt;
        let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
        let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
        // SAFETY: c_path is NUL-terminated and st is a valid out-pointer.
        if unsafe { libc::statvfs(c_path.as_ptr(), &mut st) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        #[allow(clippy::unnecessary_cast)] // field widths differ by platform
        let (frsize, bavail, blocks, favail, files) = (
            st.f_frsize as u64,
            st.f_bavail as u64,
            st.f_blocks as u64,
            st.f_favail as u64,
            st.f_files as u64,
        );
        let inodes_known = files > 0;
        Ok(Self {
            bytes_avail: bavail.saturating_mul(frsize),
            bytes_total: blocks.saturating_mul(frsize),
            inodes_avail: inodes_known.then_some(favail),
            inodes_total: inodes_known.then_some(files),
        })
    }

    /// Space on the filesystem holding `path`.
    #[cfg(not(unix))]
    pub fn of(path: &Path) -> std::io::Result<Self> {
        Ok(Self {
            bytes_avail: fs2::available_space(path)?,
            bytes_total: fs2::total_space(path)?,
            inodes_avail: None,
            inodes_total: None,
        })
    }

    /// Whether free inodes are exhausted or nearly so.
    pub fn inodes_low(&self) -> bool {
        match (self.inodes_avail, self.inodes_total) {
            (Some(avail), Some(total)) => avail < MIN_FREE_INODES.min(total / 100 + 1),
            _ => false,
        }
    }

    /// One-line summary, e.g. "12.3 GB of 931.5 GB free, 1200 of 61M inodes free".
    pub fn describe(&self) -> String {
        let mut s = format!(
            "{} of {} free",
            human_bytes(self.bytes_avail),
            human_bytes(self.bytes_total)
        );
        if let (Some(a), Some(t)) = (self.inodes_avail, self.inodes_total) {
            s.push_str(&format!(", {a} of {t} inodes free"));
        }
        s
    }
}

/// The nearest existing ancestor of `path` (the path itself if it exists),
/// so the filesystem of a file that will be created later can be checked.
pub fn existing_ancestor(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|p| !p.as_os_str().is_empty() && p.exists())
        .map(Path::to_path_buf)
}

/// Problems with the filesystem the index is saved to: too few free inodes,
/// or less free space than `need_bytes` (at least [`MIN_FREE_BYTES`]).
/// Empty when everything looks fine or nothing can be measured.
pub fn check_index_storage(index_path: &Path, need_bytes: u64) -> Vec<String> {
    let Some(dir) = existing_ancestor(index_path) else {
        return Vec::new();
    };
    let Ok(space) = FsSpace::of(&dir) else {
        return Vec::new();
    };
    let need = need_bytes.max(MIN_FREE_BYTES);
    let mut problems = Vec::new();
    if space.inodes_low() {
        problems.push(format!(
            "the filesystem holding the index ({}) is out of inodes: {}. Saving the \
             index will fail with \"No space left on device\" even though free space \
             is shown. Delete unneeded small files (caches, node_modules, build \
             output) there, or move index_path to another filesystem.",
            dir.display(),
            space.describe()
        ));
    }
    if space.bytes_avail < need {
        problems.push(format!(
            "the filesystem holding the index ({}) is nearly full: {}, and saving \
             needs about {}. Free some space or move index_path to another filesystem.",
            dir.display(),
            space.describe(),
            human_bytes(need)
        ));
    }
    problems
}

/// Problems with the filesystem holding an indexed source root. Only inode
/// exhaustion is reported: the index is not written there, but editors, git
/// and build tools on that filesystem start failing, and new files cannot be
/// created to be indexed.
pub fn check_source_storage(root: &Path) -> Option<String> {
    let space = FsSpace::of(root).ok()?;
    space.inodes_low().then(|| {
        format!(
            "the filesystem holding {} is out of inodes ({}): new files cannot be \
             created there, which shows up as \"No space left on device\"",
            root.display(),
            space.describe()
        )
    })
}

/// [`check_source_storage`] for each indexed root, reporting each
/// filesystem once even when several roots live on it.
pub fn check_source_roots<S: AsRef<str>>(roots: &[S]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    roots
        .iter()
        .map(|r| Path::new(r.as_ref()))
        .filter(|root| seen.insert(filesystem_id(root)))
        .filter_map(check_source_storage)
        .collect()
}

#[cfg(unix)]
fn filesystem_id(path: &Path) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path).ok().map(|m| m.dev())
}

#[cfg(not(unix))]
fn filesystem_id(path: &Path) -> Option<u64> {
    // Without a device id, fall back to checking every root.
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    Some(h.finish())
}

/// Every storage problem for this indexer config: the index filesystem
/// (out of inodes or space for another save) and the source filesystems
/// (out of inodes).
pub fn storage_problems(indexer: &crate::config::IndexerConfig) -> Vec<String> {
    let mut problems = Vec::new();
    if let Some(index_path) = &indexer.index_path {
        let path = Path::new(index_path);
        // Saving writes a new file beside the old one before replacing it.
        let need = std::fs::metadata(path).map(|m| m.len() * 2).unwrap_or(0);
        problems.extend(check_index_storage(path, need));
    }
    problems.extend(check_source_roots(&indexer.paths));
    problems
}

/// If `err` is a "no space left on device" error, explain whether bytes or
/// inodes ran out on the filesystem holding `path`.
pub fn explain_no_space(err: &anyhow::Error, path: &Path) -> Option<String> {
    let is_enospc = err.chain().any(|cause| {
        cause.downcast_ref::<std::io::Error>().is_some_and(|io| {
            io.kind() == std::io::ErrorKind::StorageFull || io.raw_os_error() == Some(28)
        })
    });
    if !is_enospc {
        return None;
    }
    let dir = existing_ancestor(path)?;
    let space = FsSpace::of(&dir).ok()?;
    Some(if space.inodes_low() {
        format!(
            "{} is out of inodes, not bytes ({}). Delete unneeded small files there \
             or move index_path to another filesystem.",
            dir.display(),
            space.describe()
        )
    } else {
        format!(
            "{} is out of space ({}). Free some space or move index_path.",
            dir.display(),
            space.describe()
        )
    })
}

fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut unit = 0;
    while v >= 1024.0 && unit < UNITS.len() - 1 {
        v /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn space(inodes_avail: u64, inodes_total: u64) -> FsSpace {
        FsSpace {
            bytes_avail: 10 << 30,
            bytes_total: 100 << 30,
            inodes_avail: Some(inodes_avail),
            inodes_total: Some(inodes_total),
        }
    }

    #[test]
    fn inodes_low_thresholds() {
        assert!(space(0, 1_000_000).inodes_low());
        assert!(space(5_000, 1_000_000).inodes_low());
        assert!(!space(50_000, 1_000_000).inodes_low());
        // A small filesystem is judged against 1% of its inodes.
        assert!(!space(200, 10_000).inodes_low());
        assert!(space(50, 10_000).inodes_low());
        let unknown = FsSpace {
            inodes_avail: None,
            inodes_total: None,
            ..space(0, 0)
        };
        assert!(!unknown.inodes_low());
    }

    #[test]
    fn measures_a_real_directory() {
        let dir = tempfile::tempdir().unwrap();
        let s = FsSpace::of(dir.path()).unwrap();
        assert!(s.bytes_total > 0);
        assert!(s.describe().contains("free"));
    }

    #[test]
    fn existing_ancestor_walks_up() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("a/b/index.fcsidx");
        assert_eq!(existing_ancestor(&missing).unwrap(), dir.path());
    }

    #[test]
    fn explains_enospc_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.fcsidx");
        let full = anyhow::Error::from(std::io::Error::from_raw_os_error(28));
        assert!(explain_no_space(&full, &path).is_some());
        let other = anyhow::anyhow!("permission denied");
        assert!(explain_no_space(&other, &path).is_none());
    }

    #[test]
    fn human_bytes_units() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(1536), "1.5 KB");
        assert_eq!(human_bytes(3 << 30), "3.0 GB");
    }
}
