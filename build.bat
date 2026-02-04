@echo off
REM WebView Bridge Build Script for Windows

echo === WebView Bridge Build ===
echo.

echo Cleaning previous builds...
call cargo clean

echo.
echo Building release version...
set CARGO_INCREMENTAL=0
call cargo build --release

if %ERRORLEVEL% EQU 0 (
    echo.
    echo === Build Successful! ===
    echo.
    echo Executable: target\release\webview-bridge-rust.exe
    echo.
    echo Run the application:
    echo   .\target\release\webview-bridge-rust.exe
    echo.
) else (
    echo.
    echo === Build Failed! ===
    echo.
    echo Please check the error messages above.
    echo.
)

pause
