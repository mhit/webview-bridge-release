Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$installerDir = Join-Path $root "installer"
$iconPath = Join-Path $root "docs\img\icon.ico"

# Generate header bitmap (150x57)
$headerBmp = New-Object System.Drawing.Bitmap(150, 57)
$g = [System.Drawing.Graphics]::FromImage($headerBmp)
$g.Clear([System.Drawing.Color]::FromArgb(245, 245, 250))
$g.Dispose()
$headerPath = Join-Path $installerDir "header.bmp"
$headerBmp.Save($headerPath, [System.Drawing.Imaging.ImageFormat]::Bmp)
$headerBmp.Dispose()
Write-Host "Generated: $headerPath"

# Generate welcome bitmap (164x314)
$welcomeBmp = New-Object System.Drawing.Bitmap(164, 314)
$g = [System.Drawing.Graphics]::FromImage($welcomeBmp)
$brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
    (New-Object System.Drawing.Point(0, 0)),
    (New-Object System.Drawing.Point(0, 314)),
    [System.Drawing.Color]::FromArgb(60, 80, 170),
    [System.Drawing.Color]::FromArgb(30, 40, 100)
)
$g.FillRectangle($brush, 0, 0, 164, 314)
$brush.Dispose()
$g.Dispose()
$welcomePath = Join-Path $installerDir "welcome.bmp"
$welcomeBmp.Save($welcomePath, [System.Drawing.Imaging.ImageFormat]::Bmp)
$welcomeBmp.Dispose()
Write-Host "Generated: $welcomePath"

# Compile NSIS
$nsiPath = Join-Path $installerDir "webview-bridge.nsi"
$nsisExe = "C:\Program Files (x86)\NSIS\makensis.exe"

if (-not (Test-Path $nsisExe)) {
    Write-Host "ERROR: NSIS not found at $nsisExe"
    exit 1
}

$args = @(
    "/DHEADER_BMP=$headerPath",
    "/DWELCOME_BMP=$welcomePath",
    $nsiPath
)

Write-Host "Running makensis..."
& $nsisExe $args
if ($LASTEXITCODE -eq 0) {
    Write-Host "Installer built successfully!"
    $installer = Join-Path $root "dist\WebViewBridge-3.5.0-Setup.exe"
    if (Test-Path $installer) {
        $fi = Get-Item $installer
        Write-Host "Output: $installer ($([math]::Round($fi.Length / 1024 / 1024, 1)) MB)"
    }
} else {
    Write-Host "ERROR: makensis failed with exit code $LASTEXITCODE"
    exit 1
}
