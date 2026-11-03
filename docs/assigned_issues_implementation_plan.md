# 割当 Issue 6 件の調査・対応計画

作成: 2026-10-29。対象は `TK75Attractions/struckout` の #336、#299、#308、#320、#347、#172。

## 目的と完了条件

割当中の 6 Issue をコードと既存仕様に照らして調査し、Issue に記載された振る舞いを実装する。仕様・運用手順も同じ変更で更新し、各変更に合ったビルド・lint・既存検査を行って結果を記録する。push、PR、GitHub Issue の作成・編集・クローズは行わない。

## 作業時の制約

- 作業ブランチは `codex/assigned-issues-20261029`。開始時点の tracked diff と関連 untracked files は `C:/Users/tsuka/.codex/tmp/struckout-assigned-issues-20261029` に保存済み。Session / Player API 移行、runtime optimization、計測、その他の作業を含むため、これらを戻したり今回の Issue に関係しないファイルを整形・生成・上書きしたりしない。
- 各担当は編集前に対象ファイルの作業前差分を確認し、既存の変更と今回の変更を共存させる。今回の責任範囲と関係しない既存差分は触れない。
- `docs/runtime_optimization_plan.md` と `docs/runtime_optimization_report.md` が存在する。#320 は両文書の目的と重なる箇所を確認し、現在の runtime optimization を取り消さず、リリース時の Rust 最適化プロファイルに範囲を限定する。
- `AGENTS.md` は解析用 Python の依存管理を README に記す例外を設けている。#299 で uv 管理へ変えるときにのみ、該当するルールも一緒に直す。
- 作業ツリーは全担当で共有される。下記のファイル所有を守り、同じファイルを同時に編集しない。`scripts/` のポート参照は primary agent が引き取る。

## 現状の根拠

| Issue | 確認した現状 |
|---|---|
| #336 | `ball_tracker/src/collision_output/csv.rs` は `x,y,z` のみ書き、行末を書かない。複数着弾が連結する。`docs/ball_tracker/ball_detection_plan_2_track_creation.md` にも既知問題が記録されている。 |
| #299 | `scripts/recording_analysis/README.md` は Python 3、numpy、opencv-python のインストールを手動で案内する。解析コードは両ライブラリに依存する。root `mise.toml` は mise/lock で toolchain を管理している。 |
| #308 | `.github/workflows/release.yaml` は手動実行 (`workflow_dispatch`) で、ビルド行列と複数の下流 job がある。publish 権限は job 単位で分かれている。失敗記録の重複防止には各 job の成否を集約する終端処理が必要。 |
| #320 | root `Cargo.toml` は dev profile の一部依存だけを最適化し、release profile は workspace 共通設定の既定値を使う。今回はtracker専用profileの実装とCI buildまでを扱い、既存 runtime optimization の測定値と区別して性能効果は未評価とする。 |
| #347 | `Supervisor.StopAllUnlockedAsync` は依存順の逆順に runner の `StopAsync` を逐次 await する。`ProcessModuleRunner` は Unix で `kill -TERM`、Windows でウィンドウ終了を要求する。game_master は Unix の TERM/CTRL-C を処理するが、ball_tracker と touchpanel の entry point はこの信号を処理していない。 |
| #172 | Rust crate と launcher はすでに build-time の `api/spec/servers.yaml` からポートを読み込む。Kotlin の camera と Unity projector にはポート値が残る。YAML には TCP/GRPC の listener ポートに加えて `xtask-sync.camera` もあるため、利用箇所の棚卸しをして対象を決める。 |

## 変更の担当と順序

### 0. 基準確認（primary agent）

- git status と今回触るファイルの既存差分を退避済み baseline と比較し、担当へ既存差分の保持を伝える。baseline は `C:/Users/tsuka/.codex/tmp/struckout-assigned-issues-20261029` にある。
- 各担当の完了を確認したあと、同じファイルを使う変更を順番に統合する。#308 workflow の後に #172 の生成/ビルド連携を行う。

### 1. CSV 出力（primary agent、#336）

- 所有: `ball_tracker/src/collision_output/csv.rs`、同モジュールの既存テスト領域、および `docs/ball_tracker/ball_detection_plan_2_track_creation.md` の同 Issue に直接関係する記述。
- 各 CollisionPoint3D を `x,y,z` と改行を含む一行として書く。既存の flush とエラー処理を維持する。
- Issue の完了条件に複数件の回帰テストがあるため、2 件以上の出力を読み戻して行数・3 列・値を確かめるテストを追加する。回帰テストを追加しない判断は Issue の完了条件を満たさない。

### 2. 解析用依存管理（uv worker、#299）

- 所有: `scripts/recording_analysis/pyproject.toml`（`package = false`）、uv lockfile、`scripts/recording_analysis/README.md`、root `mise.toml` と `mise.lock`、root `AGENTS.md`、および必要な `.github/dependabot.yml`。ポート generator の mise task は ports worker が所有し、共通の `mise.toml` は編集を直列化する。
- 依存と Python バージョンを uv project で宣言・固定し、既存スクリプトの実行例を uv から再現可能にする。README の手動 pip 手順を uv の sync/run に置き換える。
- AGENTS.md の「標準ライブラリのみ」の適用範囲を通常スクリプトと解析用 scripts に分け、解析で適切な依存パッケージを使うことを許す。uv の更新を Dependabot 対象に含める。
- 既存の mise 管理/lock 方針と uv project の場所を整合させ、全ライブラリを不用意に最新版へ上げない。

### 3. Release の失敗・復旧通知（CI worker、#308）

- 所有: 専用の `.github/workflows/release_monitor.yaml`。既存 `.github/workflows/release.yaml` のpublish権限や実行順には監視処理を追加しない。
- `workflow_run` の Release completed event（cancelledを含む）を受け、元runのworkflow名、`workflow_dispatch` event、master branch、repositoryを確かめる。元workflowのソースはcheckoutしない。monitorの権限は `actions: read` と `issues: write` に限定する。
- eventごとにActions runs endpointをすべてのページにわたって再取得し、対象runのうち最大の `(run_number, run_attempt)` を持つcompleted `success` / `failure` を状態根拠として選ぶ。そのrunより新しいqueued/in-progress runがあるかも確認する。失敗は最新のrunが新たに進行中でも捨てずIssueに反映する。成功はより新しいrunがqueued/in-progressの間はIssueをcloseしない。
- `cancelled` run自体は状態根拠から除くが、そのeventは再照合を起動する。したがって新しいrunの取消時に、直前の成功を反映してcloseするか、直前のfailureを記録してopenのままにする。
- 本文内の固定markerでIssueを識別し、新しいlabelは追加しない（repositoryに `c: ci` labelはない）。最新completed runがfailureで対応するopen Issueがなければ作成し、既存ならrun URL付きのコメントをrun IDごとに一度付ける。最新completed runがsuccessなら、該当Issueのみ閉じる。古いeventの再配信が後続runの結果を上書きしない。
- concurrencyにはGitHub Actionsの現行 `queue: max` を用いる。公式仕様上、同一groupで最大100件がFIFO順に待機でき、`queue: max` と `cancel-in-progress: true` は併用不可。既存release workflowのdispatch時に実行中runを取消して通知を失う設定はなくす。順序、重複、手動キャンセル、queue上限の扱いを確認する。[GitHub Actions workflow syntax](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax)
- workflow syntax と `actionlint` 等利用可能な静的検査を実行し、状態遷移（初回失敗、連続失敗、成功復旧）をレビューする。ローカルから GitHub Issue を作成・コメント・閉鎖して動作確認しない。

### 4. Rust tracker 専用 release profile（primary agent、#320）

- 所有: root `Cargo.toml` の独立した tracker 専用 Cargo profile、`.github/workflows/release.yaml`、`scripts/run_pc1.py`、ball_tracker の起動手順と該当 docs、`docs/runtime_optimization_plan.md` / `docs/runtime_optimization_report.md` の今回の対応状況。共有の通常 `[profile.release]` は変えない。
- `[profile.release-tracker] inherits = "release", lto = "thin", codegen-units = 1` を実装する。profile は ball_tracker を指定してビルドし、その実行 hot path を含む runtime dependency graph に適用する。per-package override で tracker crate だけに限定しない。
- Cargoはworkspace rootでcustom profileを定義でき、`--profile`で選択した成果物を`target/<profile-name>/`へ出力する。thin LTOはdependency graphをまたぐ最適化に適し、package overrideではLTOを設定できないため、専用profileを選んで対象依存graph全体に適用する。[Cargo Book: Profiles](https://doc.rust-lang.org/cargo/reference/profiles.html)
- release workflow では ball_tracker だけを `--profile release-tracker` で作り、`target/release-tracker/` の実行ファイルを release artifact として出す。touchpanel は従来どおり `[profile.release]` でビルドする。`run_pc1.py` の既定実行パスと関係する手順も tracker 専用profileへ合わせる。
- 今回は性能の前後比較、benchmark、replayによる比較を行わない。CI で専用profile buildが成立することを今回の完了条件とし、性能向上は未評価と報告する。既存のruntime optimization計測値は別変更・別条件の記録なので、今回のprofile採否の根拠や正式な比較結果に流用しない。専用profileの性能測定は残作業として明記する。

### 5. 並列停止と終了シグナル（shutdown worker、#347）

- 所有: `launcher/src/Struckout.Launcher.Core/Launch/Supervisor.cs` とlauncher tests、`ball_tracker/src/main.rs` とinput/listener/decoder child-task lifecycle、`touchpanel/src/main.rs`。CSV output task/channelの正常終了はprimary agentが#336内で所有する。
- 依存グラフで独立したmoduleを同時停止し、**client/依存元からserver/依存先へ**停止する。ball-trackerとtouchpanelを先に止め、camera/projector/game-masterを後に止める。同一依存層を並列にし、失敗を含め全runnerの結果を回収してUI状態とログに出す。
- `ball_tracker` と `touchpanel` に SIGTERM の graceful shutdown を実装する。ball_tracker input listenerとdecoder/recorder等のchild taskを追い、停止要求後に新規受信を止め、受理済みデータを安全にdrainしてCSV recorderをflush/closeする。既存Ctrl-Cと矛盾させず、game_masterのUnix signal patternを参考にする。
- launcher の関係する既存テストを実行する。全モジュールの順序・独立グループ、停止失敗時の回収、CLI shutdown を確認し、Windows/macOS/Linux の確認可否を報告する。

### 6. サーバーポートの一元化（ports worker、#172）

- 所有: ports workerはPython shared generator（`ServerPortsError(ValueError)`、`load_server_ports(spec_path: Path | None = None) -> dict[str, dict[str, int]]`、`get_server_port(module_name: str, endpoint: str, spec_path: Path | None = None) -> int`）、commitするgenerated C# constants、camera Gradle BuildConfig、projectorのgenerated constants利用、root mise generate/`--check`、Android/Unity CI drift checkと生成手順READMEを担当する。primary agentは`scripts/adb-reverse.sh`、`scripts/recording_analysis/`のreplay/stream client、`struckout_common.py`等のport参照を担当する。Compose生成port blockはports workerが所有する。mise/uv設定はuv worker、workflow CI連携はCI workerと調整する。`servers.yaml`を唯一のport定義に保つ。
- `camera` の TCP 5050/5052/6060 および `projector` が接続する 5000/8020 のリテラル、Android reverse 転送、scripts と projector Scene の既定値を調べる。`game_master/docker/compose.yaml` の既存 `8020:8020` mapping もSSoT対象にする。拡張子とCompose lifecycleを変えず、service port mappingを`# BEGIN GENERATED SERVER PORTS` / `# END GENERATED SERVER PORTS`の厳密な範囲に限定する。generatorと`--check`はこのblockだけを更新・検査し、既存のbuild-context変更などは保持する。
- shared generatorはYAMLのlimited structureを厳格に読み、duplicate keyと未知の形式をエラーにして最小のC# port constantsを生成・commitする。camera Gradle BuildConfigとprojector resolverは生成定数を参照する。ports用mise generate/`--check` taskとAndroid/Unity CI drift checkを追加し、port defaultの二重管理をなくす。
- projector serialized portでは`0`だけを「YAMLから生成された既定値を使う」sentinelとし、resolverでgenerated constantsに解決する。repository Sceneの元の既定値5000/8020だけを`0`へ移行し、意図的なcustom port overrideを保持する。正のInspector/CLI/env値は明示overrideとして優先する。Unity Editor起動時にPython generatorを自動実行せず、spec変更時のmise generate/check手順をREADMEに記す。
- 生成前後のdiffを確認して既存未コミット生成変更を保持する。Android build、Unity compile/edit-mode、Android/Unity CI drift checkを実施する。

## 検証・受け入れ条件

| 対象 | 受け入れ条件 |
|---|---|
| #336 | 複数着弾が個々の `x,y,z` 行に出る。回帰テスト、`SQLX_OFFLINE=true cargo check --all-targets -p ball_tracker`、`cargo test -p ball_tracker`、clippy、fmt と設計資料の更新。 |
| #299 | 新しい環境で uv sync 後に README の主要 CLI が実行でき、依存が lockfile から再現する。README、AGENTS、mise/Dependabot 設定が矛盾しない。利用可能な script syntax / help 検証を行う。 |
| #308 | run IDを重複防止に使い、古いfailure・後続in-progress・後続cancel・成功の順序を含む状態遷移を確かめる。初回失敗で1 Issue、同一連続障害でコメント、失敗より後の確定successでcloseする。固定markerと最小権限を使い、外部Issueは操作せずworkflow syntaxを検査する。 |
| #320 | `release-tracker`を追加し、CIがball_trackerをそのprofileでbuild/artifact化する。touchpanelは通常releaseのまま、`run_pc1.py`と関係docsは専用binary pathへそろえる。今回の性能比較は行わず、性能効果は未評価・後日の測定対象と明記する。 |
| #347 | client/依存元からserver/依存先へ、依存層内は並列停止し、全結果を観測する。ball_tracker/touchpanelはSIGTERMでgraceful shutdownし、既存Ctrl-Cとflushを保つ。 |
| #172 | Kotlin/C#既定portとCompose service port blockがYAML由来で、mise `--check` とAndroid/Unity CI drift checkが生成漏れを検出する。Unity serialized `0` sentinelをYAML既定値へ解決し、明示override/custom Scene値を保つ。Android/Unity実機検証ができなければ明記する。 |

利用できない SDK / 実機 / test environment に依存する確認は成功と記載せず、未実施と理由を報告する。GitHub Actions の実際の再実行やリリース公開は行わない。

## 主なリスク

- 作業前から同じファイルに未コミット差分があり、単純な revert / 全置換はユーザー作業を失う。必ず既存内容を保持した差分編集にする。
- Release workflow の失敗集約は job skip と cancellation を取り違えると誤って Issue を閉じる。安全側に状態を判定する。
- #320 の性能効果は今回測定しない。専用profile buildが通っても性能向上は未確認として報告し、既存 runtime optimization の測定値を結果に含めない。
- #347 は launcher 変更と POSIX signal の双方に影響する。SIGTERM を受けて即プロセスを終わらせず、非同期作業を停止し、書き込み待ちを回収する。
- #172 のポートのうち projector/camera の複数 transport が launcher の通常起動以外でも使われる。特定の手動・録画再生経路を置き去りにしない。
