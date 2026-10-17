<#
.SYNOPSIS
PC② をひとまとめに起動する: game_master (Docker) と、書き出した projector をプロジェクターの画面に全画面で。

.DESCRIPTION
    powershell -ExecutionPolicy Bypass -File scripts\run-pc2.ps1                       # 起動
    powershell -ExecutionPolicy Bypass -File scripts\run-pc2.ps1 -TrackerHost PC1-NAME # つなぎ先を指定して起動
    powershell -ExecutionPolicy Bypass -File scripts\run-pc2.ps1 -Build                # projector を書き出し直してから起動
    powershell -ExecutionPolicy Bypass -File scripts\run-pc2.ps1 -Stop                 # projector と game_master を止める

PC① の scripts\run-pc1.ps1 と、どちらを先に実行してもよい。
projector は ball_tracker と game_master の両方につながらないと動き出さない。

1. Docker Desktop を起動し (止まっていれば)、game_master とデータベースを起動する
2. projector の書き出し (projector\Builds\Windows) が無いか -Build なら、Unity でコマンドから書き出す
   (Unity Editor でプロジェクトを開いていると書き出せない)
3. projector を、scripts\pc2.json の設定 (つなぎ先、画面の番号、着弾位置の係数) で起動する
4. projector のログを見て、ball_tracker につながったら知らせる (PC① と PC② のどちらを先に起動してもよい)

projector は、ball_tracker が後から起動しても、途中で再起動しても、自分でつなぎ直す。

.PARAMETER TrackerHost
pc2.json の tracker_host (PC① の名前か IP) を上書きする。

.PARAMETER Build
projector を書き出し直す。

.PARAMETER Windowed
全画面にしない (動作の確かめ用)。

.PARAMETER Stop
projector と game_master を止める。
#>
param(
    [string]$TrackerHost,
    [switch]$Build,
    [switch]$Windowed,
    [switch]$Stop
)

# docker や Unity がエラー出力に書いただけで止まらないよう、失敗は終了コードとログで判断する (PowerShell 5.1)
$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot
$settings = Get-Content (Join-Path $PSScriptRoot 'pc2.json') -Raw -Encoding UTF8 | ConvertFrom-Json
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)

function Step($text) { Write-Host ""; Write-Host "== $text" -ForegroundColor Cyan }
function Ok($text) { Write-Host "  OK  $text" -ForegroundColor Green }
function Warn($text) { Write-Host "  !!  $text" -ForegroundColor Yellow }
function Fail($text) { Write-Host "  NG  $text" -ForegroundColor Red; exit 1 }

$exe = Join-Path $root $settings.projector_exe
$compose = @('compose', '-f', (Join-Path $root 'game_master\compose.yaml'), '-f', (Join-Path $root 'game_master\compose.dev.yaml'))

# データベースのパスワード。mise の MYSQL_ROOT_PASSWORD_DEV (.mise/config.local.toml、`mise run game-master:dev` と同じ) を使う。
# 環境変数に無ければ .mise/config.local.toml から読み、それも無ければ前の game_master/.env.dev を使う。値は表示しない
function Set-DatabasePassword {
    if ($env:MYSQL_ROOT_PASSWORD_DEV) { $env:MYSQL_ROOT_PASSWORD = $env:MYSQL_ROOT_PASSWORD_DEV; return $true }
    $local = Join-Path $root '.mise\config.local.toml'
    if (Test-Path $local) {
        $line = Get-Content $local -Encoding UTF8 | Where-Object { $_ -match '^\s*MYSQL_ROOT_PASSWORD_DEV\s*=\s*"([^"]*)"' } | Select-Object -First 1
        if ($line -and $line -match '^\s*MYSQL_ROOT_PASSWORD_DEV\s*=\s*"([^"]*)"') { $env:MYSQL_ROOT_PASSWORD = $Matches[1]; return $true }
    }
    $envFile = Join-Path $root 'game_master\.env.dev'
    if (Test-Path $envFile) { $script:compose += @('--env-file', $envFile); return $true }
    return $false
}

if ($Stop) {
    Step '止める'
    Get-Process ([System.IO.Path]::GetFileNameWithoutExtension($exe)) -ErrorAction SilentlyContinue | Stop-Process
    Ok 'projector'
    if ($settings.game_master) { & docker @compose stop 2>&1 | Out-Null; Ok 'game_master' }
    exit 0
}

if (-not $TrackerHost) { $TrackerHost = $settings.tracker_host }
if (-not $TrackerHost) {
    Fail 'つなぎ先 (PC①) が決まっていません。-TrackerHost <PC① の名前か IP> を付けるか、scripts\pc2.json の tracker_host に書いてください'
}

# --- 1. game_master ---
if ($settings.game_master) {
    Step 'game_master を起動する (Docker)'
    $docker = (Get-Command docker -ErrorAction SilentlyContinue).Source
    if (-not $docker) { Fail 'docker が見つかりません。Docker Desktop を入れてください' }
    & docker info 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        $desktop = Join-Path $env:ProgramFiles 'Docker\Docker\Docker Desktop.exe'
        if (-not (Test-Path $desktop)) { Fail 'Docker Desktop が見つかりません' }
        Write-Host '  ..  Docker Desktop を起動しています (1〜2 分)'
        Start-Process $desktop
        $deadline = (Get-Date).AddMinutes(3)
        $relaunch = (Get-Date).AddSeconds(30)
        do {
            Start-Sleep -Seconds 3
            # 起動した Docker Desktop がすぐ終わってしまうことがあるので、30 秒たっても居なければ 1 回だけ起動し直す
            if ($relaunch -and (Get-Date) -gt $relaunch) {
                if (-not (Get-Process 'Docker Desktop' -ErrorAction SilentlyContinue)) { Start-Process $desktop }
                $relaunch = $null
            }
            & docker info 2>&1 | Out-Null
        } until ($LASTEXITCODE -eq 0 -or (Get-Date) -gt $deadline)
        if ($LASTEXITCODE -ne 0) { Fail 'Docker が起動しません。Docker Desktop を手で起動してからやり直してください' }
    }
    Ok 'Docker'
    if (-not (Set-DatabasePassword)) {
        Fail 'データベースのパスワードがありません。.mise/config.local.toml の [env] に MYSQL_ROOT_PASSWORD_DEV を書いてください (docs/setup_pc2.md)'
    }
    # 2026-10-17 までの game_master は compose のプロジェクト名が game_master だった。残っていると 8020 番がぶつかるので止める
    $old = @(& docker ps -q --filter 'name=^game_master-' 2>$null)
    if ($old.Count -gt 0) {
        Warn '前の名前 (game_master) で動いている game_master を止めます'
        & docker stop $old 2>&1 | Out-Null
    }
    Write-Host '  ..  game_master を起動しています (初回はビルドに数分から十数分かかる)'
    & docker @compose up -d 2>&1 | ForEach-Object { "$_" } | Where-Object { $_ -match 'error|Error|failed' } | ForEach-Object { Warn $_ }
    $deadline = (Get-Date).AddMinutes(2)
    $ready = $false
    while (-not $ready -and (Get-Date) -lt $deadline) {
        $client = New-Object System.Net.Sockets.TcpClient
        try { $client.Connect('127.0.0.1', 8020); $ready = $true } catch { Start-Sleep -Seconds 2 } finally { $client.Dispose() }
    }
    if (-not $ready) { Fail 'game_master が 8020 番で待ち受けません (docker compose logs で確かめる)' }
    Ok 'game_master: 127.0.0.1:8020'
}

# --- 2. projector を書き出す ---
if ($Build -or -not (Test-Path $exe)) {
    Step 'projector を書き出す (数分)'
    $project = Join-Path $root 'projector'
    if (Test-Path (Join-Path $project 'Temp\UnityLockfile')) {
        $locked = $true
        try { [System.IO.File]::Open((Join-Path $project 'Temp\UnityLockfile'), 'Open', 'ReadWrite', 'None').Dispose(); $locked = $false } catch {}
        if ($locked) { Fail 'Unity Editor で projector を開いていると書き出せません。閉じてからやり直してください' }
    }
    $unity = $settings.unity_editor
    if (-not (Test-Path $unity)) { Fail "Unity が見つかりません: $unity (pc2.json の unity_editor)" }
    New-Item -ItemType Directory -Force (Split-Path $exe) | Out-Null
    $buildLog = Join-Path (Split-Path $exe) 'build.log'
    $p = Start-Process -FilePath $unity -Wait -PassThru -ArgumentList @(
        '-batchmode', '-quit', '-projectPath', "`"$project`"", '-buildTarget', 'Win64',
        '-buildWindows64Player', "`"$exe`"", '-logFile', "`"$buildLog`"")
    if ($p.ExitCode -ne 0 -or -not (Test-Path $exe)) { Fail "書き出しに失敗しました (ログ: $buildLog)" }
    Ok "書き出した: $exe"
}

# --- 3. projector を起動する ---
try {
    $addresses = [System.Net.Dns]::GetHostAddresses($TrackerHost) | Where-Object { $_.AddressFamily -eq 'InterNetwork' }
    Ok "つなぎ先: $TrackerHost ($($addresses.IPAddressToString -join ', '))"
} catch {
    Warn "つなぎ先 $TrackerHost の名前が引けません。IP で指定してみてください"
}
$running = Get-Process ([System.IO.Path]::GetFileNameWithoutExtension($exe)) -ErrorAction SilentlyContinue
if ($running) { Warn 'projector がすでに動いていたので止めます'; $running | Stop-Process; Start-Sleep -Seconds 1 }
$logDir = Join-Path $root ("recordings\" + (Get-Date -Format 'yyyy-MM-dd') + '_projector')
New-Item -ItemType Directory -Force $logDir | Out-Null

# projector が書いている最中のログを読む (Get-Content だと開けないことがある)
function Read-Log($path) {
    try {
        $stream = [System.IO.File]::Open($path, 'Open', 'Read', 'ReadWrite')
        try { return (New-Object System.IO.StreamReader($stream, [System.Text.Encoding]::UTF8)).ReadToEnd() } finally { $stream.Dispose() }
    } catch { return '' }
}

Step 'projector を起動する'
$log = Join-Path $logDir ("projector_" + (Get-Date -Format 'HHmmss') + '.log')
# -connectAttempts を大きくして、ball_tracker が起動するまで待ち続けてもらう (間隔は 5 秒まで)
$arguments = @('-trackerHost', $TrackerHost, '-masterHost', $settings.master_host, '-machineId', [string]$settings.machine_id,
    '-connectAttempts', '100000', '-monitor', [string]$settings.monitor, '-logFile', "`"$log`"")
if ($Windowed -or -not $settings.fullscreen) { $arguments += @('-screen-fullscreen', '0') } else { $arguments += @('-screen-fullscreen', '1') }
if ($settings.collision_transform) {
    $arguments += @('-collisionTransform', "`"$(Join-Path $root $settings.collision_transform)`"")
}
$projector = Start-Process -FilePath $exe -ArgumentList $arguments -PassThru
Ok "projector を起動した (ログ: $log)"

Step 'PC① の ball_tracker につながるのを待つ (Ctrl+C でやめても projector は待ち続ける)'
$deadline = (Get-Date).AddMinutes(10)
$lastNote = Get-Date
$connected = $false
while (-not $connected -and (Get-Date) -lt $deadline) {
    Start-Sleep -Seconds 1
    if ($projector.HasExited) { Fail "projector が終了しました (ログ: $log)" }
    $text = Read-Log $log
    if ($text -match 'Master connect failed') { Fail 'game_master につながりません (docker compose ps で確かめる)' }
    if ($text -match 'Connected to TCP server') {
        # ball_tracker の次に game_master へつなぐので、少し待ってから決める
        Start-Sleep -Seconds 5
        if ((Read-Log $log) -match 'Master connect failed') { Fail 'game_master につながりません (docker compose ps で確かめる)' }
        $connected = $true
    } elseif (((Get-Date) - $lastNote).TotalSeconds -ge 30) {
        Write-Host '  ..  ball_tracker を待っています (PC① で run-pc1.ps1 を実行する)'
        $lastNote = Get-Date
    }
}
if ($connected) {
    Ok 'ball_tracker と game_master につながった'
} else {
    Warn 'ball_tracker にまだつながりません。projector は待ち続けます。つなぎ先 (pc2.json の tracker_host) と PC① のファイアウォールを確かめてください'
}
Write-Host ""
Write-Host "止めるとき: powershell -ExecutionPolicy Bypass -File scripts\run-pc2.ps1 -Stop"
