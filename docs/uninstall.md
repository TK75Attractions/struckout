# 運営に使用した別 PC からの完全アンインストール

ストラックアウトの運営に使った **Windows PC から、持ち込んだソフト・データ・設定を取り除き、運営前の利用状態へ戻す**ための手順。
開発用 PC ではなく、片付ける運営用 PC 上で実施する。**この Markdown ファイルだけを対象 PC の Codex に渡して使用できる。**
複数の PC で運営した場合は、各 PC で導入したものを同じ順序で確認する。

ここでの「完全」は、運営アプリ、DB、録画、設定、追加した開発・実行ツールと残存ファイルの削除を指す。
運営以前からあるソフトや所有者が別用途で使うデータは維持する。OS の初期化やディスクの消去は行わない。
スマホ内のアプリ削除は対象外であり、PC の片付けにスマホの接続は必要ない。

**DB も含めて削除する。必要な記録は開始前に運営側の保管先へ移す。**
対象 PC の導入状況は未調査で、削除操作の実機検証は行っていない。
この資料には固定のユーザー名、導入先、パスワードは埋め込んでいない。これらは下記の方法で対象 PC から取得する。
既定値と異なる保存先や導入前の設定値は推測せず、実機の記録を優先する。

## 0. 撤去を担当する Codex への引き継ぎ

対象 PC の Codex に、このファイルを添付して次の依頼文を渡す。

> この資料に従い、この PC のストラックアウト用ソフト・データ・設定を完全撤去してください。
> 最初に実機を調査して削除対象と共有利用中のものを識別し、対象が確認できた運営用の追加分を削除してください。
> DB・ランキング・録画・ローカル設定も削除対象です。必要な退避があるかは作業開始時に確認してください。
> 元リポジトリや他の手順書の取得は不要です。スマホ、別 PC、リモートの Docker、クラウド上の原本は操作しないでください。
> 導入前からある環境、共有データ、識別できないものは維持し、不明点だけ確認してください。
> 実施した削除と確認結果、残したものと理由、権限不足や再起動待ちを最後に報告してください。

担当 Codex は、以下のルールで作業する。

- 以下の PowerShell コマンドは対話シェルで使う。新しい `.ps1` の作成や開発ツールのインストールは不要。
- `%APPDATA%` 等はエクスプローラー用の表記。PowerShell では `$env:APPDATA` 等を使う。
- `CONFIRMED_...` と `<...>` は実機調査後に置き換える仮記号。そのまま実行しない。
- ファイル削除前に絶対パス・内容・リンク先を確認する。ドライブ直下、ユーザーフォルダー全体、未確認のジャンクションを再帰削除しない。
- 削除対象の ID・パスと実行結果を記録する。パスワード、トークン、設定全文は会話や作業ログに出さない。
- 元リポジトリが無い場合は、プロセスの実行パス、導入記録、Docker のラベルから直接撤去する。
- ツールが見つからないだけで「削除済み」としない。実行ファイル・保存データ・ユーザー領域も確認する。
- 再起動は作業記録を残してユーザーに時機を確認する。再起動後の確認が済むまでその項目は未完了と記す。

### 最初に記録する実機情報

| 項目 | 実機で確認・記録する内容 |
|---|---|
| 対象 | コンピューター名、使用ユーザー、OS、管理者権限の有無 |
| 元からあったもの | 維持するアプリ、共有 SDK・Docker・mise、所有者の認証 |
| 運営ファイル | リポジトリ・配布物・録画・較正設定の絶対パス、複数コピーの有無 |
| Docker | ローカルの接続先、プロジェクト名、対象コンテナ・マウント・ボリューム・イメージ・ネットワーク |
| ツール | 導入方法、実行パス、版、共有利用の有無、カスタム保存先 |
| 退避 | 必要な記録の有無、退避先、退避完了の確認。不要なら「不要」と記録 |

調査用コマンド例（削除は行わない）:

```powershell
hostname
whoami
Get-CimInstance Win32_OperatingSystem | Select-Object Caption, OSArchitecture
Get-Command docker,mise,git,adb,ffmpeg,rustup,cargo,dotnet,java,python,uv -All -ErrorAction SilentlyContinue |
    Select-Object Name,Source
Get-CimInstance Win32_Process |
    Where-Object { $_.Name -match 'ball[_-]tracker|touchpanel|StruckOut|struckout-launcher' } |
    Select-Object ProcessId,Name,ExecutablePath
```

アプリ一覧の確認は Windows の「インストールされているアプリ」または `winget list` を使う。
自動修復を起動することがある `Win32_Product` の列挙は使わない。

### ランチャー導入記録の読み方

`%LOCALAPPDATA%\struckout\launcher\install-state.json` があれば、削除前にローカルで確認する。
JSON の `modules` に `ball-tracker`、`touchpanel`、`projector`、`game-master`、`camera` が記録される。
各項目の `directory` は導入先、`executable` は実行パス、`release-tag` は版を表す。
`prerequisites` の `docker`、`adb`、`ffmpeg` は次の意味で扱う。

| `kind` | 意味・使うフィールド |
|---|---|
| `found` | ランチャー導入以前に見つかったツール。`path` を確認し、既存ツールとして維持する |
| `package-manager` | ランチャーが導入。`manager` と `package-id` が削除元・対象 ID |
| `downloaded` | ランチャーが展開。`directory` が削除対象フォルダー |

記録は現在の実体と照合する。記録が無い・壊れている場合も、既定パスと実機調査で進める。
ランチャーのモジュール保存先は `%LOCALAPPDATA%\struckout\launcher\modules\<モジュール名>`、
adb のダウンロード先は同じ `launcher\tools\platform-tools`。
起動設定は `%APPDATA%\struckout\launcher\launcher.toml`、カメラ設定は同じ場所の `cameras.json`。
DB パスワードは `%LOCALAPPDATA%\struckout\launcher\secrets\mysql-root-password` にあるが、DB の削除にパスワードの読み出しは不要。

## 1. 対象 PC で準備する

1. ソフトのアンインストールに使う管理者権限を用意する。
2. 運営時に使った Windows ユーザーでサインインする。複数のユーザーを使ったなら、それぞれの保存領域も確認する。
   `%APPDATA%` などは現在サインインしているユーザーの場所を指す。
3. リポジトリ、配布ファイル、録画の保存先を確認する。デスクトップ、ダウンロード、ドキュメント、別ドライブも調べる。
4. 必要な動画・較正設定・ゲーム記録は運営側の別媒体へ退避し、開けることを確認する。
   DB が必要なら担当者がバックアップと復元方法を確認してから進める。この PC 内の退避コピーは最後に削除する。
5. 「設定 > アプリ > インストールされているアプリ」で、運営のために追加したソフトを確認する。
   既存ソフトと区別できないものは所有者に確認する。

**Docker、mise、リポジトリ、ランチャーの保存フォルダーを先に削除しない。**
DB や管理下のツールを片付けるために使う。存在しない項目は飛ばし、削除のために開発環境を新規構築しない。

## 2. 運営アプリを停止する

ランチャーを使った場合は「すべて止める」を押す。ランチャー本体は削除操作に使うため残す。
ソースから起動した場合は、次の順で停止する。

1. `scripts/run_pc1.py` や ball_tracker を実行中のターミナルで Ctrl+C を押す。
2. touchpanel を閉じる。
3. `scripts/run_pc2.py` を使った PC では、リポジトリのルートで次を実行する。

   ```bash
   python scripts/run_pc2.py --stop
   ```

4. Unity Editor、Android Studio、別途起動した projector も閉じる。

タスク マネージャーで `ball_tracker`、`touchpanel`、`StruckOut` 等の終了を確認する。
元の起動スクリプトが無い場合は、0 章のプロセス一覧で実行パスが対象配布先内であると確認し、
通常のウィンドウ終了を試みる。応答しない対象だけ `Stop-Process -Id CONFIRMED_PROCESS_ID` で終了する。
同名の他用途のアプリや、共有の Python・Java・FFmpeg を名前だけで一括停止しない。
Docker Desktop は DB 削除のため起動したままにする。

## 3. DB・Docker のコンテナ・イメージを削除する

その PC で game_master を動かした場合に実施する。**プレイヤー・ゲーム記録・ランキングもすべて削除する。**
Docker Desktop を起動し、エンジンが動くまで待つ。

削除前に `docker context show`、`docker context ls` と、`DOCKER_HOST` / `DOCKER_CONTEXT` の設定をローカルで確認し、
**操作先がこの PC の Docker Desktop であること**を確かめる。SSH・TCP 等の接続先を削除対象と推定しない。
別 PC のエンジンへ向いていればそこで停止し、ローカルの接続先を特定する。

### ランチャーで導入した場合

1. 「インストール」画面で game-master を選ぶ。
2. **「データベースも消す」を選んで**アンインストールする。
3. 実行ログを確認し、Docker の操作に失敗していないことを確認する。

通常はコンテナ、関連イメージ、DB ボリューム、保存した DB パスワードを削除する。
ランチャーは不要になった導入済みツールも削除するため、運営専用に導入した Docker が一緒に削除されることがある。
ランチャー導入のツールを後から他用途にも使い始めた場合は、この自動削除を使わず、下記の手動削除で対応する。
Docker や Compose ファイルが無い、エンジンが起動できない場合は、画面の完了表示だけで削除完了と判断しない。

### ソースから起動した場合

リポジトリのルートで実行する。

```bash
docker compose -f game_master/docker/compose.yaml -f game_master/docker/compose.dev.yaml down --volumes --rmi all
```

開発構成 `game-master-dev` のコンテナ、ネットワーク、DB ボリューム、サービス用イメージを削除する。
`MYSQL_ROOT_PASSWORD` 未設定の警告が出ることがあるが、この作業は停止・削除だけを行う。
このためにパスワードを設定し直したり、`up` でサーバーを起動したりしない。
MySQL 等のイメージを他用途でも使う場合は `--rmi all` を外し、専用イメージだけ後で削除する。

### 残存資源の確認（どちらの導入方法でも実施）

Docker が残っていれば Docker Desktop の Containers / Volumes / Images で確認する。
CLI では次の一覧を使う。

```bash
docker compose ls --all
docker ps -a
docker volume ls
docker image ls
docker system df
```

| 対象 | 標準の名前・識別方法 |
|---|---|
| ランチャー構成 | `struckout-launcher`（ランチャーが Compose のプロジェクト名を明示する） |
| 本番用 Compose を直接起動した構成 | `game-master-prod` |
| ソース起動構成 | `game-master-dev` |
| 旧構成 | `game_master` |
| DB ボリューム | `struckout-launcher_mysql_data`、`game-master-prod_mysql_data`、`game-master-dev_mysql_data`、`game_master_mysql_data` |
| イメージ | `ghcr.io/tk75attractions/game-master`、開発構成の app イメージ、使用した MySQL |

名前を変えていれば実際の名前を使う。
`docker inspect <コンテナ名>`、`docker volume inspect <ボリューム名>` で
`com.docker.compose.project` ラベルや接続先を確認できる。`<...>` は実際の名前へ置き換える。
残っている専用コンテナ、ボリューム、ネットワーク、イメージを Docker Desktop から削除する。

### リポジトリ・ランチャー・Compose ファイルが無い場合の直接削除

この方法ならリリース版、DB パスワード、Compose ファイルの取得は不要。
既定名だけで判断せず、ラベルとマウントがストラックアウトのものか確認してから削除する。
以下は例として `struckout-launcher` を調べる。他の構成名にも同じ確認を繰り返す。

```powershell
docker ps -a --filter label=com.docker.compose.project=struckout-launcher
docker volume ls --filter label=com.docker.compose.project=struckout-launcher
docker network ls --filter label=com.docker.compose.project=struckout-launcher
```

見つかった対象を個別に調べる。コンテナの環境変数全文には秘密情報があるため出力しない。

```powershell
docker inspect --format '{{json .Config.Labels}}' CONFIRMED_CONTAINER_ID
docker inspect --format '{{json .Mounts}}' CONFIRMED_CONTAINER_ID
docker inspect --format '{{.Config.Image}}' CONFIRMED_CONTAINER_ID
docker volume inspect CONFIRMED_VOLUME_NAME
docker network inspect CONFIRMED_NETWORK_NAME
docker ps -a --filter volume=CONFIRMED_VOLUME_NAME
```

DB のサービス名は `db`、アプリは `app`。MySQL の保存先はコンテナ内の `/var/lib/mysql`、
DB 名は開発構成が `main_dev`、本番構成が `main_prod`。通常は `mysql_data` という名前付きボリュームに保存する。
独自構成でホストのフォルダーを bind mount している場合は `.Mounts` の `Source` を記録する。
そのフォルダーはコンテナを消しても残るため、専用データと確認して 4 章で削除する。

識別できたコンテナを停止・削除し、その後に専用ボリューム・ネットワーク・イメージを削除する。
各行は対象 ID・名前に置き換えて個別に実行し、終了コードと再一覧で確認する。

```powershell
docker stop CONFIRMED_CONTAINER_ID
docker rm CONFIRMED_CONTAINER_ID
docker volume rm CONFIRMED_VOLUME_NAME
docker network rm CONFIRMED_NETWORK_NAME
docker image rm CONFIRMED_IMAGE_REFERENCE
```

`docker rm` だけでは名前付き DB ボリュームは消えない。`docker volume rm` まで行う。
「使用中」で失敗した場合は利用元を確認し、共有利用している別コンテナを強制削除しない。
イメージはタグ・digest と利用コンテナを確認し、共有していれば維持する。`--force` による削除は行わない。
Docker CLI が PATH に無い場合は、既存の `%PROGRAMFILES%\Docker\Docker\resources\bin\docker.exe` 等を探す。
CLI・エンジンが無くても、5 章の Docker 保存領域があればデータは残っている。
運営専用の Docker なら保存領域も削除する。共有環境で起動不能ならその項目を未完了として記録する。

運営専用に Docker を導入した PC では、ビルドキャッシュ等も 5 章の本体削除と残存フォルダー確認で片付ける。
既存 Docker を残す PC では共有キャッシュを一括削除しない。
全プロジェクトに影響する `docker system prune --all --volumes` は使わない。

## 4. PC 用アプリ・ソース・運営データを削除する

ランチャーがあれば、残った ball-tracker、projector、touchpanel 等の PC 用モジュールをアンインストールして終了する。
導入したツールを他用途にも使っている場合は自動削除を使わず、配布フォルダーを手動削除する。
camera の PC 内の APK・導入記録は次のフォルダー削除で片付ける。
スマホを接続して camera をアンインストールすると端末側も削除対象になるため、今回は接続しない。

エクスプローラーで次の場所を開き、実際のパスと中身を確認して削除する。
PowerShell での確認は `Get-Item -LiteralPath 'CONFIRMED_ABSOLUTE_PATH' -Force`、
直下の一覧は `Get-ChildItem -LiteralPath 'CONFIRMED_ABSOLUTE_PATH' -Force` を使う。
リンク・ジャンクションの場合は `LinkType` / `Target` を確認する。
対象と絶対パスを確定した後だけ、ファイルは `Remove-Item -LiteralPath 'CONFIRMED_ABSOLUTE_PATH'`、
専用フォルダーは同コマンドに `-Recurse` を付けて削除する。別シェルへパスを渡して削除しない。

| 削除する場所 | 内容 |
|---|---|
| `%APPDATA%\struckout` | ランチャー・tracker の設定、`cameras.json`、CSV、追跡 JSON |
| `%LOCALAPPDATA%\struckout` | モジュール、ツール、APK、ログ、導入記録、DB パスワード |
| `%USERPROFILE%\AppData\LocalLow\DefaultCompany\StruckOut` | projector の位置合わせ設定と Unity Player ログ |
| 運営用に取得したリポジトリ全体 | ソース、`target/`、Unity の `Library/`・`Builds/`、Gradle 成果物、録画、解析環境、個人設定 |
| リポジトリ外に置いた実行ファイル・設定・記録 | 配布フォルダー、録画、画像、CSV、JSON、解析結果 |
| デスクトップ・ダウンロード等の配布物 | `struckout-launcher-...exe`、APK、ZIP、インストーラー、ショートカット |
| `%TEMP%\struckout_game_master_up.log` | ソース起動時の Docker 起動ログ |

`%APPDATA%\struckout` の内訳は、`launcher\launcher.toml`、`launcher\cameras.json`、
`ball-tracker\config.yaml`、`tracker\csv\`、`tracker\json\`。
ランチャー側のログは `%LOCALAPPDATA%\struckout\launcher\logs\` に保存される。
ソース起動側の録画・ログはリポジトリの `recordings\<日付等>\` にあり、
`tracker.log`、`tracker_stderr.log`、`projector_*.log`、動画、較正画像、解析結果を含む。
`touchpanel` 専用の永続保存先は現行実装には無く、配布物と起動元に保存したログが主な削除対象。

リポジトリの削除だけでは DB は消えない。必ず 3 章を先に終える。
projector は `.exe` だけでなく、同じ配布フォルダーの `*_Data` や DLL もまとめて削除する。
`recordings/`、`.mise/config.local.toml`、各モジュールの `mise.local.toml`、旧 `.env.dev` 等も、リポジトリ全体とともに削除する。
リポジトリを複数コピーした場合はすべて確認する。

リポジトリの識別の手掛かりはフォルダー名 `struckout` と、直下の `ball_tracker`、`game_master`、`touchpanel`、
`projector`、`camera`、`launcher`、`scripts`、`Cargo.toml`。
古い作業コピーでは `struckoutCameraApp`、`struckoutCameraAppHighSpeed` という Android プロジェクトもあり得る。
配布ファイル名は `ball-tracker-*`、`ball_tracker.exe`、`touchpanel-*`、`touchpanel.exe`、
`projector-*.zip`、`StruckOut.exe`、`struckout-launcher-*`、`camera.apk` 等。
名前は検索の手掛かりであり、同名だけを理由に削除しない。

ショートカットのプロパティにある「リンク先」「作業フォルダー」、タスクの起動引数、
プロセスの実行先から任意の配置先をたどる。通常の保存先で見つからない場合に、所有者が運営で使用した追加ドライブを調べる。

任意の保存先を使った場合は、起動設定の `--video-record-dir`、`--video-snapshot-dir`、`--csv_path`、
`-collisionTransform`、`STRUCKOUT_COLLISION_TRANSFORM` も手掛かりにする。
Unity の Company Name / Product Name を変えた配布物では保存先も変わる。
別ドライブや同期フォルダー上のローカルコピーも確認する。運営側の正式な保管先は削除対象に混ぜない。

## 5. 運営のために追加したソフトと残存データを削除する

**この PC に運営のため新規導入したものは、開発ツールも含めて削除する。**
もとからあったソフトは維持し、今回追加した版・コンポーネントだけを外す。

### 5.1 通常のアプリ

サインアウト操作にアプリを使うため、削除前に 6 章の認証情報の片付けを先に行う。
「設定 > アプリ > インストールされているアプリ」または各製品の管理ツールで削除する。

| ソフト | 削除の順序・確認 |
|---|---|
| Docker Desktop | DB 削除後にアンインストール。他用途の Docker データも消えるため、共有利用中は本体を残す |
| Unity Editor・書き出しモジュール | Unity Hub で今回追加した Editor を削除。Hub も今回導入したなら、その後に削除 |
| Android Studio | SDK Manager で SDK の保存先を控えてから削除。SDK・設定・キャッシュは下表でも確認 |
| Visual Studio Build Tools・Windows SDK | Visual Studio Installer で今回追加したインストールを削除。既存 Visual Studio への追加分は「変更」から外す |
| FFmpeg | 導入元のパッケージ管理ツールまたはアプリ一覧で削除。手で展開した版は展開先も削除 |
| 個別導入した Rust、Java、.NET SDK、Python 等 | mise 管理分と区別し、各製品のアンインストーラーや導入元で今回追加した版を削除 |
| Git、GitHub CLI 等 | 認証の片付け後に今回導入分を削除。Git Bash はコマンド操作が終わってから削除 |

winget で削除する場合は先に `winget list --id CONFIRMED_PACKAGE_ID --exact` で対象を確認し、
`winget uninstall --id CONFIRMED_PACKAGE_ID --exact` を実行する。
既知の導入 ID は以下。実際のアプリ一覧・導入記録を優先し、列挙した ID を一括実行しない。

| ソフト | winget ID の候補 |
|---|---|
| Docker Desktop | `Docker.DockerDesktop` |
| FFmpeg | `Gyan.FFmpeg` |
| mise | `jdx.mise` |
| Git | `Git.Git` |
| Android Studio | `Google.AndroidStudio` |
| Visual Studio 2022 Build Tools | `Microsoft.VisualStudio.2022.BuildTools` |

winget に無い場合は、アプリ一覧または元のアンインストーラーを使う。
インストール情報を調べるレジストリの場所は、`HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall`、
`HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall`、
`HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall`。
表示名、版、導入先、アンインストーラーを確認する用途に限り、共有のキーをまとめて削除しない。
個別の SDK や実行環境の版はこの資料の作成時点の版を決め打ちせず、対象 PC に実在する版を使う。

### 5.2 mise と管理下のツール

mise 本体だけでなく、Rust、.NET SDK、Java、Python / uv、protoc、buf、grpcurl、sqlx-cli、Slint 等の管理下のツールも対象にする。

1. リポジトリ外へ移動し、`mise ls` で導入した版、`mise doctor` で設定・データ・キャッシュ保存先を控える。
   環境変数で変更した場合は実際の保存先を使う。出力を外部へ送る必要はない。
2. mise が以前からある場合は、今回追加した版だけを `mise uninstall <tool>@<version>` で削除する。
   本体と既存設定は残す。`<tool>@<version>` は実際のツール名・版へ置き換える。
3. mise も今回新規導入し、管理下の全データを削除する場合は、次で削除予定のパスを確認する。

   ```bash
   mise implode --dry-run --config
   ```

4. **単体ファイルとして導入した mise** は、対象が今回導入分だけであることを確認し、次を実行する。

   ```bash
   mise implode --config
   ```

5. **winget 等で導入した mise** は、導入元から本体をアンインストールする。
   控えた設定・データ・キャッシュのうち今回作られた保存先をエクスプローラーで削除する。
   管理下のツール本体、ダウンロードキャッシュ、信頼済みリポジトリの記録も取り除く。
6. PowerShell プロファイルや Git Bash の初期化ファイルに追加した `mise activate` 等の行を取り除く。

`implode` は管理下の全ツールに影響するため、既存の共有環境には使わない。

mise が既に起動できない場合の Windows の既定保存先は次のとおり。
`MISE_*` または `XDG_*` の環境変数やシェル設定に変更があれば実際の場所を優先する。

| 内容 | 既定パス | 主な上書き変数 |
|---|---|---|
| ツール・プラグイン・shims | `%LOCALAPPDATA%\mise` | `MISE_DATA_DIR`、`MISE_INSTALLS_DIR`、`MISE_SHIMS_DIR` |
| グローバル設定 | `%USERPROFILE%\.config\mise` | `MISE_CONFIG_DIR` |
| 信頼設定等の状態 | `%USERPROFILE%\.local\state\mise` | `MISE_STATE_DIR` |
| キャッシュ | `%TEMP%\mise` | `MISE_CACHE_DIR` |
| 一時ファイル | `%TEMP%\mise-tmp` | `MISE_TMP_DIR` |

単体配布の mise 本体は `%USERPROFILE%\.local\bin\mise.exe`、同じ場所の `mise-shim.exe` も確認する。
winget 版の実行ファイルは `Get-Command mise -All` とアプリ一覧で配置先を確認する。
Rust の本体が mise 管理でも、Cargo のパッケージキャッシュが `.cargo` 等に残るため次項も確認する。

### 5.3 アンインストール後の残存フォルダー

アプリを閉じてから確認する。**運営専用に新規導入したソフトは、専用の残存領域も削除する。**
共有領域は運営分を特定して削除する。`*` は名前の例であり、一括削除コマンド用の指定ではない。

| 種類 | 確認する保存先の例 |
|---|---|
| Docker | `%PROGRAMDATA%\Docker`、`%PROGRAMDATA%\DockerDesktop`、`%PROGRAMFILES%\Docker`、`%LOCALAPPDATA%\Docker`、`%APPDATA%\Docker`、`%APPDATA%\Docker Desktop`、`%USERPROFILE%\.docker` |
| Android SDK | SDK Manager で確認した保存先（標準は `%LOCALAPPDATA%\Android\Sdk`） |
| Android Studio | `%APPDATA%\Google\AndroidStudio*`、`%LOCALAPPDATA%\Google\AndroidStudio*`。独自に変更した保存先も確認 |
| Android のユーザーデータ | `%USERPROFILE%\.android`（エミュレーター、adb の認証鍵、デバッグ署名鍵等） |
| Rust / Cargo | `%USERPROFILE%\.cargo`、`%USERPROFILE%\.rustup`（環境変数で変更した場合は実際の保存先） |
| Gradle / NuGet | `%USERPROFILE%\.gradle`、`%USERPROFILE%\.nuget\packages`（カスタム保存先も確認） |
| Unity / Unity Hub | Hub に表示された Editor の保存先、`%LOCALAPPDATA%\Unity`、`%APPDATA%\Unity`、`%APPDATA%\UnityHub` 等の今回作成した設定・ログ・キャッシュ領域 |
| その他 | 手で展開した JDK・FFmpeg・Platform Tools、uv 等の今回生成したキャッシュ、インストーラー |

追加で確認するキャッシュ・ログの候補:

- uv: `%LOCALAPPDATA%\uv\cache`。起動できれば `uv cache dir` で実際の場所を確認する。
- Python: 運営用プロジェクト内の `.venv`、`__pycache__`。手で作った別の仮想環境も、導入先を特定して削除する。
- Unity Hub: `%LOCALAPPDATA%\UnityHub`、Unity のユーザーキャッシュ: `%LOCALAPPDATA%\Unity`。
- GitHub CLI: `%APPDATA%\GitHub CLI` または `GH_CONFIG_DIR` が示す場所。自分の認証だけを解除してから扱う。
- Docker のユーザー単位のインストール: `%LOCALAPPDATA%\Programs\DockerDesktop`。存在すればアプリの削除後に残存を確認する。
- winget の手動展開型パッケージ: `%LOCALAPPDATA%\Microsoft\WinGet\Packages` 配下の今回導入した FFmpeg 等。
  まず winget でアンインストールし、Packages フォルダー全体は削除しない。

Android Studio が残っていれば SDK Manager で保存先を控える。既に無ければ `ANDROID_HOME` / `ANDROID_SDK_ROOT`、
運営プロジェクトの `local.properties` の `sdk.dir`、adb の実行先を手掛かりにする。
Java は `JAVA_HOME`、Gradle は `GRADLE_USER_HOME`、Rust は `CARGO_HOME` / `RUSTUP_HOME`、
uv は `UV_CACHE_DIR`、NuGet は `NUGET_PACKAGES` も確認する。
通常の `.dotnet` 等を含む共有領域は、個別導入・mise 管理・他用途を区別し、運営専用と確認できた部分だけ削除する。

共有の `.cargo`、`.gradle`、`.nuget` 等を丸ごと消さない。
Docker 用に WSL を導入した場合も、他のディストリビューションや既存 WSL 環境は削除しない。
Windows の機能や USB ドライバーを今回追加したなら、他用途の依存が無いことを所有者と確認して元に戻す。

## 6. 認証・環境変数・Windows 設定を元に戻す

1. この PC に今回追加した GitHub、Docker、Unity 等のログインをサインアウトする。
   ブラウザーや Windows 資格情報マネージャーに保存した自分の認証情報も削除する。
   所有者の既存アカウントは変更しない。今回発行した一時トークンがあれば、発行元での失効が必要かユーザーへ引き継ぐ。
   GitHub CLI は `gh auth status` で対象を確認し、今回追加したアカウントだけを
   `gh auth logout --hostname github.com --user CONFIRMED_ACCOUNT` で解除する。
   Docker は現在の資格情報の対象を確認し、今回追加したものだけを `docker logout` または
   `docker logout CONFIRMED_REGISTRY` で解除する。共有の資格情報ファイルを丸ごと消さない。
2. 「システムの詳細設定 > 環境変数」で、ユーザー変数・システム変数の追加分を削除する。
   PATH は全体ではなく、削除したツールのパスだけを取り除く。
   adb / SDK、mise / shims、Rust、Java、FFmpeg の追加分や、今回設定した `MYSQL_ROOT_PASSWORD_DEV`、
   `MYSQL_ROOT_PASSWORD_PROD`、`DATABASE_URL`、`STRUCKOUT_*`、`GH_TOKEN` / `GITHUB_TOKEN` 等を確認する。
   既存値を変更した項目は元の値へ戻す。
3. Windows Defender ファイアウォールの詳細設定で、今回追加した `struckout ball_tracker` や、
   削除した実行ファイルを指す運営用の受信・送信規則を削除する。
4. 今回作ったスタートアップ登録、タスク スケジューラのタスク、ショートカット、起動スクリプトを削除する。
5. 固定 IP、ネットワーク設定、画面の拡張・メイン画面、解像度、拡大率、スリープ・電源設定を元へ戻す。
   元の値が分からなければ所有者に確認する。
6. ターミナル履歴、作業メモ、一時バックアップに、自分の認証情報や運営データが残っていないか確認し、該当分を削除する。

### Windows 設定の識別情報と調査コマンド

標準ポートは tracker が TCP `5000`・`5050`・`5052`・`6060`・`5080`、game_master が TCP `8020`、
開発用 MySQL がループバックの `3306`。旧カメラ同期に `6262` を使った構成もある。
これらの番号だけで他用途の規則・プロセスを削除しない。規則の実行ファイルと配置先も照合する。

```powershell
Get-NetFirewallRule -DisplayName '*struckout*','*ball_tracker*' -ErrorAction SilentlyContinue |
    Select-Object Name,DisplayName,Direction,Action,Enabled
Get-NetFirewallRule -Name CONFIRMED_RULE_NAME | Get-NetFirewallApplicationFilter
Get-NetFirewallRule -Name CONFIRMED_RULE_NAME | Get-NetFirewallPortFilter
```

対象規則と確認できたものだけ、管理者権限で `Remove-NetFirewallRule -Name CONFIRMED_RULE_NAME` を実行する。
規則名にアプリ名が含まれない場合もあるため、運営実行ファイルのパスを指す規則を追加確認する。

自動起動の確認先は次のとおり。

- ユーザーのスタートアップ: `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup`
- 全ユーザーのスタートアップ: `%PROGRAMDATA%\Microsoft\Windows\Start Menu\Programs\Startup`
- レジストリ: `HKCU:\Software\Microsoft\Windows\CurrentVersion\Run` と `RunOnce`、同じ HKLM 側。
- タスク スケジューラ: `Get-ScheduledTask` で一覧を確認し、実行ファイル・引数が対象配布先を指すものだけ撤去する。
- 独自にサービス登録した場合: `Get-CimInstance Win32_Service | Select-Object Name,State,PathName` で実行先を確認する。
  標準構成は運営モジュールを Windows サービスとして登録しない。見つけた他用途のサービスは触らない。

Run/RunOnce はキー全体ではなく対象の値だけ、タスクは対象名と TaskPath を確定したものだけ削除する。
共有の PowerShell プロファイル、`.bashrc`、`.bash_profile`、Git の設定もファイル全体を消さず追加分だけ戻す。
PowerShell の履歴ファイルは `Get-PSReadLineOption` の `HistorySavePath`、Git Bash は通常 `%USERPROFILE%\.bash_history`。
内容を会話に貼らず、今回の秘密情報・運営操作に該当する部分をローカルで整理する。

## 7. 再起動して完了を確認する

- [ ] 運営用の実行ファイル、ランチャー、ソース、APK・ZIP・インストーラーが残っていない。
- [ ] game_master のコンテナ・DB ボリューム・専用イメージが残っていない。
- [ ] 録画、画像、ログ、CSV、JSON、較正設定、DB パスワード、ローカルの退避コピーが残っていない。
- [ ] 今回導入したツール本体、SDK、キャッシュ、設定、認証情報を片付けた。
- [ ] 使用した Windows ユーザーすべてと、運営で使用した各ドライブの保存先を確認した。
- [ ] 不要な PATH・環境変数・ファイアウォール規則・自動起動登録が残っていない。
- [ ] ごみ箱の今回削除した項目を消した。他人の項目も入ったごみ箱全体は無断で空にしない。
- [ ] 再起動後に運営アプリが起動せず、削除したツールを参照する起動エラーが出ない。
- [ ] 運営以前から使っていたアプリやネットワークが利用できる。

残した共有ソフトがあれば、その名前と理由を所有者へ引き継ぐ。
必要な記録は運営側で保管し、対象 PC から運営用の追加分を取り除けたことをもって完了とする。

## 8. 作業結果の報告書式

撤去した Codex は、次をこの資料とは別の短い作業記録としてユーザーへ返す。秘密情報を記録しない。

| 区分 | 記載内容 |
|---|---|
| 対象 | PC 名、作業ユーザー、作業日 |
| 削除済み | 実際に削除したアプリ・版・絶対パス・Docker 資源名 |
| 残したもの | 共有ソフト等の名前、場所、残した理由 |
| 未完了 | 権限不足、所有者への確認待ち、起動不能な Docker、再起動待ち等 |
| 確認済み | 再一覧、残存パス確認、自動起動・規則確認、再起動後の結果 |

「コマンドが無い」「処理を開始した」「再起動が必要」と「削除完了」を区別する。
この資料自体のコピーを対象 PC から消すかは、結果報告後にユーザーの希望に合わせる。
