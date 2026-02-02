@echo off
REM Run the semantic search server with the bundled ONNX Runtime
REM This script ensures ORT_DYLIB_PATH is set correctly for Windows

setlocal

REM Get the directory where this script is located
set "SCRIPT_DIR=%~dp0"
set "PROJECT_ROOT=%SCRIPT_DIR%.."

REM Set ONNX Runtime library path (bundled version)
set "ORT_DYLIB_PATH=%PROJECT_ROOT%\onnxruntime\onnxruntime-win-x64-1.22.0\lib\onnxruntime.dll"

REM Check if ONNX Runtime exists
if not exist "%ORT_DYLIB_PATH%" (
    echo ERROR: ONNX Runtime not found at: %ORT_DYLIB_PATH%
    echo.
    echo Please download ONNX Runtime 1.18.x - 1.22.x from:
    echo   https://github.com/microsoft/onnxruntime/releases
    echo.
    echo Extract to: %PROJECT_ROOT%\onnxruntime\onnxruntime-win-x64-1.22.0\
    echo.
    echo Or set ORT_DYLIB_PATH environment variable to your onnxruntime.dll location.
    exit /b 1
)

echo Using ONNX Runtime: %ORT_DYLIB_PATH%
echo.

REM Default config file
set "CONFIG_FILE=%PROJECT_ROOT%\fast_code_search_semantic.toml"

REM Allow overriding config via argument
if not "%~1"=="" (
    set "CONFIG_FILE=%~1"
)

REM Check if config exists
if not exist "%CONFIG_FILE%" (
    echo Config file not found: %CONFIG_FILE%
    echo.
    echo Generate one with:
    echo   cargo run --release --bin fast_code_search_semantic --features ml-models -- --init
    exit /b 1
)

echo Using config: %CONFIG_FILE%
echo.

REM Run the semantic search server
"%PROJECT_ROOT%\target\release\fast_code_search_semantic.exe" --config "%CONFIG_FILE%"
