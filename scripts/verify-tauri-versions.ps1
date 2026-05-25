param(
    [string]$ProjectRoot = "$PSScriptRoot\.."
)

$ErrorActionPreference = "Stop"

function Get-MajorMinor {
    param([string]$Version)
    $clean = $Version -replace '^[\^~>=<]+', ''
    $parts = $clean -split '\.'
    if ($parts.Count -ge 2) {
        return "$($parts[0]).$($parts[1])"
    }
    return $clean
}

function Get-CargoLockVersion {
    param([string]$CrateName, [string]$LockFile)
    if (-not (Test-Path $LockFile)) {
        return $null
    }
    $pattern = "(?ms)^\[\[package\]\]\r?\nname = `"$([regex]::Escape($CrateName))`"\r?\nversion = `"([^`"]+)`""
    $content = Get-Content $LockFile -Raw
    if ($content -match $pattern) {
        return $Matches[1]
    }
    return $null
}

function Get-InstalledNpmVersion {
    param([string]$PackageName, [string]$DesktopDir, [string]$RootDir)
    $candidates = @(
        (Join-Path $DesktopDir "node_modules\$PackageName\package.json"),
        (Join-Path $RootDir "node_modules\$PackageName\package.json")
    )
    foreach ($path in $candidates) {
        if (Test-Path $path) {
            $pkg = Get-Content $path -Raw | ConvertFrom-Json
            return $pkg.version
        }
    }
    $escaped = $PackageName.Replace("@", "").Replace("/", "+")
    $pnpmDir = Join-Path $RootDir "node_modules\.pnpm"
    if (Test-Path $pnpmDir) {
        $match = Get-ChildItem $pnpmDir -Directory -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -like "${escaped}@*" } |
            Select-Object -First 1
        if ($match) {
            $pkgPath = Join-Path $match.FullName "node_modules\$PackageName\package.json"
            if (Test-Path $pkgPath) {
                $pkg = Get-Content $pkgPath -Raw | ConvertFrom-Json
                return $pkg.version
            }
        }
    }
    return $null
}

$desktopDir = Join-Path $ProjectRoot "apps\desktop"
$cargoLock = Join-Path $ProjectRoot "Cargo.lock"
$packageJsonPath = Join-Path $desktopDir "package.json"

if (-not (Test-Path $packageJsonPath)) {
    Write-Host "  ERROR: apps/desktop/package.json not found" -ForegroundColor Red
    exit 1
}

$packageJson = Get-Content $packageJsonPath -Raw | ConvertFrom-Json
$allDeps = @{}
foreach ($prop in $packageJson.dependencies.PSObject.Properties) {
    $allDeps[$prop.Name] = [string]$prop.Value
}
foreach ($prop in $packageJson.devDependencies.PSObject.Properties) {
    $allDeps[$prop.Name] = [string]$prop.Value
}

$rustToNpm = @{
    "tauri" = "@tauri-apps/api"
}

foreach ($depName in $allDeps.Keys) {
    if ($depName -like "@tauri-apps/plugin-*") {
        $rustName = $depName.Replace("@tauri-apps/", "tauri-")
        $rustToNpm[$rustName] = $depName
    }
}

$mismatches = @()

foreach ($entry in $rustToNpm.GetEnumerator()) {
    $rustCrate = $entry.Key
    $npmPackage = $entry.Value
    $rustVersion = Get-CargoLockVersion -CrateName $rustCrate -LockFile $cargoLock
    $npmVersion = Get-InstalledNpmVersion -PackageName $npmPackage -DesktopDir $desktopDir -RootDir $ProjectRoot

    if (-not $rustVersion) {
        continue
    }

    if (-not $npmVersion) {
        $mismatches += "$rustCrate (Cargo.lock $rustVersion) : $npmPackage (not installed)"
        continue
    }

    $rustMm = Get-MajorMinor $rustVersion
    $npmMm = Get-MajorMinor $npmVersion
    if ($rustMm -ne $npmMm) {
        $mismatches += "$rustCrate (v$rustVersion) : $npmPackage (v$npmVersion)"
    }
}

if ($mismatches.Count -gt 0) {
    Write-Host "  ERROR: Tauri Rust/NPM version mismatch" -ForegroundColor Red
    foreach ($line in $mismatches) {
        Write-Host "    $line" -ForegroundColor Yellow
    }
    Write-Host "  Fix: run .\scripts\1-install.ps1 (do not run cargo update manually)" -ForegroundColor Gray
    exit 1
}

Write-Host "  OK Tauri Rust/NPM versions aligned" -ForegroundColor Green
exit 0
