# 開発環境

共通ツールのバージョンとタスクはmiseで管理する。Unity、Android SDK、Docker Desktopはmiseで管理できないため、個別に用意する。

| | 何を持つか | どこで動くか |
|---|---|---|
| `mise.toml` と各モジュールの `mise.toml` | 共通・モジュールごとのツールとタスク | Windows / macOS / Linux |
| 個別に用意するツール | Unity、Android SDK、Docker Desktop | Windows / macOS / Linux |

miseで管理できるツールは`mise.toml`に置く。

## モジュールごとの環境 (mise)

[mise](https://mise.jdx.dev/tasks/monorepo.html) 2026.10.3以降のmonorepo機能を使う。
ルートの`mise.toml`で`monorepo_root = true`と`[monorepo].config_roots`を宣言し、
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
| `mise.toml` | 共通の.NET SDK、Python/uv、grpcurl、Cargoパッチ操作、録画解析タスク |
| `api/mise.toml` | Rust、protoc、buf、C#のprotobuf生成・差分確認 |
| `ball_tracker/mise.toml` | Rust、protoc |
| `camera/mise.toml` | JDK、protoc、Python、OpenCVインストール、adb reverse |
| `game_master/mise.toml` | Rust、protoc、sqlx-cli、Docker Compose操作・ビルド |
| `touchpanel/mise.toml` | Rust、protoc、slint-viewer、slint-lsp、UI操作 |
| `xtask/mise.toml`、`sandbox/testTCP/mise.toml` | Rust、protoc |
| `scripts/recording_analysis/pyproject.toml`、`uv.lock` | 録画解析用のPython依存 |

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

録画解析用の Python 依存は `scripts/recording_analysis/pyproject.toml` と `uv.lock` で管理する。
リポジトリのルートから `mise run //:recording-analysis <スクリプト> [引数...]` を実行すると、uvがロックした依存を同期してスクリプトを実行する。

タスクはbashで動く。WindowsではGit Bash (Git for Windows同梱) を使う。
既存の`patches:*`はルート共通タスクとして残り、`//:patches:status`などで呼び出せる。
DBの自動起動フックは`game_master`に入ったときだけ実行する。

### 個人設定

共有する環境変数は`.mise/config.local.toml`の`[env]`、モジュール専用の環境変数やタスクの
上書きは各モジュールの`mise.local.toml`へ書く。いずれもGit管理対象外で、すでに使っている個人設定を移動する必要はない。
例えばgame_masterのパスワードは`game_master/mise.local.toml`に以下のように設定できる。

```toml
[env]
MYSQL_ROOT_PASSWORD_DEV = "自分の開発用パスワード"
```

## Windows

Rust、.NET SDK、Python、Javaのバージョンはmiseで管理する。Windowsでもルートと開発対象モジュールで
`mise install`を実行する。タスク (`mise run ...`) はGit Bash (Git for Windows同梱) から実行する。

## 手動で用意するもの

- **Unity** — `projector` の開発に使う。
- **Android Studio と Android SDK** — `camera` の開発とAndroidデバイスとの接続に使う。
- **Docker Desktop** — `game_master` の開発と実行に使う。
