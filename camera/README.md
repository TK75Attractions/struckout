# struckoutCameraApp (高速撮影版)

`struckoutCameraApp` をコピーし、カメラの取り込みを 120fps にしたもの。
うまくいかなかったときに戻れるよう、元の `struckoutCameraApp` には手を入れていない。

## 元のアプリとの違い

- カメラは CameraX の `ImageAnalysis` ではなく、Camera2 の高速撮影セッション
  (constrained high speed) を直接使う。KYV45 は通常のセッションだと 30fps が上限のため
- 高速撮影セッションは YUV の画像を直接受け取れないので、カメラのフレームを動画エンコーダーに入れ、
  すぐにデコードして `SurfaceTexture` に描き、GPU で幅 640 (高さは撮影の縦横比に合わせる) に縮小して `glReadPixels` で読み出す
  (`camera/highspeed/`)。ファイルには書かない
  - 以前は `SurfaceTexture` でカメラのフレームを直接受けていた。KYV45 は高速撮影でもプレビュー用の
    出力先には 30fps しか渡さないため (届いたフレームの間隔が、すべて 1/120 秒のちょうど 4 倍だった。
    `recordings/2026-10-14_kyv45-120fps_2/`)、本来の出力先である動画エンコーダーを通すようにした
  - 代わりに、エンコードとデコードのぶん遅れ (数フレームの見込み、未測定) と端末の負荷が増える
  - デコーダーから出てくるフレームの時刻はマイクロ秒に丸められ、端末によっては別の時計に直されている。
    撮影結果 (`onCaptureCompleted`) のセンサー時刻と突き合わせて、センサー時刻に戻してから送る
- 解析は PC (ball_tracker) に任せられる。検出の出力先が `Network` のとき、エンコーダーが出した H.264 を
  そのまま 5052 番に送り、ball_tracker がデコードしてアプリと同じ方法で解析する (`network/PcVideoStreamer.kt`、
  `ball_tracker/src/video/`)。KYV45 では端末での解析が 1 枚 17 ms ほどかかり、120fps に追いつかないため。
  PC が引き受けているあいだ、端末は解析せず検出も送らない (プレビューと `Save frame` は使える)。
  PC につながらない (ball_tracker が古い、`--no-video-analysis`、5052 番の `adb reverse` が無い) ときは、
  これまでどおり端末で解析する。どちらで解析しているかはカメラ画面の「検出:」の行に出る
- 解析が追いつかないフレームは捨てる。カメラ画面に実測の fps と取りこぼし数を出す
- `ObjectTracker` は作業用の `Mat` を使い回す。検出の手順は変えていない
- アプリ ID は `com.taichi765.struckoutCameraApp.highspeed`、表示名は `Struckout HS`。
  元のアプリと同じ端末に入れられる。設定 (カメラの位置など) は別々に持つ
- OpenCV Android SDK は `camera/opencv/` に置く (git の管理外)。初回は `mise run install-opencv` で導入する

## ビルド

`protoc` と JDK が要る (`camera`ディレクトリで `mise install`)。
送受信ポートの既定値と、spec を変更した後の生成手順は [サーバーポートの定義](../docs/server_ports.md) を参照してください。
Android の Gradle ビルドは、build 時に spec から Kotlin 定数を生成します。

```bash
./gradlew :app:assembleDebug
./gradlew :app:testDebugUnitTest
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

## 高速撮影の大きさ

設定画面の「高速撮影の大きさ」で、従来の自動選択 (既定) と `640x480` を選べる。自動選択は固定 120fps の
16:9・幅 640 以上を優先して最小のサイズを選ぶ (KYV45 は 1280x720)。`640x480` は固定 120fps に対応するときだけ
使い、非対応なら従来の自動選択に戻して logcat に出す。設定は保存され、変更するとカメラと PC 向け動画送信を開き直す。

PC からは `scripts/cameras.json` の端末ごとに任意で `"stream_size": "640x480"` を書き、
`python scripts/camera_remote.py config` で反映する。`"1280x720"` は従来の自動選択に戻す値で、サイズの強制指定ではない。
キーを省略すると保存済みの設定を変えない。`status` の `stream_size` は希望する設定で、実際のサイズはカメラ画面か logcat で確認する。

640x480 撮影では解析画像も 640x480 になる。内部パラメータと検出範囲は実際の画像寸法から計算する。
4:3 センサー全体を横いっぱいに使い、1280x720 と横の画角が同じであると仮定しているため、
使用前に両方の設定で **Save frame の画像の横の画角を比較する**。同じなら焦点距離/解析画像幅の比を引き継げる。
画角が違うときは同じ比を使わず、640x480 用に較正し直す。実機で確認するまでは 640x480 を既定にしない。

## 実機で確かめること

- カメラ画面の「撮影」「エンコード」「受け取り」がどれも 120fps 前後であること。「撮影」だけが
  120fps なら、エンコーダーかデコーダーが追いついていない。「受け取り」に比べて「解析」が少ないときは、
  解析 (1 枚 8.3ms 以内が目安) が追いついておらず、「取りこぼし」が増える
- logcat の `CaptureTimeline` の `encoded frame timestamps are corrected by ...ns` が 1 回出ること。
  出ないときは、フレームの時刻をセンサー時刻に戻せていない (ball_tracker の時刻合わせが狂う)
- PC で解析するとき: カメラ画面が `検出: PC で解析 (動画を送信中)` になること (logcat の `PcVideoStreamer` に
  `PC accepted the video` が出る)。ball_tracker の `video analysis` の `fps` が「エンコード」と同じくらいであること。
  `--video-snapshot-dir` で保存した画像と `Save frame` の画像の向きが同じであること
- 撮影から ball_tracker に届くまでの遅れ (ball_tracker のログの `camera clock` の `min_latency_ms`)。
  エンコードとデコードを入れる前の 60〜84ms からどれだけ増えたか
- プレビューの向きが元のアプリと同じであること。フレームはセンサーの向きのまま読み出す前提で
  書いており、上下や左右が反転していると方向ベクトルが狂う
- 高速撮影時の画角。内部パラメータは「センサーの中央を撮影の縦横比で横いっぱいに切り出す」前提で
  換算しているが、高速撮影ではセンサーの読み出し範囲が狭くなる端末がある
