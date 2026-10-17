//! Write `TESTING-CHECKS.md`: the manual half of TESTING.md, with a box to
//! tick per check. Ticks survive regeneration, rows a test names are left out.
//!
//! The generator is ito's `ito-testcheck` (shared with kura and tsumugi); this
//! is minato's configuration of it: the paths and the preamble.
//!
//!     cargo run -p minato --example make-testcheck [-- --check | --stats]
//!
//! `--check` writes nothing and exits 1 when the file is out of date. The file
//! keeps `[~]` (an appearance row an agent judged from a screenshot, not done
//! until the owner looks).

use ito_testcheck::testcheck::{run, Config, Counts, Lane};

fn main() {
    run(&Config {
        source: "TESTING.md",
        notes: "scripts/testcheck-ja.toml",
        src_dirs: &["crates"],
        dep_crates: &[],
        release_list: None,
        command: "cargo run -p minato --example make-testcheck",
        lanes: &[Lane { name: "windows", out: "TESTING-CHECKS.md", marks: &['~'], setup: false, check_release: false, preamble }],
    });
}

fn preamble(_: &Counts) -> String {
    "# 実機チェックリスト

`TESTING.md` から `cargo run -p minato --example make-testcheck` で生成している。**正は TESTING.md** で、
このファイルはその表を 1 行ずつ印を付けられる形に並べたもの。食い違ったら TESTING.md を信じること。

**チェック（`[x]` と `[~]`）だけは手で書いてよく、生成し直しても残る。**それ以外を書き換えても次の生成で消える。
`[x]` は実機で確かめた印、`[~]` は見た目の行を画面の画像で判断した印（済みには数えない。持ち主が同じ画像を見て `[x]` にする）。
**件数はこのファイルに書かない**（印を付けた PR が並ぶと必ずぶつかるため）。`-- --stats` で出る。

付ける前に「失敗していたら、画面かディスクの何が違ったはずか」を自問すること。
"
    .to_owned()
}
