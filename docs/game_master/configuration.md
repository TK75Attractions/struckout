# game_master の設定

ゲームの制限時間は難易度ごとに秒単位で指定する。`normal`、`hard`、`veryhard` はすべて必須で、
それぞれ 1 以上の整数を指定する。設定ファイルがない場合や値が不正な場合は起動時にエラーにする。
開始イベントの `game_dur_secs`、残り時間の通知、ゲーム終了までの時間には、開始時に選択された難易度の値を使う。

設定ファイルはコンテナ内の `/etc/game_master/config.toml` に置く。

```toml
[game_duration_secs]
normal = 120
hard = 90
veryhard = 60
```

CLI 引数で制限時間は指定できない。以前の `game_duration_secs = 90` 形式の設定ファイルは、
上記のテーブル形式に変更する必要がある。

開発用の Docker Compose は `game_master/docker/config.dev.toml` を設定ファイルとしてマウントする。
リリース版をランチャーなしで起動する場合は、同じ形式の設定ファイルを用意し、
Compose の `app` サービスから `/etc/game_master/config.toml` に読み取り専用でマウントする。

ランチャーを使う場合は `<dirs::config_dir()>/struckout/game_master/config.toml` を編集する。
ランチャーは初回インストール時に 3 つの難易度をすべて 90 秒にしたファイルを作成し、
コンテナ内へバインドマウントする。既存の設定は上書きしないため、旧形式のファイルは手で更新する。
変更後はランチャーで game-master を再起動すると、新しい制限時間が適用される。
