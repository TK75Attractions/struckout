# touchpanel

## アーキテクチャ

### XxxViewModel
XxxViewModelTraitを実装する

### XxxViewModelRc
new()でXxxViewModelを作ってregister_viewmodel()を呼ぶ

### XxxDestination
load(route: &NavRoute)でrouteの引数をXxxViewModel::on_load()に渡す

## 終了

SIGTERM または Ctrl-C を受けると Slint のイベントループを閉じて正常終了する。
ウィンドウを閉じた場合も正常終了する。

## 起動オプション

設定ファイルは `<dirs::config_dir()>/struckout/touchpanel/config.toml` に置く。Linux では通常 `~/.config/struckout/touchpanel/config.toml`。ファイルがなければ CLI 引数だけで起動する。
launcher からインストールすると、このファイルに初期設定が作られ、更新時も保持される。launcher からの起動時には CLI 引数を渡さないので、このファイルで接続先と筐体番号を設定する。

```toml
address = "127.0.0.1"
machine-id = 1
disable-multiplay = true
```

`address` と `machine-id` は設定ファイルか CLI (`--address`、`--machine-id`) のどちらかで必ず指定する。同じ項目を両方で指定した場合は CLI を優先する。`--disable-multiplay` を指定すると、設定ファイルの値にかかわらず無効にする。設定ファイルのキーや値が不正な場合は、理由を表示して起動を止める。

`--disable-multiplay` を指定すると、名前入力後にシングルプレイのセッションを開始し、モード選択画面を経由せず難易度選択画面へ進む。
CLI と設定ファイルのどちらでも指定しなければ、これまで通りモード選択画面を表示する。
