## game-masterとprojectorの間
game-master -> projectorは主に画面遷移のコマンドや描画に必要なデータ(e.g. ランキングのデータ)を送る。  
projector -> game-masterは得点が入ったときに更新する。  

選択肢としてはgRPC、Protobuf on WebSocket、Protobuf on TCPの3つがある。  
JSON on WebSocketなどを省いたのはJSONが型安全でないため。  
gRPCはUnityに導入しようとするとかなり地雷でgRPC特有の機能(e.g. 認証)が必要なわけでもないので、コストに比べてリターンがあまりない。  
WebSocketは@taichi765も@smallfishsetもミリしらなのでどうしても必要でない限りは避けたい。  
TCPはtracker -> projectorですでに使っており学習コストがない上、性能上も特に問題ないということでTCP on Protobufを使う。
