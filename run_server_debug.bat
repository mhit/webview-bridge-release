@echo off
REM Debug script to run server and see output

echo Killing any existing server...
taskkill /F /IM webview-bridge-rust.exe >nul 2>&1
timeout /t 1 /nobreak >nul

echo.
echo Building server...
cargo build --release
if errorlevel 1 (
    echo Build failed!
    pause
    exit /b 1
)

echo.
echo Starting server (will show output)...
echo Press Ctrl+C to stop
echo.

target\release\webview-bridge-rust.exe
