## 方向ベクトルの計算方法

### 1. カメラの内部パラメータ(intrinsics)・外部パラメータ(extrinsics)取得

カメラ行列
$
K = \left(
\begin{array}{rr}
f_x & 0 & c_x\\
0 & f_y & c_y\\
0 & 0 & 1
\end{array}
\right)
$

$f_x$、$f_y$, $c_x$, $c_y$. 回転行列$R$は`Camera2`の`CameraCharacteristic`から取得できる。

### 2. 歪みの補正

正規化カメラ座標$
x_c = K^{-1} \left(
\begin{array}{rr}
u\\
v\\
1
\end{array}
\right)
$  
[OpenCVのundistortPoints()](https://docs.opencv.org/4.x/d9/d0c/group__calib3d.html#ga55c716492470bfe86b0ee9bf3a1f0f7e)
が似たようなことをやっていると思う。

### 3. 方向ベクトルの算出

ワールド座標での方向ベクトル$
d_w = R x_c
$

### 4. 解析画像への換算と、較正値を返さない端末

`LENS_INTRINSIC_CALIBRATION` はセンサーの有効画素領域 (active array) の座標系で表される。
一方、検出位置は解析画像 (`ImageAnalysis` が渡す 640x480 程度の画像) のピクセルなので、
カメラ行列は解析画像の解像度に換算してから使う (`CameraIntrinsics.scaledToImage`)。
出力ストリームは active array の中央を縦横比に合わせて切り出したもの、という前提を置いている。

`LENS_INTRINSIC_CALIBRATION` や `LENS_POSE_ROTATION` を返さない端末 (例: KYV45) では次で代用する。

- 内部パラメータ: 焦点距離 (`LENS_INFO_AVAILABLE_FOCAL_LENGTHS`) とセンサー寸法
  (`SENSOR_INFO_PHYSICAL_SIZE`、`SENSOR_INFO_PIXEL_ARRAY_SIZE`) から見積もる。
  光学中心は画像の中央とし、歪みは考えない
- レンズの向き: 端末の真後ろを向いた背面カメラの標準的な向きを `SENSOR_ORIENTATION` から決める

どちらも近似なので、実測の較正値がある端末より精度は落ちる。代用したときは logcat に警告が出る。

### 5. 参考

- [OpenCV: Camera Calibration and 3D Reconstruction](https://docs.opencv.org/4.x/d9/d0c/group__calib3d.html) ：
  公式。最も情報量が多く信頼できる
- [カメラキャリブレーションと3次元再構成 - opencv 2.2 documentation](http://opencv.jp/opencv-2svn/cpp/camera_calibration_and_3d_reconstruction.html)
  ：日本語版。やや古い。
- [カメラキャリブレーション — OpenCV-Python Tutorials 1 documentation](https://labs.eecs.tottori-u.ac.jp/sd/Member/oyamada/OpenCV/html/py_tutorials/py_calib3d/py_calibration/py_calibration.html) ：
  鳥取大学シリーズ。非常にわかりやすい。神。
- [姿勢推定 — OpenCV-Python Tutorials 1 documentation](https://labs.eecs.tottori-u.ac.jp/sd/Member/oyamada/OpenCV/html/py_tutorials/py_calib3d/py_pose/py_pose.html) ：
  鳥取大学シリーズその2。

- [CameraCharacteristics | API reference | Android Developers](https://developer.android.com/reference/android/hardware/camera2/CameraCharacteristics#LENS_POSE_ROTATION) ：
  AndroidのCamera API

