## ビルド

配布と `scripts/run_pc1.py` には tracker 専用の `release-tracker` プロファイルを使う。
`mise exec rust protoc -- cargo build --locked --profile release-tracker -p ball_tracker` でビルドし、
`target/release-tracker/ball_tracker`（Windows は `.exe` 付き）を起動する。
設定の目的と検証状況は [ビルドプロファイル](../docs/ball_tracker/ball_tracker_build_profiles.md) を参照。

## 設定ファイル

既定では、次の場所にある `config.yaml` を読み込みます。`--config PATH` または `--config=PATH` で別のファイルを指定できます。
明示したファイルがない場合はエラーになり、既定のファイルがない場合は設定ファイルなしで起動します。

- Linux: `~/.config/struckout/ball-tracker/config.yaml`
- Windows: `%APPDATA%\struckout\ball-tracker\config.yaml`
- macOS: `~/Library/Application Support/struckout/ball-tracker/config.yaml`

キーには長いオプション名 (`--` を除く) を使います。書かなかった項目は既定値になり、知らないキーはエラーになります。
`--json` のようなフラグは `true` / `false` で書きます。

```yaml
input: network
output: network
report-delay-ms: -30
json: true
machine-id: 1
```

設定ファイルとコマンドラインの両方に同じ引数がある場合は、コマンドラインの値が優先されます。
ただし、設定ファイルで `true` にしたフラグを、コマンドラインから無効にすることはできません。

筐体番号は `--machine-id` または設定の `machine-id` で指定する (既定は 1)。
すべての通常ログに `machine_id=<番号>` を付け、Sentry にも同名のタグを付ける。
別スレッドから出るログにも適用する。解析用のイベント・フレーム JSON の形式は変えない。

## ヘルスチェック
ランチャーが状態を確かめられるように、5080 番 (`api/spec/servers.yaml` の `ball-tracker.health`) で gRPC の health checking protocol に答える。
入出力の準備が終わり、処理を始める直前に `SERVING` になる。それまでは `NOT_SERVING`。
5080 番を開けないとき (ほかの ball_tracker が動いているなど) は、エラーのログを出して、ヘルスチェックなしで動き続ける。

reflection には対応していないので、grpcurl で確かめるときは [health.proto](https://github.com/grpc/grpc-proto/blob/master/grpc/health/v1/health.proto) を渡す。
```sh
grpcurl -plaintext -import-path <health.proto のあるフォルダ> -proto health.proto localhost:5080 grpc.health.v1.Health/Check
```

同じポートで `tk75attractions.struckout.ball_tracker.v1.CameraStatusService/ListCameraStatus` を提供する。
リポジトリルートから、次のコマンドでカメラごとの状態を取得できる。

```sh
grpcurl -plaintext -import-path api/proto -proto ball_tracker/camera_status.proto localhost:5080 tk75attractions.struckout.ball_tracker.v1.CameraStatusService/ListCameraStatus
```

`cameras` には `--cameras` の全カメラを記載順の `camera_id` で含め、未接続のカメラも省略しない。
設定にない端末も割り当てられた ID で含め、`name` は空にする。`device_id` は設定または制御接続の Hello から取得する。
`connected` は制御・検出・動画のいずれかの接続が残っている間は `true`。
`last_frame_at` は映像フレーム（コーデック設定を除く）か検出パケットを最後に受信した UTC 時刻で、未受信なら未設定。
`disconnected_at` は最後の接続が切れた UTC 時刻で、再接続後も直前の切断時刻を残す。
`fps` は直近に完了した 1 秒の集計区間で処理したフレーム数。検出入力は受信パケット、動画入力は解析を終えたフレームを数える。
最初の区間の完了前、処理がなかった区間、切断後は 0 になる。起動完了を判定する側は `connected` と併せて
`last_frame_at` の新しさや `fps` を確かめる。

入力は `--input network` のみ。以前の `input-sqlite` feature と未実装の `SentryEventLogger` は削除した (#298)。
録画の解析には `replay_video` や `scripts/recording_analysis/` を使う。
Sentry への通常のログ送信は引き続き `sentry` feature で有効にできる。

SIGTERM または Ctrl-C を受けると、新しい接続を止めてから受信中のフレームを処理し、CSV と動画の記録を閉じて終了する。
動画記録の終了時は、受け付け済みのデータが書き終わるまで待つ。

## アルゴリズムの説明
### フレームの時刻をそろえる
カメラの時計は PC とも互いにも合っておらず、進み方も少し違う (実機で 2 台の差が 1.4 秒、1 分に 3ms ずつ変化)。
`src/clock_sync.rs` が、カメラごとに時計の対応を測り続けて、フレームの時刻を PC の時計に直す。

- カメラの位置を登録する接続 (6060 番) で、`ball_tracker` から時刻合わせの要求を送る (接続直後に 50ms おきに 8 回、
  そのあとは 0.5 秒おき)。アプリは受け取った時刻と返す時刻を、端末の単調な時計で答える。往復時間が最も短い回から、
  NTP と同じ計算で時計の差を求める。
- アプリは検出結果に、カメラが付けたセンサー時刻 (ns) をそのまま付けて送る。センサー時刻がどの時計で
  付いているかは端末によるので、アプリが知らせた種類 (`CameraInfo`) に従って直す。
- 時刻合わせに対応していない送信元 (古いアプリ) は、「受信時刻 − カメラの時刻」の直近 10 分の最小値を
  時計のずれとみなす。撮影から受信までの遅れがカメラごとに違うと、そのぶん 2 台の時刻がずれる。
- 取りきれない一定のずれは `--time-trim-ms` で右のカメラ (位置の y が大きいほう) の時刻に足す。値は
  `scripts/recording_analysis/estimate_time_offset.py` で求める。
- センサーは行を上から順に読み出すので、画像の下にあるものほど遅く写っている。カメラが知らせた
  行ごとの時刻差 (または `--rolling-shutter-skew-ms`) と検出の縦位置から、検出 1 件ごとの時刻を出す。

### カメラの動画を PC で解析する
高速撮影版のカメラアプリは、検出の代わりに動画 (H.264) を 5052 番に送ってくることがある (端末での解析が 120fps に
追いつかないため)。`src/video/` がそれを受けて、アプリが送るのと同じ `DetectionsPacket` を作る。これより先は、
アプリから検出を受け取ったときと同じ。

- デコードは ffmpeg に任せる (`--ffmpeg`、既定は PATH の `ffmpeg`)。カメラごとに 1 つ起動し、NV12 を出す。
  動画が解析画像の 2 倍なら、Rust で輝度の 2x2 を平均して縮小し、元の色差を使って RGB24 に変換する。
  同じ大きさなら縮小せず RGB24 に変換する。それ以外の倍率だけ ffmpeg の `scale=幅:高さ:flags=area` で縮小する。
  変換はカメラのフルレンジ BT.601 (yuvj420p) を前提にするため、リミテッドレンジの動画では色が少しずれる。
  引数と変換は `video/decode.rs` にまとめ、`replay_video` と `bench_analysis` でも同じ処理を使う。
  動画か解析画像の大きさが変わったら ffmpeg を起動し直す。BMP のスナップショットは変換後の RGB を保存する。
  ffmpeg の出力には時刻が付かないので、書いた順に出てくるものとして、受け取ったフレームの時刻を順に割り当てる
  (アプリは B フレームを使わない)。
- 解析 (`video/tracker.rs`) はアプリの `ObjectTracker` (OpenCV) を移したもの。背景との差、二値化、
  楕円 5x5 でのオープン 1 回・クローズ 2 回、前景が続いたら背景に混ぜる、外側の輪郭の面積、色の変化の大きさ。
  2026-10-10 の録画 822 フレームで、OpenCV で書いた `scripts/recording_analysis/detect_objects.py` と
  検出の枠がすべて一致した。視線 (`video/analysis.rs`) もアプリの `WorldDirectionCalculator` と同じ計算。
- 1 フレームの解析の予算は 120fps で 8.33 ms で、2 台を別のスレッドで解析する。2026-10-18 に、PC① が長く動いて
  熱で遅くなり、解析が 1 フレーム 8.5〜26.8 ms に延びて追いつかなくなった。そこで、結果を 1 ビットも変えずに、
  画像全体をなぞる回数を減らした (前景を 1 画素 1 ビットに詰めて形の処理をする、領域の番号付けと色みの判定を
  横の連なり単位にする、背景をまとめて更新する。詳しくは `video/tracker.rs` の先頭)。Snapdragon X の PC で、
  1 フレーム 3〜5 ms が 0.5 ms (人が写った区間の 95% で 13 ms が 2.6 ms) になった。
- `bench_analysis` で、録画の一部を使って、解析の時間と検出の結果の指紋を測れる。解析を変えたときは、指紋が
  変わらないこと (結果が同じであること) と、時間を確かめる。途中の区間はデコードに数分かかるので、`--cache` で
  ffmpeg が出した NV12 を使い回す。解析と変換の時間は別々に出す。
  キャッシュには識別情報 (形式・サイズ・開始位置) を付ける。旧 RGB キャッシュは使えないので別のパスで作り直す。

  ```bash
  cargo run --release --bin bench_analysis -- recordings/2026-10-16_experiment_3/video/camera0_20261016-064230.vsp \
      --skip 33600 --frames 1200 --cache /tmp/run3_camera0.raw
  ```
  録画パスはリポジトリルートからの相対パス。`ball_tracker/` から実行する場合は `../recordings/` にする。
  2026-10-23 の変換単体の計測 (Snapdragon X、合成 NV12、`rustc -O`、2000 枚を 3 回) では、
  1280x720 → 640x360 が 0.243 / 0.246 / 0.249 ms/フレームだった。
  同日の指定録画 (`--skip 33600 --frames 1200`) の変更前後の計測は、どちらも ffmpeg の起動が
  アクセス拒否 (OS error 5) で失敗した。検出数・指紋・解析時間・録画での変換時間の比較は未確認。
- 範囲、内部パラメータ、向き、解析の設定は、アプリが `VideoStreamConfig` で送ってくる値を使う。
- 遅れは、60fps の録画を流して受け取ってから解析し終えるまで 50〜60 ms (2026-10-15)。
  `scripts/recording_analysis/stream_video_to_tracker.py` で、録画をアプリと同じ手順で流して確かめられる。
- 10 秒おきに、カメラごとに `video analysis` のログを出す。時間はどれも 1 フレームあたりで、`_p50`、`_p95` は
  その 10 秒の中央値と 95% の値。

  | 項目 | 内容 |
  |---|---|
  | `fps` | RPC と共通の、直近に完了した 1 秒の集計区間で解析したフレーム数 |
  | `dropped_before_ffmpeg` | この 10 秒間に ffmpeg に書く前に捨てたフレームの数 |
  | `dropped_after_ffmpeg` | この 10 秒間に ffmpeg から読み、解析せずに捨てたフレームの数 |
  | `convert_ms` | NV12 の縮小と RGB24 への変換にかかった時間の平均 |
  | `analysis_ms` | 解析にかかった時間の平均 |
  | `delay_ms_p50`、`delay_ms_p95`、`max_delay_ms` | 受け取ってから解析し終えるまで |
  | `queued` | 受け取って、まだ解析していないフレームの数 |
  | `input_ms_p95` | 受け取ってから ffmpeg に書き始めるまで (ffmpeg が標準入力を読まないあいだ伸びる) |
  | `ffmpeg_ms_p50`、`ffmpeg_ms_p95` | ffmpeg に書き始めてから、NV12 を読み終えるまで。解析が遅れて ffmpeg の出力が詰まっているあいだも含む |
  | `queued_in_ffmpeg` | ffmpeg に書いて、まだ出てきていないフレームの数 |
  | `wait_ms` | 解析のスレッドが ffmpeg の出力を待っていた時間の平均 |
  | `send_ms` | 解析のスレッドが検出を追跡に渡すのを待っていた時間の平均 |

  追いついているあいだは、`wait_ms` + `convert_ms` + `analysis_ms` + `send_ms` が 1 フレームの間隔 (120fps で 8.33 ms) に近い。
  `queued` が増えているときに `wait_ms` が 0 に近ければ解析が、`wait_ms` が残っていれば ffmpeg (デコードと NV12 の出力) が
  追いついていない。
  受信から 0.3 秒を超えた入力は、次の新しいキーフレームまで捨てて追いつく (キーフレームの間隔は 1 秒)。
  ffmpeg から出た画像も、受信から 0.3 秒を超えていれば解析せずに読み流し、書き込み側も次のキーフレームまで
  捨てる (デコードしても捨てるだけのフレームを書き続けないように)。捨てた間の着弾は出ない。
  0.3 秒は、追跡が撮影から 0.5 秒 (`RAY_LIFETIME`) を過ぎた視線を捨てることから決めた (`video/pipeline.rs` の
  `MAX_DELAY`)。
- `--no-video-analysis` で受け付けをやめると、アプリは自分で解析する。

### 視線の集まりからボールを見つける
2 台のフレームは組にしない。シャッターは同期しておらず、速い球は片方のカメラにしか映っていない時間が長いので、
組にできたフレームだけを使うと視線の大半を捨ててしまう。実装は `src/tracking/` で、経緯と値の根拠は
`docs/ball_detection_plan_2_track_creation.md` にある。

フレーム (1 台ぶん) が届くたびに、次の順で処理する。

1. 報告する時刻の来た着弾を報告し、古い視線 (0.5 秒) とトラックを捨てる。
2. 検出を世界座標の視線に直して、視線バッファ (`ray_buffer.rs`) に入れる。
3. いまあるトラックに、その時刻の軌道上の点に最も近い視線 (12cm 以内) を足して、軌道を当てはめ直す。
   当てはめの残差が大きくなる視線は足さない。
4. どのトラックにも入っていない視線から、新しいトラックを作る (`ball_track.rs` の `seed_tracks`)。
   同じカメラで続けて映った 2 本ずつを 2 台ぶん組み合わせた 4 本に軌道を当てはめ、軌道に乗る視線を加える。
5. トラックごとに、ボールとみなす条件を確かめる。満たしていれば着弾を予測する。

ボールとみなす条件 (`ball_track.rs` の先頭に定数がある。2026-10-10 の録画だけで決めた仮の値):

- 2 台のカメラそれぞれに視線が 2 本以上ある
- 視線 1 本の誤差の見積もりが 2.5cm 以下で、どの視線も軌道から 6cm 以内
- 的に向かう速さが 1.5 m/s 以上、速さが 25 m/s 以下、横の速さが的に向かう速さ以下
- 観測した位置が筐体の内側にあり、着弾点が奥面の内側にある
- 着弾が最後の観測の 0.6 秒以内で、2 台の観測の時間の空きが 0.1 秒以内
- 途中のフレームをほとんど飛ばしていない

着弾は、予測した時刻の 30ms 前に 1 回だけ projector に送る。直前の報告から 0.4 秒以内の予測と、同じころに
着弾する、より視線の多いトラックがある予測は送らない (跳ね返りや影を 2 回目の着弾にしないため)。
視線が増えないまま 0.15 秒たったトラックは捨てる。

`-j` を付けると、`%APPDATA%/struckout/tracker/json/` に 2 つのファイルを残す。

- `events_*`: トラックの作成 (`NewTrack`)、更新 (`UpdateTrack`)、破棄 (`DropTrack`)、着弾の報告 (`Collision`)、
  見送った報告 (`SuppressedCollision`)、新しいトラックを探した結果 (`SeedDiagnostics`)、報告した投球の
  時刻合わせの手がかり (`ThrowDiagnostics`)
- `frames_*`: 受け取ったフレーム。1 行に 1 フレーム

`ThrowDiagnostics` の `residual_rms` (視線と軌道の距離) が大きい投球が続くときは、2 台の時刻か、カメラの位置と
向きがずれている。直近 10 球の中央値が 3cm を超えると警告を出す。

### カメラの位置・姿勢
各カメラの設定画面で、共有する世界座標系における位置 `(x, y, z)` と姿勢を設定する。
姿勢は度数法の roll（X軸）、pitch（Y軸）、yaw（Z軸）で、端末センサー座標から
世界座標への回転として `Rz(yaw) * Ry(pitch) * Rx(roll)` の順で適用される。

`WorldDirectionCalculator` は、画像座標をカメラ座標へ戻した後、
`LENS_POSE_ROTATION` の逆回転によって端末センサー座標のレイを求める。
`ball_tracker` は受信したレイに上記の設置姿勢を適用してから、軌道の当てはめに使う。
角度がすべて `0` の場合は、端末センサー座標の軸が世界座標の軸と一致しているものとして扱う。

### 三角測量的なやつ
いまの追跡では使っていない (`src/tracking/triangulate.rs` に残してある)。

カメラ$P$からの方向ベクトルを$\vec{a}$、カメラ$Q$からの方向ベクトルを$\vec{b}$とするとき  
$\vec{p} + t \vec{a} = \vec{q} + s \vec{b} = \vec{r}$ (1)
となるような$\vec{r}$を求める。  
  
$\vec{d} = \vec{q} - \vec{p}$   
$\vec{n} = \vec{a} \times \vec{b}$とする。  
(1)を変形して
$t\vec{a} - s\vec{b} = \vec{q} - \vec{p} = \vec{d}$  
両辺に$\vec{b}$を作用させて  
$(t\vec{a} - s\vec{b}) \times \vec{b} = \vec{d} \times \vec{b}$  
$t\vec{n} - s(\vec{b} \times \vec{b}) = \vec{d} \times \vec{b}$  
$\vec{b} \times \vec{b} = \vec{0}$なので  
$t\vec{n} = \vec{d} \times \vec{b}$  
両辺に$\vec{n}$を作用させて  
$t\vec{n} \cdot \vec{n} = (\vec{d} \times \vec{b}) \cdot \vec{n}$  
$t = \frac{(\vec{d} \times \vec{b}) \cdot \vec{n}}{|\vec{n}|^2}$

### 放物線と的の交点を求める
ボールは的に届く前にカメラの視野から出るので、見えているうちに軌道を当てはめ、的の面 (x = 0) まで延長する。
実装は `src/tracking/trajectory.rs`。

ボールの位置を $\vec{r}(t) = \vec{r_0} + \vec{v_0} t + \frac{1}{2} \vec{g} t^2$ とおく。$\vec{g} = (0, 0, -9.80665)$ は既知なので、
未知数は $\vec{r_0}$ と $\vec{v_0}$ の 6 個。

当てはめには三角測量した点ではなく、各カメラの視線をそのまま使う。2 台のシャッターは同期しておらず、
撮影時刻がずれたまま同時刻とみなして三角測量すると、速い球では 10cm 以上ずれるため。
時刻 $t_i$ にカメラ $\vec{c_i}$ から単位ベクトル $\vec{d_i}$ の向きに見えたとき、その視線と軌道上の点との距離は

$|P_i (\vec{r}(t_i) - \vec{c_i})|$、ただし $P_i = I - \vec{d_i} \vec{d_i}^T$

となる。これは未知数の一次式なので、二乗和を最小にする $\vec{r_0}$, $\vec{v_0}$ は連立一次方程式を 1 回解けば求まる。

重力は z 方向にしか働かないので、x は時刻の一次式になる。的の面を通る時刻は $t = -x_0 / v_x$ ($v_x < 0$ のとき) で、
そのときの y, z が着弾点になる。
