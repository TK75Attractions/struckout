# ランキングとプレイヤー別履歴

`GameMasterSessionService` の取得 RPC は、MySQL に保存された終了済みゲームを返す。
実行中のセッションや、問い合わせ元のマシンには依存しない。

両 RPC とも `mode` と `difficulty` は必須で、指定された組み合わせのゲームだけを返す。
`UNSPECIFIED` または未定義の enum 値を指定すると `InvalidArgument` を返す。
得点の降順、同点なら `game_id` の昇順に並べる。

## GetRanking

`max_item_num` 件までのゲームを返す。0 を指定すると空の一覧になる。
各行はゲーム ID、得点、`PlayerInfo` を持ち、multi-play も1ゲームにつき1行になる。
`PlayerInfo` は single-play では1人、multi-play では両方のプレイヤーを含む。
各プレイヤーは、そのゲームで使用したマシン ID、プレイヤー ID、名前を持つ。
同じプレイヤーが両方のマシンで参加した場合も、両マシンの情報を保持する。

## GetGamesByPlayer

指定したプレイヤーが参加した、指定モード・難易度のすべての保存済みゲームについて、
ゲーム ID、得点、`PlayerInfo`、全体順位 `rank` を返す。`PlayerInfo` はランキングと同様に、
single-play では1人、multi-play では両方のプレイヤーのマシン ID、プレイヤー ID、名前を含む。
multi-play ではどちらのプレイヤーの ID で問い合わせても
同じゲームが含まれる。同じプレイヤーが両方のマシンで参加した場合も、ゲームは1件だけ返す。
順位は同じモード・難易度の全ゲームを対象とし、同点は同順位（1、2、2、4）とする。
未登録のプレイヤーや履歴のないプレイヤーには空の一覧を返す。

どちらの RPC も、DB の取得に失敗した場合は gRPC の `Internal` を返す。

## DB の初期化

初期スキーマは `migrations/mysql/0001_init.sql` に統合している。
`machine_id_1` と `player_id_1`、`machine_id_2` と `player_id_2` がそれぞれ対応し、
single-play では2人目の両 ID が NULL、multi-play では両方が必須となる。
終了済みゲームだけを保存し、`score` は必須、`status` カラムは持たない。

旧スキーマからの移行には対応しない。旧スキーマを適用済みの DB は作り直す必要がある。
SQLx のオフラインメタデータは、新しい初期スキーマを適用済みの DB に対して
`mise run //game_master:prepare` で更新する。
