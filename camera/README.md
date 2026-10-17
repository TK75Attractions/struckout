# struckoutCameraApp (高速撮影版)

`struckoutCameraApp` をコピーし、カメラの取り込みを 120fps にしたもの。
うまくいかなかったときに戻れるよう、元の `struckoutCameraApp` には手を入れていない。

## 元のアプリとの違い

- カメラは CameraX の `ImageAnalysis` ではなく、Camera2 の高速撮影セッション
  (constrained high speed) を直接使う。KYV45 は通常のセッションだと 30fps が上限のため
- 高速撮影セッションは YUV の画像を直接受け取れないので、カメラのフレームを動画エンコーダーに入れ、
  すぐにデコードして `SurfaceTexture` に描き、GPU で 640x360 に縮小して `glReadPixels` で読み出す
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
- OpenCV は複製せず `../struckoutCameraApp/opencv` を参照する

## ビルド

`protoc` と JDK が要る (リポジトリのルートで `mise install`)。

```bash
./gradlew :app:assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

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
- 高速撮影時の画角。内部パラメータは「センサーの中央を 16:9 で横いっぱいに切り出す」前提で
  換算しているが、高速撮影ではセンサーの読み出し範囲が狭くなる端末がある
