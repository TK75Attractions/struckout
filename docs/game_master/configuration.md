# game_master の設定

ゲームの制限時間は秒単位で指定する。設定ファイルか CLI 引数で必ず指定する。0 秒は指定できない。
開始イベントの `game_dur_secs` と残り時間の通知、ゲーム終了までの時間に同じ値を使う。

設定ファイルは `/etc/game_master/config.toml` に置く。
内容は次のとおり。

```toml
game_duration_secs = 120
```

起動時に `--game-duration-secs 120` でも指定できる。両方で指定した場合は CLI 引数を優先する。
どちらにも値がない場合、設定ファイルを読めない場合、値が不正な場合は起動時にエラーにする。
Docker で起動する場合、設定ファイルをコンテナ内の `/etc/game_master/config.toml` にマウントするか、
Compose の `app` サービスに `command: ["--game-duration-secs", "120"]` を指定する。

ランチャーを使う場合は `<dirs::config_dir()>/struckout/game_master/config.toml` を編集する。
ランチャーは初回インストール時に `game_duration_secs = 90` で作成し、コンテナ内へバインドマウントする。
変更後はランチャーで game-master を再起動すると、新しい制限時間が適用される。
