pub mod background_indexer;
pub mod engine;
pub mod engine_lock;
pub mod file_discovery;
pub mod fold;
pub mod incremental;
pub mod integrity;
pub mod path_filter;
pub mod query_syntax;
pub mod ranking;
pub mod regex_search;
pub mod request;
pub mod watch_loop;
pub mod watcher;

pub use background_indexer::{
    run as run_background_indexer, save_after_watcher_shutdown, save_on_watcher_update,
    BackgroundIndexerConfig,
};
pub use engine::{
    create_progress_broadcaster, unix_now_ms, BudgetRule, FileCheck, FolderProblems, GraphScope,
    ImpactOptions, ImpactReport, IndexFreshness, IndexingProgress, IndexingStatus, InvariantReport,
    LoadIndexResult, LoadingPhase, PartialIndexedFile, PathFreshness, PathMatch, PathQuery,
    PathSearch, PreIndexedFile, ProgressBroadcaster, QueryRun, RankMode, ReadFailures,
    SearchEngine, SearchLimit, SearchLimits, SearchMatch, SearchRankingInfo, SearchSettings,
    SearchStats, SharedIndexingProgress, WatchBacklog,
};
pub use file_discovery::{
    discover_files, EligibilityProbe, FileDiscoveryConfig, FileDiscoveryIterator,
};
pub use incremental::{apply_change, apply_changes, ChangeOutcome};
pub use path_filter::PathFilter;
pub use query_syntax::{
    parse as parse_query, GraphDirection, GraphFilter, ParsedQuery, SearchOptions,
};
pub use ranking::{FileScoreWeights, RankingWeights};
pub use regex_search::{pattern_with_case as regex_pattern_with_case, RegexAnalysis};
pub use request::{SearchMode, SearchOutcome, SearchRequest};
pub use watch_loop::WatchLoopConfig;
pub use watcher::{FileChange, FileWatcher, WatcherConfig};
