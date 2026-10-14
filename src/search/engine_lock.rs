//! Taking the engine lock without stalling the server.
//!
//! The engine sits behind a `std::sync::RwLock`. Searches used to give up
//! the moment a writer held it and answer "index is being updated, try
//! again"; with the watcher applying a stream of changes to a large tree
//! that was most of the time. Now the watcher holds the write lock only in
//! short steps, and a search waits a short while for it ([`read_within`])
//! instead of failing, so it is answered rather than turned away.

use crate::search::SearchEngine;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockError};
use std::time::{Duration, Instant};
use tracing::error;

/// How long a search waits for a writer (a watcher batch, a checkpoint)
/// before it is answered "index is being updated, try again".
pub const SEARCH_READ_WAIT: Duration = Duration::from_secs(5);

/// The longest sleep between attempts; the first ones are much shorter.
const MAX_BACKOFF: Duration = Duration::from_millis(10);

/// A read lock, waiting up to `wait` for a writer to finish. `None` when the
/// lock is still held after that. A poisoned lock is recovered: every
/// writer is wrapped in `catch_unwind`, so poison only means some thread
/// panicked, not that the index is unusable.
pub fn read_within(
    engine: &RwLock<SearchEngine>,
    wait: Duration,
) -> Option<RwLockReadGuard<'_, SearchEngine>> {
    let deadline = Instant::now() + wait;
    let mut backoff = Duration::from_micros(200);
    loop {
        match engine.try_read() {
            Ok(guard) => return Some(guard),
            Err(TryLockError::Poisoned(poisoned)) => {
                error!("Search engine lock was poisoned; recovering for read");
                return Some(poisoned.into_inner());
            }
            Err(TryLockError::WouldBlock) => {
                let now = Instant::now();
                if now >= deadline {
                    return None;
                }
                std::thread::sleep(backoff.min(deadline - now));
                backoff = (backoff * 2).min(MAX_BACKOFF);
            }
        }
    }
}

/// The write lock, recovering a poisoned lock.
///
/// Writers queue for it: once one is waiting, new searches wait behind it
/// (in [`read_within`]) until it has been granted and released, so writers
/// keep each hold short and pause between holds (see
/// [`crate::search::watch_loop::apply_burst`]).
pub fn write(engine: &RwLock<SearchEngine>) -> RwLockWriteGuard<'_, SearchEngine> {
    engine.write().unwrap_or_else(|poisoned| {
        error!("Search engine lock was poisoned; recovering for write");
        poisoned.into_inner()
    })
}
