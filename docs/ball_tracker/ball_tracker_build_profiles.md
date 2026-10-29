# ball_tracker のビルドプロファイル

ball_tracker の配布と `scripts/run_pc1.py` では、専用の `release-tracker` プロファイルを使う。
通常の `release` を継承し、依存 crate をまたぐ Thin LTO と `codegen-units = 1` を指定する。
ビルド時間より実行性能を重視するための設定であり、touchpanel などほかのモジュールの `release` 設定は変えない。

```bash
SQLX_OFFLINE=true mise exec rust protoc -- cargo build --locked --profile release-tracker -p ball_tracker
```

実行ファイルは `target/release-tracker/ball_tracker`（Windows では `.exe` 付き）に出る。
`scripts/run_pc1.py` はこの場所を使う。Release workflow も tracker だけをこのプロファイルでビルドし、
配布するアセット名は従来どおり `ball-tracker-<OS>` とする。
`bench_analysis` や `replay_video` を同じ設定でビルドする場合も、同じ `--profile release-tracker` を指定する。

特定 CPU 向けの `target-cpu=native` は指定しない。配布先の OS・CPU アーキテクチャに対応する
既存のビルド行列を維持し、panic 時の unwind など通常の `release` の動作も継承する。
設定の意味と出力先は [Cargo の公式資料](https://doc.rust-lang.org/cargo/reference/profiles.html) に従う。

分割前の統合ソースでは Windows Arm64 で `sentry` feature を含む全 bin のビルドを確認した。
この PR の独立した最新 master ベースでの検証結果は PR 本文に記す。
ユーザーの指示により性能比較は今回の対象から外したため、処理速度の改善率や長時間運用時の性能は未評価。
