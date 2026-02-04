@echo off
REM Start server in background and run integration tests

echo Starting server...
start /B "" cargo run --release > server.log 2>&1

REM Wait for server to start
echo Waiting for server to start...
timeout /t 15 /nobreak >nul

REM Check if server is running
tasklist | findstr "webview-bridge-rust.exe" >nul
if errorlevel 1 (
    echo Server failed to start. Check server.log for details.
    type server.log
    exit /b 1
)

echo Server is running. Running integration tests...
cargo test --release --test integration_test -- --ignored --test-threads=1

REM Cleanup: kill server
echo Cleaning up...
taskkill /F /IM webview-bridge-rust.exe >nul 2>&1

echo Done.
