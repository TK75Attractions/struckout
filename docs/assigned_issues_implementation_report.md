# 割当 Issue 6 件の対応結果

対応日: 2026-10-29。対象は #336、#299、#308、#320、#347、#172。
計画は [実装計画](assigned_issues_implementation_plan.md) を参照。
既存の未コミット変更を保持し、`codex/assigned-issues-20261029` で実装した。
push、PR 作成、Issue の作成・コメント・クローズ、GitHub Actions の起動は行っていない。

## 実装内容

| Issue | 変更 |
|---|---|
| #336 | CSV を着弾ごとの `x,y,z` 行として出力し、入力チャネルが閉じたら正常終了する。2件の行数・列数・符号付き小数の値を確認する回帰テストを追加し、既知不具合の説明も更新した。 |
| #299 | 解析スクリプトの NumPy / OpenCV を uv project と lockfile で管理する。mise の uv 固定・実行タスク、Dependabot、README、AGENTS の依存利用方針を更新した。 |
| #308 | Release の完了イベントで run 履歴を照合し、失敗 Issue の作成、続く失敗へのコメント、復旧時のクローズを実装した。固定マーカーで識別し、再通知、再実行、キャンセル、後続実行中の状態も扱う。 |
| #320 | `release-tracker` に Thin LTO と `codegen-units = 1` を設定した。tracker の Release workflow、配布元ディレクトリ、PC①起動スクリプトと案内を揃え、ほかのモジュールの通常 `release` は維持した。 |
| #347 | launcher は依存元から依存先へ、独立モジュールを並列停止する。キャンセル混在時も成功分の状態を反映する。ball_tracker と touchpanel に終了シグナル処理を追加し、tracker は接続、受信済みパケット、追跡、出力の順に終了を待つ。 |
| #172 | Rust、launcher、Android、Unity、Python、adb reverse、Compose のポート既定値を spec 由来に揃えた。生成・検査タスクと CI の入力・変更検知も更新した。 |

ユーザーの最終指示により、性能比較は今回の対象から外した。#320 の性能向上は未評価であり、
途中の測定データを改善率や採用根拠として使用しない。追加の benchmark や録画の前後比較は行わない。

## 実装確認

- CSV の回帰テストは修正前に失敗することを確認し、修正後の ball_tracker 全テストで成功した。
- `SQLX_OFFLINE=true cargo check --all-targets -p ball_tracker -p touchpanel` は成功。
- ball_tracker の既存・回帰テストは lib 55 件と bin 6 件が成功。touchpanel は全体 12/12 の成功を確認した後、
  再実行で既存の時間依存テスト `case_2_not_cancelled` が1回失敗し、その単体再実行は成功した。
  この失敗を無効化・削除していない。
- `cargo clippy --all-targets -p ball_tracker -p touchpanel` は終了 0。
  既存の Default、引数数、collapsible_if、テストの未使用変数などの警告は残る。
  今回追加した CSV テストの配置による警告は修正した。対象 Rust の rustfmt 検査も成功。
- launcher の `dotnet test Struckout.Launcher.slnx` は 53/53 成功。
  `dotnet format Struckout.Launcher.slnx --verify-no-changes` も成功。
- 一時的な Application 実行用プログラムで control/data TCP 接続、camera ID 応答、停止要求、
  Application の完了、両接続の EOF、CSV の読取可能状態を確認した。一時ソースは削除済み。
- `release-tracker` の `sentry` feature 付き全 bin のビルドを確認した。
- uv の `lock --check` と `sync --locked`、mise タスクの列挙、解析 CLI の help を確認した。
- Python の変更ファイルの構文、PC①・②の help、共通ポートの読取、spec を変えたときの取得値、
  不正・重複ポートの拒否を確認した。`ports:check` と shell の構文検査も成功。
- Compose の `config --no-interpolate --format json` で公開ポートと接続先ポートの解決を確認し、
  最終差分の `git diff --check` も成功。既存のコンテナやデータベースは操作していない。
- Android は既存の SDK を指定し、生成タスク、Kotlin/Java コンパイル、`:app:assembleDebug` が成功。
- Unity 6000.5.2f1 のコンパイルは最終変更後も成功。EditMode は 121 件中119 件成功。
  失敗2件は作業前の報告にもある `CollisionTransformStoreTests` で、今回のポート変更とは別の箇所。
- Release 監視は gh のスタブで初回失敗、重複、連続失敗、復旧、後続実行、キャンセルを検証した。
  workflow の actionlint では、現行ツールが未対応の公式 `concurrency.queue: max` の診断が出る。
  その診断だけを一時的に除外してほかの静的検査を実施した。リポジトリに除外設定は追加していない。

## 検証上の制約

- Windows Arm64 の OpenCV は現在の依存版に対応する wheel がなく、native Python では source build になる。
  uv の同期確認にはローカルの uv 管理 x64 Python を使った。リポジトリに OS 固有の回避コードは追加していない。
- Unix の SIGTERM の実送信、Android 実機、実プロジェクター、GitHub 上での自動 Issue 操作は未確認。
- 既存の `input-sqlite` feature には `PairedFrames` / `CameraFrame` の型不整合があり、今回の対象外。
  これは既存の設計資料にも記録されている。feature のビルド成功は報告しない。

関連資料: [サーバーポート](server_ports.md)、[Release](release_workflow.md)、
[tracker ビルド](ball_tracker/ball_tracker_build_profiles.md)、
[Projector のポート](projector/projector_network.md)。
