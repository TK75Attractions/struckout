# AGENTS.md

このリポジトリで作業する AI エージェント (Claude Code、Codex など) 向けの規約。
人間のコントリビューターにもそのまま当てはまる。

この規約の中心は、**メンテナーの taichi (taichi765) が書くコードやコミットに近づけること**。
taichi は Linux ユーザーで、責任の分離、型による表現、根本原因の解決を重視する。
迷ったときは「taichi ならこう書くか」を基準にする。手本は `game_master/` と `touchpanel/` の Rust、
`camera/scripts/install_opencv.py` の Python。

---

## 1. リポジトリの全体像

| ディレクトリ | 言語 | 役割 |
|---|---|---|
| `ball_tracker/` | Rust | カメラの 2 次元検出を受け取って 3 次元に三角測量し、的に当たった位置を `projector` に送る |
| `game_master/` | Rust (tonic gRPC, sqlx/MySQL, Docker) | ゲームのセッション、得点、ランキングの SSoT |
| `touchpanel/` | Rust (Slint + stern) | 受付用のタッチパネル UI。`game_master` のクライアント |
| `projector/` | C# (Unity) | 的を描き、当たり判定と得点計算をして `game_master` に通知する |
| `camera/` | Kotlin (Android) | カメラアプリ。検出結果か動画を `ball_tracker` に送る |
| `api/proto/`, `api/spec/` | protobuf, YAML | モジュール間の契約。ポートは `api/spec/servers.yaml`、色は `api/spec/color-palette.yaml` |
| `api/rust/` | Rust | `struckout-proto` (生成コードと newtype)、`struckout-yaml` (spec の読み込み) |
| `launcher/` | C# (Avalonia) | モジュールをリリースからインストールし、依存の順に起動する |
| `xtask/` | Rust | 開発用コマンド (`cargo xtask ...`) |
| `scripts/` | Python, sh | 起動や解析のスクリプト |
| `migrations/` | SQL | sqlx のマイグレーション (`mysql/` は連番) |
| `docs/` | Markdown, Mermaid | 設計メモ、仕様、セットアップ手順 |
| `sandbox/` | いろいろ | 実験用。本番のコードから参照しない |

構成図は `docs/architecture.mmd` (Mermaid) にある。PC の分担は `docs/machine_separation.md`、
`docs/setup_pc1.md`、`docs/setup_pc2.md` を見る。

### ドキュメントの置き場所

- リポジトリ全体: `README.md` (ディレクトリ構造と環境構築)
- 横断的な設計や仕様、計画: `docs/<topic>.md`。1 つのモジュールに関するものは `docs/<module>/` に置く (`docs/ball_tracker/`、`docs/projector/`)。ファイル名は snake_case で、`projector_*`、`ball_detection_plan_N_*` のようにトピックを接頭辞にする
- モジュール固有のアルゴリズムや使い方: `<module>/README.md`
- 図: Mermaid (`.mmd`) で書き、生成した PNG もコミットする。Figma などの外部ツールで描いた図は使わない
- docs/のドキュメントは日本語で書く。仕様を変えたら、同じ PR でドキュメントも直す
- LLMのメモリ以外の意味を持たないドキュメントはdocs/llm_memory/に置く。README.mdはメモリに使わない

---

## 2. 環境とツール: Windows だけでしか動かないものを作らない

taichi は Linux で開発している。CI も `ubuntu-24.04` で動く。
**どのコードやスクリプトも Linux で動くことを最優先にし、Windows と macOS でも動くようにする。**

- ツールは [mise](https://mise.jdx.dev/) で管理する (ルートの `mise.toml` と `mise.lock`、モジュールごとの `<module>/mise.toml`。`docs/development_environment.md` を見る)。新しいツールが要るときは mise に足し、手動でのインストール手順を増やさない。mise で扱えないもの (Unity、Android SDK、Docker) だけを README に書く
- 定型の作業は mise のタスクにする (`mise run proto`、`mise run //game_master:build` など)。タスクは bash で書く。Windows では Git Bash で動く
- **スクリプトは Python か POSIX sh で書く。PowerShell (`.ps1`) やバッチ (`.bat`) は足さない。** 以前の `.ps1` は Python に置き換えた (`scripts/run_pc1.py` など)。Gradle が生成する `gradlew.bat` は例外
- Python のスクリプトは:
  - 標準ライブラリだけで書き、外部パッケージを要求しない。解析用の `scripts/recording_analysis/` だけは例外で、要るものを README に書く
  - パスは `pathlib.Path` で扱う。`C:\...` や `\` 区切りを埋め込まない。実行ファイルは `shutil.which()` で探す
  - OS ごとに分ける処理は `struckout_common.py` の `IS_WINDOWS` / `per_platform()` に集め、呼び出し側に散らさない
  - `def main() -> int:` と `if __name__ == "__main__": raise SystemExit(main())` の形にする。失敗したら stderr に日本語で理由を出し、0 以外で終える
  - 捕まえる例外は具体的な型に絞る (`except (OSError, urllib.error.URLError) as error:`)。素の `except:` は書かない
  - 型ヒントを付ける (`str | None` など)
- 改行は LF にする (`.gitattributes` を見る)。シェルスクリプトと生成コードは特に必ず LF
- サーバーは Docker で動かす (`game_master/compose*.yaml`)。イメージは digest で固定し、arm64 でもビルドできるようにする
- 非 ASCII のパスや Windows のジャンクションのような、特定の環境だけの回避策をリポジトリに入れない。必要なら個人の設定 (`.mise/config.local.toml`、`<module>/mise.local.toml`、`.cargo/patches.toml`) に閉じ込める

### バージョンの固定と再現性

- ツールは `mise.lock` で固定する。生成物をコミットしているツール (`buf`、`protoc`) は特に厳密に固定する
- GitHub Actions の `uses:` はコミットハッシュで固定し、`# v6.4.0` のようにバージョンをコメントで添える
- 依存の更新は Dependabot に任せる。新しいエコシステムを足したら `.github/dependabot.yml` にも足す
- Rust の依存はルートの `Cargo.toml` の `[workspace.dependencies]` で管理し、各 crate からは `{ workspace = true }` で参照する
- ローカルだけで依存を差し替えたいときは `.cargo/patches.toml` を使う (git の管理外)。`Cargo.toml` を書き換えてコミット前に戻す、という運用はしない

### 生成コード

- protobuf から生成した C# はコミットする。`.proto` を変えたら `mise run proto` で作り直し、`mise run proto:check` で確かめる
- sqlx のオフライン用メタデータ (`.sqlx/`) は `mise run //game_master:prepare` で作り直してコミットする。CI は `SQLX_OFFLINE=true` で動く
- 生成ファイルは手で直さない。`.gitattributes` で `linguist-generated` にする

### 秘密情報

- 秘密情報は `.mise/config.local.toml` か `<module>/mise.local.toml` (どちらも git の管理外) に置き、mise の `redact = true` で隠す。`.env` は使わない
- DSN、パスワード、トークンをコードやログに書かない

---

## 3. 設計の方針

### 3.1 責任を分ける

各モジュールは、役割ごとの層に分かれている。新しいコードはその層の境界を守って置く。

- **game_master**: `lib.rs` に trait (`DataSource`) と操作ごとのエラー型、`data.rs` に DB での実装 (`DataSourceImpl`)、`service.rs` に gRPC サービス (`GameMasterServiceImpl`)。サービスは trait にだけ依存する
- **touchpanel**: `data/` (gRPC の通信と状態) と `presentation/` (画面ごとの ViewModel) と `ui/` (Slint)。画面に関する判断 (トップバーを出すかどうかなど) は Slint 側に置く
- **camera**: Screen、ViewModel、Repository、UseCase に分け、Hilt で DI する。検証のような判断のロジックは Screen ではなく ViewModel に置く
- **projector**: `Domain` / `Application` (インターフェース) / `Infrastructure` (実装と Fake) / `Unity` / `Bootstrap` を asmdef で分け、依存の向きを守る
- **ball_tracker**: 入力 (`detection_input/`)、処理 (`tracking/`)、出力 (`collision_output/`) に分ける。入出力の種類は差し替えられる

taichi が繰り返し直してきた点:

- **別の概念は別のものとして表す。** 1 つのフラグに 2 つの意味を持たせない。例: 「検出結果をどこに出すか」と「ローカルのデータをアップロードするか」は別のものなので、`networkFeatureEnabled` をやめて `detectionOutputKind` にした
- **状態の持ち主は 1 つにする (SSoT)。** 設定は Repository に 1 つだけ持ち、他は購読する。ゲームの状態は `game_master` が持つ。ポートのように複数のモジュールで使う定数は `api/spec/*.yaml` に 1 つだけ置き、Rust では `build.rs` で読み込んで `env!` で埋め込む。同じ値を複数の場所に書かない
- **実装の詳細は外に出さない。** モジュールの外から使わない型は private にする。外部のクライアントを包む trait は sealed にする (`InternalGrpcClient` を参照)
- **抽象化は、差し替える必要がはっきりしたときだけ作る。** 実装を差し替えるとき (実行時の切り替えや Fake)、またはテストでモックするときに trait やインターフェースを作る。それ以外の抽象化は作らない。taichi は「不要な抽象化を消す」という refactor を何度もしている
- 1 つのファイルに複数の責任が入ったら分ける (`data/game_master.rs` から `data/remaining_time.rs` を分けたように)

### 3.2 型で表す

- ID は newtype にする (`GameId`、`PlayerId`、`MachineId`)。素の `u32` を引き回さない
- 状態や種類は enum で表す。文字列や bool を組み合わせて表さない
- 浮動小数点数は `f64` / `double` を使う (`float` は使わない)
- 文字数は Unicode の文字単位で数える (バイト数で数えない)
- 時刻は UTC かローカル時刻かが型でわかるようにする (`UtcDateTime` と `PlainDateTime` を参照)

### 3.3 エラーの扱い

- Rust では、**操作ごとに `thiserror` で enum を定義する** (`AddPlayerError`、`GetGameResultError` など)。下の層のエラーは `#[error(transparent)] Sqlx(#[from] sqlx::Error)` のように包む。アプリケーションの最上位でだけ `anyhow` を使う
- 外からの入力、ネットワーク、DB で起きる失敗は、**クラッシュさせずにエラーとして返し、UI やログに出す** (カメラアプリで、接続の失敗を例外から戻り値に変えたように)
- 起きないはずのこと (不変条件) は `expect("理由")` で落とす。なぜ起きないのかをコメントに書く (`// guaranteed by CHECK constraint`)。理由を書かない `unwrap()` は書かない
- 設定の誤りは起動時に検出して落とす (fail-fast)。例: NavHost のルートの登録漏れは起動時に検出する。xtask は必要なディレクティブがなければエラーを返す。ビルド情報が取れなければビルドを失敗させる
- サーバーは SIGTERM で正常に終了する。依存先 (DB など) にはヘルスチェックとリトライ (`backon`) を入れる

### 3.4 観測できるようにする

- ログは `tracing` で出す。境界の関数には `#[instrument]` を付ける
- Sentry は cargo の feature (`sentry`) で有効にする。必須の依存にしない
- オプションの入出力 (sqlite の入力など) も feature で分ける
- ログや CSV のように後で解析するものは、形式を決めてドキュメントに書く

---

## 4. コードの書き方

- まわりのコードに合わせる。命名、コメントの量、イディオムを既存のファイルから読み取る
- Rust: edition 2024。`cargo fmt` と `cargo clippy` の指摘をなくしてからコミットする (`chore: Apply lints` のように、まとめて直してもよい)。Slint のファイルは `mise run //touchpanel:fmt-slint` で整形する
- 識別子とコミットメッセージは英語で書く。利用者に見せるメッセージ (CLI の出力、UI) とドキュメントは日本語で書く。コメントと doc comment は、そのファイルやモジュールの言語に合わせる (`game_master/` と `touchpanel/` は英語、`ball_tracker/` は日本語)。新しいファイルは英語で書く
- doc comment (`///`) には、**何をするかに加えて、なぜそうしているか**を書く。参考になる外部の文書があればリンクを貼る
- 一時的な妥協には `// HACK:` や `// TODO:` を付け、何が前提になっているかを書く。できれば issue 番号も書く (`// TODO(#172): ...`)
- 使わなくなったコード、ファイル、依存、設定は残さず消す (`chore: Remove stale ...`)。コメントアウトして残さない
- 外部の crate の API が変わったら、使っている側をすべて新しい API に合わせる (`fix: Follow stern's API change`)。古い使い方と新しい使い方を混ぜない

### テスト

- テストは対象と同じファイルの `#[cfg(test)] mod tests` に書く。Slint のバックエンドを初期化するようなテストだけは `tests/` に置く
- 道具: パラメータ化は `rstest`、モックは `mockall` (`#[cfg_attr(test, mockall::automock)]`)、DB は `testcontainers` の本物の MySQL、時間が関わるものは `#[tokio::test(start_paused = true)]`
- Unity は `projector/Assets/Tests/EditMode/`、Android は JUnit / Robolectric とインストゥルメンテーションテスト
- 機能を足したらテストも足す。バグを直すときは、まずそのバグを再現するテストを書く
- テストが落ちたときに、テストを消したり `#[ignore]` にしたりして通さない (5 章を見る)

---

## 5. 推奨されるリポジトリ運用

### 5.1 小手先の修正ではなく、根本原因を直す

- 症状を消すだけの修正 (値を手で調整する、`sleep` を足す、例外を握りつぶす、リトライで隠す、特定の環境だけで分岐する) で済ませない。**なぜ起きたのか**を突き止めてから直す
- 根本原因が別のモジュールや上流のライブラリにあれば、そこを直す。taichi は、slint や stern の問題を手元で回避するのではなく、fork して直している (`Tk75Attractions/slint`、`Tk75Attractions/stern`)
- どうしてもすぐに回避策が要るとき (本番の直前など) は:
  1. 根本原因と本来の直し方を **issue に書く**
  2. 回避策のコードに `// HACK:` と issue 番号を書く
  3. コミットメッセージや PR の説明に、回避策であることと issue へのリンクを書く
- CI、lint、テスト、型検査を無効にして通さない。`--no-verify` を使わない。理由のない `#[allow(...)]` や `@Suppress` を足さない。どうしても必要なら、理由をコメントに書く

### 5.2 issue に記録する

- 作業中に見つけた、**今の作業の範囲外の問題や改善点は、その場で直さずに issue にする**。PR の範囲を広げない
- 調べたことはその場で消えないように残す。会話やローカルのメモではなく、issue かドキュメントに書く
- issue の書き方 (既存の issue に合わせる):
  - タイトルは `[component] 何をするか` (例: `[touchpanel/Playing] テストを追加する`、`[game-master] Improve error handling`)
  - 本文には、なぜ必要か (背景) と、わかっている範囲での方針を短く書く。作業が複数あるときはチェックリストにする
  - ラベルは `c: <component>` (`c: camera`、`c: ball-tracker` など) と、必要なら `enhancement`
  - 閉じるときは、解決した PR (`#201`) か、閉じる理由 (「xtask-sync はもう使っていない」など) を書く
- エージェントは、issue の文面を作って人間に見せ、承認を得てから起票する (`gh issue create`)。issue を勝手に作ったり、閉じたり、コメントしたりしない

### 5.3 ブランチとコミット

- `master` に直接コミットしない。作業ごとにブランチを切る: `feat/<topic>`、`fix/<component>/<topic>`、`refactor/<component>/<topic>`、`ci/<topic>`、`docs/<topic>`、issue から作るなら `<番号>-<topic>`。複数の作業をまとめて試すときは `integration/<YYYY-MM-DD>` を使う
- コミットメッセージは Conventional Commits の形で書く。taichi の書き方に合わせる:

  ```
  <type>(<scope>): <Imperative summary, capitalized, no period>

  <なぜ変えたか。必要なら関係するコミットや上流の issue、変更履歴へのリンク>
  ```

  - type: `feat` `fix` `refactor` `chore` `ci` `docs` `tests` `ui` `optim`
  - scope: `game-master` `touchpanel` `ball-tracker` `camera` `projector` `scripts` `mise` `deps` `xtask` `proto` など。リポジトリ全体なら省く
  - 例: `fix(touchpanel): Clone reference to underlying property before removing character`
  - 例: `chore(deps): Upgrade tokio-stream from 0.1.18 to 0.1.19` (本文に、新しいバージョンのどの変更が必要なのかを書く)
- **1 つのコミットには 1 つの変更だけを入れる。** リネーム、整形、機能の追加、バグの修正を混ぜない。「いろいろ」「fix」「WIP」のような、内容のわからないメッセージを書かない
- AI と一緒に書いたコミットには `Co-Authored-By:` を付ける
- 共有しているブランチを force push しない。履歴を書き換えない

### 5.4 プルリクエスト

- タイトルは `[component] 何をするか`。1 つのコミットだけなら、そのコミットのメッセージをそのまま使ってよい
- 本文には、含まれるコミットの一覧を書く。この PR でやらないことは「〇〇は別の PR でやる」と書いて、後で別の PR に分ける
- 関連する issue は `Closes #N` でつなぐ
- 1 つの PR には 1 つのまとまった目的だけを入れる。大きい変更は、レビューできる大きさに分ける
- CI (Rust check、Unity、Android、proto) が通ってからマージする

### 5.5 変更したものを最新に保つ

- コードを変えたら、関係するドキュメント、`README.md`、`docs/architecture.mmd`、mise のタスクも同じ PR で直す
- ファイルの移動やリネームをしたら、参照しているところ (CI の `paths`、`sparse-checkout`、`dependabot.yml`、`.gitattributes`、ドキュメント) をすべて直す
- 古くなったドキュメントを見つけたら、直すか issue にする。古いまま残さない

---

## 6. エージェントへの指示

- 作業を始める前に、関係するモジュールの README と `docs/` の関係するページを読む
- 既存のパターン (層の分け方、エラー型、テストの書き方) を探して真似る。新しい書き方を持ち込む前に、既存のものでできないか確かめる
- 変更したら、少なくとも関係するものを手元で確かめる:
  - Rust: `SQLX_OFFLINE=true cargo check --all-targets -p <crate>` と `cargo test -p <crate>`、`cargo clippy`、`cargo fmt --check`
  - proto: `mise run proto:check`
  - Python: Linux で動くか考えて書き、可能なら Windows と Linux の両方で実行する
  - 確かめられなかったもの (Unity、実機) は、確かめていないとはっきり報告する
- 外に影響すること (push、PR の作成、issue の作成やクローズ、リリース、Docker イメージの公開) は、人間の承認を得てから行う
- 規約と異なる判断をするときは、その理由を PR の説明か、コミットメッセージの本文に書く
