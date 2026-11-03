# 割当 Issue の分割コミット・PR 計画

作成: 2026-10-29。対象は #336、#299、#308、#320、#347、#172。

## 目的と前提

6 Issue の実装差分を Issue ごとの独立したコミットと PR に分ける。PR は各 Issue の変更だけを含み、レビュー・CI 結果を Issue 単位で追える状態にする。

現在の HEAD は `b68772a7` (`docs/single-pc`)。親エージェントの決定に従い、6 Issue の PR はそれぞれ master を base とし、#347 のみ #336 に stack する。この計画は、親エージェントがユーザーに確認中の対象範囲が確定するまで、**6 Issue だけを PR 化する前提**で作成した。作業前の変更は `C:/Users/tsuka/.codex/tmp/struckout-assigned-issues-20261029/` の `files/`、`manifest.json`、`baseline.patch` に保存されている。API / Session 移行、runtime optimization、single-PC の資料など baseline 由来の差分は Issue PR に含めない。ユーザーがその範囲も PR 化するよう指定した場合は、別の変更群として別途分割する。

この作業では source code、commit、push、PR を作成しない。現在のテスト結果は複数 Issue と baseline 差分を含む combined tree の結果であり、分割後の branch の成功を示すものではない。各 PR を分離した後、その PR の実際の tree で必要な検証を再実行して結果を記録する。#320 の性能比較・benchmark・replay 比較はユーザー指示により今回行わず、実装と build 成立のみを確認する。

## PR base と依存関係

- #336、#299、#308、#320、#172 はそれぞれ master を base にした独立 PR とする。共有ファイルに触れる箇所は別 hunk であり、他 Issue の実行時変更を必要としない。`mise.toml`、`release.yaml` 等の共有ファイルでも、Issue 所有 hunk だけを各 PR に含める。
- #347 は正常な CSV channel close を前提にするため、#336 の PR branch / commit を base に積む。#336 が master に統合されたら #347 を master 向けに更新する。
- #299 と #172 は `mise.toml` を両方編集するが、uv 設定と ports task は別 hunk であり、実行時依存もないため独立して master に向ける。#172 の generator は標準ライブラリだけを使う。
- #320 と #172 は `.github/workflows/release.yaml` の別 hunk を編集するが、PR 間に依存はない。それぞれ master 向けにし、相手 PR の hunkを取り込まない。

## Issue ごとの PR 境界

### #336 — CSV の着弾行

**含める差分**

- `ball_tracker/src/collision_output/csv.rs`: 1 着弾につき 1 行の改行を書き、channel close で正常終了する変更と、Issue に明記された複数行の回帰テスト。
- `docs/ball_tracker/ball_detection_plan_2_track_creation.md`: CSV の既知不具合の記述を更新する Issue 関連箇所。

**境界・依存**

- `csv.rs` の channel-close 対応もこの PR に含める。#347 の shutdown 実装が writer の正常終了を使うため、同ファイルを #347 側に重複収録しない。
- 回帰テストは #336 の受け入れ条件に含まれるため、この PR に限って追加を維持する。

**分割後の受け入れ**

- 修正前の連結不具合を再現するテストが、新コードでは 2 件以上の着弾を別々の 3 列レコードとして読み戻す。
- この PR branch 上で CSV の関連テストと Rust の対象 crate 検証を実行して記録する。

### #299 — recording_analysis の依存を uv で管理

**含める差分**

- `scripts/recording_analysis/pyproject.toml`、`scripts/recording_analysis/uv.lock`。
- `scripts/recording_analysis/README.md` の環境構築・起動コマンドを uv / mise に変える箇所。
- `AGENTS.md` の解析ディレクトリに対する依存パッケージ方針。
- `.github/dependabot.yml` の uv ecosystem。
- `mise.toml` の `uv` tool と `recording-analysis` task、および `mise.lock` の uv tool entry。

**境界・依存**

- `mise.toml` にある `ports:generate` / `ports:check` は #172 にのみ含める。
- recording_analysis README 内で `target/release` を `target/release-tracker` に変える単独の実行例は #320 に含める。README のほかの uv コマンド置換は #299。
- `output/`、`tmp/`、`struckoutCameraApp*` 内の一時データは含めない。

**分割後の受け入れ**

- clean な環境で uv lock に一致する依存を同期でき、README の主要 CLI を mise task 経由で実行できる。
- Python の依存更新と uv の mise 管理に対応した静的確認を、この PR branch で記録する。

### #308 — Release 失敗の記録と成功時の復旧

**含める差分**

- 新規 `.github/workflows/release-issue.yaml`。

**境界・依存**

- workflow は `workflow_run` の完了時に実行履歴をページングして再照合し、本文 marker で対象 Issue を識別する。固定 marker、run ごとのコメント marker、必要最小限の `actions: read` / `issues: write` 権限をこの PR 内で一体として扱う。
- `release-issue.yaml` の `queue: max` は監視 workflow のイベント処理を直列化する。Release 本体の `cancel-in-progress: true` は既存動作として維持する。cancelled event でも再照合を行い、最新 completed success/failure と後続 active run から Issue 状態を決める。
- `.github/workflows/release.yaml` は変更しない。Rust build/artifact は #320、Camera/Unity sparse-checkout は #172 の所有で、どちらも #308 の PR に含めない。

**分割後の受け入れ**

- 失敗の作成、同一 Issue への後続失敗コメント、success による復旧、重複イベント、古いイベント、後続 active run、cancelled run の再照合を workflow の静的検証またはスタブで確認する。
- 実 GitHub Issue の作成・コメント・close は行わない。`queue: max` に未対応の actionlint 診断を成功扱いにせず、現行 validator の制約を記録する。

### #320 — ball_tracker 専用 Release profile

**含める差分**

- root `Cargo.toml` の `[profile.release-tracker]` (`inherits = "release"`, `lto = "thin"`, `codegen-units = 1`)。
- `.github/workflows/release.yaml` の ball_tracker 用 `--profile release-tracker` build と `target/release-tracker/` artifact path。touchpanel は通常 `--release` / `target/release/` を維持する。
- `scripts/run_pc1.py` の既定 ball_tracker binary path。
- profile build の説明・実行パスを更新する `ball_tracker/README.md`、`docs/setup_pc1.md`、`docs/ball_tracker/ball_tracker_build_profiles.md`。
- `scripts/recording_analysis/README.md` 内の tracker binary path の実行例 1 箇所。

**境界・依存**

- `Cargo.toml` の専用 profile hunk だけを含める。baseline の runtime profile / optimization 変更、計測ログ、`output/assigned_issues_20261029/profile/` の資料はこの PR に混ぜない。
- `release.yaml` の profile build/artifact hunk と、#172 の sparse-checkout hunk を別々に保つ。#308 はこの workflow を変更しない。
- `run_pc1.py` の既存 CLI/runtime 差分は baseline 由来。#320 には binary path のみを含め、#172 には同ファイルの `server_port(...)` 利用だけを含める。
- 性能の前後比較・benchmark・replay 比較を行わない。専用 profile build の成立は受け入れ条件だが、性能向上は未評価として残す。

**分割後の受け入れ**

- CI または同等の clean build で ball_tracker の専用 profile がビルドされ、Release artifact と起動手順が `release-tracker` を参照する。
- touchpanel が従来の release profile で引き続き build / artifact 化される。
- 性能改善率や比較結果を報告しない。

### #347 — launcher の並列停止と graceful shutdown

**含める差分**

- `launcher/src/Struckout.Launcher.Core/Launch/Supervisor.cs` および関連する既存 launcher tests。
- `ball_tracker/src/main.rs`、`ball_tracker/src/detection_input/`、入力・decoder・recorder task の停止伝播と drain に必要な Issue 関連差分。
- `touchpanel/src/main.rs` の SIGTERM / Ctrl-C 処理。
- `ball_tracker/Cargo.toml`、`touchpanel/Cargo.toml`、`Cargo.lock` の signal / cancellation 依存差分。
- `ball_tracker/README.md` の graceful shutdown 説明。

**境界・依存**

- launcher は依存元 (client) から依存先 (server) へ停止し、同じ停止可能層を並列化する。`Supervisor.cs` にある変更だけを #347 に含める。
- `ball_tracker/src/main.rs` は baseline の runtime optimization 差分を含む。#347 の signal / cancellation / shutdown lifecycle hunk のみを抽出し、baseline の録画・transport 最適化を含めない。
- `ball_tracker/src/collision_output/csv.rs` は #336 所有。#347 ではこのファイルを変更・再収録せず、#336 の channel-close 動作を前提とする。
- `ball_tracker/README.md` の build/profile 節は #320、SIGTERM/drain 節は #347 に分ける。

**分割後の受け入れ**

- launcher の既存テストで依存順、独立層の並列停止、停止失敗時の結果回収を確認する。
- tracker と touchpanel の関連 Rust build / 既存テストを当該 PR branch で実施し、SIGTERM の実送信など未確認環境を成功として報告しない。

### #172 — `servers.yaml` をポート SSoT にする

**含める差分**

- `scripts/generate_server_ports.py` と `docs/server_ports.md`。
- `projector/Assets/Scripts/Domain/ServerPorts.Generated.cs` と `.meta`、projector の resolver / NetworkSettings / Bootstrap / Scene の Issue 関連箇所。
- `camera/app/build.gradle.kts` と Camera network client の BuildConfig port 参照。
- `game_master/docker/compose.yaml` の `# BEGIN GENERATED SERVER PORTS` ～ `# END GENERATED SERVER PORTS` 内だけ。
- `scripts/adb-reverse.sh`、`scripts/struckout_common.py` の `server_port` helper、`scripts/run_pc1.py` / `run_pc2.py` の port 利用、recording-analysis replay / stream client の port 利用。
- `mise.toml` の `ports:generate` / `ports:check` hunk、Android/Unity CI drift check とその sparse-checkout / paths hunk、必要な projector README 生成手順。

**境界・依存**

- Compose では marker block だけを #172 に含める。baseline にある `build.context` / Dockerfile 変更や Compose lifecycle 差分は除外する。
- `release.yaml` は `unity-build` 等の sparse-checkout に `api/spec` と generator を加える hunk のみを #172 に含める。#320 の build/artifact hunk は別 PR、#308 はこの workflow を変更しない。
- `run_pc1.py` の既存 runtime 差分と #320 の profile path を除き、ports helper 利用だけを選ぶ。`scripts/recording_analysis/README.md` の uv 手順は #299、tracker binary path は #320 の所有。
- Unity の serialized `0` sentinel は YAML 由来 default を使い、positive Inspector/CLI/env override と deliberate custom Scene port を保つ。
- `mise.toml` / `mise.lock` の uv entry と task は #299 にのみ含める。Generator は標準ライブラリのため #299 の uv project に依存しない。

**分割後の受け入れ**

- `mise run //:ports:check`、Android の生成差分チェック、Unity の生成差分チェックが各 PR branch で `servers.yaml` との drift を検出する。
- Camera / Projector / Compose / scripts の既定値が spec 由来である。Scene の custom override を保ち、`0` sentinel だけを既定値解決に使う。
- Android / Unity 環境で実行できなかった検証は、その理由とともに未実施として記録する。

## 共通ファイルと baseline からの分離

| ファイル | Issue PR で含める部分 | 除外・分離する部分 |
|---|---|---|
| `mise.toml` | #299: uv tool / `recording-analysis`; #172: `ports:generate` / `ports:check` | それ以外の baseline task / tool 変更 |
| `mise.lock` | #299: uv tool entry | baseline tool version 差分 |
| `release.yaml` | #320: tracker build/artifact; #172: ports generator の checkout / CI 入力 | #308 は変更しない。ほかの baseline release 改修や隣接 hunk |
| `scripts/run_pc1.py` | #320: tracker binary path; #172: server port lookup | baseline CLI/runtime 差分 |
| `scripts/recording_analysis/README.md` | #299: uv 利用; #320: tracker binary path | baseline runtime-analysis 文書の変更 |
| `ball_tracker/README.md` | #320: profile 節; #347: shutdown 節 | baseline documentation changes |
| `ball_tracker/src/main.rs` | #347: signal / cancellation / child-task lifecycle | baseline runtime optimization 差分 |
| `game_master/docker/compose.yaml` | #172: marker で囲まれた port mapping | baseline Docker context / lifecycle 差分 |
| `docs/setup_pc1.md` | #320: release-tracker command/path hunk | baseline single-PC setup 差分 |

差分を作るときは保存した baseline と現在の Issue 差分を比較し、該当する hunk を明示的に選ぶ。`git add -A` など全変更の一括 staging は行わない。`output/`、`tmp/`、`struckoutCameraApp/`、`struckoutCameraAppHighSpeed/` にある生成物・一時データは PR に含めない。

## 未確定事項とブロッカー

1. 親エージェントがユーザーに確認中の範囲 (6 Issue のみか、baseline の API / runtime optimization 等も含むか) は未回答。この計画は 6 Issue のみを分割し、baseline 差分を issue PR に混ぜない。
2. 現在の実装レポートにある検証結果は combined tree 由来。PR を分けた後に各 branch で実施した検証だけを、その PR の結果として記載する。

## この計画作成時に行った確認

- baseline `manifest.json` と保存済み snapshot を読み、baseline と Issue 実装の差分を `release.yaml`、`run_pc1.py`、recording-analysis README、`struckout_common.py`、Compose、`setup_pc1.md` 等で比較した。
- Release 本体の `cancel-in-progress: true` は維持し、`release-issue.yaml` の `queue: max` で監視 workflow を直列化する決定を反映した。cancelled event は最新の completed success/failure を再照合する。
- source code の編集、commit、push、PR、テスト、benchmark、外部状態の変更は行っていない。
