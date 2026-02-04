@echo off
REM WebView Bridge Run Script

echo === Starting WebView Bridge ===
echo.

if not exist "target\release\webview-bridge-rust.exe" (
    echo Error: Executable not found!
    echo Please run build.bat first.
    echo.
    pause
    exit /b 1
)

echo Running: target\release\webview-bridge-rust.exe
echo.
echo Press Ctrl+C to stop the server
echo.

target\release\webview-bridge-rust.exe

pause
