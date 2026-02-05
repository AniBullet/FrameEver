# Cap aniBullet - Build Script

$OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
chcp 65001 > $null

Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  Cap aniBullet - Build Application" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

Write-Host "Verifying build prerequisites..." -ForegroundColor Yellow

$allOk = $true

if (-not (Test-Path "node_modules")) {
    Write-Host "  ERROR: node_modules not found" -ForegroundColor Red
    Write-Host "  Please run: .\1-install.ps1" -ForegroundColor Yellow
    $allOk = $false
}

$ffmpegDevDir = "$env:USERPROFILE\.ffmpeg-dev"
if (-not (Test-Path "$ffmpegDevDir\include\libavutil\avutil.h")) {
    Write-Host "  ERROR: FFmpeg development files not found" -ForegroundColor Red
    Write-Host "  Expected: $ffmpegDevDir\include\libavutil\avutil.h" -ForegroundColor Yellow
    Write-Host "  Please run: .\1-install.ps1" -ForegroundColor Yellow
    $allOk = $false
}

$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $cargo) {
    Write-Host "  ERROR: Cargo (Rust) not found in PATH" -ForegroundColor Red
    Write-Host ""
    Write-Host "Solution:" -ForegroundColor Yellow
    Write-Host "  1. Close this terminal completely" -ForegroundColor White
    Write-Host "  2. Open a NEW PowerShell window" -ForegroundColor White
    Write-Host "  3. Run: .\3-build.ps1" -ForegroundColor White
    Write-Host ""
    Write-Host "Rust is installed but PATH needs terminal restart." -ForegroundColor Gray
    $allOk = $false
}

if (-not $allOk) {
    Write-Host ""
    exit 1
}

Write-Host "  OK Prerequisites verified" -ForegroundColor Green
Write-Host ""

$arch = $env:PROCESSOR_ARCHITECTURE
if ($arch -eq "AMD64") {
    $target = "x86_64-pc-windows-msvc"
    $archName = "x64"
} elseif ($arch -eq "ARM64") {
    $target = "aarch64-pc-windows-msvc"
    $archName = "ARM64"
} else {
    $target = "x86_64-pc-windows-msvc"
    $archName = "x64"
}

Write-Host "Detected architecture: $archName ($target)" -ForegroundColor Gray
Write-Host ""

Write-Host "Setting up build environment..." -ForegroundColor Gray

$env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path", "User")

$vcpkgRoot = [System.Environment]::GetEnvironmentVariable("VCPKG_ROOT", "User")
if (-not $vcpkgRoot) {
    $vcpkgRoot = "$env:USERPROFILE\.vcpkg"
}

if (Test-Path $vcpkgRoot) {
    $env:VCPKG_ROOT = $vcpkgRoot
    Write-Host "  OK vcpkg environment configured" -ForegroundColor Green
} else {
    Write-Host "  ERROR: vcpkg not found" -ForegroundColor Red
    Write-Host "  Please run: .\1-install.ps1" -ForegroundColor Yellow
    exit 1
}

Write-Host ""

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
    Write-Host "  Architecture: $archName" -ForegroundColor White
    Write-Host "  Target: $target" -ForegroundColor White
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
