@echo off
setlocal enabledelayedexpansion

echo ========================================
echo  WebView Bridge - Use Case Tests
echo  Phase 4: 実運用テスト統合
echo ========================================
echo.

set TIMESTAMP=%date:~0,4%%date:~5,2%%date:~8,2%_%time:~0,2%%time:~3,2%%time:~6,2%
set TIMESTAMP=%TIMESTAMP: =0%

:: Check if server is running
curl -s http://localhost:9400/health >nul 2>&1
if %errorlevel% neq 0 (
    echo [INFO] Server not running. Starting server...
    start /B cargo run
    echo Waiting 30 seconds for server to start...
    timeout /t 30 /nobreak >nul
)

:: Create test_logs directory
if not exist test_logs mkdir test_logs

:: Run PowerShell tests
echo.
echo [RUN] PowerShell Use Case Tests
echo.

powershell -ExecutionPolicy Bypass -File tests\UseCase-Tests.ps1 -TestCase all

if %errorlevel% equ 0 (
    echo.
    echo ========================================
    echo  ALL TESTS PASSED!
    echo ========================================
) else (
    echo.
    echo ========================================
    echo  SOME TESTS FAILED
    echo ========================================
)

echo.
echo Press any key to exit...
pause >nul
