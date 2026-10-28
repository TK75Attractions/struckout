# TargetGenerator の層配置の調査

2026-10-28 時点の実装と Git 履歴を調査した記録。

## 確認できた経緯

- `35ebec6` (`feat: Add TargetGenerator`、2026-07-10) の直前には、Application の
  `ITargetGenerator` と Infrastructure の `FakeTargetGenerator` が存在し、
  `GameLifetimeScope` は後者を登録していた。
- 同コミットが Infrastructure に `TargetGenerator` を新設し、DI の登録先を
  `FakeTargetGenerator` から置き換えた。本実装は当初から配置計算を行っており、
  通信や Unity API に依存していたものを後から取り除いたわけではない。
- `596fda7` (2026-09-06) は命中後の配置選択に乱数を導入した。
  初期配置は決定的に保ち、命中後は空いた同じ位置へ戻ることを避けるための変更だった。
- `49f0e67` (2026-10-25) は登録前の形状 `TargetGeometry` と、サーバーの UUID を
  持つ `Target` を分離した。生成器の層配置は維持された。

確認したコミット本文と関連ドキュメントには、Infrastructure を選んだ固有の理由は
明記されていない。「Application にインターフェース、Infrastructure に実装と Fake」
という構成を引き継いだと考えられるが、これは変更内容からの推測である。

## 移動前の責務と依存関係

- `Infrastructure/TargetGenerator.cs` は `FieldBounds` と既存の `TargetGeometry` から、
  新しい的の位置・大きさや命中後の配置候補を計算する。
- 使用するのは Domain の型、Application の `ITargetGenerator`、標準ライブラリの
  数学・コレクション・乱数であり、通信、ファイル、Unity API は使っていない。
- `Application/GameRuntime.cs` がインターフェース経由で配置を計算し、
  `IGameMasterClient` 経由で登録を要求する。配置計算とサーバーへの登録は別の責務である。
- `Unity/GameLifetimeScope.cs` は NetworkMode の分岐外で本実装を登録する。
  現在は Fake モードでも本実装を使用し、`FakeTargetGenerator` の参照元は見つからなかった。
- `Assets/Tests/EditMode/TargetGeneratorTests.cs` は本実装を直接作成して検証する。

## 設計上の評価

現在の配置は依存方向として成立するが、Infrastructure に置く技術的な必然性は
見当たらない。初期配置の再現性、盤面内への収容、的同士の重なり回避はゲームの
配置ルールであり、責務からは Domain のサービスとして置く方が自然である。
`System.Random` の使用だけで Infrastructure の責務になるわけではない。

移動する場合、Domain から Application の `ITargetGenerator` を参照する形にはしない。
配置方針の差し替えが必要なら契約も Domain に置き、必要がなければ具体型を
Application から使う構成を検討する。乱数だけを差し替える必要性は別に判断する。

## 調査後の変更

`TargetGenerator` を `Domain/Targets/TargetGenerator.cs` に移し、名前空間を
`Struckout.Domain.Targets` に変更した。`ITargetGenerator` と参照元のない
`FakeTargetGenerator` は削除し、`GameRuntime` は具体型を直接受け取る。
DI は `TargetGenerator` 自体を Singleton として登録する。
配置アルゴリズムは変更せず、既存の配置テストも移動後の具体型を検証する。
