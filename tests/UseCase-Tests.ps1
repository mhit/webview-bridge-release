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

# Test: Set Cookies
function Test-SetCookies($Cookies, $TestId, $Name) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $result = Api-Post "/cookies/$SessionId" @{ cookies = $Cookies }
    $success = ($null -ne $result)
    Write-TestResult $TestId $Name $success "" $sw.ElapsedMilliseconds
    return $success
}

# Test: Execute Script
function Test-Execute($Script, $TestId, $Name) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $result = Api-Post "/execute/$SessionId" @{ script = $Script }
    $success = ($null -ne $result)
    Write-TestResult $TestId $Name $success "" $sw.ElapsedMilliseconds
    return $result
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
        "UC-01" {
            Write-TestHeader "UC-01: X Egosearch"
            Test-Navigate "https://x.com/search?q=test" "UC01-01" "Navigate to X Search"
            Test-WaitForSelector "article, main" "UC01-02" "Wait for Content"
            Test-Extract "article" "UC01-03" "Extract Articles"
        }
        "UC-02" {
            Write-TestHeader "UC-02: Google Shopping"
            Test-Navigate "https://www.google.com/search?tbm=shop&q=test" "UC02-01" "Navigate to Shopping"
            Test-WaitForSelector "[data-docid], .sh-dgr__content" "UC02-02" "Wait for Products"
            Test-Extract ".sh-dgr__content, [data-docid]" "UC02-03" "Extract Products"
        }
        "UC-03" {
            Write-TestHeader "UC-03: Amazon Search"
            Test-Navigate "https://www.amazon.co.jp/s?k=test" "UC03-01" "Navigate to Amazon"
            Test-WaitForSelector "[data-component-type='s-search-result']" "UC03-02" "Wait for Results"
            Test-Extract "[data-component-type='s-search-result'] h2" "UC03-03" "Extract Titles"
        }
        "UC-04" {
            Write-TestHeader "UC-04: Rakuten Search"
            Test-Navigate "https://search.rakuten.co.jp/search/mall/test/" "UC04-01" "Navigate to Rakuten"
            Test-WaitForSelector ".searchresultitem, .dui-card" "UC04-02" "Wait for Results"
        }
        "UC-05" {
            Write-TestHeader "UC-05: Yahoo Shopping"
            Test-Navigate "https://shopping.yahoo.co.jp/search?p=test" "UC05-01" "Navigate to Yahoo"
            Test-WaitForSelector ".ProductsListModule, .SearchResult" "UC05-02" "Wait for Results"
        }
        "UC-06" {
            Write-TestHeader "UC-06: Rakuten Books"
            Test-Navigate "https://books.rakuten.co.jp/search?sitem=test" "UC06-01" "Navigate to Books"
            Test-WaitForSelector ".rbcomp__item-list, .item" "UC06-02" "Wait for Results"
        }
        "UC-07" {
            Write-TestHeader "UC-07: EC Site Check"
            Test-Navigate "https://www.google.com/search?q=test" "UC07-01" "Navigate to Google"
            Test-WaitForSelector "#search, .g" "UC07-02" "Wait for Results"
        }
        "UC-08" {
            Write-TestHeader "UC-08: Adamas Official Site"
            Test-Navigate "https://shop.adamas-octa.com/" "UC08-01" "Navigate to Shop"
            Test-WaitForSelector "body" "UC08-02" "Wait for Body"
            Test-Screenshot "UC08-03" "Take Screenshot"
        }
        "UC-09" {
            Write-TestHeader "UC-09: Target Site Crawl"
            Test-Navigate "https://example.com" "UC09-01" "Navigate to Example"
            Test-WaitForSelector "h1" "UC09-02" "Wait for Title"
            Test-Extract "h1" "UC09-03" "Extract Title"
        }
        "all" {
            Write-TestHeader "Basic API Tests"
            Test-HealthCheck
            Test-GetStatus
            Test-GetCookies
            
            Write-TestHeader "UC-08: Adamas Official Site"
            Test-Navigate "https://shop.adamas-octa.com/" "UC08-01" "Navigate to Shop"
            Test-WaitForSelector "body" "UC08-02" "Wait for Body"
            
            Write-TestHeader "UC-09: Example Site"
            Test-Navigate "https://example.com" "UC09-01" "Navigate to Example"
            Test-WaitForSelector "h1" "UC09-02" "Wait for Title"
            Test-Extract "h1" "UC09-03" "Extract Title"
            
            Write-TestHeader "UC-03: Amazon Search"
            Test-Navigate "https://www.amazon.co.jp/s?k=test" "UC03-01" "Navigate to Amazon"
            Test-WaitForSelector "[data-component-type]" "UC03-02" "Wait for Results"
        }
        "UC-10" {
            Write-TestHeader "UC-10: Login Simulation Test"
            # Step 1: Navigate to login page
            Test-Navigate "https://httpbin.org/cookies/set/session_id/test123" "UC10-01" "Set Test Cookie"
            Start-Sleep -Seconds 2
            
            # Step 2: Verify cookies are set
            Test-GetCookies
            
            # Step 3: Navigate to cookies page to verify
            Test-Navigate "https://httpbin.org/cookies" "UC10-02" "Check Cookies Page"
            Test-WaitForSelector "pre" "UC10-03" "Wait for JSON"
            Test-Extract "pre" "UC10-04" "Extract Cookie Data"
            
            # Step 4: Execute script to get localStorage
            Test-Execute "return navigator.userAgent" "UC10-05" "Get User Agent"
        }
        "login" {
            Write-TestHeader "Login Flow Test (Cookie Persistence)"
            # Test profile-based cookie persistence
            Test-Navigate "https://httpbin.org/cookies/set/auth_token/fake_token_123" "LOGIN-01" "Set Auth Cookie"
            Start-Sleep -Seconds 2
            Test-GetCookies
            Test-Navigate "https://httpbin.org/cookies" "LOGIN-02" "Verify Cookies"
            Test-WaitForSelector "body" "LOGIN-03" "Page Loaded"
        }
        default {
            Write-Host "Unknown test: $TestCase" -ForegroundColor Red
            Write-Host "Available: basic, UC-01 to UC-10, login, all" -ForegroundColor Yellow
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
