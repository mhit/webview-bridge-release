@echo off
setlocal

set LOG_DIR=%APPDATA%\webview-bridge
set LOG_FILE=%LOG_DIR%\server.log
set EXE=%~dp0webview-bridge-rust.exe

if not exist "%LOG_DIR%" mkdir "%LOG_DIR%"

echo. >> "%LOG_FILE%"
echo ======================================== >> "%LOG_FILE%"
echo [%DATE% %TIME%] Server starting >> "%LOG_FILE%"
echo ======================================== >> "%LOG_FILE%"

set RUST_LOG=info

"%EXE%" >> "%LOG_FILE%" 2>&1
