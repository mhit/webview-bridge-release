<#
.SYNOPSIS
    WebView Bridge Server watchdog — auto-restarts the server if it exits.

.DESCRIPTION
    The server now contains an internal deadlock detector (Watchdog task) that
    calls std::process::exit(1) when all command-processor slots stop responding.
    This script monitors the process and restarts it whenever it exits, providing
    full auto-recovery from hangs.

.PARAMETER ServerPath
    Full path to webview-bridge-rust.exe.
    Defaults to the standard install location.

.PARAMETER LogPath
    File to append restart events to.

.EXAMPLE
    pwsh -File watchdog.ps1
    pwsh -File watchdog.ps1 -ServerPath "C:\MyInstall\webview-bridge-rust.exe"
#>
param(
    [string]$ServerPath = "$env:LOCALAPPDATA\Programs\webview-bridge\webview-bridge-rust.exe",
    [string]$LogPath    = "$env:APPDATA\webview-bridge\watchdog.log"
)

function Write-WatchdogLog {
    param([string]$Message)
    $ts = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    $line = "$ts  $Message"
    Write-Host $line
    Add-Content -Path $LogPath -Value $line -ErrorAction SilentlyContinue
}

Write-WatchdogLog "Watchdog started. Monitoring: $ServerPath"

while ($true) {
    # Check if any webview-bridge-rust.exe is running
    $proc = Get-Process -Name "webview-bridge-rust" -ErrorAction SilentlyContinue

    if ($null -eq $proc) {
        Write-WatchdogLog "Server process not found — starting..."

        if (-not (Test-Path $ServerPath)) {
            Write-WatchdogLog "ERROR: Binary not found at $ServerPath. Retrying in 10s."
            Start-Sleep -Seconds 10
            continue
        }

        Start-Process -FilePath $ServerPath -WorkingDirectory (Split-Path $ServerPath)
        Write-WatchdogLog "Server started (PID will be assigned by OS)."

        # Wait a few seconds for it to initialize before next check
        Start-Sleep -Seconds 5
    }

    # Poll every 5 seconds
    Start-Sleep -Seconds 5
}
