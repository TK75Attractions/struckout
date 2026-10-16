# PC① のセットアップ手順 (ball_tracker + カメラ)

`docs/architecture.mmd` の `pc1_1` にあたる PC を、何も入っていない Windows から
実機で動かせる状態にするまでの手順。

PC① が受け持つのは次の 2 つ。

- **ball_tracker** を動かす (カメラ 2 台の検知結果から衝突点を出し、projector に送る)
- **カメラ用スマホ 2 台**を USB でつなぎ、アプリの導入と `adb reverse` を行う

| 向き | 相手 | ポート | 経路 |
|---|---|---|---|
| 待ち受け | カメラ (制御・姿勢) | 6060 | USB (`adb reverse`) |
| 待ち受け | カメラ (検知データ) | 5050 | USB (`adb reverse`) |
| 待ち受け | カメラ (動画。PC で解析するとき) | 5052 | USB (`adb reverse`) |
| 待ち受け | projector (PC②) | 5000 | LAN |

ポートの出どころは `api/spec/servers.yaml`。game_master とは直接つながらない。

> **この手順書の検証範囲**
> 2026-10-05 に ARM64 の Windows 11 で、mise の導入、Rust / protoc の導入、
> ball_tracker のビルドと単体起動 (5050 / 6060 の待ち受け) までを実際に確かめた。
> **Android Studio・adb・カメラアプリ・実機カメラを使う 4 章以降は未検証**で、
> コードと設定ファイルから組み立てている。食い違いがあれば直してほしい。
>
> 2026-10-13 に、ball_tracker の作り直し (`docs/ball_detection_plan_*.md`) に合わせて
> 3・8〜11 章を書き直した (コミット `f0a4499` 時点のコードを読んで確かめた。`cargo test -p ball_tracker` は通る)。
> 時刻合わせ・較正・新しい追跡も実機では動かしていない。

## 0. 用意するもの

- Windows 10 / 11 の PC (管理者権限が要る)
- Android 9 以上のスマホ 2 台 (`minSdk = 28`。KYV45 は 1 台が Android 9、もう 1 台が Android 10) と、
  データ通信できる USB ケーブル 2 本
- PC② と同じ LAN。PC① の IP アドレスを PC② 側の設定に使うので固定しておく
- 空き容量は 15 GB を目安に (Visual Studio Build Tools、Android SDK、Rust のビルド成果物。実測ではない)

リポジトリは**日本語を含まないパス**に置く
(`C:\Users\<名前>\ドキュメント\...` は避ける。`docs/development_environment.md` 参照)。

## 1. ツールを入れる

PowerShell で順に実行する。winget が無ければ Microsoft Store の「アプリ インストーラー」を入れる。

### 1.1 Git と mise

```powershell
winget install --id Git.Git -e
winget install --id jdx.mise -e
```

入れたあと**ターミナルを開き直す** (PATH が反映されない)。

### 1.2 C++ ビルドツールと Windows SDK

Rust (MSVC) のリンクに要る。Visual Studio が既に入っていても
**Windows SDK が入っていないと失敗する**ので、下で確認すること。

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--passive --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

ARM64 の PC では末尾に `--add Microsoft.VisualStudio.Component.VC.Tools.ARM64` を足す
(x64 用のツールだけでは ARM64 向けにリンクできない)。

確認:

```powershell
Test-Path "C:\Program Files (x86)\Windows Kits\10\Lib"   # True なら Windows SDK あり
```

Visual Studio 2022 が既にある場合は、Visual Studio Installer の「変更」から
「C++ によるデスクトップ開発」と「Windows 11 SDK」を足しても同じ。

### 1.3 Android Studio

```powershell
winget install --id Google.AndroidStudio -e
```

初回起動のウィザードで Android SDK を入れる (既定のままでよい)。
`adb` はここに入るので PATH に足しておく。

> **ARM64 の Windows では、ウィザードが `An error occurred while trying to compute required packages.` で止まる。**
> ウィザードは Android エミュレーターを必ず入れようとするが、エミュレーターは ARM64 の Windows 向けに
> 提供されていないため (ログに `Dependant package with key emulator not found!`)。2026-10-15 に Android Studio
> 2026.2.1 で確認した。ウィザードは Cancel で閉じ、Welcome 画面の「More Actions > SDK Manager」で、次だけを入れる。
>
> - SDK Platforms: アプリの `compileSdk` と同じ版 (いまは Android 17、API 37)
> - SDK Tools: Android SDK Build-Tools、Android SDK Platform-Tools
>
> SDK の場所 (Android SDK Location) が空なら `C:\Users\<名前>\AppData\Local\Android\Sdk` を指定する。
> adb などは x64 版で、ARM64 の Windows でもそのまま動く。

```powershell
$adb = "$env:LOCALAPPDATA\Android\Sdk\platform-tools"
[Environment]::SetEnvironmentVariable('Path', [Environment]::GetEnvironmentVariable('Path','User') + ";$adb", 'User')
```

ターミナルを開き直して `adb version` が通れば OK。

### 1.4 ffmpeg

高速撮影版のアプリは、動画を PC① に送って解析を任せる (KYV45 は端末での解析が 120fps に追いつかないため)。
ball_tracker は動画のデコードに ffmpeg を使う。

```powershell
winget install --id Gyan.FFmpeg -e
```

入れたあと**ターミナルを開き直し**、`ffmpeg -version` が動けばよい。PATH に入らないときは、ball_tracker の
`--ffmpeg` に `ffmpeg.exe` の場所を渡す (winget なら
`%LOCALAPPDATA%\Microsoft\WinGet\Packages\Gyan.FFmpeg_*\ffmpeg-*\bin\ffmpeg.exe`)。
ARM64 の Windows では x64 版がエミュレーションで動く。2026-10-15 に ARM64 の PC (12 コア) で測ると、
1280x720 のデコードと縮小が 2 本同時で 1 本あたり約 420fps、解析が 1 枚 3.8 ms (約 260fps) で、
2 台ぶんの 120fps に間に合う。別の PC を使うときは 9 章の `video analysis` で確かめる。

## 2. リポジトリを取得する

```powershell
git clone https://github.com/TK75Attractions/struckout.git
cd struckout
mise trust
mise install rust protoc java
```

既定ブランチは `master` (以前の `main` は改名された)。

`mise install` を引数なしで実行しない。`.mise/config.toml` には PC① に不要なツールも
並んでおり、dotnet は Windows で失敗し、`cargo:*` はソースからのビルドで長くかかる。

| ツール | 用途 |
|---|---|
| rust | ball_tracker のビルド |
| protoc | `.proto` からのコード生成 (Rust と Android の両方が PATH から拾う) |
| java (Temurin 21) | カメラアプリを Gradle でビルドするとき |

## 3. ball_tracker をビルドする

リポジトリのルートで:

```powershell
$env:SQLX_OFFLINE = 'true'
mise exec rust protoc -- cargo build --release --locked -p ball_tracker
```

初回は依存クレートの取得で時間がかかる。成功すると
`target\release\ball_tracker.exe` ができる。

単体で起動できるか確かめる (projector もカメラも要らない):

```powershell
.\target\release\ball_tracker.exe --input network --output csv
```

次のログが出て止まらずに待ち続ければよい。Ctrl+C で終了する。

```
succeed to bind port for `DataTransport`
succeed to bind port for `CameraLocationListener`
succeed to bind port for `VideoTransport`
starting TCP listener
```

初回起動時に Windows のファイアウォールの確認が出たら「許可」する。

| オプション | 意味 |
|---|---|
| `--input network` | カメラから TCP で受け取る (これしか選べない) |
| `--output network` | projector に TCP で送る。**projector が 5000 につないでくるまで先へ進まない** |
| `--output csv` | 衝突点を CSV に書く。projector 無しで試すとき用。既定の出力先は `%APPDATA%\struckout\tracker\csv\`。**改行を書かないので、複数の着弾は 1 行につながる** |
| `-j` | 追跡の詳細を JSON で残す。出力先は `%APPDATA%\struckout\tracker\json\`。`events_*` (トラックの作成・更新・破棄、着弾の報告、見送った報告、時刻合わせの手がかり) と `frames_*` (受け取ったフレーム) ができる |
| `--time-trim-ms <ms>` | 右のカメラ (登録した位置の y が大きいほう) の時刻に足す補正。既定 0。`scripts/recording_analysis/estimate_time_offset.py` で求める |
| `--rolling-shutter-skew-ms <ms>` | 画像の上端と下端の露光開始の時刻の差。指定するとカメラが知らせた値の代わりに使う。既定ではカメラが知らせた値 |
| `--ffmpeg <path>` | カメラから受け取った動画のデコードに使う ffmpeg。既定は `ffmpeg` (PATH から探す) |
| `--no-video-analysis` | 動画を受け付けない (5052 番で待ち受けない)。カメラは自分で解析して検出を送る |
| `--video-snapshot-dir <dir>` | カメラごとに、PC で解析し始めた最初の画像を BMP で保存する。向きの確認用 (9 章) |
| `--video-record-dir <dir>` | カメラから受け取った動画を保存する。`replay_video` で解析と追跡をやり直せる (`scripts/start-pc1.ps1` は既定で保存する) |
| `--report-delay-ms <ms>` | 着弾を、予測した着弾時刻の何 ms 後に報告するか (負なら前、既定 -30)。撮影から届くまでの遅れ以上にすると、着弾の直前の視線を使えて精度が上がる。描画はそのぶん遅れる (`recordings/2026-10-16_experiment/notes.md`) |
| `--auto-time-trim` | 右のカメラの時刻の補正を、報告した投球 (直近 9 球の中央値) から自動で決めて `--time-trim-ms` に足す。付けないときは決めた値をログに出すだけ。カメラの向きを較正してから使う (向きがずれていると、その誤差まで時刻で吸収する) |
| `--video-hwaccel <方式>` | 動画のデコードに使う GPU。既定の `auto` は、その PC で使えるものを選ぶ (Windows は `d3d11va`→`cuda`→`qsv`→`dxva2`)。`none` で CPU。選んだ方式は起動時のログ (`decoding video on the GPU`) に出る。検出の結果は CPU と同じ。**電源につないだ状態での遅れはまだ確かめていない**。おかしければ `none` に戻す |
| `--detection-center <box|centroid>` | PC で解析するとき、視線を作る検出の位置。`box` は枠の中心を整数に切り捨てる (カメラアプリと同じ、既定)、`centroid` は前景の画素の重心。2026-10-16 の録画で centroid のほうが少しよかった (1.66 → 1.52 cm) |
| `--edge-detections <keep|drop|extrapolate>` | PC で解析するとき、画像の端に接した検出 (ボールが欠けている) の扱い。`keep` はそのまま (既定)、`drop` は捨てる (報告できない球が出る)、`extrapolate` は欠けていない向きの幅を直径とみなして中心を推定する。2026-10-16 の録画で、extrapolate は視線どうしの食い違いを 0.96 → 0.60 cm に減らし、報告できない球は出なかった |

`--time-trim-ms` と `--rolling-shutter-skew-ms` は、時刻合わせで取りきれないずれを直すためのもの。
最初は付けずに起動し、`-j` のログで残差が大きいときだけ使う (`ball_tracker/README.md`、`docs/ball_detection_plan_3_time_sync_calibration.md`)。

## 4. スマホを準備する

2 台それぞれで:

1. 「設定 > デバイス情報 > ビルド番号」を 7 回タップして開発者向けオプションを出す
2. 「開発者向けオプション > USB デバッグ」を on にする
3. USB で PC① につなぎ、端末に出る「USB デバッグを許可しますか」を許可する
4. 「開発者向けオプション > 充電中は画面をスリープにしない」を on にする
   (アプリはカメラ画面を表示している間しか検知しない)

PC 側で 2 台とも `device` と出ることを確かめる。`unauthorized` なら端末側の許可が済んでいない。

```powershell
adb devices
```

## 5. カメラアプリを入れる

Android Studio で `struckoutCameraApp` を一度開く
(`local.properties` が作られ、足りない SDK が入る)。同期が終わったら閉じてよい。

そのあとターミナルから、つながっている端末すべてに入れる。

```powershell
cd struckoutCameraApp
mise exec java protoc -- .\gradlew.bat installDebug
```

120fps で撮る高速撮影版 (`Struckout HS`) は `struckoutCameraAppHighSpeed` で同じことをする。
PC で解析する (1.4) のはこちらだけ。2 つのアプリは同じ端末に並べて入れられる。

```powershell
cd struckoutCameraAppHighSpeed
mise exec java protoc -- .\gradlew.bat installDebug
```

回線が遅い・GitHub に届かないと、mise がバージョンの一覧を取りに行って時間切れになり、入っている
protoc を入れ直そうとして失敗する (`HTTP timed out after 3.00s`、`Failed to install aqua:protocolbuffers/protobuf/protoc`)。
`mise install` が済んでいれば、オフラインにすれば入っているものをそのまま使う (2026-10-16 に確かめた)。

```powershell
$env:MISE_OFFLINE = '1'
mise exec java protoc -- .\gradlew.bat installDebug
```

`mise exec` 越しに実行するのは、Gradle が **PATH 上の `protoc`** を使うため
(`app/build.gradle.kts` に `Use system-installed protoc` とある)。
Android Studio の Run ボタンで入れる場合も、`protoc` が PATH に無いとビルドが失敗する。

OpenCV はリポジトリに含まれているので、別途入れる必要はない
(`.mise/tasks/install-opencv.sh` は使わない)。

## 6. `adb reverse` を張る

アプリの接続先は `127.0.0.1` に固定されている。スマホの中の 127.0.0.1 を
PC① のポートへ転送するのが `adb reverse` で、**これが無いと絶対につながらない**。

```powershell
adb devices | Select-String "\tdevice$" | ForEach-Object {
    $serial = ($_.Line -split "\t")[0]
    adb -s $serial reverse tcp:6060 tcp:6060
    adb -s $serial reverse tcp:5050 tcp:5050
    adb -s $serial reverse tcp:5052 tcp:5052
}
```

Git Bash が使えるなら `mise run adb-reverse` でも同じ (`scripts/adb-reverse.sh`)。

**USB を抜き差ししたとき、PC やスマホを再起動したときは張り直す。**
確認は `adb -s <シリアル> reverse --list`。

### 6.1 PC からアプリを操作する (高速撮影版)

カメラを設置したあとは画面に触れられないので、設定、カメラ画面を開く、「Save frame」を PC から adb で行う
(`scripts/camera-remote.ps1`)。端末ごとの設定は `scripts/cameras.json` に書く (シリアル、出力先、焦点距離、位置と向き)。
ball_tracker はつないだ順に camera_id を振るので、`open` は `cameras.json` に書いた順に 1 台ずつ開く。

```powershell
powershell -ExecutionPolicy Bypass -File scripts\camera-remote.ps1 config       # 設定を入れる
powershell -ExecutionPolicy Bypass -File scripts\camera-remote.ps1 open         # ball_tracker を起動してから
powershell -ExecutionPolicy Bypass -File scripts\camera-remote.ps1 status       # tcp=connected camera_screen=true になったか
powershell -ExecutionPolicy Bypass -File scripts\camera-remote.ps1 save-frame   # 画像を recordings\calibration\<シリアル> に取り込む
```

ほかに `retry` (ball_tracker への接続をやり直す)、`stop` (アプリを止める) がある。

## 7. ファイアウォールを開ける

PC② の projector が 5000 番につないでくる。3 章の確認ダイアログで許可していれば
不要だが、つながらないときは管理者の PowerShell で明示的に開ける。

```powershell
New-NetFirewallRule -DisplayName "struckout ball_tracker" -Direction Inbound -Protocol TCP -LocalPort 5000 -Action Allow
```

5050 と 6060 は USB 経由 (PC 内のループバック) なので開ける必要はない。

それでも届かないときは次を確かめる。

- **確認ダイアログを「キャンセル」した場合、`ball_tracker.exe` を拒否するルールが作られている。**
  拒否は許可より優先されるので、上のルールを足しても通らない。消してから試す。

  ```powershell
  Get-NetFirewallRule -DisplayName "*ball_tracker*" | Select-Object DisplayName, Direction, Action, Profile, Enabled
  Get-NetFirewallRule -DisplayName "*ball_tracker*" | Where-Object Action -eq Block | Remove-NetFirewallRule
  ```

- **スマホのテザリングなどは「パブリック ネットワーク」扱いになる。** ダイアログで
  プライベートだけ許可していると通らない (`Get-NetConnectionProfile` で確認できる)。
- **PC① 自身や他の PC から 5000 番へ試しに接続しない** (`Test-NetConnection -Port 5000` など)。
  ball_tracker は最初の 1 接続を projector とみなして待ち受けを閉じるので、
  本物の projector がつなげなくなる。疎通を試すなら 5050 番を使う。

## 8. カメラの位置と姿勢を決める

アプリの設定画面 (下のバーの歯車) で、カメラごとに `Camera Pose` を入れる。
単位は **m** と **度**。

ball_tracker が前提にしている世界座標系:

| 軸 | 意味 | 根拠 |
|---|---|---|
| x | 的までの奥行き。**的の面が x = 0**、投げる側が x > 0 | `tracking/trajectory.rs`: 軌道を x = 0 まで延ばして着弾点を出す |
| y | 的の面に沿った横方向。左の壁から測る | `ball_track.rs` の `BOOTH_Y` (-0.02〜1.22 m) |
| z | 高さ。上が正で、床からの高さ | `trajectory.rs`: 重力を -z にかけている。`BOOTH_Z` (1.05〜2.55 m) |

- `Location` の x / y / z は、この座標系でのカメラ (レンズ) の位置
- `Rotation` の roll (x) / pitch (y) / yaw (z) は、端末の座標軸を世界座標へ回す角度。
  `Rz(yaw) * Ry(pitch) * Rx(roll)` の順にかかる。すべて 0 なら端末の軸と世界の軸が一致している扱い
  (`ball_tracker/README.md`)

**長さの単位は m。** 重力 (9.80665)、視線をトラックに足す距離 (`ASSOCIATION_GATE` = 0.12)、筐体の範囲など、
追跡の定数はすべて m で書かれている。cm で入れると、ボールとみなす条件が通らなくなる。

原点は、y が左の壁、z が床、x が奥面 (`scripts/recording_analysis/booth.json` と `calibrate_camera.py` の筐体の座標と同じ)。
ball_tracker は観測した位置が筐体の内側にあること (`BOOTH_Y`、`BOOTH_Z`) と、着弾点が奥面の内側にあること
(`TARGET_Y`、`TARGET_Z`) を、ボールとみなす条件にしている。この範囲は筐体データの寸法で、
**実物を測って合っているか確かめていない**。カメラの位置を実物とずれた原点で入れると、ボールが条件で落ちる。
projector 側の `Collision Transform` とも合わせること。

ball_tracker は、projector には的の面の上の位置 `(y, z)` を送る (`collision_output/network.rs`)。

### 位置と向きの求め方

位置と向きの誤差は、着弾位置の偏りにそのまま出る (向き 1° で約 0.5 cm、位置 1 cm で約 0.45 cm)。
カメラを置くたびに、次の手順で求めるのがよい (詳細は `scripts/recording_analysis/README.md` の `calibrate_camera.py`)。

1. レンズの位置を、メジャーで筐体の座標 (m) に測る。
2. アプリのカメラ画面で「Save frame」を押し、`scripts/pull-calibration-frames.sh` で PC に取り込む。
3. `calibrate_camera.py pose` に測った位置とおおよその向きを渡し、向きを解く。
   重ね描きの画像で、色つきの線が筐体の縁に乗っているかを目で確かめる。
4. 表示された値を、アプリの設定画面の `Camera Pose` に入れる。

焦点距離は設定画面の `Focal length / image width` に入れる (0 ならカメラが知らせた値を使う)。
端末ごとに `calibrate_intrinsics.py` で求められる。

これらのスクリプトとアプリの「Save frame」は、実機では確かめていない (`docs/ball_detection_plan_0_overview.md`)。

## 9. 起動する

**順番に意味がある。** 再接続の仕組みが無いので、途中で落ちたら頭からやり直す。

1. (別の PC) game_master を起動しておく
2. **PC①: ball_tracker を起動する**

   ```powershell
   .\target\release\ball_tracker.exe --input network --output network
   ```

   `listening for projector` で止まるのが正常。

   `scripts\start-pc1.ps1` を使うと、`adb reverse` (4) もまとめて行い、ffmpeg を探して `--ffmpeg` に渡し、
   ログ (`tracker_<時分>.log`、UTF-8)、PC がデコードした最初の画像 (`snapshots\`)、止めたあとの `-j` の記録
   (`events_*`、`frames_*`) を記録フォルダー (既定は `recordings\<日付>_experiment`) に残す。止めるのは Ctrl+C。

   ```powershell
   powershell -ExecutionPolicy Bypass -File scripts\start-pc1.ps1
   ```

   projector 無しで試すときは `-Output csv` (着弾は記録フォルダーの `collisions_<時分>_raw.csv`)。
   スマホをあとからつないだときは、`adb reverse` だけ張り直す (6 章)。
3. **PC②: projector を起動する。** `TrackerHost` に PC① の IP を入れておく。
   PC① 側に `accepted TCP connection from projector` が出れば、ここで初めてカメラの受け付けが始まる
4. **PC①: `adb reverse` を張る** (6 章)
5. **スマホ 1 台目:** アプリを開く → 権限を許可 → 設定画面で `検知結果の出力先` を `Network`、
   `Camera Pose` と `Focal length / image width` を入力 → `Done`。PC① 側に次が出るのを待つ

   ```
   accepted new connection for TcpTransport ... camera_id=0
   camera info updated camera_id=0 ...
   camera pose updated id=0 ...
   ```

   接続した直後から、ball_tracker が 6060 番で時刻合わせの要求を送る (50ms おきに 8 回、そのあとは 0.5 秒おき)。
   アプリが答えない (古いアプリ) と、2 秒後に `the camera does not answer time sync requests` が出て、
   フレームが届いた時刻から時計のずれを見積もる方法に切り替わる。この場合は 2 台の時刻の精度が落ちる。

6. **スマホ 2 台目:** 1 台目の `camera pose updated` を**見てから**同じことをする。
   `camera_id=1` になることを確かめる
7. 両方のスマホを**カメラ画面** (下のバーのカメラのアイコン) にしたままにする。
   検出が届き始めると、ball_tracker が 10 秒おきに `camera clock` を出す。見るところ:

   | 項目 | 正常なとき |
   |---|---|
   | `synced` | `true` (時刻合わせが効いている)。`false` のときは古いアプリの方法 |
   | `fallback` | `false`。`true` なら `sensor timestamps do not match the synced clock` が出ていて、センサー時刻を使えていない |
   | `round_trip_ms` | USB (`adb reverse`) では 1 ms 前後の見込み (未測定) |
   | `row_skew_ms` | カメラが知らせた行ごとの時刻差。端末が返さないと 0 |

   高速撮影版のアプリなら、カメラ画面に `検出: PC で解析 (動画を送信中)` と出る。ball_tracker には
   `received video stream config` のあと、10 秒おきに `video analysis` が出る。見るところ:

   | 項目 | 正常なとき |
   |---|---|
   | `fps` | 120 前後 (カメラ画面の「エンコード」と同じくらい) |
   | `analysis_ms` | 1 枚の解析。8 ms 以内 (2026-10-15 にこの PC で 3.8 ms) |
   | `max_delay_ms` | 受け取ってから解析し終えるまでの最大。数十 ms (60fps の録画で 51〜60 ms)。500 ms を超えると着弾が出なくなる |
   | `queued` | デコード待ちのフレーム。数枚 |

   初回は `--video-snapshot-dir` を付けて起動し、保存された BMP (PC がデコードした画像) と、アプリの
   `Save frame` で保存した画像の**向きが同じ**か確かめる。上下や左右が反転していると方向ベクトルが狂う。

8. (PC②) touchpanel からゲームを始める

投球のあと (`-j` 付きで起動したとき)、`events_*` の `ThrowDiagnostics` に残差 (`residual_rms`) と、
最もよく合う時刻のずれ (`best_time_shift_ms`) が残る。直近 10 球の中央値が 3 cm を超えると
`rays do not fit the trajectories well` が出る。2 台の時刻か、カメラの位置と向きがずれている。

アプリ側の落とし穴 (コードから読み取ったもの。実機では未確認):

- **`Network` は選択済みに見えても必ずタップする。** 画面の初期表示は `Network` だが、
  内部の設定は `None` のままで、タップして初めて反映される (`ConfigScreen.kt`)
- **`Camera Pose` の 6 欄はすべて数値で埋める。** 数値でない欄があると、`Done` で `ConfigViewModel.kt` の
  `TODO("あとでUI追加する")` に当たってアプリが落ちる。`Focal length` と `Detection Area` の欄は、
  不正な値だと前の設定のままになる (落ちない)
- **一度 `Network` で保存すると、次回からはアプリを開いた瞬間に自動で接続しにいく。**
  2 台を同時に開くと同時に接続することになる。つなぎ直すときは先に両方のアプリを終了しておく

  ```powershell
  adb devices | Select-String "\tdevice$" | ForEach-Object {
      adb -s (($_.Line -split "\t")[0]) shell am force-stop com.taichi765.struckoutCameraApp
  }
  ```

- **カメラ画面のプレビューは、動くものを検知するまで何も映らない。** 真っ白でも故障ではない
- **リリースビルドは起動時に落ちる** (`MainActivity.kt` の `TODO("Sentryに置き換え")`)。`installDebug` で入れること

設定画面に `Detection output is set to 'Network', but TCP is not connected.` と出たら、
ball_tracker が起動していないか `adb reverse` が張れていない。直してから `Retry connection`。

## 10. うまくいかないとき

| 症状 | 見るところ |
|---|---|
| ball_tracker が `listening for projector` から進まない | 正常。projector がつなぐまで待つ。PC② から届かないなら 7 章と PC① の IP |
| アプリが TCP につながらない | `adb devices` が `device` か、`adb reverse --list` に 6060 と 5050 があるか、ball_tracker が projector の接続を受けたあとか |
| カメラ画面が `検出: この端末で解析` のまま | `adb reverse --list` に 5052 があるか、設定の出力先が `Network` で TCP がつながっているか (カメラの番号が要る)、ball_tracker を `--no-video-analysis` で起動していないか。ball_tracker のログに `accepted new connection in VideoTransport` が出ているか |
| `failed to start ffmpeg` | ffmpeg が PATH に無い。`--ffmpeg` で場所を渡す (1.4) |
| `video analysis` の `fps` が 120 に届かない、`max_delay_ms` が大きい | PC① の処理が追いついていない。`analysis_ms` (1 枚の解析) が 8 ms を超えるなら PC① が遅い。`falling behind` が出ていれば、送信か ffmpeg が詰まっている |
| アプリを開き直したら `camera_id` が 2, 3 ... になった | 番号は接続のたびに増える。前の接続の位置は消えずに残るので、2 台の組み合わせを誤る (右のカメラの判定など) おそれがある。つなぎ直したら ball_tracker も再起動し、1 台目から 9 章の 5 をやり直す |
| 検知が届かない | スマホがカメラ画面になっているか、画面が消えていないか |
| 衝突が出ない | `-j` を付けて起動し、`events_*` を見る。`SeedDiagnostics` にトラックを作れなかった理由 (`RejectReason`) が残る。`SuppressedCollision` は、直前の報告に近い、またはほかのトラックのほうがボールらしいとして見送ったもの |
| 位置がおかしい | `ThrowDiagnostics` の `residual_rms` が大きければ 2 台の時刻 (`--time-trim-ms`) かカメラの位置・向きを疑う。`residual_rms` が小さいのに偏るなら位置・向き。まず `Camera Pose` の実測値と軸の向き、原点 (8 章) を確かめる |
| `camera clock` の `fallback` が `true` | センサー時刻が時刻合わせと合わない。`timestamp_source` を見る。KYV45 の時計の種類は未確認 (`docs/ball_detection_plan_3_time_sync_calibration.md` の U-1) |
| projector を再起動した | ball_tracker も再起動する (次に送るときに落ちる)。そのあとカメラも 9 章の 5 からやり直す |
| ビルドで `link.exe` や `kernel32.lib` が見つからない | 1.2 の Windows SDK / C++ ツールが入っていない |
| `protoc` が見つからない | `mise exec rust protoc -- ...` を付け忘れている |

## 11. 既知の問題

実機で合わせ込む前に知っておくべきもの。いずれも ball_tracker 側のコードに由来する。

- **ボールとみなす条件の値は仮。** `ball_tracker/src/tracking/ball_track.rs` の先頭の定数
  (視線の誤差 2.5 cm、速さ 1.5〜25 m/s、筐体の範囲など) は、2026-10-10 の録画 10 球
  (60 fps、カメラの配置は 1 通り) だけで決めた。余裕は小さく (σ の上限 2.5 cm に対してボールは最大 2.3 cm)、
  配置やフレームレートが変わると、取りこぼしや誤報が出るおそれがある。
- **着弾の報告は予測時刻の 30 ms 前** (`REPORT_LEAD`)。撮影から PC に届くまでの遅れは測っていないので、
  実機で決め直す。
- **横に +5 cm 寄る偏りの原因は確定していない。** 行ごとの露光の時刻差とカメラの位置・向きの誤差が
  有力 (`docs/ball_detection_plan_0_overview.md`)。
- **時刻合わせは自動の補正が無い。** 残差が大きくても警告が出るだけで、`--time-trim-ms` は人が決める。
- **切断からの復帰が不十分。** カメラが切れても ball_tracker は落ちないが (`camera disconnected` を出す)、
  projector が切れると、次に送るときに落ちる。
- **`input-sqlite` 機能 (`--input sqlite`) はビルドできない。** 付けない (既定では無効)。
- **Nightly Release にバイナリは出ていない。** 公開条件が `main` ブランチのままで、
  `master` への改名後は動いていない。当面は各 PC でビルドする
