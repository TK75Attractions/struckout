# touchpanel

## 表示先ディスプレイ

`config.toml` の `monitor = <N>` か `--monitor <N>` で、受付画面を全画面表示するディスプレイを選べる (両方あれば CLI を優先する)。
省略すると、従来どおりメインのディスプレイに表示する。

```bash
touchpanel --address localhost --machine-id 1 --monitor 2
```

番号は Slint の winit バックエンドの `available_monitors()` の列挙順で **1 から**数える。
Unity の `-monitor` (projector) や Windows の「識別」の番号と一致するとは限らず、接続構成が変わると列挙順も変わりうる。
指定すると、起動ログに各番号のディスプレイ名と位置を出す。実際の表示先も確認して設定する。
バックエンドは Slint の既定の選び方 (`SLINT_BACKEND` など) に任せる。表示先を選べるのは winit バックエンドだけなので、
別のバックエンドで動いているときに表示先を指定すると、メインのディスプレイに出さずに異常終了する。
0 は引数か設定ファイルのエラーになる。存在しない番号は「ディスプレイ N がありません。M 台見つかりました」と出して異常終了する。

ランチャーから起動するときも、touchpanel の `config.toml` (後述) に `monitor = 2` のように書く。
表示先の選択と Windows のタッチ入力の割り当ては別の設定なので、次も行う。

### メインのディスプレイを決める

Windows の「設定」→「システム」→「ディスプレイ」で表示を「拡張」にする。
「識別」で各画面を確認し、受付用のタッチディスプレイを選んで「これをメイン ディスプレイにする」を有効にする。
touchpanel の表示先を省略したときも受付画面がそこに出るので、この構成を基本にする。
2026-10-26 は、タッチディスプレイをメイン、projector の monitor を 2 にして運用した。

### タッチ入力を割り当てる

表示先とタッチ入力の割り当ては別に設定する。画面の表示先だけ変えても、タッチが別の画面へ送られる場合がある。

1. コントロール パネルの「タブレット PC 設定」を開く。Windows の検索から「ペン入力またはタッチ入力に合わせた画面の調整」を開いてもよい。
2. 「ディスプレイ」タブの「セットアップ」で「タッチ入力」を選ぶ。
3. タッチディスプレイ以外に案内が出たら Enter で次の画面へ進め、タッチディスプレイに案内が出たらその画面をタッチする。
4. 入力位置にずれが残る場合は、対象ディスプレイを選んで「調整」を行う。

「セットアップ」で入力先を関連付け、「調整」で位置を補正する。
参考: [ViewSonic の拡張ディスプレイへのタッチ入力設定](https://viewsonicvsa.freshdesk.com/support/solutions/articles/43000470147-how-can-i-setup-touch-on-extended-display-)。

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
simple-mode = false
monitor = 2
```

`address` と `machine-id` は設定ファイルか CLI (`--address`、`--machine-id`) のどちらかで必ず指定する。同じ項目を両方で指定した場合は CLI を優先する。`--disable-multiplay` を指定すると、設定ファイルの値にかかわらず無効にする。設定ファイルのキーや値が不正な場合は、理由を表示して起動を止める。

`--disable-multiplay` を指定すると、名前入力後にシングルプレイのセッションを開始し、モード選択画面を経由せず難易度選択画面へ進む。
CLI と設定ファイルのどちらでも指定しなければ、これまで通りモード選択画面を表示する。

`simple-mode = true` を指定すると、起動時とゲーム終了後のホーム復帰時にプレイヤー
`NoName403` を検索し、いなければ登録する。そのプレイヤーでシングルプレイのセッションを開始してから
難易度選択画面を表示する。StartScreen と名前入力画面は表示しない。
