<#
.SYNOPSIS
PC① で ball_tracker を起動する (adb reverse と、記録の保存まで)。

.DESCRIPTION
1. つないであるスマホすべてに adb reverse を張る (6060、5050、5052)
2. ffmpeg を探す (カメラの動画を PC で解析するのに使う)
3. ball_tracker を起動する。出力は画面と記録フォルダーの tracker_<時分>.log (UTF-8) の両方に出す。
   カメラから受け取った動画は記録フォルダーの video\ に保存する (あとで解析し直すため。2 台で 1 時間に 25GB ほど)
4. 止めたあと (Ctrl+C)、この回の -j の記録 (events_*、frames_*) を記録フォルダーに写す

先に ball_tracker をビルドしておく (docs/setup_pc1.md の 3 章)。
実行ポリシーで止められるときは、次のように起動する:
    powershell -ExecutionPolicy Bypass -File scripts\start-pc1.ps1

.PARAMETER Output
network なら projector に送る (projector がつなぐまで先へ進まない)。csv なら記録フォルダーに CSV を書く。

.PARAMETER Dir
記録フォルダー。既定は recordings\<今日の日付>_experiment。

.PARAMETER NoVideoAnalysis
カメラの動画を受け付けない (カメラが自分で解析する)。

.PARAMETER NoVideoRecord
カメラから受け取った動画を保存しない。

.PARAMETER TimeTrimMs
右のカメラの時刻に足す補正 (ms)。ball_tracker の --time-trim-ms。

.PARAMETER ReportDelayMs
着弾を、予測した着弾時刻の何 ms 後に報告するか (負なら前)。ball_tracker の --report-delay-ms。

.PARAMETER AutoTimeTrim
右のカメラの時刻の補正を、報告した投球から自動で決める (ball_tracker の --auto-time-trim)。

.PARAMETER VideoHwaccel
動画のデコードに使う GPU (ball_tracker の --video-hwaccel)。既定は auto。none で CPU。

.PARAMETER TrackerArgs
ball_tracker にそのまま渡す追加の引数。空白かカンマで区切った 1 つの文字列で渡す
(例: -TrackerArgs '--rolling-shutter-skew-ms 8.333')。powershell -File で呼ぶと配列はそのまま渡せないため。

.EXAMPLE
scripts\start-pc1.ps1 -Output csv -TimeTrimMs -5 -ReportDelayMs 300
#>
param(
    [ValidateSet('network', 'csv')]
    [string]$Output = 'network',
    [string]$Dir,
    [switch]$NoVideoAnalysis,
    [switch]$NoVideoRecord,
    [Nullable[double]]$TimeTrimMs,
    [Nullable[double]]$ReportDelayMs,
    [switch]$AutoTimeTrim,
    [string]$VideoHwaccel = 'auto',
    [string[]]$TrackerArgs = @()
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$tracker = Join-Path $root 'target\release\ball_tracker.exe'
if (-not (Test-Path $tracker)) {
    throw "ball_tracker がビルドされていません: $tracker`n" +
        "リポジトリのルートで: `$env:SQLX_OFFLINE='true'; mise exec rust protoc -- cargo build --release -p ball_tracker"
}

if (-not $Dir) {
    $Dir = Join-Path $root ("recordings\{0}_experiment" -f (Get-Date -Format 'yyyy-MM-dd'))
}
New-Item -ItemType Directory -Force $Dir | Out-Null
$Dir = (Resolve-Path $Dir).Path
$stamp = Get-Date -Format 'HHmm'

# --- adb reverse ---
$adb = (Get-Command adb -ErrorAction SilentlyContinue).Source
if (-not $adb) {
    $candidate = Join-Path $env:LOCALAPPDATA 'Android\Sdk\platform-tools\adb.exe'
    if (Test-Path $candidate) { $adb = $candidate }
}
if ($adb) {
    $serials = & $adb devices | Select-String "`tdevice$" | ForEach-Object { ($_.Line -split "`t")[0] }
    if (-not $serials) {
        Write-Warning "スマホが見つかりません (adb devices)。つないだあとで scripts\adb-reverse.sh か、このスクリプトを起動し直してください"
    }
    foreach ($serial in $serials) {
        foreach ($port in 6060, 5050, 5052) {
            & $adb -s $serial reverse "tcp:$port" "tcp:$port" | Out-Null
        }
        Write-Host "adb reverse: $serial (6060, 5050, 5052)"
    }
} else {
    Write-Warning 'adb が見つかりません。スマホを使うなら Android SDK の platform-tools を入れてください'
}

# --- ffmpeg ---
$arguments = @('--input', 'network', '--output', $Output, '-j')
if ($Output -eq 'csv') {
    $arguments += @('-c', (Join-Path $Dir "collisions_${stamp}_raw.csv"))
}
if ($NoVideoAnalysis) {
    $arguments += '--no-video-analysis'
} else {
    $ffmpeg = (Get-Command ffmpeg -ErrorAction SilentlyContinue).Source
    if (-not $ffmpeg) {
        # winget で入れた直後は、開き直す前のターミナルの PATH に入っていない
        $ffmpeg = Get-ChildItem (Join-Path $env:LOCALAPPDATA 'Microsoft\WinGet\Packages') -Filter ffmpeg.exe -Recurse -ErrorAction SilentlyContinue |
            Select-Object -First 1 -ExpandProperty FullName
    }
    if (-not $ffmpeg) {
        throw "ffmpeg が見つかりません。winget install --id Gyan.FFmpeg -e で入れるか、-NoVideoAnalysis を付けてください"
    }
    $arguments += @('--ffmpeg', $ffmpeg, '--video-snapshot-dir', (Join-Path $Dir 'snapshots'))
    if (-not $NoVideoRecord) {
        $arguments += @('--video-record-dir', (Join-Path $Dir 'video'))
    }
}
if ($null -ne $TimeTrimMs) { $arguments += @('--time-trim-ms', [string]$TimeTrimMs) }
if ($null -ne $ReportDelayMs) { $arguments += @('--report-delay-ms', [string]$ReportDelayMs) }
if ($AutoTimeTrim) { $arguments += '--auto-time-trim' }
if (-not $NoVideoAnalysis) { $arguments += @('--video-hwaccel', $VideoHwaccel) }
# powershell -File で '-a','b' と渡すと 1 つの文字列 "-a,b" になるので、空白とカンマで分ける
$arguments += @($TrackerArgs | ForEach-Object { $_ -split '[,\s]+' } | Where-Object { $_ })

# --- ball_tracker ---
$log = Join-Path $Dir "tracker_$stamp.log"
$jsonDir = Join-Path $env:APPDATA 'struckout\tracker\json'
$started = Get-Date
Write-Host "記録フォルダー: $Dir"
Write-Host "ball_tracker $($arguments -join ' ')"
Write-Host '止めるときは Ctrl+C'

$env:NO_COLOR = '1'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$writer = [System.IO.StreamWriter]::new($log, $false, [System.Text.UTF8Encoding]::new($false))
$writer.AutoFlush = $true
try {
    & $tracker @arguments 2>&1 | ForEach-Object {
        $line = "$_"
        $writer.WriteLine($line)
        $line
    }
} finally {
    $writer.Dispose()
    # この回の -j の記録を写す
    Get-ChildItem $jsonDir -ErrorAction SilentlyContinue |
        Where-Object { $_.LastWriteTime -ge $started -and $_.Name -match '^(events|frames)_' } |
        ForEach-Object { Copy-Item $_.FullName (Join-Path $Dir $_.Name) -Force }
    Write-Host "ball_tracker の記録を保存しました: $Dir"
}
