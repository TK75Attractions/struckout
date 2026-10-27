# CI の構成

`.github/workflows/ci.yaml` は `master` への push と pull request で必ず起動する。
`dorny/paths-filter` が変更ファイルを調べ、対象のモジュールだけを
`workflow_call` で呼び出す。手動実行 (`workflow_dispatch`) では全モジュールを実行する。

| チェック | 対象 |
|---|---|
| Android | `camera/`、`api/proto/` |
| Launcher | `launcher/`、`api/spec/` |
| Proto | `api/proto/`、C# の生成コード、`buf.gen.*.yaml` |
| Rust | Rust の各 crate、`api/proto/`、`api/spec/`、`.cargo/`、`.mise/`、`Cargo.toml`、`Cargo.lock` |
| Unity | `projector/`、`api/proto/` |

各チェックは、対応する workflow ファイルとルートの `mise.toml`・`mise.lock` が
変わった場合にも実行する。`ci.yaml` を変えた場合は全チェックを実行する。
対象パスの正確な一覧は `ci.yaml` の `filters` を参照。

branch protection rules の必須チェックには `CI result` を設定する。
従来の各 workflow のチェックを必須条件から外す。
このジョブは変更判定と各チェックの終了後に必ず実行され、実行したチェックの
失敗やキャンセルを検出する。対象外のチェックはジョブ単位で省略されるため、
ワークフロー全体のパスフィルターによる Pending が起きない。

Proto CI は `mise run proto` で C# コードを生成し、差分があれば自動でコミットして push する。
`GITHUB_TOKEN` による push は `push` イベントの CI を再起動しない。
自動コミットで pull request が更新された場合は CI の実行に承認が必要になることがあるため、
最新コミットの `CI result` を確認し、承認待ちなら `Approve workflows to run` で実行する。
詳しくは [GitHub のトークンによる workflow 起動の仕様](https://docs.github.com/en/actions/concepts/security/github_token#when-github_token-triggers-workflow-runs) を参照。
