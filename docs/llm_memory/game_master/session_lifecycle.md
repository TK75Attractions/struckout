# game_master

ゲームのセッションと得点をメモリ上の `Sessions` で管理し、セッション破棄時に結果を MySQL に保存する。

## セッションのライフサイクル

`StartSession` → `SetGameMode` → `SetSessionDifficulty` → `SetSessionReady` の順に設定する。
シングルプレイは準備完了時、マルチプレイは両マシンの準備完了時にゲームを開始する。
ゲーム開始時刻は UTC で保持する。この段階では `games` テーブルに書き込まない。

ゲーム終了時は `MachineSession::Running` から `Finished` に遷移し、得点を確定してから
`GameFinished` イベントを送信する。終了後の的の追加・削除や命中通知は受け付けない。
`GetGameResult(machine_id)` は、そのマシンの現在のセッションから確定済みの得点を返す。
セッションが存在しない場合やゲームが未終了の場合は `FailedPrecondition` を返す。
過去のゲーム結果を DB から取得する API は提供しない。

`CompleteSession(machine_id)` はゲーム終了後のみ受け付ける。

- シングルプレイでは、そのセッションを破棄して結果を保存する。
- マルチプレイでは、最初のマシンを `WaitingForSessionCompletion` に遷移させ、両セッションと結果を保持する。
  同じマシンからの再送はそのまま成功させる。もう一方のマシンからも呼ばれた時点で両セッションを破棄し、共有結果を1件保存する。
- 破棄済みセッションや設定中・プレイ中の呼び出しは `FailedPrecondition` を返す。

セッションは DB 保存前に破棄する。保存に失敗した場合は `Internal` を返し、セッションを復元しない。
プロセス終了時に残っているセッションは DB に保存されない。

## DB

`games` テーブルは破棄済みセッションの結果のみ保持する。`score` は必須で、`status` カラムは持たない。
マイグレーション `0002_save_completed_games.sql` は既存の `running` 行を削除し、終了済みの履歴を保持する。
SQLx のオフライン用メタデータは、マイグレーション適用済みの DB に対して
`mise run //game_master:prepare` で更新する。
