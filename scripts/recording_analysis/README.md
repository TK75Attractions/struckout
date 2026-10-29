# recording_analysis

撮影した動画を PC 上で解析し、ball_tracker の検証に使うデータを作るスクリプト。
動画そのものは `recordings/` に置く (サイズが大きいので git では追跡しない)。

必要なもの: Python 3、`numpy`、`opencv-python`、PATH 上の `ffmpeg` と `ffprobe`。

| スクリプト | 役割 |
|---|---|
| `sync_audio.py` | 別々の端末で撮った動画どうしの時刻のずれを、音声の照合で求める |
| `landing_truth.py` | 別角度から撮った動画から、ボールが奥面に当たった位置と時刻 (正解データ) を求める |
| `detect_objects.py` | カメラアプリの検出処理 (`ObjectTracker.kt`) を動画に対して再現する |
| `predict_landing.py` | 2 台のカメラの検出結果から着弾位置を予測し、正解と比べる |
| `estimate_time_offset.py` | 飛んでいるボールの検出結果から、2 台のカメラの時刻のずれと、行ごとの露光の時刻差を見積もる |
| `simulate_layouts.py` | カメラの置き方ごとに、着弾位置の予測がどのくらい当たるかをシミュレーションで比べる |
| `replay_to_tracker.py` | 録画から作った検出結果を、カメラアプリと同じ通信で ball_tracker に流し込む |
| `replay_stream_to_tracker.py` | ball_tracker が保存した動画 (`.vsp`) を、スマホが送ったときと同じ間隔で ball_tracker に流し直す。PC での解析の遅れを実機なしで確かめる |
| `downscale_vsp.py` | ball_tracker の録画 (`.vsp`) の動画を小さい大きさの H.264 に再エンコードし、時刻と順番を保った `.vsp` と `.h264` を作る。スマホが小さい大きさで送った場合の PC 側の処理を実機なしで確かめる |
| `stream_video_to_tracker.py` | 普通の動画を H.264 にして、カメラアプリと同じ手順で ball_tracker に流す |
| `bench_ffmpeg.py` | ball_tracker と同じ設定で ffmpeg に録画 (`.h264`) をデコードさせ、GPU の方式と出力 (`full`、`decode`、`yuv`、`nv12`、`gpuscale`)、同時に動かす本数ごとに CPU の時間を測る。`gpuscale` は d3d11va / d3d12va 用。`numpy` と `opencv-python` は要らない |
| `calibrate_camera.py` | 筐体の線が映った解析画像から、カメラの向き (診断用に位置と焦点距離も) を求める |
| `calibrate_intrinsics.py` | チェッカーボードを撮った解析画像から、焦点距離を求める |

カメラアプリの「Save frame」で保存した画像は、`scripts/pull-calibration-frames.sh` で PC に取り込む。

## landing_truth.py

```bash
python scripts/recording_analysis/landing_truth.py recordings/2026-10-07/IMG_7044.MOV recordings/2026-10-07/landing_truth.json --out recordings/2026-10-07/impacts.csv --debug-dir recordings/2026-10-07/landing_truth_debug
```

奥面の上にある位置の分かった点を 4 点以上使って、画像の座標を奥面上の座標に直す。
ボールの軌跡が最初に折れ曲がる点を着弾とみなす。`--debug-dir` を付けると、軌跡と着弾点を
描き込んだ画像を 1 球ごとに保存するので、必ず目で確かめること。

設定ファイル (JSON) の項目:

| 項目 | 内容 |
|---|---|
| `image_size` | 動画の幅と高さ (px) |
| `focal_px` | カメラの焦点距離 (px)。視差の補正にだけ使う。iPhone の標準カメラの 4K なら 2800 前後 |
| `roi` | 解析する範囲 `[x, y, 幅, 高さ]` (px)。奥面とボールの通り道が入るようにする |
| `scale` | 解析前に縮める倍率。4K なら 0.5 で足りる |
| `ball_diameter_cm` | ボールの直径 |
| `reference_points` | 基準点。`image` は動画上の座標 (px)、`plane_cm` は奥面上の座標 (y, z) |
| `plane_bounds_cm` | 奥面の範囲。この外で折れ曲がった点は着弾とみなさない |
| `throws` | 1 球ごとのコマ番号の範囲 `[最初, 最後]`。投げる少し前から、着弾後に床へ落ちるまで |

出力する座標は `reference_points` の `plane_cm` と同じ座標系 (cm)。出力の主な列:

| 列 | 内容 |
|---|---|
| `time_s` | 着弾の時刻 (動画の先頭からの秒) |
| `y_cm`, `z_cm` | 着弾したときのボールの中心の位置 |
| `fit_mismatch_cm` | 着弾の前後に当てはめた 2 本の軌跡が、着弾時刻でどれだけ食い違うか。大きいと怪しい |
| `turn` | 着弾の前後での速度の変化 (着弾前の速さに対する比)。0.5 に近いと折れ曲がりが弱く怪しい |

## sync_audio.py

```bash
python scripts/recording_analysis/sync_audio.py recordings/2026-10-07/IMG_7044.MOV recordings/2026-10-07/cam_131208/KVID0003.mp4
```

基準の動画 (短いほう) の 0 秒が、ほかの動画の何秒にあたるかを表示する。
音が届くまでの時間差で 10 ms 前後の誤差が残る。

## detect_objects.py

```bash
python scripts/recording_analysis/detect_objects.py recordings/2026-10-07/cam_131208/KVID0003.mp4 --start 224 --duration 26 --out detections.csv
```

処理の手順と既定値は `struckoutCameraApp` の `ObjectTracker` と `CameraRepository` に合わせてある。
アプリ側を変えたらこちらも合わせること。画像はセンサーの向きのまま (端末を縦に持って撮った動画は
横倒し) で扱う。
コマは動画の先頭から復号して番号で選ぶ。`-ss` で頭出しすると、iPhone の動画では 1〜2 コマずれた。

`--ignored-margins 左,上,右,下` は、アプリの設定画面の Detection Area と同じ意味で、プレビュー (解析画像を
時計回りに 90 度回したもの) の各辺から無視する幅を百分率で指定する。

`--min-color-shift` は、検出した領域を色みの変化で絞り込むときの下限 (既定 10)。0 にすると色みでは
絞り込まない。色みの変化は、領域の内側の平均色を背景の同じ場所の平均色と比べたときに、
「明るさだけが変わった」(影や照明) とみなしても、「全チャンネルが同じ量だけ増減した」(色のないものどうしの
変化) とみなしても残る分で、単位は 8 ビットの階調。以前の `--min-saturation` (平均色の彩度の下限) は、
ぶれて色が薄まったボールを捨てていたのでやめた。

出力の列:

| 列 | 内容 |
|---|---|
| `frame` | コマ番号 (動画の先頭を 0 として、表示順に数える) |
| `time_s` | そのコマの時刻 (動画の先頭からの秒) |
| `x`, `y`, `width`, `height` | 検出した領域を囲む四角形 (解析した画像での px) |
| `color_shift` | 背景からの色みの変化。`--min-color-shift` と比べる値 |
| `saturation` | 平均色の彩度 (0 から 1)。以前の判定 (0.25 以上) と比べるための値で、絞り込みには使わない |

`color_shift` の列がない CSV は、判定を替える前の版 (`--min-saturation`) で作ったもの。

## predict_landing.py

```bash
python scripts/recording_analysis/predict_landing.py recordings/2026-10-07/predict_landing.json
```

`detect_objects.py` の検出結果を視線に直し、ball_tracker と同じ計算 (放物線を視線に当てはめて的の面まで
延長する) で着弾位置を出して、`landing_truth.py` の正解と比べる。計算は
`ball_tracker/src/tracking/trajectory.rs` と同じなので、そちらを変えたらこちらも合わせること。

設定ファイル (JSON) の項目。ファイルの場所は設定ファイルからの相対パスで書く。

| 項目 | 内容 |
|---|---|
| `truth` | `landing_truth.py` が出力した CSV |
| `image_size` | `detect_objects.py` で解析した画像の幅と高さ (px) |
| `focal_px` | その画像での焦点距離 (px)。全カメラ共通の値。カメラごとに `cameras[].focal_px` を書けば省略できる |
| `plane_x_m` | 的の面の x 座標 (m)。正解と同じ座標系で書く |
| `window_s` | 着弾時刻の何秒前から何秒前までの検出を使うか (下の補正を足したあとの時刻で比べる)。着弾後の跳ね返りを入れないよう、終わりは負にする。ボールの検出だけに絞った CSV なら広く取ってよい |
| `min_bbox_px` | これより小さい検出は使わない |
| `cameras[].detections` | `detect_objects.py` が出力した CSV |
| `cameras[].time_offset_s` | 正解を作った動画の 0 秒が、このカメラの動画の何秒にあたるか (`sync_audio.py` の結果) |
| `cameras[].focal_px` | このカメラの焦点距離 (px)。省略すると全体の `focal_px` を使う |
| `cameras[].time_correction_ms` | このカメラの時刻に足す補正 (ms)。省略すると 0 |
| `cameras[].rolling_shutter_skew_ms` | 行ごとの露光の時刻差 (ms)。画像の上端の行と下端の行が写る時刻の差。省略すると 0 |
| `cameras[].position_m` | カメラの位置 (m)。正解と同じ座標系で書く |
| `cameras[].rotation_deg` | カメラアプリの `Camera Pose` に入れるのと同じ roll, pitch, yaw (度) |

検出 1 件の時刻は「フレームの時刻 + `time_correction_ms` + `rolling_shutter_skew_ms` × 検出の中心の縦位置
(画像の上端が 0、下端が 1)」になる。行ごとの時刻差は画像の上端から測るので、`rolling_shutter_skew_ms` を
変えると、合う `time_correction_ms` も変わる。2 つは `estimate_time_offset.py` が組にして表示する。

結果の最後に、誤差の平均と標準偏差、二乗平均と最大、視線と軌道の距離 (残差) の平均を表示する。

2026-10-10 の録画には設定が 2 つある。`predict_landing_time25.json` は右の時計を +25 ms しただけのもの、
`predict_landing_row_skew.json` は左のカメラに行ごとの時刻差 12 ms を入れたもの (右の時計は +31 ms)。

## estimate_time_offset.py

```bash
python scripts/recording_analysis/estimate_time_offset.py recordings/2026-10-10/predict_landing_time25.json
```

2 台の時刻が合っていれば、2 台の視線は 1 本の放物線に乗る。右のカメラ (設定した位置の y が大きいほう) の
時刻を動かして、視線と軌道の距離がいちばん小さくなるずれを 1 球ごとに探し、球ごとの値、平均、中央値、
標準誤差 (標準偏差 ÷ 球数の平方根)、全球をまとめたときの最小を表示する。正解の着弾位置は使わない。
表示するずれは、右のカメラの `time_correction_ms` に入れる値。

続けて、行ごとの露光の時刻差 (`rolling_shutter_skew_ms`) を 1 台ずつ動かした結果も表示する。行ごとの時刻差を
変えると合う時刻のずれも動くので、値ごとにずれを合わせ直している。設定に入れるときは、表示されたずれと
組にして入れること。2 台の画像をボールが逆向きに横切る置き方では、どちらのカメラに付けても距離が
縮むので、どちらのカメラの値かは決めにくい。

設定ファイルは `predict_landing.py` や `replay_to_tracker.py` と同じ書式で、次の項目を読む
(`truth`、`plane_x_m`、`window_s` は使わない)。

| 項目 | 内容 |
|---|---|
| `image_size`、`min_bbox_px`、`time_range_s` | `replay_to_tracker.py` と同じ。`min_bbox_px` と `time_range_s` は省略できる |
| `cameras[].detections` | ボールの検出だけにした CSV。ボール以外が混ざると結果が狂う |
| `cameras[].time_offset_s`、`focal_px`、`position_m`、`rotation_deg` | `predict_landing.py` と同じ |
| `cameras[].time_correction_ms` | 左のカメラの値はそのまま使う。右のカメラの値は「いまの設定での距離」を出すのにだけ使う |
| `cameras[].rolling_shutter_skew_ms` | 2 台とも設定の値のまま使う (走査するときは、動かすほうだけ置き換える) |

主なオプション:

| オプション | 内容 |
|---|---|
| `--min-frames` | 1 台あたりのコマ数がこれより少ない球は使わない (既定 4) |
| `--range-ms 最小 最大` | 時刻のずれを探す範囲 (既定 -30 60) |
| `--skew-range-ms 最小 最大` | 行ごとの時刻差を探す範囲 (既定 -30 30) |
| `--gap-s` | 検出がこの秒数より長く途切れたら、別の球とみなす (既定 0.3) |
| `--event apex` | 下のトス試験 |

求めたずれには、カメラの位置と向きの誤差も混ざる (2026-10-10 の録画では、右のカメラの位置を奥行きに
1 cm 動かすと約 2.5 ms 変わる)。球数の目安は 10 球 (標準誤差 1.5 ms)、できれば 20 球。

### トス試験 (`--event apex`)

2 台の両方に映る場所でボールを真上に軽く投げ上げ、頂点の時刻を 2 台で比べる。カメラごとに、画像上の
軌跡へ時刻の 2 次式を当てはめて頂点の時刻を出す。カメラの位置と向きは使わない。

投げ上げの録画がまだ無いので、実際のデータでは確かめていない。計算で作った検出では、真上に上がって
落ちるボールなら 1 ms 以内で合うが、ボールが左右 (2 台を結ぶ向き) に 0.1 m/s 流れると、求めたずれが
7〜12 ms 変わった。流れの小さい回だけで見ること。表示される「横の流れ」は、そのカメラから見て横切る
向きの流れだけを表す (2 台が正面から向かい合う置き方では、問題になる流れが映らない)。

## simulate_layouts.py

```bash
python scripts/recording_analysis/simulate_layouts.py
```

投球をランダムに作り、カメラの画角・フレームレート・露光中のぶれ・時計のずれ・取り付けの誤差を入れて、
置き方ごとの着弾位置の誤差を出す。奥面 (プロジェクターの投影) やプレイヤー側を背景にした画素は
使えないものとして、観測から外している。置き方の候補は `layouts()` に、誤差の仮定はファイルの先頭にある。
誤差の仮定は 2026-10-07 の撮影をもとにした値で、いまの配置については実測 (横 ±20cm、高さ +6.5cm) と
同じ傾向になることを確かめてある。

## replay_to_tracker.py

```bash
./target/release-tracker/ball_tracker.exe --input network --output csv -c recordings/2026-10-10/collisions.csv -j
```

```bash
python scripts/recording_analysis/replay_to_tracker.py recordings/2026-10-10/replay_all.json
```

先に ball_tracker を起動しておき、別のターミナルから実行する。カメラの登録 (6060 番) と検出結果の送信
(5050 番) を実機と同じ手順で行い、検出結果は録画どおりの間隔で送る。ball_tracker に `-j` を付けると、
着弾の報告が時刻つきで `%APPDATA%/struckout/tracker/json/events_*` に残る。

`events_*` は JSON を続けて書いたもので、1 つが 1 回の処理ぶん。`timestamp` はその処理をした時刻 (PC の時計) で、
`Collision` のときは着弾の報告を送り出した時刻にあたる。録画の時刻に直すには、スクリプトが最後に表示する
「基準の時計の何秒が UNIX 秒のいくつにあたるか」を使う。`NewTrack` と `UpdateTrack` には、トラックの視線の本数
(カメラの番号ごと)、視線 1 本の誤差の見積もり、予測している着弾の時刻と位置が入る。

設定ファイル (JSON) の項目。ファイルの場所は設定ファイルからの相対パスで書く。

| 項目 | 内容 |
|---|---|
| `image_size` | `detect_objects.py` で解析した画像の幅と高さ (px) |
| `time_range_s` | 送る範囲 (全カメラ共通の時計での秒) |
| `min_bbox_px` | これより小さい検出は送らない |
| `cameras[].detections` | `detect_objects.py` が出力した CSV |
| `cameras[].time_offset_s` | 共通の時計の 0 秒が、このカメラの動画の何秒にあたるか |
| `cameras[].focal_px` | 解析した画像での焦点距離 (px) |
| `cameras[].position_m` | カメラの位置 (m)。ball_tracker の座標 (的の面でボールの中心が x = 0) で書く |
| `cameras[].rotation_deg` | カメラアプリの `Camera Pose` に入れるのと同じ roll, pitch, yaw (度) |
| `time_sync` | true にすると、時刻合わせに対応したアプリと同じ動きをする (下の説明)。省略すると false |
| `cameras[].clock_offset_s`、`clock_drift_ppm` | `time_sync` のとき、そのカメラの時計を PC からずらす量と、進み方の差 |
| `cameras[].timestamp_source` | `time_sync` のとき、センサー時刻の時計の種類。`realtime` (既定) か `unknown` |
| `cameras[].uptime_lag_s` | `timestamp_source` が `unknown` のとき、センサー時刻の時計が遅れている量 (端末が眠っていた時間にあたる) |
| `cameras[].rolling_shutter_skew_ms` | `time_sync` のとき、ball_tracker に知らせる行ごとの露光の時刻差 |

`time_sync` を true にすると、カメラごとに PC とずれた時計を持たせ、その時計でのセンサー時刻と検出の画像上の
位置を送り、ball_tracker からの時刻合わせの要求に答える。ball_tracker の時刻合わせを、端末なしで確かめるためのもの。
2026-10-10 の録画には `replay_sync.json` (2 台の時計を −1234.5 秒と +987.6 秒ずらし、進み方を ±40 ppm 変えたもの) と
`replay_sync_skew.json` (さらに左のカメラに行ごとの時刻差 12 ms を入れたもの) がある。右のカメラの残りのずれは、
ball_tracker の `--time-trim-ms` に 25 (行ごとの時刻差を入れるときは 31) を渡す。

## calibrate_camera.py

```bash
python scripts/recording_analysis/calibrate_camera.py pose recordings/calibration/<シリアル>/frame_XXXX.png --position 0.257 0.180 1.230 --rotation 140 -2 -31
```

カメラを置くたびに、次の手順で位置と向きを求める。

1. レンズの位置を、筐体の座標 (x: 奥面からの奥行き、y: 左の壁から、z: 高さ。m) でメジャーで測る。
2. カメラアプリのカメラ画面で「Save frame」を押し、`scripts/pull-calibration-frames.sh` で取り込む。
   画像と同じ名前の JSON に、アプリが使っている焦点距離などが入っている。
3. `pose` を実行する。`--position` に測った位置、`--rotation` におおよその向き (アプリの Camera Pose と同じ角度) を渡す。
   前に解いた結果があれば `--previous` で渡せる。
4. 重ね描きの画像 (結果と同じ名前の .jpg) で、色つきの線が筐体の縁に乗っているかを目で確かめる。
5. 表示された「設定画面 (Camera Pose) に入れる値」を、アプリの設定画面に入れる。

位置は測った値に固定し、向きだけを解く。位置まで画像から解くと、筐体の基準 (`booth.json`) が 1cm 違うだけで
位置が 4cm 動く。`--free-position`、`--free-focal`、`--radial` は診断用。

線のおおよその位置は、位置と向きの見込みから自動で探す。2026-10-10 の録画では、手で選んだ線で解いた向きと
0.2 度以内で合った。別の縁を拾うことがあるので、うまく乗らないときは `--pick` で線ごとに 2 点をクリックして選ぶ
(選んだ位置は JSON に残り、次から `--lines` で使える)。筐体の線の位置は `booth.json` にあり、いまは筐体データの値。
実物を測ったら書き換えること。

```bash
python scripts/recording_analysis/calibrate_camera.py check --help
```

`check` は、位置を測った印にボールを当てた状態の画像を 2 台ぶん渡すと、ボールの中心を三角測量して、測った位置との
差を表にする。時計や検出の遅れが関わらないので、位置と向きだけの確認になる。計算で作った画像でしか確かめていない。

## calibrate_intrinsics.py

```bash
python scripts/recording_analysis/calibrate_intrinsics.py recordings/calibration/<シリアル> --board 9x6 --out intrinsics.json
```

チェッカーボード (印刷したものか PC の画面) を「Save frame」で 15 枚ほど、向きと位置を変えて撮り、焦点距離・
光学中心・歪み 1 項を求める。端末と撮影の方式 (30fps 版、120fps 版) ごとに 1 回行い、表示された
「画像の幅に対する比」をアプリの設定画面の Focal length に入れる。端末の画像がまだ無いので、実際の画像では
確かめていない。
