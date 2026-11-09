<#
.SYNOPSIS
    构建并启动 MetaAlign（Windows）。

.DESCRIPTION
    默认用调试配置构建后启动；加 -Release 用发布配置。
    可以把文件或目录当作启动参数传进去，应用会直接把它们塞进文件队列。
    构建失败、产物缺失时立刻报错退出，不会静默启动旧产物。

.PARAMETER Release
    使用 release 配置构建并启动（target\release\MetaAlign.exe）。

.PARAMETER Files
    启动时预填充的路径，可写多个：-Files C:\docs, D:\a.docx

.PARAMETER NoBuild
    跳过构建，直接启动已有产物（适合刚 build 完反复调试界面）。

.PARAMETER NoLaunch
    只构建并打印产物路径，不启动窗口（适合脚本化冒烟检查）。

.EXAMPLE
    pwsh -File scripts/run.ps1
    pwsh -File scripts/run.ps1 -Files C:\docs
    pwsh -File scripts/run.ps1 -Release
#>
param(
    [switch]$Release,
    [string[]]$Files,
    [switch]$NoBuild,
    [switch]$NoLaunch
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $root

if (-not $NoBuild) {
    $args = @("build", "-p", "metaalign-gui")
    if ($Release) {
        $args = @("build", "--release", "-p", "metaalign-gui")
    }
    Write-Host ("==> cargo " + ($args -join " "))
    & cargo @args
    if ($LASTEXITCODE -ne 0) {
        throw "构建失败（exit $LASTEXITCODE）"
    }
}

$exe = if ($Release) {
    Join-Path $root "target\release\MetaAlign.exe"
} else {
    Join-Path $root "target\debug\MetaAlign.exe"
}
if (-not (Test-Path -LiteralPath $exe)) {
    throw "缺少产物：$exe（先去掉 -NoBuild 构建一次）"
}

if ($NoLaunch) {
    Write-Host "==> 已构建：$exe"
    return
}

Write-Host "==> 启动：$exe"
if ($Files) {
    & $exe @Files
} else {
    & $exe
}
