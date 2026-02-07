# Rakuten Adamasocta Search Script using WBP2
$BaseUrl = "http://127.0.0.1:9400"
$SessionName = "rakuten_search"
$OutputDir = "C:\Users\mhit\Documents\GitHub\webview-bridge\output\adamasocta"

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

Write-Host "=== WBP2 Rakuten Adamasocta Search ===" -ForegroundColor Cyan

# 1. Acquire session
Write-Host "`n[1] Acquiring session..." -ForegroundColor Yellow
$acquireBody = '{"name":"' + $SessionName + '","headless":false}'
$session = Invoke-RestMethod -Uri "$BaseUrl/v2/session/acquire" -Method Post -Body $acquireBody -ContentType "application/json"
Write-Host "  Session: $($session.session)" -ForegroundColor Green

Start-Sleep -Seconds 3

# 2. Navigate to Rakuten search
Write-Host "`n[2] Navigating to Rakuten..." -ForegroundColor Yellow
$navBody = '{"session":"' + $SessionName + '","type":"navigate","target":"https://search.rakuten.co.jp/search/mall/adamasocta/","timeout_ms":30000}'
$navResult = Invoke-RestMethod -Uri "$BaseUrl/v2/goal" -Method Post -Body $navBody -ContentType "application/json"
Write-Host "  Navigation: $($navResult.success)" -ForegroundColor Green

Start-Sleep -Seconds 5

# 3. Wait for search results
Write-Host "`n[3] Waiting for results..." -ForegroundColor Yellow
$waitBody = '{"session":"' + $SessionName + '","selector":".searchresultitem","condition":"present","timeout_ms":15000}'
$waitResult = Invoke-RestMethod -Uri "$BaseUrl/v2/wait" -Method Post -Body $waitBody -ContentType "application/json"
Write-Host "  Found: $($waitResult.found)" -ForegroundColor Green

# 4. Take screenshot
Write-Host "`n[4] Taking screenshot..." -ForegroundColor Yellow
$screenshotBody = '{"session":"' + $SessionName + '","mode":"viewport","format":"png","timeout_ms":10000}'
$screenshotResult = Invoke-RestMethod -Uri "$BaseUrl/v2/screenshot" -Method Post -Body $screenshotBody -ContentType "application/json"
if ($screenshotResult.image) {
    $imageBytes = [Convert]::FromBase64String($screenshotResult.image)
    $screenshotPath = Join-Path $OutputDir "search_results.png"
    [System.IO.File]::WriteAllBytes($screenshotPath, $imageBytes)
    Write-Host "  Saved: $screenshotPath" -ForegroundColor Green
}

# 5. Extract product data via JavaScript
Write-Host "`n[5] Extracting product data..." -ForegroundColor Yellow
$extractScript = "(function(){var items=document.querySelectorAll('.searchresultitem');var products=[];items.forEach(function(item,i){if(i>=10)return;var title=item.querySelector('.title')?.textContent?.trim();var price=item.querySelector('.price')?.textContent?.trim();var link=item.querySelector('a')?.href;var img=item.querySelector('img')?.src;if(title){products.push({index:i+1,title:title.substring(0,80),price:price,link:link,image:img});}});return JSON.stringify(products);})();"

$extractBody = @{
    session    = $SessionName
    type       = "execute_script"
    target     = $extractScript
    timeout_ms = 15000
} | ConvertTo-Json

$extractResult = Invoke-RestMethod -Uri "$BaseUrl/v2/goal" -Method Post -Body $extractBody -ContentType "application/json"
Write-Host "  Extract: $($extractResult.success)" -ForegroundColor Green

# Save result
$extractResult | ConvertTo-Json -Depth 10 | Out-File -FilePath (Join-Path $OutputDir "products.json") -Encoding utf8

# 6. Click first product
Write-Host "`n[6] Opening first product..." -ForegroundColor Yellow
$clickBody = '{"session":"' + $SessionName + '","type":"click","target":".searchresultitem a.title","timeout_ms":10000}'
$clickResult = Invoke-RestMethod -Uri "$BaseUrl/v2/goal" -Method Post -Body $clickBody -ContentType "application/json"
Write-Host "  Click: $($clickResult.success)" -ForegroundColor Green

Start-Sleep -Seconds 5

# 7. Screenshot of product page
Write-Host "`n[7] Product page screenshot..." -ForegroundColor Yellow
$detailScreenshot = Invoke-RestMethod -Uri "$BaseUrl/v2/screenshot" -Method Post -Body $screenshotBody -ContentType "application/json"
if ($detailScreenshot.image) {
    $imageBytes = [Convert]::FromBase64String($detailScreenshot.image)
    $detailPath = Join-Path $OutputDir "product_detail.png"
    [System.IO.File]::WriteAllBytes($detailPath, $imageBytes)
    Write-Host "  Saved: $detailPath" -ForegroundColor Green
}

Write-Host "`n=== Complete ===" -ForegroundColor Cyan
Write-Host "Output: $OutputDir" -ForegroundColor Green
