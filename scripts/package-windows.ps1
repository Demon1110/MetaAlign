<#
.SYNOPSIS
    打包 Windows 运行包。

.DESCRIPTION
    构建 release 产物，把可执行文件、README、LICENSE 和应用图标装进
    dist\MetaAlign-<版本>-windows-x64.zip，并打印体积与 SHA-256。
    版本号默认从 Cargo.toml 的 [workspace.package] 读取，可用 -Version 覆盖。

.PARAMETER Version
    覆盖版本号（默认取 Cargo.toml 里的 version）。

.PARAMETER SkipBuild
    跳过 cargo build，直接打包已有 release 产物。

.PARAMETER NoZip
    只生成 dist\stage 目录，不打 zip（方便本地直接跑）。

.PARAMETER Upx
    打包前用 UPX 压缩 exe（体积约 2.4 MB，启动稍慢；部分杀软可能误报）。

.EXAMPLE
    pwsh -File scripts/package-windows.ps1
    pwsh -File scripts/package-windows.ps1 -SkipBuild
    pwsh -File scripts/package-windows.ps1 -Upx
    pwsh -File scripts/package-windows.ps1 -Version 0.2.0
#>
param(
    [string]$Version,
    [switch]$SkipBuild,
    [switch]$NoZip,
    [switch]$Upx
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $root

if (-not $Version) {
    $toml = Get-Content -LiteralPath "Cargo.toml" -Raw
    $match = [regex]::Match($toml, '(?m)^version\s*=\s*"([^"]+)"')
    if (-not $match.Success) {
        throw "无法从 Cargo.toml 解析版本号，请用 -Version 指定"
    }
    $Version = $match.Groups[1].Value
}

if (-not $SkipBuild) {
    Write-Host "==> cargo build --release -p metaalign-gui"
    cargo build --release -p metaalign-gui
    if ($LASTEXITCODE -ne 0) {
        throw "构建失败（exit $LASTEXITCODE）"
    }
}

$exe = Join-Path $root "target\release\MetaAlign.exe"
if (-not (Test-Path -LiteralPath $exe)) {
    throw "缺少产物：$exe（先去掉 -SkipBuild 构建一次）"
}

$name = "MetaAlign-$Version-windows-x64"
$stage = Join-Path $root "dist\stage\$name"
if (Test-Path -LiteralPath $stage) {
    Remove-Item -LiteralPath $stage -Recurse -Force
}
New-Item -ItemType Directory -Path $stage -Force | Out-Null

Copy-Item -LiteralPath $exe -Destination (Join-Path $stage "MetaAlign.exe")
foreach ($file in @("README.md", "LICENSE")) {
    $src = Join-Path $root $file
    if (Test-Path -LiteralPath $src) {
        Copy-Item -LiteralPath $src -Destination $stage
    }
}
Copy-Item -LiteralPath (Join-Path $root "crates\metaalign-gui\assets\icon.png") `
          -Destination (Join-Path $stage "MetaAlign.png")

$exeInfo = Get-Item -LiteralPath $exe
Write-Host ("==> 产物目录：{0}" -f $stage)
Write-Host ("    MetaAlign.exe  {0:N2} MB" -f ($exeInfo.Length / 1MB))

if ($Upx) {
    $stagedExe = Join-Path $stage "MetaAlign.exe"
    # 变量名不能叫 $upx，和 -Upx 这个 switch 参数同名（PowerShell 不区分大小写）
    $upxPath = (Get-Command upx -ErrorAction SilentlyContinue).Source
    if (-not $upxPath) {
        throw "未找到 upx，可 winget install UPX.UPX，或去掉 -Upx"
    }
    $before = (Get-Item -LiteralPath $stagedExe).Length
    & $upxPath --best --lzma $stagedExe *> $null
    if ($LASTEXITCODE -ne 0) {
        throw "upx 压缩失败（exit $LASTEXITCODE）"
    }
    $after = (Get-Item -LiteralPath $stagedExe).Length
    Write-Host ("    UPX 压缩：{0:N2} MB -> {1:N2} MB" -f ($before / 1MB), ($after / 1MB))
}

if ($NoZip) {
    return
}

$zip = Join-Path $root "dist\$name.zip"
if (Test-Path -LiteralPath $zip) {
    Remove-Item -LiteralPath $zip -Force
}
Compress-Archive -Path $stage -DestinationPath $zip -Force

$zipInfo = Get-Item -LiteralPath $zip
$hash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLower()
# 校验和随包发布，方便下载后核对
"$hash  $($zipInfo.Name)" | Set-Content -LiteralPath "$zip.sha256" -Encoding ascii
Write-Host ("==> 压缩包：{0}" -f $zip)
Write-Host ("    {0:N2} MB" -f ($zipInfo.Length / 1MB))
Write-Host ("    sha256: {0}" -f $hash)
