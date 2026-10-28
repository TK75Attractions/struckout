
## ゲーム開始イベント

`GameStarted.player_info` の Single / Multi から、セッションのプレイヤー ID と
自分の machine ID が両方一致する参加者を探し、該当する場合だけプレイ中へ進む。
プレイヤー情報の必須フィールドが欠けたイベントはエラーとして扱う。
開始時はイベントの難易度を反映し、得点を 0、残り時間を `game_dur_secs` で初期化する。
以後の残り時間と得点は `GameTimeLimitNotify` と `GameScoreChanged` で更新する。
