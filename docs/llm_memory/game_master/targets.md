# ターゲットの登録・削除と得点

`GameMasterSessionService` の `AddTarget` / `RemoveTarget` は、ゲーム実行中 (`Running`) のセッションで利用する。
`RunningState` はマシンごとの `machine_id` と `player_id` を持ち、ゲームの `mode`・`difficulty`・`score`・`targets` は
`SharedRunningState` にまとめて `Arc<RwLock<SharedRunningState>>` で保持する。
マルチプレイ開始時は両セッションが同じ共有状態を参照し、一方での変更は他方からも見える。
シングルプレイはゲームごとに独立した状態を持つ。
`owner` はターゲットとともに保存する。projector の `AddTarget` で作る的はモードを問わず
`Single.machine_id = machine_id` とし、game_master が内部で作る特殊な的だけ `All` にする。
ターゲットは削除されるか、ゲームの共有状態が破棄されるまで保持する。

- `AddTarget`: `machine_id` と必須の `target_data` を受け取り、登録ごとに UUID v4 を生成して `target_id` として返す。要求に `owner` は含めない。`Single.machine_id = machine_id` を設定する。
- `RemoveTarget`: UUID 形式の `target_id` を受け取り、指定マシンが参加するゲームのターゲットを削除する。マルチプレイではどちらのマシンからも同じターゲットを削除できる。削除済み、未登録、別ゲームの ID は `NotFound` を返す。
- 必須項目の欠落や不正な UUID は `InvalidArgument`、セッションがない場合や実行中でない場合は `FailedPrecondition` を返す。

登録成功時は UUID・owner・形状を含む `TargetAdded`、削除成功時は UUID を含む
`TargetRemoved` を送信する。`Single` は所有マシンだけ、`All` は両マシンに配信する。
ゲーム終了時の一括 `TargetRemoved` は送信しない。projector は `GameFinished` でローカルの的を消す。

## 命中と得点

`HitTarget` は実行中のゲームに登録済みのターゲットを受け取り、`100000 / radius` の小数点以下を切り捨てた得点を加算する。
計算は `f64` で行い、累計は `RunningState.shared` の `score` (`u32`) に保存する。
ターゲットは命中後も登録を保持し、繰り返し命中できる。
成功時は累計得点を `GameScoreChanged` で通知する。シングルプレイは要求したマシンに、マルチプレイは両マシンに通知する。
マルチプレイでは共有ターゲットへの命中をどちらのマシンからでも受け付け、同じ累計得点を更新する。

不正な UUID は `InvalidArgument`、未登録・削除済みのターゲットは `NotFound` を返す。
セッションがない場合や実行中でない場合、ターゲットの形状が未設定の場合、半径が0以下・非有限の場合は `FailedPrecondition` を返す。
1回の得点または累計得点が `u32` の範囲を超える場合は `OutOfRange` を返す。
エラー時は得点を変更せず、得点変更イベントも送信しない。

## 検証状況（2026-10-26）

`SQLX_OFFLINE=true cargo check --all-targets -p game_master` と `cargo test -p game_master`
（83 件）は成功した。追加・削除の配信範囲と、
終了時に一括削除通知を送らない契約をテストしている。
