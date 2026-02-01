@echo off
REM ============================================================================
REM Fast Code Search - Test Corpus Setup Script
REM This script clones test repositories and indexes them for benchmarking
REM ============================================================================

setlocal EnableDelayedExpansion

set CORPUS_DIR=%~dp0..\test_corpus
set SERVER_URL=http://127.0.0.1:50051

echo ============================================
echo  Fast Code Search - Test Corpus Setup
echo ============================================
echo.

REM Create test corpus directory
if not exist "%CORPUS_DIR%" (
    echo Creating test corpus directory: %CORPUS_DIR%
    mkdir "%CORPUS_DIR%"
)

cd /d "%CORPUS_DIR%"

REM ============================================================================
REM Clone repositories (use --depth 1 for faster cloning)
REM ============================================================================

echo.
echo [1/5] Cloning test repositories...
echo.

REM Rust - rust-lang/rust (~500MB)
if not exist "rust" (
    echo Cloning rust-lang/rust...
    git clone --depth 1 https://github.com/rust-lang/rust.git
) else (
    echo [SKIP] rust already exists
)

REM Python - cpython (~150MB)
if not exist "cpython" (
    echo Cloning python/cpython...
    git clone --depth 1 https://github.com/python/cpython.git
) else (
    echo [SKIP] cpython already exists
)

REM TypeScript/JavaScript - VS Code (~200MB)
if not exist "vscode" (
    echo Cloning microsoft/vscode...
    git clone --depth 1 https://github.com/microsoft/vscode.git
) else (
    echo [SKIP] vscode already exists
)

REM Optional: Linux kernel (uncomment for larger test ~1.5GB)
REM if not exist "linux" (
REM     echo Cloning torvalds/linux...
REM     git clone --depth 1 https://github.com/torvalds/linux.git
REM ) else (
REM     echo [SKIP] linux already exists
REM )

echo.
echo [2/5] Repository cloning complete!
echo.

REM Calculate corpus size
echo [3/5] Calculating corpus size...
set SIZE=0
for /f "tokens=3" %%a in ('dir /s "%CORPUS_DIR%" 2^>nul ^| findstr "File(s)"') do set SIZE=%%a
echo Total corpus size: approximately %SIZE% bytes
echo.

REM ============================================================================
REM Build the project in release mode
REM ============================================================================

echo [4/5] Building fast_code_search in release mode...
cd /d "%~dp0.."
cargo build --release
if errorlevel 1 (
    echo ERROR: Build failed!
    exit /b 1
)
echo Build complete!
echo.

REM ============================================================================
REM Start server and index
REM ============================================================================

echo [5/5] Starting server and indexing...
echo.
echo The server will start in a new window.
echo After the server is ready, run the indexing client.
echo.

REM Start server in a new window
start "Fast Code Search Server" cmd /k "cd /d %~dp0.. && cargo run --release"

echo Waiting 5 seconds for server to start...
timeout /t 5 /nobreak >nul

REM Create a temporary client script to index the corpus
echo.
echo ============================================
echo  Server started! 
echo ============================================
echo.
echo To index the test corpus, run:
echo   cargo run --example client -- --index "%CORPUS_DIR%"
echo.
echo Or manually modify examples/client.rs to index these paths:
echo   - %CORPUS_DIR%\rust
echo   - %CORPUS_DIR%\cpython  
echo   - %CORPUS_DIR%\vscode
echo.
echo Server is running at: %SERVER_URL%
echo.

pause
