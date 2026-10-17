# PC② のセットアップと起動 (projector + game_master)

PC② は projector (Unity) とプロジェクターを受け持つ。projector は **ball_tracker (PC①、TCP 5000) と
game_master (gRPC 8020) の両方につながらないと動き出さない** (`docs/projector_debug.md`)。
game_master は PC② の Docker で動かす。

## 用意するもの

| もの | 入れ方 |
|---|---|
| Unity 6000.5.2f1 と Windows の書き出しの部品 | Unity Hub から (`projector/ProjectSettings/ProjectVersion.txt` の版) |
| Docker Desktop | <https://www.docker.com/products/docker-desktop/> |
| リポジトリ | PC① と同じ |
| データベースのパスワード | `.mise/config.local.toml` の `[env]` に `MYSQL_ROOT_PASSWORD_DEV = "..."` を書く (コミットしない)。`run-pc2.ps1` はこれを読む。前の `game_master/.env.dev` があればそれも使える |
| プロジェクター | PC② につなぎ、Windows の画面の設定で「拡張」、1920x1080 |

PC② のファイアウォールは開けなくてよい (PC② から PC① へつなぎに行くだけ)。PC① は 5000 番の受信を許可しておく
(`docs/setup_pc1.md` の 7 章)。

## 設定 (`scripts/pc2.json`)

| 項目 | 内容 |
|---|---|
| `tracker_host` | PC① の名前か IP。PC① の `run-pc1.ps1` が projector を待つときに表示する |
| `monitor` | プロジェクターをつないだ画面の番号 (1 から)。PC の画面が 1 なら、ふつうは 2 |
| `collision_transform` | 着弾位置の係数のファイル (空なら projector の既定)。projector の F2 の調整画面で保存したもの |
| `projector_exe` | 書き出した projector。無ければ `run-pc2.ps1` が書き出す |
| `unity_editor` | 書き出しに使う Unity |

## 起動

PC② で次の 1 つを実行する。PC① の `scripts\run-pc1.ps1` と、どちらを先に起動してもよい。

```powershell
powershell -ExecutionPolicy Bypass -File scripts\run-pc2.ps1
```

1. Docker Desktop を起動し (止まっていれば)、game_master とデータベースを起動する (初回はビルドに数分から十数分)
2. projector の書き出し (`projector/Builds/Windows`) が無ければ、Unity でコマンドから書き出す。
   **Unity Editor で projector を開いていると書き出せない**。projector を直したあとは `-Build` で書き出し直す
3. projector を、プロジェクターの画面に全画面で起動する (つなぎ先は `pc2.json`、`-TrackerHost` で上書きできる)
4. projector は ball_tracker が起動するまで待ち続ける (5 秒おきにつなぎに行く)。つながったら「ball_tracker と
   game_master につながった」と出る。10 分たってもつながらなければ知らせるが、projector は待ち続ける
5. projector のログは `recordings\<日付>_projector\projector_<時分秒>.log`

projector は、ball_tracker とも game_master とも、切れたら 2 秒おきにつなぎ直す。PC① の ball_tracker や game_master を
起動し直しても、projector はそのままでよい。ただし、game_master と切れているあいだに終わったゲームの終了は届かないので、
次のゲームが始まるまで、projector は遊んでいる状態のまま残る。

止めるときは `run-pc2.ps1 -Stop` (projector と game_master を止める)。動作だけ確かめるときは `-Windowed` で窓のまま。

## 着弾位置の合わせ込み

projector は、ball_tracker が送る座標 (横 = 左の壁からの距離、縦 = 地面からの高さ、m) を `Collision Transform` の係数で
盤面 (1920x1080) に写す。合わせ方は `docs/projector_debug.md` (偽の ball_tracker `sandbox/testTcpCLI` で既知の点を撃つ)。
奥の板の 4 隅は、左右の黒い柱の内側の縁 (y = 0.07 と 1.13 m) と、上の梁の下の縁 (z = 2.47 m)・上下の板の継ぎ目
(z = 1.79 m) が交わる点 (2026-10-16 に実測)。
