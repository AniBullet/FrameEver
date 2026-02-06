param(
    [Parameter(Mandatory=$true)]
    [string]$Version,
    
    [Parameter(Mandatory=$false)]
    [switch]$PreRelease,
    
    [Parameter(Mandatory=$false)]
    [switch]$Force
)

$ErrorActionPreference = "Stop"

$CargoToml = "apps/desktop/src-tauri/Cargo.toml"

Write-Host "=== Cap aniBullet Release Script ===" -ForegroundColor Cyan
Write-Host ""

if ($PreRelease) {
    Write-Host "Creating pre-release version: $Version" -ForegroundColor Yellow
} else {
    Write-Host "Creating release version: $Version" -ForegroundColor Green
}

if (-not (Test-Path $CargoToml)) {
    Write-Host "Error: Cannot find $CargoToml" -ForegroundColor Red
    exit 1
}

$gitStatus = git status --porcelain
if ($gitStatus -and -not $Force) {
    Write-Host "Error: Working directory is not clean. Commit or stash changes first." -ForegroundColor Red
    Write-Host "Use -Force to override this check." -ForegroundColor Yellow
    exit 1
}

Write-Host ""
Write-Host "Step 1: Updating version in $CargoToml" -ForegroundColor Cyan

$content = Get-Content $CargoToml -Raw
$content = $content -replace 'version\s*=\s*"[^"]*"', "version = `"$Version`""
$content | Set-Content $CargoToml -NoNewline

Write-Host "Version updated to $Version" -ForegroundColor Green

Write-Host ""
Write-Host "Step 2: Committing version change" -ForegroundColor Cyan

git add $CargoToml
git commit -m "chore: bump version to $Version"

Write-Host "Committed version change" -ForegroundColor Green

Write-Host ""
Write-Host "Step 3: Creating and pushing tag" -ForegroundColor Cyan

$tagName = "v$Version"
git tag $tagName

Write-Host "Created tag: $tagName" -ForegroundColor Green

Write-Host ""
Write-Host "Step 4: Pushing to remote" -ForegroundColor Cyan

git push origin main
git push origin $tagName

Write-Host "Pushed commit and tag to remote" -ForegroundColor Green

Write-Host ""
Write-Host "=== Release process complete! ===" -ForegroundColor Cyan
Write-Host "GitHub Actions will now build the release." -ForegroundColor Green
Write-Host "Monitor progress at: https://github.com/$(git config --get remote.origin.url | Select-String -Pattern '([^/:]+/[^/]+)\.git$' | ForEach-Object { $_.Matches.Groups[1].Value })/actions" -ForegroundColor Yellow
