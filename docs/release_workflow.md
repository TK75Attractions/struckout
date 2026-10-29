# リリース失敗の追跡

リリースは `.github/workflows/release.yaml` の `Release` workflow を `master` から手動実行する。完了結果を記録し、復旧を追跡するため、`.github/workflows/release-issue.yaml` が実行結果を監視する。

## 失敗と復旧

- `master` から開始した `workflow_dispatch` の Release が失敗すると、本文に `release-workflow-incident:master` マーカーを含む Issue を作る。
- 既に同じマーカーの open Issue があれば、新しい Issue を作らず、失敗した run のリンクをコメントする。同じ run attempt の再通知ではコメントを重ねない。
- 完了イベントを受け取るたびに Actions API から `master` の Release 実行履歴を読み直し、`workflow_dispatch` のうち成功または失敗で完了した最新の実行 (`run_number`、`run_attempt` 順) を現在の結果として扱う。古い完了イベントが遅れて届いても、より新しい完了結果を上書きしない。
- 最新結果が失敗なら、その後に queued / in progress の実行があっても失敗を記録する。新しい失敗が続いたときは同じ Issue にコメントし、run attempt マーカーで再通知による重複を防ぐ。
- 最新結果が成功なら、より新しい queued / in progress の実行がある間は Issue を開いたままにする。後続実行の完了後、成功が最新結果であれば成功 run のリンクをコメントして閉じる。
- 取り消しイベント自体は成功・失敗の結果として扱わないが、履歴を再確認する契機にする。たとえば、最新の成功より後に動いていた実行が取り消された場合、以前の成功を再評価して Issue を閉じる。`master` 以外からの実行は対象外。

Issue の識別にはタイトルの部分一致ではなく固定マーカーを使う。監視 workflow はソースや artifact を checkout せず、`actions: read` と `issues: write` のみを使う。状態変更は GitHub Actions の `GITHUB_TOKEN` で行い、イベント処理は workflow の concurrency queue で直列化する。

`workflow_run` の監視ファイルは default branch に存在して初めてイベントを受け取る。初回導入前に完了した run は遡って処理されない。手動実行の権限や通知の障害は Actions の `Release issue` workflow run を確認する。
