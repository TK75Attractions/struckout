# ボール検出の改善計画 ③: 2 台の時刻合わせとカメラ位置の較正

作成日 2026-10-11。対象はブランチ `feature/camera-kyv45-support` の作業ツリー (未コミットの変更を含む。これが基準)。
この文書は計画で、コードは変えていない。「確認済み」はコード・記録データ・公式ドキュメントのいずれかで確かめたこと、「仮定」「未確認」は確かめていないこと。
計画どうしの順序とファイルの分担は `docs/ball_detection_plan_0_overview.md` を優先する (この計画の A.5 と 4 章の分担は、そこで一部変えている)。

## 1. 目的と期待する動作

- 2 台のスマホが送るフレームの時刻を、PC の時計の上で 2 ms 以内にそろえる。数時間動かしてもずれない。
- スマホを置いた人が 15 分ほどで位置・向き・焦点距離を求め、アプリに入れられる。その結果、着弾位置の横 (y) の偏りが 2 cm 以下になる。
- 残っている誤差 (時計のずれ、視線と軌道の距離) が ball_tracker のログで分かる。

期待する動作:

1. ball_tracker は 6060 番の接続で定期的に時刻合わせの要求を送り、アプリは端末の単調な時計で答える。ball_tracker はカメラごとに「センサー時刻 → PC の時計」の対応を持ち続ける。
2. アプリは検出結果にセンサー時刻 (ns) をそのまま付けて送る。ball_tracker は追跡に渡す前に PC の時計へ直す。
3. 対応していない送信元 (旧アプリ、今の `replay_to_tracker.py`) は、今までと同じ方法で動く。projector には影響しない。
4. 位置と向きは、解析画像 1 枚とメジャーで測ったレンズ位置から PC のツールで解き、表示された値を設定画面に入れる。

## 2. 調査でわかったことと制約

### 2.1 時刻: いまのコード (確認済み)

| 項目 | 現状 | 場所 |
|---|---|---|
| CameraX 版の時刻 | `image.imageInfo.timestamp` (ns) を ms に切り捨て、`System.currentTimeMillis() - SystemClock.elapsedRealtime()` を足して送る | `struckoutCameraApp/.../camera/MyAnalyzer.kt:25`、`CameraViewModel.kt:64`、`:101-105` |
| 高速版の時刻 | `SurfaceTexture.timestamp` を同じ式で直す。`setRepeatingBurst` にコールバックを渡しておらず `CaptureResult` は読んでいない | `struckoutCameraAppHighSpeed/.../camera/highspeed/GlFrameReader.kt:94-97`、`CameraViewModel.kt:83`、`:158-162`、`HighSpeedCamera.kt:271-273` |
| 送信 | 検出があるフレームだけ送る。送信先は `127.0.0.1` 固定で、`scripts/adb-reverse.sh` による USB 経由。`TCP_NODELAY` なし | `CameraViewModel.kt:53-55` (高速版 `:74-76`)、`network/TcpSessionImpl.kt:174-176`、`DataConnectionImpl.kt:50`、`:90-95` |
| 6060 番 | アプリは接続時に camera id を 1 回読むだけで、その後は読まない。書くのは `UpdateCameraPose` だけ | `TcpSessionImpl.kt:83-93`、`:112-114`、`:132-145` |
| ball_tracker の受信 | `Utc::now()` を受信時刻にする。中身のない `TcpClientPacket` は警告して読み飛ばす | `ball_tracker/src/detection_input/network.rs:132`、`:209-221` |
| ball_tracker の時刻合わせ | (camera_id, session_id) ごとに「受信時刻 − カメラの時刻」の最小値を覚え、カメラの時刻に足す。20 ms 以内のフレームを組にする | `ball_tracker/src/detection_input/mod.rs:93-112`、`:25` |
| proto | `DetectionsPacket.timestamp` は int64 で単位の記載なし (コードは ms として扱う)。`TcpServerPacket` は `camera_id` だけ | `api/proto/camera__ball-tracker.proto:8-15`、`:48-53`、`mod.rs:98` |

ここから言えること:

- アプリの式は、センサー時刻が `elapsedRealtime` と同じ時計のときだけ正しい。フレームごとに 2 つの時計を ms 単位で読むので ±1 ms 揺れ、端末の時刻が修正されると飛ぶ。
- いまの ball_tracker の時刻は「撮影時刻 + そのカメラの最小の遅延」になる。2 台の最小遅延 (撮影から PC に届くまで) の差が、そのまま 2 台の時刻のずれになる。大きさは未測定。
- 最小値は忘れないので、PC より遅く進む時計には追従しない。
- `docs/transport_protocol.md:1-9` は game-master と projector の間だけで、カメラと ball_tracker の間の記述は無い。

### 2.2 位置・向き・焦点距離: いまのコード (確認済み)

- 設定画面の `Camera Pose` に x, y, z, roll, pitch, yaw を手で入れる (`config/ConfigScreen.kt:319-347`)。DataStore に保存し (`ConfigStoreRepositoryImpl.kt:39-59`、`:106-117`)、接続時と変更時に `UpdateCameraPose` で送る (`TcpSessionImpl.kt:58-68`、`:137-142`)。端末のセンサー (加速度など) は使っていない。
- ball_tracker は `Rz(yaw)·Ry(pitch)·Rx(roll)` で、端末座標の視線を世界座標に回す (`ball_tracker/src/types.rs:80-88`)。
- 焦点距離は設定できない。`LENS_INTRINSIC_CALIBRATION` が無い端末では焦点距離 (mm) とセンサー寸法から計算し (`camera/CameraRepository.kt:53-86`)、「出力は有効画素の中央を縦横比に合わせて切り出したもの」という前提で解析画像に換算する (`CameraIntrinsics.kt:20-41`)。KYV45 はこの経路で、640 px 幅で fx ≈ 482。光学中心は画像の中央とし、歪みは考えない。
- 高速撮影で画角が変わらないかは未確認 (`struckoutCameraAppHighSpeed/README.md:31-33`)。10/07 の KYV45 の動画 (標準カメラ、1080p) は 640 px 幅で 482 px だった (`recordings/2026-10-07/notes.md`)。
- 視線は端末座標で送る。`defaultBackCameraRotation(90)` の転置 × K⁻¹(u, v, 1) (`WorldDirectionCalculator.kt:32-58`、`CameraIntrinsics.kt:88-101`)。
- 解析画像を保存する機能は無い。プレビューは検出枠を描き込んで 90° 回した画像 (`MyAnalyzer.kt:57-69`、高速版 `CameraViewModel.kt:105-142`) で、CameraX 版は検出があるときしか更新されない (`CameraViewModel.kt:53-58`)。
- 着弾の当てはめの残差 `BallisticTrajectory::residual_rms` (`tracking/trajectory.rs:31-32`) は、イベントのログ (`tracking/event.rs:51-59`) に出ていない。

### 2.3 Android の API

公式ドキュメント (AOSP のソース内の記述を含む) で確認したこと:

- `CaptureResult.SENSOR_TIMESTAMP`: センサーの 1 行目の露光開始の時刻 (ns)。全端末で取れ、同じ撮影のすべての出力バッファに同じ値が付く。
- `SENSOR_INFO_TIMESTAMP_SOURCE`: `REALTIME` なら `SystemClock.elapsedRealtimeNanos()` と同じ時計。`UNKNOWN` なら単調増加だが他の時計とは正確には比べられず、「おおよそ `SystemClock.uptimeMillis()` と同じ時計」。
- `SENSOR_ROLLING_SHUTTER_SKEW`: 1 行目と最終行の露光開始の差 (ns)。LIMITED 以上が対象だが、値が null の端末もある。
- `SurfaceTexture.getTimestamp()`: ns。意味と原点は画像の送り手による。Android 10 の `Camera3OutputStream.cpp` は、HW コンポーザー向けと動画エンコーダー向けの出力だけ時計を変換し、それ以外にはカメラの時刻をそのまま付ける。高速版の出力先 (SurfaceTexture) は後者にあたる。
- CameraX の `ImageInfo.getTimestamp()`: 「意味は画像の送り手による」としか書かれていない。

未確認:

- KYV45 の 2 台が `SENSOR_INFO_TIMESTAMP_SOURCE` と `SENSOR_ROLLING_SHUTTER_SKEW` に何を返すか。較正値を返さない端末なので `REALTIME` を前提にしない。
- CameraX の `ImageAnalysis` の時刻が `SENSOR_TIMESTAMP` と同じ値であること (実装上はそのはず)。
- 高速撮影セッションで露光を手動にできないこと (自動露光のままになるという理解)。
- 計画時に Android の公式リファレンスのページ本文は取得できなかった。API の記述は AOSP のソース (`CameraMetadata.java`、`SurfaceTexture.java`、`Camera3OutputStream.cpp`)、androidx のソース (`ImageInfo.java`)、リファレンスの写し (古い版の要約を含む) で確認した。

参照: [CameraMetadata](https://developer.android.com/reference/android/hardware/camera2/CameraMetadata#SENSOR_INFO_TIMESTAMP_SOURCE_REALTIME)、[CaptureResult](https://developer.android.com/reference/android/hardware/camera2/CaptureResult#SENSOR_TIMESTAMP)、[SurfaceTexture](https://developer.android.com/reference/android/graphics/SurfaceTexture#getTimestamp())、[ImageInfo](https://developer.android.com/reference/androidx/camera/core/ImageInfo#getTimestamp())、[Camera3OutputStream.cpp (android-10.0.0_r1)](https://android.googlesource.com/platform/frameworks/av/+/refs/tags/android-10.0.0_r1/services/camera/libcameraservice/device3/Camera3OutputStream.cpp)

### 2.4 記録データで確かめたこと

計画を書くために、スクラッチパッドの `analyze.py`、`calib.py` と `landing_truth.py` を読み込んで計算した。ファイルは作っておらず、結果は保存していない。担当 P が手順 P-1 / P-2 で再現する。座標は筐体データ (m)、誤差は「予測 − 正解」。

基準 (右の時計 +25 ms): 横 y 平均 +5.37 cm (二乗平均 6.06、最大 9.7)、高さ z 平均 +0.90 cm (二乗平均 2.03、最大 5.1)、視線と軌道の距離 1.37 cm。`notes.md` と一致。

時刻:

- 球ごとに「右の時計を何 ms 進めると視線が軌道に最もよく乗るか」を求めると、22.1 / 20.4 / 27.2 / 23.7 / 23.7 / 23.4 / 17.2 / 22.2 / 22.4 / 34.4 ms。平均 23.7、中央値 22.9、標準偏差 4.5 ms。10 球で ±1.5 ms、20 球で ±1 ms まで決まる計算になる。
- 右の時計を ±5 ms 動かすと、y が平均 ±0.9 cm (コマの少ない 7 球目は ±3 cm) 動く。
- 実機の過去のログ `ball_tracker/logs/pair_20260922_1249` (57 秒、1694 組) では、2 台の生の時刻の差が 1.365 秒あり、−3.0 ms/分で変わっていた。端末の時計には頼れず、1 回合わせただけでは数分でずれる。

横のずれの内訳 (3.B.1 の表の根拠):

- ローリングシャッター: ボールは 2 台の画像の短辺を端から端まで、互いに逆向きに横切る (左は v が 13→340、右は 350→22)。行ごとの時刻差を「左 12 ms、右 0 ms」とすると視線と軌道の距離が 1.41→1.26 cm に減り (正解は使っていない)、y が +5.4→+4.3、z の二乗平均が 2.0→1.2 cm になる。7 球目は +9.7→+4.9。右の値は残差からは決まらない (±8 ms 変えても 0.03 cm しか動かない)。
- 補正後の y は +2.0〜+6.3 (2 コマずつしか無い 8 球目だけ +8.6) で、速さへの依存はほぼ消える。低い球ほど大きい (z 1.5 m で約 +5、2.0 m で約 +2)。
- 線による較正は、筐体データのずれに弱い。基準の線 1 本を 1 cm ずらして解き直すと、位置が最大 4 cm、向きが 1.3° 動き、着弾は平均で最大 0.7 cm、1 球では最大 5.2 cm 動く。位置を固定して向きだけ解けば、向き 0.6° 以下、平均 0.6 cm 以下、1 球で 1.8 cm 以下になる。メジャーの位置が 1 cm 違っても y は 0.45 cm 以下しか動かない。
- 右のカメラを 5 cm / 10 cm 下げて向きを解き直すと、y の平均は +2.5 / +0.7 cm、z は −1.8 / −3.3 cm になる。較正値は右が左より 11 cm 高い (1.340 と 1.230)。一方、別角度の映像に見えるライトの方向 (右へ 5 cm、下へ 5 cm) に動かしても y は +4.2 のまま。
- どの動かし方でも視線と軌道の距離は 1.2〜1.3 cm のままで、飛球だけからは横の偏りを見分けられない (`notes.md` の「向きを合わせ直すと悪くなる」と同じ理由)。
- 2 台で当てはめた軌道を別角度の映像に映すと、飛行の途中ではボールの像との横のずれは平均ほぼ 0 で、前半 −1〜−2 cm、後半 +1〜+3 cm と奥面に近づくほど右へ寄る (よく映った 6 球。別角度のカメラの位置の決め方を変えても同じ傾向)。一定のずれではなく、軌道の向きが数度違っている形。
- 検出の中心の遅れ: 外接矩形は進行方向に 1 コマぶんの動きの 0.4〜0.5 倍だけ伸びていて、遅れは約 0.25 コマ。2 台の差は 0.05 コマ (約 1 ms) 程度。中心を 0.5 コマぶん進めてみると、2 台とも進めた場合は y −0.2 cm、右だけなら −1.3 cm (7 球目 −5.3)、左だけなら +1.7 cm (7 球目 +9.3)。
- 画像の端で切れた検出や最初と最後のコマを除いても、y は +3.6〜+4.3 で変わらない。
- 右の位置が x 方向に 1 cm 違うと、最もよく合う時計のずれが約 1.2 ms 変わる。飛球から求めた時刻のずれには位置の誤差が混ざる。
- KYV45 のレンズの歪み: 10/07 の動画の 2 フレームで、中心から 575 px 離れた長さ 700 px ほどの直線の曲がりが 0.8 px 以下 (1920 px 幅)。画像の隅で 1.5% 以下にあたる。線が 2 本しか取れておらず粗い。

### 2.4.1 段階 1 (P-1、P-2) の実装でわかったこと (2026-10-12 追記)

`predict_landing.py` の拡張と `estimate_time_offset.py` で 2.4 の数字は再現できた。次の点は 2.4 の記述を改める。

- **行ごとの時刻差の測り方**: 2.4 の「左 12 ms、右の時計 +25 ms」は、行の時刻を画像の中央から測った計算だった。A.5 の定義 (画像の上端から測る) では、同じ状態は「左 12 ms、右の時計 +31 ms」になる。行ごとの時刻差を入れるときは、`--time-trim-ms` も組で決め直す (ツールが組にして表示する)。
- **どちらのカメラの時刻差かは決まらない**: 右だけに付けても、視線と軌道の距離は左に付けたときとほぼ同じだけ縮む (1.26 cm と 1.21 cm)。2 台の画像をボールが逆向きに横切るため。端末が返す値 (U-1) で決める。
- **行ごとの時刻差を無視すると、時計のずれの推定が偏る**: 合成データで、真のずれ +25 ms・左 12 ms のとき、時刻差を 0 とみなすと +17 ms と出る。
- **位置の誤差の混ざり方**: 右のカメラを x 方向に 1 cm 動かすと、最もよく合うずれは約 2.5 ms 変わる (2.4 と A.1 の 1.2 ms は過小)。
- **トス試験 (A.7)**: 真上に上がるボールなら 1 ms 以内で合うが、2 台を結ぶ向きに 0.1 m/s 流れると 7〜12 ms ずれる (合成データ)。5 回の平均で ±2 ms という見込みは、流れが 0.03 m/s 程度に収まる場合に限る。実データでは未検証。
- A.6 の「1 台あたり 4 コマ以上」の規則では、10/10 のデータは 7 球になる (平均 24.8、中央値 23.4 ms)。2.4 の 23.7 / 22.9 ms は 2 コマ以上の 10 球での値。

### 2.5 制約

- 未コミットの変更を保つ。`git stash`、`git checkout --`、`git reset` は使わない。コミットはユーザーが頼んだときだけ。
- テストの実行とビルドはユーザーが頼んだときだけ。新しいテストはユーザーが頼んだときだけ足す。
- 2 つのアプリはほぼ同じ内容なので、アプリ側の変更は両方に入れる。
- KYV45 は 1 台目 `166810131208` が Android 9、2 台目 `166810122536` が Android 10。120 fps は高速撮影 (1920×1080) だけ。

## 3. 検討した方式と採用する方式

### A. 2 台の時刻合わせ

#### A.1 比較

| 方式 | 精度の見込み | 長所 | 短所 | 判断 |
|---|---|---|---|---|
| 端末の時計に頼る (いまのアプリの式) | 実測で 2 台の差 1.4 秒、3 ms/分で変化 | 変更が要らない | 使えない | 不採用 |
| PC 側で最小の遅延に合わせる (いまの `align_timestamp`) | 2 台の最小遅延の差が残る (未測定)。遅く進む時計に追従しない | 実装済み。アプリの対応が要らない | 数分〜数十分でずれる | 対応していない送信元のためだけに残す |
| 6060 番で往復時間を測って時計を合わせる (NTP と同じ計算) | USB (adb reverse) で 1 ms 以下、Wi-Fi で 1〜2 ms (どちらも仮定) | 接続直後から効き、時計の進み方の差を追い続ける | アプリに受信の処理が要る | 主な方式として採用 |
| 飛球から残りのずれを推定する | 10 球で ±1.5 ms、20 球で ±1 ms (10/10 のデータ) | センサー時刻の定義や露光の差まで含めて合う | 位置の誤差も吸う (右の x 1 cm ≈ 1.2 ms)。球が要る | 残りの補正と監視に採用 |
| 2 台に同時に見える出来事で確かめる | ±2 ms (仮定) | 位置・向きの誤差に左右されない | 手作業 | 確認の手順として採用 |

#### A.2 各アプリが撮影時刻として出せるもの

| | CameraX 版 (30 fps) | 高速版 (120 fps) |
|---|---|---|
| フレームの時刻 | `ImageInfo.timestamp` (ns) | `SurfaceTexture.timestamp` (ns) |
| 中身 | `SENSOR_TIMESTAMP` と同じはず (未確認) | カメラの時刻そのまま (Android 10 のソースで確認) |
| 時計の種類 | `SENSOR_INFO_TIMESTAMP_SOURCE` による。`REALTIME` なら `elapsedRealtimeNanos`、`UNKNOWN` ならおおよそ uptime | 同じ |
| 行ごとの時刻差 | `SENSOR_ROLLING_SHUTTER_SKEW` (null のこともある)。Camera2Interop でコールバックを足して読む | 同じキー。`setRepeatingBurst` にコールバックを足して読む |

どちらの種類の時計でも動くよう、アプリは時刻合わせの応答に `elapsedRealtimeNanos()` と `System.nanoTime()` の両方を入れ、どちらを使うかは ball_tracker が決める。

#### A.3 採用する方式

1. アプリはセンサー時刻 (ns) を変換せずに送る。いまの `timestamp` (ms) も、古い ball_tracker のために今までどおり入れる。
2. ball_tracker は 6060 番の接続ごとに時刻合わせの要求を送る。接続直後に 50 ms 間隔で 8 回、その後は 0.5 秒ごと。
   - PC が送った時刻を T1、端末が受けた時刻を t2、返した時刻を t3、PC が受けた時刻を T4 とする。
   - 時計の差 (端末 − PC) = ((t2 − T1) + (t3 − T4)) / 2、往復時間 = (T4 − T1) − (t3 − t2)。
   - 直近 8 回のうち往復時間が最も短い回の値を使う。
   - 5 回続けて応答が無いカメラには送るのをやめ、従来の方法に切り替える。
3. センサー時刻を PC の時計に直す。
   - `REALTIME` なら、センサー時刻 − 時計の差。
   - `UNKNOWN` なら、センサー時刻 + (t3 での elapsedRealtime − nanoTime) − 時計の差。
   - 直した時刻が受信時刻より後、または 1 秒以上前になるカメラは時計の種類の見込みが外れているので、従来の方法に切り替えて警告を出す。
4. 対応していない送信元は「受信時刻 − カメラの時刻」の最小値で合わせる (いまと同じ考え方)。最小値は直近 10 分の窓で取り、遅く進む時計にも追従させる。
5. 残りの一定のずれは、起動時の指定 `--time-trim-ms` で「右のカメラ (設定した位置の y が大きいほう) の時刻に足す」。値はオフラインのツールで求める。② の実装後はオンラインでも推定する (A.6)。camera id は接続順で変わるので、左右は位置で決める。

システムの時刻合わせ (NTP) に頼る方式は、2.4 の実測のとおり使えない。

#### A.4 プロトコルの変更 (番号はここで確定する。3 人の担当が並行して使う)

```proto
message DetectionsPacket {
  // 1〜5 は変更なし。3 (timestamp) は今までどおり ms で入れる
  // フレームのセンサー時刻 (ns)。変換しない値。0 は未対応の送信元
  int64 sensor_timestamp_ns = 6;
}

message Detection {
  // 1〜5 は変更なし
  // 検出の中心の、解析画像 (センサーの向き) での位置。左上 (0, 0)、右下 (1, 1)。送らない送信元では 0
  float image_x = 6;
  float image_y = 7;
}

enum TimestampSource {
  TIMESTAMP_SOURCE_UNSPECIFIED = 0;
  TIMESTAMP_SOURCE_UNKNOWN = 1;
  TIMESTAMP_SOURCE_REALTIME = 2;
}

message TcpClientPacket {
  message UpdateCameraPose { /* 変更なし */ }
  message TimeSyncResponse {
    uint64 sequence = 1;
    int64 receive_elapsed_realtime_ns = 2;  // t2
    int64 send_elapsed_realtime_ns = 3;     // t3
    int64 send_uptime_ns = 4;               // t3 と同じ瞬間の System.nanoTime()
  }
  // 接続直後と、撮影の設定が分かった・変わったときに送る
  message CameraInfo {
    uint32 camera_id = 1;
    string device_model = 2;
    string app_variant = 3;               // "camerax" / "highspeed"
    TimestampSource timestamp_source = 4;
    int64 rolling_shutter_skew_ns = 5;    // 取れないときは 0
    int64 exposure_time_ns = 6;           // 最後に見た値。監視用。取れないときは 0
    uint32 image_width = 7;               // 解析画像
    uint32 image_height = 8;
    double focal_length_px = 9;           // アプリが視線の計算に使っている値
    double frame_rate = 10;
  }
  oneof data {
    UpdateCameraPose update_camera_pose = 1;
    TimeSyncResponse time_sync_response = 2;
    CameraInfo camera_info = 3;
  }
}

message TcpServerPacket {
  message TimeSyncRequest { uint64 sequence = 1; }
  oneof data {
    uint32 camera_id = 1;
    TimeSyncRequest time_sync_request = 2;
  }
}
```

互換性:

- 新しいアプリと古い ball_tracker: 要求が来ないので応答しない。`CameraInfo` は「中身のないパケット」として読み飛ばされる (`network.rs:219-221`)。`timestamp` は今までどおり。
- 古いアプリと新しい ball_tracker: 要求は読まれずに溜まる (1 回 10 バイト程度) ので 5 回で打ち切り、従来の方法で合わせる。
- projector が使うのは `ball-tracker__projector.proto` だけなので影響しない (`buf.gen.ball-tracker.yaml`)。

#### A.5 ball_tracker での適用箇所と、計画 ② との境界

時刻の処理は新しいファイル `ball_tracker/src/clock_sync.rs` に置く。

```rust
// ball_tracker/src/clock_sync.rs (この計画が所有)
impl ClockSync {
    /// フレームの時刻を PC の時計で返す。カメラごとの補正 (時計の対応、--time-trim-ms) を適用済み。
    pub fn frame_time(&self, packet: &DetectionsPacket, received_at: DateTime<Utc>) -> Option<DateTime<Utc>>;
    /// 検出 1 件の時刻。行ごとの露光の時刻差 (image_y × そのカメラの値) を足す。
    pub fn detection_time(&self, camera_id: CameraId, frame_time: DateTime<Utc>, detection: &Detection) -> DateTime<Utc>;
    /// frame_time に足してある共通の遅延。真の撮影時刻が要るときは引く。
    pub fn pipeline_delay(&self) -> TimeDelta;
    /// 着弾を報告した投球の視線を渡す。2 台の時刻のずれの推定と監視に使う。
    pub fn observe_throw(&self, rays: &[ThrowRay]);
}
pub struct ThrowRay { pub camera_id: CameraId, pub time: DateTime<Utc>, pub origin: Vector3<f64>, pub direction: Vector3<f64> }
```

取り決め:

- `frame_time` は「真の撮影時刻 + 共通の遅延 L」を返す。L は、接続中のカメラの「撮影から受信までの最小時間」のうち最大のもの。
  - いまの追跡は予測した着弾時刻を `Utc::now()` と比べている (`tracking/mod.rs:129`、`:147-161`)。真の撮影時刻をそのまま渡すと、着弾前のフレームが届く前に着弾を報告してしまう。L を足せば、フレームの時刻が受信時刻とほぼそろう今の性質を保てる。
  - 全カメラに同じ値を足すので、2 台の間のずれには影響しない。L は 10 ms 以上変わったときだけ更新する。
- 計画 ② が先に入る前提 (`ball_detection_plan_0_overview.md`) なので、`frame_time` は ② が作る `CameraFrame.time` の作り方を置き換える形で入れる。② が用意する「補正値を足す場所」と `align_timestamp` は、`ClockSync` の従来の方法 (A.3 の 4) に移す。
- 追跡の側で要る変更 (② の完了後に、この計画の担当 R が行う):
  1. 視線を作るときにフレームの時刻ではなく `detection_time` を使う。
  2. 着弾を報告するときに `observe_throw` を呼ぶ。`BallisticTrajectory::fit` を `pub(crate)` で `clock_sync.rs` から呼べるようにする。
  3. 真の時刻で報告したい場合は `pipeline_delay` を引く。② の `REPORT_LEAD` と合わせて決め直す。
- 2 台とも時刻合わせに対応しているときだけ精度が出る。片方でも従来の方法なら、いまと同じ精度として警告を出す。

`network.rs` を直すついでに、同じファイルの問題を 2 つ直す。

- 接続が切れると `network.rs:207` で panic する。
- camera id が「登録済みの位置の数」で決まる (`lib.rs:93-95`) ので、2 台がほぼ同時に接続すると同じ id になりうる。時刻合わせの状態を id で引くので、接続ごとの連番にする。

#### A.6 飛球からの推定

- オフライン (先に作る): `scripts/recording_analysis/estimate_time_offset.py`。
  - 入力は検出の CSV と設定 (いまの `replay_*.json` の書式)、または ball_tracker が残すフレームのログ (A.8)。
  - 投球ごとに、右のカメラの時刻を −30〜+60 ms の範囲で動かして視線と軌道の距離が最小になる値を求め、球ごとの値・中央値・標準誤差・全球をまとめた最小を出す。同じ方法で行ごとの時刻差も走査する。
  - 1 台あたり 4 コマ以上ある球だけを使う。必要な球数は 10 球 (±1.5 ms)、できれば 20 球 (±1 ms)。
- オンライン (② の後): `observe_throw` で同じ計算を投球ごとに行う。
  - 直近 30 球の中央値を使い、10 球たまるまでは何もしない。
  - 1 球ごとの変化は 0.5 ms まで。補正の幅は ±5 ms まで (従来の方法のカメラが混ざるときは ±40 ms)。再接続でやり直す。
  - 既定ではログに出すだけにし、`--auto-time-trim` を付けたときだけ適用する。
  - 推定が ±5 ms の端に張り付いたら、時計ではなく位置・向きを疑うよう警告する (2.4 のとおり位置の誤差が混ざるため)。

#### A.7 手作業での確認 (位置・向きによらない)

- トス試験: 2 台の両方に映る場所でボールを真上に軽く投げ上げ、頂点の時刻を 2 台で比べる。頂点はどのカメラから見ても同じ瞬間なので、位置・向きの誤差が混ざらない。`estimate_time_offset.py --event apex` で、カメラごとに画像上の軌跡へ 2 次式を当てはめて頂点の時刻を出す。5 回の平均で ±2 ms を見込む (仮定。記録が無く未検証)。
- 照明の点滅のように画面全体が変わる出来事は、フレームの間隔 (120 fps で 8 ms) までしか分からない。10 ms 以上の大きなずれが無いことの確認にだけ使う。

#### A.8 監視

- カメラごとに 10 秒おきにログに出す: 時計の差、往復時間の最小と中央値、撮影から受信までの最小と中央値、時計の種類、従来の方法に切り替えたかどうか。
- `-j` を付けたときは、イベントのログと同じ場所に次の 2 つを残す。
  - `sync_*`: 時刻合わせの記録。
  - `frames_*`: 受信した全フレーム。1 行 1 フレームの JSON で、camera_id、frame_id、センサー時刻、受信時刻、直した時刻、検出を入れる。オフラインのツールの入力になり、② のログの書式に依存しない。
- 投球ごと (② の後): 視線と軌道の距離、その球で最も合う時刻のずれ、カメラごとの視線の数。直近 10 球の距離の中央値が 3 cm を超えたら警告する (10/10 のデータでは正しい時刻で 1.4 cm、25 ms ずれると 3.8 cm)。

### B. カメラの位置・向き・焦点距離の較正

#### B.1 横のずれの切り分け (大きなものを作る前に)

2.4 の計算から、候補を「分かる量 ÷ 手間」の順に並べる。

| 順 | 候補 | いま分かっていること | 次にやること | 見込み |
|---|---|---|---|---|
| 1 | カメラの位置・向きの誤差 | 線 4 本から 7 変数を解いていて、線 1 cm のずれが位置 4 cm になる。右を 5〜10 cm 下げると横の偏りが半分〜ほぼ 0 になる。飛球からは見分けられない | 【ユーザー】レンズ位置をメジャーで 3 方向、筐体の実寸を 6 か所。位置を固定して向きを解き直す。静止したボールで三角測量 (B.3) | 最有力。3〜4 cm |
| 2 | ローリングシャッター | 左 12 ms で y −1.1 cm、z の二乗平均 2.0→1.2 cm。速い球ほど効く | `estimate_time_offset.py` で走査して再現。KYV45 は `CaptureResult` の値を読む | 約 1 cm。速さへの依存の大半 |
| 3 | 2 台の時計の残り | ±5 ms で ±0.9 cm。球ごとのばらつき 4.5 ms | A の実装後に 20 球で推定 | 1 cm 以下 |
| 4 | 検出の中心の遅れ (計画 ①) | 遅れは約 0.25 コマで 2 台ほぼ同じ。共通の遅れは着弾時刻が遅れるだけで、位置は動かない。片方だけ 0.5 コマ遅れると ±1.3〜1.7 cm | ① の修正後に同じ動画で検出し直して比べる | 2 台の差は約 1 ms → 0.3 cm 以下 |
| 5 | レンズの歪み | iPhone の動画は補正済み (仮定)。KYV45 は隅で 1.5% 以下 (粗い) | チェッカーボード (B.5) の副産物で確認 | 1 cm 未満 |
| 6 | 正解の誤差 | 焦点距離の仮定で 0.8 cm。着弾の瞬間が 8 ms ずれると、6 m/s の球で横に約 1 cm | 奥面に印を付けて測る (B.3 の静止点と兼ねる) | 1〜2 cm 以下 |
| 7 | 画像の端で切れた検出 | 除いても変わらない | なし | 小 |

10/10 の配置ではスマホの実際の位置を測っていないので、このデータだけでは 1 と 6 を切り分けきれない。決着は KYV45 を置いた状態での実測で付ける (手順 U-3)。

#### B.2 採用する較正の方式

いまの方法 (位置 3、向き 3、焦点距離 1 を反対側の壁の線 4 本から同時に解く) は、線が 1 枚の壁の上にしか無く、筐体データと実物のずれを 4 倍に広げる。次のように分ける。

1. 端末ごと・撮影モードごとに 1 回: 焦点距離をチェッカーボードで求める (B.5)。以後は固定する。
2. 置くたびに: レンズの位置をメジャーで測って入れ (許す幅 1 cm)、向きだけを線と印から解く。
3. 確認: 位置の分かった静止点を 2 台で三角測量する。

位置を固定すると、線 1 cm のずれの影響が 1 球で最大 5.2 cm から 1.8 cm に減る (2.4)。向きだけなら線 4 本で 8 つの条件に対して未知数が 3 つになり、当てはめの残差が確認として意味を持つ。

#### B.3 ツール: `scripts/recording_analysis/calibrate_camera.py`

スクラッチパッドの `calib.py` (線の縁の点の抽出、Levenberg–Marquardt) を移植する。サブコマンドは 2 つ。

`pose` (位置と向きを解く)

- 入力
  - アプリが保存した解析画像 1 枚 (B.4)。10/10 の確認用には動画から切り出した静止画も受け付ける。
  - 筐体の基準 `scripts/recording_analysis/booth.json`: 名前の付いた線分と点 (筐体データの座標、m) と、それぞれの確からしさ (cm)。左の壁、右の壁、奥面、上の横木を別々に持つ。右の壁を左の壁の鏡像とは仮定しない。ユーザーが測った実寸で上書きできる。
  - レンズの位置 (メジャーの値) と許す幅、焦点距離 (固定)。
  - 線のおおよその位置。位置と大まかな向きから線を画像に映して近くの縁を探すのを基本にし、見つからないときは `--pick` で線ごとに 2 点をクリックして JSON に残す。
- 解き方
  - 線ごとに重みをそろえる (点の数ではなく線 1 本を 1 つの観測として扱う)。
  - 既定は「位置を固定、向きだけ」。`--free-position`、`--free-focal`、`--radial` (歪み 1 項) は診断と 10/10 の再現用。
- 出力
  - 設定画面にそのまま入れる値。x, y, z は ball_tracker の座標で、奥面でボールの中心が x = 0 になるよう x から 0.0455 + ボールの半径を引く。
  - roll, pitch, yaw。換算は R = rcwᵀ · D (D は `defaultBackCameraRotation(90)`)、yaw = atan2(R₁₀, R₀₀)、pitch = −asin(R₂₀)、roll = atan2(R₂₁, R₂₂)。この式が `replay_all.json` の値 (左 139.821 / −2.049 / −30.955、右 134.593 / −1.112 / −150.28) を再現することは確認済み。
  - 焦点距離 (解析画像の px と、幅に対する比)、線ごとの残差 (px と cm)、推定の幅。
  - 重ね描きした画像。使った線は色つき、使っていない線は白で描き、独立した確認に使う。結果の JSON。

`check` (静止点の確認)

- 奥面や壁の、位置を測った印にボールを当てた状態で 2 台の画像を保存し、ボールの中心 (色で自動、だめならクリック) を三角測量して、測った位置との差と 2 本の視線の離れを表にする。
- 時計・ローリングシャッター・検出の遅れ・正解の動画のどれも関わらないので、位置と向きだけの確認になる。

置くたびの手順:

1. レンズの位置をメジャーで測る。
2. アプリで画像を保存する。
3. `scripts/pull-calibration-frames.sh` で PC に取り込む。
4. `calibrate_camera.py pose` を実行し、重ね描きと残差を見る。
5. 表示された値を設定画面に入れる。ball_tracker のログ (`network.rs:216` の `camera pose updated`) で値を見比べる。
6. 初回と置き直したときは `check` を行う。

#### B.4 較正用の画像

- 標準カメラの写真は使わない。解析に使う出力と切り出し・画角が違いうる (高速撮影は特に未確認)。
- 画面のスクリーンショットも使わない。検出枠が入り、拡大されていて画素の対応が取れない。
- 両方のアプリのカメラ画面に「Save frame」を足す。
  - 次のフレームを、枠を描く前・センサーの向きのまま PNG で `getExternalFilesDir("calibration")` に保存する。
  - 同じ名前の JSON に、画像の大きさ、アプリが使っている内部パラメータ、`SENSOR_ORIENTATION`、時計の種類、行ごとの時刻差、撮影の大きさと fps、機種名を書く。
  - 取り込みは adb (`scripts/adb-reverse.sh` と同じく、接続中の全端末をシリアルごとのフォルダに分ける)。
- 解析画像は 640 px 幅だが、線は長いので縁は 0.2〜0.3 px で決まる (0.03° 程度)。向きに必要な 0.5° に対して十分。

#### B.5 焦点距離の求め方とアプリへの入れ方

- `scripts/recording_analysis/calibrate_intrinsics.py`: チェッカーボード (A3 の印刷か PC の画面) を「Save frame」で 15 枚ほど撮り、OpenCV で焦点距離・光学中心・歪み 1 項を求める。端末 2 台 × 使う撮影モードで 1 回ずつ。高速撮影で画角が狭まっていないかも、これで分かる。
- 設定画面の `Camera Pose` に「Focal length (画像の幅に対する比。0 で自動)」を 1 つ足す。
  - 例: 640 px 幅で 482 px なら 0.753。
  - 0 以外のとき、`CameraRepository.calculatorFor` は fx = fy = 比 × 画像の幅を使う。光学中心は画像の中央のまま (中央からのずれは、向きを解くときにほぼ吸収される)。
- 位置と向きは今までどおり設定画面に入れ、`UpdateCameraPose` で送る。プロトコルは変えない。使われている焦点距離は `CameraInfo` で ball_tracker のログに出る。
- PC 側に較正ファイルを持って上書きする案は採らない。camera id が接続順で決まり、どの端末かを PC から区別できないため。

#### B.6 レンズの歪み

- アプリでは扱わない (いまのまま)。
- 根拠: KYV45 の動画で隅でも 1.5% 以下 (640 px 幅で 5 px、0.6° 程度)。中心から半分の距離では 1 px 未満。向きを線から解くので、低次の成分は向きに吸収される。着弾への影響は 1 cm 未満の見込み。
- B.5 のチェッカーボードで実際の値が出る。ボールの通り道で 640 px 幅の 1.5 px を超えていたら、`WorldDirectionCalculator` に 1 項を足すことを別途検討する。
- 線だけから歪みを安定して求めるのは難しい (2.4 の試算でも値がばらついた)。`--radial` は目安にとどめる。

## 4. 影響するファイル

担当ごとに所有を分ける。ほかの担当は触らない。

担当 R (Rust と proto)

- `api/proto/camera__ball-tracker.proto`
- `ball_tracker/src/clock_sync.rs` (新規)
- `ball_tracker/src/detection_input/network.rs`
- `ball_tracker/src/detection_input/mod.rs` (② の完了後。フレームの時刻を作る箇所だけ)
- `ball_tracker/src/tracking/` (② の完了後。A.5 の 3 点だけ)
- `ball_tracker/src/lib.rs` (`pub mod clock_sync;` の追加と camera id の採番だけ)
- `ball_tracker/src/main.rs` (起動時の指定と `ClockSync` の受け渡し)
- `docs/transport_protocol.md` (カメラと ball_tracker の間の節を追加)

担当 K (Kotlin。両方のアプリの同じ場所)

- 時刻: `network/TcpSessionImpl.kt`、`network/DataConnectionImpl.kt`、`network/NetworkManager.kt`、`network/types/DetectionData.kt`、`camera/CameraViewModel.kt`、`camera/MyAnalyzer.kt` (CameraX 版)、`camera/highspeed/HighSpeedCamera.kt` (高速版)、`camera/CameraClockInfo.kt` (新規)
- 較正: `camera/CameraScreen.kt`、`camera/CameraViewModel.kt`、`camera/CalibrationFrameSaver.kt` (新規)、`camera/CameraRepository.kt`、`config/ConfigScreen.kt`、`config/ConfigUiState.kt`、`config/ConfigViewModel.kt`、`config/ConfigStoreRepository.kt`、`config/ConfigStoreRepositoryImpl.kt`、`app/src/test/.../FakeConfigStoreRepository.kt`、各アプリの `README.md`

担当 P (Python と手順)

- `scripts/recording_analysis/predict_landing.py`、`replay_to_tracker.py`、`README.md`
- 新規: `estimate_time_offset.py`、`calibrate_camera.py`、`calibrate_intrinsics.py`、`booth.json`、`scripts/pull-calibration-frames.sh`
- `recordings/2026-10-10/` の設定ファイル (git の管理外)

この計画で触らないもの: `camera/ObjectTracker.kt` (計画 ①)、`collision_output/*`、projector。

## 5. 実装手順と担当

実装は担当ごとのサブエージェントが行い、段階ごとに別のレビュー担当が確認する。【ユーザー】は実測や端末が要る手順。

### 段階 0: 事前の確認 (コード変更なし)

- U-1【ユーザー: 端末 2 台を接続】`adb -s <シリアル> shell dumpsys media.camera` の出力から、`android.sensor.info.timestampSource`、`android.info.supportedHardwareLevel`、`android.request.availableResultKeys` に `sensor.rollingShutterSkew` と `sensor.exposureTime` があるか、高速撮影の大きさと fps を控える。結果で A.3 の手順 3 の既定が決まる (設計はどちらでも動く)。
- U-2【ユーザー: メジャー】筐体の実寸を左右の壁それぞれで測る: 内側の幅、斜めの木の縁の位置 (上下 2 か所)、手前の柱の位置、板の継ぎ目の高さ、上の横木の高さ、奥面の継ぎ目の高さ。`booth.json` に入れる。

### 段階 1: 既存データでの再現 (担当 P。ほかに依存しない)

- P-1 `predict_landing.py` を拡張する: カメラごとの `focal_px` (いまは全体で 1 つ。`predict_landing.py:68`)、カメラごとの行の時刻差、時刻の補正。10/10 用の設定を作り、2.4 の基準の数字を再現する。
- P-2 `estimate_time_offset.py` を作り、10/10 で球ごとの値と行の時刻差の走査を再現する。

### 段階 2: 時刻合わせ (R は計画 ② の後。K は計画 ① の後。P は並行)

- R-1 proto を A.4 のとおり変える。
- R-2 `clock_sync.rs` を作る: 時刻合わせの標本と最小往復時間の選択、時計の種類の判定、従来の方法 (窓つきの最小値)、`--time-trim-ms`、共通の遅延、`frame_time`、`detection_time`、ログ。`observe_throw` は受け口だけ。
- R-3 `network.rs` を直す: 6060 番で要求を送り応答と `CameraInfo` を受ける、切断の処理、`frames_*` のログ。フレームの時刻を `frame_time` で作る。`lib.rs` の採番。`main.rs` の指定。
- R-4 `docs/transport_protocol.md` に節を足す。
- P-3 `replay_to_tracker.py` を拡張する: `sensor_timestamp_ns` と `image_x`、`image_y` を送る、時刻合わせの要求に答える、カメラごとに任意の時計のずれと進み方の差を付ける。設定で切り替え、既定は今までと同じ動作。
- K-1 (① の後) アプリの時刻:
  - `TcpSessionImpl` に受信の処理を足し、要求に答える。受けた直後と書く直前に時刻を取る。応答は `outChannel` を通さず、書き込みを直列化して直接書く。
  - 2 つの接続に `tcpNoDelay` を付ける。
  - `CameraInfo` を送る。時計の種類は `CameraCharacteristics` から、行の時刻差と露光時間は最初の `CaptureResult` から取る。
  - 検出にセンサー時刻 (ns) と画像上の位置を付ける。いまの `timestamp` は残す。
- レビュー 1: A.4 の番号と互換性 (新旧 4 通りの組み合わせ)、所有外のファイルに触れていないこと、未コミットの変更が残っていること (作業前後の `git status --short` と `git diff --stat` を比べる)。

### 段階 3: 較正 (P は段階 1 の後いつでも。K は K-1 の後)

- P-4 `booth.json` を作る (スクラッチパッドの `calib.py` の `LEFT_WALL` と `calib_other.py` の `LINES` を元に、U-2 の実寸で直す)。
- P-5 `calibrate_camera.py` (`pose`、`check`) を作る。
- P-6 `calibrate_intrinsics.py`、`pull-calibration-frames.sh`、`README.md` を作る・直す。
- K-2 「Save frame」と JSON (B.4)。
- K-3 焦点距離の設定 (B.5)。`FakeConfigStoreRepository.kt` も合わせる。
- レビュー 2: 換算の式 (B.3)、設定画面の値の往復、両方のアプリで同じ変更になっていること。

### 段階 4: 実機での確認【ユーザー: 端末、筐体】

- U-3 KYV45 を置き、B.3 の手順で較正する。チェッカーボード (初回のみ)、静止点の確認。
- U-4 ball_tracker を `-j` で動かし、時刻合わせのログを見る。トス試験 5 回、投球 20 球 (別角度の動画つき)。
- U-5 `estimate_time_offset.py` と `predict_landing.py` で 6 章の基準を確かめる。

### 段階 5: 追跡との接続 (計画 ② の後)

- R-5 A.5 の 3 点を追跡に入れ、`observe_throw` の中身 (投球ごとの推定、監視、`--auto-time-trim`) を入れる。
- レビュー 3。

## 6. 検証方法と受け入れ基準

次はどれもユーザーの了承が要る: `cargo build`、`cargo test`、Gradle のビルドとテスト、ball_tracker を起動しての再生、端末での確認、新しいテストの追加。追加してよいか決めてほしいテストは、`clock_sync.rs` の計算 (最小往復時間の選択、時計の種類の判定、窓つきの最小値) と、`calibrate_camera.py` の角度の換算。

既存データ (10/10) での確認:

| 項目 | 基準 |
|---|---|
| P-1 の再現 | y 平均 +5.4 (二乗平均 6.1)、z 平均 +0.9 (二乗平均 2.0)、残差 1.4 cm (±0.1) |
| P-2 の再現 | 球ごとの平均 23.7 ms、中央値 22.9 ms (±0.5)。行の時刻差の最小が左 12 ms 付近 |
| 行の時刻差を入れた予測 | y 平均 +4.3 (±0.3)、z の二乗平均 1.2 cm 以下、全球 10 cm 以内 |
| `calibrate_camera.py` (自由度をすべて開けた設定) | 左 (0.257, 0.180, 1.230)、1782 px、残差 0.57 px。右 (0.163, 0.993, 1.340)、1466 px、残差 0.99 px。位置 2 mm、角度 0.05°、焦点距離 2 px 以内 |
| 角度の出力 | `replay_all.json` の roll, pitch, yaw と 0.01° 以内で一致 |
| 従来の経路の再生 (`replay_ball_only.json`、変更前の `replay_to_tracker.py` と同じ送り方) | 計画 ② の完了時点と同じ球を報告し、位置の差 1 cm 以内 |
| 新しい経路の再生 (P-3。時計に ±1000 秒のずれと ±40 ppm の差を付け、右の +25 ms は `--time-trim-ms 25` で与える) | 上と同じ球、差 1 cm 以内 |

10/10 のデータでは、横の偏りの目標 (2 cm 以下) は求めない。スマホの実際の位置を測っておらず、位置の誤差と正解の誤差を分けられないため。このデータでの合格は、偏りが B.1 の内訳で説明でき、ツールが同じ数字を再現することとする。

実機 (U-3〜U-5) での受け入れ基準:

| 項目 | 基準 | 根拠 |
|---|---|---|
| 時刻合わせ | 2 台とも往復時間の最小 5 ms 以下。時計の差は、直線的な変化を除いて 10 分間 ±0.5 ms 以内 | USB 経由の仮定。満たせなければ値を見直す |
| 2 台の時刻のずれ | 20 球からの推定が ±3 ms 以内で、2 回の測定の差が 1.5 ms 以内。トス試験と 3 ms 以内で一致 | ±5 ms で 0.9 cm。予算は 2 ms |
| 視線と軌道の距離 | 20 球の中央値 1.5 cm 以下 | 10/10 で 1.3〜1.4 cm |
| 静止点 (5 点以上) | 各点 2 cm 以内。横の差の平均 1 cm 以内 | 位置 1 cm で 0.45 cm、向き 1° で 0.5 cm 程度 |
| 着弾 (20 球、正解つき) | 横・高さとも偏りの平均 2 cm 以内。1 台 3 コマ以上映った球はすべて 10 cm 以内 | `simulate_layouts.py` では、向き 2°・位置 2 cm・時計 5 ms の誤差で 10 cm 以内が 87% |

## 7. リスクと未決事項

計画 ①・② との関係:

- ① (端末側の検出): `camera/CameraRepository.kt` を ① も変える。K-1〜K-3 は ① の後に行う。① の後は色の判定が変わるだけで検出の位置は変わらない見込みだが、時刻のずれの推定 (U-4) は ① を入れた状態で測る。
- ② (追跡と組み合わせ): `detection_input/`、`lib.rs`、`main.rs`、`tracking/` は ② が大きく書き換える。R の作業は ② の完了後に、その状態を基準にして行う。② の前にできるのは段階 0・1 と P の作業、proto の番号の確定まで。
- 調査時点では SQLite 入力が `PairedFrames` の変更に追従していなかった。この入力経路は #298 で廃止した。

リスク:

- KYV45 の時計の種類が `UNKNOWN` で、センサー時刻が uptime からも揺れている場合。従来の方法に切り替わり、いまと同じ精度に戻る。飛球からの推定で一定のずれは取れるが、揺れは取れない。U-1 と U-4 で早めに分かる。
- 行ごとの時刻差が端末から取れない場合。飛球からの走査で求めるが、決まり方が緩い (10/10 では ±4 ms 変えても残差が 0.02 cm しか変わらない)。必要なら、既知の周波数で点滅させた LED を撮って縞の数から求める (未検証の案)。
- 解析画像が有効画素の一部の切り出しなので、端末が返す時刻差をそのまま画像の上端から下端に当てはめてよいかは未確認。読み出しの向きが画像の上から下であることも仮定。`--rolling-shutter-skew-ms` で上書きできるようにする。
- 2 台の露光時間が違うと、その差の半分がずれになる (120 fps で最大 4 ms)。`CameraInfo` で見えるようにし、一定の分は飛球からの推定で吸収する。
- PC の時計が飛ぶと、数秒間 (時刻合わせの窓の長さ) ずれる。
- 飛球から求めたずれには位置・向きの誤差が混ざる。補正の幅を ±5 ms に抑え、端に張り付いたら位置を疑う。
- 設定画面への手入力の間違い (符号、桁)。ball_tracker のログと見比べる手順で防ぐ。

未決事項 (ユーザーの判断が要る):

- 位置を固定して向きを解いても横の偏りが残った場合の扱い。奥面の印に当てた結果から、出力の (y, z) に補正を掛ける案がある。`collision_output` か projector 側の変更になり、この計画の範囲外。
- 設定値を adb から流し込む仕組み (手入力をなくす) を作るか。
- オンラインの自動補正 (`--auto-time-trim`) を既定で有効にするか。実機で数日ぶんのログを見てから決めるのがよい。
- 新しいテストを追加するか (6 章に挙げた 2 か所)。
