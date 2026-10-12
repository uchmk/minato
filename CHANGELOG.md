# 変更ログ

形式は [Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) に従う。日付は JST。

## [0.1.0] - 2026-10-12

### Added

- 骨組み: Cargo workspace（`minato` と、egui に依らない `minato-core`）。
- 枠なしの窓: ロゴ、`minato`、ビューのタブ（`1` 受信箱 / `2` 一覧 / `3` Actions / `4` ボード）、パレットの欄、通知のベル、アカウント、窓のボタン、辺と角でのサイズ変更。
- 空の 5 つの画面と、下のキーバー（いまの画面のキー、⟳ 経過、API の残り、時計）。
- `?` のキー一覧、`,` の設定の画面（`ito-prefs` の共通ページ: 言語・大きさ・時計・テーマ）。
- `icon.rs` がロゴをコードで描き、`build.rs` が exe に焼く。
- 折りたたみの計算（`minato-core::fold`）と状態の文字列の単体試験。
- `scripts/verify.sh`、CI（Linux / Windows の test と clippy、ARM64）、issue フォーム、ラベルの作成、TESTING.md とチェック表。
