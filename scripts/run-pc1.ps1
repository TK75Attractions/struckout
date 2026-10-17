<#
.SYNOPSIS
PC① をひとまとめに起動する: ball_tracker を起動し、スマホに設定を入れて開き、確かめるところまで。

.DESCRIPTION
    powershell -ExecutionPolicy Bypass -File scripts\run-pc1.ps1
    powershell -ExecutionPolicy Bypass -File scripts\run-pc1.ps1 -Output csv -Name test
    powershell -ExecutionPolicy Bypass -File scripts\run-pc1.ps1 -Record          # 受け取った動画も残す (実験用)

次の順に進め、それぞれ ball_tracker のログで済んだことを確かめてから次へ進む。

1. 電源、ffmpeg、adb、スマホ (scripts\cameras.json の端末) を確かめる
2. スマホのアプリをいったん止め、adb reverse (6060、5050、5052) を張り、設定 (cameras.json) を入れる。
   このとき端末の識別子 (device_id) を cameras.json に記録する
3. ball_tracker を scripts\pc1.json の設定と --cameras scripts\cameras.json で起動する。
   ball_tracker は device_id で端末を見分け、cameras.json の順の camera_id (0, 1, ...) を振り、位置と焦点距離を端末に送る
4. (output が network のとき) PC② に入れるつなぎ先を表示する。projector は待たない (いつつないでもよい)
5. スマホを開き、動画が届くのを確かめる
6. 届いている fps と遅れを表にし、Save frame を撮って取り込む
7. 投球中は、着弾と警告、projector とスマホのつながり具合だけを表示する。Ctrl+C で止めると、記録をまとめ、
   スマホのアプリを止める

スマホのアプリと projector は、ball_tracker が止まっても自分でつなぎ直すので、ball_tracker だけを起動し直してもよい
(このスクリプトをもう一度実行する)。

記録は recordings\<日付>_<時分>[_<Name>] に残す (ball_tracker のログ、-j の記録、Save frame。-Record のときは
受け取った動画も)。動画は 2 台で 1 時間に約 45 GB になるので、稼働中は残さない。ディスクの空きは起動時と 1 分おきに確かめる。

.PARAMETER Output
pc1.json の output を上書きする。network は projector に送る。csv は projector なしで試すとき (着弾は CSV に書く)。

.PARAMETER Name
記録フォルダーの名前に付ける言葉。

.PARAMETER NoSaveFrame
Save frame を撮らない。

.PARAMETER Record
受け取った動画を残す (あとで解析し直すため。pc1.json の record_video より優先)。2 台で 1 時間に約 45 GB。
#>
param(
    [ValidateSet('network', 'csv')]
    [string]$Output,
    [string]$Name,
    [switch]$NoSaveFrame,
    [switch]$Record
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$settings = Get-Content (Join-Path $PSScriptRoot 'pc1.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$cameraConfig = Get-Content (Join-Path $PSScriptRoot 'cameras.json') -Raw -Encoding UTF8 | ConvertFrom-Json
if (-not $Output) { $Output = $settings.output }
$recordVideo = $Record -or $settings.record_video
# 動画は 2 台で 1 時間に約 45 GB (2026-10-16 の実測: 8.5 分で 6.4 GB)
$videoGbPerHour = 45
$remote = Join-Path $PSScriptRoot 'camera-remote.ps1'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)

function Step($text) { Write-Host ""; Write-Host "== $text" -ForegroundColor Cyan }
function Ok($text) { Write-Host "  OK  $text" -ForegroundColor Green }
function Warn($text) { Write-Host "  !!  $text" -ForegroundColor Yellow }
# 止める。try の中から呼んでも finally (記録のまとめ) は走る
function Fail($text) { Write-Host "  NG  $text" -ForegroundColor Red; exit 1 }

# --- 1. 前提を確かめる ---
Step '前提を確かめる'
$tracker = Join-Path $root 'target\release\ball_tracker.exe'
if (-not (Test-Path $tracker)) {
    Fail "ball_tracker がビルドされていません: $tracker (docs\setup_pc1.md の 3 章)"
}
Ok "ball_tracker: $tracker"
$battery = Get-CimInstance Win32_Battery -ErrorAction SilentlyContinue
if ($battery -and $battery.BatteryStatus -eq 1) {
    Warn 'バッテリーで動いています。動画の解析が 120fps に追いつかないことがあるので、電源につないでください'
}
$ffmpeg = (Get-Command ffmpeg -ErrorAction SilentlyContinue).Source
if (-not $ffmpeg) {
    $ffmpeg = Get-ChildItem (Join-Path $env:LOCALAPPDATA 'Microsoft\WinGet\Packages') -Filter ffmpeg.exe -Recurse -ErrorAction SilentlyContinue |
        Select-Object -First 1 -ExpandProperty FullName
}
if (-not $ffmpeg) { Fail 'ffmpeg が見つかりません (winget install --id Gyan.FFmpeg -e)' }
Ok "ffmpeg: $ffmpeg"
$adb = (Get-Command adb -ErrorAction SilentlyContinue).Source
if (-not $adb) {
    $adb = Join-Path $env:LOCALAPPDATA 'Android\Sdk\platform-tools\adb.exe'
    if (-not (Test-Path $adb)) { Fail 'adb が見つかりません (Android SDK の platform-tools)' }
}
$connected = @(& $adb devices | Select-String "`tdevice$" | ForEach-Object { ($_.Line -split "`t")[0] })
$cameras = @($cameraConfig.cameras | Where-Object { $connected -contains $_.serial })
foreach ($camera in $cameraConfig.cameras) {
    if ($connected -contains $camera.serial) { Ok "スマホ $($camera.name): $($camera.serial)" }
    else { Warn "スマホ $($camera.name) ($($camera.serial)) がつながっていません" }
}
if ($cameras.Count -eq 0) { Fail 'cameras.json の端末が 1 台もつながっていません' }

# 記録を置くドライブの空き (GB)
function Get-FreeGb { [math]::Round((New-Object System.IO.DriveInfo((Split-Path -Qualifier $root))).AvailableFreeSpace / 1GB, 1) }
$freeGb = Get-FreeGb
if ($recordVideo) {
    $hours = [math]::Round($freeGb / $videoGbPerHour, 1)
    if ($freeGb -lt 20) { Fail "ディスクの空きが $freeGb GB しかありません。-Record を外すか、recordings の古い動画を移してください" }
    elseif ($hours -lt 3) { Warn "ディスクの空き $freeGb GB: 動画を残すと約 $hours 時間で埋まります" }
    else { Ok "ディスクの空き $freeGb GB (動画を残して約 $hours 時間ぶん)" }
} else {
    if ($freeGb -lt 2) { Fail "ディスクの空きが $freeGb GB しかありません" }
    Ok "ディスクの空き $freeGb GB (動画は残さない。残すときは -Record)"
}

# --- 2. スマホを止めて adb reverse、設定を入れる ---
Step 'スマホのアプリを止め、adb reverse を張り、設定を入れる'
foreach ($camera in $cameras) {
    & $adb -s $camera.serial shell am force-stop $cameraConfig.package | Out-Null
    foreach ($port in 6060, 5050, 5052) {
        & $adb -s $camera.serial reverse "tcp:$port" "tcp:$port" | Out-Null
    }
    Ok "$($camera.name): 6060、5050、5052"
}
# 端末の識別子 (device_id) も cameras.json に記録される。ball_tracker はこれで端末を見分ける
& $remote config | ForEach-Object { Write-Host "  $_" }
$cameraConfig = Get-Content (Join-Path $PSScriptRoot 'cameras.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$cameras = @($cameraConfig.cameras | Where-Object { $connected -contains $_.serial })
foreach ($camera in $cameras) {
    if (-not $camera.device_id) { Warn "$($camera.name) の device_id が分かりません。ball_tracker はこの端末をつないだ順の番号で扱います" }
}

# --- 3. ball_tracker を起動する ---
Step 'ball_tracker を起動する'
$stamp = Get-Date -Format 'yyyy-MM-dd_HHmm'
$dir = Join-Path $root ("recordings\$stamp" + $(if ($Name) { "_$Name" } else { '' }))
New-Item -ItemType Directory -Force $dir | Out-Null
$log = Join-Path $dir 'tracker.log'
$arguments = @('--input', 'network', '--output', $Output, '-j',
    '--cameras', (Join-Path $PSScriptRoot 'cameras.json'),
    '--ffmpeg', $ffmpeg,
    '--video-hwaccel', $settings.video_hwaccel,
    '--report-delay-ms', [string]$settings.report_delay_ms,
    '--time-trim-ms', [string]$settings.time_trim_ms,
    '--rolling-shutter-skew-ms', [string]$settings.rolling_shutter_skew_ms,
    '--detection-center', $settings.detection_center,
    '--edge-detections', $settings.edge_detections,
    '--video-snapshot-dir', (Join-Path $dir 'snapshots'))
if ($settings.auto_time_trim) { $arguments += '--auto-time-trim' }
if ($recordVideo) { $arguments += @('--video-record-dir', (Join-Path $dir 'video')) }
if ($Output -eq 'csv') { $arguments += @('-c', (Join-Path $dir 'collisions_raw.csv')) }
$quoted = ($arguments | ForEach-Object { if ($_ -match '\s') { '"' + $_ + '"' } else { $_ } }) -join ' '
$env:NO_COLOR = '1'
$started = Get-Date
$process = Start-Process -FilePath $tracker -ArgumentList $quoted -NoNewWindow -PassThru `
    -RedirectStandardOutput $log -RedirectStandardError (Join-Path $dir 'tracker_stderr.log')
Ok "記録フォルダー: $dir"

# ログを読み進める。読んだところまでを覚えておき、新しい行だけを返す
$script:logOffset = 0
$script:logRest = ''
$script:pending = New-Object System.Collections.Generic.Queue[string]
function Read-FileLines {
    if (-not (Test-Path $log)) { return @() }
    $stream = [System.IO.File]::Open($log, 'Open', 'Read', 'ReadWrite')
    try {
        if ($stream.Length -le $script:logOffset) { return @() }
        $stream.Seek($script:logOffset, 'Begin') | Out-Null
        $buffer = New-Object byte[] ($stream.Length - $script:logOffset)
        $read = $stream.Read($buffer, 0, $buffer.Length)
        $script:logOffset += $read
    } finally { $stream.Dispose() }
    $text = $script:logRest + [System.Text.Encoding]::UTF8.GetString($buffer, 0, $read)
    $parts = $text -split "`n"
    $script:logRest = $parts[-1]
    return @($parts[0..($parts.Count - 2)] | ForEach-Object { $_.TrimEnd("`r") })
}

# まだ見ていない行をすべて返す (待っていた行より後ろの行も捨てない)
function Read-NewLines {
    foreach ($line in Read-FileLines) { $script:pending.Enqueue($line) }
    $lines = @($script:pending.ToArray())
    $script:pending.Clear()
    return $lines
}

# 見たい行が出るまで待つ。途中の警告は表示する。$TimeoutSec が 0 なら待ち続ける
function Wait-Log([string]$pattern, [int]$TimeoutSec, [string]$waitingText) {
    $deadline = if ($TimeoutSec -gt 0) { (Get-Date).AddSeconds($TimeoutSec) } else { [DateTime]::MaxValue }
    $lastNote = Get-Date
    while ((Get-Date) -lt $deadline) {
        foreach ($line in Read-FileLines) { $script:pending.Enqueue($line) }
        while ($script:pending.Count -gt 0) {
            $line = $script:pending.Dequeue()
            if ($line -match ' WARN | ERROR ') { Warn ($line -replace '^\S+\s+\w+\s+ball_tracker::', '') }
            if ($line -match $pattern) { return $line }
        }
        if ($process.HasExited) { Fail "ball_tracker が終了しました (ログ: $log)" }
        if ($waitingText -and ((Get-Date) - $lastNote).TotalSeconds -ge 15) {
            Write-Host "  ..  $waitingText"
            $lastNote = Get-Date
        }
        Start-Sleep -Milliseconds 300
    }
    return $null
}

try {
    $hw = Wait-Log 'video::hwaccel' 30
    if ($hw) { Ok ($hw -replace '^.*video::hwaccel: ', 'デコード: ' -replace ' available=.*', '') }

    # --- 4. projector のつなぎ先を知らせる ---
    if ($Output -eq 'network') {
        Step 'PC② の projector のつなぎ先'
        if (-not (Wait-Log 'listening for projector' 30)) { Fail 'ball_tracker が projector の待ち受けを始めません' }
        $addresses = (Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.IPAddress -notlike '127.*' -and $_.IPAddress -notlike '169.254*' }).IPAddress -join '、'
        Write-Host "  PC② で scripts\run-pc2.ps1 を実行してください (pc2.json の tracker_host: このPCの名前 $env:COMPUTERNAME、または $addresses)"
        Write-Host '  projector はいつつないでもよい。つながったら下に表示する'
    }
    if (-not (Wait-Log 'starting TCP listener' 30)) { Fail 'ball_tracker がスマホの受け付けを始めません' }
    Ok 'スマホの受け付けを始めた'

    # --- 5. スマホを開く ---
    Step 'スマホを開く'
    $package = $cameraConfig.package
    $activity = "$package/com.taichi765.struckoutCameraApp.MainActivity"
    $ids = @{}
    foreach ($camera in $cameras) {
        # device_id が分かっていれば、camera_id は cameras.json の順番になる
        $index = [array]::IndexOf(@($cameraConfig.cameras | ForEach-Object { $_.serial }), $camera.serial)
        $idPattern = if ($camera.device_id) { "$index\b" } else { '\d+' }
        $id = $null
        foreach ($attempt in 1, 2) {
            & $adb -s $camera.serial shell am start -n $activity --es screen camera | Out-Null
            $line = Wait-Log "received video stream config camera_id=$idPattern" 40 "$($camera.name) を待っています"
            if ($line -and $line -match 'camera_id=(\d+)') { $id = [int]$Matches[1]; break }
            Warn "$($camera.name) から動画が届きません。アプリを開き直します"
            & $adb -s $camera.serial shell am force-stop $package | Out-Null
            Start-Sleep -Seconds 2
        }
        if ($null -eq $id) { Fail "$($camera.name) ($($camera.serial)) がつながりません" }
        $ids[$id] = $camera.name
        Ok "$($camera.name) = camera_id $id"
    }

    # --- 6. 確かめる ---
    Step '届いている動画を確かめる (約 12 秒)'
    $stats = @{}
    $latency = @{}
    $until = (Get-Date).AddSeconds(25)
    while ((Get-Date) -lt $until -and $stats.Count -lt $ids.Count) {
        foreach ($line in Read-NewLines) {
            if ($line -match 'video analysis camera_id=(\d+) fps="([\d.]+)".*delay_ms_p50=(\d+) delay_ms_p95=(\d+) max_delay_ms=(\d+)') {
                $stats[[int]$Matches[1]] = $Matches
            }
            if ($line -match 'camera clock camera_id=(\d+).*min_latency_ms=([\d.]+)') { $latency[[int]$Matches[1]] = [double]$Matches[2] }
            if ($line -match ' WARN ') { Warn ($line -replace '^\S+\s+\w+\s+ball_tracker::', '') }
        }
        Start-Sleep -Milliseconds 500
    }
    foreach ($id in ($ids.Keys | Sort-Object)) {
        $s = $stats[$id]
        if (-not $s) { Warn "camera_id $id ($($ids[$id])): まだ統計が出ていません"; continue }
        $fps = [double]$s[2]
        $text = "camera_id $id ($($ids[$id])): $fps fps、PC での遅れ 中央値 $($s[3]) ms / 95% $($s[4]) ms / 最大 $($s[5]) ms"
        if ($latency.ContainsKey($id)) { $text += "、スマホから PC まで最短 $([math]::Round($latency[$id])) ms" }
        if ($fps -lt 110 -or [int]$s[4] -gt 100) { Warn $text } else { Ok $text }
    }
    & $remote status | ForEach-Object {
        if ($_ -match 'tcp=connected camera_screen=true') { Ok $_ } else { Warn $_ }
    }
    if ($settings.save_frame -and -not $NoSaveFrame) {
        Step 'Save frame を撮って取り込む (向きの較正用。筐体の中に人がいない状態で)'
        & $remote save-frame -Dest (Join-Path $dir 'calibration') | ForEach-Object { Write-Host "  $_" }
    }

    Step '準備完了。投げてください (止めるときは Ctrl+C)'
    $collisions = 0
    $lastStatus = Get-Date
    while (-not $process.HasExited) {
        foreach ($line in Read-NewLines) {
            $short = $line -replace '^\S+\s+\w+\s+ball_tracker::', ''
            if ($line -match 'detected collision') {
                $collisions++
                Write-Host ("  {0}  着弾 {1} 件目" -f (Get-Date -Format 'HH:mm:ss'), $collisions)
            } elseif ($line -match 'accepted TCP connection from projector') {
                Ok ("{0}  projector がつながった" -f (Get-Date -Format 'HH:mm:ss'))
            } elseif ($line -match 'no projector is connected') {
                Warn ("{0}  projector がつながっていないので、着弾を的に送れませんでした" -f (Get-Date -Format 'HH:mm:ss'))
            } elseif ($line -match 'camera connected camera_id=(\d+)') {
                Ok ("{0}  スマホ camera_id {1} がつながった" -f (Get-Date -Format 'HH:mm:ss'), $Matches[1])
            } elseif ($line -match 'automatic time trim') {
                if ($line -match 'trim_ms="([-\d.]+)"') { Write-Host "  ..  右のカメラの時刻の自動補正: $($Matches[1]) ms" }
            } elseif ($line -match ' WARN |disconnected|lost') {
                Warn $short
            } elseif ($line -match 'video analysis camera_id=(\d+) fps="([\d.]+)".*delay_ms_p50=(\d+) delay_ms_p95=(\d+) max_delay_ms=(\d+)') {
                $stats[[int]$Matches[1]] = $Matches
            }
        }
        if (((Get-Date) - $lastStatus).TotalSeconds -ge 60) {
            $lastStatus = Get-Date
            $parts = foreach ($id in ($stats.Keys | Sort-Object)) { "camera_id $id $($stats[$id][2]) fps (遅れ 95% $($stats[$id][4]) ms)" }
            Write-Host ("  ..  {0}  着弾 {1} 件、{2}" -f (Get-Date -Format 'HH:mm:ss'), $collisions, ($parts -join '、'))
            $freeGb = Get-FreeGb
            if ($recordVideo -and $freeGb -lt 20) {
                Warn "ディスクの空きが $freeGb GB です (動画で 1 時間に約 $videoGbPerHour GB 減る)。Ctrl+C で止め、-Record なしで起動し直してください"
            } elseif ($freeGb -lt 2) {
                Warn "ディスクの空きが $freeGb GB です"
            }
        }
        Start-Sleep -Milliseconds 300
    }
    Warn "ball_tracker が終了しました (ログ: $log)"
} finally {
    if (-not $process.HasExited) {
        Stop-Process -Id $process.Id -ErrorAction SilentlyContinue
        $process.WaitForExit(5000) | Out-Null
    }
    # この回の -j の記録を写す
    Get-ChildItem (Join-Path $env:APPDATA 'struckout\tracker\json') -ErrorAction SilentlyContinue |
        Where-Object { $_.LastWriteTime -ge $started -and $_.Name -match '^(events|frames)_' } |
        ForEach-Object { Copy-Item $_.FullName (Join-Path $dir $_.Name) -Force }
    # 使い終わったスマホは、熱を持たないようにアプリを止める
    foreach ($camera in $cameras) { & $adb -s $camera.serial shell am force-stop $cameraConfig.package | Out-Null }
    Write-Host ""
    Write-Host "記録: $dir"
}
