# minato — Claude 向けメモ

GitHub の issue・PR・Discussion・Actions・Project のボードを、いくつものリポジトリにまたがってキーボードで扱うアプリ。Rust + egui、Windows 優先。
受信箱は「あなたの番のもの」だけを理由つきで並べ、一覧はそれ以外を探す。姉妹は [tsumugi](https://github.com/uchmk/tsumugi)（ターミナル）と
[yagura](https://github.com/uchmk/yagura)（見張り）。共有のクレート（`ito-*`）は [ito](https://github.com/uchmk/ito) にある。

- **画面と動きの正は [docs/design.md](docs/design.md) と HTML のモック [docs/mock/minato.html](docs/mock/minato.html)**（ブラウザで開いて `?`）。
  持ち主が 2026-10-12 に確定した。変えるときは先に `question` の issue で聞く。
- 最初に頼まれた作業の手順は [docs/dev-prompt.md](docs/dev-prompt.md)。
- **uchmk のアプリの共通仕様**（common.toml・設定の画面・テーマ・時計、`ito-common`・`ito-theme`・`ito-prefs`）は
  ito の [docs/common-spec.md](https://github.com/uchmk/ito/blob/main/docs/common-spec.md)。これに従う。
- Cargo の workspace。クレートは `crates/` の下に置く。版はルートの `Cargo.toml` の `[workspace.package]` で 1 つ。
- **push の前に `scripts/verify.sh` を 1 回回す。**最後の行が `ALL OK: …` なら push してよい（test、clippy を Linux・Windows・`aarch64-apple-darwin` の 3 つで `-D warnings`。どの機械からでも 3 つとも回る。ring が C を要るので、ホストでないものは cargo-zigbuild で（無ければ PyPI から入れる）。Windows の外では Windows を `x86_64-pc-windows-gnu` で見る。msvc は CI の `clippy-windows`）。

## 作業ルール

- 返答は日本語。コード・コメント・コミットメッセージは英語。
- 頼まれるまでコミットしない。
- 作業のブランチに push したら、頼まれなくても `main` にマージして push する（merge コミット。`main` が進んでいれば先にブランチに取り込み、`scripts/verify.sh` を回し直す）。
- 改行は LF。スクリプトで書き換えるときは改行を変えない（Python なら `newline=''`）。
- やることは TODO.md に書く（`【人】` は持ち主の作業）。
- 所見は `finding` の issue、人への確認事項は `question` の issue（背景・止まっている TODO・選択肢（推奨を 1 つ））。
  TODO.md の項目には `（要確認: #N）` を付ける。回答が来るまでは推奨で進めてよい。持ち主のコメント `回答: …` がいつも優先する。
- 持ち主の Windows の PC でしかできないこと（実機の確認・リリース・PC の環境）は、ラベル `needs-pc` の issue で PC のセッションに頼む。
  やること・終わりの条件・版かコミットを、会話の記録を読まなくても進められるように書く。
- **整形は `cargo fmt --all` で揃える**（v0.39.11 から。uchmk の全部のプロジェクトで使う）。設定は ito・yagura と同じ `rustfmt.toml`（`max_width = 140`、
  `use_small_heuristics = "Max"`）。CI と verify.sh が `cargo fmt --all --check` で見る。整形だけのコミットは `.git-blame-ignore-revs` に書く。
- clippy は `--all-targets -- -D warnings` で警告ゼロを保つ。検証は CI と同じ stable で回す。
  CI の `clippy` が Linux の機械から Linux と `aarch64-apple-darwin`（2026-10-16 から、verify.sh と同じく cargo-zigbuild で）を、`clippy-windows` が Windows の機械で msvc を見る。
- `Audit`（`.github/workflows/audit.yml`、2026-10-16 から。kura・tsumugi・ito・yagura・michi と同じ）は `cargo audit`（RustSec の脆弱性の一覧）を
  main への push・PR・毎週月曜に回す。脆弱性で落ち、警告では落ちない。赤くなったら、直った版に `cargo update -p <クレート>` で上げる。

## 共有のクレート（ito）

- `ito-* = { git = "https://github.com/uchmk/ito", rev = "…" }` で読む。**全部のクレートを同じ `rev` にそろえる。**
  使えそうなもの: `ito-common`・`ito-theme`・`ito-prefs`（設定）、`ito-i18n`（言語）、`ito-keys`（キー）、`ito-palette`（パレット）、
  `ito-match`（あいまい検索）、`ito-notify`（トースト）、`ito-winres`（exe のアイコン）、`ito-testcheck`（TESTING の表）。
- 足りないものや直しは ito に入れて push し、ここの `rev` を上げる（`cargo build` で `Cargo.lock` も合わせる）。ito のファイルは ito の CLAUDE.md に従う。
- 手元で一緒に直すときは `rev` を書き換えずにローカルの ito を差し込む（クレートごとに 1 つ `--config`）:

  ```sh
  cargo build --config 'patch."https://github.com/uchmk/ito".ito-theme.path="../ito/crates/ito-theme"'
  ```

## 実機のテスト（TESTING.md）

- 画面の無い環境では確かめられないもの（窓・キー・トースト・見た目・資格情報ストア）は TESTING.md の表に積む。
  **機能を足したら、そこに行を足してから完了にする。**動きを変えた行は印を外して（`[ ]` に戻して）から作り直す。
- `[x]` は実機で確かめた印で、付けてよいのは実機のセッションか持ち主だけ。見た目の行は `[~]`（画面の画像で判断した）まで。
- 文字で確かめられるもの（GitHub の応答の解釈・受信箱の規則・絞り込み・折りたたみの計算）は単体テストにして CI に回す。
  GitHub への本物の通信はテストでしない（記録した JSON を `tests/fixtures/` に置いて読む）。

## 版と変更ログ（コードが入ったら）

tsumugi と同じ。`Cargo.toml` の `version` が正、版の繰り上げと CHANGELOG.md（日本語、Keep a Changelog、日付は JST）はセットでコミットする。
`main` への直接の push を許す。マージは merge コミットで行う（rebase / squash を使わない）。

## 設計の約束事

- **トークンをファイルに書かない。**`gh auth token` から読むか、OS の資格情報ストア（Windows の資格情報マネージャー・macOS のキーチェーン・
  Linux の Secret Service）に置く。ログ・パニックのメッセージ・設定の画面にも出さない。
- **ディスクやネットワークに触る処理は UI スレッドで実行しない**（ワーカースレッドとチャネル、古い依頼は捨てる）。
  GitHub への問い合わせは ETag（`If-None-Match`）で 304 を受け、レート制限（`X-RateLimit-*`）を守って間隔を広げる。
- **描画とロジックを分ける**: GitHub の型・受信箱の規則・絞り込み・折りたたみは egui に依らない層（クレートかモジュール）に置き、単体テストを付ける。
- **マルチ OS**: Windows・macOS（Cmd キー）・Linux の違いは `#[cfg(...)]` のモジュールに閉じ込める。修飾キーは macOS では Cmd、ほかは Ctrl。
- **マルチアーキテクチャ**: x86_64 と ARM64 の 6 ターゲットへのクロスコンパイルを軽く保つ。Pure Rust のクレートを優先する（HTTP は rustls、OpenSSL を使わない）。
  rustls の暗号は C を含む ring のまま（2026-10-16 の持ち主の判断）。C の要らない rustls-graviola は 0.4 と若く、新しめの CPU（x86_64 は AVX2・ADX・BMI2、ARM64 は AES・SHA2）が要り、
  rustls-rustcrypto は alpha のままなので見送った。graviola が 1.0 になるか、rustls の組み込みの provider になったら考え直す。
- **GitHub を変える操作は取り消せるか確かめる。**マージ・閉じる・再実行・止めるなどはトーストで結果を出し、`u` で戻せるものは戻す。
  受信箱の「済み」「あとで」は GitHub を変えないローカルの印。
- **スクロールバーを出さない。**入りきらない行は 1 行に畳み、それでも足りなければ「上に N 件」「下に N 件」の行にまとめる（design.md の「折りたたみ」）。
- 色は tsumugi の Dark の系統（ito-theme）。アイコンは `icon.rs` でコードから描き、`build.rs` で exe に焼く（tsumugi・yagura と同じ）。
