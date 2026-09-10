# projector の描画系 — 現状と引き継ぎ

描画の強化に着手する前に、いま何がどうなっているかを調べた記録。
2026-09-10 に調査し、**同日 uGUI からスプライトへの移行を行ったので、
その結果を反映してある。**

---

## 要点

**描画にあたるものは、まだほとんど無い。**

- アセットは Prefab が 1 つだけ。マテリアル・スプライト・テクスチャ・アニメーション・
  シェーダは**リポジトリに 1 つも無い**
- 的とマーカーの絵は実行時に生成した仮のもの（`PlaceholderSprites`）
- **URP 2D は描画経路に入った。** 的はカメラを通して描かれるので、
  2D ライト・シェーダ・パーティクル・ポストプロセスがそのまま使える

強化フェーズは「既存の作り込みを壊さないように直す」ではなく
**ほぼ更地に作る**話になる。制約は少ない。

---

## 描画経路

```
GameRuntime ──> IUIService ──> UIService ──> TargetField (ワールド空間) の子として
                                             SpriteRenderer を実行時に生成
                                                    │
                                             Main Camera が描画
                                             (URP 2D / Renderer2D)
```

座標は **盤面ピクセル (0〜1920, 0〜1080)** のまま `IUIService` まで流れ、
`UIService` と `CircleTargetUI` がワールド座標に直す。
`GameRuntime` から上と `TargetGenerator` は座標系を意識しない。

```
物理 (m) --CollisionCoordinateTransform--> 盤面 (px) --WorldCoordinateTransform--> ワールド (unit)
```

### 以前との違い

移行前は Canvas が **Screen Space - Overlay** で、カメラも Global Light 2D も
描画経路に関与していなかった。Overlay の Canvas はカメラを介さず最前面に
描かれるため、2D ライトもポストプロセスも的には一切かからず、
**URP 2D が完全に遊んでいた。**

移行の判断と、そのとき比較した選択肢は「設計判断」の節に残してある。

### Canvas は残してある

`Canvas`（Screen Space - Overlay）はシーンに残っているが、いまは**空**。
得点表示・ゲーム開始/終了の画面といった HUD はこちらに置く。
盤面はワールド空間、HUD は Overlay、と役割で分けている。

---

## シーンの構成

`Assets/Scenes/SampleScene.unity`。GameObject は 8 つ。

| GameObject | 中身 |
|---|---|
| `Main Camera` | 直交投影。`FieldCamera` が盤面に合わせて表示範囲を決める |
| `Global Light 2D` | URP 2D のグローバルライト。**的に効くようになった** |
| `TargetField` | 的とマーカーの親。ワールド原点に置いた素の Transform |
| `Canvas` | Screen Space - Overlay。いまは空で、HUD 用に残してある |
| `UIService` | 的とマーカーの生成を担う。`IUIService` の実装 |
| `MainThreadDispatcher` | 受信スレッドから UI スレッドへ戻すため |
| `GameLifetimeScope` | VContainer の DI 定義。設定値もここに載る |
| `EventSystem` | uGUI の既定 |

シーン名が `SampleScene` のまま。改名は履歴が追いにくくなるので、
やるなら描画作業に入る前が良い。

---

## アセットの中身

### `Assets/Prefab/CircleTarget.prefab`（唯一の Prefab）

```
CircleTarget          Transform
                      SpriteRenderer
                        material: Sprite-Lit-Default (URP 2D)
                        sprite:   未設定 → 実行時に PlaceholderSprites.Circle が入る
                        color:    白, alpha 0.392
                      CircleTargetUI
```

- **絵を差し替える起点はここ。** SpriteRenderer に Sprite を割り当てれば、
  仮の生成は行われなくなる
- マテリアルが `Sprite-Lit-Default` なので、**2D ライトがそのまま当たる**。
  ライトを使いたくないなら `Sprite-Unlit-Default` に変える
- 大きさは `CircleTargetUI` が**スプライトの実寸 (bounds) を見てから**倍率を出す。
  どんな pixelsPerUnit の絵を割り当てても、見た目と当たり判定は一致する

### 仮のスプライト（`PlaceholderSprites`）

リポジトリに画像を 1 枚も置かずに動かすため、実行時に生成している。
符号付き距離から 1px 幅でアンチエイリアスした円と菱形。
的は直径 500px 級で表示されるので、粗い絵だと縁が目立つ。

絵ができたら Prefab 側に割り当てるだけでよく、このクラスは呼ばれなくなる。

### 着弾マーカー

Prefab が存在しない。`UIService._collisionMarkerUI` が未設定のため、
`UIService.CreateDefaultMarker` が実行時に `SpriteRenderer` を組み立てている。
的が円なので区別できるよう菱形。

`UIService` の Collision Marker UI に Prefab を割り当てれば、
コード側の生成は使われなくなる（見た目も寿命もその Prefab の責任になる）。
**演出を作る場合の最初の差し込み口はここ。** スプライトになったので、
パーティクルや 2D ライトを持たせられる。

---

## 表示の設定

| 項目 | 値 | 効いてくる場面 |
|---|---|---|
| 盤面 (`FieldBounds`) | 1920 x 1080 px | 的の配置と描画のスケールが共有する |
| ピクセル密度 | 100 px / world unit | カメラの表示範囲がここから決まる |
| カメラ | 直交。`orthographicSize` は実行時に算出 | 手で設定しない |
| 既定解像度 | 1920 x 1080 | |
| フルスクリーン | Fullscreen Window（ボーダーレス） | |
| ウィンドウリサイズ | 不可 | |
| Canvas Scaler | Scale With Screen Size、参照 1920x1080 | **HUD にのみ効く** |

### アスペクト比の扱い

**盤面の比率と投影面の比率が違っても、盤面全体が必ず映る。**

`FieldCamera` が `WorldCoordinateTransform.OrthographicSizeFor(aspect)` を
毎フレーム当てている。投影面が盤面より横長なら左右に、縦長なら上下に余白が出る。

移行前は CanvasScaler が**幅基準**で、`TargetGenerator` は Y を 0〜1080 に
固定して置いていたため、**16:9 からずれると的が画面外に出ていた。**
その問題は無くなったが、実機の比率が 16:9 でないなら余白の扱い
（背景をどう見せるか）は別途決める必要がある。

`orthographicSize` を手で設定してはいけない。盤面の広さとピクセル密度から
一意に決まる値で、両方を手で持つと必ずいつかずれる。

---

## 描画に関わるコードの地図

どこを触ると何が変わるか。

| ファイル | 役割 | 強化時に触る可能性 |
|---|---|---|
| `Unity/UIService.cs` | 的の生成、移動の委譲、マーカーの生成 | **高**。演出の起点 |
| `Unity/CircleTargetUI.cs` | 的 1 個の見た目。位置・大きさ・移動補間 | **高** |
| `Unity/PlaceholderSprites.cs` | 仮の絵の生成 | 中。素材ができたら不要になる |
| `Unity/ITargetUI.cs` | 的 UI の契約（`Initialize` / `MoveTo`） | 中。種類を増やすなら |
| `Unity/CollisionMarker.cs` | マーカーの拡大とフェード | 中。Prefab に置き換えるなら不要になる |
| `Unity/FieldCamera.cs` | 盤面に合わせてカメラの表示範囲を決める | 低 |
| `Unity/UIRoot.cs` | 的を置く親 Transform を保持 | 低 |
| `Domain/WorldCoordinateTransform.cs` | 盤面 px → ワールド unit | 低。ただし全部の前提 |
| `Domain/FieldBounds.cs` | 盤面の広さ | 低。比率を変えるなら |
| `Infrastructure/TargetGenerator.cs` | 的の**位置と大きさ**を決める | 中。見た目の前提を決めている |
| `Domain/TargetDefinition/Target.cs` | 的の状態。`Size` は直径 | 低 |
| `Application/GameRuntime.cs` | 当たり判定と得点。UI を呼ぶ側 | 低 |

### 押さえておくべき約束事

- **`Target.Size` は直径**。`CircleTargetUI` がスプライトの実寸から倍率を出し、
  `CollisionSolver` が `RadiusSquared` で判定するので、この関係が崩れると
  **見た目と当たり判定がずれる**。`TargetTests` が固定している
- **盤面の広さは `FieldBounds` が一箇所で持つ**。`TargetGenerator`（的を置く範囲）と
  `WorldCoordinateTransform`（描画のスケール）が同じインスタンスを見ている。
  片方だけ変えると、比率を変えたときに的だけ元の範囲に置かれる
- **`Target` は参照で等価**。`UIService` が `Dictionary<Target, Transform>` で
  対応づけているため、座標を等価性に含めると移動のたびに対応が壊れる
- **的は消えない**。当たったら同じ GameObject が移動する
  （`docs/projector_behavior.md`）。当たり判定は移動先で即座に有効になり、
  見た目が追いつくまでの短い間だけ表示位置とずれる
- **着弾マーカーは当たり外れに関わらず出す**。座標変換がずれているのか
  単に外したのかを区別するため

---

## 設計判断

### 1. 描画経路 — **決着済み（スプライトへ移行）**

2026-09-10 に uGUI（Screen Space - Overlay）からスプライトへ移した。
そのとき比較した選択肢と代償を、判断の根拠として残しておく。

| 選択肢 | 得られるもの | 塞がれるもの | コード改変 |
|---|---|---|---|
| Overlay 維持 | 独自スプライト、Shader Graph(Canvas)、Animator | パーティクル、2D ライト、ポストプロセス | 0 |
| Screen Space - Camera | 上＋カメラのポストプロセス | パーティクル、2D ライト | シーンの 3 値のみ |
| World Space Canvas | Screen Space - Camera と同じ | 同上 | 中 |
| **スプライト（採用）** | **URP 2D 全部** | — | 約 285 行 |

**World Space Canvas でも 2D ライトは uGUI の `Image` には当たらない。**
障壁は Canvas の RenderMode ではなく Graphic のシェーダ（`UI/Default`）で、
`Light2D` は Sprite-Lit の経路でしか効かない。そのため World Space は
Screen Space - Camera の上位互換にならず、選択肢として意味を失っていた。

**Overlay が本当に塞いでいたのは 3 つ。** パーティクル・2D ライト・
カメラのポストプロセス。`Image` はマテリアルを受けるので Shader Graph
（Canvas ターゲット）は Overlay でも動く。決め手は**パーティクル**で、
`ParticleSystem` は `Graphic` ではないため Overlay Canvas 内に描画できない。
ボールを壁に投げるゲームで着弾のバーストが作れないのは致命的だった。

移行コストが安いのは今だけ、という判断もある。移行対象は
`UIService` / `CircleTargetUI` / `UIRoot` の約 285 行のみで、移行する素材は 0。
`IUIService` / `ITargetUI` / `GameRuntime` / `CollisionSolver` /
`TargetGenerator` と既存テストは無傷だった。

### 2. 的の見た目をどう作るか — 未決

いまは仮の単色スプライト。素材を作るのか、シェーダで描くのか。
`TargetType` は `Circle` 1 種類しか無いので、形を増やすならここから。

的は直径 500px 級で表示される（`TargetGenerator` が空きスペースから
大きさを決めるため、1 個目は直径 500 固定）。ラスタ画像よりシェーダや
ベクタ寄りの絵のほうが向く。

### 3. 難易度で見た目を変えるか — 未決

`docs/projector_todo.md` の **G**。的の数・大きさ・配置パターンが候補。
`GameSettings` は `InitialTargetCount` しか持っていない。

### 4. ゲーム終了時の画面 — 未決

`docs/projector_todo.md` の **H**。`GamePhase.Finished` は実装済みで
状態は取れるが、**画面上は何も起きない**。Canvas を残してあるのはこのため。

---

## 既知の不整合

移行時に機能に影響するものは直した。残りは実害が無い。

| # | 内容 | 状態 |
|---|---|---|
| 1 | `MasterPort` がシーン上で `5001` だった | **修正済み**（8020） |
| 2 | `UIService._coolingDownColor` が残っていた | **修正済み**（`_ignoredColor`） |
| 3 | `_gameSettings.TargetCooldownSeconds` が残っていた | **削除済み** |
| 4 | `_scoreSettings` がシーンに無い | 未対応。コード既定（仮の値）が使われる。編集するには一度 Unity で保存が必要 |
| 5 | `_networkSettings.MachineId` がシーンに無い | 未対応。既定の 1 が使われる |
| 6 | `Assets/_Recovery/0.unity` が git 追跡下にある | 未対応。Unity のクラッシュ復旧用の生成物で、追跡から外すべき |
| 7 | Fake モードでゲームが始まらなかった | **修正済み**（下記） |
| 8 | 本物の game_master でも開始を取りこぼしうる | **未対応。要判断**（下記） |

### 7. Fake モードでゲームが始まらなかった

`FakeGameMasterClient.ConnectAsync` が `return` の前に `GameStarted` を
同期的に発火していた。呼ぶ側は `NetworkBootstrap` が `ConnectAsync` を
await し、**そのあとで** `GameBootstrap` が購読するため、開始が誰も居ない
ところへ投げられて消えていた。結果、Fake モードでは的が一つも出ず、
「対向なしで描画調整が完結する」が成立していなかった。

ダミー側で開始したことを覚えておき、後から購読した相手にも渡すようにした。
`FakeGameMasterClientTests` が固定している。

### 8. 本物でも開始を取りこぼしうる（未対応）

根本は**接続してから購読している**こと。`GrpcGameMasterClient` でも、
購読が始まる前にサーバが `GameStarted` を流せば同じように落ちる。
7 の修正はダミー側で辻褄を合わせただけで、この順序そのものは直していない。

正しくは購読してから接続する。`RootBootstrap` が
`NetworkBootstrap.Initialize()` → `GameBootstrap.Initialize()` の順で
呼んでいるところを入れ替えるか、購読だけを接続前に分ける必要がある。
本番の接続経路に手を入れるので、独立した変更として扱うのがよい。

---

## 開発環境の制約

| | |
|---|---|
| Unity | **6000.5.2f1**（`ProjectVersion.txt` が固定） |
| レンダーパイプライン | URP 2D（`Assets/Settings/Renderer2D.asset`） |
| DI | VContainer |
| 非同期 | UniTask |
| テスト | EditMode **105 件**。`Assets/Tests/EditMode/` |

HDR は有効、MSAA は無効。`Assets/DefaultVolumeProfile.asset` は存在するが
**全項目が中立値**（Bloom intensity 0）。ポストプロセスの土台は揃っていて
未設定の状態なので、Volume を 1 つ置けば効き始める。
`Renderer2D.asset` の Renderer Features は空。

### テストについて

**描画クラスそのものは EditMode テストで直接は触れない。** `UIService` と
`CircleTargetUI` は `MonoBehaviour` で、テスト用の asmdef は
`Struckout.Unity` を参照していない（Domain / Application / Infrastructure のみ）。

ただし移行にあたって、**「どこに、どの大きさで置くか」の計算は
`Domain` に出してあるので検証できる。**

- `WorldCoordinateTransformTests` — 盤面座標からワールド座標への変換、
  往復、どの比率でも盤面が収まること
- `FieldBoundsTests` — 盤面の矩形
- `TargetGeneratorTests` — 盤面の広さを変えたとき的もついてくること

見た目そのものを検証したいなら PlayMode テストを足す必要がある。
`CircleTargetUI` の移動補間（`SmoothStep`）はまだ `MonoBehaviour` の中にあり、
検証したければ切り出せる。

### このマシン固有の注意

- 物理メモリ 16GB。**Unity を 2 つ開くとバッチモードのテストが
  CoreCLR のヒープ確保に失敗して落ちる**
- バッチモードで回すときは `DOTNET_gcServer=0` を付ける

```powershell
$env:DOTNET_gcServer="0"
& "C:\Program Files\Unity\Hub\Editor\6000.5.2f1\Editor\Unity.exe" -batchmode -nographics -runTests -projectPath D:\struckout\projector -testPlatform EditMode -testResults results.xml -logFile unity.log
```

PowerShell から起動すると呼び出しが待たずに戻ることがある。
`Wait-Process -Name Unity` で終了を待ってから結果を読むこと。

---

## 動かして確かめる方法

**対向（ball_tracker / game_master）なしで起動できる。**
`GameLifetimeScope` の Network Settings で `Mode` を `Fake` にするか、
コマンドライン引数 `-networkMode fake` を渡す。

Fake モードでは以下が動く。

- `FakeClientService` がダミーの着弾を流す
- `FakeGameMasterClient` が接続直後に `GameStarted` を、
  150 秒後に `GameFinished` を流す
- 得点計算は**本物**が動く（`PointCalculator`）

つまり **描画とアニメーションの調整は対向を立てずに完結する。**
（以前は開始イベントが落ちて的が出なかった。上記 7 を参照）

起動時のログに以下が出る。合っているか最初に確認するとよい。

```
[Field] field=1920x1080 ppu=100
[Field] field=1920x1080 ppu=100 aspect=1.778 orthographicSize=5.400
```

座標変換の係数を合わせ込むときは、`GameLifetimeScope` の
Collision Transform で `Log Conversions` を有効にすると
変換前後の値がログに出る。

---

## 関連文書

- `docs/projector_behavior.md` — 的の挙動の仕様（当たったら移動、難易度）
- `docs/projector_debug.md` — デバッグ環境。**gRPC 移行前の記述が残っており古い**
- `docs/projector_todo.md` — 未着手項目。**完了済みの項目が多数残っており古い**
- `docs/machine_separation.md` — projector と ball_tracker を別マシンにする理由
