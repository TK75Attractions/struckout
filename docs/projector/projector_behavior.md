# プロジェクターの挙動

## ゲームの進行と得点

projector は `GameMasterSessionService.ListenEvents` を自分の `machine_id` で購読する。
定義は `api/proto/game_master/` にあり、ゲーム開始・残り時間・累計得点・終了は
それぞれ `GameStarted`、`GameTimeLimitNotify`、`GameScoreChanged`、`GameFinished` に従う。
ゲーム ID は送受信しない。チュートリアル通知による画面遷移は未実装。

初期盤面を接続前に用意し、`GameStarted` の後で各 projector が自分の盤面の的を登録する。
`AddTargetRequest` は `machine_id` と `target_data`（円の中心座標と半径）を持ち、
所有者は game_master がセッションのモードから決める。シングルは `Single.machine_id = machine_id`、
マルチは `All` とする。
座標・半径は当たり判定と同じ盤面ピクセル単位を使い、直径を半径として送らない。
応答の UUID とローカルの的を対応付け、登録応答が来るまではその的への命中を通知しない。
マルチプレイでも各 projector が自分の盤面を生成する。累計得点は両マシンで共有する。

命中時は `HitTarget(machine_id, target_id)` を送る。
得点は game_master が `100000 / radius` の小数点以下を切り捨てて加算し、
`GameScoreChanged.total_score` で通知する。projector はローカルで加算しない。
HUD は通知された累計得点と残り時間を表示する。

## 命中後の移動

ボールが的に当たった後に的が移動するのは、難易度を問わず共通。
命中の受理後、次の順に処理する。

1. `RemoveTarget` で元の UUID の登録を削除する。
2. 他の的を避けて移動先を選び、`AddTarget` で新しい位置を登録する。
3. 新しい UUID に対応付けを更新し、描画と当たり判定を移動先に切り替える。

同じ的の処理中に届いた追加の着弾は無視し、二重加点や削除・登録の競合を防ぐ。
命中通知に失敗した場合は移動しない。削除や再登録を確認できなかった的は、
サーバーとの対応が不明になるため、次のゲーム開始まで命中の送信を停止して警告を出す。
ゲーム終了・切断時は処理をキャンセルし、遅れて届いた応答で盤面や UUID を更新しない。
切断時は待機表示に戻る。購読は再接続するが、現在のセッションを取得する API がないため、
次の `GameStarted` まで命中を送信しない。

通常時は的が動かない難易度と的が動き続ける難易度を作るが、労力が足りない場合は前者を優先して作る。

## Fake モード

`FakeGameMasterClient` も的の登録・削除・命中とサーバー式による得点通知を提供する。
開始時に登録と得点をリセットし、90 秒で終了する。
実通信・マルチプレイの確認には本物の game_master を使う。
