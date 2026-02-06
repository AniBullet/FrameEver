param(
    [Parameter(Mandatory=$true)]
    [string]$Version,
    
    [Parameter(Mandatory=$false)]
    [switch]$PreRelease
)

$ErrorActionPreference = "Stop"

$CargoToml = "apps/desktop/src-tauri/Cargo.toml"
$tagName = "v$Version"

Write-Host "=== Cap aniBullet Auto Release Script ===" -ForegroundColor Cyan
Write-Host ""

if ($PreRelease) {
    Write-Host "Preparing pre-release version: $Version" -ForegroundColor Yellow
} else {
    Write-Host "Preparing release version: $Version" -ForegroundColor Green
}

if (-not (Test-Path $CargoToml)) {
    Write-Host "Error: Cannot find $CargoToml" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "Step 1: Check working directory" -ForegroundColor Cyan

$gitStatus = git status --porcelain 2>&1
if ($gitStatus -and $gitStatus -match '^\s*[MADRCU]') {
    Write-Host "Working directory has uncommitted changes, auto-staging..." -ForegroundColor Yellow
    git add -A
    git commit -m "chore: prepare for version $Version release"
    Write-Host "Changes committed automatically" -ForegroundColor Green
} else {
    Write-Host "Working directory is clean" -ForegroundColor Green
}

Write-Host ""
Write-Host "Step 2: Update version number" -ForegroundColor Cyan

$content = Get-Content $CargoToml -Raw
$oldVersion = if ($content -match 'version\s*=\s*"([^"]*)"') { $matches[1] } else { "unknown" }
Write-Host "Current version: $oldVersion" -ForegroundColor Gray
Write-Host "New version: $Version" -ForegroundColor Gray

$content = $content -replace 'version\s*=\s*"[^"]*"', "version = `"$Version`""
$content | Set-Content $CargoToml -NoNewline

Write-Host "Version number updated" -ForegroundColor Green

Write-Host ""
Write-Host "Step 3: Commit version update" -ForegroundColor Cyan

git add $CargoToml
git commit -m "chore: bump version to $Version" 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) {
    Write-Host "Warning: Commit failed or not needed (version may already be updated)" -ForegroundColor Yellow
}

Write-Host "Version update committed" -ForegroundColor Green

Write-Host ""
Write-Host "Step 4: Handle Git tag" -ForegroundColor Cyan

$localTagExists = git tag -l $tagName
if ($localTagExists) {
    Write-Host "Local tag $tagName exists, deleting..." -ForegroundColor Yellow
    git tag -d $tagName | Out-Null
    Write-Host "Old local tag deleted" -ForegroundColor Green
}

git ls-remote --tags origin 2>&1 | Out-Null
if ($LASTEXITCODE -eq 0) {
    $remoteTags = git ls-remote --tags origin 2>&1
    if ($remoteTags -match "refs/tags/$tagName") {
        Write-Host "Remote tag $tagName exists, deleting..." -ForegroundColor Yellow
        git push origin ":refs/tags/$tagName" 2>&1 | Out-Null
        if ($LASTEXITCODE -eq 0) {
            Write-Host "Old remote tag deleted" -ForegroundColor Green
        } else {
            Write-Host "Warning: Cannot delete remote tag (may lack permission or tag does not exist)" -ForegroundColor Yellow
        }
        Start-Sleep -Seconds 2
    }
}

git tag $tagName
if ($LASTEXITCODE -ne 0) {
    Write-Host "Error: Cannot create tag $tagName" -ForegroundColor Red
    exit 1
}

Write-Host "New tag created: $tagName" -ForegroundColor Green

Write-Host ""
Write-Host "Step 5: Push to remote repository" -ForegroundColor Cyan

Write-Host "Pushing commit..." -ForegroundColor Gray
$pushOutput = git push origin main 2>&1
if ($LASTEXITCODE -ne 0) {
    if ($pushOutput -match "Everything up-to-date") {
        Write-Host "Commit is already up-to-date" -ForegroundColor Green
    } else {
        Write-Host "Error: Failed to push commit" -ForegroundColor Red
        Write-Host $pushOutput -ForegroundColor Gray
        Write-Host "Cleaning up local tag..." -ForegroundColor Yellow
        git tag -d $tagName | Out-Null
        exit 1
    }
} else {
    Write-Host "Commit pushed" -ForegroundColor Green
}

Write-Host "Pushing tag..." -ForegroundColor Gray
$maxRetries = 3
$retryCount = 0
$tagPushed = $false

while ($retryCount -lt $maxRetries -and -not $tagPushed) {
    $pushTagOutput = git push origin $tagName 2>&1
    if ($LASTEXITCODE -eq 0) {
        $tagPushed = $true
        Write-Host "Tag pushed" -ForegroundColor Green
    } else {
        $retryCount++
        if ($retryCount -lt $maxRetries) {
            Write-Host "Push failed, retrying in 2 seconds ($retryCount/$maxRetries)..." -ForegroundColor Yellow
            Start-Sleep -Seconds 2
        } else {
            Write-Host "Error: Failed to push tag" -ForegroundColor Red
            Write-Host $pushTagOutput -ForegroundColor Gray
            exit 1
        }
    }
}

Write-Host ""
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "Release process complete!" -ForegroundColor Green
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Version: $Version" -ForegroundColor White
Write-Host "Tag: $tagName" -ForegroundColor White
Write-Host ""
Write-Host "GitHub Actions will start building in a few seconds..." -ForegroundColor Yellow

$repoUrl = git config --get remote.origin.url
if ($repoUrl -match '([^/:]+/[^/]+)\.git') {
    $repoPath = $matches[1]
    Write-Host "Monitor build: https://github.com/$repoPath/actions" -ForegroundColor Cyan
    Write-Host "View tags: https://github.com/$repoPath/tags" -ForegroundColor Cyan
}

Write-Host ""
Write-Host "Note: Build typically takes 10-20 minutes to complete" -ForegroundColor Gray
