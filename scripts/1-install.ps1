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
    $llvm18Url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-18.1.8/LLVM-18.1.8-win64.exe"
    $llvm18Installer = "$env:TEMP\LLVM-18.1.8-win64.exe"

    try {
        Invoke-WebRequest -Uri $llvm18Url -OutFile $llvm18Installer -UseBasicParsing
        Write-Host "  Extracting libclang.dll (this takes a moment)..." -ForegroundColor Gray

        $sevenZipPaths = @(
            "${env:ProgramFiles}\7-Zip\7z.exe",
            "${env:ProgramFiles(x86)}\7-Zip\7z.exe"
        )
        $sevenZip = $null
        foreach ($p in $sevenZipPaths) {
            if (Test-Path $p) { $sevenZip = $p; break }
        }

        if ($sevenZip) {
            New-Item -ItemType Directory -Force -Path $llvm18Dir | Out-Null
            & $sevenZip x $llvm18Installer -o"$llvm18Dir" "bin\libclang.dll" -y 2>&1 | Out-Null
        }
        else {
            Write-Host "  7-Zip not found, running LLVM 18 silent installer..." -ForegroundColor Gray
            Start-Process -FilePath $llvm18Installer -ArgumentList "/S", "/D=$llvm18Dir" -Wait -NoNewWindow
        }

        Remove-Item $llvm18Installer -ErrorAction SilentlyContinue

        if (Test-Path "$llvm18Bin\libclang.dll") {
            Write-Host "  OK LLVM 18 libclang installed" -ForegroundColor Green
        }
        else {
            Write-Host "  WARNING: Failed to extract libclang.dll" -ForegroundColor Yellow
            Write-Host "  Please install LLVM 18 manually from:" -ForegroundColor Gray
            Write-Host "  $llvm18Url" -ForegroundColor Gray
        }
    }
    catch {
        Write-Host "  WARNING: Failed to download LLVM 18" -ForegroundColor Yellow
        Write-Host "  Error: $_" -ForegroundColor Red
        Write-Host "  Please install LLVM 18 manually and set LIBCLANG_PATH" -ForegroundColor Gray
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

# Install project dependencies
Write-Host ""
Write-Host "[9/9] Installing project dependencies..." -ForegroundColor Yellow

$needsInstall = $false
$projectRoot = "$PSScriptRoot\.."
$lockFile = "$projectRoot\pnpm-lock.yaml"
$nodeModules = "$projectRoot\node_modules"
$desktopNodeModules = "$projectRoot\apps\desktop\node_modules"

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
    $workspaceModified = (Get-Item "$projectRoot\pnpm-workspace.yaml").LastWriteTime
    $packageModified = (Get-Item "$projectRoot\package.json").LastWriteTime
    
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
    
    $lockFile = "$projectRoot\pnpm-lock.yaml"
    $lockExists = Test-Path $lockFile
    
    if ($lockExists) {
        $lockCreationTime = (Get-Item $lockFile).CreationTime
        $lockModifiedTime = (Get-Item $lockFile).LastWriteTime
        
        if ($lockCreationTime -lt (Get-Date).AddMonths(-1)) {
            Write-Host "  Lockfile is older than 1 month, regenerating to ensure compatibility..." -ForegroundColor Gray
            Remove-Item $lockFile -Force
        }
    }
    
    Push-Location $projectRoot
    pnpm install --no-optional=false
    Pop-Location
    
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
    Push-Location "$PSScriptRoot\..\apps\desktop\src-tauri"
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
