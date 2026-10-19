# projector のデバッグ環境

projector (Unity) は ball_tracker と game_master の 2 つにつながっている。
**通信方式が違う。** ball_tracker は生の TCP、game_master は gRPC。

| 相手 | 既定ポート | 方式 | 向き | 中身 |
|---|---|---|---|---|
| ball_tracker | 5000 | TCP | 受信 | `ProjectorPacket` (`CollisionPoint` / `TestMessage`) |
| game_master | **8020** | **gRPC** | 受信 | `ListenEvents` のストリーム (`Event`) |
| game_master | 8020 | gRPC | 送信 | `AddScore` |

ポートの定義は `api/spec/servers.yaml` が唯一の出どころ。Rust は `build.rs` が
そこから読み、projector は `NetworkSettings` の既定値として持っている
(**両者は自動では揃わないので、spec を変えたら C# 側も直すこと**)。

TCP のフレーミングは LE u32 の長さ + protobuf で、実装は
`api/rust/src/lib.rs` の `write_packet` / `read_packet`。

## デバッグ用の対向は 2 つある

| 対向 | プログラム | 立て方 |
|---|---|---|
| ball_tracker | `sandbox/testTcpCLI` (C#) | `dotnet run --project sandbox/testTcpCLI` |
| game_master | `sandbox/fake_game_master` (Rust) | `cargo run` (**リポジトリには入っていない**。下記) |

2 つとも立てれば、projector は **Fake モードではなく本物の通信経路のまま**動く。
シリアライズや gRPC のストリームまで含めて確かめたいときはこちら。
描画やアニメーションだけを触るなら Fake モード (後述) のほうが早い。

## 座標系と得点の約束事

- `CollisionPoint` は**物理座標 (m)**。ball_tracker は的の面の上での着弾位置を送る。
  `x` が面に沿った横方向 (世界座標の y)、`y` が高さ (世界座標の z)
  (`ball_tracker/src/collision_output/network.rs`: `x = coll.y, y = coll.z`)。
- **得点は差分**。game_master (`game_master/src/service.rs`) が `game.score += score_to_add`
  としているので、projector は累計ではなく的に当たったぶんだけを送る。
- `AddScore` には `game_id` が要る。projector は `GameStarted` を受け取ったときに
  `Event.game_id` を覚える。**ゲームが始まっていないと得点は送れない**
  (本物は `NotFound` を返す。偽物も同じ)。

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
| 既定 | `%USERPROFILE%\AppData\LocalLow\DefaultCompany\StruckOut\collision_transform.json` (Editor とプレイヤーで共通) |

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
  150 秒後に `GameFinished` を流す
- 得点計算は**本物**が動く (`PointCalculator`)。送信先だけがログになる

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

## 3. 偽 game_master につなぐ (fake_game_master)

本物は MySQL を要求するので、イベント処理を触るたびに Docker を立てるのは重い。
こちらは DB を持たず、`GameMasterService` のうち対向が実際に使うところだけを喋る。

> **これはリポジトリに入っていない。** projector (Unity) を触る人だけが使う道具なので、
> `sandbox/fake_game_master/` は `.gitignore` で追跡から外してある。
> 手元に無ければ作り直しが要る。ルートの cargo ワークスペースには属さない
> 独立クレートなので、ルートのビルドはこれが無くても壊れない。

```bash
cd sandbox/fake_game_master
cargo run
```

```
> start                  # NORMAL で開始。GameStarted が飛ぶ
> start hard 20          # HARD で 20 秒だけのゲーム
> finish                 # 時間を待たずに終わらせる
> status                 # 進行中のゲームと購読者の数
> exit
```

| 起動オプション | 意味 | 既定 |
|---|---|---|
| `--port` | gRPC のポート | `api/spec/servers.yaml` の 8020 |
| `--machine-id` | 対話コマンドが対象にする号機 | 1 |
| `--duration` | ゲームの長さ (秒) | 150 (本物と同じ) |

projector が送ってきた `AddScore` はその場で表示される。送信側が実装できたかの
確認はここを見るのが早い。

### 号機番号がずれていると何も起きない

game_master は `machine_id` でイベントを絞る (`pass_events_through_by_machine_id`)。
**繋がってはいるのにイベントが一件も来ない**ときは、まずここを疑うこと。

- projector 側: `GameLifetimeScope` の `Network Settings > Machine Id`
  (シーンに値が無ければ既定の 1)
- 偽 game_master 側: `--machine-id`

`status` が `listeners : 0` のままなら、そもそも projector が繋がっていない。

### 実装している RPC

| RPC | 使う側 | 偽物の挙動 |
|---|---|---|
| `ListenEvents` | projector | 号機で絞ってイベントを流す |
| `AddScore` | projector | 加算して表示。ゲームが無ければ `NotFound` (本物と同じ) |
| `StartGame` | touchpanel | ゲームを開始し、そのゲームのイベントを返す |
| `AddPlayer` | touchpanel | 通し番号を返すだけ。名前は覚えない |

`GameTimeLimitNotify` は本物と同じく毎秒流れる。projector は今これを捨てているが、
ストリームが生きているかの確認にはなる。

**本物と違うところ**: 偽物は `ListenEvents` で繋いできた相手に、既に始まっている
ゲームがあれば `GameStarted` を流し直す。本物は流さないので、touchpanel が
start したあとに projector を繋ぐと開始を取りこぼす。
先に projector を立ち上げておけば本物でも問題ない。

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

CI では `.github/workflows/proto_ci.yml` が同じ確認を回している。
