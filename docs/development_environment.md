# 開発環境

ツールの入手先は 2 つに分かれている。

| | 何を持つか | どこで動くか |
|---|---|---|
| `mise.toml` | protoc, JDK | Windows / macOS / Linux |
| `scripts/Bootstrap-Windows.ps1` | Unity, Android SDK, msys2, .NET SDK, rustup, dev.db, ASCII ジャンクション | Windows のみ |

`mise.toml` に置けるものは置く、という方針。
残っているものは OS 固有の事情があって mise では表現できないものだけ。

## 共通 (mise)

[mise](https://mise.jdx.dev/) を入れて、リポジトリのルートで:

```bash
mise trust
mise install
```

`mise trust` は初回だけ必要。mise が任意のリポジトリの設定を勝手に読み込まないための確認で、
これを実行しないと `mise.toml` は無視される。

以後このディレクトリに入ると `protoc` が PATH に載り、`JAVA_HOME` が設定される。

タスク:

```bash
mise run proto          # api/proto から C# を生成する
mise run proto:check    # 生成済みの C# が .proto と一致するか確認する
```

タスクは bash で動く。Windows では Git Bash (Git for Windows 同梱) が使われる。

## Windows

```powershell
.\scripts\Bootstrap-Windows.ps1     # 初回のみ。mise 本体も入れる
. .\scripts\Enter-DevShell.ps1      # シェルごとに読み込む
```

`Enter-DevShell.ps1` は内部で `mise env` を読み込んだうえで、
mise では扱えない以下を設定する。

- **Rust の退避先** — このリポジトリが `...\ドキュメント\` 配下にあると
  GNU binutils が非 ASCII パスで壊れる。`CARGO_HOME` / `RUSTUP_HOME` /
  `CARGO_TARGET_DIR` を `%USERPROFILE%\src\.struckout-tools` に逃がし、
  リポジトリ自体も `%USERPROFILE%\src\struckout` のジャンクション越しに参照する
- **msys2 の mingw** — `x86_64-pc-windows-gnu` ターゲットのリンクに使う
  `CC` / `AR` / リンカ / `RUSTFLAGS -C dlltool=`
- **`DOTNET_ROOT`** と **Android SDK の探索**
- **`DATABASE_URL`** — mise.toml の値を ASCII 側のパスで上書きする

## mise に入れていないもの

**Rust。** `x86_64-pc-windows-gnu` (msys2 の mingw とリンクする GNU ABI) と、
`mcu/` 用の xtensa ツールチェインの 2 つが要る。どちらも rustup 固有の機能なので
rustup に任せている。

**.NET SDK。** mise の dotnet バックエンドは Windows で
`installs/dotnet/<version>` から `dotnet-root` へのシンボリックリンクを作るが、
Windows 側からこれが解決できず「missing」と判定される。
その結果 `mise env` のたびに再インストールが走り、PATH にも載らない。
SDK 自体は正しく入るのでリンクだけの問題だが、実用にならないため
`Bootstrap-Windows.ps1` の `dotnet-install.ps1` に残してある。
非 Windows で問題なく動くことが確認できたら `mise.toml` に移してよい。

**Unity / Android SDK / msys2。** インストーラや `sdkmanager` のライセンス同意が
必要で、mise の守備範囲外。
