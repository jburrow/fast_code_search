# fast_code_search — Go implementation Makefile

MODULE  := github.com/jburrow/fast_code_search
BIN_DIR := bin

.PHONY: all build test lint vet fmt clean run-server run-validator tidy

all: build

## Download dependencies
tidy:
	go mod tidy

## Build all binaries
build: tidy
	@mkdir -p $(BIN_DIR)
	go build -o $(BIN_DIR)/fast_code_search_server   ./cmd/server/
	go build -o $(BIN_DIR)/fast_code_search_validator ./cmd/validator/
	@echo "Built binaries in $(BIN_DIR)/"

## Run all tests
test:
	go test -v -race ./...

## Run integration tests only
test-integration:
	go test -v -race ./tests/...

## Run vet
vet:
	go vet ./...

## Format code
fmt:
	gofmt -w .

## Lint (requires golangci-lint)
lint:
	golangci-lint run ./...

## Run the server (builds first)
run-server: build
	./$(BIN_DIR)/fast_code_search_server

## Run the validator
run-validator: build
	./$(BIN_DIR)/fast_code_search_validator

## Generate protobuf Go code (requires protoc + plugins)
proto:
	protoc \
	  --go_out=proto/search \
	  --go_opt=paths=source_relative \
	  --go_opt=Msearch.proto=github.com/jburrow/fast_code_search/proto/search \
	  --go-grpc_out=proto/search \
	  --go-grpc_opt=paths=source_relative \
	  --go-grpc_opt=Msearch.proto=github.com/jburrow/fast_code_search/proto/search \
	  -I ../proto ../proto/search.proto
	protoc \
	  --go_out=proto/semantic_search \
	  --go_opt=paths=source_relative \
	  --go_opt=Msemantic_search.proto=github.com/jburrow/fast_code_search/proto/semantic_search \
	  --go-grpc_out=proto/semantic_search \
	  --go-grpc_opt=paths=source_relative \
	  --go-grpc_opt=Msemantic_search.proto=github.com/jburrow/fast_code_search/proto/semantic_search \
	  -I ../proto ../proto/semantic_search.proto

## Clean build artifacts
clean:
	rm -rf $(BIN_DIR)
