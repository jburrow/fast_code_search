// Package server implements the gRPC CodeSearch service, equivalent to the
// Rust src/server/service.rs module.
package server

import (
	"context"
	"log/slog"

	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"

	pb "github.com/jburrow/fast_code_search/proto/search"
	"github.com/jburrow/fast_code_search/internal/search"
)

// CodeSearchService implements the pb.CodeSearchServer gRPC interface.
type CodeSearchService struct {
	pb.UnimplementedCodeSearchServer
	engine *search.Engine
}

// NewCodeSearchService creates a gRPC service that searches engine.
func NewCodeSearchService(engine *search.Engine) *CodeSearchService {
	return &CodeSearchService{engine: engine}
}

// Register adds the service to a gRPC server.
func (s *CodeSearchService) Register(srv *grpc.Server) {
	pb.RegisterCodeSearchServer(srv, s)
}

// Search streams matching results back to the client.
// It implements the streaming RPC defined in proto/search.proto.
func (s *CodeSearchService) Search(
	req *pb.SearchRequest,
	stream pb.CodeSearch_SearchServer,
) error {
	ctx := stream.Context()

	if req.Query == "" {
		return status.Error(codes.InvalidArgument, "query must not be empty")
	}

	opts := search.SearchOptions{
		Query:           req.Query,
		MaxResults:      int(req.MaxResults),
		IsRegex:         req.IsRegex,
		SymbolsOnly:     req.SymbolsOnly,
		IncludePatterns: req.IncludePaths,
		ExcludePatterns: req.ExcludePaths,
	}
	if opts.MaxResults <= 0 {
		opts.MaxResults = 100
	}

	var matches []search.SearchMatch
	var err error

	if req.SymbolsOnly {
		matches = s.engine.SearchSymbols(req.Query, opts.MaxResults)
	} else {
		matches, err = s.engine.Search(opts)
		if err != nil {
			slog.Error("grpc search error", "query", req.Query, "err", err)
			return status.Errorf(codes.Internal, "search failed: %v", err)
		}
	}

	for _, m := range matches {
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}

		result := &pb.SearchResult{
			FilePath:         m.FilePath,
			Content:          m.Content,
			LineNumber:       int32(m.LineNumber),
			Score:            m.Score,
			MatchType:        pb.MatchType(m.MatchType),
			MatchStart:       int32(m.MatchStart),
			MatchEnd:         int32(m.MatchEnd),
			ContentTruncated: m.ContentTruncated,
		}
		if err := stream.Send(result); err != nil {
			return err
		}
	}
	return nil
}

// Index triggers (re-)indexing of the provided paths.
func (s *CodeSearchService) Index(
	ctx context.Context,
	req *pb.IndexRequest,
) (*pb.IndexResponse, error) {
	if len(req.Paths) == 0 {
		return nil, status.Error(codes.InvalidArgument, "at least one path required")
	}

	n := s.engine.IndexBatch(req.Paths, 0)
	stats := s.engine.Stats()

	return &pb.IndexResponse{
		FilesIndexed: int32(n),
		TotalSize:    stats.TotalBytes,
		Message:      "indexed successfully",
	}, nil
}
