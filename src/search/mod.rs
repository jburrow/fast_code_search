pub mod background_indexer;
pub mod engine;
pub mod file_discovery;
pub mod incremental;
pub mod path_filter;
pub mod query_syntax;
pub mod ranking;
pub mod regex_search;
pub mod watcher;

pub use background_indexer::{
    run as run_background_indexer, save_after_watcher_shutdown, save_on_watcher_update,
    BackgroundIndexerConfig,
};
pub use engine::{
    create_progress_broadcaster, IndexingProgress, IndexingStatus, LoadIndexResult, LoadingPhase,
    PartialIndexedFile, PreIndexedFile, ProgressBroadcaster, QueryRun, RankMode, SearchEngine,
    SearchLimits, SearchMatch, SearchRankingInfo, SearchStats, SharedIndexingProgress,
};
pub use file_discovery::{
    discover_files, EligibilityProbe, FileDiscoveryConfig, FileDiscoveryIterator,
};
pub use incremental::{apply_change, apply_changes, ChangeOutcome};
pub use path_filter::PathFilter;
pub use query_syntax::{parse as parse_query, ParsedQuery, SearchOptions};
pub use ranking::{FileScoreWeights, RankingWeights};
pub use regex_search::RegexAnalysis;
pub use watcher::{FileChange, FileWatcher, WatcherConfig};
