param(
    [switch]$ClippyOnly,
    [switch]$SkipDesktop
)

$ErrorActionPreference = "Stop"
$OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

$root = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
while (-not (Test-Path "$root\package.json")) { $root = Split-Path -Parent $root }
Set-Location $root

Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  Verify CI steps locally (Windows)" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

$vcpkgRoot = [System.Environment]::GetEnvironmentVariable("VCPKG_ROOT", "User")
if (-not $vcpkgRoot) { $vcpkgRoot = "$env:USERPROFILE\.vcpkg" }
if (-not (Test-Path $vcpkgRoot)) {
    Write-Host "ERROR: VCPKG_ROOT not set and $vcpkgRoot not found. Run .\scripts\1-install.ps1 first." -ForegroundColor Red
    exit 1
}
$env:VCPKG_ROOT = $vcpkgRoot
Write-Host "VCPKG_ROOT = $vcpkgRoot" -ForegroundColor Gray

$hasFfmpeg = & "$vcpkgRoot\vcpkg.exe" list 2>$null | Select-String "ffmpeg:x64-windows"
if (-not $hasFfmpeg) {
    Write-Host "ERROR: ffmpeg:x64-windows not installed. Run .\scripts\1-install.ps1 first." -ForegroundColor Red
    exit 1
}
Write-Host "OK ffmpeg:x64-windows installed" -ForegroundColor Green

$binDir = "$vcpkgRoot\installed\x64-windows\bin"
$targetDir = "target\ffmpeg\bin"
if (-not (Test-Path $binDir)) {
    Write-Host "ERROR: $binDir not found." -ForegroundColor Red
    exit 1
}
New-Item -ItemType Directory -Force -Path $targetDir | Out-Null
Copy-Item "$binDir\*.dll" -Destination $targetDir -Force
Write-Host "OK Prepared FFmpeg DLLs in $targetDir" -ForegroundColor Green

if (-not (Test-Path ".env")) {
    "VITE_ENVIRONMENT=production" | Out-File -FilePath .env -Encoding utf8
    Write-Host "OK Created .env" -ForegroundColor Green
}
if (Test-Path "apps\desktop") {
    Copy-Item .env apps\desktop\.env -Force -ErrorAction SilentlyContinue
}

if (-not $ClippyOnly) {
    Write-Host ""
    Write-Host "Running node scripts/setup.js ..." -ForegroundColor Yellow
    node scripts/setup.js
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    Write-Host "OK setup.js" -ForegroundColor Green
}

Write-Host ""
Write-Host "Running cargo clippy (same as CI) ..." -ForegroundColor Yellow
cargo clippy --workspace --all-features --locked -- -D warnings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host "OK clippy" -ForegroundColor Green

if (-not $SkipDesktop -and -not $ClippyOnly) {
    Write-Host ""
    Write-Host "Running desktop build (same as CI build-desktop) ..." -ForegroundColor Yellow
    $env:RUST_TARGET_TRIPLE = "x86_64-pc-windows-msvc"
    Push-Location apps\desktop
    try {
        pnpm -w cap-setup
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
        pnpm tauri build --debug --target x86_64-pc-windows-msvc --no-bundle
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    } finally { Pop-Location }
    Write-Host "OK desktop build" -ForegroundColor Green
}

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  All CI steps passed locally." -ForegroundColor Green
Write-Host "================================================================" -ForegroundColor Cyan
