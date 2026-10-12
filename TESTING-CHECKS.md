# 実機チェックリスト

`TESTING.md` から `cargo run -p minato --example make-testcheck` で生成している。**正は TESTING.md** で、
このファイルはその表を 1 行ずつ印を付けられる形に並べたもの。食い違ったら TESTING.md を信じること。

**チェック（`[x]` と `[~]`）だけは手で書いてよく、生成し直しても残る。**それ以外を書き換えても次の生成で消える。
`[x]` は実機で確かめた印、`[~]` は見た目の行を画面の画像で判断した印（済みには数えない。持ち主が同じ画像を見て `[x]` にする）。
**件数はこのファイルに書かない**（印を付けた PR が並ぶと必ずぶつかるため）。`-- --stats` で出る。

付ける前に「失敗していたら、画面かディスクの何が違ったはずか」を自問すること。

## 1. Window and title bar

- [ ] **1.1** Start `minato` → A 1280x800 window with no system title bar; the logo, `minato` (no 港), the tabs `1 Inbox` `2 List` `3 Actions` `4 Board`, a palette field in the middle, the bell, the account mark and three window buttons on the right
- [ ] **1.2** Drag the empty part of the title bar → The window moves; double-click maximises and restores
- [ ] **1.3** Click minimise, maximise, close → Each does what it says; close shows a red button under the pointer
- [ ] **1.4** Drag each edge and corner → The window resizes with the matching pointer; nothing resizes while maximised
- [ ] **1.5** Shrink the window → It stops at 720x420; nothing scrolls and no scrollbar appears anywhere
- [ ] **1.6** Look at the exe in Explorer and the taskbar → The same logo as in the title bar (tile, cyan and gold walls, blue wave, two lights)

## 2. Views and keys

- [ ] **2.1** Press `1`, `2`, `3`, `4` → The matching tab lights with an underline and the empty view's name and text change
- [ ] **2.2** Click each tab → Same as the digit
- [ ] **2.3** Press `?` → A card lists the global keys and the current view's keys; `?` or `Esc` closes it; digits do nothing while it is open
- [ ] **2.4** Press `,` or click the comma button → The settings screen opens; `Esc` closes it
- [ ] **2.5** Press `Ctrl+Shift+P` → A toast says it is not connected yet (the palette comes later)
- [ ] **2.6** Look at the keybar in each view → The keys of the view at the left, the first one lit, ending with `?`; no overflow at 720 px

## 3. Keybar status and clock

- [ ] **3.1** Look at the right of the keybar → `⟳ —`, `API —` and the clock `2026/10/12 (Mon) 17:40`-style text; the minute changes on its own
- [ ] **3.2** Switch the language to Japanese in the settings → Tabs, empty texts, help and the weekday (月) change; Japanese glyphs show (a system font loads in the background)
- [ ] **3.3** Turn the clock off in the settings → The clock disappears from the keybar

## 4. Settings

- [ ] **4.1** Open the settings → The common screen of the uchmk apps: General (language, size, clock) and Theme pages, a search, `Ctrl+Tab` moves between pages
- [ ] **4.2** Change the scale to 150% → The whole window is drawn larger at once and stays so after a restart
- [ ] **4.3** Pick another theme → The window's colours change at once; the sample card follows the theme
- [ ] **4.4** Edit `%APPDATA%\uchmk\common.toml` by hand while the app runs → The change shows within a few seconds
