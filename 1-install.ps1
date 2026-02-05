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

# Setup FFmpeg development environment (using vcpkg for proper Rust FFI support)
Write-Host ""
Write-Host "[7/9] Setting up FFmpeg development environment..." -ForegroundColor Yellow

$vcpkgRoot = "$env:USERPROFILE\.vcpkg"
$vcpkgExe = "$vcpkgRoot\vcpkg.exe"

if (-not (Test-Path $vcpkgExe)) {
    Write-Host "  Installing vcpkg (package manager for C++ libraries)..." -ForegroundColor Gray
    
    if (Test-Path $vcpkgRoot) {
        Remove-Item $vcpkgRoot -Recurse -Force
    }
    
    git clone https://github.com/microsoft/vcpkg.git $vcpkgRoot 2>&1 | Out-Null
    
    if ($LASTEXITCODE -ne 0) {
        Write-Host "  ERROR: Failed to clone vcpkg" -ForegroundColor Red
        exit 1
    }
    
    Push-Location $vcpkgRoot
    .\bootstrap-vcpkg.bat 2>&1 | Out-Null
    Pop-Location
    
    if (Test-Path $vcpkgExe) {
        Write-Host "  OK vcpkg installed" -ForegroundColor Green
    }
    else {
        Write-Host "  ERROR: vcpkg installation failed" -ForegroundColor Red
        exit 1
    }
}
else {
    Write-Host "  OK vcpkg already installed" -ForegroundColor Green
}

$ffmpegInstalled = & $vcpkgExe list | Select-String "ffmpeg:x64-windows"

if (-not $ffmpegInstalled) {
    Write-Host "  Installing FFmpeg development libraries via vcpkg..." -ForegroundColor Yellow
    Write-Host "  This will take 15-30 minutes (downloads and compiles FFmpeg)..." -ForegroundColor Gray
    
    & $vcpkgExe install ffmpeg[core,avcodec,avdevice,avfilter,avformat,swresample,swscale]:x64-windows 2>&1 | Out-Null
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  OK FFmpeg development libraries installed" -ForegroundColor Green
    }
    else {
        Write-Host "  ERROR: FFmpeg installation via vcpkg failed" -ForegroundColor Red
        exit 1
    }
}
else {
    Write-Host "  OK FFmpeg development libraries already installed" -ForegroundColor Green
}

$vcpkgPath = "$vcpkgRoot\installed\x64-windows"
$currentVcpkgRoot = [System.Environment]::GetEnvironmentVariable("VCPKG_ROOT", "User")

if ($currentVcpkgRoot -ne $vcpkgRoot) {
    Write-Host "  Setting vcpkg environment variables..." -ForegroundColor Gray
    [System.Environment]::SetEnvironmentVariable("VCPKG_ROOT", $vcpkgRoot, "User")
    $needsRestart = $true
}

$env:VCPKG_ROOT = $vcpkgRoot

Write-Host "  Cleaning up old FFmpeg environment variables..." -ForegroundColor Gray
$oldFfmpegDir = [System.Environment]::GetEnvironmentVariable("FFMPEG_DIR", "User")
if ($oldFfmpegDir) {
    [System.Environment]::SetEnvironmentVariable("FFMPEG_DIR", $null, "User")
    [System.Environment]::SetEnvironmentVariable("FFMPEG_INCLUDE_DIR", $null, "User")
    [System.Environment]::SetEnvironmentVariable("FFMPEG_LIB_DIR", $null, "User")
    Write-Host "  OK Removed old FFmpeg environment variables (now using vcpkg)" -ForegroundColor Green
    $needsRestart = $true
}

$vcpkgBinDir = "$vcpkgRoot\installed\x64-windows\bin"
$projectFfmpegDir = "$PSScriptRoot\target\ffmpeg\bin"

if (Test-Path $vcpkgBinDir) {
    $vcpkgDlls = Get-ChildItem "$vcpkgBinDir\*.dll" -ErrorAction SilentlyContinue
    
    if ($vcpkgDlls.Count -gt 0) {
        $needsCopy = $false
        
        if (-not (Test-Path $projectFfmpegDir)) {
            $needsCopy = $true
        }
        else {
            $targetDlls = Get-ChildItem "$projectFfmpegDir\*.dll" -ErrorAction SilentlyContinue
            
            if ($vcpkgDlls.Count -ne $targetDlls.Count) {
                $needsCopy = $true
            }
        }
        
        if ($needsCopy) {
            Write-Host "  Copying FFmpeg DLLs for runtime..." -ForegroundColor Gray
            New-Item -ItemType Directory -Force -Path $projectFfmpegDir | Out-Null
            Copy-Item "$vcpkgBinDir\*.dll" -Destination $projectFfmpegDir -Force
            $dllCount = (Get-ChildItem $projectFfmpegDir -Filter "*.dll").Count
            Write-Host "  OK Copied $dllCount DLL files" -ForegroundColor Green
        }
    }
}

# Install LLVM (required for bindgen which generates Rust FFI bindings)
Write-Host ""
Refresh-Path
$clang = Get-Command clang -ErrorAction SilentlyContinue

if (-not $clang) {
    Write-Host "  Installing LLVM (required for Rust FFI bindings)..." -ForegroundColor Gray
    Write-Host "  Installing LLVM via winget..." -ForegroundColor Gray
    winget install --id=LLVM.LLVM --silent --accept-source-agreements --accept-package-agreements | Out-Null
    Refresh-Path
    
    # Try to find LLVM installation
    $llvmPaths = @(
        "C:\Program Files\LLVM\bin",
        "${env:ProgramFiles}\LLVM\bin",
        "${env:ProgramFiles(x86)}\LLVM\bin"
    )
    
    $llvmBinDir = $null
    foreach ($path in $llvmPaths) {
        if (Test-Path "$path\libclang.dll") {
            $llvmBinDir = $path
            break
        }
    }
    
    if ($llvmBinDir) {
        $currentLibclangPath = [System.Environment]::GetEnvironmentVariable("LIBCLANG_PATH", "User")
        if ($currentLibclangPath -ne $llvmBinDir) {
            [System.Environment]::SetEnvironmentVariable("LIBCLANG_PATH", $llvmBinDir, "User")
            $needsRestart = $true
        }
        $env:LIBCLANG_PATH = $llvmBinDir
        Write-Host "  OK LLVM installed" -ForegroundColor Green
    }
    else {
        Write-Host "  WARNING: LLVM installed but libclang.dll not found" -ForegroundColor Yellow
        Write-Host "  You may need to set LIBCLANG_PATH manually" -ForegroundColor Gray
    }
}
else {
    $llvmVersion = clang --version 2>&1 | Select-String "version" | Select-Object -First 1
    if ($llvmVersion) {
        Write-Host "  OK LLVM already installed ($llvmVersion)" -ForegroundColor Green
    }
    else {
        Write-Host "  OK LLVM already installed" -ForegroundColor Green
    }
}

Write-Host "  Configuring Cargo environment..." -ForegroundColor Gray
$cargoConfigDir = "$PSScriptRoot\.cargo"
$cargoConfigFile = "$cargoConfigDir\config.toml"

if (-not (Test-Path $cargoConfigDir)) {
    New-Item -ItemType Directory -Force -Path $cargoConfigDir | Out-Null
}

$cargoConfig = @"
[env]
"@

Set-Content -Path $cargoConfigFile -Value $cargoConfig -Encoding UTF8
Write-Host "  OK Cargo configuration updated" -ForegroundColor Green

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

# Install project dependencies
Write-Host ""
Write-Host "[9/9] Installing project dependencies..." -ForegroundColor Yellow

$needsInstall = $false
$lockFile = "$PSScriptRoot\pnpm-lock.yaml"
$nodeModules = "$PSScriptRoot\node_modules"
$desktopNodeModules = "$PSScriptRoot\apps\desktop\node_modules"

if (-not (Test-Path $nodeModules)) {
    Write-Host "  Root node_modules not found - fresh install needed" -ForegroundColor Gray
    $needsInstall = $true
}
elseif (-not (Test-Path $desktopNodeModules)) {
    Write-Host "  Workspace node_modules incomplete - install needed" -ForegroundColor Gray
    $needsInstall = $true
}
elseif (-not (Test-Path $lockFile)) {
    Write-Host "  Lock file not found - reinstall needed" -ForegroundColor Gray
    $needsInstall = $true
}
else {
    $lockModified = (Get-Item $lockFile).LastWriteTime
    $workspaceModified = (Get-Item "$PSScriptRoot\pnpm-workspace.yaml").LastWriteTime
    $packageModified = (Get-Item "$PSScriptRoot\package.json").LastWriteTime
    
    if ($workspaceModified -gt $lockModified -or $packageModified -gt $lockModified) {
        Write-Host "  Workspace or package.json changed - reinstall needed" -ForegroundColor Gray
        $needsInstall = $true
    }
    else {
        Write-Host "  OK Dependencies up to date" -ForegroundColor Green
        $needsInstall = $false
    }
}

if ($needsInstall) {
    Write-Host "  This may take a few minutes..." -ForegroundColor Gray
    Write-Host ""
    
    $lockFile = "$PSScriptRoot\pnpm-lock.yaml"
    $lockExists = Test-Path $lockFile
    
    if ($lockExists) {
        $lockCreationTime = (Get-Item $lockFile).CreationTime
        $lockModifiedTime = (Get-Item $lockFile).LastWriteTime
        
        if ($lockCreationTime -lt (Get-Date).AddMonths(-1)) {
            Write-Host "  Lockfile is older than 1 month, regenerating to ensure compatibility..." -ForegroundColor Gray
            Remove-Item $lockFile -Force
        }
    }
    
    pnpm install --no-optional=false
    
    if ($LASTEXITCODE -ne 0) {
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
}

Write-Host ""
Write-Host "[9/9] Updating Rust dependencies..." -ForegroundColor Yellow
Write-Host "  Ensuring all Rust crates are up-to-date..." -ForegroundColor Gray

$cargoCmd = Get-Command cargo -ErrorAction SilentlyContinue
if ($cargoCmd) {
    Push-Location "$PSScriptRoot\apps\desktop\src-tauri"
    try {
        cargo update 2>&1 | Out-Null
        if ($LASTEXITCODE -eq 0) {
            Write-Host "  Rust dependencies updated successfully" -ForegroundColor Green
        } else {
            Write-Host "  Warning: Cargo update returned non-zero exit code (may be harmless)" -ForegroundColor Yellow
        }
    } catch {
        Write-Host "  Warning: Could not update Rust dependencies" -ForegroundColor Yellow
    } finally {
        Pop-Location
    }
} else {
    Write-Host "  Skipping (cargo not in PATH - will be available after terminal restart)" -ForegroundColor Gray
}

Write-Host ""
Write-Host "================================================================" -ForegroundColor Green
Write-Host "  Installation Complete!" -ForegroundColor Green
Write-Host "================================================================" -ForegroundColor Green
Write-Host ""

if ($needsRestart) {
    Write-Host "IMPORTANT:" -ForegroundColor Yellow
    Write-Host "  Some tools were just installed." -ForegroundColor Yellow
    Write-Host "  Please CLOSE and REOPEN your terminal," -ForegroundColor Yellow
    Write-Host "  then run: .\2-dev.ps1" -ForegroundColor White
}
else {
    Write-Host "Next steps:" -ForegroundColor Cyan
    Write-Host "  .\2-dev.ps1   - Start development" -ForegroundColor White
    Write-Host "  .\3-build.ps1 - Build application" -ForegroundColor White
}
Write-Host ""
