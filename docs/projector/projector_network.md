# Projector の接続ポート

Projector が接続する ball_tracker と game_master の既定ポートは、[`api/spec/servers.yaml`](../../api/spec/servers.yaml) で管理する。
`projector/Assets/Scripts/Domain/ServerPorts.Generated.cs` はこの定義から生成され、Resolver が既定値に使う。全モジュールでの生成方法は[サーバーポートの定義](../server_ports.md)を参照。

Projector の Inspector 設定でポートに `0` を指定すると spec の既定値を使う。正の値を指定した場合は明示的な上書きとして使う。
コマンドライン引数・環境変数による上書きも引き続き使え、優先順位はコマンドライン、環境変数、Inspector の順。
0 未満または 65535 を超えるポート値は、不正な設定として起動時にエラーになる。

`api/spec/servers.yaml` を変更したら、リポジトリのルートで `mise run //:ports:generate` を実行する。
Unity をビルドする前に `mise run //:ports:check` を実行すると、コミット対象の生成物が spec と一致しているか確認できる。
