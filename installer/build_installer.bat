@echo off
setlocal EnableDelayedExpansion
chcp 65001 >nul

echo.
echo ========================================
echo  WebView Bridge Installer Build
echo ========================================
echo.

REM Navigate to project root
cd /d "%~dp0\.."

REM ---- 1. Check prerequisites ----
echo [1/4] Checking prerequisites...

where makensis >nul 2>&1
if %errorlevel% neq 0 (
    echo ERROR: NSIS not found.
    echo   Install from: https://nsis.sourceforge.io/Download
    echo   Add to PATH:  C:\Program Files ^(x86^)\NSIS
    exit /b 1
)
echo   NSIS: OK

where cargo >nul 2>&1
if %errorlevel% neq 0 (
    echo ERROR: Rust/Cargo not found.
    exit /b 1
)
echo   Cargo: OK

REM ---- 2. Build Rust project ----
echo.
echo [2/4] Building WebView Bridge (release)...
cargo build --release
if %errorlevel% neq 0 (
    echo ERROR: Build failed
    exit /b 1
)
echo   Build: OK

REM ---- 3. Generate installer bitmaps from project images ----
echo.
echo [3/4] Generating installer images...

set NSIS_FLAGS=

powershell -ExecutionPolicy Bypass -NoProfile -Command ^
  "Add-Type -AssemblyName System.Drawing; ^
   $ErrorActionPreference = 'Stop'; ^
   $imgDir = 'docs\img'; ^
   $outDir = 'installer'; ^
   ^
   # --- Header bitmap (150x57) from banner --- ^
   try { ^
     $banner = [System.Drawing.Image]::FromFile(\"$imgDir\Gemini_Generated_Image_p2f3bwp2f3bwp2f3.png\"); ^
     $header = New-Object System.Drawing.Bitmap(150, 57); ^
     $g = [System.Drawing.Graphics]::FromImage($header); ^
     $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic; ^
     $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality; ^
     $srcH = [int]($banner.Width * 57 / 150); ^
     $srcY = [int](($banner.Height - $srcH) / 2); ^
     $g.DrawImage($banner, ^
       (New-Object System.Drawing.Rectangle(0, 0, 150, 57)), ^
       (New-Object System.Drawing.Rectangle(0, $srcY, $banner.Width, $srcH)), ^
       [System.Drawing.GraphicsUnit]::Pixel); ^
     $g.Dispose(); ^
     $banner.Dispose(); ^
     $header.Save(\"$outDir\header.bmp\", [System.Drawing.Imaging.ImageFormat]::Bmp); ^
     $header.Dispose(); ^
     Write-Host '  header.bmp: OK'; ^
   } catch { Write-Host \"  header.bmp: SKIP ($($_.Exception.Message))\"; }; ^
   ^
   # --- Welcome/Finish bitmap (164x314) from icon --- ^
   try { ^
     $icon = [System.Drawing.Image]::FromFile(\"$imgDir\icon.png\"); ^
     $welcome = New-Object System.Drawing.Bitmap(164, 314); ^
     $g = [System.Drawing.Graphics]::FromImage($welcome); ^
     $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic; ^
     $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality; ^
     $g.Clear([System.Drawing.Color]::FromArgb(15, 23, 42)); ^
     $sz = 120; ^
     $x = [int]((164 - $sz) / 2); ^
     $y = [int]((314 - $sz) / 2) - 30; ^
     $g.DrawImage($icon, $x, $y, $sz, $sz); ^
     $font = New-Object System.Drawing.Font('Yu Gothic UI', 9, [System.Drawing.FontStyle]::Bold); ^
     $brush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::White); ^
     $sf = New-Object System.Drawing.StringFormat; ^
     $sf.Alignment = [System.Drawing.StringAlignment]::Center; ^
     $rect = New-Object System.Drawing.RectangleF(0, ($y + $sz + 12), 164, 20); ^
     $g.DrawString('WebView Bridge', $font, $brush, $rect, $sf); ^
     $fontSub = New-Object System.Drawing.Font('Yu Gothic UI', 7); ^
     $brushSub = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(160, 180, 210)); ^
     $rect2 = New-Object System.Drawing.RectangleF(0, ($y + $sz + 30), 164, 16); ^
     $g.DrawString('Autonomous Visual Agent', $fontSub, $brushSub, $rect2, $sf); ^
     $g.Dispose(); ^
     $icon.Dispose(); ^
     $welcome.Save(\"$outDir\welcome.bmp\", [System.Drawing.Imaging.ImageFormat]::Bmp); ^
     $welcome.Dispose(); ^
     Write-Host '  welcome.bmp: OK'; ^
   } catch { Write-Host \"  welcome.bmp: SKIP ($($_.Exception.Message))\"; }; ^
  "

REM Check which BMPs were generated and pass as defines
if exist "installer\header.bmp"  set "NSIS_FLAGS=!NSIS_FLAGS! /DHEADER_BMP=header.bmp"
if exist "installer\welcome.bmp" set "NSIS_FLAGS=!NSIS_FLAGS! /DWELCOME_BMP=welcome.bmp"

REM ---- 4. Build installer ----
echo.
echo [4/4] Building installer...

if not exist dist mkdir dist

makensis !NSIS_FLAGS! installer\webview-bridge.nsi
if %errorlevel% neq 0 (
    echo.
    echo ERROR: NSIS compilation failed
    exit /b 1
)

echo.
echo ========================================
echo  BUILD COMPLETE
echo ========================================
echo  Output: dist\WebViewBridge-3.5.0-Setup.exe
echo ========================================
echo.
