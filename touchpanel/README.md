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
