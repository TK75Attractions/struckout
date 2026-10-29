# Release workflow の `get-version` キャッシュ調査

2026-10-29 時点の調査結果。

## 観測結果

`get-version` は `Swatinem/rust-cache` のデフォルト設定を使う。キャッシュキーにはジョブ ID、Rust 環境、ワークスペース内の `Cargo.lock` / `Cargo.toml` などが含まれる。このため、`rust-build` や `rust_check` のキャッシュとは共有されず、依存関係の変更でもキーが変わる。

以下の実行では、`get-version` のキャッシュキーはすべて `v0-rust-get-version-Linux-x64-a74c6f26-9fff7b0b` だった。

| Release 実行開始 (UTC) | 実行結果 | `get-version` の復元結果 |
|---|---|---|
| [2026-10-29 04:39](https://github.com/TK75Attractions/struckout/actions/runs/40818989737) | failure | 完全一致でヒット |
| [2026-10-29 05:01](https://github.com/TK75Attractions/struckout/actions/runs/40820990400) | success | `No cache found`。ジョブ終了時に約 623 MB を保存 |
| [2026-10-29 08:10](https://github.com/TK75Attractions/struckout/actions/runs/40840244481) | success | `No cache found`。ジョブ終了時に約 623 MB を保存 |

同一キーでヒットした後、約 22 分後には見つからなくなった。これはキーの変化では説明できない。

2026-10-29 の調査時点で、GitHub Actions のキャッシュ API はリポジトリ内に 59 エントリ、合計 11.72 GB を返し、`v0-rust-get-version` で始まるエントリは 0 件だった。内訳は Unity Library が約 4.76 GiB、`rust-build` が約 2.97 GiB、Docker BuildKit が約 1.95 GiB、その他が約 1.24 GiB だった。

08:10 の実行では `get-version` が 08:13 にキャッシュを保存し、現存するキャッシュの最も古い最終アクセス時刻は 08:16:59 だった。`get-version` のキャッシュが他の現存エントリより先に削除された時系列は、最終アクセス順の退避と一致する。

`unity-build` は `get-version` が生成した `projector/Assets/Scripts/Unity/BuildInfo.Generated.cs` をダウンロードしてから、`projector/Assets/**` のハッシュを Unity Library のキャッシュキーに使う。生成ファイルにはリリースバージョンとコミットハッシュが入るため、リリースごとに 4 プラットフォーム分の新しいキーが作られる。04:39 の[Windows 向け Unity ジョブ](https://github.com/TK75Attractions/struckout/actions/runs/40818989737)でも、完全一致ではなく `restore-key` による復元だった。

## 原因と対応方針

主因は、リリースごとに大きなキャッシュが作られ、先にアクセスした `get-version` のエントリが容量上限により退避されることと考えられる。[GitHub のキャッシュ仕様](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching)では、容量上限に達すると最終アクセスが古いキャッシュから削除される。リポジトリに設定された容量上限そのものは今回確認していないため、退避の直接の理由までは断定できない。

改善する場合は、リポジトリのキャッシュ容量設定を確認するとともに、Unity Library のキーからリリース固有の生成ファイルを除外し、リリースごとの大容量キャッシュの作成を減らすことを検討する。`get-version` のキャッシュキーだけを変更しても、同一キーのエントリが消える問題は解決しない。

参考: [Swatinem/rust-cache のキャッシュキー仕様](https://github.com/Swatinem/rust-cache#cache-details)、[GitHub のキャッシュ容量設定](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/enabling-features-for-your-repository/managing-github-actions-settings-for-a-repository#configuring-cache-settings-for-your-repository)。
