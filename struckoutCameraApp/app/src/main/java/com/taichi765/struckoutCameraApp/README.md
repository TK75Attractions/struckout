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

### 4. 参考

- [OpenCV: Camera Calibration and 3D Reconstruction](https://docs.opencv.org/4.x/d9/d0c/group__calib3d.html) ：
  公式。最も情報量が多く信頼できる
- [カメラキャリブレーションと3次元再構成 - opencv 2.2 documentation](http://opencv.jp/opencv-2svn/cpp/camera_calibration_and_3d_reconstruction.html)
  ：日本語版。やや古い。
- [カメラキャリブレーション — OpenCV-Python Tutorials 1 documentation](https://labs.eecs.tottori-u.ac.jp/sd/Member/oyamada/OpenCV/html/py_tutorials/py_calib3d/py_calibration/py_calibration.html) ：
  鳥取大学シリーズ。非常にわかりやすい。神。
- [姿勢推定 — OpenCV-Python Tutorials 1 documentation](https://labs.eecs.tottori-u.ac.jp/sd/Member/oyamada/OpenCV/html/py_tutorials/py_calib3d/py_pose/py_pose.html) ：
  鳥取大学シリーズその2。

