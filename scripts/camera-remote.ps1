<#
.SYNOPSIS
カメラ用スマホのアプリ (高速撮影版) を、画面に触れずに PC から adb で操作する。

.DESCRIPTION
    scripts\camera-remote.ps1 config       # cameras.json の設定 (出力先、焦点距離、位置と向き) を入れ、端末の識別子を記録する
    scripts\camera-remote.ps1 open         # アプリをカメラ画面で開く
    scripts\camera-remote.ps1 status       # 各端末の設定と接続の状態
    scripts\camera-remote.ps1 save-frame   # 全端末で「Save frame」し、画像を PC に取り込む
    scripts\camera-remote.ps1 retry        # ball_tracker への接続をやり直す
    scripts\camera-remote.ps1 stop         # アプリを止める

config は、端末が返す識別子 (device_id) を cameras.json に書き込む。ball_tracker を --cameras cameras.json で起動すると、
これで端末を見分け、つないだ順によらず cameras.json の順の camera_id を振り、位置と焦点距離を端末に送る。
そのため、アプリと ball_tracker のどちらを先に起動してもよく、開く順番も問わない。
端末ごとに 1 回 (アプリを別の PC で署名し直したときも) config を実行しておけばよい。
実行ポリシーで止められるときは powershell -ExecutionPolicy Bypass -File scripts\camera-remote.ps1 <命令>。

.PARAMETER Settings
端末ごとの設定。既定は scripts\cameras.json。

.PARAMETER Wait
open で、1 台開いてから次を開くまでの秒数 (2 台同時に高速撮影を始めると重くなるので、少しあける)。

.PARAMETER Dest
save-frame で画像を取り込むフォルダー。端末のシリアルごとのフォルダーを作る。既定は recordings\calibration。
#>
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidateSet('config', 'open', 'status', 'save-frame', 'retry', 'stop')]
    [string]$Command,
    [string]$Settings,
    [int]$Wait = 3,
    [string]$Dest
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if (-not $Settings) { $Settings = Join-Path $PSScriptRoot 'cameras.json' }
if (-not $Dest) { $Dest = Join-Path $root 'recordings\calibration' }
$config = Get-Content $Settings -Raw -Encoding UTF8 | ConvertFrom-Json
$package = $config.package
$receiver = "$package/com.taichi765.struckoutCameraApp.remote.RemoteControlReceiver"
$activity = "$package/com.taichi765.struckoutCameraApp.MainActivity"
$prefix = 'com.taichi765.struckoutCameraApp.remote.'

$adb = (Get-Command adb -ErrorAction SilentlyContinue).Source
if (-not $adb) {
    $adb = Join-Path $env:LOCALAPPDATA 'Android\Sdk\platform-tools\adb.exe'
    if (-not (Test-Path $adb)) { throw 'adb が見つかりません' }
}
$connected = & $adb devices | Select-String "`tdevice$" | ForEach-Object { ($_.Line -split "`t")[0] }

# cameras.json に書いた端末のうち、つながっているもの (書いた順)
$cameras = @($config.cameras | Where-Object { $connected -contains $_.serial })
foreach ($camera in $config.cameras) {
    if ($connected -notcontains $camera.serial) {
        Write-Warning "$($camera.name) ($($camera.serial)) がつながっていません"
    }
}
foreach ($serial in $connected) {
    if (-not ($config.cameras | Where-Object { $_.serial -eq $serial })) {
        Write-Warning "$serial は $Settings に書かれていないので操作しません"
    }
}

function Send-Command($camera, $action, [string[]]$extras = @()) {
    # -f 32 (FLAG_INCLUDE_STOPPED_PACKAGES): アプリを止めたあとでも届くようにする
    $out = & $adb -s $camera.serial shell am broadcast -f 32 -a "$prefix$action" -n $receiver @extras 2>&1 | Out-String
    # 結果は「Broadcast completed: result=0, data="..."」の data
    if ($out -match 'data="(.*)"') { $result = $Matches[1] } else { $result = $out.Trim() }
    "{0,-6} {1}: {2}" -f $camera.name, $camera.serial, $result
}

# cameras.json の、シリアルが $serial の端末に device_id を書く。書式を崩さないよう、行を直接書き換える
function Save-DeviceId($serial, $deviceId) {
    $text = [System.IO.File]::ReadAllText($Settings)
    $pattern = '(?m)^(?<indent>[ \t]*)"serial"\s*:\s*"' + [regex]::Escape($serial) + '",[ \t]*\r?\n(?:[ \t]*"device_id"\s*:\s*"[^"]*",[ \t]*\r?\n)?'
    $match = [regex]::Match($text, $pattern)
    if (-not $match.Success) { Write-Warning "$Settings に $serial の serial の行が見つからないので、device_id を書けません"; return }
    $newline = if ($text.Contains("`r`n")) { "`r`n" } else { "`n" }
    $indent = $match.Groups['indent'].Value
    $replacement = "$indent""serial"": ""$serial"",$newline$indent""device_id"": ""$deviceId"",$newline"
    if ($match.Value -eq $replacement) { return }
    $text = $text.Substring(0, $match.Index) + $replacement + $text.Substring($match.Index + $match.Length)
    [System.IO.File]::WriteAllText($Settings, $text, (New-Object System.Text.UTF8Encoding($false)))
    Write-Host "  $serial の device_id ($deviceId) を $Settings に書きました"
}

switch ($Command) {
    'config' {
        foreach ($camera in $cameras) {
            $extras = @('--es', 'output', $camera.output,
                '--es', 'focal_ratio', ([string]$camera.focal_ratio),
                '--es', 'pose', (($camera.pose | ForEach-Object { [string]$_ }) -join ','))
            if ($camera.area) { $extras += @('--es', 'area', (($camera.area | ForEach-Object { [string]$_ }) -join ',')) }
            $line = Send-Command $camera 'SET_CONFIG' $extras
            $line
            if ($line -match 'device_id=(\S+)') { Save-DeviceId $camera.serial $Matches[1] }
            else { Write-Warning "$($camera.name) が device_id を返しません (アプリが古い)。ball_tracker はこの端末を見分けられません" }
        }
    }
    'open' {
        for ($i = 0; $i -lt $cameras.Count; $i++) {
            $camera = $cameras[$i]
            & $adb -s $camera.serial shell am start -n $activity --es screen camera | Out-Null
            Write-Host "$($camera.name) ($($camera.serial)) を開きました"
            if ($i -lt $cameras.Count - 1) {
                Write-Host "  $Wait 秒待ちます"
                Start-Sleep -Seconds $Wait
            }
        }
    }
    'status' {
        foreach ($camera in $cameras) { Send-Command $camera 'STATUS' }
    }
    'retry' {
        foreach ($camera in $cameras) { Send-Command $camera 'RETRY_CONNECTION' }
    }
    'stop' {
        foreach ($camera in $cameras) {
            & $adb -s $camera.serial shell am force-stop $package
            "{0,-6} {1}: stopped" -f $camera.name, $camera.serial
        }
    }
    'save-frame' {
        foreach ($camera in $cameras) { Send-Command $camera 'SAVE_FRAME' }
        # PC が解析しているあいだ、端末は 1 秒に 1 枚しか画像を作らないので、少し待ってから取り込む
        Start-Sleep -Seconds 3
        foreach ($camera in $cameras) {
            $remote = "/sdcard/Android/data/$package/files/calibration"
            $local = Join-Path $Dest $camera.serial
            New-Item -ItemType Directory -Force $local | Out-Null
            & $adb -s $camera.serial pull "$remote/." $local | Out-Null
            $latest = Get-ChildItem $local -Filter 'frame_*.png' | Sort-Object Name | Select-Object -Last 1
            "{0,-6} {1}: {2}" -f $camera.name, $camera.serial, $(if ($latest) { $latest.FullName } else { '画像がありません' })
        }
    }
}
