# 変更ログ

形式は [Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) に従う。日付は JST。

## [0.5.0] - 2026-10-12

### Added

- 書く欄（`r`、受信箱と一覧）: 選んだ項目へのコメント。`Ctrl+Enter` で送る、`Ctrl+Shift+Enter` で送って閉じる（Issue と PR）、`Ctrl+P` でプレビュー（見出し・箇条書き・引用・コード・段落）、`Ctrl+B` `Ctrl+I` `Ctrl+K` で太字・斜体・リンク、`Esc` でやめる。Discussion へのコメントは GraphQL で送る。
- 項目の選び（書く欄の中）: `Alt+L` でラベル、`Alt+A` で担当者。入力で絞り込み、`Enter` で選び、`Esc` で反映する（変えたときだけ送る）。リポジトリのラベルと担当できる人は書く欄を開いたときに取る。
- `m`（受信箱）: マージの確認を出し、`Enter` で merge commit のマージ。下書きの PR・マージ済み・Issue には出さない。`a`: 自分に割り当て。
- 書き込みの通信（`minato-core::github`）: `Request::Send`・`Client::send`・`Call::Send`（POST / PUT / PATCH）。書いたあとは一覧と詳しくを取り直す。応答に `errors` があれば画面の下に出す。
- 書き込みの組み立て（`github::write`）と、書く欄の純粋な部分（`composer`: 選択を包む、Markdown の読み、選びの箱）を単体試験。

### Not yet

- Status・Priority・Milestone・Reviewers の選びと、項目の見せ方を設定で変える画面は、まだ。

## [0.4.0] - 2026-10-12

### Added

- 受信箱の画面（`1`）: 左に 3 つの組（あなたの番 / 待っている / 今日済んだ）のカード、下に状態・リポジトリ・タグの札、右に詳しく。詳しくの上に「なぜここにあるか」と次にやること。
- 受信箱に入れる規則 7 つ（`minato-core::inbox`）: 自分の PR の CI が赤 / 緑でマージできる / 自分への質問（ラベル `question`）/ `needs-owner` / 担当のいない `finding` / `lane-stalled` / dependabot の PR（既定は切）。
- キー: `j` `k`、`Space`（詳しくをめくる）、`d` 済み、`s` あとで（明日 9:00 に戻る）、`u` 取り消し、`o` ブラウザ、`f` `F` `t` で状態・リポジトリ・タグの絞り込みを順に回す。
- 「済み」「あとで」は GitHub を変えないローカルの印（今は再起動で消える）。
- 規則・組分け・印と取り消し・絞り込みを単体試験。

## [0.3.0] - 2026-10-12

### Added

- 一覧の画面（`2`）: 3 つの列（絞り込み / 3 行の項目 / 詳細）。`h` `l` で列を移り、`j` `k` で動く。`Space`・`Enter` で絞り込みを選び、`o` でブラウザで開く。
- 絞り込み: リポジトリ・種類（Issue / PR / Discussion）・作成者・保存した絞り込み（自分が担当、CI が赤）。上に GitHub の検索の書き方で出す。
- 項目は 3 行（題 / ラベル / リポジトリ・番号・作成者・経過）、右に CI の印と 💬 の数。入りきらない行は折りたたみ（1 行）、それでも足りなければ「⋯ 上に N 件」。スクロールバーは出さない。
- 詳細: 題・状態・本文・CI のチェック・会話。選んだ項目のぶんだけ取る（古い答えは捨てる）。
- GraphQL の検索（`user:{自分} is:open`）を 2 分ごとに回す。Discussions は別の問い合わせ。
- `minato-core::list`（絞り込み・選択・キー）と `github::search`（検索と詳細の解釈）を、記録した JSON で単体試験。

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
