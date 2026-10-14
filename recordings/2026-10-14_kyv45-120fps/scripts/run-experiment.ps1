# ball_tracker を動かしながら、つながっている全端末の画面を録画する。Ctrl+C で止めると、
# 録画を閉じてから、動画・ログ・JSON・logcat を回ごとのフォルダに集める。
Set-Location 'C:\Users\tsuka\Documents\GitHub\struckout'
$recorder = Join-Path $PSScriptRoot 'record-screen.ps1'
$runId = Get-Date -Format 'yyyyMMdd_HHmmss'
$runDir = Join-Path $env:APPDATA "struckout\tracker\runs\$runId"
$videoDir = Join-Path $runDir 'video'
$stopFile = Join-Path $runDir 'stop'
New-Item -ItemType Directory -Force $videoDir | Out-Null

$serials = @(adb devices | Select-String "`tdevice$" | ForEach-Object { ($_.Line -split "`t")[0] })
Write-Host "run: $runDir"
Write-Host "devices: $($serials -join ', ')"

# 録画は別のコンソールで動かす。同じコンソールだと Ctrl+C が adb にも届き、動画が閉じられずに壊れる。
$recorders = foreach ($serial in $serials) {
    adb -s $serial shell input keyevent KEYCODE_WAKEUP
    Start-Process powershell -WindowStyle Hidden -PassThru -ArgumentList @(
        '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $recorder,
        $serial, $runId, (Join-Path $videoDir "segments_$serial.csv"), $stopFile)
}
$startedAt = Get-Date
$trackerLog = Join-Path $runDir 'tracker.log'

try {
    & '.\target\release\ball_tracker.exe' --input network --output csv -j -c (Join-Path $runDir 'collisions.csv') |
        Tee-Object -FilePath $trackerLog
}
finally {
    Write-Host 'stopping screen recordings...'
    # 先に止める合図を出してから録画を止める。逆だと、次の録画が始まってしまう。
    New-Item -ItemType File -Force $stopFile | Out-Null
    foreach ($serial in $serials) {
        # SIGINT で止めると、screenrecord は動画を閉じてから終わる。
        adb -s $serial shell 'kill -2 $(pidof screenrecord)' 2>$null
    }
    foreach ($process in $recorders) {
        if (-not $process.WaitForExit(10000)) {
            Write-Host "recorder $($process.Id) did not exit; killing it"
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        }
    }
    Remove-Item -Force $stopFile -ErrorAction SilentlyContinue

    foreach ($serial in $serials) {
        $files = @(adb -s $serial shell 'ls /sdcard/struckout_rec/' | ForEach-Object { $_.Trim() } |
            Where-Object { $_ -like "${runId}_*" })
        foreach ($file in $files) {
            adb -s $serial pull "/sdcard/struckout_rec/$file" $videoDir | Out-Null
        }
        Write-Host "$serial : pulled $($files.Count) video file(s)"
        adb -s $serial logcat -d -v threadtime | Out-File -Encoding utf8 (Join-Path $runDir "logcat_$serial.txt")
    }

    $jsonDir = Join-Path $env:APPDATA 'struckout\tracker\json'
    Get-ChildItem $jsonDir | Where-Object { $_.CreationTime -ge $startedAt.AddSeconds(-10) } |
        Copy-Item -Destination $runDir

    # Tee-Object は UTF-16 で書くので、色の制御文字を除いた UTF-8 に直す。
    if (Test-Path $trackerLog) {
        $text = (Get-Content -Raw $trackerLog) -replace "\x1b\[[0-9;]*m", ''
        [IO.File]::WriteAllText($trackerLog, $text, (New-Object Text.UTF8Encoding $false))
    }
    Write-Host "saved to $runDir"
}
