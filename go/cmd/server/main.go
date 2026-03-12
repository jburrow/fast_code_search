// fast_code_search_server is the primary binary.
// It starts the background indexer, REST API, and gRPC server.
// It is the Go equivalent of the Rust src/main.rs / fast_code_search_server binary.
package main

import (
	"context"
	"flag"
	"fmt"
	"log/slog"
	"net"
	"net/http"
	"os"
	"os/signal"
	"syscall"

	"google.golang.org/grpc"

	"github.com/jburrow/fast_code_search/internal/config"
	"github.com/jburrow/fast_code_search/internal/search"
	"github.com/jburrow/fast_code_search/internal/server"
	"github.com/jburrow/fast_code_search/internal/telemetry"
	"github.com/jburrow/fast_code_search/internal/web"
)

func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, "error:", err)
		os.Exit(1)
	}
}

func run() error {
	// ── CLI flags ─────────────────────────────────────────────────────────────
	configPath := flag.String("config", "", "Path to TOML configuration file")
	grpcAddr   := flag.String("grpc-addr", "", "Override gRPC listen address (host:port)")
	webAddr    := flag.String("web-addr", "", "Override REST/web listen address (host:port)")
	indexPaths := flag.String("index", "", "Comma-separated paths to index (overrides config)")
	genConfig  := flag.Bool("gen-config", false, "Write a config template and exit")
	verbose    := flag.Bool("v", false, "Enable verbose/debug logging")
	flag.Parse()

	// ── Logging ───────────────────────────────────────────────────────────────
	logLevel := slog.LevelInfo
	if *verbose {
		logLevel = slog.LevelDebug
	}
	slog.SetDefault(slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: logLevel})))

	// ── Config template helper ────────────────────────────────────────────────
	if *genConfig {
		path := "fast_code_search.toml"
		if *configPath != "" {
			path = *configPath
		}
		if err := config.GenerateTemplate(path); err != nil {
			return fmt.Errorf("generating config: %w", err)
		}
		slog.Info("config template written", "path", path)
		return nil
	}

	// ── Load configuration ────────────────────────────────────────────────────
	cfgPath := "fast_code_search.toml"
	if *configPath != "" {
		cfgPath = *configPath
	}
	cfg, err := config.Load(cfgPath)
	if err != nil {
		return fmt.Errorf("loading config: %w", err)
	}

	// Apply CLI overrides.
	if *grpcAddr != "" {
		cfg.Server.GRPCAddr = *grpcAddr
	}
	if *webAddr != "" {
		cfg.Server.WebAddr = *webAddr
	}
	if *indexPaths != "" {
		var paths []string
		for _, p := range splitCSV(*indexPaths) {
			if p != "" {
				paths = append(paths, p)
			}
		}
		if len(paths) > 0 {
			cfg.Indexer.Paths = paths
		}
	}

	// ── Telemetry ─────────────────────────────────────────────────────────────
	tp := telemetry.Init(&cfg.Telemetry)
	defer func() {
		ctx := context.Background()
		_ = tp.Shutdown(ctx)
	}()

	// ── Search engine ─────────────────────────────────────────────────────────
	engine := search.NewEngine(cfg)

	// Attempt to restore a previously persisted index.
	if cfg.Indexer.PersistIndex {
		if err := engine.LoadIndex(); err != nil {
			slog.Info("no persisted index found, will build fresh", "reason", err)
		} else {
			slog.Info("loaded persisted index", "files", engine.Stats().FilesIndexed)
		}
	}

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// ── Background indexer ────────────────────────────────────────────────────
	bi := search.NewBackgroundIndexer(engine, cfg)
	go func() {
		if err := bi.Run(ctx); err != nil && err != context.Canceled {
			slog.Error("background indexer exited", "err", err)
		}
	}()

	// Log indexing progress.
	go func() {
		for p := range bi.Progress() {
			if p.Done {
				slog.Info("indexing complete", "files", p.FilesIndexed)
				break
			}
			if p.TotalFiles > 0 {
				slog.Debug("indexing progress",
					"done", p.FilesIndexed,
					"total", p.TotalFiles,
				)
			}
		}
	}()

	// ── gRPC server ───────────────────────────────────────────────────────────
	grpcLis, err := net.Listen("tcp", cfg.Server.GRPCAddr)
	if err != nil {
		return fmt.Errorf("gRPC listen %s: %w", cfg.Server.GRPCAddr, err)
	}
	grpcSrv := grpc.NewServer()
	svc := server.NewCodeSearchService(engine)
	svc.Register(grpcSrv)

	go func() {
		slog.Info("gRPC server listening", "addr", cfg.Server.GRPCAddr)
		if err := grpcSrv.Serve(grpcLis); err != nil {
			slog.Error("gRPC server error", "err", err)
		}
	}()

	// ── REST / web server ─────────────────────────────────────────────────────
	httpSrv := &http.Server{
		Addr:    cfg.Server.WebAddr,
		Handler: web.Router(engine, cfg),
	}
	go func() {
		slog.Info("web server listening", "addr", cfg.Server.WebAddr)
		if err := httpSrv.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			slog.Error("web server error", "err", err)
		}
	}()

	// ── Graceful shutdown ─────────────────────────────────────────────────────
	quit := make(chan os.Signal, 1)
	signal.Notify(quit, syscall.SIGINT, syscall.SIGTERM)
	sig := <-quit
	slog.Info("shutting down", "signal", sig)

	cancel()
	grpcSrv.GracefulStop()
	shutCtx := context.Background()
	return httpSrv.Shutdown(shutCtx)
}

func splitCSV(s string) []string {
	var out []string
	for _, part := range splitOn(s, ',') {
		trimmed := trimSpace(part)
		if trimmed != "" {
			out = append(out, trimmed)
		}
	}
	return out
}

func splitOn(s string, sep rune) []string {
	var parts []string
	start := 0
	for i, r := range s {
		if r == sep {
			parts = append(parts, s[start:i])
			start = i + 1
		}
	}
	parts = append(parts, s[start:])
	return parts
}

func trimSpace(s string) string {
	result := s
	for len(result) > 0 && (result[0] == ' ' || result[0] == '\t') {
		result = result[1:]
	}
	for len(result) > 0 && (result[len(result)-1] == ' ' || result[len(result)-1] == '\t') {
		result = result[:len(result)-1]
	}
	return result
}
