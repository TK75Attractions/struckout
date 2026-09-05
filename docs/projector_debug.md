# projector のデバッグ環境

projector (Unity) は ball_tracker と game_master の 2 つに TCP でつながっている。
どちらも projector が **client** で、対向が listen する側。

| 相手 | 既定ポート | 向き | 中身 |
|---|---|---|---|
| ball_tracker | 5000 | 受信 | `ProjectorPacket` (`CollisionPoint` / `TestMessage`) |
| game_master | 5001 | 受信 | `MasterProjectorPacket` (`StartGame`) |
| game_master | 5001 | 送信 | `ProjectorMasterPacket` (`score`) |

ポートの定義は `api/spec/*.yaml`、フレーミングは LE u32 の長さ + protobuf で、
実装は `api/rust/src/lib.rs` の `write_packet` / `read_packet`。

## 座標系と得点の約束事

- `CollisionPoint` は**物理座標 (m)**。ball_tracker は三角測量の結果をそのまま送る
  (`ball_tracker/src/collision_output/network.rs`: `x = coll.x, y = coll.z`)。
- **得点は差分**。game_master (`game_master/src/session.rs`) が `cur_score += score` としているので、
  projector は累計ではなく的に当たったぶんだけを送る。

### 物理座標 → 描画座標の変換

`TargetGenerator` は 1920x1080 の描画座標に的を置くので、受け取った物理座標を
そこへ写す必要がある。変換は `SensorProvider` が行い、係数は
`GameLifetimeScope` の Inspector (`Collision Transform`) で調整できる。

| 項目 | 意味 | 既定値 |
|---|---|---|
| `Pixels Per Metre X` / `Y` | 1 m あたりの描画座標 | 960 / 540 |
| `Origin X` / `Y` | 物理座標 (0, 0) が来る描画座標 | 960 / 0 |
| `Flip X` / `Flip Y` | 軸の向きが逆のとき | off |
| `Swap Axes` | 縦横が入れ替わっているとき | off |
| `Log Conversions` | 変換の前後をログに出す (合わせ込み用) | off |

**既定値は実測に基づいていない。** 物理 x[-1, 1] m / y[0, 2] m がちょうど 1920x1080 に
収まる、というだけの仮置きなので、盤面の実寸を測って必ず入れ直すこと。
合わせ込むときは `Log Conversions` を on にすると、送った物理座標と変換結果が並んで出る。

> **未解決**: ball_tracker は `CollisionPoint` に `(x, z)` を詰めているが、そこに
> `// FIXME: これあってる?` が残っている。縦に使う軸が違っていた場合は
> `Swap Axes` で暫定的に逃げられるが、本来は ball_tracker 側で決着させるべき。

## 1. 対向なしで起動する (Fake モード)

`GameLifetimeScope` の Inspector で `Network Settings > Mode` を `Fake` にすると、
TCP を一切使わずに起動できる。描画やアニメーションだけを触りたいときはこれ。

- `FakeClientService` が 1 秒おきにランダムな衝突点 (物理座標、既定 x[-1,1] y[0,2] m) を流す
- `FakeMasterService` が起動直後に一度だけ `StartGame(NORMAL)` を流す

## 2. ダミーの対向サーバにつなぐ (testTcpCLI)

本物の通信経路を通したいが ball_tracker / game_master を動かしたくない場合は、
`testTcpCLI` が両方のふりをする。

```bash
dotnet run --project sandbox/testTcpCLI
```

```
> listen sensor          # ball_tracker のかわりに 5000 で待つ
> listen master          # game_master のかわりに 5001 で待つ
                         # ここで Unity を Play する
> hit 960 540            # 画面中央に当てる
> hit                    # ランダムな座標に当てる
> auto 500               # 500ms ごとに撃ち続ける
> auto off
> start hard             # StartGame(HARD) を送る
> status
> exit
```

projector が送ってきた `ProjectorMasterPacket` (得点) は受信次第そのまま表示される。
送信側が実装できたかの確認はここを見るのが早い。

ポートを変えたいときは `listen sensor 6000` のように第 2 引数で指定する。

## 3. 別マシンの ball_tracker につなぐ

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
| `-masterPort` | `STRUCKOUT_MASTER_PORT` | `5001` |
| `-connectAttempts` | `STRUCKOUT_CONNECT_ATTEMPTS` | `5` |

接続は指数バックオフで `connectAttempts` 回まで再試行するので、
Unity を先に Play してから対向を立ち上げても間に合う。

## 4. protobuf のコード生成

`api/proto` の `.proto` が唯一の定義。Rust は `api/rust/build.rs`、Kotlin は Gradle が
自動生成するが、**C# だけは生成結果をコミットしている**。手で protoc を叩かないこと。

```bash
scripts/generate-proto.sh            # 生成してコミット対象を更新
scripts/generate-proto.sh --check    # ズレていないかだけ確認 (exit 1 で失敗)
```

mise を入れているなら `mise run proto` / `mise run proto:check` でもよい。

Windows からは Git Bash (Git for Windows 同梱) で実行する。
PowerShell から叩くなら `& "C:\Program Files\Git\bin\bash.exe" scripts/generate-proto.sh`。

生成先は `projector/Assets/Scripts/ProtoBuf/Generated` と
`sandbox/testTcpCLI/Network/Protocol/Generated` の 2 か所。
`scripts/Test-Dynamic.ps1` は最初にこの `--check` を走らせる。
CI でも `.github/workflows/proto_ci.yml` が同じスクリプトを回している。
