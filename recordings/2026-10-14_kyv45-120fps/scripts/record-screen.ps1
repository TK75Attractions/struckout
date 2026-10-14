# 1 台の画面を 180 秒ずつ録画し続ける (screenrecord の上限が 180 秒のため)。
# 端末が抜けたら、つながり直すのを待って adb reverse をかけ直し、録画を再開する。
# StopFile ができたら、次の録画を始めずに終わる。
param(
    [string]$Serial,
    [string]$RunId,
    [string]$SegmentLog,
    [string]$StopFile
)

$index = 0
while (-not (Test-Path $StopFile)) {
    adb -s $Serial wait-for-device
    if (Test-Path $StopFile) { break }
    adb -s $Serial reverse tcp:6060 tcp:6060 | Out-Null
    adb -s $Serial reverse tcp:5050 tcp:5050 | Out-Null
    adb -s $Serial shell mkdir -p /sdcard/struckout_rec
    $index++
    $name = '{0}_{1}_{2:D3}.mp4' -f $RunId, $Serial, $index
    $startedAt = Get-Date
    # 録画を始める直前の PC の時刻。動画と ball_tracker のログを突き合わせる目安 (誤差は数百 ms)。
    '{0},{1}' -f $startedAt.ToUniversalTime().ToString('o'), $name | Add-Content -Encoding UTF8 $SegmentLog
    adb -s $Serial shell screenrecord --time-limit 180 --bit-rate 4000000 "/sdcard/struckout_rec/$name"
    if (((Get-Date) - $startedAt).TotalSeconds -lt 2) {
        # すぐ失敗したとき (画面が消えている、端末が抜けた) に、空回りしないようにする。
        Start-Sleep -Seconds 2
    }
}