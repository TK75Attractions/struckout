# game-master

## アーキテクチャ

### XxxViewModel
XxxViewModelTraitを実装する

### XxxViewModelRc
new()でXxxViewModelを作ってregister_viewmodel()を呼ぶ

### XxxDestination
load(route: &NavRoute)でrouteの引数をXxxViewModel::on_load()に渡す

## game_master との通信

プレイヤー操作は `GameMasterPlayerService`、ゲーム開始・イベント購読・結果取得は
`GameMasterSessionService` を使う。どちらも同じ gRPC 接続上に作り、データ層でまとめて扱う。

難易度画面からゲームを開始するときは、先に `ListenEvents` を開き、`StartSession`、
`SetGameMode(SINGLE)`、`SetSessionDifficulty`、`SetSessionReady` の順で呼ぶ。
Playing 画面へ進むのは、Ready 応答の `game_id` と一致した `GameStarted` を受け取ってから。
各通知も payload 内の `game_id` で選別するため、再接続時に遅れて届く前のゲームの snapshot は表示に反映しない。

イベント stream が切れた場合は Playing 画面をエラーへ遷移する。tutorial と MULTI mode は
server が `UNIMPLEMENTED` を返すため、この UI からは呼ばない。
