@echo off
REM ============================================================================
REM WebView Bridge - Project Validation Script
REM ============================================================================
REM
REM This script validates the project by running:
REM 1. Code formatting check (rustfmt)
REM 2. Linting (clippy)
REM 3. Build verification
REM 4. Tests
REM 5. Security audit (cargo-audit)
REM
REM Usage:
REM   validate.bat         - Run all validations
REM   validate.bat quick   - Quick validation (fmt + clippy)
REM   validate.bat full    - Full validation (all checks)
REM
REM ============================================================================

setlocal enabledelayedexpansion

REM Color codes for output
set "INFO=[INFO]"
set "OK=[OK]"
set "WARN=[WARN]"
set "ERROR=[ERROR]"
set "SECTION=[====]"

REM Parse arguments
set VALIDATION_LEVEL=standard
if "%1"=="quick" set VALIDATION_LEVEL=quick
if "%1"=="full" set VALIDATION_LEVEL=full

echo.
echo %SECTION% ==========================================
echo   WebView Bridge - Project Validation
echo   Level: %VALIDATION_LEVEL%
echo ==========================================%SECTION%
echo.

REM Counters
set PASSED=0
set FAILED=0
set SKIPPED=0

REM ============================================================================
REM Function: Check and display result
REM ============================================================================
:check_result
if %ERRORLEVEL% EQU 0 (
    echo %OK% %~1
    set /a PASSED+=1
) else (
    echo %ERROR% %~1
    set /a FAILED+=1
)
exit /b 0

REM ============================================================================
REM Function: Skip test
REM ============================================================================
:skip_test
echo %WARN% %~1
set /a SKIPPED+=1
exit /b 0

REM ============================================================================
REM 1. Environment Check
REM ============================================================================
echo %INFO% Checking environment...

where cargo >nul 2>&1
call :check_result "Cargo is installed"

where rustc >nul 2>&1
call :check_result "Rust compiler is installed"

REM Display versions
for /f "tokens=2" %%i in ('cargo --version') do set CARGO_VERSION=%%i
echo %INFO% Cargo version: %CARGO_VERSION%

for /f "tokens=2" %%i in ('rustc --version') do set RUSTC_VERSION=%%i
echo %INFO% Rust version: %RUSTC_VERSION%
echo.

REM ============================================================================
REM 2. Code Formatting Check
REM ============================================================================
echo %SECTION% Code Formatting (rustfmt) %SECTION%
cargo fmt -- --check >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    echo %OK% Code is properly formatted
    set /a PASSED+=1
) else (
    echo %WARN% Code formatting issues detected, running cargo fmt...
    cargo fmt
    if %ERRORLEVEL% EQU 0 (
        echo %OK% Code formatted successfully
        set /a PASSED+=1
    ) else (
        echo %ERROR% Failed to format code
        set /a FAILED+=1
    )
)
echo.

REM ============================================================================
REM 3. Linting (Clippy)
REM ============================================================================
echo %SECTION% Linting (clippy) %SECTION%
if "%VALIDATION_LEVEL%"=="quick" goto skip_clippy
cargo clippy --all-targets --all-features -- -D warnings
call :check_result "Clippy passed"
echo.
:skip_clippy

REM ============================================================================
REM 4. Build Verification
REM ============================================================================
echo %SECTION% Build Verification %SECTION%
if "%VALIDATION_LEVEL%"=="quick" goto skip_build
set CARGO_INCREMENTAL=0
cargo build --release
call :check_result "Build succeeded"
echo.

REM Check binary size
if exist "target\release\webview-bridge-rust.exe" (
    for %%A in ("target\release\webview-bridge-rust.exe") do set SIZE=%%~zA
    set /a SIZE_MB=%SIZE% / 1048576
    echo %INFO% Binary size: !SIZE_MB! MB
)
echo.
:skip_build

REM ============================================================================
REM 5. Tests
REM ============================================================================
echo %SECTION% Tests %SECTION%
if "%VALIDATION_LEVEL%"=="quick" goto skip_tests
cargo test
call :check_result "Tests passed"
echo.
:skip_tests

REM ============================================================================
REM 6. Security Audit (if cargo-audit is installed)
REM ============================================================================
echo %SECTION% Security Audit %SECTION%
if "%VALIDATION_LEVEL%" NEQ "full" goto skip_audit
where cargo-audit >nul 2>&1
if %ERRORLEVEL% NEQ 0 (
    call :skip_test "cargo-audit not installed (install with: cargo install cargo-audit)"
    goto skip_audit
)
cargo audit
if %ERRORLEVEL% EQU 0 (
    echo %OK% No security vulnerabilities found
    set /a PASSED+=1
) else (
    echo %WARN% Security vulnerabilities detected
    set /a FAILED+=1
)
echo.
:skip_audit

REM ============================================================================
REM Summary
REM ============================================================================
echo %SECTION% ==========================================
echo   Validation Summary
echo ==========================================%SECTION%
echo.
echo Passed: %PASSED%
echo Failed: %FAILED%
echo Skipped: %SKIPPED%
echo.

if %FAILED% GTR 0 (
    echo %ERROR% VALIDATION FAILED
    echo.
    echo Please fix the errors above and run again.
    endlocal
    exit /b 1
) else (
    echo %OK% VALIDATION PASSED
    echo.
    echo Project is ready to build/deploy.
    endlocal
    exit /b 0
)
