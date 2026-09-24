## アルゴリズムの説明
### 同時刻の別カメラからのフレームを探す
単純にtimestamp(UNIX時間)の差が一定以下のものを探す。

### それぞれのカメラフレームで対応するオブジェクトを探す(割り当て問題)
Kalman Filterの事前推定(prior estimation)で出した3D座標とレイの
-> 三角測量で座標の観測値を出す
-> Kalman Filterを更新

### カメラの位置・姿勢
各カメラの設定画面で、共有する世界座標系における位置 `(x, y, z)` と姿勢を設定する。
姿勢は度数法の roll（X軸）、pitch（Y軸）、yaw（Z軸）で、端末センサー座標から
世界座標への回転として `Rz(yaw) * Ry(pitch) * Rx(roll)` の順で適用される。

`WorldDirectionCalculator` は、画像座標をカメラ座標へ戻した後、
`LENS_POSE_ROTATION` の逆回転によって端末センサー座標のレイを求める。
`ball_tracker` は受信したレイに上記の設置姿勢を適用してから三角測量する。
角度がすべて `0` の場合は、端末センサー座標の軸が世界座標の軸と一致しているものとして扱う。

### 三角測量的なやつ
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
