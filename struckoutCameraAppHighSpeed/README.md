# struckoutCameraApp (高速撮影版)

`struckoutCameraApp` をコピーし、カメラの取り込みを 120fps にしたもの。
うまくいかなかったときに戻れるよう、元の `struckoutCameraApp` には手を入れていない。

## 元のアプリとの違い

- カメラは CameraX の `ImageAnalysis` ではなく、Camera2 の高速撮影セッション
  (constrained high speed) を直接使う。KYV45 は通常のセッションだと 30fps が上限のため
- 高速撮影セッションは YUV の画像を直接受け取れないので、`SurfaceTexture` で受けて GPU で
  640x360 に縮小し、`glReadPixels` で読み出す (`camera/highspeed/`)
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

- カメラ画面の「カメラ」が 120fps 前後、「取りこぼし」が 0 付近であること
- プレビューの向きが元のアプリと同じであること。フレームはセンサーの向きのまま読み出す前提で
  書いており、上下や左右が反転していると方向ベクトルが狂う
- 高速撮影時の画角。内部パラメータは「センサーの中央を 16:9 で横いっぱいに切り出す」前提で
  換算しているが、高速撮影ではセンサーの読み出し範囲が狭くなる端末がある
