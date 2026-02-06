param(
    [Parameter(Mandatory=$true)]
    [string]$Version,
    
    [Parameter(Mandatory=$false)]
    [switch]$PreRelease
)

$ErrorActionPreference = "Stop"

$CargoToml = "apps/desktop/src-tauri/Cargo.toml"
$tagName = "v$Version"

Write-Host "=== Cap aniBullet 自动发布脚本 ===" -ForegroundColor Cyan
Write-Host ""

if ($PreRelease) {
    Write-Host "准备发布预发布版本: $Version" -ForegroundColor Yellow
} else {
    Write-Host "准备发布正式版本: $Version" -ForegroundColor Green
}

if (-not (Test-Path $CargoToml)) {
    Write-Host "错误: 找不到 $CargoToml" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "步骤 1: 检查工作区状态" -ForegroundColor Cyan

$gitStatus = git status --porcelain 2>&1
if ($gitStatus -and $gitStatus -match '^\s*[MADRCU]') {
    Write-Host "工作区有未提交的更改，自动暂存..." -ForegroundColor Yellow
    git add -A
    git commit -m "chore: prepare for version $Version release"
    Write-Host "已自动提交更改" -ForegroundColor Green
} else {
    Write-Host "工作区干净" -ForegroundColor Green
}

Write-Host ""
Write-Host "步骤 2: 更新版本号" -ForegroundColor Cyan

$content = Get-Content $CargoToml -Raw
$oldVersion = if ($content -match 'version\s*=\s*"([^"]*)"') { $matches[1] } else { "unknown" }
Write-Host "当前版本: $oldVersion" -ForegroundColor Gray
Write-Host "新版本: $Version" -ForegroundColor Gray

$content = $content -replace 'version\s*=\s*"[^"]*"', "version = `"$Version`"", 1
$content | Set-Content $CargoToml -NoNewline

Write-Host "版本号已更新" -ForegroundColor Green

Write-Host ""
Write-Host "步骤 3: 提交版本更新" -ForegroundColor Cyan

git add $CargoToml
git commit -m "chore: bump version to $Version" 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) {
    Write-Host "警告: 提交失败或无需提交（可能版本已更新）" -ForegroundColor Yellow
}

Write-Host "版本更新已提交" -ForegroundColor Green

Write-Host ""
Write-Host "步骤 4: 处理 Git 标签" -ForegroundColor Cyan

$localTagExists = git tag -l $tagName
if ($localTagExists) {
    Write-Host "本地标签 $tagName 已存在，自动删除..." -ForegroundColor Yellow
    git tag -d $tagName | Out-Null
    Write-Host "已删除本地旧标签" -ForegroundColor Green
}

git ls-remote --tags origin 2>&1 | Out-Null
if ($LASTEXITCODE -eq 0) {
    $remoteTags = git ls-remote --tags origin 2>&1
    if ($remoteTags -match "refs/tags/$tagName") {
        Write-Host "远程标签 $tagName 已存在，自动删除..." -ForegroundColor Yellow
        git push origin ":refs/tags/$tagName" 2>&1 | Out-Null
        if ($LASTEXITCODE -eq 0) {
            Write-Host "已删除远程旧标签" -ForegroundColor Green
        } else {
            Write-Host "警告: 无法删除远程标签（可能没有权限或标签不存在）" -ForegroundColor Yellow
        }
        Start-Sleep -Seconds 2
    }
}

git tag $tagName
if ($LASTEXITCODE -ne 0) {
    Write-Host "错误: 无法创建标签 $tagName" -ForegroundColor Red
    exit 1
}

Write-Host "已创建新标签: $tagName" -ForegroundColor Green

Write-Host ""
Write-Host "步骤 5: 推送到远程仓库" -ForegroundColor Cyan

Write-Host "推送提交..." -ForegroundColor Gray
$pushOutput = git push origin main 2>&1
if ($LASTEXITCODE -ne 0) {
    if ($pushOutput -match "Everything up-to-date") {
        Write-Host "提交已是最新" -ForegroundColor Green
    } else {
        Write-Host "错误: 推送提交失败" -ForegroundColor Red
        Write-Host $pushOutput -ForegroundColor Gray
        Write-Host "清理本地标签..." -ForegroundColor Yellow
        git tag -d $tagName | Out-Null
        exit 1
    }
} else {
    Write-Host "提交已推送" -ForegroundColor Green
}

Write-Host "推送标签..." -ForegroundColor Gray
$maxRetries = 3
$retryCount = 0
$tagPushed = $false

while ($retryCount -lt $maxRetries -and -not $tagPushed) {
    $pushTagOutput = git push origin $tagName 2>&1
    if ($LASTEXITCODE -eq 0) {
        $tagPushed = $true
        Write-Host "标签已推送" -ForegroundColor Green
    } else {
        $retryCount++
        if ($retryCount -lt $maxRetries) {
            Write-Host "推送失败，等待 2 秒后重试 ($retryCount/$maxRetries)..." -ForegroundColor Yellow
            Start-Sleep -Seconds 2
        } else {
            Write-Host "错误: 推送标签失败" -ForegroundColor Red
            Write-Host $pushTagOutput -ForegroundColor Gray
            exit 1
        }
    }
}

Write-Host ""
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "✓ 发布流程完成！" -ForegroundColor Green
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "版本: $Version" -ForegroundColor White
Write-Host "标签: $tagName" -ForegroundColor White
Write-Host ""
Write-Host "GitHub Actions 将在几秒钟内开始构建..." -ForegroundColor Yellow

$repoUrl = git config --get remote.origin.url
if ($repoUrl -match '([^/:]+/[^/]+)\.git') {
    $repoPath = $matches[1]
    Write-Host "查看构建进度: https://github.com/$repoPath/actions" -ForegroundColor Cyan
    Write-Host "查看标签: https://github.com/$repoPath/tags" -ForegroundColor Cyan
}

Write-Host ""
Write-Host "提示: 构建通常需要 10-20 分钟完成" -ForegroundColor Gray
