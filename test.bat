@echo off
REM ============================================================================
REM WebView Bridge - Test Runner
REM ============================================================================
REM
REM This script runs:
REM 1. Unit tests (cargo test)
REM 2. Integration tests (requires server running)
REM
REM Usage:
REM   test.bat              - Run unit tests only
REM   test.bat integration   - Run integration tests (requires server)
REM   test.bat all           - Run all tests
REM ============================================================================

setlocal enabledelayedexpansion

set "TEST_RESULT=0"

echo.
echo ==========================================
echo   WebView Bridge - Test Runner
echo ==========================================
echo.

REM Parse arguments
set TEST_TYPE=unit
if "%1"=="integration" set TEST_TYPE=integration
if "%1"=="all" set TEST_TYPE=all

REM ============================================================================
REM Unit Tests
REM ============================================================================
if "%TEST_TYPE%"=="unit" goto skip_unit
if "%TEST_TYPE%"=="integration" goto skip_unit

echo [Unit Tests]
echo Running cargo test...
echo.

cargo test --lib
if %ERRORLEVEL% NEQ 0 (
    echo [FAILED] Unit tests failed
    set "TEST_RESULT=1"
) else (
    echo [OK] Unit tests passed
)
echo.

:skip_unit

REM ============================================================================
REM Integration Tests
REM ============================================================================
if "%TEST_TYPE%"=="unit" goto skip_integration
if "%TEST_TYPE%"=="all" if not exist ".\target\release\webview-bridge-rust.exe" (
    echo [SKIP] Integration tests skipped (server not built)
    goto skip_integration
)

echo [Integration Tests]
echo.
echo NOTE: Integration tests require the server to be running.
echo       Start the server first: run.bat
echo.

REM Check if server is running
curl -s http://127.0.0.1:9400/health >nul 2>&1
if %ERRORLEVEL% NEQ 0 (
    echo [SKIP] Server not running. Start with: run.bat
    goto skip_integration
)

echo Running integration tests...
echo.

REM Run PowerShell integration tests
powershell -ExecutionPolicy Bypass -File "%~dp0test_api.ps1"
if %ERRORLEVEL% NEQ 0 (
    echo [FAILED] Integration tests failed
    set "TEST_RESULT=1"
) else (
    echo [OK] Integration tests passed
)
echo.

:skip_integration

REM ============================================================================
REM Summary
REM ============================================================================
echo ==========================================
echo   Test Summary
echo ==========================================
echo.

if %TEST_RESULT% EQU 0 (
    echo [OK] All tests passed
    endlocal
    exit /b 0
) else (
    echo [FAILED] Some tests failed
    endlocal
    exit /b 1
)
