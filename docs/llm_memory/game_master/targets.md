# ターゲットの登録・削除と得点

`GameMasterSessionService` の `AddTarget` / `RemoveTarget` は、ゲーム実行中 (`Running`) のセッションで利用する。
`RunningState` はマシンごとの `machine_id` と `player_id` を持ち、ゲームの `mode`・`difficulty`・`score`・`targets` は
`SharedRunningState` にまとめて `Arc<RwLock<SharedRunningState>>` で保持する。
マルチプレイ開始時は両セッションが同じ共有状態を参照し、一方での変更は他方からも見える。
シングルプレイはゲームごとに独立した状態を持つ。
`owner` はターゲットとともに保存する。ターゲットはゲームの共有状態が破棄されるまで保持する。

- `AddTarget`: `target`、`target_data`、`owner` を必須とし、登録ごとに UUID v4 を生成して `target_id` として返す。
- `RemoveTarget`: UUID 形式の `target_id` を受け取り、指定マシンが参加するゲームのターゲットを削除する。マルチプレイではどちらのマシンからも同じターゲットを削除できる。削除済み、未登録、別ゲームの ID は `NotFound` を返す。
- 必須項目の欠落や不正な UUID は `InvalidArgument`、セッションがない場合や実行中でない場合は `FailedPrecondition` を返す。

登録・削除ではイベントを送信しない。

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

## 関連する既存課題（issue 案）

以下は今回の実装範囲外として記録する。issue は未起票。

### [game-master] イベント順序テストを開始手順に合わせる

`event_stream_notifies_in_correct_order` は難易度設定直後に `GameStarted` を期待するが、
実際には `SessionDifficultyChanged` を受け取り失敗する。難易度変更イベントを検証し、
`SetSessionReady` でゲームを開始してから開始イベントと残り時間を検証する必要がある。

### [game-master] 2 番目のセッションを正しく削除する

`Sessions::remove` は 2 番目のスロットで ID が一致した場合も 1 番目のスロットを削除する。
2 番目のシングルプレイが終了すると、他方のセッションとターゲットを誤って破棄しうる。
スロットごとの削除と他方のセッションの保持を再現テストで確認して修正する必要がある。

### [game-master] 既存の整形と Clippy の指摘を解消する

既存コードには `HitTarget` の未使用引数、不要な `into()` や `return` などの警告がある。
また、ワークスペース全体の `cargo fmt --check` は `api/rust/proto/build.rs` と
`ball_tracker/src/bin/replay_video.rs` の既存の整形差分で失敗する。
対象モジュールごとに別の変更で整理する。
