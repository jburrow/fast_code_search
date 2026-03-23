# Run both keyword and semantic search servers together.
# The keyword server (gRPC :50059, Web :8080) and semantic server (gRPC :50052, Web :8081)
# are started as parallel background jobs; Ctrl+C stops both.
#
# Usage:
#   .\scripts\run_combined_servers.ps1
#   .\scripts\run_combined_servers.ps1 -KeywordConfig .keyword_config.toml -SemanticConfig .semantic_config.toml

param(
    [string]$KeywordConfig  = ".keyword_config.toml",
    [string]$SemanticConfig = ".semantic_config.toml"
)

$ErrorActionPreference = "Stop"

$ScriptDir   = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir

# Resolve relative config paths against the project root
function Resolve-Config([string]$Path) {
    if ([System.IO.Path]::IsPathRooted($Path)) { return $Path }
    return Join-Path $ProjectRoot $Path
}

$KeywordConfig  = Resolve-Config $KeywordConfig
$SemanticConfig = Resolve-Config $SemanticConfig

# --- Validate keyword binary & config ---
$KeywordExe = Join-Path $ProjectRoot "target\release\fast_code_search_server.exe"
if (-not (Test-Path $KeywordExe)) {
    Write-Host "ERROR: Keyword binary not found: $KeywordExe" -ForegroundColor Red
    Write-Host "Build it with:  cargo build --release --bin fast_code_search_server"
    exit 1
}
if (-not (Test-Path $KeywordConfig)) {
    Write-Host "ERROR: Keyword config not found: $KeywordConfig" -ForegroundColor Red
    exit 1
}

# --- Validate semantic binary, ONNX Runtime & config ---
$SemanticExe = Join-Path $ProjectRoot "target\release\fast_code_search_semantic.exe"
if (-not (Test-Path $SemanticExe)) {
    Write-Host "ERROR: Semantic binary not found: $SemanticExe" -ForegroundColor Red
    Write-Host "Build it with:  cargo build --release --bin fast_code_search_semantic --features ml-models"
    exit 1
}

$OrtDllPath = Join-Path $ProjectRoot "onnxruntime\onnxruntime-win-x64-1.22.0\lib\onnxruntime.dll"
if (-not (Test-Path $OrtDllPath)) {
    Write-Host "ERROR: ONNX Runtime not found at: $OrtDllPath" -ForegroundColor Red
    Write-Host "Download from https://github.com/microsoft/onnxruntime/releases and extract to:"
    Write-Host "  $ProjectRoot\onnxruntime\onnxruntime-win-x64-1.22.0\"
    exit 1
}

if (-not (Test-Path $SemanticConfig)) {
    Write-Host "ERROR: Semantic config not found: $SemanticConfig" -ForegroundColor Red
    exit 1
}

$env:ORT_DYLIB_PATH = $OrtDllPath

Write-Host ""
Write-Host "Starting combined search servers..." -ForegroundColor Cyan
Write-Host "  Keyword  config : $KeywordConfig"
Write-Host "  Semantic config : $SemanticConfig"
Write-Host "  ONNX Runtime    : $OrtDllPath"
Write-Host ""
Write-Host "Press Ctrl+C to stop both servers." -ForegroundColor Yellow
Write-Host ""

# Start keyword server as a background job
$keywordJob = Start-Job -ScriptBlock {
    param($exe, $cfg)
    & $exe --config $cfg
} -ArgumentList $KeywordExe, $KeywordConfig

Write-Host "Keyword  server started (job $($keywordJob.Id))  — gRPC :50059  Web :8080" -ForegroundColor Green

# Small delay so the keyword server can bind its ports before semantic starts
Start-Sleep -Seconds 1

# Start semantic server as a background job
$semanticJob = Start-Job -ScriptBlock {
    param($exe, $cfg, $ort)
    $env:ORT_DYLIB_PATH = $ort
    & $exe --config $cfg
} -ArgumentList $SemanticExe, $SemanticConfig, $OrtDllPath

Write-Host "Semantic server started (job $($semanticJob.Id)) — gRPC :50052  Web :8081" -ForegroundColor Green
Write-Host ""

# Stream output from both jobs until interrupted
try {
    while ($true) {
        Receive-Job -Job $keywordJob  | ForEach-Object { Write-Host "[keyword ] $_" }
        Receive-Job -Job $semanticJob | ForEach-Object { Write-Host "[semantic] $_" }

        # Stop if either job has exited unexpectedly
        if ($keywordJob.State  -eq 'Failed') { Write-Host "Keyword server failed!"  -ForegroundColor Red;  break }
        if ($semanticJob.State -eq 'Failed') { Write-Host "Semantic server failed!" -ForegroundColor Red;  break }

        Start-Sleep -Milliseconds 500
    }
} finally {
    Write-Host ""
    Write-Host "Stopping servers..." -ForegroundColor Yellow
    Stop-Job  -Job $keywordJob,  $semanticJob -ErrorAction SilentlyContinue
    Remove-Job -Job $keywordJob, $semanticJob -Force -ErrorAction SilentlyContinue
    Write-Host "Both servers stopped." -ForegroundColor Green
}
