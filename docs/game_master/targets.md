# ターゲットの登録と削除

`GameMasterSessionService` の `AddTarget` / `RemoveTarget` は、ゲーム実行中 (`Running`) のセッションで利用する。
シングルプレイ、マルチプレイともに、ターゲットは `machine_id` ごとのセッションに保持し、セッション終了時に破棄する。
`owner` はターゲットとともに保存し、別のマシンの登録情報には複製しない。

- `AddTarget`: `target`、`target_data`、`owner` を必須とし、登録ごとに UUID v4 を生成して `target_id` として返す。
- `RemoveTarget`: UUID 形式の `target_id` を受け取り、指定マシンのターゲットを削除する。削除済み、未登録、別マシンの ID は `NotFound` を返す。
- 必須項目の欠落や不正な UUID は `InvalidArgument`、セッションがない場合や実行中でない場合は `FailedPrecondition` を返す。

登録・削除ではイベントを送信しない。`HitTarget` による得点処理は別途実装する。

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
