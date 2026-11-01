# サーバーポートの定義

サーバーが待ち受けるポートとクライアントが接続する既定ポートは、`api/spec/servers.yaml` にまとめている。同じ値を複数のコードや設定に手で書かず、このファイルから読み込むか生成する。

Rust の `build.rs` は `struckout_utils::yaml::get_port_def` で値を読み、`env!` 用の環境変数を設定する。共通読み込み処理は `cargo:rerun-if-changed` も出すため、spec を変えたときに build script が再実行される。

launcher は `servers.yaml` をビルド時にアセンブリへ埋め込み、実行時に読み込む。Android は Gradle build 時に generator を呼び、ビルドディレクトリ内に Kotlin の `ServerPorts` を生成する。Projector は generator が出力する `ServerPorts.Generated.cs` を使い、既定値を持つシーンの port `0` は spec の値へ解決する。

game_master の Compose 公開ポートは、`compose.yaml` の `# BEGIN GENERATED SERVER PORTS` と `# END GENERATED SERVER PORTS` の間を generator が更新する。この範囲の外にある設定は通常の Compose 設定として管理する。

Python の launcher・解析スクリプトは `struckout_common.server_port` を通じて spec を読む。`adb-reverse.sh` は同じ generator の `--ports` を使い、複数ポートをまとめて取得する。たとえば次のコマンドは既定で `6060 5050 5052` を表示する。

```bash
python scripts/generate_server_ports.py --ports ball-tracker camera-ctrl camera-data camera-video
```

`servers.yaml` のポートを変えた後は、ルートで `mise run //:ports:generate` を実行する。これにより Projector の C# 定数と Compose の公開ポートを更新する。Android の Kotlin 定数は次回 Gradle build 時に生成される。生成済みファイルが spec と一致するかは `mise run //:ports:check` で確かめる。Android と Unity の CI でもこの生成物を確認する。
