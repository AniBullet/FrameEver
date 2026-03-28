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

$projectRoot = "$PSScriptRoot\.."
$nativeDepsDir = "$projectRoot\target\native-deps"

if (Test-Path "$nativeDepsDir\lib\avcodec.lib") {
    $env:FFMPEG_DIR = (Resolve-Path $nativeDepsDir).Path
    Write-Host "  FFmpeg environment configured (target/native-deps)" -ForegroundColor Green
} else {
    $userFfmpegDir = [System.Environment]::GetEnvironmentVariable("FFMPEG_DIR", "User")
    if ($userFfmpegDir -and (Test-Path $userFfmpegDir)) {
        $env:FFMPEG_DIR = $userFfmpegDir
        Write-Host "  FFmpeg environment configured (user env)" -ForegroundColor Green
    }
}

$cargoConfig = "$projectRoot\.cargo\config.toml"
if (Test-Path $cargoConfig) {
    $configContent = Get-Content $cargoConfig -Raw
    if ($configContent -match 'LIBCLANG_PATH\s*=\s*"([^"]+)"') {
        $libclangFromConfig = $matches[1]
        if (Test-Path $libclangFromConfig) {
            $env:LIBCLANG_PATH = $libclangFromConfig
        }
    }
}
$libclangPath = [System.Environment]::GetEnvironmentVariable("LIBCLANG_PATH", "User")
if (-not $env:LIBCLANG_PATH -and $libclangPath -and (Test-Path $libclangPath)) {
    $env:LIBCLANG_PATH = $libclangPath
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

Push-Location "$PSScriptRoot\..\apps\desktop"
pnpm dev
Pop-Location
