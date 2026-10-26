# プロジェクターの挙動

## ゲームの進行と得点

projector は `GameMasterSessionService.ListenEvents` を自分の `machine_id` で購読する。
定義は `api/proto/game_master/` にあり、ゲーム開始・残り時間・累計得点・終了は
それぞれ `GameStarted`、`GameTimeLimitNotify`、`GameScoreChanged`、`GameFinished` に従う。
ゲーム ID は送受信しない。チュートリアル通知による画面遷移は未実装。

初期配置の座標・形状・大きさだけを接続前に計算し、`GameStarted` の後で各 projector が
`AddTarget` を呼ぶ。`TargetAdded` で UUID と形状を受信したときにだけ `Target` を生成し、
描画・当たり判定に追加する。RPC の成功応答だけでは追加しない。
`AddTargetRequest` は `machine_id` と `target_data`（円の中心座標と半径）を持ち、
所有者は game_master がセッションのモードから決める。シングルは `Single.machine_id = machine_id`、
マルチは `All` とする。
座標・半径は当たり判定と同じ盤面ピクセル単位を使い、直径を半径として送らない。
`Target.Id` は `TargetAdded.target_id` の UUID そのもので、座標・形状・大きさとともに変更しない。
追加イベントの `target.machine_id` が自分のマシンと一致する場合にだけ描画する。
UUID や円の形状が不正な通知は警告して無視する。
登録前の配置データは ID を持たない `TargetGeometry` として扱い、描画や当たり判定には渡さない。
ローカル独自の ID や `Target` と UUID の対応表は持たない。
マルチプレイでも各 projector が自分の盤面を生成する。累計得点は両マシンで共有する。

命中時は `HitTarget(machine_id, target_id)` を送る。
得点は game_master が `100000 / radius` の小数点以下を切り捨てて加算し、
`GameScoreChanged.total_score` で通知する。projector はローカルで加算しない。
HUD は通知された累計得点と残り時間を表示する。

## 命中後の移動

ボールが的に当たった後に的が移動するのは、難易度を問わず共通。
命中の受理後、次の順に処理する。

1. `RemoveTarget` で元の UUID の削除を要求する。
2. 削除要求が成功したら、元の的を除いて新しい配置を計算し、`AddTarget` で登録を要求する。
3. `TargetRemoved` を受信したときにだけ、元の `Target` と対応する描画を削除する。
4. `TargetAdded` を受信したときにだけ、新しい UUID の別エンティティと GameObject を生成する。

RPC 応答とイベントは別の経路で届く。イベントが先でも後でも、状態と描画の変更元は
常にイベントとする。元のエンティティの座標変更や同じ GameObject の再利用は行わない。
受信したイベントはすべて同じメインスレッドのキューで順序を保って適用する。

命中通知に失敗した場合は元の的を保持する。削除要求を出した後は、RPC 応答の成否にかかわらず
`TargetRemoved` を受信するまでその UUID への追加の命中送信を止める。
削除応答が不明な場合や再登録要求が失敗した場合も、ローカル判断で的を追加・削除しない。
重複した追加イベントと、存在しない UUID の削除イベントは状態を変更しない。

ゲーム終了・切断時は進行中の要求をキャンセルし、画面の表示状態だけを切り替える。
受信済みの的は保持し、これらの場合も削除は `TargetRemoved` に従う。
切断後の購読は再接続するが、次の `GameStarted` までは命中を送信しない。

通常時は的が動かない難易度と的が動き続ける難易度を作るが、労力が足りない場合は前者を優先して作る。

## Fake モード

`FakeGameMasterClient` も的の登録・削除・命中とサーバー式による得点通知を提供する。
追加・削除の成功時にはそれぞれ `TargetAdded` / `TargetRemoved` を通知する。
90 秒のゲーム終了時には、全ターゲットの削除通知を終了通知より先に送る。
切断時は削除通知を送らず、再接続時に前回のターゲットの削除通知を開始通知より先に送る。
実通信・マルチプレイの確認には本物の game_master を使う。

## 配信側との契約と実装範囲

今回の変更は projector と `TargetAdded.target_id` の proto 定義のみ。
game_master の追加・削除イベント配信は未実装のため、実通信での的の生成には配信側の実装が必要。
送信側は追加時に UUID・`machine_id`・形状を通知し、削除時に対応する UUID を通知する。
ゲーム終了時や次のゲーム開始前にも、不要になった的の `TargetRemoved` を配信する。

現在の API には、再接続後に現在のターゲット一覧を取得する機能がない。
切断中に取り逃したイベントの復元と状態の再同期は、今回の対象外とする。
