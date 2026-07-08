# 例外・null・接続失敗時の扱いについて

このプロジェクトでは、基本的な処理の流れは成立しているものの、例外や空参照、接続失敗に対する扱いがまだ弱いです。特に、ネットワーク層と初期化処理は「動く」ことを優先した実装になっており、運用を意識すると不安定な部分があります。

## 1. 例外・null・未初期化状態への対処が甘い

以下の箇所で、例外・null・未初期化状態に対する扱いが弱くなっています。

- [Assets/Scripts/Bootstrap/GameBootstrap.cs](../Assets/Scripts/Bootstrap/GameBootstrap.cs)
  - コンストラクタで依存オブジェクトの null を明示的に弾くように修正されていますが、今後は依存群をまとめて受け取る構造にすると、初期化の意図がより明確になります。
  - `context.PacketRouter` や `sensorProvider` などの依存関係が未設定だった場合に、後続処理がそのまま壊れる可能性があるため、初期化前に状態検証を一元化するのが望ましいです。

- [Assets/Scripts/Bootstrap/NetworkBootStrap.cs](../Assets/Scripts/Bootstrap/NetworkBootStrap.cs)
  - 接続失敗時に例外ではなく `NetworkConnectionResult` を返すように修正されていますが、より厳密には失敗理由とメッセージを持つ結果型にすると、ログや UI への反映がしやすくなります。
  - `DisposeAsync` では切断時のリソース解放が重要なので、初期化済みかどうかをより明示的に判定する構造にすると安全です。

- [Assets/Scripts/Infrastructure/TCP/TCPClientBase.cs](../Assets/Scripts/Infrastructure/TCP/TCPClientBase.cs)
  - `ConnectAsync` と `DisconnectAsync` は `ConnectionState` によって状態遷移を管理するように改善されていますが、さらに `ConnectAsync` / `DisconnectAsync` の同時実行を防ぐ仕組みを入れると、競合をより強く防げます。
  - `ReceiveDataAsync` 内で受信ループ中に例外が起きても、状態を安全に戻す処理を追加すると、接続異常時の回復性が上がります。

### 1-1. null を前提にした処理が散在している

いくつかの箇所では、引数や内部状態が null である可能性を考慮せずにそのまま使用しています。たとえば、UIサービスやゲーム起動処理では、依存オブジェクトが存在しない場合にその場で例外やログ出力に頼る形になっています。

この実装では、以下のような問題が起きやすいです。

- 依存オブジェクトが DI で注入されていない場合、後続処理で null 参照が発生する
- UI やターゲット生成処理で対象データが null のまま渡されると、意図しない挙動が起きる
- 例外が発生しても、呼び出し元で適切に回復できない

このような状態では、開発中にはログで気づけても、実際の運用では原因特定が難しく、ゲームが停止したまま復旧できない可能性があります。

### 1-2. 例外を「投げる」だけで終わっている

一部では例外を発生させる実装がありますが、呼び出し側でその例外を適切に処理していません。つまり、例外が発生した時点で処理を中断するだけで、再試行やフォールバック、ユーザーへの通知までつながっていません。

たとえば、接続失敗や初期化失敗が発生した場合に、単に例外を投げるだけでは、以下の問題が残ります。

- どの段階で失敗したかが分かりにくい
- 失敗後にリソースが残ったままになる可能性がある
- 画面側やゲーム側で「何をすべきか」が決まっていない

本来は、例外を投げる前に、失敗理由を明示し、状態を安全な形に戻し、必要なら再試行可能な構造にするべきです。

### 1-3. null チェックが曖昧で、意図が伝わりにくい

現状の実装では、null を見つけたらログを出して return する形が多く、処理の継続可否が曖昧です。これは短期的には動くものの、長期的には保守性を著しく下げます。

より良い設計では、以下のような考え方が必要です。

- 失敗したら例外として明示する
- 失敗を呼び出し側に返す
- 失敗時の状態を明確にする
- リカバリー手段を用意する

つまり、単なる「ログ出力」ではなく、処理が失敗したことを型や制御フローで扱えるようにするべきです。

# TCP 周りの実装における問題点

TCP 通信は、ネットワークの基本動作としては概ね実装されています。しかし、ここも「とりあえず動く」レベルに留まっており、接続の安定性や状態遷移の整合性に問題があります。

## 2. 再接続・切断時の状態管理が弱い

### 2-1. 接続状態の管理が一貫していない

接続状態は `_isConnected` で管理されていますが、接続成功時・失敗時・切断時の遷移が十分に明確ではありません。特に、以下のようなケースで状態が不整合になります。

- 接続に失敗したのに、後続の受信タスクが開始される可能性がある
- 切断時に `_receiveCancellationToken` が未初期化のまま参照される可能性がある
- `_tcpClient` や `_networkStream` が null のまま dispose や read を試みる可能性がある

このような状態では、通信が一度でも失敗すると、後続の処理が壊れやすくなります。

### 2-2. 切断処理が安全性を保証していない

切断処理では、`_receiveCancellationToken.Cancel()` を行い、その後に `_networkStream` や `_tcpClient` を破棄しています。しかし、以下のような点が危険です。

- `_receiveCancellationToken` が null のまま呼ばれる可能性がある
- `_receiveTask` がまだ開始されていない場合に await すると不整合が起きる
- 切断中に別スレッドが受信ループに入ると、状態競合が発生する

ネットワーク通信では、接続の開始・終了・再開が複雑なため、単純なフラグ管理では不十分です。接続状態を enum などで明示し、遷移を制御した方が安全です。

### 2-3. 再接続の考慮がない

今の実装は、起動時に一度だけ接続する構造です。もしサーバーが一時的に落ちていたり、ネットワークが切れたりした場合に、再接続を自動で試みる仕組みがありません。

再接続がないと、以下のような問題が発生します。

- 一度切断すると、アプリがそのまま通信不能になる
- ユーザーが再起動するまで復帰できない
- 例外が起きても握りつぶされ、状態が不明なままになる

実際のゲームやサービスでは、ネットワーク切断に対しては再接続ポリシーが必要です。短時間で再試行する、指数バックオフを使う、切断イベントを通知する、といった設計が望まれます。

## 3. パーサー初期化の堅牢性が不足している

### 3-1. `parser` が初期化されていない可能性がある

`TCPClientBase` の実装では、`parser` フィールドが参照されていますが、コンストラクタでの初期化が見当たりません。これは重大な問題です。もし parser が未設定のまま受信処理が走ると、受信時に null 参照が発生し、通信が壊れます。

この点は単なる「コード上の小さな欠陥」ではなく、通信基盤の根本的な信頼性に関わる問題です。受信処理の前に parser が必ず設定されていることを保証しなければ、接続が成功しても実際のデータ処理が失敗する可能性があります。

### 3-2. パーサーの失敗時に処理が止まらず、状態が不明になる

現在の実装では、パース失敗時にログを出して continue しています。これは一応の回復動作ですが、原因が分からないまま受信ループが続くため、デバッグが難しくなります。

より良い実装では、以下のように分けるべきです。

- パース失敗をログに残す
- 失敗したパケットを破棄する
- 必要なら再同期処理を行う
- 接続状態を異常として報告する

## 4. まとめ

この二点は、どちらも「動けばよい」というレベルの実装から、より実用的で保守しやすい実装へ進むために必要な観点です。

- 例外・null・接続失敗時の扱いが弱い
  - 失敗時にどのように回復するかが曖昧
  - 状態が不整合なまま残りやすい
  - 原因追跡が難しい

- TCP 周りの状態管理とパーサー初期化が弱い
  - 再接続・切断・異常終了の考慮が不足している
  - 接続状態の遷移が明確でない
  - 受信処理の前提条件が保証されていない

これらは、プロトタイプ段階では許容されることもありますが、実際に使う前提なら早急に改善すべきポイントです。特に、ネットワーク通信と初期化処理は、安定性を最優先で設計するべき領域です。

## 4. 具体的な修正案

ここからは、現状のコードにそのまま落とし込めるように、より具体的な修正案を示します。

### 4-1. 依存オブジェクトの null を防ぐ

[Assets/Scripts/Bootstrap/GameBootstrap.cs](../Assets/Scripts/Bootstrap/GameBootstrap.cs) では、コンストラクタで依存オブジェクトの null を明示的に弾くように修正されています。これは十分に良い改善ですが、さらに堅牢にするなら、依存を 1 つずつ受けるのではなく、`BootstrapDependencies` のような小さな受け渡しオブジェクトにまとめると、将来の依存追加に強くなります。

より良い変更案:

- `GameBootstrap` のコンストラクタ引数を個別ではなく、依存のまとまりとして受け取る
- `Initialize()` の前に `ValidateDependencies()` を実行し、異常時には明示的に失敗させる
- 依存の役割を名前付きの小さなクラスに分離し、テストしやすくする

この方法にすると、依存の増減に強く、コンストラクタが長くなりすぎることも防げます。

### 4-2. 接続失敗時に「例外だけ」ではなく「結果」を返す

[Assets/Scripts/Bootstrap/NetworkBootStrap.cs](../Assets/Scripts/Bootstrap/NetworkBootStrap.cs) では、接続失敗時に例外ではなく [Assets/Scripts/Domain/NetworkConnectionResult.cs](../Assets/Scripts/Domain/NetworkConnectionResult.cs) を返すように修正されています。これは良い方針ですが、さらに良くするなら、単なる列挙型ではなく、成功/失敗と理由をまとめた結果型を使うのがより扱いやすいです。

より良い変更案:

- `NetworkConnectionResult` だけでなく、`NetworkConnectionOutcome` のような結果型を導入する
- 成功時には `Success` を返し、失敗時には `Reason` と `Message` を持たせる
- 上位層で「再試行するか」「ログを出すか」「UI にエラーを表示するか」を分岐しやすくする

この設計にすると、列挙型よりも原因追跡と拡張性が高くなります。

### 4-3. 接続状態をフラグではなく状態列挙で管理する

[Assets/Scripts/Infrastructure/TCP/TCPClientBase.cs](../Assets/Scripts/Infrastructure/TCP/TCPClientBase.cs) では、`_isConnected` ではなく [Assets/Scripts/Domain/ConnectionState.cs](../Assets/Scripts/Domain/ConnectionState.cs) の `ConnectionState` 列挙型を使った状態管理に更新されています。これは十分に改善されていますが、さらに堅牢にするためには「状態遷移のルール」を明示するのが望ましいです。

より良い変更案:

- `ConnectionState` をそのまま使いつつ、遷移を `TryTransitionTo()` で制御する
- 不正な遷移を禁止し、`InvalidTransition` を返す
- `StateChanged` イベントを設けて、接続状態の変化を監視できるようにする
- `ConnectAsync()` / `DisconnectAsync()` が同時に呼ばれた場合の競合を防ぐため、状態変更にロックをかける

これにより、状態遷移のバグを減らしつつ、デバッグしやすい構造になります。

### 4-4. 切断処理の順序を安全にする

[Assets/Scripts/Infrastructure/TCP/TCPClientBase.cs](../Assets/Scripts/Infrastructure/TCP/TCPClientBase.cs) の `DisconnectAsync()` は、状態チェックと `try/finally` を使って安全性を高めています。ここはさらに良くするために、接続・切断処理の同時実行を防ぐ仕組みを入れるとより安定します。

より良い変更案:

- `SemaphoreSlim` や `lock` を使って、`ConnectAsync()` と `DisconnectAsync()` が同時に実行されないようにする
- 切断要求が入ったら、受信ループが終了するまで待機するための `TaskCompletionSource` を用意する
- `DisposeAsync()` と `DisconnectAsync()` の責務を分け、終了処理を一元化する

この変更により、ネットワーク接続の開始・終了が競合しにくくなり、再接続や異常終了時の安定性が上がります。

### 4-5. 再接続を前提に設計する

現状の実装は、起動時に一度接続するだけで、切断後の復帰が考慮されていません。実際のネットワーク環境では、サーバー停止や一時的な通信断が起こり得るため、再接続処理を入れるべきです。

改善例:

```csharp
public async Task<bool> ConnectWithRetryAsync(int maxAttempts = 5)
{
    for (var attempt = 1; attempt <= maxAttempts; attempt++)
    {
        try
        {
            if (await ConnectAsync())
            {
                return true;
            }
        }
        catch (Exception ex)
        {
            Debug.LogWarning($"Connect attempt {attempt} failed: {ex.Message}");
        }

        await Task.Delay(TimeSpan.FromSeconds(Math.Pow(2, attempt)));
    }

    return false;
}
```

### 4-6. パーサーを必須依存として明示する

[Assets/Scripts/Infrastructure/TCP/TCPClientBase.cs](../Assets/Scripts/Infrastructure/TCP/TCPClientBase.cs) では、パーサーが未初期化のまま使われる可能性があります。これは非常に危険です。コンストラクタで受け取るか、初期化時に必ず設定するようにすべきです。

改善例:

```csharp
private readonly IMessageParser<T> _parser;

public TCPClientBase(IMessageParser<T> parser)
{
    _parser = parser ?? throw new ArgumentNullException(nameof(parser));
}
```

### 4-7. パース失敗時の扱いを詳細化する

現在はパースに失敗したらログを出して `continue` していますが、これは追跡しにくいです。失敗件数を記録し、異常が一定回数を超えたら接続を切るようにすると、障害の検出精度が上がります。

改善例:

```csharp
private int _parseFailureCount;

try
{
    packet = _parser.MessageParse(data);
}
catch (Exception ex)
{
    _parseFailureCount++;
    Debug.LogWarning($"Parse failed ({_parseFailureCount}): {ex.Message}");

    if (_parseFailureCount >= 10)
    {
        _state = ConnectionState.Failed;
        break;
    }

    continue;
}
```

### 4-8. 追加で入れると良い改善点

さらに、以下の改善を入れると品質が大きく上がります。

- 接続・切断・エラー時にイベントを発火する
- `Debug.Log` を `Debug.LogError` や `Debug.LogWarning` で使い分ける
- UI 更新やネットワーク受信を分離し、例外が UI 側に伝播しないようにする
- 接続失敗・切断・パース失敗をテストで再現する