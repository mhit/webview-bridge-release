Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$installerDir = Join-Path $root "installer"
$iconPath = Join-Path $root "docs\img\icon.ico"
$icon128Path = Join-Path $root "docs\img\icon-128.png"

# ============================================================
# Generate header bitmap (150x57) — right-aligned icon + product name
# ============================================================
$headerBmp = New-Object System.Drawing.Bitmap(150, 57)
$g = [System.Drawing.Graphics]::FromImage($headerBmp)
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
$g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit

# Background gradient (dark blue)
$headerBrush = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
    (New-Object System.Drawing.Point(0, 0)),
    (New-Object System.Drawing.Point(150, 0)),
    [System.Drawing.Color]::FromArgb(35, 45, 85),
    [System.Drawing.Color]::FromArgb(55, 70, 130)
)
$g.FillRectangle($headerBrush, 0, 0, 150, 57)
$headerBrush.Dispose()

# Draw icon (right side, 40x40)
if (Test-Path $icon128Path) {
    $iconImg = [System.Drawing.Image]::FromFile($icon128Path)
    $g.DrawImage($iconImg, 102, 8, 40, 40)
    $iconImg.Dispose()
}

$g.Dispose()
$headerPath = Join-Path $installerDir "header.bmp"
$headerBmp.Save($headerPath, [System.Drawing.Imaging.ImageFormat]::Bmp)
$headerBmp.Dispose()
Write-Host "Generated: $headerPath"

# ============================================================
# Generate welcome bitmap (164x314) — icon + product name + tagline
# ============================================================
$welcomeBmp = New-Object System.Drawing.Bitmap(164, 314)
$g = [System.Drawing.Graphics]::FromImage($welcomeBmp)
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
$g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit

# Background gradient (dark blue vertical)
$welcomeBrush = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
    (New-Object System.Drawing.Point(0, 0)),
    (New-Object System.Drawing.Point(0, 314)),
    [System.Drawing.Color]::FromArgb(35, 45, 85),
    [System.Drawing.Color]::FromArgb(20, 25, 55)
)
$g.FillRectangle($welcomeBrush, 0, 0, 164, 314)
$welcomeBrush.Dispose()

# Draw icon (centered, 80x80)
if (Test-Path $icon128Path) {
    $iconImg = [System.Drawing.Image]::FromFile($icon128Path)
    $g.DrawImage($iconImg, 42, 40, 80, 80)
    $iconImg.Dispose()
}

# Product name
$titleFont = New-Object System.Drawing.Font("Segoe UI", 11, [System.Drawing.FontStyle]::Bold)
$whiteBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::White)
$sf = New-Object System.Drawing.StringFormat
$sf.Alignment = [System.Drawing.StringAlignment]::Center
$titleRect = New-Object System.Drawing.RectangleF(0, 130, 164, 30)
$g.DrawString("WebView Bridge", $titleFont, $whiteBrush, $titleRect, $sf)
$titleFont.Dispose()

# Version
$verFont = New-Object System.Drawing.Font("Segoe UI", 8, [System.Drawing.FontStyle]::Regular)
$lightBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(180, 190, 220))
$verRect = New-Object System.Drawing.RectangleF(0, 158, 164, 20)
$g.DrawString("v3.7.0", $verFont, $lightBrush, $verRect, $sf)
$verFont.Dispose()

# Tagline
$tagFont = New-Object System.Drawing.Font("Segoe UI", 7, [System.Drawing.FontStyle]::Regular)
$tagRect = New-Object System.Drawing.RectangleF(5, 190, 154, 40)
$g.DrawString("AI Browser Automation`nMCP Server", $tagFont, $lightBrush, $tagRect, $sf)
$tagFont.Dispose()

# Decorative line
$linePen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(80, 100, 180), 1)
$g.DrawLine($linePen, 30, 240, 134, 240)
$linePen.Dispose()

# Bottom text
$bottomFont = New-Object System.Drawing.Font("Segoe UI", 6.5, [System.Drawing.FontStyle]::Regular)
$bottomRect = New-Object System.Drawing.RectangleF(5, 255, 154, 40)
$g.DrawString("WebView2 + CDP`nWindows 10/11", $bottomFont, $lightBrush, $bottomRect, $sf)
$bottomFont.Dispose()

$lightBrush.Dispose()
$whiteBrush.Dispose()
$sf.Dispose()
$g.Dispose()

$welcomePath = Join-Path $installerDir "welcome.bmp"
$welcomeBmp.Save($welcomePath, [System.Drawing.Imaging.ImageFormat]::Bmp)
$welcomeBmp.Dispose()
Write-Host "Generated: $welcomePath"

# ============================================================
# Compile NSIS
# ============================================================
$nsiPath = Join-Path $installerDir "webview-bridge.nsi"
$nsisExe = "C:\Program Files (x86)\NSIS\makensis.exe"

if (-not (Test-Path $nsisExe)) {
    Write-Host "ERROR: NSIS not found at $nsisExe"
    exit 1
}

$nsisArgs = @(
    "/DHEADER_BMP=$headerPath",
    "/DWELCOME_BMP=$welcomePath",
    $nsiPath
)

Write-Host "Running makensis..."
& $nsisExe $nsisArgs
if ($LASTEXITCODE -eq 0) {
    Write-Host "Installer built successfully!"
    $installer = Join-Path $root "dist\WebViewBridge-3.6.0-Setup.exe"
    if (Test-Path $installer) {
        $fi = Get-Item $installer
        Write-Host "Output: $installer ($([math]::Round($fi.Length / 1024 / 1024, 1)) MB)"
    }
} else {
    Write-Host "ERROR: makensis failed with exit code $LASTEXITCODE"
    exit 1
}
