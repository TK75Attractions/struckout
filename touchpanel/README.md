# touchpanel

受付用のタッチパネル。`game_master` のプレイヤーサービスとセッションサービスに接続する。

## 画面と通信

接続時に `ListenEvents(machine_id)` の購読を始め、ゲームをまたいで維持する。
プレイヤー名を登録または検索した後、シングルプレイか協力プレイを選ぶ。
モード確定時に `StartSession(player_id, machine_id)` と `SetGameMode` を送る。
難易度画面では `SetSessionDifficulty` を送り、サーバーの `SessionDifficultyChanged` で表示を同期する。
初期難易度は NORMAL。`START` で `SetSessionReady` を送った後は、`GameStarted` まで待機する。
協力プレイは両マシンの準備完了を待つ。

ゲーム中の残り時間と累計得点はサーバーのイベントから表示する。
`GameFinished` の後、touchpanel には得点・ランキングを表示しない。
「プロジェクターにランキングを表示」で `GoRankingScreen` を、
「ホームに戻る」で `GoHomeScreen` を送る。RPC が成功した場合だけ touchpanel も次の画面へ進む。
失敗した場合は画面上にエラーを出し、再試行できる。

現在の API にはセッション取消やモード再選択がないため、モード選択の送信後と準備完了後の戻る操作は無効にする。
チュートリアル画面は未実装。`game_master` の `GoRankingScreen` と `GoHomeScreen` も現時点では未実装で、
サーバーが対応するまで実際の projector の画面遷移はできない。
また、現行の `game_master` では協力プレイで片方が先に準備完了すると、
もう片方が後から難易度を設定したときに拒否される。
両端末がモード選択を終えてから準備完了に進む必要がある。
`GameStarted` も最後に準備完了した端末にだけ配信されるため、
先に待機した端末はゲーム開始を検知できない。

## アーキテクチャ

`data/` は gRPC 通信とサーバーから受け取ったセッション状態を持つ。
`presentation/` は画面ごとの ViewModel、`ui/` は Slint の画面を持つ。
各 `XxxDestination` は `load(route)` で ViewModel に画面引数を渡す。
