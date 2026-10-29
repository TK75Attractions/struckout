# game-master の Docker ビルドキャッシュ調査

2026-10-28 に GitHub CLI の `gh run list`、`gh run view --log`、`gh api` で調査した。
対象は `.github/workflows/release.yaml` の `publish-game-master-image`。

## 確認した実行

| 実行 | バージョン | amd64 の Cargo ビルド | arm64 の Cargo ビルド |
| --- | --- | --- | --- |
| [40618692038](https://github.com/TK75Attractions/struckout/actions/runs/40618692038) | 2026-10-28.0 | 1分48秒 | 20分12秒 |
| [40635157556](https://github.com/TK75Attractions/struckout/actions/runs/40635157556/job/122641133315) | 2026-10-28.1 | 2分22秒 | 20分13秒 |

両方のジョブで依存を含む `Compiling` が477件記録されている。
最新実行では `Updating crates.io index`、Git依存の取得、crates のダウンロードも発生した。
runner の Docker は `Architecture: x86_64` で、QEMU の `qemu-aarch64` が登録されている。
amd64 はネイティブ実行、arm64 は QEMU 上で ARM 版のコンパイラを実行する構成なので、
時間差の主因はエミュレーションと考えられる。両方とも `dev` profile のビルドであり、
amd64 の短いビルド時間はキャッシュヒットを示していない。
`importing cache manifest from gha` とキャッシュのエクスポート処理は実行されているが、
これだけでは Cargo のキャッシュが復元されたことにはならない。

## 原因

### Cargo のキャッシュマウントが実行間で保存されない

Dockerfile は registry、Git依存、target を `RUN --mount=type=cache` に置いている。
ワークフローの `cache-to: type=gha,mode=max` はビルドレイヤーを保存するが、
このキャッシュマウントの内容は保存しない。
新しい GitHub hosted runner の BuildKit では、ビルドを再実行すると Cargo の
ダウンロード済み依存とコンパイル済み依存が利用できない。

この制約は [Docker公式ドキュメントの Cache mounts](https://docs.docker.com/build/ci/github-actions/cache/#cache-mounts)
にも記載されている。`mode=max` への変更だけでは解決しない。

### バージョン引数がビルド環境のレイヤーも無効化する

`ARG ST_VERSION` と `ARG ST_COMMIT_SHORT_HASH` が builder ステージの先頭にある。
`ARG` より後の `RUN` は引数を暗黙の環境変数として受け取るため、
リリースごとの値の変更で最初の `RUN apt update ...` からキャッシュミスになる。
最新実行でも apt、ユーザー作成、mise のインストールが再実行されている。

仕様は [Dockerfile reference の Impact on build caching](https://docs.docker.com/reference/dockerfile/#impact-on-build-caching)
を参照。`COPY ./ ./` も、ビルドコンテキストに含まれるファイルの変更で
後続の Cargo ビルドレイヤーを無効化する。

## 修正方針

- バージョンなどの `ARG` を必要なビルド処理の直前に移し、環境構築のレイヤーを再利用する。
- Cargo の依存ビルドをソースのビルドから分離し、成果物を通常のレイヤーに保持して
  `type=gha` で保存する構成を検討する。依存ビルドをキャッシュマウント内だけに残す構成では解決しない。
- キャッシュマウントを維持する場合は、公式ドキュメントが紹介する
  `buildkit-cache-dance` などで内容を明示的に退避・復元する。
  amd64 と arm64 の target キャッシュは分ける必要がある。

## ネイティブビルドへの変更

`release.yaml` は次のジョブ構成に変更した。

1. `build-game-master-image` の matrix で amd64 を `ubuntu-24.04`、
   arm64 を `ubuntu-24.04-arm` でビルドする。QEMU は使用しない。
2. 各ジョブは GHCR に digest でイメージを公開し、digest を artifact に保存する。
   Docker レイヤーのキャッシュ scope は `game-master-amd64` と
   `game-master-arm64` に分け、並列ジョブ間の上書きを避ける。
3. `publish-game-master-image` が両方の完了後に artifact を取得し、
   `docker buildx imagetools create` で両方の digest を参照する manifest list を
   リリースバージョンのタグで公開する。
4. `publish-release` は引き続き `publish-game-master-image` の完了を待つ。

## Cargo の依存レイヤー

Dockerfile に `cargo-chef` の `prepare` と `cook` を追加した。`cook` は
`game_master` と同じプロファイル・feature で実行し、`target` と Cargo の
取得済み依存を通常の Docker レイヤーに保存する。これらの内容は
`type=gha,mode=max` の対象になる。`cook` に必要な protobuf と
`api/spec` はその前にコピーするため、これらの変更時には依存レイヤーを作り直す。
`ST_VERSION` と `ST_COMMIT_SHORT_HASH` は本体のビルド直前に宣言する。

`cargo-chef` は `game_master/mise.toml` と `mise.lock` で 0.1.78 に固定した。
ソースだけを変更した場合も workspace 内の crate は再コンパイルされる場合がある。
GitHub Actions とローカル Docker の完成ビルドは実行していない。

## Release #28 の権限エラー

[Release #28](https://github.com/TK75Attractions/struckout/actions/runs/40665865506) は
`cargo chef cook` が `/build/.cargo/config.toml` を書き直す際の
`Permission denied (os error 13)` で失敗した。`USER builder` の後でも
`COPY` の既定の所有者は root になるため、planner と builder にコピーする
ソース・recipe・Cargo 設定・API 定義を `--chown=builder:builder` で配置する。
`cook` は recipe から workspace のファイルを作成・更新するため、コピーした
入力にも builder の書き込み権限が必要になる。

各アーキテクチャのビルドジョブも digest で GHCR に push するため、
`build-game-master-image` の `GITHUB_TOKEN` に `packages: write` を付与する。

## Release #30 の権限エラー

[Release #30](https://github.com/TK75Attractions/struckout/actions/runs/40817338719) は
`cargo chef cook` が `/build/game_master/Cargo.toml` を作成できず失敗した。
`COPY --chown` はコピー対象の所有者を変更するが、コピー先の親ディレクトリは
root 所有で作られる。`base` ステージで `game_master/mise.toml` をコピーした時点で
`/build/game_master` が root 所有になり、前回の修正だけでは不足していた。
`cook` の直前に `/build` 以下を builder 所有に揃える。

## Release #31 の digest push エラー

[Release #31](https://github.com/TK75Attractions/struckout/actions/runs/40818989737) は
arm64 ジョブの Docker ビルド後、GHCR への push で
`can't push tagged ref ... by digest` と表示して失敗した。
`push-by-digest=true` を指定した出力にバージョン付きの `tags` も渡していたため、
digest と tag の二つの push 方式が衝突した。
アーキテクチャ別のジョブは tag のないイメージ名で digest を push し、
両方の digest を取得した後に `publish-game-master-image` がバージョンタグを付ける。
