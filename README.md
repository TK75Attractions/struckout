## ディレクトリ構造
- `ball_watcher`(Rust): カメラからの情報を集めて処理するサーバ(PCで動く)
- `struckoutCameraApp` (Kotlin): カメラ用のAndroidアプリ。
- `game_master` (Rust): 合計得点やランキングなどを管理する。たぶんESP32で動くがPCにするかも。
- `projector` (Rust/Slint -> C#/Unity): ball_watcherからあたった場所を受け取って点数が増えたらgame_masterに通知する。現在はRust/Slintだがアニメーションが厳しいのでC#/Unityに移行する。
- `protocol`: protobufの定義をおいておく場所。
- `mcu`: 後で消す
