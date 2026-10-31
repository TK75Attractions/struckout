## game_master と projector の間

`api/proto/game_master/` の `GameMasterSessionService` を gRPC で呼び出す。
projector は `ListenEvents(machine_id)` のストリームで開始・残り時間・累計得点・終了と画面遷移通知を受け取る。
シングルプレイの `GameStarted` は準備完了した筐体にだけ通知する。
協力プレイでは両筐体の準備完了後、準備完了の順番にかかわらず、両筐体へ同じ難易度と両参加者の情報を一度ずつ通知する。
開始通知のプレイヤー情報を筐体ごとに選び、終了通知のゲーム ID で `GetGameResult` を呼ぶ。
ランキング画面では `GetRanking` と `GetGamesByPlayer` の結果を統合する。
各 projector は自分の盤面の的を `AddTarget(machine_id, target_data)` で登録し、
応答の UUID で `HitTarget` を通知する。所有者の設定と得点計算は game_master が行う。
移動時は `RemoveTarget` の後に新しい位置を登録する。
詳細は [projector の挙動](projector/projector_behavior.md) を参照。

## game_master と touchpanel の間

`SetGameMode` で Multi を選ぶと、相手の Multi セッションが選択済みの難易度を引き継ぐ（相手が準備完了でも同じ）。
引き継いだ場合だけ、要求元の筐体へ `SessionDifficultyChanged` を通知し、難易度選択画面とサーバーの状態を同期する。
引き継ぐ難易度がない場合、この RPC はイベントを通知しない。

## カメラとball_trackerの間
定義は `api/proto/camera__ball-tracker.proto`。どちらの接続も TCP で、パケットは「長さ (u32、リトルエンディアン) + Protobuf」。
アプリは `adb reverse` で PC のポートに繋ぐ (`scripts/adb-reverse.sh`)。

| ポート | 向き | パケット | 内容 |
|---|---|---|---|
| 6060 | カメラ → ball_tracker | `TcpClientPacket.Hello` | 接続した直後に名乗る (端末の識別子 `device_id` = ANDROID_ID) |
| 6060 | ball_tracker → カメラ | `TcpServerPacket.camera_id` | カメラの番号を割り当てる |
| 6060 | ball_tracker → カメラ | `TcpServerPacket.CameraSettings` | PC が持つ位置と向き、焦点距離、検出の範囲 (`--cameras` に書いた端末だけ)。カメラは自分の設定に入れる |
| 6060 | カメラ → ball_tracker | `TcpClientPacket.UpdateCameraPose` | カメラの位置と向き。接続時と、設定を変えたとき |
| 6060 | カメラ → ball_tracker | `TcpClientPacket.CameraInfo` | センサー時刻の時計の種類、行ごとの露光の時刻差、解析画像の大きさ、焦点距離など |
| 6060 | ball_tracker → カメラ | `TcpServerPacket.TimeSyncRequest` | 時刻合わせの要求。接続直後に 50ms おきに 8 回、そのあとは 0.5 秒おき |
| 6060 | カメラ → ball_tracker | `TcpClientPacket.TimeSyncResponse` | 要求を受け取った時刻と返事を送る時刻 (端末の単調な時計、ns) |
| 5050 | カメラ → ball_tracker | `DetectionsPacket` | 検出のあったフレームごとに、検出の視線と、フレームのセンサー時刻 |
| 5052 | ball_tracker → カメラ | `VideoStreamServerPacket` | 接続した直後に、動画を解析するか (`accepted`) を答える |
| 5052 | カメラ → ball_tracker | `VideoStreamPacket.config` | 動画と解析画像の大きさ、内部パラメータ、向き、範囲、解析の設定。最初と、設定を変えたとき |
| 5052 | カメラ → ball_tracker | `VideoStreamPacket.frame` | エンコーダーが出した H.264 (Annex B) をフレームごとに。センサー時刻つき |

5052 番は、カメラの解析を PC に任せるためのもの (高速撮影版のアプリだけが使う)。アプリは 2 秒おきに
つなぎに行き、ball_tracker が `accepted` を返したら、検出 (5050 番) を送るのをやめて動画を送る。
ball_tracker は ffmpeg でデコードしてアプリと同じ方法で解析し、アプリが送るのと同じ `DetectionsPacket` を作る
(`ball_tracker/src/video/`)。`adb reverse` は PC 側で誰も待ち受けていなくても接続を受けるので、
アプリは返事が 2 秒来なければ、PC は解析しないとみなして自分で解析する。

時刻合わせは、古いアプリと新しい ball_tracker、新しいアプリと古い ball_tracker のどちらの組み合わせでも動く。

- 古いアプリは 6060 番を読まないので要求に答えない。ball_tracker は返事が 2 秒来なければ要求をやめ、
  フレームが届いた時刻から時計のずれを見積もる (これまでと同じ方法)。
- 古い ball_tracker は要求を送らない。`CameraInfo` は中身のないパケットとして読み飛ばす。
  `DetectionsPacket.timestamp` (端末の時計での ms) は、今までどおり入れてある。

### 端末の見分け方と、つなぎ直し

- ball_tracker を `--cameras scripts/cameras.json` で起動すると、`Hello.device_id` が `cameras.json` の `device_id` と
  一致した端末に、書いた順の camera_id (0, 1, ...) を振る。つなぎ直しても、開く順番が変わっても同じ番号になる。
  位置と向きは `cameras.json` の値を使い (カメラが送る `UpdateCameraPose` は使わない)、`CameraSettings` でカメラにも送る。
  `device_id` は `scripts/camera_remote.py config` が端末から聞いて (`SET_CONFIG` の返事) `cameras.json` に書く。
- 一致しない端末と、名乗らない送信元 (古いアプリ、`scripts/recording_analysis` の再生用スクリプト) には、
  これまでどおり、つないだ順の番号 (`cameras.json` の台数から) を振る。ball_tracker は名乗るのを 0.5 秒待ってから番号を送る。
- 高速撮影版のアプリは、検出の送り先が Network のあいだ、ball_tracker とのつながりが切れたら 2 秒おきにつなぎ直す。
  ball_tracker は、つなぎ直したカメラの時計の差を測り直す。
- projector (5000 番) は、ball_tracker が起動したあといつつないでも、つなぎ直してもよい。つないでいる projector すべてに
  着弾を送り、1 台もつないでいないあいだの着弾は捨てる。projector は ball_tracker と切れたら 2 秒おきにつなぎ直す。

projector が使うのは `ball-tracker__projector.proto` だけで、上の変更の影響を受けない。
