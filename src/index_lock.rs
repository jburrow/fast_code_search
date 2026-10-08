//! One server per index file.
//!
//! Two servers given the same `index_path` would each save over the other's
//! index. The server holds an exclusive lock on `<index_path>.lock` for its
//! whole run and writes its PID and addresses to `<index_path>.lock.owner`,
//! so a second server can say exactly which process already owns the index.
//! The owner text lives in its own file because a Windows lock also blocks
//! reading the locked file.
//!
//! The lock is an OS advisory lock (`flock` on Unix, `LockFileEx` on
//! Windows): it is released when the process exits, even after a crash or
//! SIGKILL, so a leftover `.lock` file never blocks a later start.

use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

/// Held for as long as this process owns the index.
#[derive(Debug)]
pub struct IndexLock {
    path: PathBuf,
    _file: File,
}

/// Why the lock could not be taken.
#[derive(Debug)]
pub enum LockError {
    /// Another process holds it. `owner` is what that process wrote into
    /// the owner file (PID and addresses), if it could be read.
    HeldByAnother {
        lock_path: PathBuf,
        owner: Option<String>,
    },
    /// The lock file could not be created or locked for another reason
    /// (permissions, read-only filesystem, no space or inodes).
    Io {
        lock_path: PathBuf,
        error: std::io::Error,
    },
}

impl std::fmt::Display for LockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LockError::HeldByAnother { lock_path, owner } => write!(
                f,
                "another fast_code_search server is already using this index ({}): {}",
                lock_path.display(),
                owner.as_deref().unwrap_or("owner unknown")
            ),
            LockError::Io { lock_path, error } => {
                write!(f, "cannot lock {}: {error}", lock_path.display())
            }
        }
    }
}

impl std::error::Error for LockError {}

/// `<index_path>.lock`
pub fn lock_path_for(index_path: &Path) -> PathBuf {
    with_suffix(index_path, ".lock")
}

/// `<index_path>.lock.owner`: who holds the lock.
pub fn owner_path_for(index_path: &Path) -> PathBuf {
    with_suffix(index_path, ".lock.owner")
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

impl IndexLock {
    /// Take the lock for `index_path` without waiting. `owner_info` is
    /// written into the lock file for a second server to report.
    pub fn acquire(index_path: &Path, owner_info: &str) -> Result<Self, LockError> {
        let lock_path = lock_path_for(index_path);
        let io_err = |error| LockError::Io {
            lock_path: lock_path.clone(),
            error,
        };
        if let Some(parent) = lock_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(io_err)?;
        }
        let owner_path = owner_path_for(index_path);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(io_err)?;
        if let Err(e) = file.try_lock_exclusive() {
            if e.kind() == std::io::ErrorKind::WouldBlock
                || e.raw_os_error() == fs2::lock_contended_error().raw_os_error()
            {
                let owner = std::fs::read_to_string(&owner_path)
                    .ok()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                return Err(LockError::HeldByAnother { lock_path, owner });
            }
            return Err(io_err(e));
        }
        // Best effort: the lock is what matters, the text only helps a
        // second server explain itself.
        let _ = std::fs::write(&owner_path, format!("{owner_info}\n"));
        Ok(Self {
            path: lock_path,
            _file: file,
        })
    }

    /// The lock file's path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// The text a server writes into its lock file.
pub fn describe_owner(grpc: &str, web: &str) -> String {
    let started = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!(
        "pid {} (gRPC {grpc}, web {web}), started at unix time {started}",
        std::process::id()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_holder_is_refused_and_told_who_owns_it() {
        let dir = tempfile::tempdir().unwrap();
        let index = dir.path().join("sub/work.fcsidx");
        let first = IndexLock::acquire(&index, "pid 1 (first)").unwrap();
        assert_eq!(first.path(), lock_path_for(&index));
        match IndexLock::acquire(&index, "pid 2 (second)") {
            Err(LockError::HeldByAnother { owner, .. }) => {
                assert_eq!(owner.as_deref(), Some("pid 1 (first)"));
            }
            other => panic!("expected HeldByAnother, got {other:?}"),
        }
        drop(first);
        let _again = IndexLock::acquire(&index, "pid 3 (third)").unwrap();
        let text = std::fs::read_to_string(owner_path_for(&index)).unwrap();
        assert_eq!(text.trim(), "pid 3 (third)");
    }

    #[test]
    fn lock_path_appends_suffix() {
        assert_eq!(
            lock_path_for(Path::new("/x/work.fcsidx")),
            PathBuf::from("/x/work.fcsidx.lock")
        );
        assert_eq!(
            owner_path_for(Path::new("/x/work.fcsidx")),
            PathBuf::from("/x/work.fcsidx.lock.owner")
        );
    }
}
