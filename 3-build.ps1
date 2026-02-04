# Cap aniBullet - Build Script

$OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
chcp 65001 > $null

Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  Cap aniBullet - Build Application" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

# Check dependencies
if (-not (Test-Path "node_modules")) {
    Write-Host "ERROR: node_modules not found" -ForegroundColor Red
    Write-Host "Please run: .\1-install.ps1" -ForegroundColor Yellow
    exit 1
}

# Build env: refresh PATH and set vars from 1-install layout (no per-machine special cases)
$env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path", "User")
$ffmpegDevDir = "$env:USERPROFILE\.ffmpeg-dev"
if ($env:OS -eq "Windows_NT" -and (Test-Path "$ffmpegDevDir\include")) {
    $env:FFMPEG_DIR = $ffmpegDevDir
    $env:FFMPEG_INCLUDE_DIR = "$ffmpegDevDir\include"
    $env:FFMPEG_LIB_DIR = "$ffmpegDevDir\lib"
}
if ($env:OS -eq "Windows_NT") {
    $hostTarget = rustc -vV 2>$null | Select-String "host:"
    if ($hostTarget -match "pc-windows-gnu") { Write-Host "Tip: rustup default stable-x86_64-pc-windows-msvc" -ForegroundColor Yellow }
}

# Ask build type
Write-Host "Select build type:" -ForegroundColor Yellow
Write-Host "  [1] Development (Fast, for testing)" -ForegroundColor White
Write-Host "  [2] Production Release (Optimized, for distribution)" -ForegroundColor White
Write-Host ""
$choice = Read-Host "Enter option (1 or 2)"

$buildMode = ""

switch ($choice) {
    "1" {
        $buildMode = "Development"
        Write-Host ""
        Write-Host "Selected: Development build" -ForegroundColor Green
    }
    "2" {
        $buildMode = "Production"
        Write-Host ""
        Write-Host "Selected: Production build" -ForegroundColor Green
    }
    default {
        Write-Host ""
        Write-Host "Invalid option, using Development" -ForegroundColor Yellow
        $buildMode = "Development"
    }
}

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  Starting $buildMode Build" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "This may take 10-30 minutes..." -ForegroundColor Gray
Write-Host "Please ensure:" -ForegroundColor Yellow
Write-Host "  1. Stable network connection" -ForegroundColor White
Write-Host "  2. Sufficient disk space (10GB+)" -ForegroundColor White
Write-Host "  3. Keep this window open" -ForegroundColor White
Write-Host ""

$startTime = Get-Date

# Build
Push-Location "apps\desktop"

if ($buildMode -eq "Production") {
    Write-Host "[1/1] Building Production version..." -ForegroundColor Yellow
    Write-Host ""
    pnpm build:tauri --config src-tauri/tauri.prod.conf.json
} else {
    Write-Host "[1/1] Building Development version..." -ForegroundColor Yellow
    Write-Host ""
    pnpm build:tauri
}

$buildExitCode = $LASTEXITCODE
$endTime = Get-Date
$duration = $endTime - $startTime

Pop-Location

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan

if ($buildExitCode -eq 0) {
    Write-Host "  Build Successful!" -ForegroundColor Green
    Write-Host "================================================================" -ForegroundColor Green
    Write-Host ""
    Write-Host "Build info:" -ForegroundColor Cyan
    Write-Host "  Mode: $buildMode" -ForegroundColor White
    Write-Host "  Duration: $($duration.Minutes)m $($duration.Seconds)s" -ForegroundColor White
    Write-Host ""
    
    $outputPath = "apps\desktop\src-tauri\target\release"
    $bundlePath = "apps\desktop\src-tauri\target\release\bundle"
    
    if ($buildMode -eq "Production") {
        Write-Host "Product Name: Cap aniBullet (Production)" -ForegroundColor Cyan
        Write-Host "Identifier: so.cap.desktop.anibullet" -ForegroundColor White
    } else {
        Write-Host "Product Name: Cap aniBullet - Development" -ForegroundColor Cyan
        Write-Host "Identifier: so.cap.desktop.anibullet.dev" -ForegroundColor White
    }
    
    Write-Host ""
    Write-Host "Output:" -ForegroundColor Cyan
    Write-Host "  Executable: $outputPath" -ForegroundColor White
    Write-Host "  Installer: $bundlePath" -ForegroundColor White
    
    # List installers
    Write-Host ""
    Write-Host "Generated installers:" -ForegroundColor Cyan
    
    $msiPath = "apps\desktop\src-tauri\target\release\bundle\msi"
    $nsiPath = "apps\desktop\src-tauri\target\release\bundle\nsis"
    
    if (Test-Path $msiPath) {
        $msiFiles = Get-ChildItem $msiPath -Filter "*.msi" -ErrorAction SilentlyContinue
        foreach ($file in $msiFiles) {
            $sizeMB = [math]::Round($file.Length / 1MB, 2)
            Write-Host "  OK $($file.Name) ($sizeMB MB)" -ForegroundColor Green
        }
    }
    
    if (Test-Path $nsiPath) {
        $exeFiles = Get-ChildItem $nsiPath -Filter "*.exe" -ErrorAction SilentlyContinue
        foreach ($file in $exeFiles) {
            $sizeMB = [math]::Round($file.Length / 1MB, 2)
            Write-Host "  OK $($file.Name) ($sizeMB MB)" -ForegroundColor Green
        }
    }
    
    Write-Host ""
    exit 0
    
} else {
    Write-Host "  Build Failed" -ForegroundColor Red
    Write-Host "================================================================" -ForegroundColor Red
    Write-Host ""
    Write-Host "Duration: $($duration.Minutes)m $($duration.Seconds)s" -ForegroundColor White
    Write-Host ""
    Write-Host "If build failed: run .\1-install.ps1, restart terminal, then retry. See README for requirements." -ForegroundColor Yellow
    Write-Host ""
    exit 1
}
