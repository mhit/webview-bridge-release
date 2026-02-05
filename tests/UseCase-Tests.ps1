# WebView Bridge - Use Case Tests (Simplified)
# Phase 4: 実運用テスト統合

param(
    [string]$ServerUrl = "http://localhost:9400",
    [string]$TestCase = "all"
)

$ErrorActionPreference = "Continue"
$TestResults = @()
$SessionId = $null

function Write-TestHeader($Title) {
    Write-Host "`n========================================" -ForegroundColor Cyan
    Write-Host " $Title" -ForegroundColor Cyan
    Write-Host "========================================" -ForegroundColor Cyan
}

function Write-TestResult($TestId, $Name, $Passed, $Message = "", $Duration = 0) {
    $status = if ($Passed) { "PASS" } else { "FAIL" }
    $color = if ($Passed) { "Green" } else { "Red" }
    Write-Host "[$status] $TestId : $Name ($Duration ms)" -ForegroundColor $color
    if ($Message) { Write-Host "         $Message" -ForegroundColor Yellow }
    $script:TestResults += @{ id = $TestId; name = $Name; passed = $Passed }
}

function Api-Get($Endpoint) {
    try {
        return Invoke-RestMethod -Uri "$ServerUrl$Endpoint" -Method Get -TimeoutSec 60
    }
    catch {
        return $null
    }
}

function Api-Post($Endpoint, $Body) {
    try {
        $json = $Body | ConvertTo-Json -Depth 5
        return Invoke-RestMethod -Uri "$ServerUrl$Endpoint" -Method Post -Body $json -ContentType "application/json" -TimeoutSec 60
    }
    catch {
        return $null
    }
}

function Api-Delete($Endpoint) {
    try {
        return Invoke-RestMethod -Uri "$ServerUrl$Endpoint" -Method Delete -TimeoutSec 30
    }
    catch {
        return $null
    }
}

# Test: Create Session
function Test-CreateSession {
    Write-TestHeader "Creating Test Session"
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    
    $result = Api-Post "/create" @{ profile = "test-profile"; headless = $false }
    
    if ($result -and $result.id) {
        $script:SessionId = $result.id
        Write-TestResult "SES-01" "Create Session" $true "Session ID: $($result.id)" $sw.ElapsedMilliseconds
        Start-Sleep -Seconds 2
        return $true
    }
    Write-TestResult "SES-01" "Create Session" $false "Failed" $sw.ElapsedMilliseconds
    return $false
}

# Test: Health Check
function Test-HealthCheck {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $result = Api-Get "/health"
    Write-TestResult "API-01" "Health Check" ($result -eq "OK") "" $sw.ElapsedMilliseconds
}

# Test: Get Status
function Test-GetStatus {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $result = Api-Get "/status/$SessionId"
    Write-TestResult "API-02" "Get Status" ($null -ne $result) "" $sw.ElapsedMilliseconds
}

# Test: Get Cookies
function Test-GetCookies {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $result = Api-Get "/cookies/$SessionId"
    Write-TestResult "API-03" "Get Cookies" ($null -ne $result) "" $sw.ElapsedMilliseconds
}

# Test: Navigate
function Test-Navigate($Url, $TestId, $Name) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $result = Api-Post "/navigate/$SessionId" @{ url = $Url }
    Write-TestResult $TestId $Name ($null -ne $result) "" $sw.ElapsedMilliseconds
    Start-Sleep -Seconds 5
    return ($null -ne $result)
}

# Test: Wait for Selector
function Test-WaitForSelector($Selector, $TestId, $Name) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $result = Api-Post "/wait/$SessionId" @{ selector = $Selector; timeout = 10000 }
    $found = ($null -ne $result -and $result.found -eq $true)
    Write-TestResult $TestId $Name $found "Selector: $Selector" $sw.ElapsedMilliseconds
    return $found
}

# Test: Extract
function Test-Extract($Selector, $TestId, $Name) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $result = Api-Post "/extract/$SessionId" @{ selector = $Selector; attribute = "text"; extract_all = $true }
    $count = 0
    if ($null -ne $result -and $null -ne $result.data) {
        if ($result.data -is [array]) { $count = $result.data.Count }
        else { $count = 1 }
    }
    Write-TestResult $TestId $Name ($count -gt 0) "Found $count items" $sw.ElapsedMilliseconds
    return $count
}

# Test: Screenshot
function Test-Screenshot($TestId, $Name) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $result = Api-Get "/screenshot/$SessionId"
    $hasImage = ($null -ne $result -and $null -ne $result.image)
    Write-TestResult $TestId $Name $hasImage "" $sw.ElapsedMilliseconds
}

# Close Session
function Close-Session {
    if ($SessionId) {
        Api-Delete "/close/$SessionId" | Out-Null
        Write-Host "Session closed: $SessionId" -ForegroundColor Gray
    }
}

# ============================================================================
# Main
# ============================================================================

Write-Host @"
========================================
 WebView Bridge - Use Case Test Suite
 Phase 4: 実運用テスト
========================================
"@ -ForegroundColor Magenta

Write-Host "Server: $ServerUrl"
Write-Host "Test: $TestCase"

# Check server
$health = Api-Get "/health"
if ($health -ne "OK") {
    Write-Host "Server not running at $ServerUrl" -ForegroundColor Red
    exit 1
}
Write-Host "Server is healthy" -ForegroundColor Green

# Create session
if (-not (Test-CreateSession)) {
    Write-Host "Failed to create session" -ForegroundColor Red
    exit 1
}

try {
    switch ($TestCase) {
        "basic" {
            Test-HealthCheck
            Test-GetStatus
            Test-GetCookies
        }
        "UC-08" {
            Write-TestHeader "UC-08: Adamas Official Site"
            Test-Navigate "https://shop.adamas-octa.com/" "UC08-01" "Navigate to Shop"
            Test-WaitForSelector "body" "UC08-02" "Wait for Body"
            Test-Screenshot "UC08-03" "Take Screenshot"
        }
        "all" {
            Write-TestHeader "Basic API Tests"
            Test-HealthCheck
            Test-GetStatus
            Test-GetCookies
            
            Write-TestHeader "UC-08: Adamas Official Site"
            Test-Navigate "https://shop.adamas-octa.com/" "UC08-01" "Navigate to Shop"
            Test-WaitForSelector "body" "UC08-02" "Wait for Body"
            Test-Screenshot "UC08-03" "Take Screenshot"
            
            Write-TestHeader "UC-03: Amazon Search"
            Test-Navigate "https://www.amazon.co.jp/s?k=test" "UC03-01" "Navigate to Amazon"
            Test-WaitForSelector "[data-component-type]" "UC03-02" "Wait for Results"
        }
        default {
            Write-Host "Unknown test: $TestCase (available: basic, UC-08, all)" -ForegroundColor Yellow
        }
    }
}
finally {
    Close-Session
}

# Summary
Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host " TEST SUMMARY" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

$passed = ($TestResults | Where-Object { $_.passed }).Count
$failed = ($TestResults | Where-Object { -not $_.passed }).Count
$total = $TestResults.Count

$color = if ($failed -eq 0) { "Green" } else { "Yellow" }
Write-Host "Total: $total | Passed: $passed | Failed: $failed" -ForegroundColor $color

exit $(if ($failed -eq 0) { 0 } else { 1 })
