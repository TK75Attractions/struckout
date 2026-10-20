# 投擲ゲーム

## 目次
- [ディレクトリ構造](#ディレクトリ構造)
- [環境構築](#環境構築)
- [参考リンク](#参考リンク)

## ディレクトリ構造
[docs/architecture.png](./docs/architecture.png)も参照されたし。
### 各種モジュール
- `ball-tracker`(Rust): カメラからボールの二次元座標を継続的に受け取り、それらを組み合わせて三次元座標を特定し、三次元座標の推移から的のどの部分にボールがあたったか判定し、それを`projector`に送る。
- `game-master` (Rust): 合計得点やランキングなどを管理する。MySQLをデータベースとして使っている。
- `touchpanel` (Rust/Slint): タッチパネルのUI。game-masterにゲームスタートのリクエストをしたりprojectorがgame-masterに送った点数を取得したり。
- `projector` (C#/Unity): ball_trackerから的にあたった場所を受け取り、`projector`が持っている的の座標と照合し何点入るか計算する。入る点数をgame_masterに通知する。
- `camera` (Kotlin): 二次元座標をカメラから取得して`ball_tracker`に送信する。
- `launcher` (C#/Avalonia): 上のモジュールを GitHub Releases などからインストールし、依存の順に起動する。使い方は [launcher/README.md](./launcher/README.md)。

### その他
- `.cargo`: cargo(Rustでnpmにあたるもの)の設定。
- `.mise`: miseのmonorepo設定と共通タスク。各モジュールのツール・タスクは各ディレクトリの`mise.toml`で管理している。
- `.github`: GitHubの設定。
- `api`: protobufの定義やモジュール間で共有するやつを置いておく場所。
- `docs`: メモとかを置いておく場所。
- `migrations`: sqlxで使うデータベースのマイグレーションスクリプトを置いておく場所。
- `sandbox`: 雑多なプログラムを置いておく場所。
- `xtask`: package.jsonのscriptsの豪華版みたいなもん。複雑なスクリプトはPowerShellとかbash、PythonではなくRustで書いてここに置いてほしい。

## 環境構築
1. miseをインストールする
https://mise.jdx.dev/getting-started.html

1. mise 2026.10.3以降を使い、ルートで設定を信頼する
```shell
mise trust

# bashの場合
eval "$(mise activate bash)"
# PowerShellの場合
(&mise activate pwsh) | Out-String | Invoke-Expression
```

1. 開発するモジュールに移動して必要なツールをインストールする
```shell
cd camera
mise install
mise run install-opencv
```

`mise install`は現在のモジュールと親の設定だけを読み込む。他のモジュールの
sqlx-cliやSlintなどはインストールしない。共通の.NET SDKとgrpcurlは全モジュールで継承する。
ツール・タスクの配置とローカル設定は[開発環境](docs/development_environment.md)を参照。

ルートや別モジュールからはパスでタスクを指定できる。
```shell
mise run //camera:install-opencv
mise run //game_master:dev
mise run //touchpanel:fmt-slint
mise run //api:proto
mise run //api:proto:check
mise tasks ls --all
```

1. 手動でインストールするもの
- Docker
`game-master`の起動、開発に使う。
- [Android Studio](https://developer.android.com/studio?hl=ja)
`camera`の開発・Androidデバイスとの接続に使う。Visual StudioのAndroid版のようなものと思って良い。
- [Unity]
`projector`の開発に使う。

## 参考リンク
- [ByteTrack](https://github.com/FoundationVision/ByteTrack) ... 2021年に出たMOTアルゴリズム
- [SORT](https://github.com/abewley/sort) ... シンプルなMOTアルゴリズム
- [tracktor](https://github.com/szma/tracktor) ... RustのMTTライブラリ
- [【論文ざっくり紹介】ByteTrack ~単純なアルゴリズムでSOTAを達成(2021年12月時点)~](https://qiita.com/tomo_v/items/f1b9ab396add42c98d3b)
- [現在のトラッキングモデルの基礎ともいえる SORT を解説！](https://deepsquare.jp/2022/06/sort/)
- [エピボーラ幾何](https://qiita.com/Thought_Nibbler/items/9cb7c2637000eecc1a30) ... 3次元空間を異なる位置のカメラから撮影したときの幾何
- [ハンガリー法（ハンガリアンアルゴリズム）を使って割当問題を解く方法を1つずつ丁寧に解説してみた](https://yukashun.com/hungarian_algorithm/) ... 割り当て問題のアルゴリズム
- [高校数学で紐解くカルマンフィルタ](https://rikei-tawamure.com/entry/2025/03/22/192101) ... 用語がよくまとまっている
