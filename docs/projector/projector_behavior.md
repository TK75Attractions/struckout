# プロジェクターの挙動

## ゲームの進行と得点

projector は `GameMasterSessionService.ListenEvents` を自分の `machine_id` で購読する。
定義は `api/proto/game_master/` にあり、ゲーム開始・残り時間・累計得点・終了は
それぞれ `GameStarted`、`GameTimeLimitNotify`、`GameScoreChanged`、`GameFinished` に従う。
ゲームの進行状態 (`GamePhase`) と表示画面 (`ProjectorScreen`) は分けて管理する。
チュートリアル通知による画面遷移は未実装。

## 結果とランキングの画面遷移

1. `GameStarted` の Single / Multi に含まれる `PlayerInfo` から、自分の `machine_id` に
   対応する `player_id` を選んで保持し、PlayingScreen を表示する。
2. `GameFinished(game_id)` を受信すると命中送信を止めて ScoreScreen に切り替え、
   `GetGameResult(game_id)` で DB 保存済みの確定得点を取得して表示する。
   プレイ中の HUD の値を確定結果として流用しない。
3. 自分のマシンの `NavigateToRankingScreenRequested` を受信すると RankingScreen に切り替える。
   `GetRanking(max_item_num = 10)` と `GetGamesByPlayer(player_id)` を並行して取得し、
   `game_id` で重複を除いて得点の降順に統合する。同点の場合は `game_id` の昇順とする。
4. 自分のマシンの `NavigateToHomeScreenRequested` を受信すると HomeScreen に切り替え、
   結果表示用のゲーム ID とプレイヤー ID を破棄する。

統合後の得点上位 12 件を表示する。データが 12 件未満の場合は、存在する記録だけ表示する。
「あなた」の記録と今回の `game_id` を強調する。
12 行の固定レイアウトで一画面に収め、得点順に上から下へ並べる。
スクロールやページ切り替えは行わない。
API は履歴の全体順位を返さないため、統合後の行番号を全体順位として表示しない。
マルチプレイの同一ゲームが複数のランキング行に含まれる場合もゲーム ID 単位でまとめ、
本人の名前がランキング内にある場合はその行を優先する。

画面切り替え時に取得処理をキャンセルする。遅れて返った結果が HomeScreen や
次のゲームの画面を上書きしないよう、応答を適用する前にもキャンセルを確認する。
取得中は読み込み表示、取得失敗時はその画面上にエラーを表示する。
ランキングと本人履歴の片方だけ取得できた場合は、完全な統合結果として表示しない。
`CompleteSession` は現行 API に存在せず、projector からセッション解放は要求しない。

## 的の登録

初期配置の座標・形状・大きさだけを接続前に計算し、`GameStarted` の後で各 projector が
`AddTarget` を呼ぶ。`TargetAdded` で UUID と形状を受信したときにだけ `Target` を生成し、
描画・当たり判定に追加する。RPC の成功応答だけでは追加しない。
`AddTargetRequest` は `machine_id` と `target_data`（円の中心座標と半径）を持ち、
projector が要求した的の所有者は、モードを問わず `Single.machine_id = machine_id` になる。
`All` は game_master が内部で生成する特殊な的に使う。
座標・半径は当たり判定と同じ盤面ピクセル単位を使い、直径を半径として送らない。
`Target.Id` は `TargetAdded.target.target_id` の UUID そのもので、座標・形状・大きさとともに変更しない。
追加イベントの owner が自分のマシンの `Single` または `All` の場合に描画し、
ローカルの `Target.Ownership` にも `Single` / `All` を保持する。
UUID や円の形状が不正な通知は警告して無視する。
登録前の配置データは ID を持たない `TargetGeometry` として扱い、描画や当たり判定には渡さない。
ローカル独自の ID や `Target` と UUID の対応表は持たない。
マルチプレイでも各 projector が自分の盤面を生成する。累計得点は両マシンで共有する。

命中時は `HitTarget(machine_id, target_id)` を送る。
得点は game_master が `100000 / radius` の小数点以下を切り捨てて加算し、
`GameScoreChanged.total_score` で通知する。projector はローカルで加算しない。
HUD は通知された累計得点と残り時間を表示する。

## 命中後の移動

`Single` の的は、難易度を問わず命中後に別の場所へ生成し直す。
`All` の的は命中通知のみを送り、projector から `RemoveTarget` や再登録を要求しない。
削除は game_master がトリガーし、projector は `TargetRemoved` に従って反映する。

`Single` の命中を受理された後は、次の順に処理する。

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
終了時には Fake 内で結果とゲーム ID を保持し、取得 RPC も提供する。
結果画面を 5 秒表示した後にランキング遷移通知、その 5 秒後にホーム遷移通知を送る。
これは Fake が送る通知で、実通信時の画面遷移は常にサーバーの通知に従う。
実通信・マルチプレイの確認には本物の game_master を使う。

## 配信側との契約と実装範囲

ターゲットの追加・削除を状態と描画に反映する経路は、引き続き受信イベントに限定する。
game_master の追加・削除イベント配信は未実装のため、実通信での的の生成には配信側の実装が必要。
送信側は追加時に `Target` 内の UUID・owner・形状を通知し、削除時に対応する UUID を通知する。
ゲーム終了時や次のゲーム開始前にも、不要になった的の `TargetRemoved` を配信する。

現時点の game_master では `GetRanking` / `GetGamesByPlayer` / 画面遷移 RPC も未実装。
projector 側の取得・通知処理と Fake は対応済みだが、実サーバーでの一連の画面遷移には
配信側と取得 RPC の実装が必要。

現在の API には、再接続後に現在のターゲット一覧やプレイヤー情報を取得する機能がない。
切断中に取り逃したイベントの復元と状態の再同期は、今回の対象外とする。
