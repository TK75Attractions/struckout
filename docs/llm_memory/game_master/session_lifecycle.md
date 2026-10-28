# game_master

ゲームのセッションと得点をメモリ上の `Sessions` で管理し、ゲーム終了時に結果を MySQL に保存する。

## セッションのライフサイクル

`StartSession` → `SetGameMode` → `SetSessionDifficulty` → `SetSessionReady` の順に設定する。
シングルプレイは準備完了時、マルチプレイは両マシンの準備完了時にゲームを開始する。
`GameStarted.player_info` はプレイヤー ID・名前と対応するマシン ID を含む。
名前は DB から状態遷移前に取得し、失敗時は `Internal`、取得中に設定が変わった場合は `Aborted` を返して開始しない。
ゲーム開始時刻は UTC で保持する。この段階では `games` テーブルに書き込まない。

ゲーム終了時は `MachineSession::Running` から `Finished` に遷移し、得点を確定する。
保存中の的の追加・削除や命中通知は受け付けない。
`save_game()` が結果を DB に保存して返した `game_id` を `GameFinished` イベントに載せる。
終了時の一括 `TargetRemoved` は送信せず、projector は `GameFinished` でローカルの的を消す。
DB 保存後、通知前にセッションを破棄する。マルチプレイでは共有結果を1件保存し、両セッションを破棄する。
結果を画面に表示し終えるまでセッションを保持する処理や `CompleteSession` RPC は提供しない。

保存に失敗した場合もセッションを破棄し、イベントストリームで `Internal` を通知する。
この場合 `GameFinished` は送信しない。プロセス終了時に残っているゲームは DB に保存されない。

## 結果の取得

`GetGameResult(game_id)` は指定 ID の結果を DB から取得し、`total_score` を返す。
セッションの有無やマシンに依存せず、保存済みの結果を取得できる。
指定 ID が存在しない場合は `NotFound`、DB の問い合わせに失敗した場合は `Internal` を返す。

## DB

`games` テーブルは終了済みゲームのみ保持する。`score` は必須で、`status` カラムは持たない。
初期スキーマは `0001_init.sql` に統合済みで、旧スキーマからの移行は行わない。
各プレイヤーに対応するマシン ID も保存する。詳細は
[ランキングとプレイヤー別履歴](ranking.md) を参照。
SQLx のオフライン用メタデータは、マイグレーション適用済みの DB に対して
`mise run //game_master:prepare` で更新する。
