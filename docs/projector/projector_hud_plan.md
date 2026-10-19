# Projector の残り時間・得点 HUD

## 目的と期待動作

- touchpanel が同梱する Oxanium を Unity の HUD と終了メッセージで使用する。
- game_master の `GameTimeLimitNotify.remaining` と `GameScoreChanged.total_score` を受信し、既存 Canvas 上に残り時間と累計得点を表示する。
- 時間は touchpanel と同じく秒未満を切り捨てた `MM:SS`、得点は受信した `uint` の十進表記。表示のためにローカル得点を加算したり独自タイマーを進めたりしない。
- Idle は HUD 非表示。Playing に切り替わったら得点を `0`、時間を `--:--` にリセットし、通知受信後に表示を更新する。Finished は残り時間を `00:00` にし、最後に受信した得点を保持して表示する。

## 確認した実装と制約

- `api/proto/game-master.proto` に必要な通知が既にあり、Unity の生成コードも含まれる。proto と game_master の変更は不要。
- `game_master/src/service.rs` は残り時間を毎秒通知し、終了時に `GameFinished` を送る。終了直前の時間通知がゼロになることには依存しない。
- `GrpcGameMasterClient.HandleEvent` は時間通知を捨て、得点通知を未処理扱いにしている。`IGameMasterClient` からアプリケーション側へ通知を公開する。
- `RootBootstrap` は現在ネットワークを接続してから `GameBootstrap.Initialize` で購読するため、開始直後の通知を取り逃す余地がある。Fake も接続時に即座に開始を通知する。購読と初期盤面の構築を接続前に済ませる。
- Unity オブジェクトへのアクセスは既存の `IMainThreadDispatcher.Enqueue` 経由で行い、開始・時間・得点・終了のイベント順序を保持する。
- `UIService` は盤面を SpriteRenderer で描画し、HUD は Canvas に置く設計。`SampleScene.unity` には 1920×1080 基準の Overlay Canvas がある。Canvas の transform scale は現在ゼロなので、HUD が描画される値に修正する。
- フォントの元データは `touchpanel/ui/assets/fonts/Oxanium-VariableFont_wght.ttf`。Unity のアセット参照として取り込み、実行時に touchpanel のディレクトリを参照しない。新しい外部依存は追加しない。
- GameRuntime のローカル得点と `ScoreAdded` による差分送信は維持し、HUD の累計得点は game_master から受信した値で更新する。
- 既存の作業中変更を保持する。テストの追加は行わない。

## 変更範囲と担当

### Primary: UI、アセット、インターフェイス

対象:

- `projector/Assets/Scripts/Application/Interface/IUIService.cs`
- `projector/Assets/Scripts/Unity/UIService.cs`
- `projector/Assets/Scenes/SampleScene.unity`
- `projector/Assets/Fonts/` などの Oxanium アセットと対応する `.meta`

既存の Unity asmdef に uGUI の参照があるため、assembly 定義の変更は不要。

手順:

1. `IUIService` に `SetRemainingTime(TimeSpan)` と `SetScore(uint)` を追加する。
2. `UIService` に serialized `Canvas _hudCanvas` と `Font _displayFont` を用意し、シーンで既存 Canvas と Oxanium を割り当てる。
3. Canvas 上の左右上部に legacy uGUI Text で時間・得点表示を作成し、盤面や文字の重なりを避ける。ラベルは Oxanium に含まれる英数字で構成する。新しい scene コンポーネントを増やす必要がなければ `UIService` 内で構築する。
4. `SetGamePhase` で表示状態と初期値を管理し、受信値の表示 API を実装する。負の残り時間はゼロに丸め、時間の分数を 59 に制限しない。uint 得点を int に縮小しない。
5. `CreateFinishedText` の TextMesh にもフォントとその material を割り当てる。
6. フォントを Unity が正常にインポートし利用できることを確認する。変換が必要な場合も同じ Oxanium を使用し、別のフォントへの黙示的な差し替えはしない。

### Worker: 受信経路、起動順序、Fake

対象:

- `projector/Assets/Scripts/Application/Interface/IGameMasterClient.cs`
- `projector/Assets/Scripts/Infrastructure/Grpc/GrpcGameMasterClient.cs`
- `projector/Assets/Scripts/Infrastructure/Fake/FakeGameMasterClient.cs`
- `projector/Assets/Scripts/Bootstrap/GameBootstrap.cs`
- `projector/Assets/Scripts/Bootstrap/RootBootstrap.cs`

手順:

1. `IGameMasterClient` に `Action<TimeSpan>` の残り時間イベントと `Action<uint>` の累計得点イベントを追加し、受信スレッド上から通知される契約を記載する。
2. gRPC の既存イベント switch で protobuf Duration を TimeSpan に変換し、時間と累計得点を通知する。既存のゲーム ID 管理と整合させ、別ゲームの通知が現在の HUD を上書きしないようにする。
3. `GameBootstrap` で二つのイベントを購読し、`_dispatcher.Enqueue` 経由で IUIService を更新する。開始と終了も同じキューを利用する。
4. `RootBootstrap` の接続前にゲームのイベント購読と `GameSetup` を完了させる。ネットワーク失敗時の既存のエラー処理を保持する。
5. Fake にも同じ時間・得点イベントを実装する。開始時の時間・得点の通知、毎秒の時間通知、得点差分を受け取った後の累計通知、終了時のゼロ時間と終了通知を提供する。切断時に timer をキャンセルし、開始時に累計をリセットする。実通信と同様にゲーム外の加点を拒否する。

## 検証と受け入れ条件

- 差分確認で proto、game_master、ローカル得点計算と他の作業中ファイルを変更していないことを確認する。
- 利用可能な Unity Editor または既存生成 C# プロジェクトによるコンパイルで、変更した各 assembly の参照と API 整合性を確認する。既存環境の問題で実行できない部分は、その範囲と理由を報告する。
- Editor で実行可能なら Fake モードで開始直後の値、毎秒の更新、加点後の累計、終了表示、Oxanium と配置を目視確認する。実行できない場合、確認済みと未確認を区別する。
- 実 game_master 接続で確認可能なら、同じゲームの通知値が HUD に一致することを確認する。サーバーを利用できなければ実機確認を残課題として明示する。
- 新しいテストは追加しない。テストの実行範囲は親エージェントの適用指示に従い、実際に実行した検証のみ報告する。

## リスク・未確認事項

- Oxanium は可変フォントのため Unity のインポート結果を確認する必要がある。現在のフォントと同じファミリーを使用することを優先する。
- game_master のストリームは再接続時の現在値スナップショットを保証していない。今回の接続前購読でローカル初期化の取り逃しを解消するが、サーバーから送られない履歴の復元はスコープ外。
- 検証環境で Unity の起動・描画を確認できない場合、コードのコンパイルと実際のフォント表示を分けて報告する。

## 実施結果（2026-10-17）

- 上記の UI、シーン参照、フォント、受信経路、起動順序、Fake の実装を完了した。
- Unity 6000.5.2f1 を `-batchmode -nographics -quit` で起動し、C# コンパイルエラーなし、終了コード 0 を確認した。Application / Bootstrap / Infrastructure / Unity の assembly が更新された。
- TrueTypeFontImporter による Oxanium の取り込み完了をログで確認した。touchpanel 側と Unity 側のフォントファイルの SHA-256 は一致する。
- `git diff --check` で今回の変更に空白エラーがないことを確認した。テストの追加・実行は行っていない。
- 同じ起動ログに Visual Studio Editor パッケージの初期化例外（`AssetDatabase.IsAssetImportWorkerProcess` の呼び出し制限）が出た。今回の C# コンパイルとフォント取り込みは完了しているが、Editor 連携の問題は変更対象外とした。
- 描画を伴う Play Mode と実 game_master 接続は未確認。HUD の実機表示・通信による更新の確認は残る。
- 検証ログ: `C:/Users/tsuka/AppData/Local/Temp/struckout-hud-unity.log`。
