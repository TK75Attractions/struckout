## ディレクトリ構造
- `ball_watcher`(Rust): カメラからの情報を集めて処理するサーバ(PCで動く)
- `struckoutCameraApp` (Kotlin): カメラ用のAndroidアプリ。
- `game_master` (Rust): 合計得点やランキングなどを管理する。たぶんESP32で動くがPCにするかも。
- `projector` (Rust/Slint -> C#/Unity): ball_watcherからあたった場所を受け取って点数が増えたらgame_masterに通知する。現在はRust/Slintだがアニメーションが厳しいのでC#/Unityに移行する。
- `protocol`: protobufの定義をおいておく場所。
- `mcu`: 後で消す

## 参考リンク
- [ByteTrack](https://github.com/FoundationVision/ByteTrack) ... 2021年に出たMOTアルゴリズム
- [SORT](https://github.com/abewley/sort) ... シンプルなMOTアルゴリズム
- [tracktor](https://github.com/szma/tracktor) ... RustのMTTライブラリ
- [【論文ざっくり紹介】ByteTrack ~単純なアルゴリズムでSOTAを達成(2021年12月時点)~](https://qiita.com/tomo_v/items/f1b9ab396add42c98d3b)
- [現在のトラッキングモデルの基礎ともいえる SORT を解説！](https://deepsquare.jp/2022/06/sort/)
- [エピボーラ幾何](https://qiita.com/Thought_Nibbler/items/9cb7c2637000eecc1a30) ... 3次元空間を異なる位置のカメラから撮影したときの幾何
- [ハンガリー法（ハンガリアンアルゴリズム）を使って割当問題を解く方法を1つずつ丁寧に解説してみた](https://yukashun.com/hungarian_algorithm/) ... 割り当て問題のアルゴリズム
- [高校数学で紐解くカルマンフィルタ](https://rikei-tawamure.com/entry/2025/03/22/192101) ... 用語がよくまとまっている
