# 投擲ゲーム

## ディレクトリ構造
### 各種モジュール
- `ball_tracker`(Rust): カメラからボールの二次元座標を継続的に受け取り、それらを組み合わせて三次元座標を特定し、三次元座標の推移から的のどの部分にボールがあたったか判定し、それを`projector`に送る。
- `game_master` (Rust): 合計得点やランキングなどを管理する。データは現時点ではSQLiteに保存しているが将来的にはMySQLにするかも。今の所タッチパネルのUIとランキング管理が同じプログラムだが、筐体を複数にすることを考えると単一のgame_masterをSSoT(Single Source Of True)にして、touchpanelがそれに接続するという形のほうがいいかも。ゲームのセッション管理もgame_master側にしたい。
- `projector` (C#/Unity): ball_trackerから的にあたった場所を受け取り、`projector`が持っている的の座標と照合し何点入るか計算する。入る点数をgame_masterに通知する。
- `struckoutCameraApp(camera)` (Kotlin): 二次元座標をカメラから取得して`ball_tracker`に送信する。

### その他
- `.cargo`: cargo(Rustでnpmにあたるもの)の設定。
- `.github`: GitHubの設定。
- `api`: protobufの定義やモジュール間で共有するやつを置いておく場所。
- `docs`: メモとかを置いておく場所。
- `migrations`: sqlxで使うデータベースのマイグレーションスクリプトを置いておく場所。
- `mcu`: 後で消す
- `sandbox`: 雑多なプログラムを置いておく場所。
- `stern`: 私が作っているSlintを拡張するGUIフレームワーク。
- `xtask`: package.jsonのscriptsの豪華版みたいなもん。

## 参考リンク
- [ByteTrack](https://github.com/FoundationVision/ByteTrack) ... 2021年に出たMOTアルゴリズム
- [SORT](https://github.com/abewley/sort) ... シンプルなMOTアルゴリズム
- [tracktor](https://github.com/szma/tracktor) ... RustのMTTライブラリ
- [【論文ざっくり紹介】ByteTrack ~単純なアルゴリズムでSOTAを達成(2021年12月時点)~](https://qiita.com/tomo_v/items/f1b9ab396add42c98d3b)
- [現在のトラッキングモデルの基礎ともいえる SORT を解説！](https://deepsquare.jp/2022/06/sort/)
- [エピボーラ幾何](https://qiita.com/Thought_Nibbler/items/9cb7c2637000eecc1a30) ... 3次元空間を異なる位置のカメラから撮影したときの幾何
- [ハンガリー法（ハンガリアンアルゴリズム）を使って割当問題を解く方法を1つずつ丁寧に解説してみた](https://yukashun.com/hungarian_algorithm/) ... 割り当て問題のアルゴリズム
- [高校数学で紐解くカルマンフィルタ](https://rikei-tawamure.com/entry/2025/03/22/192101) ... 用語がよくまとまっている
