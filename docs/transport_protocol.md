## game-masterとprojectorの間
game-master -> projectorは主に画面遷移のコマンドや描画に必要なデータ(e.g. ランキングのデータ)を送る。  
projector -> game-masterは得点が入ったときに更新する。  

選択肢としてはgRPC、Protobuf on WebSocket、Protobuf on TCPの3つがある。  
JSON on WebSocketなどを省いたのはJSONが型安全でないため。  
gRPCはUnityに導入しようとするとかなり地雷でgRPC特有の機能(e.g. 認証)が必要なわけでもないので、コストに比べてリターンがあまりない。  
WebSocketは@taichi765も@smallfishsetもミリしらなのでどうしても必要でない限りは避けたい。  
TCPはtracker -> projectorですでに使っており学習コストがない上、性能上も特に問題ないということでTCP on Protobufを使う。

## カメラとball_trackerの間
定義は `api/proto/camera__ball-tracker.proto`。どちらの接続も TCP で、パケットは「長さ (u32、リトルエンディアン) + Protobuf」。
アプリは `adb reverse` で PC のポートに繋ぐ (`scripts/adb-reverse.sh`)。

| ポート | 向き | パケット | 内容 |
|---|---|---|---|
| 6060 | ball_tracker → カメラ | `TcpServerPacket.camera_id` | 接続した直後に、カメラの番号を割り当てる |
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

projector が使うのは `ball-tracker__projector.proto` だけで、上の変更の影響を受けない。
