//! Diagnostics and self-test module for Fast Code Search
//!
//! Provides health checks, self-tests, and rich diagnostics information
//! for both keyword and semantic search servers.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Server start time (set once at startup)
static SERVER_START_TIME: AtomicU64 = AtomicU64::new(0);

/// Initialize server start time. Call this once at server startup.
pub fn init_server_start_time() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    SERVER_START_TIME.store(now, Ordering::SeqCst);
}

/// Get server uptime in seconds
pub fn get_uptime_secs() -> u64 {
    let start = SERVER_START_TIME.load(Ordering::SeqCst);
    if start == 0 {
        return 0;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    now.saturating_sub(start)
}

/// Format uptime as human-readable string
pub fn format_uptime(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;

    if days > 0 {
        format!("{}d {}h {}m {}s", days, hours, minutes, seconds)
    } else if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

/// Overall health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// All systems operational
    Healthy,
    /// Some non-critical issues detected
    Degraded,
    /// Critical issues, service may be unusable
    Unhealthy,
}

impl HealthStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded => "degraded",
            HealthStatus::Unhealthy => "unhealthy",
        }
    }
}

/// Result of a single self-test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// Name of the test
    pub name: String,
    /// Test passed or failed
    pub passed: bool,
    /// Time taken to run the test in milliseconds
    pub duration_ms: f64,
    /// Descriptive message (especially useful for failures)
    pub message: String,
    /// Optional details (e.g., what was searched, what was found)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl TestResult {
    pub fn passed(name: impl Into<String>, duration: Duration, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed: true,
            duration_ms: duration.as_secs_f64() * 1000.0,
            message: message.into(),
            details: None,
        }
    }

    pub fn failed(name: impl Into<String>, duration: Duration, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed: false,
            duration_ms: duration.as_secs_f64() * 1000.0,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

/// Configuration summary for diagnostics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSummary {
    /// Paths being indexed
    pub indexed_paths: Vec<String>,
    /// File extensions included (empty = all)
    pub include_extensions: Vec<String>,
    /// Exclusion patterns
    pub exclude_patterns: Vec<String>,
    /// Maximum file size
    pub max_file_size_bytes: u64,
    /// Persistence path (if configured)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_path: Option<String>,
    /// File watcher enabled
    pub watch_enabled: bool,
}

impl From<&crate::config::IndexerConfig> for ConfigSummary {
    fn from(config: &crate::config::IndexerConfig) -> Self {
        Self {
            indexed_paths: config.paths.clone(),
            include_extensions: config.include_extensions.clone(),
            exclude_patterns: config.exclude_patterns.clone(),
            max_file_size_bytes: config.max_file_size,
            index_path: config.index_path.clone(),
            watch_enabled: config.watch,
        }
    }
}

/// File extension breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionBreakdown {
    pub extension: String,
    pub count: usize,
    pub total_bytes: u64,
}

/// Common diagnostics query parameters
#[derive(Debug, Deserialize)]
pub struct DiagnosticsQuery {
    /// Force refresh of cached diagnostics
    #[serde(default)]
    pub force_refresh: bool,
    /// Number of random files to sample for self-tests (default: 5)
    #[serde(default = "default_sample_count")]
    pub sample_count: usize,
}

fn default_sample_count() -> usize {
    5
}

// ============================================================================
// Keyword Search Diagnostics
// ============================================================================

/// Diagnostics response for keyword search server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeywordDiagnosticsResponse {
    /// Overall health status
    pub status: HealthStatus,
    /// Server version
    pub version: String,
    /// Server uptime in seconds
    pub uptime_secs: u64,
    /// Human-readable uptime
    pub uptime_human: String,
    /// Timestamp when diagnostics were generated
    pub generated_at: String,
    /// Configuration summary
    pub config: ConfigSummary,
    /// Index statistics
    pub index: KeywordIndexDiagnostics,
    /// Self-test results
    pub self_tests: Vec<TestResult>,
    /// Overall test summary
    pub test_summary: TestSummary,
}

/// Keyword index diagnostics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeywordIndexDiagnostics {
    /// Number of indexed files
    pub num_files: usize,
    /// Total size of indexed content in bytes
    pub total_size_bytes: u64,
    /// Human-readable total size
    pub total_size_human: String,
    /// Number of unique trigrams
    pub num_trigrams: usize,
    /// Number of dependency edges (import relationships)
    pub dependency_edges: usize,
    /// Breakdown by file extension
    pub files_by_extension: Vec<ExtensionBreakdown>,
    /// Sample of indexed file paths (for verification)
    pub sample_files: Vec<String>,
}

/// Summary of test results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub total_duration_ms: f64,
}

impl TestSummary {
    pub fn from_results(results: &[TestResult]) -> Self {
        let passed = results.iter().filter(|r| r.passed).count();
        let total_duration_ms: f64 = results.iter().map(|r| r.duration_ms).sum();
        Self {
            total: results.len(),
            passed,
            failed: results.len() - passed,
            total_duration_ms,
        }
    }
}

/// Format bytes as human-readable string.
/// Re-exported from `crate::utils::format_bytes` to avoid duplication.
pub use crate::utils::format_bytes;

// ============================================================================
// Semantic Search Diagnostics
// ============================================================================

/// Diagnostics response for semantic search server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticDiagnosticsResponse {
    /// Overall health status
    pub status: HealthStatus,
    /// Server version
    pub version: String,
    /// Server uptime in seconds
    pub uptime_secs: u64,
    /// Human-readable uptime
    pub uptime_human: String,
    /// Timestamp when diagnostics were generated
    pub generated_at: String,
    /// Configuration summary
    pub config: ConfigSummary,
    /// Semantic index statistics
    pub index: SemanticIndexDiagnostics,
    /// Embedding model information
    pub model: ModelDiagnostics,
    /// Self-test results
    pub self_tests: Vec<TestResult>,
    /// Overall test summary
    pub test_summary: TestSummary,
}

/// Semantic index diagnostics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticIndexDiagnostics {
    /// Number of indexed files
    pub num_files: usize,
    /// Number of code chunks
    pub num_chunks: usize,
    /// Embedding dimension
    pub embedding_dim: usize,
    /// Query cache size
    pub cache_size: usize,
    /// Cache hit rate (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_hit_rate: Option<f64>,
    /// Breakdown by chunk type
    pub chunks_by_type: ChunkTypeBreakdown,
    /// Sample of indexed file paths (for verification)
    pub sample_files: Vec<String>,
}

/// Breakdown of chunks by type
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChunkTypeBreakdown {
    pub functions: usize,
    pub classes: usize,
    pub modules: usize,
    pub fixed: usize,
}

/// Embedding model diagnostics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDiagnostics {
    /// Model name/type
    pub name: String,
    /// Whether the model is loaded and functional
    pub loaded: bool,
    /// Embedding dimension
    pub embedding_dim: usize,
    /// Model type (ONNX, TF-IDF, etc.)
    pub model_type: String,
}

/// Get current timestamp as ISO 8601 string
pub fn get_timestamp() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    // Format as ISO 8601
    let secs = now.as_secs();
    let datetime = chrono::DateTime::from_timestamp(secs as i64, 0)
        .unwrap_or(chrono::DateTime::<chrono::Utc>::UNIX_EPOCH);
    datetime.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_uptime() {
        assert_eq!(format_uptime(0), "0s");
        assert_eq!(format_uptime(30), "30s");
        assert_eq!(format_uptime(90), "1m 30s");
        assert_eq!(format_uptime(3661), "1h 1m 1s");
        assert_eq!(format_uptime(90061), "1d 1h 1m 1s");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 bytes");
        assert_eq!(format_bytes(500), "500 bytes");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
        assert_eq!(format_bytes(1073741824), "1.00 GB");
    }

    #[test]
    fn test_test_result_creation() {
        let duration = Duration::from_millis(100);
        let passed = TestResult::passed("test1", duration, "Success");
        assert!(passed.passed);
        assert_eq!(passed.name, "test1");
        assert!((passed.duration_ms - 100.0).abs() < 0.1);

        let failed = TestResult::failed("test2", duration, "Failed").with_details("details here");
        assert!(!failed.passed);
        assert_eq!(failed.details, Some("details here".to_string()));
    }

    #[test]
    fn test_test_summary() {
        let results = vec![
            TestResult::passed("t1", Duration::from_millis(10), "ok"),
            TestResult::passed("t2", Duration::from_millis(20), "ok"),
            TestResult::failed("t3", Duration::from_millis(30), "fail"),
        ];
        let summary = TestSummary::from_results(&results);
        assert_eq!(summary.total, 3);
        assert_eq!(summary.passed, 2);
        assert_eq!(summary.failed, 1);
        assert!((summary.total_duration_ms - 60.0).abs() < 0.1);
    }

    #[test]
    fn test_health_status_serialization() {
        assert_eq!(HealthStatus::Healthy.as_str(), "healthy");
        assert_eq!(HealthStatus::Degraded.as_str(), "degraded");
        assert_eq!(HealthStatus::Unhealthy.as_str(), "unhealthy");
    }
}
