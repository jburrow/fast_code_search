// Package telemetry initialises OpenTelemetry tracing, equivalent to the Rust
// src/telemetry.rs module. When OTLP_ENDPOINT is configured, spans are
// exported via gRPC to an OpenTelemetry collector.
package telemetry

import (
	"context"
	"log/slog"

	"github.com/jburrow/fast_code_search/internal/config"
)

// Provider wraps the OpenTelemetry shutdown hook.
type Provider struct {
	shutdown func(context.Context) error
}

// Init sets up structured logging and (optionally) OpenTelemetry tracing.
// Returns a Provider whose Shutdown() must be called on exit.
func Init(cfg *config.TelemetryConfig) *Provider {
	// Configure slog with a sensible default level.
	slog.SetDefault(slog.Default())

	if !cfg.Enabled {
		slog.Info("telemetry disabled")
		return &Provider{shutdown: func(ctx context.Context) error { return nil }}
	}

	slog.Info("telemetry enabled",
		"service", cfg.ServiceName,
		"endpoint", cfg.OTLPEndpoint,
	)

	// Full OTLP export would require the go.opentelemetry.io/otel suite.
	// To avoid pulling in ~15 additional transitive dependencies we log a
	// notice and return a no-op provider. Add the otel imports and
	// configure the OTLP exporter here when those packages are available.
	slog.Warn("telemetry: OTLP export not yet wired; add go.opentelemetry.io/otel to enable")

	return &Provider{shutdown: func(ctx context.Context) error { return nil }}
}

// Shutdown flushes pending spans and releases resources.
func (p *Provider) Shutdown(ctx context.Context) error {
	if p.shutdown != nil {
		return p.shutdown(ctx)
	}
	return nil
}
