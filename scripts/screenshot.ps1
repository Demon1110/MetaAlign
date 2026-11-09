param(
    [string]$Out = "D:\java\git\MetaAlign\docs\screenshots\app.png",
    [string]$Page = "files",
    [string[]]$Files = @(),
    [int]$DelaySeconds = 8
)

Set-Location "D:\java\git\MetaAlign"
Get-Process -Name MetaAlign -ErrorAction SilentlyContinue | Stop-Process -Force

cargo build -p metaalign-gui *> "$env:TEMP\ma_build.txt"
if ($LASTEXITCODE -ne 0) {
    "BUILD FAILED"
    Get-Content -LiteralPath "$env:TEMP\ma_build.txt" -Tail 25
    exit 1
}

New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Out) | Out-Null
Remove-Item -LiteralPath $Out -ErrorAction SilentlyContinue

$env:METAALIGN_SHOT = $Out
$env:METAALIGN_SHOT_PAGE = $Page

$arguments = @()
foreach ($item in $Files) {
    if (Test-Path -LiteralPath $item) { $arguments += $item }
}

$errorLog = "$env:TEMP\ma_shot_err.txt"
$startArgs = @{
    FilePath = "D:\java\git\MetaAlign\target\debug\MetaAlign.exe"
    PassThru = $true
    RedirectStandardError = $errorLog
}
if ($arguments.Count -gt 0) { $startArgs['ArgumentList'] = $arguments }
$process = Start-Process @startArgs

Start-Sleep -Seconds $DelaySeconds
"shot=$(Test-Path -LiteralPath $Out) exited=$($process.HasExited) page=$Page"
if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force }
Get-Content -LiteralPath $errorLog -Raw
