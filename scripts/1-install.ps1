# Cap aniBullet - Auto Install All Dependencies

$OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
chcp 65001 > $null

Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  Cap aniBullet - Auto Install" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

$needsRestart = $false

# Function to refresh PATH
function Refresh-Path {
    $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
}

# Check Node.js
Write-Host "[1/9] Checking Node.js..." -ForegroundColor Yellow
$node = Get-Command node -ErrorAction SilentlyContinue
if ($node) {
    $nodeVersion = node --version
    Write-Host "  OK Node.js $nodeVersion" -ForegroundColor Green
}
else {
    Write-Host "  Installing Node.js..." -ForegroundColor Yellow
    winget install OpenJS.NodeJS.LTS --silent --accept-source-agreements --accept-package-agreements
    Refresh-Path
    $node = Get-Command node -ErrorAction SilentlyContinue
    if ($node) {
        Write-Host "  OK Node.js installed" -ForegroundColor Green
        $needsRestart = $true
    }
    else {
        Write-Host "  ERROR: Failed to install Node.js" -ForegroundColor Red
        Write-Host "  Please install manually: https://nodejs.org/" -ForegroundColor Yellow
        exit 1
    }
}

# Check pnpm
Write-Host ""
Write-Host "[2/9] Checking pnpm..." -ForegroundColor Yellow
Refresh-Path
$pnpm = Get-Command pnpm -ErrorAction SilentlyContinue
if ($pnpm) {
    $pnpmVersion = pnpm --version
    Write-Host "  OK pnpm $pnpmVersion" -ForegroundColor Green
}
else {
    Write-Host "  Installing pnpm..." -ForegroundColor Yellow
    npm install -g pnpm
    Refresh-Path
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  OK pnpm installed" -ForegroundColor Green
    }
    else {
        Write-Host "  ERROR: Failed to install pnpm" -ForegroundColor Red
        exit 1
    }
}

# Check CMake (Kitware preferred: supports all VS generators; required for whisper-rs-sys)
Write-Host ""
Write-Host "[3/9] Checking CMake..." -ForegroundColor Yellow
Refresh-Path
$kitwareCmakeBin = "${env:ProgramFiles}\CMake\bin"
if (-not (Test-Path "$kitwareCmakeBin\cmake.exe")) { $kitwareCmakeBin = "${env:ProgramFiles(x86)}\CMake\bin" }
$cmake = Get-Command cmake -ErrorAction SilentlyContinue
if ($cmake) {
    $ver = cmake --version 2>$null | Select-String "version" | Select-Object -First 1
    Write-Host "  OK CMake: $ver" -ForegroundColor Green
    if (Test-Path "$kitwareCmakeBin\cmake.exe") {
        $userPath = [System.Environment]::GetEnvironmentVariable("Path", "User")
        if ($userPath -notlike "$kitwareCmakeBin*") {
            [System.Environment]::SetEnvironmentVariable("Path", "$kitwareCmakeBin;$userPath", "User")
            $env:Path = "$kitwareCmakeBin;$env:Path"
            Write-Host "  Kitware CMake preferred in PATH" -ForegroundColor Gray
            $needsRestart = $true
        }
    }
}
else {
    Write-Host "  Installing CMake (Kitware)..." -ForegroundColor Yellow
    winget install --id=Kitware.CMake --silent --accept-source-agreements --accept-package-agreements
    Refresh-Path
    $kitwareCmakeBin = "${env:ProgramFiles}\CMake\bin"
    if (-not (Test-Path "$kitwareCmakeBin\cmake.exe")) { $kitwareCmakeBin = "${env:ProgramFiles(x86)}\CMake\bin" }
    if (Test-Path "$kitwareCmakeBin\cmake.exe") {
        [System.Environment]::SetEnvironmentVariable("Path", "$kitwareCmakeBin;" + [System.Environment]::GetEnvironmentVariable("Path", "User"), "User")
        $env:Path = "$kitwareCmakeBin;$env:Path"
        Write-Host "  OK CMake installed" -ForegroundColor Green
    }
    else { Write-Host "  OK CMake installed (restart terminal to use)" -ForegroundColor Yellow }
    $needsRestart = $true
}

# Check Rust
Write-Host ""
Write-Host "[4/9] Checking Rust..." -ForegroundColor Yellow
Refresh-Path
$rust = Get-Command rustc -ErrorAction SilentlyContinue
if ($rust) {
    # Try to get version
    $rustVersion = rustc --version 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  OK $rustVersion" -ForegroundColor Green
    }
    else {
        # rustc exists but no default toolchain configured
        Write-Host "  Configuring Rust default toolchain..." -ForegroundColor Yellow
        $rustup = Get-Command rustup -ErrorAction SilentlyContinue
        if ($rustup) {
            rustup default stable 2>&1 | Out-Null
            if ($LASTEXITCODE -eq 0) {
                $rustVersion = rustc --version
                Write-Host "  OK $rustVersion" -ForegroundColor Green
            }
            else {
                Write-Host "  ERROR: Failed to configure Rust" -ForegroundColor Red
                exit 1
            }
        }
        else {
            Write-Host "  ERROR: rustup not found" -ForegroundColor Red
            exit 1
        }
    }
}
else {
    Write-Host "  Installing Rust..." -ForegroundColor Yellow
    Write-Host "  Downloading rustup-init.exe..." -ForegroundColor Gray
    
    $rustupUrl = "https://win.rustup.rs/x86_64"
    $rustupPath = "$env:TEMP\rustup-init.exe"
    
    try {
        # Download
        Invoke-WebRequest -Uri $rustupUrl -OutFile $rustupPath -UseBasicParsing
        
        # Install silently
        Write-Host "  Installing (this takes 2-5 minutes)..." -ForegroundColor Gray
        $process = Start-Process -FilePath $rustupPath -ArgumentList "-y", "--default-toolchain", "stable" -Wait -PassThru -NoNewWindow
        
        # Clean up
        Remove-Item $rustupPath -ErrorAction SilentlyContinue
        
        if ($process.ExitCode -eq 0) {
            # Refresh PATH
            Refresh-Path
            
            # Verify installation
            $rust = Get-Command rustc -ErrorAction SilentlyContinue
            if ($rust) {
                $rustVersion = rustc --version
                Write-Host "  OK Rust installed: $rustVersion" -ForegroundColor Green
            }
            else {
                Write-Host "  OK Rust installed (restart terminal to use)" -ForegroundColor Yellow
                $needsRestart = $true
            }
        }
        else {
            Write-Host "  ERROR: Rust installation failed" -ForegroundColor Red
            exit 1
        }
    }
    catch {
        Write-Host "  ERROR: Failed to install Rust" -ForegroundColor Red
        Write-Host "  Error: $_" -ForegroundColor Red
        Write-Host "  Please install manually: https://rustup.rs/" -ForegroundColor Yellow
        exit 1
    }
}

# Configure Cargo git-fetch-with-cli (required for Git dependencies)
$cargoConfigDir = "$env:USERPROFILE\.cargo"
$cargoConfigFile = "$cargoConfigDir\config.toml"

if (-not (Test-Path $cargoConfigFile)) {
    if (-not (Test-Path $cargoConfigDir)) {
        New-Item -ItemType Directory -Force -Path $cargoConfigDir | Out-Null
    }
    
    $cargoConfig = @"
[net]
git-fetch-with-cli = true
"@
    
    Set-Content -Path $cargoConfigFile -Value $cargoConfig -Encoding UTF8
    Write-Host "  OK Cargo configured" -ForegroundColor Green
}

# Auto-detect and configure Git proxy for GitHub access
Write-Host "  Checking Git proxy configuration..." -ForegroundColor Gray

$gitHttpProxy = git config --global http.proxy 2>$null
$gitHttpsProxy = git config --global https.proxy 2>$null

if (-not $gitHttpProxy -and -not $gitHttpsProxy) {
    # Try to detect common VPN proxy ports
    $commonPorts = @(7890, 10809, 1080, 7891, 10808)
    $detectedPort = $null
    
    foreach ($port in $commonPorts) {
        try {
            $tcpClient = New-Object System.Net.Sockets.TcpClient
            $connect = $tcpClient.BeginConnect("127.0.0.1", $port, $null, $null)
            $wait = $connect.AsyncWaitHandle.WaitOne(100, $false)
            
            if ($wait) {
                $tcpClient.EndConnect($connect)
                $detectedPort = $port
                $tcpClient.Close()
                break
            }
            $tcpClient.Close()
        }
        catch {
            # Port not available, continue
        }
    }
    
    if ($detectedPort) {
        $proxyUrl = "http://127.0.0.1:$detectedPort"
        git config --global http.proxy $proxyUrl 2>$null
        git config --global https.proxy $proxyUrl 2>$null
        Write-Host "  OK Auto-configured Git proxy: $proxyUrl" -ForegroundColor Green
    }
    else {
        Write-Host "  OK No proxy detected (direct connection)" -ForegroundColor Green
    }
}
else {
    Write-Host "  OK Git proxy already configured" -ForegroundColor Green
}

# Check MSVC (required for Rust FFI compilation with bindgen)
Write-Host ""
Write-Host "[5/9] Checking MSVC C++ Build Tools..." -ForegroundColor Yellow
Refresh-Path

$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$vcvarsall = $null

if (Test-Path $vswhere) {
    $vsPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null
    if ($vsPath) {
        $vcvarsall = Join-Path $vsPath "VC\Auxiliary\Build\vcvarsall.bat"
    }
}

if (-not $vcvarsall -or -not (Test-Path $vcvarsall)) {
    $possiblePaths = @(
        "${env:ProgramFiles}\Microsoft Visual Studio\2022\*\VC\Auxiliary\Build\vcvarsall.bat",
        "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\*\VC\Auxiliary\Build\vcvarsall.bat",
        "${env:ProgramFiles}\Microsoft Visual Studio\2019\*\VC\Auxiliary\Build\vcvarsall.bat",
        "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2019\*\VC\Auxiliary\Build\vcvarsall.bat"
    )
    
    foreach ($pattern in $possiblePaths) {
        $found = Get-Item $pattern -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($found) {
            $vcvarsall = $found.FullName
            break
        }
    }
}

if ($vcvarsall -and (Test-Path $vcvarsall)) {
    Write-Host "  OK MSVC Build Tools found" -ForegroundColor Green
} else {
    Write-Host "  WARNING: MSVC Build Tools not found" -ForegroundColor Yellow
    Write-Host "  Rust FFI compilation (bindgen) may fail without MSVC" -ForegroundColor Gray
    Write-Host ""
    Write-Host "  To install:" -ForegroundColor Cyan
    Write-Host "    Option 1: winget install Microsoft.VisualStudio.2022.BuildTools" -ForegroundColor White
    Write-Host "    Option 2: Download from https://aka.ms/vs/17/release/vs_BuildTools.exe" -ForegroundColor White
    Write-Host "             Select: Desktop development with C++" -ForegroundColor White
}

# Check FFmpeg runtime
Write-Host ""
Write-Host "[6/9] Checking FFmpeg runtime..." -ForegroundColor Yellow
Refresh-Path
$ffmpeg = Get-Command ffmpeg -ErrorAction SilentlyContinue
if ($ffmpeg) {
    Write-Host "  OK FFmpeg installed" -ForegroundColor Green
}
else {
    Write-Host "  Installing FFmpeg..." -ForegroundColor Yellow
    winget install Gyan.FFmpeg --silent --accept-source-agreements --accept-package-agreements
    Refresh-Path
    $ffmpeg = Get-Command ffmpeg -ErrorAction SilentlyContinue
    if ($ffmpeg) {
        Write-Host "  OK FFmpeg installed" -ForegroundColor Green
    }
    else {
        Write-Host "  OK FFmpeg installed (restart terminal to use)" -ForegroundColor Yellow
        $needsRestart = $true
    }
}

Write-Host ""
Write-Host "[7/9] Setting up FFmpeg development environment..." -ForegroundColor Yellow

$projectRoot = "$PSScriptRoot\.."
$targetDir = "$projectRoot\target"
$ffmpegVersion = "7.1"
$ffmpegZipName = "ffmpeg-${ffmpegVersion}-full_build-shared"
$ffmpegZipUrl = "https://github.com/GyanD/codexffmpeg/releases/download/${ffmpegVersion}/${ffmpegZipName}.zip"

New-Item -ItemType Directory -Force -Path $targetDir | Out-Null

$ffmpegDir = "$targetDir\ffmpeg"
$nativeDepsDir = "$targetDir\native-deps"
$ffmpegZipPath = "$targetDir\ffmpeg-${ffmpegVersion}.zip"
$downloadNeeded = $false

if (-not (Test-Path "$ffmpegDir\lib\avcodec.lib")) {
    $downloadNeeded = $true
}

if ($downloadNeeded) {
    if (-not (Test-Path $ffmpegZipPath)) {
        Write-Host "  Downloading FFmpeg ${ffmpegVersion} (GyanD stable build)..." -ForegroundColor Yellow
        try {
            Invoke-WebRequest -Uri $ffmpegZipUrl -OutFile $ffmpegZipPath -UseBasicParsing
            Write-Host "  Downloaded ffmpeg-${ffmpegVersion}.zip" -ForegroundColor Gray
        }
        catch {
            Write-Host "  ERROR: Failed to download FFmpeg" -ForegroundColor Red
            Write-Host "  Error: $_" -ForegroundColor Red
            Write-Host "  Try manual download: $ffmpegZipUrl" -ForegroundColor Yellow
            exit 1
        }
    }
    else {
        Write-Host "  Using cached ffmpeg-${ffmpegVersion}.zip" -ForegroundColor Gray
    }

    Write-Host "  Extracting..." -ForegroundColor Gray
    Expand-Archive -Path $ffmpegZipPath -DestinationPath $targetDir -Force
    if (Test-Path $ffmpegDir) {
        Remove-Item $ffmpegDir -Recurse -Force -ErrorAction SilentlyContinue
    }
    Rename-Item -Path "$targetDir\$ffmpegZipName" -NewName "ffmpeg" -Force
    Write-Host "  Extracted ffmpeg to target/ffmpeg" -ForegroundColor Gray
}
else {
    Write-Host "  Using cached target/ffmpeg" -ForegroundColor Gray
}

if (-not (Test-Path "$ffmpegDir\lib\avcodec.lib")) {
    Write-Host "  ERROR: FFmpeg extraction failed (avcodec.lib not found)" -ForegroundColor Red
    exit 1
}

New-Item -ItemType Directory -Force -Path "$targetDir\debug" | Out-Null
$dlls = Get-ChildItem "$ffmpegDir\bin\*.dll" -ErrorAction SilentlyContinue
if ($dlls.Count -gt 0) {
    Copy-Item "$ffmpegDir\bin\*.dll" -Destination "$targetDir\debug" -Force
    Write-Host "  Copied $($dlls.Count) FFmpeg DLLs to target/debug" -ForegroundColor Gray
}

New-Item -ItemType Directory -Force -Path $nativeDepsDir | Out-Null
if (Test-Path "$nativeDepsDir\lib") { Remove-Item "$nativeDepsDir\lib" -Recurse -Force }
if (Test-Path "$nativeDepsDir\include") { Remove-Item "$nativeDepsDir\include" -Recurse -Force }
Copy-Item "$ffmpegDir\lib" -Destination "$nativeDepsDir\lib" -Recurse -Force
Copy-Item "$ffmpegDir\include" -Destination "$nativeDepsDir\include" -Recurse -Force
Write-Host "  Copied lib + include to target/native-deps" -ForegroundColor Gray

foreach ($envName in @("FFMPEG_DIR", "PKG_CONFIG_PATH", "VCPKG_ROOT")) {
    $oldVal = [System.Environment]::GetEnvironmentVariable($envName, "User")
    if ($oldVal) {
        [System.Environment]::SetEnvironmentVariable($envName, $null, "User")
        Write-Host "  Cleaned old User env: $envName" -ForegroundColor Gray
        $needsRestart = $true
    }
}

Write-Host "  OK FFmpeg ${ffmpegVersion} dev environment ready" -ForegroundColor Green

# Setup LLVM 18 libclang (required for bindgen which generates Rust FFI bindings)
# NOTE: bindgen 0.70.x is incompatible with LLVM 22+. We pin libclang to LLVM 18.
Write-Host ""
Refresh-Path

$llvm18Dir = "$env:USERPROFILE\.llvm-18"
$llvm18Bin = "$llvm18Dir\bin"

if (Test-Path "$llvm18Bin\libclang.dll") {
    Write-Host "  OK LLVM 18 libclang already installed" -ForegroundColor Green
}
else {
    Write-Host "  Downloading LLVM 18 libclang (required for Rust FFI bindings)..." -ForegroundColor Yellow
    $llvmVersion = "18.1.8"
    $llvmTarName = "clang+llvm-${llvmVersion}-x86_64-pc-windows-msvc.tar.xz"
    $llvmTarUrl = "https://github.com/llvm/llvm-project/releases/download/llvmorg-${llvmVersion}/${llvmTarName}"
    $llvmTarPath = "$targetDir\${llvmTarName}"
    $llvmExtractDir = "$targetDir\llvm-18-extract"
    $llvmSourceBin = "$llvmExtractDir\clang+llvm-${llvmVersion}-x86_64-pc-windows-msvc\bin"
    $llvmTarMinBytes = 700000000

    try {
        $tar = Get-Command tar -ErrorAction SilentlyContinue
        if (-not $tar) {
            throw "Windows tar not found. Requires Windows 10 or later."
        }

        if (-not (Test-Path "$llvmSourceBin\libclang.dll")) {
            if (Test-Path $llvmTarPath) {
                $tarSize = (Get-Item $llvmTarPath).Length
                if ($tarSize -lt $llvmTarMinBytes) {
                    Write-Host "  Cached ${llvmTarName} is incomplete, re-downloading..." -ForegroundColor Gray
                    Remove-Item $llvmTarPath -Force
                }
            }

            if (-not (Test-Path $llvmTarPath)) {
                Write-Host "  Downloading ${llvmTarName} (~800 MB)..." -ForegroundColor Gray
                Invoke-WebRequest -Uri $llvmTarUrl -OutFile $llvmTarPath -UseBasicParsing
                $tarSize = (Get-Item $llvmTarPath).Length
                if ($tarSize -lt $llvmTarMinBytes) {
                    Remove-Item $llvmTarPath -Force
                    throw "Download incomplete ($tarSize bytes). Check network and retry."
                }
                Write-Host "  Downloaded ${llvmTarName}" -ForegroundColor Gray
            }
            else {
                Write-Host "  Using cached ${llvmTarName}" -ForegroundColor Gray
            }

            Write-Host "  Extracting libclang (this may take a few minutes)..." -ForegroundColor Gray
            if (Test-Path $llvmExtractDir) {
                Remove-Item $llvmExtractDir -Recurse -Force -ErrorAction SilentlyContinue
            }
            New-Item -ItemType Directory -Force -Path $llvmExtractDir | Out-Null
            & tar -xf $llvmTarPath -C $llvmExtractDir
            if ($LASTEXITCODE -ne 0) {
                Remove-Item $llvmExtractDir -Recurse -Force -ErrorAction SilentlyContinue
                throw "tar extraction failed (exit $LASTEXITCODE)."
            }
        }
        else {
            Write-Host "  Using cached target/llvm-18-extract" -ForegroundColor Gray
        }

        if (Test-Path "$llvmSourceBin\libclang.dll") {
            New-Item -ItemType Directory -Force -Path $llvm18Bin | Out-Null
            Copy-Item "$llvmSourceBin\*.dll" -Destination $llvm18Bin -Force
            Write-Host "  OK LLVM 18 libclang installed" -ForegroundColor Green
        }
        else {
            throw "libclang.dll not found after extraction."
        }
    }
    catch {
        Write-Host "  ERROR: LLVM 18 libclang setup failed" -ForegroundColor Red
        Write-Host "  Error: $_" -ForegroundColor Red
        Write-Host "  Manual download: $llvmTarUrl" -ForegroundColor Gray
        Write-Host "  Extract bin/*.dll to $llvm18Bin and set LIBCLANG_PATH" -ForegroundColor Gray
        exit 1
    }
}

if (Test-Path "$llvm18Bin\libclang.dll") {
    $currentLibclangPath = [System.Environment]::GetEnvironmentVariable("LIBCLANG_PATH", "User")
    if ($currentLibclangPath -ne $llvm18Bin) {
        [System.Environment]::SetEnvironmentVariable("LIBCLANG_PATH", $llvm18Bin, "User")
        $needsRestart = $true
    }
    $env:LIBCLANG_PATH = $llvm18Bin
}

Write-Host "  Configuring .cargo/config.toml..." -ForegroundColor Gray
$cargoConfigDir = "$projectRoot\.cargo"
$cargoConfigFile = "$cargoConfigDir\config.toml"

if (-not (Test-Path $cargoConfigDir)) {
    New-Item -ItemType Directory -Force -Path $cargoConfigDir | Out-Null
}

$cargoConfig = @"
[env]
FFMPEG_DIR = { relative = true, force = true, value = "target/native-deps" }
"@

if (Test-Path "$llvm18Bin\libclang.dll") {
    $libclangEscaped = $llvm18Bin.Replace("\", "/")
    $cargoConfig += "`nLIBCLANG_PATH = `"$libclangEscaped`""
}

Set-Content -Path $cargoConfigFile -Value $cargoConfig -Encoding UTF8
Write-Host "  OK .cargo/config.toml written at project root" -ForegroundColor Green

# Verify Cargo (for better error message)
Write-Host ""
Write-Host "[8/9] Verifying Rust Cargo..." -ForegroundColor Yellow
Refresh-Path
$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if ($cargo) {
    Write-Host "  OK Cargo ready" -ForegroundColor Green
}
else {
    Write-Host "  WARNING: Cargo not in PATH yet" -ForegroundColor Yellow
    Write-Host "  You'll need to restart terminal after installation" -ForegroundColor Gray
    $needsRestart = $true
}

Write-Host ""
Write-Host "[9/9] Installing project dependencies..." -ForegroundColor Yellow

$projectRoot = "$PSScriptRoot\.."

Write-Host "  Syncing pnpm dependencies..." -ForegroundColor Gray
Push-Location $projectRoot
pnpm install --no-optional=false --no-frozen-lockfile
$pnpmExit = $LASTEXITCODE
Pop-Location

if ($pnpmExit -ne 0) {
    Write-Host ""
    Write-Host "================================================================" -ForegroundColor Red
    Write-Host "  Installation Failed" -ForegroundColor Red
    Write-Host "================================================================" -ForegroundColor Red
    Write-Host ""
    Write-Host "Common issues:" -ForegroundColor Yellow
    Write-Host "  1. Network - Check internet connection" -ForegroundColor White
    Write-Host "  2. Permissions - Try running as Administrator" -ForegroundColor White
    Write-Host "  3. Restart terminal if tools were just installed" -ForegroundColor White
    Write-Host ""
    exit 1
}

Write-Host "  OK pnpm dependencies installed" -ForegroundColor Green

Write-Host ""
Write-Host "[9/9] Fetching Rust dependencies (Cargo.lock)..." -ForegroundColor Yellow

$cargoCmd = Get-Command cargo -ErrorAction SilentlyContinue
if ($cargoCmd) {
    Push-Location $projectRoot
    try {
        cargo fetch 2>&1 | Out-Null
        if ($LASTEXITCODE -eq 0) {
            Write-Host "  OK Rust dependencies fetched from Cargo.lock" -ForegroundColor Green
        } else {
            Write-Host "  ERROR: cargo fetch failed" -ForegroundColor Red
            exit 1
        }
    } finally {
        Pop-Location
    }
} else {
    Write-Host "  Skipping (cargo not in PATH - will be available after terminal restart)" -ForegroundColor Gray
    $needsRestart = $true
}

Write-Host ""
Write-Host "  Checking Tauri Rust/NPM version alignment..." -ForegroundColor Gray
& "$PSScriptRoot\verify-tauri-versions.ps1" -ProjectRoot $projectRoot
if ($LASTEXITCODE -ne 0) {
    exit 1
}

Write-Host ""
Write-Host "Verifying installation..." -ForegroundColor Yellow

$installIssues = @()
if (-not (Test-Path "$nativeDepsDir\include\libavutil\avutil.h")) {
    $installIssues += "FFmpeg dev files (target/native-deps)"
}
if (-not (Test-Path "$llvm18Bin\libclang.dll")) {
    $installIssues += "LLVM 18 libclang ($llvm18Bin)"
}

if ($installIssues.Count -gt 0) {
    Write-Host ""
    Write-Host "================================================================" -ForegroundColor Red
    Write-Host "  Installation Incomplete" -ForegroundColor Red
    Write-Host "================================================================" -ForegroundColor Red
    Write-Host ""
    foreach ($issue in $installIssues) {
        Write-Host "  Missing: $issue" -ForegroundColor Red
    }
    Write-Host ""
    Write-Host "Re-run: .\scripts\1-install.ps1" -ForegroundColor Yellow
    Write-Host ""
    exit 1
}

Write-Host "  OK All native dependencies verified" -ForegroundColor Green
Write-Host ""
Write-Host "================================================================" -ForegroundColor Green
Write-Host "  Installation Complete!" -ForegroundColor Green
Write-Host "================================================================" -ForegroundColor Green
Write-Host ""

if ($needsRestart) {
    Write-Host "IMPORTANT:" -ForegroundColor Yellow
    Write-Host "  Some tools were just installed." -ForegroundColor Yellow
    Write-Host "  Please CLOSE and REOPEN your terminal," -ForegroundColor Yellow
    Write-Host "  then run: .\scripts\2-dev.ps1" -ForegroundColor White
}
else {
    Write-Host "Next steps:" -ForegroundColor Cyan
    Write-Host "  .\scripts\2-dev.ps1   - Start development" -ForegroundColor White
    Write-Host "  .\scripts\3-build.ps1 - Build application" -ForegroundColor White
}
Write-Host ""
