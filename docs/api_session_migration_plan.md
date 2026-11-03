# Session / Player API の統合計画

作成: 2026-10-27。[同時起動の軽量化計画](runtime_optimization_plan.md)の追加作業。
ユーザーの「最新ソースの API 統合まで対応する」という指示を受け、旧ビルドと新ソースの混在を解消する。

## 目的と維持する動作

同一のソース状態から game_master、touchpanel、projector、ball_tracker を新しくビルドし、
名前の登録・再利用 → 難易度の選択 → 90 秒のゲーム → 得点の保存・表示 → 次のゲーム、を繰り返せるようにする。
game_master を状態の持ち主とし、projector は着弾ごとの得点差分を送り、touchpanel と projector は
同じ残り時間・累計得点・終了を受け取る。

既存の UI とゲームの規則を保つ。新しいチュートリアル画面、複数台モードの選択 UI、ランキング仕様は追加しない。
既に実装した既定 60 fps、INFO ログ、診断 JSON の任意化、録画キューの上限、起動経路の修正は保持する。

## 確認した不整合

| 層 | 現在の状態 | 必要な整合 |
|---|---|---|
| proto | `game_master/session.proto` と `player.proto` に分割され、package は `tk75attractions.struckout.game_master.v1` | この定義を唯一の契約とする |
| Ready | request が空で対象号機を特定できず、response も空で開始したゲームを特定できない | request に `machine_id`、response に確定した `game_id` を追加する |
| Event | game_id は各 child payload にあり、Event 直下にはない | server/client とも child の game_id を使う |
| server | Session/Player 分割途中。旧 running_games の参照、未定義変数、型 alias の誤り、`todo!()` が残る | Session 状態に基づく開始・加点・終了まで完成させる |
| touchpanel | 旧単一サービスと `StartGame` の応答 stream を前提にしている | Player/Session の両クライアントと独立した `ListenEvents` に適合する |
| projector | C# 生成物と gRPC client が旧 GameMasterService のまま | Session サービスの再生成と型・イベント参照を更新する |
| 生成経路 | `buf.gen.game-master.yaml` が削除済み `api/proto/game-master.proto` を指す | 現在の session.proto を入力にする |

`session.proto` の説明図には存在しない `SetSessionPlayer` があり、チュートリアル用の nested message は
`event_data` の oneof に含まれない。今回は存在しない wire 動作を補うための画面やイベントを追加しない。
未対応の tutorial RPC と MULTI mode は明示的な `UNIMPLEMENTED` を返し、`todo!()` で panic させない。
状態不正や不正な enum は `FAILED_PRECONDITION` / `INVALID_ARGUMENT` として返す。

## 決定した API と既存 UI の対応

| 既存操作・表示 | 使用する新 API / event |
|---|---|
| 初回の名前登録、名前重複の確認 | GameMasterPlayerService の AddPlayer / ValidatePlayerName |
| 既存プレイヤーの名前を使う | GameMasterPlayerService の GetPlayer |
| 難易度画面の「開始」 | 下記の single 開始手順を data 層で実行 |
| Playing への遷移 | Ready response の game_id と一致する GameStarted を受けて、その game_id を使う |
| 残り時間、累計得点 | 同じ game_id の GameTimeLimitNotify / GameScoreChanged |
| 終了後の得点表示 | GameFinished を受けて GetGameResult |
| 命中時の加点 | GameMasterSessionService の AddScore(machine_id, game_id, score_to_add) |

single の開始手順は次に固定する。

1. `ListenEvents(machine_id)` を開始し、RPC の初回応答が成立したことを確認する。
2. `StartSession(machine_id, player_id)` を呼ぶ。
3. `SetGameMode(SINGLE, machine_id)` を呼ぶ。
4. `SetSessionDifficulty(difficulty, machine_id)` を呼ぶ。
5. `SetSessionReady(machine_id)` を呼び、response の game_id を新しいゲームの ID として確定する。
6. 既に購読中の stream から、その game_id と一致する GameStarted を受け、既存の Playing 画面へ進む。

公開する touchpanel の `start_game(player_id, difficulty)` はこの手順を包み、既存 ViewModel の
画面遷移を維持してよい。wire の旧 StartGame RPC や旧 service を互換用に残さない。
開始待ちには期限を設け、stream が切れた場合も待ち続けず既存のエラー表示へ返す。

ListenEvents の初期 snapshot はネットワークで遅れて届く可能性がある。旧ゲームが終了して次の Ready が
成功した後でも、その旧 GameStarted が stream に残る場合があるため、単に「Ready の後に受けた最初の
GameStarted」を新ゲームとはみなさない。drain や待ち時間で排除せず、Ready が返した確定 ID と照合する。

## 実装の順序と所有範囲

計画の管理は planner、統合と計測は primary agent。API を先に確定し、クライアントはその契約に従う。
同じファイルを複数の担当が編集しない。以下の所有範囲の変更は primary agent が調整する。

### A. 契約と server（担当: server worker）

対象: `api/proto/game_master/session.proto`、`game_master/src/session.rs`、`game_master/src/services/`、
`game_master/src/main.rs`、必要な `lib.rs` と `data.rs`、既存の関係するテスト、server の説明文書。

- Ready request に `uint32 machine_id = 1`、Ready response に `uint32 game_id = 1` を追加し、
  response には DB 登録・Running への遷移が成功したゲームの ID を返す。
  古い説明図を実在する RPC と今回の対応範囲に直す。child event の既存フィールド番号は維持する。
  tutorial oneof や新しい RPC は追加しない。
- Session は少なくとも設定中 → 開始処理中（Starting）→ 稼働中（Running）の状態を表す。
  Running は GameId、machine/player、難易度、得点、終了時刻を保持する。
  ゲームの識別子を別の独立した running_games と二重に管理しない。
- Ready は設定・enum・状態を検証してから、DB await の前に Starting を予約する。
  同時の Ready からゲームを二重 INSERT しない。DB 失敗時は再試行できる設定状態へ戻す。
  parking_lot の lock を await をまたいで保持しない。
- Ready の予約から DB 登録・失敗時の復帰・Running への遷移・timer の設置までは、RPC handler の
  cancellation から独立した owned task で完結させる。handler はその結果を待ち、response を返す。
  client が DB await 中に RPC をキャンセルしても、予約だけが残る状態や、timer の無いゲームを作らない。
  DB 失敗時の復帰は予約した session にだけ適用し、別の session を巻き戻さない。
- StartSession は同じ号機の稼働中ゲームを上書きしない。まだ開始していない同じ player の設定をやり直す操作は
  再試行できるようにする。別 player の進行中 session を黙って置換しない。
- DB 登録に成功した GameId で Running に遷移し、GameStarted を公開する。
  90 秒のタイマーは経過した実時間に従い、毎秒の残り時間と最後の 0 を通知する。
  古いタイマーは GameId を照合し、新しいゲームへ加点・終了通知を適用しない。
- AddScore は対象 machine と稼働 GameId が両方一致することを検証する。加算は overflow を検出し、
  不正入力で panic や wraparound を起こさない。異なる号機の GameId に加点しない。
- 終了時は Running を Finishing に遷移して加点を止め、確定した最終得点を DB に保存する。
  DB 処理が返った後は成功・失敗のどちらでも、その machine_id **かつ** game_id が一致する Finishing の
  session だけを解放する。これは次のゲームを受け付けるための号機の解放であり、保存成功の宣言ではない。
  新しいゲームや別の状態の session を古い完了処理が消してはならない。
- 保存成功時だけ GameFinished を通知する。保存失敗時は machine_id・game_id・final_score と理由を warning に残し、
  stream へエラーを返す。GameFinished や成功した結果として通知せず、client は既存のエラー表示・再接続経路へ進む。
  DB エラー後に再試行や解除をする経路のない Finishing を残して、号機を永久に使用不能にしない。
- 保存エラーでは DB の更新が未適用の場合と、通信エラー等で適用結果を確認できない場合を区別できないことがある。
  GetGameResult は DB で確認できた状態だけを返し、未保存の得点を保存済みの結果として補わない。
  本計画では永続的な再送キュー・自動再保存は追加しないため、保存失敗したゲームの得点を後で回復できる保証はない。
  この限界は結果報告に残し、号機が次のゲームへ復帰できることと、失敗した結果の永続化を混同しない。
- broadcast に受信者がいない場合、遅い受信者が lag した場合、stream を client が閉じた場合を通常の通信状態として扱う。
  `send().unwrap()`、`expect("receiver dropped")`、lag の expect を除き、受信者のいない転送タスクを終了させる。
  受信 stream の初回応答が成立してから Ready を呼べるよう、購読登録を応答前に終える。
- `PlayerServer` の型 alias と health の service 名を実際の Player service に合わせる。
  tutorials / MULTI は状態変更前に明示的な未対応エラーを返す。

### B. touchpanel の data 層移行（担当: touchpanel worker）

対象: `touchpanel/src/data/game_master.rs`、`data/mod.rs`、必要な `presentation/difficulty_select.rs`・
`presentation/playing.rs`、既存の関係するテストと README。

- 同じ接続に Session / Player のクライアントを置き、既存の sealed trait を新 API に合わせる。
  API の変化だけを理由に ViewModel や Slint へ通信ロジックを移さない。
- 名前登録・検証・検索は Player service、開始・購読・結果取得は Session service へ分離する。
  `start_game` は上記の single 開始手順を実行する。
- Ready response で確定した game_id と一致する GameStarted を待つ。遅延した前ゲームの snapshot は無視する。
  以後の event も child game_id で選別し、無関係なゲームの通知で画面・得点を変えない。
  タイマーの 0、最初の得点 0、ゲーム終了と結果取得の順を既存画面へ伝える。
- 終了・stream エラー時に購読、watch の keep-alive、完了待ちタスクを終了させる。
  古いゲームの完了処理で次のゲームの Session を `None` にしない。
  受信者が UI 遷移で消えたことを異常な不変条件として unwrap しない。
- 初回応答や開始の失敗は既存の Fallback へ伝え、失敗した開始操作のローカル Session と購読を片付ける。
  新しい tutorial / mode 選択画面は足さない。

### C. C# 生成と projector（担当: projector worker）

対象: `buf.gen.game-master.yaml`、`projector/Assets/Scripts/ProtoBuf/Generated/` と対応する `.meta`、
新しい Difficulty/Event を参照する C#、`Infrastructure/Grpc/GrpcGameMasterClient.cs`、
関係する Fake と既存 EditMode テスト、projector の通信説明文書。

- session.proto から pinned protoc / remote grpc C# plugin で生成し直す。生成ファイルは手で修正しない。
  projector は Player RPC を使わないので、template は session.proto を入力にする。
  旧 GameMaster 生成物・metadata を残して古い service もコンパイルできる状態にしない。
- ball-tracker と game-master は生成先のディレクトリを共有するため、片方の生成時に出力全体を削除する設定は使わない。
  旧生成物だけを限定して除去する。新 metadata は Unity が生成したものを保存する。
- game-master の型を `Tk75Attractions.Struckout.GameMaster.V1` の生成 namespace に合わせる。
  ProjectorPacket / CollisionPoint 等は従来の ball-tracker namespace に残るため、一括の using 置換はしない。
- GameMasterSessionServiceClient を使い、GameStarted / Finished / TimeLimitNotify / ScoreChanged の
  game_id を各 payload から読む。機能と得点差分の送信を維持する。
- 接続喪失時は古い game_id で AddScore を送り続けない。以後の購読と cancellation を適切に片付け、
  既存の 2 秒間隔の再接続を保つ。

### D. 再接続の状態同期（A・B・C の依存点）

Session service の `ListenEvents` は、稼働中のゲームがあれば既存 3 イベントで初期状態を送る。
順序は GameStarted → GameTimeLimitNotify → GameScoreChanged とし、残り時間は保持した終了時刻から求める。
新しい snapshot RPC や UI は作らない。

購読の登録と初期状態取得は state lock と順序をそろえる。状態変更と nonblocking broadcast の公開も同じ
lock 区間で順序をそろえ、snapshot より古い score が後から届いて表示を巻き戻す状況を避ける。
lock 中のネットワーク送信・DB await は行わない。

projector は同じ GameId の GameStarted を冪等に扱い、稼働中の得点やターゲットを不要に初期化しない。
接続喪失時はローカルの Playing と加点を停止し、再購読で Running の通知があればサーバーの状態で復帰する。
稼働中 session がなければゲーム通知は無い。サーバー再起動などでゲームが消えたときも、切断前の Playing を永久に残さない。
touchpanel のゲーム内 stream エラーは既存のエラー画面へ伝える。自動の画面復元 UI は今回追加しない。

### E. fresh build と統合検証（担当: primary agent）

- `.dockerignore` に非 Rust の `camera`・`launcher` とローカルの `.claude`・`output`・`tmp` を追加し、
  OpenCV や作業生成物を game_master の build context へ送らない。
  Cargo workspace の member、`api/`、migration、offline metadata、mise lock は残す。
- `game_master/docker/Dockerfile` の release 最適化と任意の Sentry feature を独立させる。
  RELEASE_MODE は profile だけを決め、Sentry は明示的な指定があるときだけ有効にする。
  最適化ビルドに DSN を必須としない。既存の DSN 指定を feature 選択として使う最小変更でよく、
  新しい秘密情報の保存方法を追加しない。
  `game_master/mise.toml` の `--release` と `--sentry` も独立させ、引数の説明を合わせる。
- `.github/workflows/release.yaml` の Docker build は現状 RELEASE_MODE を渡していないため debug profile になる。
  release image 用の build-args に `RELEASE_MODE=true` を明示する。公開処理そのものは実行しない。
- A の proto 契約確定後に B と C を進める。Rust と C# の両生成物が揃ってから全体の fresh build を行う。
  既存の旧 server / touchpanel で動くことを新 API の動作確認に代用しない。
- `mise run proto` と `mise run proto:check` を実行する。削除済み proto / service への production code の参照を検索し、
  意図した文字列（履歴説明など）以外を残さない。
- 関係する Rust crate の all-targets check、fmt、clippy、既存の関係するテストを実行する。
  古い API 前提のテストは新フローへ更新し、失敗するテストを消したり ignore したりしない。
  既存の TODO テストは意図した検証を実装して有効なままにする。新規のテストファイル・不要な追加テストは作らず、
  snapshot、加点の検証、開始の冪等性などは既存テストの必要な assertion と実結合試験で確かめる。
- 全 SQL migration を含む game_master の Docker イメージを作り直す。SQL を変更する場合は
  sqlx offline metadata を正規の prepare で更新する。必要性のない schema migration は追加しない。
- 既存の DB ボリュームを保全し、検証用の独立した Compose project / volume で空 DB から起動・migration を確かめる。
  credentials は既存の個人設定から読み、コマンド出力や計測記録に書かない。
- tracker・touchpanel の release と projector の native Arm64 player を作り直し、同じソース状態の実行ファイルと image を記録する。
  一時的なビルド設定変更はユーザーの変更と区別して戻す。
- 実接続で player 登録・再利用、single 開始、加点、残り時間、終了、GetGameResult、次のゲームを確かめる。
  同時 Ready、DB 登録中の Ready RPC cancellation、別号機・古い GameId の加点、未対応 mode、
  購読者の切断・再購読、server 再起動も検証する。
  古い GameStarted が Ready の response 後に届く順序も、既存の fake stream を用いた検証で再現し、
  新しい game_id の GameStarted まで Playing へ進まないことを確認する。
  終了の保存失敗は既存の stub を用いて再現し、stream error が届いて GameFinished は届かないこと、
  該当する Finishing だけが解放されること、DB が回復した後に次のゲームを開始できることを確かめる。
  検証対象外の tutorial / MULTI UI は未対応と報告する。

## 合否と性能の再測定

- 新 Session / Player API の全モジュールがビルドでき、起動して health とゲームのフローが成立する。
- 同じゲームの開始・終了が各 client に一度の意味で適用され、90 秒後の保存得点が累計と一致する。
  続くゲームの開始・加点・終了も成功する。
- 終了の保存失敗では成功した GameFinished を送らず、warning と stream error で失敗を示す。
  該当ゲームの Finishing は解放するが、別 GameId の session は維持する。得点の自動再保存は検証成功と主張しない。
- 同時 Ready で DB に一つの game だけを作る。RPC cancellation 後も Starting が残留せず、
  成功したゲームには timer があり、失敗時は再試行できる。Ready response の ID と client が開始した ID が一致する。
  誤った machine/GameId、overflow、不正 enum はエラーで返す。
- 切断・lag・購読解除・非接続の受信者で panic せず、終了したゲームのタスクや channel を保持し続けない。
- 新 player 60 fps + 新 tracker INFO/JSON OFF + 新 touchpanel + 新 server/DB の同時起動で、
  既知の 100 秒・22 着弾の録画区間を再測定し、入力 fps・drop・遅延・CPU・メモリ・ログ量を記録する。
  必要な長めの試験と必要スペックの判断は軽量化計画の基準に従う。
- API 統合前に得た「FPS 0 は約 300 fps、1 試行で GPU reset `887a0007` と他モジュールの描画・復号停止」
  「FPS 60 は 100 秒で 22/22、drop なし」は参考結果として保存する。旧 server / touchpanel を含むため、
  最新ソース全体の検証成功とは表現しない。
- 不安定だった無制限描画を長時間再実行する必要はない。性能比較は安全に測れた区間と条件を明示する。

## 残るリスクと更新条件

- Starting の予約・DB 登録・復帰、timer の GameId 照合、終了時の保存失敗、購読初期状態の順序は重要な競合点。
  実装で矛盾が見つかった場合は依存する実装を止め、planner がこの計画を修正する。
- API の移行によりビルドする crate の上流 API 変更が新たに表面化した場合、既存 fork / mise / 設定を保持し、
  使用側を正しい現行 API に合わせる。動く環境への無関係な移行は行わない。
- 外部プロジェクター、実機スマホ、目標時間の連続運転、低スペック PC の実機を使わない計測はその限界を報告する。
  1 台の性能制限試験を別 CPU / GPU の保証として扱わない。

2026-10-28 の実装確認で、保存失敗時の session 解放の意味を明確化した。
`run_game` が `complete_game().await` の後に成功・失敗にかかわらず `remove_finished(machine_id, game_id)` を
呼ぶ方針は、この計画に適合する。照合対象は同じゲームの Finishing に限定し、保存失敗は成功通知に変換しない。
