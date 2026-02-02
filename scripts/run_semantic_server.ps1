# Run the semantic search server with the bundled ONNX Runtime
# This script ensures ORT_DYLIB_PATH is set correctly for Windows
#
# Usage:
#   .\scripts\run_semantic_server.ps1
#   .\scripts\run_semantic_server.ps1 -ConfigFile path\to\config.toml

param(
    [string]$ConfigFile = ""
)

$ErrorActionPreference = "Stop"

# Get project root (parent of scripts directory)
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir

# Set ONNX Runtime library path (bundled version)
$OrtDllPath = Join-Path $ProjectRoot "onnxruntime\onnxruntime-win-x64-1.22.0\lib\onnxruntime.dll"

# Check if ONNX Runtime exists
if (-not (Test-Path $OrtDllPath)) {
    Write-Host "ERROR: ONNX Runtime not found at: $OrtDllPath" -ForegroundColor Red
    Write-Host ""
    Write-Host "Please download ONNX Runtime 1.18.x - 1.22.x from:"
    Write-Host "  https://github.com/microsoft/onnxruntime/releases"
    Write-Host ""
    Write-Host "Extract to: $ProjectRoot\onnxruntime\onnxruntime-win-x64-1.22.0\"
    Write-Host ""
    Write-Host "Or set ORT_DYLIB_PATH environment variable to your onnxruntime.dll location."
    exit 1
}

# Set environment variable for this session
$env:ORT_DYLIB_PATH = $OrtDllPath
Write-Host "Using ONNX Runtime: $OrtDllPath" -ForegroundColor Green
Write-Host ""

# Default config file
if ([string]::IsNullOrEmpty($ConfigFile)) {
    $ConfigFile = Join-Path $ProjectRoot "fast_code_search_semantic.toml"
}

# Check if config exists
if (-not (Test-Path $ConfigFile)) {
    Write-Host "Config file not found: $ConfigFile" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "Generate one with:"
    Write-Host "  cargo run --release --bin fast_code_search_semantic --features ml-models -- --init"
    exit 1
}

Write-Host "Using config: $ConfigFile" -ForegroundColor Green
Write-Host ""

# Run the semantic search server
$ExePath = Join-Path $ProjectRoot "target\release\fast_code_search_semantic.exe"
if (-not (Test-Path $ExePath)) {
    Write-Host "ERROR: Binary not found: $ExePath" -ForegroundColor Red
    Write-Host ""
    Write-Host "Build it first with:"
    Write-Host "  cargo build --release --bin fast_code_search_semantic --features ml-models"
    exit 1
}

& $ExePath --config $ConfigFile
