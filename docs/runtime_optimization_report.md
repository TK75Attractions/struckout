# 同時起動の軽量化と連続運用の調査・計測

対象: PC①・PC②の役割をビルド済みプログラムで同時に動かす構成。目標は AC 電源で 1 日 8〜12 時間。
作業日: 2026-10-28。実装計画は `runtime_optimization_plan.md`、Session/Player API 統合計画は `api_session_migration_plan.md`。

## 調査と優先順位

調査したのは tracker の入力・復号・画像変換・検出・追跡・出力・記録、Android の送信と端末内解析、
projector の描画・イベント配送・演出・接続、game_master の状態管理・DB・通知、touchpanel、launcher、起動スクリプト。
以下はコードとログから確認した候補であり、未計測の効果は数値にしない。

| 優先度 | 所見・根拠 | 対応と理由 |
|---|---|---|
| 最優先 | 新 Session/Player API と旧クライアント・生成設定が不整合。server に未実装と未定義参照 | server、touchpanel、projector のクライアントと生成コードを最新契約へ統合。新 API を使った PC①・②同時起動でも結合確認 |
| 高 | projector は VSync 0、FPS 上限なし。実測で約 300 fps | 既定 60 fps と低頻度の実 FPS 統計を実装。同一ビルドで `-targetFps 0` と比較 |
| 高 | 無制限描画の比較試行で GPU device reset、Unity native crash、touchpanel swap error、復号停止が同時発生 | ログを保存。60 fps の完走だけで GPU 障害の根治とは判断しない。ドライバーや電源設定は変更していない |
| 高 | `run_pc1.py` の `-j` 常時指定。JSON を File に細かく書き込む | 診断 JSON を `--json` で任意化。動画の `--record` とは独立 |
| 高 | JSON 無効時も詳細な追跡イベントが既定 TRACE ログに出る | 通常 INFO、`RUST_LOG` で詳細化。着弾通知は INFO へ上げ、件数と統計・警告を残す |
| 高 | Recorder の無制限キュー。遅いディスクで複製された動画がたまる | カメラごとに 8 MiB・240 パケットまで。過負荷では録画のみ一度停止し、解析を待たせない |
| 高 | Compose 移動後の起動・ビルド参照が古い | `game_master/docker/` に追従。既存 DB ボリュームと project 名を保持 |
| 要対応 | game_master との切断中に終了イベントを失うと projector が Playing のままになる | projector は再接続後に Session イベント購読を再開する。切断中に発生した終了の状態回復は game_master 側の現在状態取得を含む別課題として残る |
| 要対応 | 共有パケット読み取りが外部の u32 長に応じて確保し、上限がない | 通常負荷の原因とは未確認。異常入力への容量上限は追加検討事項。Recorder の 8 MiB はこの受信バッファまで制限しない |
| 低 | Unity MainThreadDispatcher が無上限 ConcurrentQueue と毎 Update の Stopwatch を使用 | 通常の投球頻度で滞留の証拠なし。今回の FPS 削減で処理回数も減る。イベントを勝手に破棄しない |
| 低 | 命中の波紋を都度 Instantiate/Destroy、ターゲットの色・位置を毎フレーム更新 | 寿命のあるオブジェクト。現時点で支配的な割り当て・リークの証拠なし。先に FPS を制限 |
| 低 | Android 内の解析 fallback と OpenCV の一時割り当て | 端末によって 120 fps に間に合わない既存記録あり。通常の PC 解析とは別。実機 profiling 前に検出ロジックを変更しない |
| 低 | Docker VM のメモリ、DB/launcher の定期監視 | VM は過去約 1.8 GB。監視は数秒間隔で、削減効果の根拠なし。未計測のメモリ上限で DB を不安定にしない |
| 運用 | 通常ログ・DB の保持期間がなく、録画は約 45 GB/時 | 動画・診断 JSON は通常無効。ディスク容量と記録の保存期間を運用条件に含める。ユーザーの過去データは削除しない |
| 検証課題 | Unity の座標変換 JSON の既存テスト 2 件が失敗 | 係数の実フィールドは camelCase、失敗した入力は旧 PascalCase。今回の描画設定とは別に記録し、テストを無効化しない |

## 実装した負荷対策

- projector は描画品質・ゲームの速度・得点規則を維持し、上限だけを 60 fps にする。
  `-targetFps 0` は比較用の無制限、正整数は明示的な上書き。不正値は終了コード 1。
  約 30 秒ごとに rendered frame 数 / 実時間を記録する。最大 Update 間隔は GPU 処理時間ではない。
- INFO の解析統計・着弾・接続・警告は残す。詳細なフレーム記録は `run_pc1.py --json`、
  動画再解析に必要な組み合わせは `--record --json`。`RUST_LOG` の不正値は起動前に終了コード 2。
- Recorder の予算は書き込み中を含むペイロードのバイト数。キュー管理領域、既存の受信バッファ、ffmpeg、
  2 個の 1 MiB 書き込みバッファまで含めたプロセス総メモリ上限ではない。
  過負荷後は encode/clone をせず、受理済みの連続した先頭部分だけを書いて閉じる。
  OS のディスク書き込みが無期限に停止したときの終了時 join まで時間制限する変更ではない。
- 計測用の再送は全動画をメモリに載せず、位置の索引と 1 パケットずつの読み込みを使う。
  `--repeat` では接続し直すため、実機の接続が連続する試験とは区別する。

Unity は desktop では VSync による pacing を勧めているが、90/120/144 Hz の画面でも描画負荷を 60 fps に抑えるため、
今回は既存の VSync 0 と FPS 上限を組み合わせる。表示の滑らかさは実機プロジェクターでも確認する必要がある。
参考: [Unity Application.targetFrameRate](https://docs.unity.com/en-us/engine/6000.6/script-reference/unityengine/application/targetframerate)。

## 過去の測定との関係

`docs/setup_single_pc.md` 9 章と、2026-10-24 の `colocate_bench.py` / `runs/*` の生データを確認した。
過去は x64 projector を Arm 上で動かした試行と native Arm64 の試行があり、電池・他アプリ・ゲーム中の条件も異なる。
そのスクリプトは tracker に `-j` を付けていない。今回の変更前後と混ぜて改善率を計算しない。

| 過去の条件 | 結果 | 解釈 |
|---|---|---|
| 2026-10-24 全部同時、100 秒 | 着弾 22/22、追いつき 0、到着後の遅延中央値 約 34 ms | 短時間の基準 |
| 同日、ゲーム反復 約 460 秒 | 着弾 24/24、追いつき 1、中央値 約 39 ms、全 CPU 約 25%、GPU 3D 約 15% | 8〜12 時間の保証ではない |
| 別の Unity Editor 併用 | 100 秒で追いつき 12 | 他アプリとの GPU 競合の影響 |
| CPU の LLM サーバー併用 | 投球と競合し着弾欠落 | コア数だけでは必要性能を決められない |
| 2026-10-26 実機ログ 約 21 分 | 2 台とも約 120 fps、drop 0、到着後の遅延中央値 約 23/32 ms | 全モジュール同時稼働かはログから確定できない |

旧 raw データの所在は今回の計測フォルダーに記録する。旧ビルドのハッシュも保存する。

## 今回の比較条件

Snapdragon X Elite X1E-78-100、12 コア、RAM 約 32 GB、Adreno X1-85、Windows Arm64。
AC 接続後に測定。ffmpeg 9.0.2 x64、tracker と projector は native Arm64、描画 1920×1080 のウィンドウ。
スマホと外部プロジェクターは未接続。同一の 2026-10-16 録画、15:46:50 から 100 秒、2 台・120 fps を実際の間隔で再送。
測定中はコンパイラと Unity Editor を終了。背景アプリは採取するがユーザーのアプリを強制終了していない。

初期比較は旧 API の game_master イメージと touchpanel の既存ビルドを使用した。
新 API に統合した全体の結果とは区別する。同一の新 tracker / projector で FPS とログ設定を切り替えた。

| 初期試行 | 条件 | 100 秒区間の結果 |
|---|---|---|
| controlled_a1 | 無制限 FPS、TRACE、JSON | 着弾は 22 件まで届いたが、後半に GPU reset と native crash。未完走 |
| controlled_b1 | 60 fps、INFO、JSON なし | 着弾 22/22、追いつき 0、完走 |
| diagnostics_60 | 60 fps、TRACE、JSON | 着弾 22/22、追いつき 0、完走。診断 JSON 758,050 bytes |

2026-10-28 は最新 Session/Player API を含むビルドで同じ 100 秒再生を再実施した。最初の全体試行では別 Unity Editor が同時に動作し、空きメモリも少なかったため、再試行と分けて扱う。

| 最新 API 試行 | 条件 | 結果 |
|---|---|---|
| latest_api_60 | 最新 API、60 fps、INFO、診断 JSON なし。Unity Editor の同時稼働あり | 21/22、追いつき警告 3 回、完走。GPU 3D 平均 11.62%、空きメモリ最小 2.4 GiB。競合を含む結果 |
| latest_api_60_repeat | 同じ設定で再試行 | 22/22、追いつき警告 1 回、完走。GPU 3D 平均 11.88%、空きメモリ最小 3.1 GiB |
| latest_api_60_4core | tracker、projector、touchpanel のプロセス affinity を4論理 CPUに制限 | 22/22、追いつき 0、各カメラ約120 fps、drop 0、完走。GPU 3D 平均 11.14%、空きメモリ最小 3.9 GiB |

主要プロセスの CPU 使用コア数（CPU 秒 / 実時間秒）と projector の GPU 3D 使用率は次のとおり。`controlled_a1` は旧無制限条件、ほかは60 fps。単位は CPU がコア、GPU が %。

| 試行 | tracker CPU | ffmpeg CPU | projector CPU | projector GPU 3D 平均 |
|---|---:|---:|---:|---:|
| controlled_a1 | 0.302 | 0.559 | 0.573 | 38.05 |
| controlled_b1 | 0.308 | 0.544 | 0.206 | 12.58 |
| latest_api_60 | 0.305 | 0.553 | 0.211 | 11.62 |
| latest_api_60_repeat | 0.308 | 0.507 | 0.195 | 11.88 |
| latest_api_60_4core | 0.302 | 0.492 | 0.214 | 11.14 |

`controlled_a1` の GPU 3D 平均 38.05% に対して、最新 API の無競合再試行は 11.88%（約69%低下）、4論理 CPU affinity 試行は11.14%だった。これは FPS 制限と通常ログ化など複数変更をまとめた前後比較で、FPS 上限単独の因果効果ではない。旧試行は TRACE/JSON を有効にしており GPU reset と異常終了も起きているため、厳密な A/B 測定ではない。

Projector CPU は旧無制限/TRACE/JSON 試行の約0.573コアから、最新試行の約0.195〜0.214コアへ約63〜66%減少した。tracker と ffmpeg の CPU 使用量は同程度だった。全試行の所要時間は各100秒で、8〜12時間運転の安定性を示すものではない。

最新 API 試行で観測した各プロセスの private memory は tracker 約17 MiB、ffmpeg 約190 MiB、projector 約734 MiB、touchpanel 約134 MiB。Docker Desktop/VM、game_master と DB、Windows、他の常駐アプリはこの合計に含まれない。全体の空きメモリは Unity Editor 併用時2.4 GiB、再試行3.1 GiB、4論理 CPU affinity 時3.9 GiBだったため、PC①/②個別の16GB RAM目安は Docker と周辺アプリの余裕を考慮した推奨開始点である。

最新 API の実機能確認では、player 登録から Session 開始、SINGLE/NORMAL 設定、Ready、得点加算、完了後の GetGameResult 取得まで通った。game_id 5 では同時 Ready の2要求のうち1件のみ受理、もう1件は FailedPrecondition、異なる game_id の得点要求は拒否され、最終得点9を取得した。MULTI は未実装応答を返し、状態を壊さないことを確認した。

GPU reset のログは `Device removed reason (887a0007)`。Microsoft の定義では `DXGI_ERROR_DEVICE_RESET`。
ログは GPU ローカルメモリ不足を否定している。原因を FPS だけに断定できず、ドライバー/エンジンの切り分けが残る。
参考: [Microsoft DXGI_ERROR](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/dxgi-error)。

生データ、実行コマンド、実行ファイルのハッシュ、CPU/RSS/private memory、GPU カウンター、失敗した試行のログは
`recordings/2026-10-27_runtime/` と `recordings/2026-10-28_runtime/` に保存する。CPU は CPU 秒 / 壁時計秒の使用コア数。
RSS は共有ページを含むためプロセス合計を物理メモリ使用量と同一視しない。
Windows private は commit、Linux private は resident なので OS 間の同一指標として比較しない。
10 秒窓の遅延 p95 の平均や中央値は、全フレームの p95 ではない。

## 検証状況

初期の tracker release ビルド、全ターゲット check、既存 60 テスト、fmt は成功。
clippy は終了 0 だが、変更していない SentryEventLogger の Default と analyze_loop の引数数に既存警告が 2 件ある。
`-D warnings` はこの 2 件で失敗した。警告を無効化していない。
Unity Arm64 のビルド、最新 API を含む game_master ARM64 Docker image、touchpanel と ball_tracker の release ビルドは成功。game_master のサービス単体テスト4件も成功。テストコンパイルを塞いでいた health test の未定義 `TestService::NAME` を、登録済みのサービス名に合わせた。Unity EditMode テストは 119/121 成功。失敗は `CollisionTransformStoreTests` の旧 PascalCase JSON 入力を期待する2件で、今回の描画/API 変更とは独立している。失敗を無効化していない。
無効 FPS、RUST_LOG の未設定/空/対象指定/不正値/非 Unicode、録画の正常読み戻しを実行確認した。

Recorder へ上限を超える 5 MiB payload（複製後は 8 MiB 超）を 1,000 回渡した実行検証では、警告 1 回、
受理済み config 1 件だけの 14-byte `.vsp` が読み戻せた。停止後に渡した正常パケットも記録再開しなかった。
実ディスクの故障や無期限 I/O 停止を再現した試験ではない。

計測スクリプトは Windows と、既存 Linux コンテナの `/proc` で実行確認済み。最新 API の gRPC 結合確認と100秒同時起動の反復は成功。長時間 soak test、外部スマートフォン・実プロジェクターを接続した計測、低スペックの別 PC での実測は未実施。

## 長時間運用向け PC スペック目安

以下は今回の負荷と各モジュールの要件から決めた購入・選定時の推奨開始点であり、別の低スペック PC で合格を確認した「保証最低値」ではない。現在の計測機は Snapdragon X Elite X1E-78-100、12コア、RAM約32GB、Adreno X1-85。100秒の4論理 CPU affinity 試験は通ったが、同じ機械上の affinity 制限で、Docker/game_master と GPU、RAM、電力・熱条件は制限していない。別 CPU/GPU/メモリ構成にそのまま換算できない。

| 構成 | 推奨開始スペック | 選定時の確認 |
|---|---|---|
| PC① tracker/映像処理 | 4物理コア以上・8スレッド以上、RAM 16GB、SSD空き20GB以上、2ストリーム同時の H.264 1280×720 120fps hardware decode 対応 GPU/iGPU | FFmpeg のビルドとドライバーが D3D11VA などの hardware acceleration を実際に利用できること。対応 codec/profile と同時ストリーム数を現物のカメラ映像で確認 |
| PC② projector/game_master/touchpanel | 4物理コア以上・8スレッド以上、RAM 16GB、SSD空き20GB以上、Unity が使える DX11 以上対応 GPU/iGPU | 実プロジェクターの解像度で Player を60fpsで継続描画でき、Docker/DBを同時起動してもメモリに余裕があること |
| 1台へ集約 | 8物理コア以上、RAM 32GB、両カメラの hardware decode と projector 描画を同時処理できる GPU、SSD空き50GB以上 | この構成の実測 soak test が済むまで現場投入の保証値にしない |

Unity 6 の Player は Windows で x86/x64/Arm64 と DX10/11/12 または Vulkan 対応 GPU、ベンダーの正式ドライバーを要件としている。これは起動要件であり、対象解像度での60fps保証ではない。[Unity 6 system requirements](https://docs.unity3d.com/6000.0/Documentation/Manual/system-requirements.html)。FFmpeg は D3D11VA、DXVA2、QSV 等の hardware decode 経路を提供するが、利用可能な codec/profile は GPU とビルドに依存する。[FFmpeg hardware acceleration options](https://www.ffmpeg.org/ffmpeg.html)。

動画を保存すると過去実測の約45GB/時で、8〜12時間に360〜540GBを使う。1勤務分の動画を残すなら1TBの空き領域を目安とし、通常運用では録画と詳細 JSON を無効にする。これはアプリの出力だけの概算で、Windows、Docker イメージ、ログ、他データの空き容量は別途必要。

どちらの PC も AC 電源、冷却を塞がない設置、安定した有線 Ethernet を推奨する。録画なしの20GB/PCはアプリ稼働と更新用の運用余白であり、OS自体の必要容量は含めない。Windows と Unity の要件を満たしても、1日8〜12時間の安定性は soak test と熱・ネットワーク・USB 周りの確認が必要。
