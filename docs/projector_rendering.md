# projector の描画系 — 現状と引き継ぎ

描画の強化に着手する前に、いま何がどうなっているかを調べた記録。
2026-09-10 時点。コミット `e73b91b`（`feature/Projector-Grpc`）で確認した。

---

## 要点

**描画にあたるものが、ほぼ何も無い。**

- アセットは Prefab が 1 つだけ。マテリアル・スプライト・テクスチャ・アニメーション・
  シェーダは**リポジトリに 1 つも無い**
- 的は Unity 組み込みの UI スプライトを白 39% で置いただけ
- 着弾マーカーはアセットではなく、コードが実行時に組み立てた四角
- URP の 2D レンダラが入っているが、**描画経路がそれを通っていない**（後述）

裏を返すと、強化フェーズは「既存の作り込みを壊さないように直す」のではなく
**ほぼ更地に作る**話になる。制約は少ない。

---

## シーンの構成

`Assets/Scenes/SampleScene.unity`（720 行。Unity のシーンとしては極小）。
GameObject は 7 つしかない。

| GameObject | 中身 |
|---|---|
| `Main Camera` | 直交投影、size 5、単色クリア |
| `Global Light 2D` | URP 2D のグローバルライト |
| `Canvas` | **Screen Space - Overlay**。的の親でもある |
| `UIService` | 的とマーカーの生成を担う。`IUIService` の実装 |
| `MainThreadDispatcher` | 受信スレッドから UI スレッドへ戻すため |
| `GameLifetimeScope` | VContainer の DI 定義。設定値もここに載る |
| `EventSystem` | uGUI の既定 |

シーン名が `SampleScene` のまま。改名は履歴が追いにくくなるので、
やるなら描画作業に入る前が良い。

### 描画経路

```
GameRuntime ──> IUIService ──> Canvas (Screen Space - Overlay) の子として
                               RectTransform + Image を実行時に生成
```

**カメラも Global Light 2D も、この経路には関与しない。**
Screen Space - Overlay の Canvas はカメラを介さず最前面に描かれるため、
2D ライトもポストプロセスも的には一切かからない。ワールド空間には
オブジェクトが 1 つも無いので、カメラが描いているのは背景色だけ。

つまり **URP 2D の機能はいま完全に遊んでいる。**

---

## アセットの中身

### `Assets/Prefab/CircleTarget.prefab`（唯一の Prefab、126 行）

```
CircleTarget          RectTransform  sizeDelta 2x2  anchor(0,0)  pivot(0.5,0.5)
└─ Panel              Image
                        sprite: Unity 組み込み（guid が 0000...f000... はビルトイン）
                        color:  白, alpha 0.392
```

- ルートの `sizeDelta` が 2x2 なのは、`CircleTargetUI.Initialize` が
  実行時に `Target.Diameter` で上書きするため。Prefab 側の値には意味がない
  （`localScale` で拡縮しない理由はコード中のコメントに書いてある）
- **プロジェクト固有の画像素材は使っていない**。差し替えの起点はここ

### 着弾マーカー

Prefab が存在しない。`UIService._collisionMarkerUI` が未設定のため、
`UIService.CreateDefaultMarker` が実行時に `GameObject` を組み立てている。
的が円なので区別できるよう 45 度回した四角。

`UIService` の Collision Marker UI に Prefab を割り当てれば、
コード側の生成は使われなくなる（見た目も寿命もその Prefab の責任になる）。
**演出を作る場合の最初の差し込み口はここ。**

---

## 表示の設定

| 項目 | 値 | 効いてくる場面 |
|---|---|---|
| 既定解像度 | 1920 x 1080 | `TargetGenerator` が座標を 0〜1920 / 0〜1080 で置いている |
| フルスクリーン | Fullscreen Window（ボーダーレス） | |
| ウィンドウリサイズ | 不可 | |
| Canvas Scaler | Scale With Screen Size、参照 1920x1080 | |
| Screen Match Mode | **Match Width Or Height = 0（幅に合わせる）** | ⚠ 下記 |
| Canvas Render Mode | Screen Space - Overlay | ⚠ 下記 |

⚠ **幅基準のスケーリング**は、投影面のアスペクト比が 16:9 からずれると
縦方向の可視範囲が変わることを意味する。`TargetGenerator` は Y を 0〜1080 で
固定して置くので、比率が違うと的が画面外に出るか余白が出る。
実機のプロジェクタの比率が決まっているなら、ここは早めに詰めたほうがよい。

---

## 描画に関わるコードの地図

どこを触ると何が変わるか。

| ファイル | 役割 | 強化時に触る可能性 |
|---|---|---|
| `Unity/UIService.cs` | 的の生成、移動の委譲、マーカーの生成 | **高**。演出の起点 |
| `Unity/CircleTargetUI.cs` | 的 1 個の見た目。位置・大きさ・移動補間 | **高** |
| `Unity/ITargetUI.cs` | 的 UI の契約（`Initialize` / `MoveTo`） | 中。種類を増やすなら |
| `Unity/CollisionMarker.cs` | マーカーの拡大とフェード | 中。Prefab に置き換えるなら不要になる |
| `Unity/UIRoot.cs` | 的を置く親 RectTransform を保持 | 低 |
| `Infrastructure/TargetGenerator.cs` | 的の**位置と大きさ**を決める | 中。見た目の前提を決めている |
| `Domain/Target.cs` | 的の状態。`Size` は直径 | 低 |
| `Application/GameRuntime.cs` | 当たり判定と得点。UI を呼ぶ側 | 低 |

### 押さえておくべき約束事

- **`Target.Size` は直径**。`CircleTargetUI` が `sizeDelta` に直径をそのまま入れ、
  `CollisionSolver` が `RadiusSquared` で判定するので、この関係が崩れると
  **見た目と当たり判定がずれる**。`TargetTests` が固定している
- **`Target` は参照で等価**。`UIService` が `Dictionary<Target, Transform>` で
  対応づけているため、座標を等価性に含めると移動のたびに対応が壊れる
- **的は消えない**。当たったら同じ GameObject が移動する
  （`docs/projector_behavior.md`）。当たり判定は移動先で即座に有効になり、
  見た目が追いつくまでの短い間だけ表示位置とずれる
- **着弾マーカーは当たり外れに関わらず出す**。座標変換がずれているのか
  単に外したのかを区別するため

---

## 着手時に最初にぶつかる設計判断

### 1. Screen Space - Overlay のままにするか

**これが最大の分岐。** 2D ライト、シェーダ、パーティクル、ポストプロセスを
使いたいなら Overlay では不可能で、次のいずれかに移す必要がある。

| 選択肢 | 得られるもの | 代償 |
|---|---|---|
| Screen Space - Camera | カメラのポストプロセスが乗る | Canvas とカメラの距離・ソート順の調整 |
| World Space Canvas | 2D ライトが当たる、3D 的な演出 | 座標系の変換が増える |
| uGUI をやめてスプライトに | URP 2D の機能をフルに使える | `UIService` 周りを書き直し |

**現状のコードは uGUI（RectTransform + Image）に強く結びついている。**
移す場合、`UIService` と `CircleTargetUI` は書き直しに近くなる。
逆に `IUIService` / `ITargetUI` というインターフェースで隔ててあるので、
`GameRuntime` から上は影響を受けない。

### 2. 的の見た目をどう作るか

いまは組み込みスプライトの単色。素材を作るのか、シェーダで描くのか。
`TargetType` は `Circle` 1 種類しか無いので、形を増やすならここから。

### 3. 難易度で見た目を変えるか

`docs/projector_todo.md` の **G**（難易度で何を変えるか）が未決のまま。
的の数・大きさ・配置パターンが候補に挙がっている。
`GameSettings` は `InitialTargetCount` しか持っていない。

### 4. ゲーム終了時の画面

`docs/projector_todo.md` の **H** が未決。`GamePhase.Finished` は実装済みで
状態は取れるが、**画面上は何も起きない**。

---

## 着手前に直しておきたい不整合

シーンがコードから遅れている。Unity でシーンを開いて保存し直せば
多くは解消するが、1 つだけ機能に影響するものがある。

| # | 内容 | 影響 |
|---|---|---|
| 1 | **`GameLifetimeScope._networkSettings.MasterPort` がシーン上で `5001` のまま** | ⚠ **機能する**。コード既定は 8020 に直したが、シーンの値が優先されるので **game_master に繋がらない** |
| 2 | `UIService._coolingDownColor` が残っている | コードは `_ignoredColor` に改名済み。値は同じなので実害なし |
| 3 | `_gameSettings.TargetCooldownSeconds` が残っている | クールダウンは廃止済み。実害なし |
| 4 | `_scoreSettings` がシーンに無い | コード既定（仮の値）が使われる。編集するには一度保存が必要 |
| 5 | `_networkSettings.MachineId` がシーンに無い | 既定の 1 が使われる |
| 6 | `Assets/_Recovery/0.unity` が git 追跡下にある | Unity のクラッシュ復旧用の生成物。追跡から外すべき |

**#1 は描画作業とは無関係だが、実機確認をする前に必ず直すこと。**

---

## 開発環境の制約

| | |
|---|---|
| Unity | **6000.5.2f1**（`ProjectVersion.txt` が固定） |
| レンダーパイプライン | URP 2D（`Assets/Settings/Renderer2D.asset`） |
| DI | VContainer |
| 非同期 | UniTask |
| テスト | EditMode 71 件。`Assets/Tests/EditMode/` |

### テストについて

**描画クラスは EditMode テストで直接は触れない。** `UIService` と
`CircleTargetUI` は `MonoBehaviour` で、既存の 71 件はすべて
Unity 非依存の Domain / Application / Infrastructure を対象にしている。

見た目を検証したいなら PlayMode テストを足すか、
ロジックを `MonoBehaviour` の外に出す必要がある。
`CircleTargetUI` の移動補間（`SmoothStep`）などは切り出せば EditMode で検証できる。

### このマシン固有の注意

- 物理メモリ 16GB。**Unity を 2 つ開くとバッチモードのテストが
  CoreCLR のヒープ確保に失敗して落ちる**
- バッチモードで回すときは `DOTNET_gcServer=0` を付ける

```powershell
$env:DOTNET_gcServer="0"
& "C:\Program Files\Unity\Hub\Editor\6000.5.2f1\Editor\Unity.exe" `
  -batchmode -nographics -runTests `
  -projectPath D:\struckout\projector -testPlatform EditMode `
  -testResults results.xml -logFile unity.log
```

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

座標変換の係数を合わせ込むときは、`GameLifetimeScope` の
Collision Transform で `Log Conversions` を有効にすると
変換前後の値がログに出る。

---

## 関連文書

- `docs/projector_behavior.md` — 的の挙動の仕様（当たったら移動、難易度）
- `docs/projector_debug.md` — デバッグ環境。**gRPC 移行前の記述が残っており古い**
- `docs/projector_todo.md` — 未着手項目。**完了済みの項目が多数残っており古い**
- `docs/machine_separation.md` — projector と ball_tracker を別マシンにする理由
