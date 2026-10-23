//! Fault injection for tests: make a named point in the code fail on purpose.
//!
//! Some failures cannot be produced from outside the process on demand (a
//! read that fails once and then works, a panic half way through applying a
//! watcher burst), and tests running as root cannot make a file unreadable
//! with `chmod`. A fault point is a call such as
//! `faults::io("parse.read", path)?` at the place the failure would happen.
//!
//! Without the `failpoints` cargo feature every point compiles to nothing.
//! With it, a test arms a point with [`set`]; the point then fails for the
//! paths it names, as many times as asked. CI runs the fault tests with
//! `cargo test --features failpoints --test fault_injection`.
//!
//! Points:
//! - `parse.read`: reading a file to index it (initial build and watcher).
//! - `watch.apply`: installing a parsed watcher burst (panics).
//! - `watch.lose`: the watch loop drops a change event, as a backend that
//!   loses events would (the background scrub must find and fix it).

use std::path::Path;

/// What an armed point does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Return an I/O error of this kind.
    IoError(std::io::ErrorKind),
    /// Panic.
    Panic,
}

/// An armed fault point.
#[derive(Debug, Clone)]
pub struct Fault {
    /// What the point does when it fires.
    pub action: Action,
    /// Fire only for paths containing this text (any path when `None`).
    pub path_contains: Option<String>,
    /// Fire this many times, then stop (`None`: every time).
    pub times: Option<usize>,
}

#[cfg(feature = "failpoints")]
mod registry {
    use super::*;
    use std::sync::Mutex;

    static POINTS: Mutex<Vec<(String, Fault)>> = Mutex::new(Vec::new());

    pub fn set(name: &str, fault: Fault) {
        POINTS
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push((name.to_string(), fault));
    }

    pub fn clear(name: &str, path_contains: Option<&str>) {
        POINTS
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .retain(|(n, f)| !(n == name && f.path_contains.as_deref() == path_contains));
    }

    pub fn fire(name: &str, path: &Path) -> Option<Action> {
        let mut points = POINTS.lock().unwrap_or_else(|p| p.into_inner());
        let path = path.to_string_lossy();
        let i = points.iter().position(|(n, f)| {
            n == name
                && f.times != Some(0)
                && f.path_contains.as_deref().is_none_or(|p| path.contains(p))
        })?;
        let fault = &mut points[i].1;
        if let Some(t) = fault.times.as_mut() {
            *t -= 1;
        }
        Some(fault.action)
    }
}

/// Arm `name`. Several faults can be armed on one point (for different
/// paths); the first that matches fires.
#[cfg(feature = "failpoints")]
pub fn set(name: &str, fault: Fault) {
    registry::set(name, fault);
}

/// Disarm the faults on `name` armed with this `path_contains`.
#[cfg(feature = "failpoints")]
pub fn clear(name: &str, path_contains: Option<&str>) {
    registry::clear(name, path_contains);
}

/// An I/O fault point: `Err` when `name` is armed for `path` with an
/// [`Action::IoError`]; panics when armed with [`Action::Panic`].
#[inline]
pub fn io(name: &str, path: &Path) -> std::io::Result<()> {
    #[cfg(feature = "failpoints")]
    match registry::fire(name, path) {
        Some(Action::IoError(kind)) => {
            return Err(std::io::Error::new(kind, format!("injected fault {name}")))
        }
        Some(Action::Panic) => panic!("injected fault {name} at {}", path.display()),
        None => {}
    }
    let _ = (name, path);
    Ok(())
}

/// A decision fault point: true when `name` is armed for `path` (any
/// action), for faults that change what the code does rather than fail.
#[inline]
pub fn fires(name: &str, path: &Path) -> bool {
    #[cfg(feature = "failpoints")]
    if registry::fire(name, path).is_some() {
        return true;
    }
    let _ = (name, path);
    false
}

/// A panic fault point: panics when `name` is armed for `path`.
#[inline]
pub fn panic_point(name: &str, path: &Path) {
    #[cfg(feature = "failpoints")]
    if registry::fire(name, path).is_some() {
        panic!("injected fault {name} at {}", path.display());
    }
    let _ = (name, path);
}
