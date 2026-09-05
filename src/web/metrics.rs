//! Prometheus-style counters for the REST API (`/metrics`).
//!
//! Hand-rendered text exposition (format 0.0.4) so no extra dependency is
//! needed; everything is an atomic, so recording is lock-free.

use std::sync::atomic::{AtomicU64, Ordering};

/// Histogram bucket upper bounds for search latency, in milliseconds.
const LATENCY_BUCKETS_MS: [u64; 11] = [1, 5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000];

/// Counters shared by all handlers.
#[derive(Debug, Default)]
pub struct Metrics {
    searches_total: AtomicU64,
    search_client_errors_total: AtomicU64,
    search_unavailable_total: AtomicU64,
    search_rejected_total: AtomicU64,
    latency_buckets: [AtomicU64; 11],
    latency_count: AtomicU64,
    latency_sum_us: AtomicU64,
}

impl Metrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// A completed search (any outcome).
    pub fn record_search(&self, elapsed: std::time::Duration) {
        self.searches_total.fetch_add(1, Ordering::Relaxed);
        let ms = elapsed.as_millis() as u64;
        for (i, &bound) in LATENCY_BUCKETS_MS.iter().enumerate() {
            if ms <= bound {
                self.latency_buckets[i].fetch_add(1, Ordering::Relaxed);
            }
        }
        self.latency_count.fetch_add(1, Ordering::Relaxed);
        self.latency_sum_us
            .fetch_add(elapsed.as_micros() as u64, Ordering::Relaxed);
    }

    /// A search rejected with 4xx (bad regex, bad filter).
    pub fn record_client_error(&self) {
        self.search_client_errors_total
            .fetch_add(1, Ordering::Relaxed);
    }

    /// A search that got 503 because the index was being written.
    pub fn record_unavailable(&self) {
        self.search_unavailable_total
            .fetch_add(1, Ordering::Relaxed);
    }

    /// A search that got 503 because the concurrency limit was reached.
    pub fn record_rejected(&self) {
        self.search_rejected_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Render everything, plus the given index gauges, in Prometheus text format.
    pub fn render(&self, index: &IndexGauges) -> String {
        use std::fmt::Write as _;
        let mut out = String::with_capacity(2048);
        let _ = writeln!(
            out,
            "# HELP fcs_search_requests_total Search requests handled."
        );
        let _ = writeln!(out, "# TYPE fcs_search_requests_total counter");
        let _ = writeln!(
            out,
            "fcs_search_requests_total {}",
            self.searches_total.load(Ordering::Relaxed)
        );
        let _ = writeln!(
            out,
            "# HELP fcs_search_errors_total Search requests that did not run, by reason."
        );
        let _ = writeln!(out, "# TYPE fcs_search_errors_total counter");
        let _ = writeln!(
            out,
            "fcs_search_errors_total{{reason=\"client\"}} {}",
            self.search_client_errors_total.load(Ordering::Relaxed)
        );
        let _ = writeln!(
            out,
            "fcs_search_errors_total{{reason=\"index_busy\"}} {}",
            self.search_unavailable_total.load(Ordering::Relaxed)
        );
        let _ = writeln!(
            out,
            "fcs_search_errors_total{{reason=\"concurrency_limit\"}} {}",
            self.search_rejected_total.load(Ordering::Relaxed)
        );
        let _ = writeln!(
            out,
            "# HELP fcs_search_duration_seconds Search latency (engine + serialization)."
        );
        let _ = writeln!(out, "# TYPE fcs_search_duration_seconds histogram");
        for (i, &bound) in LATENCY_BUCKETS_MS.iter().enumerate() {
            let _ = writeln!(
                out,
                "fcs_search_duration_seconds_bucket{{le=\"{}\"}} {}",
                bound as f64 / 1000.0,
                self.latency_buckets[i].load(Ordering::Relaxed)
            );
        }
        let count = self.latency_count.load(Ordering::Relaxed);
        let _ = writeln!(
            out,
            "fcs_search_duration_seconds_bucket{{le=\"+Inf\"}} {count}"
        );
        let _ = writeln!(
            out,
            "fcs_search_duration_seconds_sum {}",
            self.latency_sum_us.load(Ordering::Relaxed) as f64 / 1_000_000.0
        );
        let _ = writeln!(out, "fcs_search_duration_seconds_count {count}");

        let gauge = |out: &mut String, name: &str, help: &str, value: u64| {
            let _ = writeln!(out, "# HELP {name} {help}");
            let _ = writeln!(out, "# TYPE {name} gauge");
            let _ = writeln!(out, "{name} {value}");
        };
        gauge(
            &mut out,
            "fcs_index_files",
            "Live files in the index.",
            index.files,
        );
        gauge(
            &mut out,
            "fcs_index_trigrams",
            "Distinct trigrams.",
            index.trigrams,
        );
        gauge(
            &mut out,
            "fcs_index_dependency_edges",
            "Resolved import edges.",
            index.dependency_edges,
        );
        gauge(
            &mut out,
            "fcs_index_content_bytes",
            "Bytes of indexed text.",
            index.content_bytes,
        );
        gauge(
            &mut out,
            "fcs_indexing",
            "1 while a build/reconcile is running.",
            u64::from(index.indexing),
        );
        gauge(
            &mut out,
            "fcs_ready",
            "1 when the index is ready to serve.",
            u64::from(index.ready),
        );
        out
    }
}

/// Point-in-time index values rendered as gauges.
#[derive(Debug, Default, Clone, Copy)]
pub struct IndexGauges {
    pub files: u64,
    pub trigrams: u64,
    pub dependency_edges: u64,
    pub content_bytes: u64,
    pub indexing: bool,
    pub ready: bool,
}
