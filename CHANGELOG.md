# 変更ログ

形式は [Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) に従う。日付は JST。

## [0.2.0] - 2026-10-12

### Added

- GitHub とつなぐ（`minato-core::github`）: トークンは `GITHUB_TOKEN` / `GH_TOKEN`、なければ `gh auth token`（値は `Debug`・`Display` に出さない）。
- HTTP は `ureq`（rustls）。ワーカースレッドとチャネルで UI から離し、同じキーの古い依頼は捨てる。
- ETag の `If-None-Match` と 304（手元の写しを返す）、レート制限の読み取り（残りが少ないと間隔を広げる）、GraphQL の送信。
- 記録した JSON（`tests/fixtures/`）と偽の通信で単体試験。本物の GitHub には通信しない。
- 窓: アカウントの印にログインの頭文字、keybar に `API 残り/上限` と `⟳` の経過、トークンが無いときの案内。

## [0.1.0] - 2026-10-12

### Added

- 骨組み: Cargo workspace（`minato` と、egui に依らない `minato-core`）。
- 枠なしの窓: ロゴ、`minato`、ビューのタブ（`1` 受信箱 / `2` 一覧 / `3` Actions / `4` ボード）、パレットの欄、通知のベル、アカウント、窓のボタン、辺と角でのサイズ変更。
- 空の 5 つの画面と、下のキーバー（いまの画面のキー、⟳ 経過、API の残り、時計）。
- `?` のキー一覧、`,` の設定の画面（`ito-prefs` の共通ページ: 言語・大きさ・時計・テーマ）。
- `icon.rs` がロゴをコードで描き、`build.rs` が exe に焼く。
- 折りたたみの計算（`minato-core::fold`）と状態の文字列の単体試験。
- `scripts/verify.sh`、CI（Linux / Windows の test と clippy、ARM64）、issue フォーム、ラベルの作成、TESTING.md とチェック表。
