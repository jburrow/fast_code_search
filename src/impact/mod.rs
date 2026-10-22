//! Diff impact analysis: from a diff to the symbols it changes, and from
//! each symbol to the files that use it.
//!
//! The parts that need no index live here ([`diff`], [`defs`]); the search
//! over the index is [`crate::search::SearchEngine::impact`]. The method and
//! its measured accuracy: `docs/book/src/reference/impact.md`.

pub mod defs;
pub mod diff;
pub mod report;
