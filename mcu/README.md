## ホスト側のセットアップ
### USBの設定(Linux)
```sh
sudo cp ./resources/99-struckout-mcu-esp.rules /etc/udev/rules.d/
sudo udevadm control --reload 
sudo udevadm trigger` 
```

## アルゴリズムの説明
### 座標の計算
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
    
### 座標から放物線を求める
逐次最小二乗法(RLS; Recursive Least Square)を使って推定する。  
Kalman Filterを使う方法もあるが(たぶん)システムが静的なのでRLSの方が適している。

### 放物線と的の交点を求める

