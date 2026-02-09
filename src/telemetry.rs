//! OpenTelemetry tracing integration for fast_code_search.
//!
//! Initializes a layered tracing subscriber that combines:
//! - A `fmt` layer for human-readable console output (always active)
//! - An OpenTelemetry tracing layer exporting spans via OTLP/gRPC (when enabled)
//!
//! Configuration is driven by the `TelemetryConfig` / `SemanticTelemetryConfig`
//! structs (loaded from TOML), with standard OTel environment variable overrides.

use anyhow::{Context, Result};
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::TracerProvider as SdkTracerProvider;
use opentelemetry_sdk::Resource;
use tracing::Level;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize the tracing subscriber with an optional OpenTelemetry OTLP layer.
///
/// When `enabled` is `true`, spans are exported to an OTLP/gRPC collector at
/// `otlp_endpoint` under `service_name`. Console (fmt) output is always active.
///
/// # Arguments
/// * `enabled` – whether to attach the OTel exporter layer
/// * `otlp_endpoint` – e.g. `"http://localhost:4317"`
/// * `service_name` – e.g. `"fast_code_search"`
/// * `log_level` – minimum tracing level for the fmt layer
///
/// # Errors
/// Returns an error if the OTLP exporter or tracer provider fails to initialise.
pub fn init_telemetry(
    enabled: bool,
    otlp_endpoint: &str,
    service_name: &str,
    log_level: Level,
) -> Result<()> {
    // Build the console (fmt) layer – always active
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false);

    // Build an env-filter that respects RUST_LOG, falling back to the CLI level
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level.to_string()));

    if enabled {
        // Build OTLP exporter targeting the configured gRPC endpoint
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(otlp_endpoint)
            .build()
            .context("Failed to build OTLP span exporter")?;

        let provider = SdkTracerProvider::builder()
            .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
            .with_resource(Resource::new(vec![opentelemetry::KeyValue::new(
                "service.name",
                service_name.to_owned(),
            )]))
            .build();

        let tracer = provider.tracer(service_name.to_owned());

        // Register the provider globally so shutdown can flush it
        opentelemetry::global::set_tracer_provider(provider);

        let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .with(otel_layer)
            .init();

        tracing::info!(
            otlp_endpoint = otlp_endpoint,
            service_name = service_name,
            "OpenTelemetry tracing enabled"
        );
    } else {
        // OTel disabled – console only
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();

        tracing::info!("OpenTelemetry tracing disabled, console-only logging active");
    }

    Ok(())
}

/// Flush pending spans and shut down the global tracer provider.
///
/// Call this during graceful shutdown (e.g. after receiving Ctrl-C) to ensure
/// all in-flight spans are exported before the process exits.
pub fn shutdown_telemetry() {
    opentelemetry::global::shutdown_tracer_provider();
    tracing::info!("OpenTelemetry tracer provider shut down");
}
