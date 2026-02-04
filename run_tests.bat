@echo off
REM Simple Integration Test Runner
REM This script uses cargo test directly which handles binary discovery

set TIMESTAMP=%date:~0,4%%date:~5,2%%date:~7,2%_%time:~0,2%%time:~3,2%%time:~6,2%
set LOGFILE=test_logs\test_%TIMESTAMP%.log

echo === WebView Bridge Integration Test Runner ===
echo Timestamp: %TIMESTAMP%
echo.

REM Create logs directory if not exists
if not exist test_logs mkdir test_logs

REM Kill any existing server
echo [1/4] Cleaning up any existing server...
taskkill /F /IM webview-bridge-rust.exe >nul 2>&1

REM Build server first
echo.
echo [2/4] Building server...
cargo build --release >> "%LOGFILE%" 2>&1
if errorlevel 1 (
    echo Build failed! Check %LOGFILE% for details.
    type "%LOGFILE%"
    pause
    exit /b 1
)

REM Start server in background with logging
echo.
echo [3/4] Starting server...
start /MIN "" cmd /C "cargo run --release >> test_logs\server_%TIMESTAMP%.log 2>&1"

REM Wait for server to initialize
echo Waiting for server to initialize (30 seconds)...
timeout /t 30 /nobreak >nul

REM Check if server is running
tasklist | findstr "webview-bridge-rust.exe" >nul
if errorlevel 1 (
    echo Server failed to start. Check test_logs\server_%TIMESTAMP%.log for details.
    type test_logs\server_%TIMESTAMP%.log
    pause
    exit /b 1
)

REM Run tests using cargo test (handles binary discovery)
echo.
echo [4/4] Running integration tests...
echo Log file: %LOGFILE%
cargo test --release --test integration_test -- --ignored --test-threads=1 >> "%LOGFILE%" 2>&1
set TEST_RESULT=%errorlevel%

REM Cleanup
echo.
echo Cleaning up...
taskkill /F /IM webview-bridge-rust.exe >nul 2>&1

if %TEST_RESULT% equ 0 (
    echo.
    echo === All tests passed! ===
) else (
    echo.
    echo === Some tests failed ===
    echo Check %LOGFILE% for details.
)

echo.
echo Logs saved to: %LOGFILE%
echo Press any key to exit...
pause >nul
