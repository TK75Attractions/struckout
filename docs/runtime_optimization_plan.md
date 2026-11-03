# 同時起動の軽量化と連続運用の改善計画

作成: 2026-10-27。対象は PC①・PC② の役割をビルド済みのプログラムで同時に動かす構成。
調査・実装・計測の担当を分けるための計画であり、完了した検証は結果報告に記す。

## 追加された作業（2026-10-27）

ユーザーの指示により、最新ソースの Session / Player API への統合まで対応する。
詳細と担当範囲は [API 統合計画](api_session_migration_plan.md) に記す。
API 統合前の性能試験は旧 server / touchpanel と新 player / tracker が混在する参考値として保存し、
同一ソースから全モジュールをビルドし直した後に統合動作と性能を測り直す。
既存 UI の single ゲームを維持し、新 tutorial / MULTI の UI は追加しない。

## 目的と期待する動作

- 2 台分の 640×360・120 fps の入力と着弾の判定を維持し、projector の不要な描画と診断記録の負荷を減らす。
- 録画先のディスクが処理に追いつかないときも、録画待ちのメモリを無制限に増やさず、検出・ゲームを継続する。
- 起動・測定を再現できる状態にし、過去の起動データと今回の同条件比較を区別して報告する。
- 必要な PC スペックを、確認できた構成・暫定の選定目安・追加の合否試験に分けて示す。

## 調査で確認したことと制約

| 対象 | 確認した状態 | 判断 |
|---|---|---|
| projector | `QualitySettings.asset` の現在の品質は Very Low、`vSyncCount: 0`。実行コードに `Application.targetFrameRate` の設定がない | 描画上限を最優先で比較する。描画品質やゲームの規則は変えない |
| ball_tracker | 既存のビット演算等の最適化後、通常の解析は約 0.5〜0.6 ms/frame。入力の遅れには追いつく処理がある | 検出アルゴリズムやフレームレートを根拠なく変更しない |
| 診断 JSON と詳細ログ | `run_pc1.py` が常に `-j` を指定。JSON を切ると `FmtEventLogger` が追跡イベントを TRACE へ出し、`main.rs` の既定も TRACE | JSON と詳細ログをそれぞれ任意にする。JSON を切るだけではログ出力先を移すため不十分 |
| 動画記録 | `video/recorder.rs` のキューは無制限。受信ごとに protobuf と H.264 の複製を入れる | 記録を有効にしたときの長時間メモリ増加を防ぐ |
| game_master / touchpanel / launcher | 監視は数秒間隔。現時点で大きな常時負荷源の根拠がない | DB のメモリ制限、監視頻度、購読構造の変更は対象にしない |
| 起動経路 | Compose が `game_master/docker/` へ移動した一方、`run_pc2.py`・mise タスク・build context/Dockerfile の参照が移動前 | 同時起動を確かめる前提として参照を修正する |

`docs/setup_single_pc.md` 9 章には、2026-10-24 の同時起動で 100 秒の着弾 22/22・遅延中央値 34 ms、
460 秒の着弾 24/24・中央値 39 ms・CPU 平均約 25%・GPU 3D 約 15% がある。
Arm64 版の別の回は他アプリの負荷条件が違い、着弾を落としているので、アーキテクチャ間の改善率には使えない。
`recordings/2026-10-26_1847/tracker.log` は約 21 分の実機入力で約 120 fps、報告上の drop は 0 だが、
全モジュール同時稼働の証拠はなく、今回の一対一の比較対象とは扱わない。
過去計測の `colocate_bench.py` と `runs/` はローカルの scratchpad に残っている。その起動コマンドは `-j` を
指定しておらず、上記の過去計測は診断 JSON を出さない条件だった。22 件の既知区間は `--from 15:46:50 --seconds 100`。
今回の比較コマンドと生データの所在も報告に残す。

現在の PC は Snapdragon X Elite 12 コア・約 32 GB。調査開始時にスマホは未接続、Docker は停止していた。
AC 電源の有無、背景アプリ、ビルドの CPU アーキテクチャ、解像度・画面数を各計測に記録する。
既存の `scripts/pc2.json` の未コミット変更と他の未追跡ファイルは保持する。
ドライバー更新、Docker/DB の置換、検出精度の変更、push・PR・issue の公開はこの計画に含めない。

## 実装の分担と順序

計画の所有者は planner、全体の実行・判断は primary agent。以下の worker は担当名であり、
同じファイルを複数の担当が同時に編集しない。環境や測定の都合で primary agent が担当を引き取ってよい。

### 1. 起動と計測の前提をそろえる（担当: startup worker / primary agent）

- 対象: `scripts/run_pc2.py`、`game_master/mise.toml`、`game_master/docker/compose.yaml`、関係する起動手順。
- Compose の配置に合わせて参照を直す。context はリポジトリのルート、Dockerfile はそこからのパスで指定する。
- 既存の DB ボリューム、パスワード、Compose project 名を維持する。計測用のゲームは既存のデータと識別できる名前を使う。
- baseline に使う既存の tracker・touchpanel・projector の版と実行ファイルを保存する。
  起動修正自体を負荷削減の効果に数えない。
- release 実行時にビルドツールを動かさない。現行 mise が要求バージョンより古い場合も、共有設定を下げて回避しない。

### 2. projector の描画を制御する（担当: projector worker）

- 対象: `projector/Assets/Scripts/Unity/` の起動設定・実行統計、および必要な登録箇所、`docs/projector/projector_rendering.md`。
- ビルド済みプレイヤーを既定 60 fps にする。Inspector のネットワーク設定とは別の責任として配置する。
- 同一ビルドで無制限と 60 fps を測れる、明示的なコマンドライン指定を用意する。
  0 などの明示値を無制限比較用に使い、通常の運用手順は 60 fps とする。無効な値は起動時に通知する。
- 既存の `vSyncCount: 0` と software pacing の組合せを基本案とする。Unity の公式資料は desktop では VSync を勧めるため、
  60 Hz 以外の画面でも上限を守る目的と、表示の滑らかさへの影響を文書に記す。
- 実 fps を長い間隔で記録し、毎フレームの文字列生成やファイル書き込みを増やさない。
  必要なら既存 dispatcher のキュー深さも同じ間隔で観測する。観測のためにイベントを捨てたりキュー仕様を変えたりしない。
- ゲームの時間・移動・得点は経過時間に従う既存仕様を維持する。30 fps などへの変更は今回の既定にしない。

公式資料: [Application.targetFrameRate](https://docs.unity.com/en-us/engine/6000.6/script-reference/unityengine/application/targetframerate)。
導入 API と利用中の Unity 6000.5 の対応は、実装時に公式資料・ローカル API で確認する。

### 3. 任意の記録による負荷と蓄積を抑える（担当: tracker worker）

- 対象: `scripts/run_pc1.py`、`scripts/pc1.json`、
  `ball_tracker/src/main.rs`、`ball_tracker/src/tracking/mod.rs`、
  `ball_tracker/src/video/recorder.rs`、呼び出し元の `video/transport.rs`、`ball_tracker/README.md`、`docs/setup_pc1.md`。
- 通常の運用では全フレームの診断 JSON を出さず、明示的なオプションで有効にする。
  `--json` と `--record` は別の選択にする。通常の INFO ログ・解析統計・着弾の記録は残す。
  `replay_video` の再解析には `frames_*` の時刻対応が必要なので、その用途には `--record --json` を案内する。
- `main.rs` の通常ログを既定 INFO にする。`FmtEventLogger` の詳細な追跡イベントの `trace!` は残し、
  通常はその文字列化・出力をフィルターで抑える。JSON の指定と標準出力のログレベルを連動させない。
- 詳細調査には、既に使用中の `tracing_subscriber::filter::Targets` の `FromStr` で `RUST_LOG` を読む。
  対応する指定は `info,ball_tracker::tracking=trace` のような target と level の構文とし、
  新しい依存・EnvFilter・独自のログ設定形式を増やさない。未設定、空文字、既定 level を含まない指定でも
  指定されていない target は INFO にする。既存の `h2`・`hyper`・`tower`・`tonic` の INFO を既定として維持する。
  `RUST_LOG` に target を明示した場合はその指定を優先する。解釈できない値や Unicode として読めない環境変数は、
  日本語の理由を標準エラーへ出し、通信を開始する前に非 0 で終了する。
- `tracking/mod.rs` の `detected collision` は現在 TRACE なので、着弾 1 件ごとのこの通知を INFO にする。
  `run_pc1.py` の件数表示・今回と既存の測定スクリプトが使うメッセージを維持する。
  大きい追跡イベント DTO の TRACE 出力を INFO に上げて代用しない。
- 容量削減の効果は、JSON 有無の実ファイルサイズと書き込み量で測る。
  `BufWriter` 化は任意化だけで効果が得られれば今回は実装しない。診断 ON の負荷が今回の目的を妨げる場合に計画を更新する。
- 動画の記録キューにはバイト数の上限を設ける（初期案: カメラごとに 8 MiB の待ちデータ）。
  パケット数だけで上限を表す場合は、最大パケット長が別途保証される必要がある。
- キューの上限または書き込み失敗では、その接続の録画を停止し、一度だけ理由を warning に出す。
  後続フレームの encode/clone をやめる。受信・解析のスレッドで書き込み待ちや worker の join をしない。
  録画途中のパケットを間引いて復号できないファイルを黙って作る動作にはしない。
- 既に受け付けたデータは worker が可能な範囲で順番に書いて閉じる。録画停止後の自動再開はしない。
  ディスクの書き込み自体が無期限に停止した場合の終了待ちは、上限の保証範囲と分けて明記する。

API の根拠: [tracing-subscriber Targets の公式資料](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/targets/struct.Targets.html)。
`FromStr` はカンマ区切りの target/level 指定を扱う。span の値を使う EnvFilter の構文までは対応対象にしない。

### 4. 再現可能な比較と報告（担当: primary agent）

- 対象: 標準ライブラリだけの測定スクリプト（`scripts/recording_analysis/`）、必要な OS 処理を置く
  `scripts/struckout_common.py`、分析の README、`docs/runtime_optimization_report.md`、`docs/setup_single_pc.md`。
- 同じ録画 `recordings/2026-10-16_experiment_3`、`poses_live.json`、同じ 120 fps の入力間隔と同じ区間を使う。
  記録を全てメモリに載せる既存 replay の使用量が比較を支配しないよう、逐次読み込みにする。
  長い運転の再現には同区間の繰り返しを用意する。
- 全モジュール同時起動で baseline と改善後を比較し、可能なら A/B を交互に 2 回以上行う。
  FPS 上限と JSON・詳細ログの効果は条件を分けて測り、複数変更後だけの差を個別の改善率として扱わない。
  過去の `-j` なしの baseline も既定 TRACE であり、JSON OFF とログ負荷 OFF は同義ではない。
- 各結果に日時、コミット/差分、実行コマンド、ビルド種別、入力区間、秒数、AC、背景負荷、完走/失敗を保存する。
  秘密情報はコマンド・メタデータへ含めない。
- 同じプロセスを継続した長めの試験を行い、開始直後・定常区間・終盤のメモリと解析遅延を比べる。
  録画の再接続を繰り返す場合は、その回数を示し、実機の連続した動画と同じ試験だとは記さない。
- tracker 再起動または動画接続の切断後に、projector が接続を回復し、着弾が再開することを確かめる。
  実 DB を壊す操作や既存ゲームの意図しない変更はしない。
- CPU デコードや制限した負荷の試験を追加できる場合は、ハードウェアの異なる低スペック PC そのものの確認と区別する。

## 検証項目と合否

| 項目 | 判定 |
|---|---|
| 起動 | Compose の解決した context/Dockerfile が実在し、DB の health と game_master 接続、tracker・projector・touchpanel の接続が成立する |
| FPS | 60 fps 指定時に実 fps が上限付近で推移し、命中表示・タイマー・的の動作に破綻がない。無制限条件と同解像度で CPU/GPU を比べる |
| 検出 | 同区間の baseline と比べ、着弾件数を落とさない。座標・順序も比較可能な場合は比較する |
| 入力の処理 | 各カメラ約 120 fps、定常時の drop/追いつき回数が増えない。10 秒窓の delay p95、最大、キュー深さを記録する |
| 遅延 | 改善前と同じ方式で集計する。10 秒窓の p95 の平均を全フレームの p95 と呼ばない。通常負荷で 100 ms を越える窓は原因を確認する |
| メモリ | CPU 秒/壁時計秒（使用コア数）、RSS と private memory を分けて測る。ウォームアップ後の終盤に継続した増加がないかを示す。短時間の横ばいだけでリークなしとは断言しない |
| 録画の過負荷 | 記録待ちが設計した上限を超えず、停止の warning があり、停止後も入力と解析が続く。正常時は記録の読み戻しが一致する |
| 記録量 | 同区間の診断 JSON のサイズ、通常ログ、任意録画の増加率を算出する。ログを無効にして性能だけ良く見せない |
| ログ設定 | 通常起動で 10 秒ごとの解析統計・着弾件数・warning が残り、大きな追跡イベントの TRACE が出ない。`RUST_LOG=info,ball_tracker::tracking=trace` では詳細が出る。無効な level 指定は通信前に非 0 で終了する |
| 復旧 | 切断後に接続し直し、次の共通区間で着弾を受け取れる |

変更した Rust は `SQLX_OFFLINE=true cargo check --all-targets -p ball_tracker`、関係する既存テスト、
`cargo clippy`、`cargo fmt --check` を確認する。テストの実行・追加は現在のユーザー指示と上位指示に従い、
必要な範囲に絞る。Unity は実プレイヤーの再ビルドと関係する既存 EditMode テストを確認する。
Python は構文・CLI・実計測を確認し、Linux で実行できなければその旨を記す。生成コードは変更しない。
ツールや環境の不足で確認できないものは、成功の扱いにせず、理由と再実行コマンドを残す。

## 結果報告と必要スペック

- 調査した改善候補を、採用・今回は見送り・実測待ちの理由とともに残す。
- 改善前後の数値、改善率、入力条件、測定時間、ログの場所、失敗を含む試行を示す。
- 2026-10-24 の過去記録との比較は参考値とし、今回の同条件 A/B と混ぜて因果を断定しない。
- PC①のみ、PC②のみ、1 台で全て、の選定目安を分ける。CPU のコア数だけで性能を保証せず、
  H.264 2 本のハードウェア復号、1920×1080 描画、RAM、SSD 空き、給電・冷却・USB・画面出力の条件を示す。
- RAM は各プロセスと Docker VM、OS の使用量に余裕を足す。現在の 16 GB/推奨 32 GB という経験的な目安を、
  実測なしに引き下げない。特定 CPU の最低仕様はその PC で合否試験を通すまで確定しない。
- 8〜12 時間または 24 時間の目標はユーザー回答を反映する。今回の試験時間を超える運転、実機スマホ・USB ハブ・
  外部プロジェクター・低スペック実機を未確認なら、必要な追加試験として明示する。

## 未確定事項と計画の更新条件

- 目標の連続運転時間、AC 接続、実機スマホ・外部画面を今回使えるかは、現状とユーザー回答で確定する。
- FPS 上限で描画の乱れや入力遅延が増える、JSON を有効にする既存の用途を壊す、録画上限で検出が止まる場合は、
  依存する実装を止め、planner がこの計画を修正する。
- 実測が新しい支配的なボトルネックを示した場合は、その原因を調べてから優先順位を更新する。
  根拠のないプール化、キュー破棄、メモリ制限、追加の抽象化は先に実装しない。
