# game_master の設定

ゲームの制限時間は難易度ごとに秒単位で指定する。`normal`、`hard`、`veryhard` はすべて必須で、
それぞれ 1 以上の整数を指定する。コンボ到達時のボーナスも `combo_bonuses` で必ず指定する。
`game_start_delay_secs` は準備完了から `GameStarted` までの秒数で、1 以上の整数を必ず指定する。
設定ファイルがない場合、必須項目がない場合、値が不正な場合は起動時にエラーにする。
開始イベントの `game_dur_secs`、残り時間の通知、ゲーム終了までの時間には、開始時に選択された難易度の値を使う。

設定ファイルはコンテナ内の `/etc/game_master/config.toml` に置く。

```toml
game_start_delay_secs = 3

[game_duration_secs]
normal = 120
hard = 90
veryhard = 60

[[combo_bonuses]]
combo = 3
points = 500

[[combo_bonuses]]
combo = 5
points = 800
```

シングルプレイは `SetSessionReady` の受付時、マルチプレイは両マシンの準備がそろった時に
`GameWillStart.secs_until_start` を設定秒数から 1 秒ごとに 1 まで送る。
設定秒数後に `GameStarted` を送り、ゲームの制限時間を計り始める。

`combo` と `points` はともに 1 以上の整数で、同じ `combo` は重複できない。
指定したコンボ数に到達したときだけ、その `points` を命中得点に加算する。
リセット後に再び到達した場合も加算する。ボーナスを無効にする場合も
`combo_bonuses = []` を明示する。設定を変更したら game-master を再起動する。
既存の設定ファイルに `combo_bonuses` がない場合は、追記するまで起動できない。

CLI 引数で制限時間は指定できない。以前の `game_duration_secs = 90` 形式の設定ファイルは、
上記のテーブル形式に変更する必要がある。

開発用の Docker Compose は `game_master/docker/config.dev.toml` を設定ファイルとしてマウントする。
リリース版をランチャーなしで起動する場合は、同じ形式の設定ファイルを用意し、
Compose の `app` サービスから `/etc/game_master/config.toml` に読み取り専用でマウントする。

ランチャーを使う場合は `<dirs::config_dir()>/struckout/game_master/config.toml` を編集する。
ランチャーは初回インストール時に開始待ち時間を 3 秒、3 つの難易度をすべて 90 秒、コンボボーナスを
3 コンボで 500 点と 5 コンボで 800 点にしたファイルを作成し、
コンテナ内へバインドマウントする。既存の設定は上書きしないため、`game_start_delay_secs` がないファイルや旧形式のファイルは手で更新する。
変更後はランチャーで game-master を再起動すると、新しい制限時間が適用される。
