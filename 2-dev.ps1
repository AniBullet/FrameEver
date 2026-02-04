# Cap aniBullet - Development Mode

$OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
chcp 65001 > $null

Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  Cap aniBullet - Development Mode" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

# Refresh PATH to pick up newly installed tools
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")

# Setup FFmpeg environment variables for compilation
$ffmpegDevDir = "$env:USERPROFILE\.ffmpeg-dev"
if (Test-Path "$ffmpegDevDir\include") {
    $env:FFMPEG_DIR = $ffmpegDevDir
    $env:FFMPEG_INCLUDE_DIR = "$ffmpegDevDir\include"
    $env:FFMPEG_LIB_DIR = "$ffmpegDevDir\lib"
    Write-Host "  FFmpeg environment configured" -ForegroundColor Green
    
    # Ensure DLLs are copied to project directory
    $projectFfmpegDir = "$PSScriptRoot\target\ffmpeg\bin"
    if (-not (Test-Path $projectFfmpegDir)) {
        Write-Host "  Copying FFmpeg DLLs to project..." -ForegroundColor Gray
        New-Item -ItemType Directory -Force -Path $projectFfmpegDir | Out-Null
        Copy-Item "$ffmpegDevDir\bin\*.dll" -Destination $projectFfmpegDir -Force
    }
}

# Setup LLVM/libclang for bindgen
$llvmPaths = @(
    "C:\Program Files\LLVM\bin",
    "${env:ProgramFiles}\LLVM\bin",
    "${env:ProgramFiles(x86)}\LLVM\bin"
)

foreach ($path in $llvmPaths) {
    if (Test-Path "$path\libclang.dll") {
        $env:LIBCLANG_PATH = $path
        Write-Host "  LLVM/libclang environment configured" -ForegroundColor Green
        break
    }
}

# Verify required tools
Write-Host "Checking environment..." -ForegroundColor Yellow

$allOk = $true

$node = Get-Command node -ErrorAction SilentlyContinue
if (-not $node) {
    Write-Host "  ERROR: Node.js not found" -ForegroundColor Red
    $allOk = $false
}

$pnpm = Get-Command pnpm -ErrorAction SilentlyContinue
if (-not $pnpm) {
    Write-Host "  ERROR: pnpm not found" -ForegroundColor Red
    $allOk = $false
}

$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $cargo) {
    Write-Host "  ERROR: Cargo (Rust) not found in PATH" -ForegroundColor Red
    Write-Host "" -ForegroundColor Red
    Write-Host "Solution:" -ForegroundColor Yellow
    Write-Host "  1. Close this terminal completely" -ForegroundColor White
    Write-Host "  2. Open a NEW PowerShell window" -ForegroundColor White
    Write-Host "  3. Run: .\2-dev.ps1" -ForegroundColor White
    Write-Host ""
    Write-Host "Rust is installed but PATH needs terminal restart." -ForegroundColor Gray
    exit 1
}

if (-not $allOk) {
    Write-Host ""
    Write-Host "Please run: .\1-install.ps1" -ForegroundColor Yellow
    exit 1
}

Write-Host "  OK Environment ready" -ForegroundColor Green
Write-Host ""

# Start development
Write-Host "Starting development server..." -ForegroundColor Cyan
Write-Host "Press Ctrl+C to stop" -ForegroundColor Gray
Write-Host ""

Push-Location "apps\desktop"
pnpm dev
Pop-Location
