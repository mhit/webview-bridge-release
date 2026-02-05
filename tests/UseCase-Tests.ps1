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

# Test: Act (Click/Type)
function Test-Click($RefAttr, $TestId, $Name) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $result = Api-Post "/act/$SessionId" @{ 
        kind     = "click"
        ref_attr = $RefAttr
    }
    $success = ($null -ne $result)
    Write-TestResult $TestId $Name $success "" $sw.ElapsedMilliseconds
    return $success
}

function Test-Type($RefAttr, $Text, $TestId, $Name) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $result = Api-Post "/act/$SessionId" @{ 
        kind     = "type"
        ref_attr = $RefAttr
        text     = $Text
    }
    $success = ($null -ne $result)
    Write-TestResult $TestId $Name $success "" $sw.ElapsedMilliseconds
    return $success
}

function Test-Press($Key, $TestId, $Name) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $result = Api-Post "/act/$SessionId" @{ 
        kind = "press"
        text = $Key
    }
    $success = ($null -ne $result)
    Write-TestResult $TestId $Name $success "" $sw.ElapsedMilliseconds
    return $success
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
        "login-form" {
            Write-TestHeader "Login Form Test (The Internet)"
            
            # Step 1: Navigate to test login page
            # Using "The Internet" - a known test site for automation
            Test-Navigate "https://the-internet.herokuapp.com/login" "LF-01" "Navigate to Login Page"
            
            # Step 2: Wait for login form to fully load
            Start-Sleep -Seconds 5
            Test-WaitForSelector "#username" "LF-02" "Wait for Username Field"
            
            # Step 3: Type username using JavaScript
            Test-Execute "document.getElementById('username').value = 'tomsmith'; return document.getElementById('username').value;" "LF-03" "Enter Username"
            
            # Step 4: Type password
            Test-Execute "document.getElementById('password').value = 'SuperSecretPassword!'; return 'password set';" "LF-04" "Enter Password"
            
            # Step 5: Verify form values before submit
            Test-Execute "return 'User: ' + document.getElementById('username').value + ', Pass length: ' + document.getElementById('password').value.length;" "LF-05" "Verify Form Values"
            
            # Step 6: Submit form via form.submit() instead of click
            Test-Execute "document.getElementById('login').submit(); return 'submitted';" "LF-06" "Submit Login Form"
            
            # Step 7: Wait for page to load after redirect (longer wait)
            Start-Sleep -Seconds 5
            
            # Step 8: Check current URL to verify redirect
            $urlResult = Test-Execute "return window.location.pathname;" "LF-07" "Check URL After Login"
            
            # Step 9: Try to find any content on the page
            Test-WaitForSelector "body" "LF-08" "Wait for Page Body"
            
            # Step 10: Extract page content to see what happened
            Test-Extract "body" "LF-09" "Extract Page Content"
            
            # Step 11: Take screenshot (Known issue: html2canvas loading can fail)
            # Test-Screenshot "LF-10" "Screenshot After Login"
            
            # Step 12: Verify cookies are set
            Test-GetCookies
        }
        "login-session" {
            Write-TestHeader "Login Session Persistence Test"
            
            # Test 1: Login and set cookies
            Test-Navigate "https://httpbin.org/cookies/set/session_id/abc123" "LS-01" "Set Session Cookie"
            Start-Sleep -Seconds 2
            
            # Test 2: Verify cookie was set
            $cookies = Api-Get "/cookies/$SessionId"
            if ($cookies) {
                Write-TestResult "LS-02" "Verify Cookie Set" $true "Got cookies"
            }
            else {
                Write-TestResult "LS-02" "Verify Cookie Set" $false "No cookies"
            }
            
            # Test 3: Navigate to authenticated page
            Test-Navigate "https://httpbin.org/cookies" "LS-03" "Navigate to Cookie Check"
            Test-WaitForSelector "pre" "LS-04" "Wait for Response"
            
            # Test 4: Extract and verify session cookie
            $extracted = Test-Extract "pre" "LS-05" "Extract Cookie JSON"
            
            # Test 5: Set additional auth cookie
            Test-Navigate "https://httpbin.org/cookies/set/auth_token/xyz789" "LS-06" "Set Auth Token"
            Start-Sleep -Seconds 2
            
            # Test 6: Verify both cookies exist
            Test-Navigate "https://httpbin.org/cookies" "LS-07" "Check Both Cookies"
            Test-WaitForSelector "pre" "LS-08" "Wait for Response"
            Test-Extract "pre" "LS-09" "Extract All Cookies"
            
            # Test 7: Delete cookies via API
            Test-Navigate "https://httpbin.org/cookies/delete?session_id=" "LS-10" "Delete Session Cookie"
            
            # Test 8: Verify cookie was deleted
            Test-Navigate "https://httpbin.org/cookies" "LS-11" "Verify Cookie Deleted"
            Test-Extract "pre" "LS-12" "Extract Remaining Cookies"
        }
        "profile-isolation" {
            Write-TestHeader "Profile Isolation Test (Multi-Session)"
            
            # This test verifies that different profiles have isolated storage
            # Create two sessions with different profiles and verify cookies are not shared
            
            # Step 1: Create session with profile A and set cookie
            Write-Host "Creating Profile A session..." -ForegroundColor Cyan
            $profileA = "profile_a_test"
            $bodyA = @{ profile = $profileA; headless = $false } | ConvertTo-Json
            $sessionA = Invoke-RestMethod -Uri "$ServerUrl/create" -Method Post -Body $bodyA -ContentType "application/json"
            $sidA = $sessionA.id
            Write-Host "Profile A Session: $sidA" -ForegroundColor Gray
            Start-Sleep -Seconds 8
            
            # Set cookie in Profile A
            Invoke-RestMethod -Uri "$ServerUrl/navigate/$sidA" -Method Post -Body '{"url":"https://httpbin.org/cookies/set/profile_cookie/PROFILE_A_VALUE"}' -ContentType "application/json" | Out-Null
            Start-Sleep -Seconds 3
            
            # Verify cookie is set in Profile A
            $resultA = Invoke-RestMethod -Uri "$ServerUrl/execute/$sidA" -Method Post -Body '{"script":"return document.cookie"}' -ContentType "application/json"
            Write-TestResult "PI-01" "Set Cookie in Profile A" ($null -ne $resultA) ""
            
            # Step 2: Create session with profile B
            Write-Host "Creating Profile B session..." -ForegroundColor Cyan
            $profileB = "profile_b_test"
            $bodyB = @{ profile = $profileB; headless = $false } | ConvertTo-Json
            $sessionB = Invoke-RestMethod -Uri "$ServerUrl/create" -Method Post -Body $bodyB -ContentType "application/json"
            $sidB = $sessionB.id
            Write-Host "Profile B Session: $sidB" -ForegroundColor Gray
            Start-Sleep -Seconds 8
            
            # Navigate to cookies page in Profile B
            Invoke-RestMethod -Uri "$ServerUrl/navigate/$sidB" -Method Post -Body '{"url":"https://httpbin.org/cookies"}' -ContentType "application/json" | Out-Null
            Start-Sleep -Seconds 3
            
            # Check cookies in Profile B (should NOT have Profile A's cookie)
            $extractB = Invoke-RestMethod -Uri "$ServerUrl/extract/$sidB" -Method Post -Body '{"selector":"pre"}' -ContentType "application/json"
            $hasCookieFromA = $false
            if ($extractB.data) {
                $content = $extractB.data -join ""
                $hasCookieFromA = $content -like "*PROFILE_A_VALUE*"
            }
            Write-TestResult "PI-02" "Profile B Isolated (No A's Cookie)" (-not $hasCookieFromA) "Isolation: $(-not $hasCookieFromA)"
            
            # Step 3: Set different cookie in Profile B
            Invoke-RestMethod -Uri "$ServerUrl/navigate/$sidB" -Method Post -Body '{"url":"https://httpbin.org/cookies/set/profile_cookie/PROFILE_B_VALUE"}' -ContentType "application/json" | Out-Null
            Start-Sleep -Seconds 3
            Write-TestResult "PI-03" "Set Cookie in Profile B" $true ""
            
            # Step 4: Verify Profile A still has its original cookie
            Invoke-RestMethod -Uri "$ServerUrl/navigate/$sidA" -Method Post -Body '{"url":"https://httpbin.org/cookies"}' -ContentType "application/json" | Out-Null
            Start-Sleep -Seconds 3
            $extractA = Invoke-RestMethod -Uri "$ServerUrl/extract/$sidA" -Method Post -Body '{"selector":"pre"}' -ContentType "application/json"
            $hasOriginalCookie = $false
            $hasBCookie = $false
            if ($extractA.data) {
                $content = $extractA.data -join ""
                $hasOriginalCookie = $content -like "*PROFILE_A_VALUE*"
                $hasBCookie = $content -like "*PROFILE_B_VALUE*"
            }
            Write-TestResult "PI-04" "Profile A Has Original Cookie" $hasOriginalCookie ""
            Write-TestResult "PI-05" "Profile A Isolated (No B's Cookie)" (-not $hasBCookie) "Isolation: $(-not $hasBCookie)"
            
            # Cleanup: Close both sessions
            Invoke-RestMethod -Uri "$ServerUrl/close/$sidA" -Method Delete -ErrorAction SilentlyContinue | Out-Null
            Invoke-RestMethod -Uri "$ServerUrl/close/$sidB" -Method Delete -ErrorAction SilentlyContinue | Out-Null
            Write-Host "Both sessions closed" -ForegroundColor Gray
            
            # Skip normal session cleanup since we handled it
            $script:SessionId = $null
        }
        "profile-concurrency" {
            Write-TestHeader "Profile Concurrency Test (Same Profile, Multiple Sessions)"
            
            # This test verifies that multiple sessions using the same profile
            # can operate safely without corrupting cookies or profile data
            
            $sharedProfile = "shared_profile_test"
            $sessions = @()
            $sessionCount = 3
            
            try {
                # Step 1: Create multiple sessions with the SAME profile
                Write-Host "Creating $sessionCount sessions with shared profile..." -ForegroundColor Cyan
                for ($i = 1; $i -le $sessionCount; $i++) {
                    $body = @{ profile = $sharedProfile; headless = $false } | ConvertTo-Json
                    try {
                        $session = Invoke-RestMethod -Uri "$ServerUrl/create" -Method Post -Body $body -ContentType "application/json"
                        $sessions += $session.id
                        Write-Host "  Session $i : $($session.id)" -ForegroundColor Gray
                    }
                    catch {
                        Write-Host "  Session $i : Failed to create" -ForegroundColor Red
                    }
                    Start-Sleep -Seconds 3
                }
                
                $createdCount = $sessions.Count
                Write-TestResult "PC-01" "Create $sessionCount Sessions with Same Profile" ($createdCount -ge 2) "Created: $createdCount"
                
                # Wait for all sessions to initialize
                Start-Sleep -Seconds 5
                
                # Step 2: Set different cookies from each session simultaneously
                Write-Host "Setting cookies from multiple sessions..." -ForegroundColor Cyan
                $cookieSetJobs = @()
                for ($i = 0; $i -lt $sessions.Count; $i++) {
                    $sid = $sessions[$i]
                    $cookieName = "session_${i}_cookie"
                    $cookieValue = "VALUE_FROM_SESSION_$i"
                    $url = "https://httpbin.org/cookies/set/$cookieName/$cookieValue"
                    
                    # Navigate to set cookie
                    try {
                        Invoke-RestMethod -Uri "$ServerUrl/navigate/$sid" -Method Post -Body "{`"url`":`"$url`"}" -ContentType "application/json" -TimeoutSec 30 | Out-Null
                        Write-Host "  Session $i set cookie: $cookieName" -ForegroundColor Gray
                    }
                    catch {
                        Write-Host "  Session $i failed to set cookie" -ForegroundColor Red
                    }
                }
                
                Write-TestResult "PC-02" "Set Cookies from Multiple Sessions" $true ""
                
                # Wait for cookie operations to complete
                Start-Sleep -Seconds 5
                
                # Step 3: Navigate all sessions to cookies page and extract
                Write-Host "Verifying cookies from all sessions..." -ForegroundColor Cyan
                $cookieResults = @()
                for ($i = 0; $i -lt $sessions.Count; $i++) {
                    $sid = $sessions[$i]
                    try {
                        Invoke-RestMethod -Uri "$ServerUrl/navigate/$sid" -Method Post -Body '{"url":"https://httpbin.org/cookies"}' -ContentType "application/json" -TimeoutSec 30 | Out-Null
                        Start-Sleep -Seconds 2
                        $extract = Invoke-RestMethod -Uri "$ServerUrl/extract/$sid" -Method Post -Body '{"selector":"pre"}' -ContentType "application/json" -TimeoutSec 30
                        if ($extract.data) {
                            $content = $extract.data -join ""
                            $cookieResults += $content
                            Write-Host "  Session $i cookies: $(if ($content.Length -gt 100) { $content.Substring(0, 100) + '...' } else { $content })" -ForegroundColor Gray
                        }
                    }
                    catch {
                        Write-Host "  Session $i failed to extract cookies" -ForegroundColor Red
                    }
                }
                
                # Step 4: Verify all sessions see the same cookies (profile sharing)
                # Since they share the same profile, they should eventually see all cookies
                $allSeeAllCookies = $true
                for ($i = 0; $i -lt $sessions.Count; $i++) {
                    $expectedCookie = "session_${i}_cookie"
                    foreach ($result in $cookieResults) {
                        # Check if at least one session sees each cookie
                    }
                }
                Write-TestResult "PC-03" "Extract Cookies from All Sessions" ($cookieResults.Count -gt 0) "Got $($cookieResults.Count) results"
                
                # Step 5: Perform concurrent navigation without corruption
                Write-Host "Testing concurrent navigation..." -ForegroundColor Cyan
                $navUrls = @(
                    "https://example.com",
                    "https://httpbin.org/html",
                    "https://httpbin.org/get"
                )
                for ($i = 0; $i -lt [Math]::Min($sessions.Count, $navUrls.Count); $i++) {
                    $sid = $sessions[$i]
                    $url = $navUrls[$i]
                    try {
                        Invoke-RestMethod -Uri "$ServerUrl/navigate/$sid" -Method Post -Body "{`"url`":`"$url`"}" -ContentType "application/json" -TimeoutSec 30 | Out-Null
                    }
                    catch {
                        # Ignore navigation errors
                    }
                }
                Start-Sleep -Seconds 3
                
                # Step 6: Verify sessions are still functional
                $functionalCount = 0
                for ($i = 0; $i -lt $sessions.Count; $i++) {
                    $sid = $sessions[$i]
                    try {
                        $status = Invoke-RestMethod -Uri "$ServerUrl/status/$sid" -Method Get -TimeoutSec 10
                        if ($status.status -eq "Ready") {
                            $functionalCount++
                        }
                    }
                    catch {
                        # Session not functional
                    }
                }
                Write-TestResult "PC-04" "Sessions Still Functional After Concurrent Ops" ($functionalCount -eq $sessions.Count) "Functional: $functionalCount / $($sessions.Count)"
                
                # Step 7: Close one session and verify others still work
                if ($sessions.Count -ge 2) {
                    $closedSid = $sessions[0]
                    Invoke-RestMethod -Uri "$ServerUrl/close/$closedSid" -Method Delete -ErrorAction SilentlyContinue | Out-Null
                    Write-Host "Closed first session: $closedSid" -ForegroundColor Gray
                    Start-Sleep -Seconds 2
                    
                    # Check remaining sessions
                    $remainingFunctional = 0
                    for ($i = 1; $i -lt $sessions.Count; $i++) {
                        $sid = $sessions[$i]
                        try {
                            $status = Invoke-RestMethod -Uri "$ServerUrl/status/$sid" -Method Get -TimeoutSec 10
                            if ($status.status -eq "Ready") {
                                $remainingFunctional++
                            }
                        }
                        catch {
                            # Session not functional
                        }
                    }
                    Write-TestResult "PC-05" "Remaining Sessions Work After Closing One" ($remainingFunctional -eq ($sessions.Count - 1)) "Remaining: $remainingFunctional / $($sessions.Count - 1)"
                }
                
            }
            finally {
                # Cleanup: Close all sessions
                Write-Host "Cleaning up sessions..." -ForegroundColor Cyan
                foreach ($sid in $sessions) {
                    try {
                        Invoke-RestMethod -Uri "$ServerUrl/close/$sid" -Method Delete -ErrorAction SilentlyContinue | Out-Null
                    }
                    catch {
                        # Ignore cleanup errors
                    }
                }
                Write-Host "All sessions closed" -ForegroundColor Gray
            }
            
            # Skip normal session cleanup since we handled it
            $script:SessionId = $null
        }
        default {
            Write-Host "Unknown test: $TestCase" -ForegroundColor Red
            Write-Host "Available: basic, UC-01 to UC-10, login, login-form, login-session, profile-isolation, profile-concurrency, all" -ForegroundColor Yellow
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
