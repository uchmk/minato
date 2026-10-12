# 開発のセッションへの指示

minato の開発を始めるセッション（クラウドの Claude Code）に貼る文。持ち主が 2026-10-12 に設計を確定したときに書いた。

---

uchmk/minato で、GitHub を扱うデスクトップのアプリ minato を作り始めてください。

## まず読むもの

1. `CLAUDE.md`（作業ルール。返答は日本語、コード・コメント・コミットは英語、LF、`cargo fmt` は走らせない、push の前に `scripts/verify.sh`）
2. `docs/design.md`（確定した設計。画面・キー・受信箱の規則・折りたたみ・ロゴ）
3. `docs/mock/minato.html`（動くモック。ブラウザで開いて `?`。迷ったらこれと同じ動きにする）
4. 姉妹の作りを見本にする: tsumugi（`crates/tsumugi/src/icon.rs`・`build.rs`・`scripts/verify.sh`・`.github/workflows/`・
   `.github/ISSUE_TEMPLATE/`・TESTING.md の仕組み）と yagura、共有のクレートの ito（`docs/common-spec.md` と各 crate の README）。

## 進め方

下の段ごとに、動くところまで作って `scripts/verify.sh` が `ALL OK` になったら、版を上げ CHANGELOG.md を書いてコミットし、main に push してください
（このリポジトリは push を頼まなくてよい）。段の途中でも、区切りがよければ小さく push してかまいません。
画面の要る確認は TESTING.md に行を足し、持ち主の PC での確認が要るときは `needs-pc` の issue を立ててください。
設計にない決めごとが出たら `question` の issue に推奨を書いて、推奨で先に進めてください。やることは TODO.md に積んでください。

### 1. 骨組み（v0.1.0）

- Cargo の workspace、`crates/minato`（bin）。GitHub と受信箱のロジックは egui に依らないモジュールか別のクレート（例 `crates/minato-core`）に分ける。
- ito の `ito-common`・`ito-theme`・`ito-prefs`・`ito-i18n`・`ito-keys`・`ito-winres` を同じ `rev` で読む（tsumugi が今使っている `rev` に合わせる）。
- egui の窓: タイトルバー（ロゴ・`minato`・ビューのタブ・パレットの欄・通知の鈴）、空のビュー 5 つ（`1` `2` `3` `4` `,`）、keybar（時計つき）、
  `?` のキー一覧。設定の画面は ito の共通のもの。
- `icon.rs` で `docs/mock/logo.svg` の絵をコードから描き、窓のアイコンにする。`build.rs` で exe に焼く。
- 折りたたみの計算（design.md の「折りたたみ」）を egui に依らない関数にし、単体テストを付ける。
- `scripts/verify.sh`、CI（test と clippy `-D warnings` を Linux と Windows、ARM64 は arm64.yml）、`.gitattributes`、
  issue のフォーム（finding・question・needs-pc・retest）とラベル、TESTING.md と make-testcheck、TODO.md、CHANGELOG.md。

### 2. GitHub とつなぐ

- トークンは `gh auth token` から読む。無ければ設定の画面で入れてもらい、OS の資格情報ストアに置く（ファイル・ログに出さない）。
- HTTP は Pure Rust（`ureq` などに rustls）。ワーカースレッドとチャネルで UI から離す。
- ETag（`If-None-Match`）と 304、レート制限の残りを keybar に出す、少なければ間隔を広げる。
- 型と解釈は、記録した JSON（`tests/fixtures/`）で単体テストする。テストから本物の GitHub に通信しない。

### 3. 一覧と詳しく（読むだけ）

3 列（絞り込み・3 行の項目・詳しく）、`h` `j` `k` `l`、保存した絞り込み、CI のチェックとログ、やりとり。

### 4. 受信箱

規則（design.md の 7 つ、設定で付け外し）→ 3 つの組（あなたの番・待っている・今日済んだ）、why の 1 行、次にやること、
左下の状態とリポジトリの札とタグの札（`f` `F` `t`）、`d` `s` `u`（ローカルの印）、`o`、`g`。規則と組分けは単体テストを付ける。

### 5. 書く欄と項目

Markdown の書く欄（`Ctrl+Enter` など）、Preview、項目の札と `Alt+文字` の選ぶ窓（Labels・Assignees・Status・Milestone・Priority・Reviewers、
設定で出す・並び）。Project の項目は GraphQL の `updateProjectV2ItemFieldValue`。受信箱の `m`（merge コミット）・`a`・`R`。

### 6. Actions

実行の並びと絞り込み、詳しく（job・ログ・直近 10 回・artifacts）、`R` `X` `D` `e` `p` `o`。

### 7. ボード

Project「uchmk」の Status の列、`H` `L` とドラッグで動かして GitHub に書く。

### 8. 通知とパレット

`/notifications` の欄（`n`、`Shift+A`）、`Ctrl+Shift+P` のパレット（ito-palette・ito-match、`#番号`、命令）。

## 守ること

- UI スレッドでディスクとネットワークに触らない。
- GitHub を変える操作はトーストで結果を出し、失敗したら表示を戻す。戻せるものは `u` で戻す。
- スクロールバーを出さない（折りたたみ）。色は ito-theme の Dark。漢字の「港」をロゴの横に出さない。
- ito に足りないものは ito に入れて push し、`rev` を上げる（ito は `cargo fmt` で整形、ito の CLAUDE.md に従う）。
- 実機の印 `[x]` は付けない（持ち主か実機のセッションだけ）。
