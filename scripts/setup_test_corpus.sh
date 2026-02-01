#!/bin/bash
# ============================================================================
# Fast Code Search - Test Corpus Setup Script
# This script clones test repositories and indexes them for benchmarking
# ============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
CORPUS_DIR="$PROJECT_DIR/test_corpus"
SERVER_URL="http://127.0.0.1:50051"

echo "============================================"
echo " Fast Code Search - Test Corpus Setup"
echo "============================================"
echo ""

# Create test corpus directory
if [ ! -d "$CORPUS_DIR" ]; then
    echo "Creating test corpus directory: $CORPUS_DIR"
    mkdir -p "$CORPUS_DIR"
fi

cd "$CORPUS_DIR"

# ============================================================================
# Clone repositories (use --depth 1 for faster cloning)
# ============================================================================

echo ""
echo "[1/5] Cloning test repositories..."
echo ""

# Rust - rust-lang/rust (~500MB)
if [ ! -d "rust" ]; then
    echo "Cloning rust-lang/rust..."
    git clone --depth 1 https://github.com/rust-lang/rust.git
else
    echo "[SKIP] rust already exists"
fi

# Python - cpython (~150MB)
if [ ! -d "cpython" ]; then
    echo "Cloning python/cpython..."
    git clone --depth 1 https://github.com/python/cpython.git
else
    echo "[SKIP] cpython already exists"
fi

# TypeScript/JavaScript - VS Code (~200MB)
if [ ! -d "vscode" ]; then
    echo "Cloning microsoft/vscode..."
    git clone --depth 1 https://github.com/microsoft/vscode.git
else
    echo "[SKIP] vscode already exists"
fi

# Optional: Linux kernel (uncomment for larger test ~1.5GB)
# if [ ! -d "linux" ]; then
#     echo "Cloning torvalds/linux..."
#     git clone --depth 1 https://github.com/torvalds/linux.git
# else
#     echo "[SKIP] linux already exists"
# fi

echo ""
echo "[2/5] Repository cloning complete!"
echo ""

# Calculate corpus size
echo "[3/5] Calculating corpus size..."
if command -v du &> /dev/null; then
    SIZE=$(du -sh "$CORPUS_DIR" 2>/dev/null | cut -f1)
    echo "Total corpus size: $SIZE"
else
    echo "Total corpus size: (du not available)"
fi
echo ""

# ============================================================================
# Build the project in release mode
# ============================================================================

echo "[4/5] Building fast_code_search in release mode..."
cd "$PROJECT_DIR"
cargo build --release
echo "Build complete!"
echo ""

# ============================================================================
# Start server and index
# ============================================================================

echo "[5/5] Starting server and indexing..."
echo ""

# Function to cleanup on exit
cleanup() {
    if [ ! -z "$SERVER_PID" ]; then
        echo "Stopping server (PID: $SERVER_PID)..."
        kill $SERVER_PID 2>/dev/null || true
    fi
}
trap cleanup EXIT

# Start server in background
echo "Starting server..."
cargo run --release &
SERVER_PID=$!

echo "Waiting for server to start..."
sleep 3

# Check if server is running
if ! kill -0 $SERVER_PID 2>/dev/null; then
    echo "ERROR: Server failed to start!"
    exit 1
fi

echo ""
echo "============================================"
echo " Server started! (PID: $SERVER_PID)"
echo "============================================"
echo ""
echo "To index the test corpus, run:"
echo "  cargo run --example client -- --index \"$CORPUS_DIR\""
echo ""
echo "Or manually modify examples/client.rs to index these paths:"
echo "  - $CORPUS_DIR/rust"
echo "  - $CORPUS_DIR/cpython"
echo "  - $CORPUS_DIR/vscode"
echo ""
echo "Server is running at: $SERVER_URL"
echo ""
echo "Press Ctrl+C to stop the server..."

# Wait for server
wait $SERVER_PID
