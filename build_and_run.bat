@echo off
REM Build and Run WebView Bridge

echo === WebView Bridge Build and Run ===
echo.

REM Get Windows IP for WSL access
for /f "tokens=2 delims=:" %%I in ('ipconfig ^| findstr "IPv4"') do set WINDOWS_IP=%%I
for /f "tokens=*" %%I in ("%WINDOWS_IP%") do set WINDOWS_IP=%%I
set WINDOWS_IP=%WINDOWS_IP: =%

echo Windows IP for WSL access: %WINDOWS_IP%
echo.

REM Clean and build
echo Building...
set CARGO_INCREMENTAL=0
cargo build --release

if %ERRORLEVEL% EQU 0 (
    echo.
    echo === Build Successful! ===
    echo.
    echo Starting WebView Bridge...
    echo.
    echo Server will listen on: http://localhost:9400
    echo WSL can access via: http://%WINDOWS_IP%:9400
    echo.
    echo Press Ctrl+C to stop
    echo.
    target\release\webview-bridge-rust.exe
) else (
    echo.
    echo === Build Failed! ===
    echo.
)

pause
