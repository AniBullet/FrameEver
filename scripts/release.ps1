param(
    [Parameter(Mandatory=$true)]
    [string]$Version,
    
    [Parameter(Mandatory=$false)]
    [switch]$PreRelease
)

$ErrorActionPreference = "Stop"

$tagName = "v$Version"

Write-Host "=== Cap aniBullet Release Script ===" -ForegroundColor Cyan
Write-Host ""

if ($PreRelease) {
    Write-Host "Creating pre-release: $Version" -ForegroundColor Yellow
} else {
    Write-Host "Creating release: $Version" -ForegroundColor Green
}

Write-Host ""
Write-Host "Step 1: Check working directory" -ForegroundColor Cyan

$gitStatus = git status --porcelain 2>&1
if ($gitStatus -and $gitStatus -match '^\s*[MADRCU]') {
    Write-Host "Warning: Working directory has uncommitted changes" -ForegroundColor Yellow
    Write-Host "These changes will NOT be included in the release tag" -ForegroundColor Yellow
    Write-Host ""
}

Write-Host ""
Write-Host "Step 2: Handle existing tag" -ForegroundColor Cyan

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
        $prevErrorAction = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        git push origin ":refs/tags/$tagName" 2>&1 | Out-Null
        $ErrorActionPreference = $prevErrorAction
        if ($LASTEXITCODE -eq 0) {
            Write-Host "Old remote tag deleted" -ForegroundColor Green
            Start-Sleep -Seconds 2
        } else {
            Write-Host "Warning: Cannot delete remote tag" -ForegroundColor Yellow
        }
    }
}

Write-Host ""
Write-Host "Step 3: Create and push tag" -ForegroundColor Cyan

git tag $tagName
if ($LASTEXITCODE -ne 0) {
    Write-Host "Error: Cannot create tag $tagName" -ForegroundColor Red
    exit 1
}

Write-Host "Tag created: $tagName" -ForegroundColor Green

Write-Host "Pushing tag to remote..." -ForegroundColor Gray

$maxRetries = 3
$retryCount = 0
$tagPushed = $false

while ($retryCount -lt $maxRetries -and -not $tagPushed) {
    $prevErrorAction = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $pushTagOutput = git push origin $tagName 2>&1
    $ErrorActionPreference = $prevErrorAction
    if ($LASTEXITCODE -eq 0) {
        $tagPushed = $true
        Write-Host "Tag pushed successfully" -ForegroundColor Green
    } else {
        $retryCount++
        if ($retryCount -lt $maxRetries) {
            Write-Host "Push failed, retrying in 2 seconds ($retryCount/$maxRetries)..." -ForegroundColor Yellow
            Start-Sleep -Seconds 2
        } else {
            Write-Host "Error: Failed to push tag" -ForegroundColor Red
            Write-Host $pushTagOutput -ForegroundColor Gray
            git tag -d $tagName | Out-Null
            exit 1
        }
    }
}

Write-Host ""
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "Release tag created!" -ForegroundColor Green
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Tag: $tagName" -ForegroundColor White
Write-Host ""
Write-Host "GitHub Actions will start building in a few seconds..." -ForegroundColor Yellow

$repoUrl = git config --get remote.origin.url
if ($repoUrl -match '([^/:]+/[^/]+)\.git') {
    $repoPath = $matches[1]
    Write-Host ""
    Write-Host "Monitor build: https://github.com/$repoPath/actions" -ForegroundColor Cyan
    Write-Host "View release: https://github.com/$repoPath/releases/tag/$tagName" -ForegroundColor Cyan
}

Write-Host ""
Write-Host "Note: Build typically takes 10-20 minutes to complete" -ForegroundColor Gray
