pub mod background_indexer;
pub mod engine;
pub mod file_discovery;
pub mod path_filter;
pub mod regex_search;
pub mod watcher;

pub use background_indexer::{run as run_background_indexer, BackgroundIndexerConfig};
pub use engine::{
    create_progress_broadcaster, IndexingProgress, IndexingStatus, LoadIndexResult, LoadingPhase,
    PreIndexedFile, ProgressBroadcaster, SearchEngine, SearchMatch, SearchStats,
    SharedIndexingProgress,
};
pub use file_discovery::{discover_files, FileDiscoveryConfig, FileDiscoveryIterator};
pub use path_filter::PathFilter;
pub use regex_search::RegexAnalysis;
pub use watcher::{FileChange, FileWatcher, WatcherConfig};
