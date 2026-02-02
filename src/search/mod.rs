pub mod engine;
pub mod path_filter;
pub mod regex_search;
pub mod watcher;

pub use engine::{
    IndexingProgress, IndexingStatus, LoadIndexResult, PreIndexedFile, SearchEngine, SearchMatch,
    SearchStats, SharedIndexingProgress,
};
pub use path_filter::PathFilter;
pub use regex_search::RegexAnalysis;
pub use watcher::{FileChange, FileWatcher, WatcherConfig};
