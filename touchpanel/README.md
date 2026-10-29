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

`--disable-multiplay` を指定すると、名前入力後にシングルプレイのセッションを開始し、モード選択画面を経由せず難易度選択画面へ進む。
省略した場合は、これまで通りモード選択画面を表示する。
