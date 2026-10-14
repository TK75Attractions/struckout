# 2026-10-14 の 2 台接続テストでわかった不具合と、直すべき点

テストの構成と結果は `notes.md` にある。ここでは、直す必要があることを、確かめた方法とあわせてまとめる。

- **確認済み**: テストのログや端末のログで、実際に起きたことを確かめた。
- **コードから推測**: コードを読むと起きるはずだが、テストでは確かめていない。

行番号は、ブランチ `feature/camera-kyv45-support` の `f0a4499` 時点のもの。

## 一覧

| # | 対象 | 内容 | 重さ | 確かめ方 |
|---|---|---|---|---|
| 1 | カメラアプリ | ball_tracker との接続が切れると、アプリが落ちる | 高 | 確認済み (5 回) |
| 2 | カメラアプリ | 送り先が Network でつながっていないときに検出すると、アプリが落ちる | 高 | 確認済み (1 回) |
| 3 | カメラアプリ | 接続が切れたことに気づかず、再接続もしない | 高 | コードから推測 |
| 4 | カメラアプリ | 設定画面の送り先が、保存した値と違って表示される | 中 | 確認済み |
| 5 | ball_tracker | 着弾の CSV に改行が入らない | 中 | 確認済み |
| 6 | ball_tracker | 切れたカメラが残り、camera_id がつないだ順で変わる | 中 | 一部確認済み |
| 7 | ball_tracker | 同じ分に起動すると、前の出力ファイルを消してしまう | 低 | コードから推測 |
| 8 | 検出 | ボールがないあいだも、何かを検出し続けている | 要調査 | 確認済み (原因は未調査) |
| 9 | 追跡 | 2 台の時刻のずれが、探す範囲の端 (-30ms) に張り付く | 要調査 | 確認済み (原因は未調査) |
| 10 | 追跡 | 30fps の端末からは、1 球あたり視線が 2 本しかない | 要調査 | 確認済み |
| 11 | 運用 | 実験の手順で困ったこと | 低 | 確認済み |

アプリが落ちたときの記録は `logcat_crash_pixel9.txt` と `logcat_crash_cph2639.txt` にある
(各端末の logcat の crash バッファ)。

## 1. 接続が切れると、カメラアプリが落ちる

**起きたこと**: ball_tracker との接続が切れたとき、2 台ともアプリが `FATAL EXCEPTION` で落ちた。

| 時刻 | 端末 | きっかけ |
|---|---|---|
| 12:05:18 | OPPO | USB を抜いた |
| 12:20:21 | OPPO | USB の挿し直し |
| 12:21:00 | Pixel | USB の挿し直し |
| 12:35:20 | OPPO | ball_tracker を Ctrl+C で止めた |
| 12:35:23 | Pixel | ball_tracker を Ctrl+C で止めた |

どれも同じ例外で落ちている。

```
java.lang.IllegalArgumentException: the size of bytes must be 4 in order to convert to Int. Actual size was: 0
    at ...network.PacketHeaderKt.bytesToInt(PacketHeader.kt:29)
    at ...network.PacketHeaderKt$readPacket$len$1.invokeSuspend(PacketHeader.kt:18)
    at ...network.TcpSessionImpl.inputActor(...)
```

**原因**: 相手が接続を閉じると、`readPacket` (`PacketHeader.kt:17-19`) が読めた 0 バイトを
`bytesToInt` に渡し、`require` (`PacketHeader.kt:29`) が `IllegalArgumentException` を投げる。
`TcpSessionImpl.inputActor` (`TcpSessionImpl.kt:219-242`) は `IOException` しか捕まえないので、
例外がコルーチンの外に出てアプリが落ちる。

**同じ原因で落ちそうな場所 (コードから推測)**: 接続した直後にカメラの番号を読む `TcpSessionImpl.connect()`
(`TcpSessionImpl.kt:105-115`) も、`IOException` しか捕まえない。adb reverse は、PC 側で ball_tracker が
動いていなくてもスマホ側の接続をいったん受けてから閉じる。そのため、ball_tracker を起動する前に
送り先を Network にすると、ここで落ちる可能性がある。

**直し方の案**: 読めたバイト数が足りないときは、接続が閉じたこととして `EOFException` (`IOException` の一種)
を投げる。呼び出し側では、接続が切れた状態に戻す (3 へ)。

**ついでに起きたこと**: 12:35:27 に、Pixel のカメラ HAL (`liblyric_hwl.so`) が
`GraphRunner watchdog fired` で異常終了していた (crash バッファの tombstone)。アプリが落ちた 4 秒後なので、
カメラを使っている途中でアプリが落ちた影響と思われる。アプリが落ちなくなれば起きないかを確認する。

## 2. 送り先が Network でつながっていないときに検出すると、アプリが落ちる

**起きたこと**: 11:32:38 に、Pixel の 30fps 版 (`com.taichi765.struckoutCameraApp`) がメインスレッドで落ちた。

```
java.lang.IllegalStateException: TcpSession must be initialized before sending detection via network
    at ...network.NetworkManager.pushDetection(NetworkManager.kt:208)
    at ...config.PushFrameUseCase.invoke(PushFrameUseCase.kt:20)
    at ...camera.CameraViewModel$analyzer$1$1.invokeSuspend(CameraViewModel.kt:72)
```

11:48 に入れ直す前の版で起きたので、`CameraViewModel` の行番号は今のコードと合わない。ただし、
今のコードにも同じ確認 (`NetworkManager.kt:207-217` の `check`) がある。

**原因**: 送り先が Network のとき、`PushFrameUseCase` (`PushFrameUseCase.kt:19-21`) は接続を確かめずに
`NetworkManager.pushDetection` を呼ぶ。つながっていないと `check` が `IllegalStateException` を投げ、
`CameraViewModel` (`CameraViewModel.kt:138-140`) の `viewModelScope.launch` には例外を受け止める処理が
ないので、アプリが落ちる。ball_tracker が動いていないときや、接続に失敗したあとにボールなどを検出すると起きる。

**直し方の案**: つながっていないときは、検出を捨てる (または、つながるまでためる)。`check` で落とさない。

## 3. 接続が切れたことに気づかず、再接続もしない (コードから推測)

1 と 2 を直したあとに問題になる。

- `TcpSessionImpl` は、接続が切れても状態を `DisConnected` に戻さない (`TcpSessionImpl.kt:239-241` は
  ログを出すだけ)。`DataConnectionImpl` も同じで、状態を `Connected` にする処理しかない
  (`DataConnectionImpl.kt:46-62`)。
- `NetworkManager` が自動でつなぐのは、送り先が Network に変わったときの 1 回だけ
  (`NetworkManager.kt:106-149`、`distinctUntilChanged`)。失敗しても、設定画面の再接続ボタンを押すまで
  つなぎ直さない。
- 再接続ボタン (`NetworkManager.retryTcpConnection`) は、状態が `Connected` のままだと何もしない
  (`NetworkManager.kt:173-177`)。

**このテストで困ったこと**: ball_tracker を先に起動しないとつながらない。また、USB を挿し直したり
ball_tracker を起動し直したりするたびに、アプリを開き直す必要があった (実際には 1 で落ちていた)。

**直し方の案**: 読み書きに失敗したら状態を `DisConnected` に戻し、送り先が Network のあいだは、
一定の間隔でつなぎ直す。

## 4. 設定画面の送り先が、保存した値と違って表示される

**起きたこと**: Pixel の 120fps 版アプリで送り先を Network にして保存したつもりでも、保存されていなかった。
設定ファイル (`files/datastore/config.preferences_pb`) は 11:50 から書き換わっておらず、送り先の項目
(`detection_output_kind`) がなかった。そのため初期値の Local のまま動き、ball_tracker にはつながらなかった。
「Local」を押してから「Network」を押し直して保存したら、つながった。

**原因**: ラジオボタンの選択が、保存した値を読まずに、いつも先頭の「Network」で始まる。

```kotlin
// ConfigScreen.kt:55
val (selectedOption, setSelectedOption) = remember { mutableStateOf(radioOptions[0]) }
```

保存されるのは、ボタンを押したときに選んだ値 (`ConfigScreen.kt:106-110`) だけ。値が変わっていなければ
書き込まない (`ConfigViewModel.kt:80`)。

**直し方の案**: ラジオボタンの選択を `savedState.detectionOutputKind` から決める。

## 5. 着弾の CSV に改行が入らない

**起きたこと**: `20261014_1222.csv` が 1 行になり、点がつながった (`collisions_1222_raw.csv`)。
`2.1130140832677350,...` は、z = 2.113014083267735 のあとに次の点の x = 0 が続いたもの。
数の区切りがわからなくなるので、CSV としては読めない。

**原因**: `ball_tracker/src/collision_output/csv.rs:44` が `"{},{},{}"` を `\n` なしで書いている。

**直し方の案**: 行の最後に `\n` を付ける。分析に使えるように、時刻を列に加えることも検討する。
このテストでは、`events_1222.json` の `Collision` から `collisions_1222.csv` を作り直して使った。

## 6. 切れたカメラが残り、camera_id がつないだ順で変わる

**起きたこと (確認済み)**: camera_id はつないだ順に 0 から振られるので、起動するたびに端末との対応が変わった
(1211 の回は OPPO が 0、1222 の回は Pixel が 0)。ログを camera_id で見比べるときに、取り違えやすい。

**コードから推測**: カメラの接続が切れても、ball_tracker はそのカメラの位置を消さない
(`ball_tracker/src/detection_input/network.rs:246-248` はログを出すだけで、`CameraLocationStore`
(`ball_tracker/src/lib.rs:76-105`) に消す処理がない)。同じ ball_tracker のまま端末がつなぎ直すと、
新しい番号で登録されて、古い番号の位置も残る。`--time-trim-ms` の補正をかける「右のカメラ」は、
登録された位置の y が最も大きいものなので (`lib.rs:95-104`)、同じ位置の古い番号が選ばれると、
つながっているカメラに補正がかからないおそれがある。このテストでは、つなぎ直すたびに ball_tracker を
起動し直したので起きていない。

**直し方の案**: 切れたカメラの位置 (と時刻合わせの記録) を消す。番号は、端末ごとに変わらないもの
(端末が名乗る ID など) にすることも検討する。

## 7. 同じ分に起動すると、前の出力ファイルを消してしまう (コードから推測)

CSV と JSON のファイル名は分までの時刻で決まり (`ball_tracker/src/main.rs:130`、
`ball_tracker/src/tracking/event.rs:134,140`)、`File::create` で開くので、同じ名前のファイルは空になる。
同じ分のうちに ball_tracker を起動し直すと、前の回の出力が消える。

**直し方の案**: ファイル名に秒まで入れるか、同じ名前があれば番号を付ける。

## 8. ボールがないあいだも、何かを検出し続けている (要調査)

**起きたこと**: アプリは何かを検出したフレームしか送らない (`CameraViewModel.kt:110`)。それなのに、
1222 の回 (約 11 分、投球は 7 回) で、2 台からほぼ途切れずにフレームが届いた。

| | Pixel (camera_id 0) | OPPO (camera_id 1) |
|---|---|---|
| フレーム | 9,516 | 1,847 |
| 検出 | 17,705 | 2,512 |
| フレームが届いた秒数 | 223 秒 | 184 秒 |
| フレームが届いた 1 秒あたりのフレーム数 (中央値) | 44 | 8 |
| 検出の大きさ (面積の中央値 / 上位 10%) | 812px² / 11,832px² 以上 | 1,540px² / 26,414px² 以上 |

最初のフレーム (Pixel) の検出は 262×80px で、画像の下のほう (image_y 0.89) にある。ボールにしては大きい。
追跡では、軌道の候補を作って捨てた回数が 80,378 回あり、そのうち 78,915 回が `NoisyFit`
(視線が 1 本の軌道に乗らない) だった。誤って作られた着弾は見つかっていないが、計算が増え、
本物の球を取り違えるおそれがある。

**調べること**: 何を検出しているか (背景、人、照明のちらつきなど) をアプリ側で画像と一緒に確かめる。
そのうえで、検出の範囲、大きさの上限、色の条件を見直す。

## 9. 2 台の時刻のずれが、探す範囲の端に張り付く (要調査)

`ThrowDiagnostics` は、右のカメラ (このテストでは Pixel、y = 1.08) の時刻をずらしながら軌道を当てはめ直し、
視線と軌道の距離が最も小さくなるずれを探す (`ball_tracker/src/tracking/mod.rs:408-444`)。
探す範囲は -30〜+60ms (`mod.rs:47`)。

7 球のうち 6 球で、最もよいずれが範囲の端の **-30ms** だった。本当のずれは、もっと大きい可能性がある。

| track_id | 視線 (Pixel / OPPO) | ずらさないときの距離 | 最もよいずれ | そのときの距離 |
|---|---|---|---|---|
| 1 | 4 / 2 | 1.90cm | -30ms | 1.69cm |
| 2 | 2 / 2 | 1.51cm | -30ms | 1.27cm |
| 5 | 5 / 2 | 2.57cm | -30ms | 2.21cm |
| 6 | 6 / 2 | 2.73cm | -30ms | 2.46cm |
| 7 | 6 / 2 | 2.71cm | -30ms | 2.60cm |
| 9 | 2 / 2 | 1.04cm | -3ms | 0.40cm |
| 10 | 5 / 2 | 2.33cm | -30ms | 1.67cm |

このずれには、カメラの位置と向きの誤差も混ざる (`mod.rs:411`)。時刻合わせは 2 台とも成功しているが
(`synced=true`、往復 2〜5ms)、行ごとの露光のずれ (Pixel 4.5ms、OPPO 26.1ms)、露光時間
(OPPO は 30〜50ms と長い)、120fps 版のセンサー時刻の扱いのどれかがずれている可能性がある。

**調べること**: 探す範囲を広げて、本当に最もよいずれを求める。`frames_1222.json` を使って追跡をやり直せば、
実機なしで確かめられる。同時に、カメラの位置と向きを測り直す。

## 10. 30fps の端末からは、1 球あたり視線が 2 本しかない (要調査)

7 球すべてで、OPPO (30fps) の視線はちょうど 2 本だった。Pixel (120fps) は 2〜6 本。2 台の視線がそろわないと
軌道が決まらないので、OPPO が 1 コマでも取り逃すと、その球は出せない。OPPO は露光時間も 30〜50ms と長く、
ボールが流れて写りやすい。

**調べること**: 着弾が出なかった球がないかを、実際に投げた数と比べる (このテストでは投げた数を記録していない)。
2 台とも 120fps にするか、30fps の端末の露光時間を短くできないか検討する。

## 11. 実験の手順で困ったこと

| 困ったこと | 次からの対策 |
|---|---|
| USB を挿し直すと adb reverse が消え、つながらなくなった | 挿し直したら `scripts/adb-reverse.sh` を実行する。自動でかけ直す仕組みも検討する |
| 着弾の正解 (投げた数と、当たった位置) がない | 動画を撮るか、1 球ごとに当たった位置を記録する |
| ball_tracker のログ `tracker.log` を、起動のたびに上書きしてしまった (11:53 の回のログが残っていない) | 回ごとに別の名前で保存する |
| PowerShell のリダイレクトでログが UTF-16 になり、色の制御文字も入った | ログをファイルに直接書く機能を ball_tracker に加えるか、色を切る |
| ball_tracker が書いている最中の CSV と JSON が、エクスプローラーでは 0 バイトに見えた | 開いたままのファイルのサイズの表示が更新されないだけ。ball_tracker を止めてから見る |
| アプリの通常のログ (logcat) を残せなかった (集める前にアプリが落ちていた)。crash バッファだけは残っていた | テストの直後に `adb logcat -d` で端末ごとに保存する |
