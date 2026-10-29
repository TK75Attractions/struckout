# projector のデバッグ環境

projector (Unity) は ball_tracker と game_master の 2 つにつながっている。
**通信方式が違う。** ball_tracker は生の TCP、game_master は gRPC。

| 相手 | 既定ポート | 方式 | 向き | 中身 |
|---|---|---|---|---|
| ball_tracker | 5000 | TCP | 受信 | `ProjectorPacket` (`CollisionPoint` / `TestMessage`) |
| game_master | **8020** | **gRPC** | 受信 | `ListenEvents` のストリーム (`Event`) |
| game_master | 8020 | gRPC | 送信 | `AddTarget` / `RemoveTarget` / `HitTarget`、結果・ランキング・本人履歴の取得 |

ポートの定義は `api/spec/servers.yaml` が唯一の出どころ。Rust は `build.rs` が
そこから読み、projector は `NetworkSettings` の既定値として持っている
(**両者は自動では揃わないので、spec を変えたら C# 側も直すこと**)。

TCP のフレーミングは LE u32 の長さ + protobuf で、実装は
`api/rust/src/lib.rs` の `write_packet` / `read_packet`。

## デバッグ用の対向

ball_tracker の代わりには `sandbox/testTcpCLI` を使える。
`dotnet run --project sandbox/testTcpCLI` で起動する。
game_master は本物を起動すれば、Fake モードを使わずにシリアライズと gRPC のストリームを確認できる。
描画やアニメーションだけを触るなら Fake モード (後述) を使う。

現在の game_master は `TargetAdded` / `TargetRemoved` の配信が未実装。
本物との接続で的を表示するには配信側の実装が必要で、Fake モードはこれらのイベントを通知する。

## 座標系と得点の約束事

- `CollisionPoint` は**物理座標 (m)**。ball_tracker は的の面の上での着弾位置を送る。
  `x` が面に沿った横方向 (世界座標の y)、`y` が高さ (世界座標の z)
  (`ball_tracker/src/collision_output/network.rs`: `x = coll.y, y = coll.z`)。
- projector は `GameStarted` 後に自分の盤面の的を `AddTarget` で登録し、`TargetAdded` の UUID と形状から的を生成・描画する。
- 命中時は `HitTarget` を送り、game_master が半径から得点を計算する。
  HUD は `GameScoreChanged.total_score` を表示する。ゲーム ID や得点差分は送らない。
- 命中後は `RemoveTarget` で元の的を削除し、`TargetRemoved` で描画を削除し、`TargetAdded` で新しい UUID の別エンティティを生成する。
  詳細は [projector_behavior.md](projector_behavior.md) を参照。

### 物理座標 → 盤面座標の変換

`TargetGenerator` は盤面 (既定 1920x1080) に的を置くので、受け取った物理座標を
そこへ写す必要がある。変換は `SensorProvider` が行い、係数は
`GameLifetimeScope` の Inspector (`Collision Transform`) で調整できる。

| 項目 | 意味 | 既定値 |
|---|---|---|
| `Pixels Per Metre X` / `Y` | 1 m あたりの盤面座標 | 960 / 540 |
| `Origin X` / `Y` | 物理座標 (0, 0) が来る盤面座標 | 960 / 0 |
| `Flip X` / `Flip Y` | 軸の向きが逆のとき | off |
| `Swap Axes` | 縦横が入れ替わっているとき | off |
| `Log Conversions` | 変換の前後をログに出す (合わせ込み用) | off |

**既定値は実測に基づいていない。** 物理 x[-1, 1] m / y[0, 2] m がちょうど 1920x1080 に
収まる、というだけの仮置きなので、盤面の実寸を測って必ず入れ直すこと。
合わせ込むときは `Log Conversions` を on にすると、送った物理座標と変換結果が並んで出る。

#### 書き出したプレイヤーで係数を変える

Inspector の値は既定値で、起動時に設定ファイル `collision_transform.json` があれば
その値で上書きする (`CollisionTransformStore`)。ファイルの場所は次の順で決まる。

| 指定 | 例 |
|---|---|
| コマンドライン | `StruckOut.exe -collisionTransform C:\struckout\collision_transform.json` |
| 環境変数 | `STRUCKOUT_COLLISION_TRANSFORM` |
| 既定 | `<dirs::config_dir()>/struckout/projector/collision_transform.json` (Editor とプレイヤーで共通) |

既定の設定ディレクトリは launcher・ball_tracker と同じ規則で決まる。

| OS | 設定ディレクトリ |
|---|---|
| Linux | `$XDG_CONFIG_HOME/struckout/projector` (未設定・空・相対パスなら `~/.config/struckout/projector`) |
| Windows | `%APPDATA%\struckout\projector` |
| macOS | `~/Library/Application Support/struckout/projector` |

F2 の「保存」と起動時の読み込みは同じパスを使い、保存時にディレクトリがなければ作る。
従来の `Application.persistentDataPath` にある設定は自動では移さないため、既存の
`collision_transform.json` を上のディレクトリへ移すか、コマンドライン・環境変数でそのファイルを指定する。
projector 独自の保存処理はこの設定ファイルだけで、データディレクトリへ保存するファイルは現在ない。
Unity 自身が出力する `Player.log` は Unity の保存先に従い、起動時の `-logFile` で変更できる。

ファイルが無ければ Inspector の値のまま動く。中身がおかしい (1 m あたりが 0 以下など) ときも
Inspector の値を使い、`Player.log` に `[Collision] could not read ...` と出る。

実行中は **F2** で調整画面 (`CollisionCalibrationPanel`) が開く。変えた値はすぐ着弾の表示に効く。

- 係数を直接打ち込む。入力欄から Esc で抜けると、矢印キーで原点を 1 px、Ctrl+矢印キーで
  1 m あたりを 0.5% 動かせる (Shift で 10 倍)
- 「板の 4 隅と中央に印を出す」: 実測した板の点 (左の壁から 0.07 / 1.13 m、地面から 1.79 / 2.47 m と、その中央)
  に印を出し続ける。印が板の角に乗るまで係数を動かす
- 「合わせ用の 2 点に印を出す」: 盤面の (480, 270) と (1440, 810) に印を出す。それぞれが板の上で映った位置
  (左の壁からの距離, 地面からの高さ。m) を測って入れ、「2 点から係数を計算して入れる」を押すと係数が決まる
  (向きが逆なら `Flip` も立つ)
- 「保存」で設定ファイルに書く。次の起動からその値を使う。「ファイルから読み直す」「Inspector の値に戻す」もある

盤面座標からワールド座標への変換はさらに後段で、
`docs/projector_rendering.md` に書いてある。

> ball_tracker は以前 `CollisionPoint` に世界座標の `(x, z)` を詰めていた。的の面は x = 0 なので
> 横方向がつねに 0 になっていたが、`(y, z)` を送るように直してある。`Swap Axes` は不要になった。

## 1. 対向なしで起動する (Fake モード)

`GameLifetimeScope` の Inspector で `Network Settings > Mode` を `Fake` にするか、
`-networkMode fake` を渡すと、通信を一切せずに起動できる。
描画やアニメーションだけを触りたいときはこれが最も早い。

- `FakeClientService` がランダムな衝突点 (物理座標、既定 x[-1,1] y[0,2] m) を流す
- `FakeGameMasterClient` が接続直後に `GameStarted(NORMAL)` を、
  90 秒後にゲーム ID 付き `GameFinished` を流す。結果取得後、5 秒おきにランキング・ホームへの遷移通知を送る
- `FakeGameMasterClient` が的の UUID と半径を保持し、本物と同じ式で得点を計算して通知する

## 2. 偽 ball_tracker につなぐ (testTcpCLI)

```bash
dotnet run --project sandbox/testTcpCLI
```

```
> listen sensor          # ball_tracker のかわりに 5000 で待つ
                         # ここで Unity を Play する
> hit 0 1                # 物理座標 (m) で撃つ。既定係数なら画面中央
> hit                    # ランダムな座標に撃つ
> msg hello              # TestMessage を送る
> auto 500               # 500ms ごとに撃ち続ける
> auto off
> status
> exit
```

**`hit` の引数はメートルであってピクセルではない。** 盤面のどこに当たるかは
Collision Transform の係数しだいで、既定値なら `hit 0 1` が画面中央、
`hit -1 0` が左下、`hit 1 2` が右上になる。

ポートを変えたいときは `listen sensor 6000` のように第 2 引数で指定する。

## 3. 手元の偽 game_master を使う場合

追跡対象外の `sandbox/fake_game_master` は旧サービス向けの開発用ツールだった。
現在の `api/proto/game_master/` の契約への対応は、このリポジトリでは保証しない。
対向なしの確認には Fake モード、実通信の確認には本物の game_master を使う。

projector の `Network Settings > Machine Id` と、touchpanel がセッションを開始する
`machine_id` を一致させる。イベントはマシンごとに配信される。
projector をセッション開始前に接続すること。途中から購読しても開始通知は再送されない。

## 4. 本物の game_master を使う

```bash
mise run //game_master:dev   # docker compose (app + MySQL)
```

> **既知の問題**: `game_master/compose.yaml` が公開しているのは `8080:8080` だが、
> バイナリが listen するのは `api/spec/servers.yaml` 由来の **8020**。
> このままではホストから届かない。

## 5. 別マシンの ball_tracker につなぐ

`docs/machine_separation.md` のとおり Unity と Android Studio は別マシンで動かすので、
接続先は Inspector 以外からも変えられるようにしてある。優先順位は
**コマンドライン > 環境変数 > Inspector**。

```bash
# ビルド済みプレイヤー
projector.exe -trackerHost 192.168.0.10 -masterHost 192.168.0.11

# 環境変数でも可 (Unity Editor から起動する場合はこちらが楽)
STRUCKOUT_TRACKER_HOST=192.168.0.10
STRUCKOUT_NETWORK_MODE=fake
```

使えるキー:

| コマンドライン | 環境変数 | 既定値 |
|---|---|---|
| `-networkMode` | `STRUCKOUT_NETWORK_MODE` | `Real` |
| `-trackerHost` | `STRUCKOUT_TRACKER_HOST` | `127.0.0.1` |
| `-trackerPort` | `STRUCKOUT_TRACKER_PORT` | `5000` |
| `-masterHost` | `STRUCKOUT_MASTER_HOST` | `127.0.0.1` |
| `-masterPort` | `STRUCKOUT_MASTER_PORT` | **`8020`** |
| `-machineId` | `STRUCKOUT_MACHINE_ID` | `1` |
| `-connectAttempts` | `STRUCKOUT_CONNECT_ATTEMPTS` | `5` |

ball_tracker への接続は指数バックオフ (間隔は 5 秒まで) で `connectAttempts` 回まで再試行するので、
Unity を先に Play してから対向を立ち上げても間に合う (`scripts/run_pc2.py` は 100000 回を渡して待ち続けさせる)。
つながったあとで ball_tracker と切れたら、2 秒おきにつなぎ直す (projector を起動し直さなくてよい)。
game_master のイベントの購読も、切れたら 2 秒おきにやり直す (`GrpcGameMasterClient`)。
gRPC のチャネルは遅延接続なので、game_master は後から立てても繋がる。

## 6. protobuf のコード生成

`api/proto` の `.proto` が唯一の定義。Rust は各クレートの `build.rs`、Kotlin は Gradle が
自動生成するが、**C# だけは生成結果をコミットしている**。手で protoc を叩かないこと。

```bash
mise run //api:proto          # 生成してコミット対象を更新
mise run //api:proto:check    # ズレていないかだけ確認 (exit 1 で失敗)
```

`mise run` はタスク実行前に未導入ツールを入れようとして dotnet で失敗することがある。
その場合は `MISE_TASK_RUN_AUTO_INSTALL=0` を付ける。

生成先は `projector/Assets/Scripts/ProtoBuf/Generated` と
`sandbox/testTcpCLI/Network/Protocol/Generated` の 2 か所。
テンプレートが 2 つに分かれているのは、buf のプラグインが inputs 全体に
一律で適用されるため (`buf.gen.game-master.yaml` の先頭に理由がある)。
`testTcpCLI` には game-master の生成物を渡していない。偽 ball_tracker としてしか
使わないうえ、gRPC スタブが `Grpc.Core.Api` を参照していないぶんビルドを壊すため。

CI では `.github/workflows/ci.yaml` が変更パスに応じて `proto_ci.yml` を呼び出し、
生成コードに差分があれば自動でコミットして push する。

## 7. ログの出力

Application・Infrastructure・Bootstrap と Unity の画面コンポーネントは
`Microsoft.Extensions.Logging.ILogger<T>` を使う。Domain にロギングの依存は置かない。
通常の C# クラスではコンストラクター、Scene のコンポーネントでは `[Inject]` の初期化メソッドで受け取る。
動的に作る UI は生成元からロガーを渡し、初期化より先に設定する。

`GameLifetimeScope` は `ILogger<T>` → `ProjectorLogger<T>` を VContainer に singleton として登録する。
カテゴリごとのロガーは同じ `IExternalScopeProvider` を共有し、Sentry の `IHub` には
既存 SDK へ接続する `HubAdapter` を使う。コンテナー構築前の設定解決と終了時のログも同じ経路を通る。
ロガーは SDK を初期化・終了せず、SDK の寿命は Sentry Unity に任せる。

### Console と Sentry への出力

Console と `Player.log` にはカテゴリ（型の完全修飾名）と整形済み本文を出す。
Trace / Debug / Information は通常ログ、Warning は警告、Error / Critical はエラーになる。
None は出力しない。例外が付いていれば詳細とスタックトレースも表示する。
Sentry が無効でもローカル出力は行う。直接 `Debug.Log*` を呼ぶのは `UnityLogOutput` だけとする。

Sentry Logs には同じログを、対応するレベル（Critical は Fatal）で属性付きで送る。
Error / Critical は例外の有無にかかわらず `IHub.CaptureEvent` で Issues のイベントも作る。
例外が付く場合は文字列化だけでなく例外オブジェクトも渡す。
Warning 以下は Logs のみで、Issues のイベントは作らない。

`SentryOptions.asset` では Unity ログの構造化ログ収集、Breadcrumb 収集、
`Debug.LogError` のイベント化を無効にしている。`ILogger` から Console へ出したログを
再収集して、属性のないログや重複したイベントを作らないため。
Unity の例外ハンドラーとネイティブクラッシュ収集は維持する。
このため、外部コードの `Debug.LogException` は引き続き例外イベントとして収集される。
`SetBeforeSendLog` は属性追加や送信フィルターに使い、Console 出力には使わない。

### 属性とスコープ

値は `logger.LogWarning("的がありません: {TargetId}", id)` のようにメッセージテンプレートで渡す。
例外は `LogError(exception, "処理に失敗しました。")` の例外引数へ渡す。
Sentry の本文は `ILogger` の formatter で一度だけ整形し、元のテンプレートは
`sentry.message.template` 属性に保持する。名前付きプロパティは同じキーの属性として送る。
追加メタデータは `logger.category`、`event.id`、`event.name`、例外の
`exception.type` / `exception.message` / `exception.stacktrace`。
Issues にも同じ情報を Extra として付ける。

`BeginScope` に渡したキーと値はカテゴリをまたいで共有する。
キーが重複した場合は外側スコープ、内側スコープ、ログ自身の順で上書きする。
ロガーのメタデータは呼び出し側の同名属性より優先する。
基本型は型を保ち、それ以外（UUID や独自の ID 型など）は文字列にする。null の値は属性から除く。
Console の本文はスコープ属性の有無にかかわらず同じ表示とする。

スコープは .NET の実行コンテキストに従う。通常の Task では非同期処理へ伝播するが、
UniTask は実行コンテキストを自動で伝播しない。
UniTask の `await` をまたぐスコープには依存せず、必要な属性を各ログに明示する。
今回の変更では Activity / Span や独自の非同期伝播機構は追加しない。
既存の `logConversions` による診断ログの切り替えは維持する。

### テスト

出力を必要としないテストでは `NullLogger<T>.Instance` を明示的に渡す。
ローカル出力のテストでは無効な Hub、Sentry のテストではメモリ内 Transport を使い、実サービスへは送信しない。
Unity SDK の公開 API はグローバルに初期化するため、SDK を使うテストは各ケースで初期化・終了する。
