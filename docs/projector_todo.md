# projector の未着手項目と実装方針

デバッグ環境を整える過程で分かった、projector 側に残っている課題と推奨する進め方。
上から順に優先度が高い。

---

## 判断が要るもの・要らないもの

着手前に決めることがあるかどうかで分けたもの。
下の各節はこの分類にそって読むとよい。

### 設計判断が要らない（そのまま着手できる）

振る舞いの解釈に余地がないか、既存の挙動を固定するだけのもの。

| 記号 | 内容 | 判断が不要な理由 |
|---|---|---|
| **A** | 切断を `ConnectionState` に反映する（`Connected → Failed` の追加、ストリーム解放、`Disconnected` 通知） | 状態が実態と食い違っている欠陥。直し方は一意で、再接続をするかどうかとは独立 |
| **B** | `StartGame` を受け取るまで得点を送らない（初期状態を `Idle` にする） | game_master が panic する経路のうち「開始前」を塞ぐ。仕様の解釈余地がない |
| **C** | Unity 非依存のテスト（座標変換・状態遷移・当たり判定・`Target`・フレーミング） | 既存の振る舞いを固定するだけ |
| **D** | `TargetGenerator` の初期配置を固定値で検証するテスト | 決定的な配置は仕様と確認済み |
| **E** | テスト容易化のリファクタ（時間源の注入、環境変数読み取りの差し替え） | 内部構造のみ。外から見た振る舞いは変わらない |

**B は「開始前」しか塞げない。**「終了後」を塞ぐには終了の伝達手段が要るので、
下の F が決まるまで完全には解決しない。それでも panic の経路が半分減る。

### 設計判断が要る

| 記号 | 決めること | 選択肢・論点 | 相談相手 |
|---|---|---|---|
| **F** | ゲーム終了をどう伝えるか | `EndGame` パケットを足す / `StartGame` に制限時間を載せて projector が自前で数える / 残り時間を定期送信する | game_master 担当 |
| **G** | 難易度で何を変えるか | 的の数・大きさ・クールダウン・配置パターン | 自分 |
| **H** | 終了時に画面をどうするか | 的を消す / 入力だけ無視する / スコアを出す | 自分 |
| **I** | 再接続のポリシー | 無限に試すか回数制限か。セッション中に切れたときの扱い。画面に出すか | 自分 + 運用 |
| **J** | CI を入れるか | Windows ランナーのコスト、対象範囲 | チーム |
| **K** | Unity ライセンスを CI に置くか | Secrets 管理、ライセンス種別 | 組織 |
| **L** | `bin`/`obj` の追跡を外すか | 同僚のコミット領域に触れる | チーム |
| **M** | 未使用になった API を残すか | `AddCollisionTargetAction` など | 自分 |
| **N** | `CollisionPoint` の `(x, z)` を確定させる | 縦に使う軸 | ball_tracker 担当 |

**F が全体の律速。** G・H は F が決まらないと形にならず、
セッション状態まわりは F 待ちになる。先に F を詰めるのが早い。

---

## 1. セッション状態を持たせる（最優先）

### 何が問題か

projector は的に当たれば**いつでも**得点を送る。一方 game_master は
`game_master/src/session.rs` でこう受けている。

```rust
let Some(session) = &mut guard.session else {
    panic!("recieved score from projector, but session not started");
};
```

**セッション開始前や終了後に得点が届くと game_master が panic する。**
projector が game_master を落とせてしまう状態で、これは早めに塞ぎたい。

さらに、制限時間 (`TIMELIMIT` = 1分30秒) を数えているのは game_master であり、
時間切れは `cb()` をローカルに呼ぶだけで **projector には何も送っていない**。
`MasterProjectorPacket` に `StartGame` しか無いため、そもそも伝える手段がない。

### 足りないもの

| | 現状 |
|---|---|
| projector 側のゲーム状態 | 無い。常に「プレイ中」として振る舞う |
| `StartGame` の処理 | 受信するがログを出すだけ。`GameRuntimeState.SetDifficulty` は未使用 |
| ゲーム終了の通知 | **プロトコルに存在しない** |
| 難易度による違い | 無い |

### 推奨方針

**手順1: プロトコルに終了を足す。** これは api/proto の変更なので、
game_master の担当者と合意が要る。

```protobuf
message EndGame {}

message MasterProjectorPacket {
  oneof payload {
    StartGame start_game = 1;
    EndGame end_game = 2;   // 追加
  }
}
```

game_master 側は `start_session` の `cb`（時間切れコールバック）から
`EndGame` を送ればよく、変更は小さいはず。

**手順2: projector にゲーム状態を持たせる。**
`GameRuntimeState` に `Idle` / `Playing` を追加し、`GameRuntime.CollisionDetected` の
先頭で `Idle` なら何もせず抜ける。得点送信も自然に止まる。

- `StartGame` 受信 → `SetDifficulty` して `Playing` に遷移、的を再配置
- `EndGame` 受信 → `Idle` に遷移、的を消すか入力を無視する

`OnGameStartReceived` の購読は `GameBootstrap` で行う。`PacketRouter` は
購読者がいなければログを出して捨てるようにしてあるので、
段階的に実装しても落ちない。

**手順3: 難易度を効かせる。** `GameSettings` を難易度ごとに持つのが素直。

```csharp
[Serializable]
public class DifficultySettings
{
    public Difficulty Difficulty;
    public int TargetCount;
    public float TargetCooldownSeconds;
    // 将来: 的の大きさ、配置パターン
}
```

`GameLifetimeScope` に配列で持たせ、`StartGame` の難易度で選ぶ。
的の配置パターンを複数用意する予定があるので、そのときにここへ合流させる。

> **注意**: `session.rs` の得点受信は `recv().await` を**ループしておらず1回で終わる**ように
> 見える。連続した得点が処理されるかは game_master の担当者に確認したほうがよい。

---

## 2. 自動再接続

### 何が問題か

接続は起動時に一度きり。ケーブルが抜けたり ball_tracker が再起動したりすると、
projector を再起動するまで復帰しない。本番はプロジェクタを常時稼働させるので、
運用に入る前に必要になる。

さらに、いま**切断が状態に反映されていない**。
`TCPClientBase.ReceiveDataAsync` は IO エラーでループを抜けるだけで、
`_state` は `Connected` のまま残る。そのため

- 切れているのに接続中に見える
- `SendAsync` が死んだストリームに書きに行く
- 上位層に切断を知らせる手段がない

`ConnectionStateMachine` にも **`Connected → Failed` の遷移が無い**ため、
「通信中に切れた」を表現できない。

### 推奨方針

**手順1: 切断を状態にする。**

- `ConnectionStateMachine` に `Connected → Failed` を追加
- 受信ループがエラーで抜けたら `Failed` に遷移し、ストリームを解放
- `IClientService` に `event Action Disconnected` を追加して通知

**手順2: 再接続を管理する層を作る。**
`TCPClientBase` に再接続まで持たせると責務が増えるので、分けたほうがよい。

```csharp
public class ConnectionSupervisor
{
    // Disconnected を購読し、ConnectRetryAsync を指数バックオフで呼び直す
}
```

`NetworkSettings` に `AutoReconnect` と `ReconnectDelaySeconds` を足して
Inspector から切れるようにする。デバッグ中は切れていたほうが分かりやすい場面もある。

**手順3: 再購読の二重登録に注意する。**
`OnReceived` の購読は `NetworkBootstrap.Initialize` で行っている。
再接続のたびに購読し直すと多重呼び出しになるので、
購読は `TCPClientBase` のインスタンス単位で1回だけにする（現状の作りならそのままでよい）。

**確認方法**: デバッグ GUI の `Stop` → `Listen` で切断を再現できる。
[tools/TESTING.md](../tools/TESTING.md) の Step 6 がそのまま回帰テストになる。

---

## 3. テストを書く

### 何が問題か

テストが1本も無い。Unity Test Framework は導入済み（`com.unity.test-framework` 1.7.0）だが、
テストアセンブリが存在しない。

今回のデバッグで見つかった不具合のうち、
**描画と当たり判定の半径の不一致**と**購読者なしイベントの NullReferenceException** は
どちらもテストがあれば実装時に気づけたもの。

### 推奨方針

`Assets/Tests/EditMode/` に `Struckout.Tests.EditMode.asmdef` を置く。
`Domain` / `Application` / `Infrastructure` を参照し、
`UnityEngine.TestRunner` と `UnityEditor.TestRunner` を references に、
`defineConstraints` に `UNITY_INCLUDE_TESTS` を入れる。

**すぐ書けるもの**（Unity 非依存で、モックもほぼ不要）

| 対象 | 内容 |
|---|---|
| `CollisionCoordinateTransform.ToRenderSpace` | 既定係数での四隅と中央。`FlipX/Y`、`SwapAxes` の各組み合わせ |
| `ConnectionStateMachine.Transition` | 許可された遷移と、例外になる遷移 |
| `CollisionSolver.TryCollision` | 円の内側・外側・**境界ちょうど** (`distance == radius`) |
| `Target` | `Size` が直径であること、`Radius` `RadiusSquared` の一貫性 |
| `TargetGenerator` | **初期4個の座標を固定値で検証**。決定的な配置は仕様なので、意図しない変化を検出できる |

**先にリファクタが要るもの**

- `GameRuntimeState` のクールダウンは内部で `Stopwatch` を持っているため時間を進められない。
  時間源をコンストラクタで受け取る形（`Func<long>` など）にすれば時間を制御してテストできる
- `NetworkSettingsResolver` は `Environment` を直接読む。
  検索関数を差し替えられるようにすれば、優先順位（コマンドライン > 環境変数 > Inspector）を検証できる

**やる価値が高いもの**

- フレーミングのラウンドトリップ。`api/rust` の `write_packet` が作るバイト列を
  固定値として持ち、C# 側が同じものを作り、同じものを読めることを確認する。
  今回 proto の生成ズレで実際に壊れていた箇所なので、回帰として効く

---

## 4. CI に C# / Unity を載せる

### 何が問題か

`.github/workflows` にあるのは `android_ci.yaml` と `rust_ci.yml` だけ。
しかも `rust_ci.yml` は `paths: mcu/**` に限定されているため、
`ball_tracker` や `game_master` すら回っていない。

**projector の変更は CI で一切検証されない。**
`scripts/Test-Dynamic.ps1` はローカル専用。

### 推奨方針

**段階1: ライセンス不要な部分から。** これは今すぐ入れられる。

`windows-latest` で以下を回すワークフローを追加する。

- `scripts/Generate-Proto.ps1 -Check` — proto の生成ズレを検出
- `dotnet build sandbox/testTcpCLI` — ダミー対向のビルド

proto のズレは実際に起きた事故なので、これだけでも入れる価値がある。
Unity ライセンスが不要なので導入の障壁も無い。

**段階2: Unity の EditMode テスト。**
`game-ci/unity-test-runner` を使う。ただし **Unity のライセンス認証が必要**で、
`UNITY_LICENSE` を Secrets に入れる必要がある。Personal ライセンスの扱いを含め、
組織として判断が要るので、段階1と分けて進めたほうがよい。
`6000.5.2f1` のイメージが提供されているかも事前に確認すること。

**段階3: `rust_ci.yml` の対象を広げる。**
`mcu/**` 限定を外し、ワークスペース全体を対象にする。これは私の担当外なので、
Rust 側の担当者に相談する話。

---

## 5. その他、小さいもの

| 内容 | 場所 |
|---|---|
| `sandbox/testTcpCLI/bin`・`obj` が `.gitignore` に入っているのに追跡されたまま。ビルドのたびに差分が出る | `git rm --cached` で外す。同僚のコミットにも含まれる領域なので要相談 |
| ball_tracker が `CollisionPoint` に `(x, z)` を詰めている箇所の `// FIXME: これあってる?` | 軸が違った場合は `Swap Axes` で暫定回避できるが、本来は ball_tracker 側で決着させる |
| `AddCollisionTargetAction` / `RemoveCollisionTargetAction` / `GameRuntimeState.RemoveTarget` が未使用 | 的を消さなくなったため。拡張点として残してあるが、使わないなら整理する |
| `FakeSensorProvider` が未登録 | 前任者のテストダブルとして意図的に残している |
