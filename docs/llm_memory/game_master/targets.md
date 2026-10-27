# ターゲットの登録・削除と得点

`GameMasterSessionService` の `AddTarget` / `RemoveTarget` は、ゲーム実行中 (`Running`) のセッションで利用する。
`RunningState` はマシンごとの `machine_id` と `player_id` を持ち、ゲームの `mode`・`difficulty`・`score`・`targets` は
`SharedRunningState` にまとめて `Arc<RwLock<SharedRunningState>>` で保持する。
マルチプレイ開始時は両セッションが同じ共有状態を参照し、一方での変更は他方からも見える。
シングルプレイはゲームごとに独立した状態を持つ。
`owner` はターゲットとともに保存する。projector の `AddTarget` で作る的はモードを問わず
`Single.machine_id = machine_id` とし、PocketWatch も各マシンに `Single.machine_id = machine_id` で1個ずつ作る。
`All` は両マシンへの配信に使えるが、PocketWatch には使わない。
ターゲットは削除されるか、ゲームの共有状態が破棄されるまで保持する。

- `AddTarget`: `machine_id` と必須の `target_data` を受け取り、登録ごとに UUID v4 を生成して `target_id` として返す。要求に `owner` は含めない。`Single.machine_id = machine_id` を設定する。
- `RemoveTarget`: UUID 形式の `target_id` を受け取り、指定マシンが参加するゲームのターゲットを削除する。マルチプレイではどちらのマシンからも同じターゲットを削除できる。削除済み、未登録、別ゲームの ID は `NotFound` を返す。
- 必須項目の欠落や不正な UUID は `InvalidArgument`、セッションがない場合や実行中でない場合は `FailedPrecondition` を返す。

登録成功時は UUID・owner・形状を含む `TargetAdded`、削除成功時は UUID を含む
`TargetRemoved` を送信する。`Single` は所有マシンだけ、`All` は両マシンに配信する。
ゲーム終了時の一括 `TargetRemoved` は送信しない。projector は `GameFinished` でローカルの的を消す。

## 命中と得点

`HitTarget` は実行中のゲームに登録済みのターゲットを受け取り、Compass では `100000 / radius` の小数点以下を切り捨てた得点を加算する。
計算は `f64` で行い、累計は `RunningState.shared` の `score` (`u32`) に保存する。
Compass は命中後も登録を保持し、繰り返し命中できる。
PocketWatch は固定500点を加算し、元のターゲットを削除して同じ所有者・新しい UUID で追加する。
残り時間は変更しない。得点の加算と置き換えは同じロック内で行い、オーバーフロー時はどちらも変更しない。
ゲーム開始時は `GameStarted` の後に各マシンの PocketWatch の `TargetAdded` を通知する。
命中時は `GameScoreChanged`、所有マシンへの `TargetRemoved`、`TargetAdded` の順で通知する。
成功時は累計得点を `GameScoreChanged` で通知する。シングルプレイは要求したマシンに、マルチプレイは両マシンに通知する。
マルチプレイでは共有ターゲットへの命中をどちらのマシンからでも受け付け、同じ累計得点を更新する。

不正な UUID は `InvalidArgument`、未登録・削除済みのターゲットは `NotFound` を返す。
セッションがない場合や実行中でない場合、ターゲットの形状が未設定の場合、半径が0以下・非有限の場合は `FailedPrecondition` を返す。
1回の得点または累計得点が `u32` の範囲を超える場合は `OutOfRange` を返す。
エラー時は得点を変更せず、得点変更イベントも送信しない。

## 検証状況（2026-10-27）

`SQLX_OFFLINE=true cargo check --all-targets -p game_master`、`cargo test -p game_master`
（90件）、`cargo clippy --all-targets -p game_master -- -D warnings`、`cargo fmt --all --check` は成功した。
開始時の PocketWatch の生成数・所有者、命中時の得点・削除・再追加・配信範囲、
古い UUID の拒否、得点オーバーフロー時に状態を保持することをテストしている。
C# は `mise run proto` で再生成し、再実行で生成物が変わらないことを確認した。

PR #342 のマージでは、`GameStarted.game_dur_secs` の追加と別ブランチの配信テスト追加が重なり、
シングル・マルチプレイの期待値2か所でフィールドが不足してテストのコンパイルが失敗した。
期待値にも `GAME_DURATION` から変換した秒数を設定し、配信先とともにゲーム時間を検証する。
