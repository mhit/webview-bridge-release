@echo off
set TIMESTAMP=%date:~0,4%%date:~5,2%%date:~7,2%_%time:~0,2%%time:~1,2%%time:~3,2%
set LOGFILE=test_logs\integration_test_%TIMESTAMP%.log

echo === WebView Bridge Integration Test Runner ===
echo Timestamp: %TIMESTAMP%
echo.

REM Create logs directory if not exists
if not exist test_logs mkdir test_logs

REM Kill any existing server
echo [0/4] Cleaning up any existing server...
taskkill /F /IM webview-bridge-rust.exe >nul 2>&1

REM Build everything first (server + test binary)
echo.
echo [1/4] Building server and test binary...
cargo build --release >> "%LOGFILE%" 2>&1
if errorlevel 1 (
    echo Build failed! Check %LOGFILE% for details.
    pause
    exit /b 1
)

REM Start server in background with logging
echo.
echo [2/4] Starting server...
start /MIN "" cmd /C "cargo run --release >> test_logs\server_%TIMESTAMP%.log 2>&1"

REM Wait for server to initialize
echo Waiting for server to initialize (25 seconds)...
timeout /t 25 /nobreak >nul

REM Run tests using pre-built binary with logging
echo.
echo [3/4] Running integration tests...
echo Log file: %LOGFILE%
REM Find the latest integration test binary dynamically
for /f "delims=" %%i in ('dir /b /o-d target\release\deps\integration_test-*.exe 2^>nul') do (
    set TEST_BIN=%%i
    goto :found
)
:found
if not defined TEST_BIN (
    echo ERROR: Integration test binary not found in target\release\deps\
    echo Run 'cargo build --release' first.
    pause
    exit /b 1
)
target\release\deps\%TEST_BIN% --ignored >> "%LOGFILE%" 2>&1
set TEST_RESULT=%errorlevel%

REM Cleanup
echo.
echo [4/4] Cleaning up...
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
