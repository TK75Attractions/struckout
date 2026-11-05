# プロジェクターの挙動

## ゲームの進行と得点

projector は `GameMasterSessionService.ListenEvents` を自分の `machine_id` で購読する。
定義は `api/proto/game_master/` にあり、ゲーム開始・残り時間・累計得点・終了は
それぞれ `GameStarted`、`GameTimeLimitNotify`、`GameScoreChanged`、`GameFinished` に従う。
`GameWillStart` を受信したら Playing 画面の前面に残り秒数を大きく表示し、
`GameStarted` でオーバーレイを消してゲームを開始する。
ゲームの進行状態 (`GamePhase`) と表示画面 (`ProjectorScreen`) は分けて管理する。
チュートリアル通知による画面遷移は未実装。

## 結果とランキングの画面遷移

1. `GameStarted.player_info` の Single / Multi に含まれるプレイヤーから、自分の `machine_id` に
   対応する `player_id` を選び、Single / Multi から求めたモードと開始時の難易度とともに保持し、PlayingScreen を表示する。
2. `GameFinished(game_id)` を受信すると命中送信を止めて ScoreScreen に切り替え、
   `GetGameResult(game_id)` で DB 保存済みの確定得点を取得して表示する。
   プレイ中の HUD の値を確定結果として流用しない。
   モード・難易度と SCORE ラベルを表示し、確定得点を約1秒で0から数え上げる。
   得点の表示中に、同じモード・難易度の `GetRanking(max_item_num = 10)` を取得し、
   今回のゲームが上位に含まれるときは「ランキング N 位！」を表示する。
   ランキングを取得できなくても得点表示は維持する。
3. 通常モードでは、得点の数え上げアニメーションが終わった1秒後に RankingScreen に切り替える。
   `GetRanking(max_item_num = 10, mode, difficulty)` と `GetGamesByPlayer(player_id, mode, difficulty)` を並行して取得し、
   `game_id` で重複を除いて得点の降順に統合する。同点の場合は `game_id` の昇順とする。
   `-simpleMode true` で起動した場合は RankingScreen に切り替えず、ScoreScreen でホーム要求を待つ。
4. 自分のマシンの `NavigateToHomeScreenRequested` を受信すると HomeScreen に切り替え、
   結果表示用のゲーム ID とプレイヤー ID を破棄する。

統合後の得点上位 12 件を表示する。データが 12 件未満の場合は、存在する記録だけ表示する。
「あなた」の記録と今回の `game_id` を強調する。
12 行の固定レイアウトで一画面に収め、得点順に上から下へ並べる。
スクロールやページ切り替えは行わない。
順位は本人履歴の各行で `GetGamesByPlayer` が返す全体順位を使う。
上位10件の行は Domain で同点を同順位（1、2、2、4）として計算する。
順位・プレイヤー・スコアの見出しと、モード・難易度を表示する。
1〜3位の順位と得点は金・銀・銅で強調する。2位の銀は投影時にも4位以降の白と区別できるよう、明度を抑えた青灰色にする。
サーバーのランキングは multi-play も1ゲーム1行とし、両プレイヤーの情報を含む。
ランキング・本人履歴とも参加者情報を使い、single-play は名前1人分、multi-play は
`player1 / player2` の形式で両名を表示する。どちらかの参加者が本人なら行を強調する。
同じプレイヤーが両マシンで参加した場合も両方の名前を表示する。
本人履歴の参加者情報は `GetGamesByPlayerResponse.GamesByPlayerItem.player_info` から取得する。
本人履歴の順位は `GetGamesByPlayerResponse.GamesByPlayerItem.rank` から取得する。

画面切り替え時に取得処理をキャンセルする。遅れて返った結果が HomeScreen や
次のゲームの画面を上書きしないよう、応答を適用する前にもキャンセルを確認する。
取得中は読み込み表示、取得失敗時はその画面上にエラーを表示する。
ランキングと本人履歴の片方だけ取得できた場合は、完全な統合結果として表示しない。
`CompleteSession` は現行 API に存在せず、projector からセッション解放は要求しない。

## 的の登録

初期配置の座標・形状・大きさだけを接続前に計算し、`GameStarted` の後で各 projector が
`AddTarget` を呼ぶ。`TargetAdded` で UUID と形状を受信したときにだけ `Target` を生成し、
描画・当たり判定に追加する。RPC の成功応答だけでは追加しない。
`AddTargetRequest` は `machine_id` と `target_data`（円の初期中心座標、半径、x/y 方向の移動速度）を持ち、
projector が要求した的の所有者は、モードを問わず `Single.machine_id = machine_id` になる。
`All` は game_master が内部で生成する特殊な的に使う。
座標・半径は当たり判定と同じ盤面ピクセル単位を使い、直径を半径として送らない。
`Target.Id` は `TargetAdded.target.target_id` の UUID そのもので、登録時の座標・速度・形状・大きさは変更しない。
追加イベントの owner が自分のマシンの `Single` または `All` の場合に描画し、
ローカルの `Target.Ownership` にも `Single` / `All` を保持する。
UUID や円の形状が不正な通知は警告して無視する。
登録前の配置データは ID を持たない `TargetGeometry` として扱い、描画や当たり判定には渡さない。
ローカル独自の ID や `Target` と UUID の対応表は持たない。
マルチプレイでも各 projector が自分の盤面を生成する。累計得点は両マシンで共有する。

命中時は `HitTarget(machine_id, target_id)` を送る。
得点は game_master が `100000 / radius` の小数点以下を切り捨てて加算し、
`GameScoreChanged.total_score` で通知する。`diff` は今回加算した点数で、
`score_reason` は `TargetHit(target_id)` または `Combo` とする。projector はローカルで加算しない。
HUD は通知された累計得点と残り時間を表示する。命中点は対象の的があった位置に、
コンボボーナスはコンボラベルのすぐ下に `+300` のように短時間表示する。

### コンボ

ゲーム開始時のコンボ数は 0。`HitTarget` が成功するたびに 1 増え、
`ComboChanged.combo` を累計得点と同じ配信先へ通知する。協力プレイでは両マシンで共有する。
球がどの的にも当たらなかったとき、projector は `ResetCombo(machine_id)` を呼ぶ。
リセット後は 0 を通知するが、既に 0 の場合は通知しない。ゲーム中でない場合は
`HitTarget` と同じく FailedPrecondition を返す。

到達時に加算するボーナスは game_master の `config.toml` の `combo_bonuses` で指定する。
ランチャーが作成する設定では、3 コンボ到達時に +500 点、5 コンボ到達時に +800 点を加算する。
リセット後に再びしきい値に達した場合も加点する。ボーナス込みの累計を
命中点とボーナスはそれぞれ別の `GameScoreChanged` で通知し、
ボーナス加算後の値を終了時に保存するため、結果・ランキングにも含まれる。
HUD は通知された値を `3 COMBO` の形式で表示し、2 未満では隠す。
Fake モードもコンボ通知・リセット・ボーナスに対応する。

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
常にイベントとする。命中後の再配置に元のエンティティの座標変更や同じ GameObject の再利用は行わない。
受信したイベントと着弾は同じメインスレッドのキューで順序を保って適用する。

命中通知に失敗した場合は元の的を保持する。削除要求を出した後は、RPC 応答の成否にかかわらず
`TargetRemoved` を受信するまでその UUID への追加の命中送信を止める。
削除応答が不明な場合や再登録要求が失敗した場合も、ローカル判断で的を追加・削除しない。
重複した追加イベントと、存在しない UUID の削除イベントは状態を変更しない。

ゲーム終了時は進行中の要求をキャンセルし、`GameFinished` を受けた projector が
残っているローカルの的と描画をすべて削除する。game_master は終了時に
残存する的の `TargetRemoved` を一括配信しない。
切断時は進行中の要求をキャンセルするが、受信済みの的は保持する。
切断後の購読は再接続するが、次の `GameStarted` までは命中を送信しない。

Compass の初期配置は難易度によらず同じとする。登録時に x/y 方向の速度
（盤面ピクセル/秒）を付ける。normal は `vx = vy = 0`、hard は合成速度 100 px/秒、
veryhard は合成速度 200 px/秒とする。hard と veryhard の方向は生成ごとにランダムに選ぶ。
Compass はゲーム中に動き続け、円の縁が盤面の端に届いたら反射する。
projector は通知された初期位置と速度から現在位置を計算し、同じ位置を描画と当たり判定で使う。
game_master は登録時の位置と速度を保存・配信し、移動中の位置は管理しない。
命中後に再生成する Compass には、その時点の難易度に従って新しい方向と速度を付ける。

## 懐中時計 (PocketWatch)

懐中時計は game_master が作成する。`TargetAdded.target.pocket_watch` は座標を持たず、
projector が受信時に既存の的を避ける場所を選ぶ。初期の Compass の追加通知より先に
届く場合は、準備済みの初期配置も避ける。配置には竜頭を含む画像全体の余白を確保する。
置ける余白がない場合は既存の配置処理と同様に、試した候補のうち重なりが最小の場所を使う。
同じ UUID の追加通知では位置を引き直さない。

文字盤の直径は UIService の Inspector の `Pocket Watch Diameter` で調整する
（既定 200 盤面ピクセル）。描画と円形の当たり判定で同じ値を使い、文字盤の外にある
竜頭は当たり判定に含めない。画像は `pocket_watch_face.png` と `pocket_watch_hand.png`
を同じ中心で重ねる。元画像の 1254px のキャンバスに対し、文字盤の外径は 930px として扱う。
シーンへの画像の再割り当てには `Tools → STRUCK OUT → Apply Pocket Watch Art` を使う。

`GameStarted.game_dur_secs` を初期の一周の時間とし、開始時に真上を指した針が時計回りに
360 度回って止まる。全ての時計は共通のゲーム時計を参照し、毎フレーム、単調増加の
実時間から角度を計算する。`GameTimeLimitNotify` の残り時間を受信すると基準を補正し、
次の通知までの間も滑らかに動かす。再出現時にはその時点のゲーム進行を引き継ぎ、
針を真上に戻さない。制限時間が 0 の開始通知では針を真上で止める。
`TotalDurationChanged` では通知されたゲーム全体の時間を一周の時間に更新し、
経過した時間を保って針の角度を計算し直す。前回の総制限時間より増えた場合は、
残り時間ラベルに増加分を `+5s` の形式で得点加算表示と同じ 1 秒間表示する。

命中時は owner が `Single` / `All` のどちらでも `HitTarget` だけを送る。
成功後は削除イベントまで同じ UUID への重複した命中送信を止める。
`RemoveTarget` / `AddTarget` は要求せず、game_master の `TargetRemoved` と
次の `TargetAdded` に従って別の UUID・位置・GameObject に置き換える。
得点や制限時間の変更は projector で計算しない。
game_master はゲーム開始通知の後、各マシンに `OwnerSingle` の懐中時計を1個ずつ追加する。
命中時は共有得点に固定500点を加算し、所有マシンへ削除・新しい UUID での追加を順に通知する。
命中時は game_master の必須設定 `pocket_watch_bonus_secs`（ランチャーの初期値は 5 秒）だけ
残り時間と終了時刻を延ばす。追加後の総制限時間は `TotalDurationChanged` で通知する。
`GameStarted.game_dur_secs` はサーバーが選択した難易度のゲーム時間を通知する。

## Fake モード

`FakeGameMasterClient` も的の登録・削除・命中とサーバー式による得点通知を提供する。
追加・削除の成功時にはそれぞれ `TargetAdded` / `TargetRemoved` を通知する。
Fake は開始時に懐中時計を 1 個生成し、命中時には削除・再追加を通知する。
懐中時計の命中時は Fake でも総制限時間を 5 秒延ばして通知する。
制限時間が尽きたときには、全ターゲットを Fake 内で破棄し、削除通知なしで `GameFinished` を送る。
切断時は削除通知を送らず、再接続時に前回のターゲットの削除通知を開始通知より先に送る。
終了時には Fake 内で結果とゲーム ID を保持し、取得 RPC も提供する。
結果表示からランキングへの遷移は実通信時と同様に projector が行う。
シンプルモードでは Fake モードでも ScoreScreen に留まる。
Fake はゲーム終了後、設定された待ち時間を経てホーム遷移通知を送る。
実通信・マルチプレイの確認には本物の game_master を使う。

## 配信側との契約と実装範囲

プレイ中にターゲットの追加・削除を状態と描画に反映する経路は、受信イベントに限定する。
game_master は追加時に `Target` 内の UUID・owner・形状を通知し、削除時に対応する
UUID を通知する。`Single` の的は所有マシンに、`All` の的は両マシンに配信する。
ゲーム終了時には残存する的の `TargetRemoved` を一括配信せず、projector が
`GameFinished` に合わせてローカルの的を消す。

game_master は `GetRanking` / `GetGamesByPlayer` に対応している。取得順と協力プレイの扱いは
[ランキングとプレイヤー別履歴](../llm_memory/game_master/ranking.md) を参照。
`GoHomeScreen` はセッション破棄後も指定マシンに画面遷移イベントを配信する。

現在の API には、再接続後に現在のターゲット一覧やプレイヤー情報を取得する機能がない。
切断中に取り逃したイベントの復元と状態の再同期は、今回の対象外とする。

## ログと筐体番号

通常のログには、接続に使う設定と同じ `machine_id` を付ける。
ローカルのログには `[machine_id=N]`、Sentry の構造化ログには数値属性 `machine_id`、
エラーイベントには検索用タグ `machine_id` を保存する。
`-machineId` / `STRUCKOUT_MACHINE_ID` による上書き後の値を使う。
