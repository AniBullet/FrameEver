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

# Ensure FFmpeg dev env is set for ffmpeg-sys-next (Windows)
$ffmpegDevDir = "$env:USERPROFILE\.ffmpeg-dev"
if ($IsWindows -ne $false -and (Test-Path "$ffmpegDevDir\include")) {
    if (-not $env:FFMPEG_DIR) {
        $env:FFMPEG_DIR = $ffmpegDevDir
        $env:FFMPEG_INCLUDE_DIR = "$ffmpegDevDir\include"
        $env:FFMPEG_LIB_DIR = "$ffmpegDevDir\lib"
        Write-Host "FFmpeg dev: using $ffmpegDevDir (FFMPEG_DIR)" -ForegroundColor Gray
    }
}

# Recommend MSVC toolchain on Windows (avoids pkg-config requirement)
$rustTarget = rustc -vV 2>$null | Select-String "host:"
if ($rustTarget -match "pc-windows-gnu") {
    Write-Host ""
    Write-Host "WARNING: You are using the GNU toolchain (pc-windows-gnu)." -ForegroundColor Yellow
    Write-Host "  FFmpeg build often fails with 'pkg-config could not be found'." -ForegroundColor Yellow
    Write-Host "  Switch to MSVC toolchain and retry:" -ForegroundColor Yellow
    Write-Host "    rustup default stable-x86_64-pc-windows-msvc" -ForegroundColor White
    Write-Host ""
}

# Use CMake that supports "Visual Studio 17 2022" (whisper-rs-sys); VS2019's CMake does not
if ($env:OS -eq "Windows_NT") {
    $preferredDirs = @(
        "$env:ProgramFiles\CMake\bin",
        "$env:ProgramFiles(x86)\CMake\bin"
    )
    $vs2022Cmake = Get-ChildItem -Path "$env:ProgramFiles(x86)\Microsoft Visual Studio\2022" -Recurse -Filter "cmake.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($vs2022Cmake) { $preferredDirs = @($vs2022Cmake.DirectoryName) + $preferredDirs }
    $preferredCmakeDir = $preferredDirs | Where-Object { Test-Path $_ } | Select-Object -First 1
    $currentCmake = (Get-Command cmake -ErrorAction SilentlyContinue).Source
    if ($currentCmake -match "2019\\Community" -and $preferredCmakeDir) {
        $env:Path = "$preferredCmakeDir;$env:Path"
        Write-Host "CMake: using build-compatible path (VS 2022 / Kitware)" -ForegroundColor Gray
    }
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
    Write-Host "Common issues:" -ForegroundColor Yellow
    Write-Host "  1. Run .\1-install.ps1 first (installs FFmpeg dev + sets FFMPEG_DIR)" -ForegroundColor White
    Write-Host "  2. If you see 'pkg-config could not be found': use MSVC toolchain:" -ForegroundColor White
    Write-Host "     rustup default stable-x86_64-pc-windows-msvc" -ForegroundColor Gray
    Write-Host "  3. If you see 'Could not create named generator Visual Studio 17 2022':" -ForegroundColor White
    Write-Host "     Run .\1-install.ps1 (prefers Kitware/VS2022 CMake over VS2019)" -ForegroundColor Gray
    Write-Host "  4. Restart terminal after 1-install.ps1 so PATH/FFMPEG_DIR take effect" -ForegroundColor White
    Write-Host "  5. Ensure sufficient disk space (10GB+)" -ForegroundColor White
    Write-Host ""
    exit 1
}
