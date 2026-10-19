# 開発環境

ツールの入手先は 2 つに分かれている。

| | 何を持つか | どこで動くか |
|---|---|---|
| `.mise/config.toml`と各モジュールの`mise.toml` | モジュールごとのツール・タスク | Windows / macOS / Linux |
| `scripts/Bootstrap-Windows.ps1` | Unity, Android SDK, msys2, .NET SDK, rustup, dev.db, ASCII ジャンクション | Windows のみ |

`mise.toml` に置けるものは置く、という方針。
残っているものは OS 固有の事情があって mise では表現できないものだけ。

## モジュールごとの環境 (mise)

[mise](https://mise.jdx.dev/tasks/monorepo.html) 2026.10.3以降のmonorepo機能を使う。
ルートの`.mise/config.toml`で`monorepo_root = true`と`[monorepo].config_roots`を宣言し、
各モジュールの`mise.toml`でツール・タスクを定義する。

```bash
# 初回のみ、リポジトリのルートで実行する。
mise trust

# 開発するモジュールだけをセットアップする例。
cd touchpanel
mise install
mise run fmt-slint
```

| 設定 | ツール・タスク |
|---|---|
| `.mise/config.toml` | 共通の.NET SDK、grpcurl、Cargoパッチ操作 |
| `api/mise.toml` | Rust、protoc、buf、C#のprotobuf生成・差分確認 |
| `ball_tracker/mise.toml` | Rust、protoc |
| `camera/mise.toml` | JDK、protoc、Python、OpenCVインストール、adb reverse |
| `game_master/mise.toml` | Rust、protoc、sqlx-cli、Docker Compose操作・ビルド |
| `touchpanel/mise.toml` | Rust、protoc、slint-viewer、slint-lsp、UI操作 |
| `xtask/mise.toml`、`sandbox/testTCP/mise.toml` | Rust、protoc |
| `scripts/mise.toml` | Python |

共通ツールはすべてのモジュールに継承される。protocは必要なモジュールに同じバージョンを宣言する。
各設定と同じディレクトリの`mise.lock`でバージョン・チェックサムを管理する。
更新時は対象モジュールで`mise lock`を実行し、protocの更新時は各モジュールの宣言・ロックを揃える。
ルートの`mise install`は共通ツールだけを入れる。全モジュールが必要な場合だけ、
ルートで`mise install --monorepo`を実行する。

タスクはモジュール内では短い名前、どの場所からでも`//ディレクトリ:タスク`で呼び出せる。
依存タスクも定義元のモジュールのツール・環境で動く。

```bash
mise run //api:proto           # api/proto からC#を生成する
mise run //api:proto:check     # 生成済みのC#との差分を確認する
mise run //game_master:dev
mise run //game_master:compose -- ps
mise run //touchpanel:view --help
mise tasks ls --all            # 全モジュールのタスク一覧
```

タスクはbashで動く。WindowsではGit Bash (Git for Windows同梱) を使う。
既存の`patches:*`はルート共通タスクとして残り、`//:patches:status`などで呼び出せる。
DBの自動起動フックは`game_master`に入ったときだけ実行する。

### 個人設定

共有する環境変数は`.mise/config.local.toml`の`[env]`、モジュール専用の環境変数やタスクの
上書きは各モジュールの`mise.local.toml`へ書く。いずれもGit管理対象外。
例えばgame_masterのパスワードは`game_master/mise.local.toml`に以下のように設定できる。

```toml
[env]
MYSQL_ROOT_PASSWORD_DEV = "自分の開発用パスワード"
```

旧`.mise/config.local.toml`の`[tasks."game-master:dev"]`、`[tasks."game-master:db"]`、
`[tasks."game-master:prod"]`による上書きは`game_master/mise.local.toml`へ移し、
セクション名をそれぞれ`[tasks.dev]`、`[tasks.db]`、`[tasks.prod]`へ変更する。
`run_pc2.py`でも使う`MYSQL_ROOT_PASSWORD_DEV`は共通の`.mise/config.local.toml`の`[env]`に置く。

## Windows

```powershell
.\scripts\Bootstrap-Windows.ps1     # 初回のみ。mise 本体も入れる
. .\scripts\Enter-DevShell.ps1      # シェルごとに読み込む
```

`Enter-DevShell.ps1` は内部で `mise env` を読み込んだうえで、
mise では扱えない以下を設定する。

- **Rust の退避先** — このリポジトリが `...\ドキュメント\` 配下にあると
  GNU binutils が非 ASCII パスで壊れる。`CARGO_HOME` / `RUSTUP_HOME` /
  `CARGO_TARGET_DIR` を `%USERPROFILE%\src\.struckout-tools` に逃がし、
  リポジトリ自体も `%USERPROFILE%\src\struckout` のジャンクション越しに参照する
- **msys2 の mingw** — `x86_64-pc-windows-gnu` ターゲットのリンクに使う
  `CC` / `AR` / リンカ / `RUSTFLAGS -C dlltool=`
- **`DOTNET_ROOT`** と **Android SDK の探索**
- **`DATABASE_URL`** — mise.toml の値を ASCII 側のパスで上書きする

## 手動で用意するもの

**Unity / Android SDK / msys2。** インストーラや`sdkmanager`のライセンス同意など、
OSごとの設定が必要になる。

Rustと.NET SDKのバージョンはmiseで管理する。Windows固有のRustターゲットや
.NETのパス設定などは、利用する環境に応じて別途設定する。
