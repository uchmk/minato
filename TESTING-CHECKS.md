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

- [x] **2.1** Press `1`, `2`, `3`, `4` → The matching tab lights with an underline and the empty view's name and text change
- [ ] **2.2** Click each tab → Same as the digit
- [x] **2.3** Press `?`, then `F1` → A card lists in two columns the global keys and the notifications' at the left and the current view's at the right, every key that works there among them (the list's `M`, Actions' `p`); `F1`, `?` or `Esc` closes it; digits do nothing while it is open
- [x] **2.3a** In the inbox, press `F1` (then `Tab` if the window is small or zoomed in); then in the board → In the inbox, a table "Inbox cards: why they are there and what to do" has a row per reason (its key, the reason as on the card, what to do), then `c`, `d` and `s`: as a third column beside the keys on a wide window, below them on a narrower one, else on a second page that `Tab` turns to and back ("Tab: the keys or the cards' reasons" at the bottom right). Nothing is cut off or overlaps the closing line. In the board there is no such table
- [ ] **2.4** Press `,` or click the comma button → The settings screen opens; `Esc` closes it
- [ ] **2.5** Press `Ctrl+Shift+P` → A toast says it is not connected yet (the palette comes later)
- [x] **2.6** Look at the keybar in each view, then narrow the window or zoom in (`Ctrl+=`) → The keys of the view at the left, the first one lit, ending with `F1`; when they do not fit, the view's last keys go and `F1` stays, never under `⟳`, `API` or the clock

## 3. Keybar status and clock

- [x] **3.1** Look at the right of the keybar → `⟳ —`, `API —` and the clock `2026/10/12 (Mon) 17:40`-style text; the minute changes on its own
- [ ] **3.2** Switch the language to Japanese in the settings → Tabs, empty texts, help and the weekday (月) change; Japanese glyphs show (a system font loads in the background)
- [ ] **3.3** Turn the clock off in the settings → The clock disappears from the keybar

## 4. Settings

- [x] **4.1** Open the settings → The common screen of the uchmk apps: General (language, size, clock) and Theme pages, a search, `Ctrl+Tab` moves between pages
- [ ] **4.2** Change the scale to 150% → The whole window is drawn larger at once and stays so after a restart
- [ ] **4.3** Pick another theme → The window's colours change at once; the sample card follows the theme
- [ ] **4.4** Edit `%APPDATA%\uchmk\common.toml` by hand while the app runs → The change shows within a few seconds

## 5. GitHub connection

- [x] **5.1** Start `minato` where `gh auth login` is done → Within a few seconds the account mark shows your initial (hover: your login) and the keybar shows `API 4,xxx/5,000` and `⟳ 0s ago`
- [ ] **5.2** Start it with no `gh` login and no `GITHUB_TOKEN` → A toast says there is no token and how to get one; the account mark stays `?`
- [ ] **5.3** Start it with `GITHUB_TOKEN=bad` → A toast says GitHub refused the token (401); the token text appears nowhere
- [ ] **5.4** Leave it open for 3 minutes → The `⟳` age counts up and drops back to a few seconds after each poll; the window stays idle otherwise

## 6. List view

- [x] **6.1** Press `2` after the connection is up → Within a few seconds your open issues, PRs and discussions list in three lines each; on the title's line at the right stand, in this order, the CI mark, the comment count and the `受信箱` badge, inside the row even when it has no labels
- [ ] **6.2** Press `j` / `k` → The selected row moves; the detail at the right follows within a second
- [ ] **6.3** Shrink the window height until the rows do not fit → As many whole rows as fit show around the selected one; the rest hide with "⋯ 上に/下に N 件"; no row is folded to one line and no scrollbar appears
- [ ] **6.4** Press `h`, then `j` and `Space` on a repository → The list narrows to it and the query line over the list changes; the chosen option is filled
- [ ] **6.5** Pick "CI is red" → Only PRs with a red mark remain
- [ ] **6.6** Press `l` twice and `j` → The detail scrolls by lines, with no scrollbar; moving the row resets it
- [ ] **6.7** Press `o` → The item opens in the built-in browser at the window's right (Windows, macOS; Linux: the system's browser)
- [ ] **6.8** Look at a PR with checks → The detail lists each check with ✓ ✗ ◐ in its colour, then the conversation
- [ ] **6.9** Zoom in (`Ctrl+=` twice) and look at the filter column, then at Actions' → A long repository name ends with "…" before its count; "Author" has no empty row; the rows that do not fit hide with "⋯ 下に N 件" at the bottom (and "⋯ 上に N 件" at the top once the cursor is moved down), none is cut in half

## 7. Inbox view

- [x] **7.1** Press `1` after the connection is up → Cards of your turn (red or ready PRs, `question` / `needs-owner` / `lane-stalled` items, unassigned findings) stand under "Your turn"; each shows its reason in colour
- [ ] **7.2** Select a card with `j` / `k` → The detail at the right follows; its top line shows the reason and the next key
- [ ] **7.3** Press `d`, then `u` → The card moves to "Done today" with a toast, and `u` brings it back
- [ ] **7.4** Press `s` → The card disappears (back tomorrow 9:00) with a toast; `u` brings it back
- [ ] **7.5** Press `f`, `F`, `t` → The chips at the lower left change and the cards narrow; after the last choice they return to All
- [x] **7.6** Shrink the window height → As many whole cards as fit show around the selected one; the rest hide behind a "⋯" line inside their own section, under its header ("⋯ 下に N 件", or "⋯ N 件" for a section with none shown); no card is folded to its title and no scrollbar
- [ ] **7.7** Press `o` → The item opens in the built-in browser at the window's right (Windows, macOS; Linux: the system's browser)
- [x] **7.8** Narrow the inbox column (drag the divider left, or zoom in with `Ctrl+=`) → The chips at the lower left wrap to more rows instead of running off the right edge; every one stays clickable and the cards above make room

## 8. Writing box and fields

- [ ] **8.1** Select an item and press `r` → The box under the detail (GitHub's "Add a comment") takes the cursor in place, about five lines high; no box covers the window
- [ ] **8.2** Type text, press `Ctrl+P`, then `Esc` → The preview shows headings, bullets, quotes and code as blocks; `Esc` returns to the field
- [ ] **8.3** Select a word and press `Ctrl+B`, `Ctrl+I`, `Ctrl+K` → `**word**`, `_word_` and `[word](url)` (with `url` selected)
- [ ] **8.4** Press `Alt+L`, type part of a label, `Down`, `Enter`, `Esc` → The label is ticked `[x]`; on `Esc` a toast says the labels were updated and GitHub shows the change
- [ ] **8.5** Press `Alt+A`, tick yourself, `Esc` → The assignee is set; the detail shows it after the refresh
- [ ] **8.5a** In the list, select an item that is in the Project, press `r`, then `Alt+S`, choose a Status with `Down`/`Enter`, `Esc` → A one-choice picker (`(o)`/`( )`) lists the Status options; after `Esc` a toast says so and the item's Status changes in the GitHub Project
- [ ] **8.5b** Press `Alt+P` the same way → The Priority options show and the choice is written to the Project
- [ ] **8.5c** Press `Alt+S` on an item that is not in the Project → A toast says the item is not in the Project; nothing is written
- [ ] **8.6a** In the list (and in the inbox) press `x` on an open issue or PR, then `Esc` → A box asks to close it; `Esc` cancels and nothing changes on GitHub
- [ ] **8.6b** Press `x`, then `Enter` → The item is closed on GitHub and a toast says so; after the next refresh it leaves the open list
- [ ] **8.6c** Press `m` in the list on a ready PR, `Enter` → The merge box shows first; after `Enter` the PR is merged. A draft or an issue shows a toast instead
- [ ] **8.6d** Press `x` on a merged item or a discussion → A toast says only an open issue or PR can be closed
- [ ] **8.6** Type a comment and press `Ctrl+Enter` → The box closes, a toast says it was sent, and the comment appears in the detail
- [ ] **8.7** On an open issue press `r`, type, `Ctrl+Shift+Enter` → The comment is sent and the issue is closed
- [ ] **8.8** On a Discussion press `r` and send → The comment appears in the Discussion
- [ ] **8.9** In the inbox, select a green PR and press `m`, then `Esc` → A confirmation shows the PR; `Esc` leaves it unmerged
- [ ] **8.10** Press `m`, then `Enter` on a PR you may merge → The PR is merged and the card leaves "Your turn" after the refresh
- [ ] **8.11** Press `a` on an issue → You are assigned; a toast says so
- [ ] **8.12** While the box is open, press `1`..`4` and letters → They are typed into the field; the views do not change
- [ ] **8.13** Press `r`, then click the Labels chip above the box; click two labels, then click outside the panel → The chip's border lights under the pointer; the click opens the labels picker as `Alt+L` does; each click ticks a row; the outside click closes it and a toast says the labels were updated
- [ ] **8.14** Click the Assignees chip, then without pressing `r` first click the Status chip of an item in the Project and click a Status → Assignees opens its picker (the box opens with it); the Status picker writes the choice at once on the click and closes; a toast says so
- [ ] **8.15** With a picker open, click Comment or Close in the box's footer → The picker's change is written first, then the comment is sent (or the item closed)
- [ ] **8.16** Select an item of a repository not looked at yet and at once press `r` then `Alt+A` → The picker may say loading for a moment, then the names fill in by themselves (no need to close and reopen it)
- [ ] **8.17** Select an item, wait a second, then press `r` and `Alt+A`; quit, start again and do the same → The names are there at once both times (read ahead on the selection, then kept in the cache)
- [x] **8.18** Zoom in (`Ctrl+=` twice) and look at the field chips above the writing box → A chip that does not fit is left out whole, none is cut at the right; "⚙ 出す項目は設定（,）で選ぶ" shows only when it fits

## 9. Actions

- [x] **9.1** Press `3` → The runs of your repositories appear, newest first, under a query box with the count; the selected run shows its mark (✓ ✗ ◐ ◷ ⊘), bold title, duration, a workflow chip, event · branch, then repository · #number · actor · age; the others show whole too, as many as fit
- [ ] **9.2** Press `h`, move with `j k`, press `Space` on "Failed" → Only failed runs stay; the query line and the count change; the filter shows counts
- [ ] **9.3** Select a run → The detail shows the repository tag, `▶ Run`, workflow and number and a status pill; the title; the meta line; for a failed run a "Next" box with `R` and how often it failed, for a running one `X`; the keys `e p D o`; the jobs with durations; the last 10 results as squares ("failed N times" or "all passed") and artifacts
- [ ] **9.4** Select a failed run and press `R` → A toast says the failed jobs are re-running; after a moment the run shows as running
- [ ] **9.5** Select a running run and press `X` → A toast says it is stopping; the run turns to cancelled
- [ ] **9.6** Press `D` on a run of a workflow with `workflow_dispatch` → A toast says the workflow started; a new run appears
- [ ] **9.7** Press `e` on a failed run → The built-in browser shows the failed job's page
- [ ] **9.8** Press `p` on a run of a pull request, then `o` → The PR opens; then the run opens
- [ ] **9.9** Press `R` on a passed run → A toast says only a failed run can be re-run; nothing is sent
- [ ] **9.10** Fill the list with many runs → No scrollbar; the rows that do not fit hide and "more" shows; none is folded
- [ ] **9.11** Select a failed run and click the `R` button in its "Next" box; on a running run click `X` → The pointer turns to a hand over the button; the click does what the key does (the toast and the run's change as in 9.4 and 9.5)
- [ ] **9.12** In the inbox, click the button of a "Next" box of a card of yours (`m`, `r`, `a` or `o`) → The click does what the key does
- [ ] **9.13** In the settings set "User or organization" to an owner not looked at yet (say `rust-lang`), press Enter and open Actions → The runs of its repositories fill in within about two seconds, several repositories at a time (not one by one over six or more seconds)

## 10. Board

- [x] **10.1** Press `4` → Project "uchmk" shows its Status options as columns with counts; each card shows repo, number, kind, title and labels; a "No status" column leads if some card has none
- [ ] **10.2** Move with `h l j k` → The column header and the card are highlighted; the cursor stays inside the columns
- [ ] **10.3** Press `L` on a card → The card moves to the next column at once, a toast says so, and the card's Status changes in the GitHub Project
- [ ] **10.4** Press `H` on a card in the first Status column → Nothing moves (no write)
- [ ] **10.5** Drag a card onto another column with the mouse → The card follows the pointer, the column under it is outlined, and on release the card moves and GitHub changes
- [ ] **10.6** Press `u` after a move → The card goes back and GitHub changes back
- [ ] **10.7** Press `Enter` on a card that is in the list, then on one that is not → The list opens on it; otherwise a toast says it is not in the list
- [ ] **10.8** Press `o` on a card → The built-in browser shows the issue or PR
- [ ] **10.9** Use a token without the `project` scope and press `L` → An error toast appears and, after a moment, the card returns to its column
- [ ] **10.10** Put many cards in one column → No scrollbar; the cards that do not fit hide and "more" shows; none is folded
- [x] **10.11** Look at a card with a long title → The title ends with "…" inside the card, not cut mid-glyph at its edge

## 11. Notifications and palette

- [x] **11.1** Press `n` with unread GitHub notifications → A list hangs from the bell over the view, where tsumugi's does: "Notifications N unread" on top; each row shows the unread dot, the repository tag, the number, an `Inbox` badge when the thread is a card there, the age, what happened in its colour (the inbox's reason, else why GitHub sent it) and the title; the rows that do not fit hide, none is folded to one line
- [ ] **11.2** Press `Enter` on a thread minato has, then on one of a merged PR (or a closed issue) → The list closes and the inbox (or the list) selects that thread; for the merged PR a toast says it is being got from GitHub, then the list selects it (marked merged, its body, comments and checks on the right) and the inbox has no card of it. Each time the row goes and the count drops by one
- [ ] **11.2a** Press `d` on a card in the inbox, then `Enter` on a notification of the same thread → The inbox opens on that card under "Done today" (the notification has no `Inbox` badge)
- [ ] **11.3** Press `d` on a thread → The row goes without opening anything; after a refresh it stays gone on GitHub
- [ ] **11.4** Press `A` → All rows go, a toast says so, and the bell shows none; GitHub shows them read
- [ ] **11.4a** Press `d` on two threads, wait two minutes, then quit minato and start it again → The two do not come back, neither after the wait nor after the start (only a thread with something new in it comes back)
- [ ] **11.4b** With a notification "CI workflow run failed for main branch", press `Enter` on it; then `o` on another such → `Enter` opens the inbox on that repository's "CI is red on main" card when there is one, else Actions with that run selected (the newest of that workflow on that branch; the filters cleared if they hid it); `o` shows the run's page in the built-in browser. When minato has neither, the repository's Actions page is shown, not its top
- [ ] **11.5** Press `n` again, `Esc`, or click outside the list → The list closes; clicking the bell opens and closes it
- [ ] **11.5a** Click a row, then "Mark all read" at the top → The row opens as with `Enter`; the button marks every row read
- [x] **11.6** Press `Ctrl+Shift+P` → A palette opens over the window with a search field; the title bar's field reads "Command palette" (Japanese: コマンドパレット)
- [ ] **11.7** Type `#` and part of a title, press `Enter` → The list opens on that item
- [ ] **11.8** Type `board`, press `Enter` → The Board view opens
- [ ] **11.9** Press `Esc`, and `Ctrl+Shift+P` twice → The palette closes; the second press closes it again

## 12. Dispatch form, job log and needs graph

- [ ] **12.1** In Actions, open a run of a workflow whose YAML has `needs:` → The jobs are drawn as boxes in columns by depth, joined by lines, coloured by result with the time under the name
- [ ] **12.2** Open a run of a workflow with no `needs:` or five or more levels → The jobs are listed instead, with `← needs` where it applies
- [ ] **12.3** Press `D` on a run whose workflow has `workflow_dispatch` inputs → A form lists the inputs with their descriptions, defaults and `*` for required
- [ ] **12.4** `Up`/`Down` to move, type into a text input, `Enter`/`Space` on a boolean or choice → Text is typed; booleans and choices change; `Tab` moves down
- [ ] **12.5** Press `Ctrl+Enter` with a required input empty → A toast says a value is needed; the form stays
- [ ] **12.6** Fill the inputs and press `Ctrl+Enter` → The form closes, a toast says the workflow started, and a new run with those inputs appears on GitHub
- [ ] **12.7** Press `D` on a workflow without inputs → The workflow starts at once, as before
- [ ] **12.8** Press `Esc` in the form → It closes and nothing starts
- [ ] **12.9** Press `L` on a failed run → A panel shows the failed job's log from a few lines before the first error, errors in red
- [ ] **12.10** `j`/`k`/`Space`/`PageUp`/`g`/`G` in the log, then `Esc` → The log scrolls with no scrollbar; `Esc` closes it

## 13. Milestone, reviewers and saved marks

- [ ] **13.1** In the list, press `r`, then `Alt+M`, pick a milestone, `Esc` → A toast says the milestone was updated; the issue or PR shows it on GitHub
- [ ] **13.2** On a PR press `Alt+R`, tick a person, `Esc` → A toast says the review was requested; GitHub lists the person under Reviewers
- [ ] **13.3** On an issue press `Alt+R` → A toast says reviewers can only be asked on a PR; no picker opens
- [ ] **13.4** In the inbox press `d` on a card and `s` on another, quit minato and start it again → Both cards are still hidden; `minato-marks.json` sits beside common.toml
- [ ] **13.5** Press `d`, then `u`, quit and start again → The card is back (the undo was saved too)
- [x] **13.6** In the inbox press `c` on a card, then paste into a Claude Code session (or Notepad) → A toast says a prompt for Claude was copied; the paste has the repository, the kind and number, the title and the URL, then the reason the card is there and what to do, in the app's language
- [x] **13.7** Look at "Waiting" in the inbox → No `Re-test: …` issue, no `Lane status: …` issue, no issue labelled `lane:win`/`lane:arm` or `needs-pc` and no lane PR (`test/win-*`, `test/arm-*`) is in it; your own other open issues and PRs still are

## 14. Settings key, palette field and bell

- [ ] **14.1** Look at the title bar → There is no `,` button; the account mark, the bell and the palette field remain
- [ ] **14.2** Press `Ctrl+,` → The settings screen opens; `Esc` returns
- [ ] **14.3** Click the palette field in the title bar → The command palette opens, the same as `Ctrl+Shift+P`
- [ ] **14.4** With no unread notification look at the bell, then with some → Dim cup without a badge; with unread, a gold badge with the count (`9+` past nine), the same look as tsumugi's bell
- [ ] **14.5** Click the bell → The bell stands right of the palette field; the notifications list opens under it, its right edge under the bell

## 15. The minato page of the settings

- [ ] **15.1** `Ctrl+,`, open the "minato" page → Two cards: the seven inbox rules and the six writing-box fields, each with a switch; Milestone, Priority, Reviewers and the dependabot rule are off
- [ ] **15.2** Switch the dependabot rule on and go to the inbox → A dependabot PR now appears with its reason
- [ ] **15.3** Switch Milestone on, press `r` then `Alt+M` on an item → The milestone picker opens; with it off, a toast says the field is hidden
- [ ] **15.4** Change a switch, quit and start again → The switches keep their state; `minato-settings.json` sits beside common.toml
- [ ] **15.5** Open the minato page; type `rust-lang` in "User or organization" of "Whose repositories" and press Enter → The first card shows the owner (hint: your login) and the board Project (`uchmk`); after Enter the inbox, the list, Actions and the board fill with rust-lang's repositories within a few seconds; the account circle gets an accent ring and its hover says "Showing: rust-lang"
- [ ] **15.6** Type `https://github.com/@rust-lang` in the owner box and press Enter → The box shows `rust-lang`
- [ ] **15.7** Type `Triage Queue` in "Board Project" and press Enter, then open the board → The board shows that Project; a title no Project has shows the owner's first Project
- [ ] **15.8** Clear the owner box and press Enter; quit and start again → Your own repositories come back with no old rust-lang cards or runs left; the ring goes; the owner and the Project are kept across the restart

## 16. Closed items

- [ ] **16.1** In the list view press `C` → A toast says closed items are shown; within a few seconds the list holds closed issues and PRs, no discussions
- [ ] **16.2** On a closed issue press `x` → No confirmation; the toast says it was reopened and GitHub shows it open
- [ ] **16.3** On a merged PR press `x` → Nothing is sent; a toast says it cannot be closed or reopened
- [ ] **16.4** Press `C` again → Open items return, discussions included

## 17. Field order and conflicts

- [ ] **17.1** In the "minato" page press ▼ on Labels, then open the writing box → The key line lists `Alt+A` before `Alt+L`; hidden fields are not listed
- [ ] **17.2** Restart minato → The order is kept
- [ ] **17.3** Have a green PR of yours with merge conflicts → It is not in the inbox as "ready to merge"

## 18. Mentions, red main CI and rule priority

- [ ] **18.1** Use a Project that has no Priority field and open the inbox → No "FieldConfiguration … Priority" toast
- [ ] **18.2** Have someone mention you on their open issue → The issue is in the inbox with "Mentions you"
- [ ] **18.3** Have a repository of yours whose main CI is red → A row "CI is red on main" is in the inbox; `o` opens its Actions page; `r`, `x`, `m` and `a` only show a toast
- [ ] **18.4** In the "minato" page press ▲ on a rule, then look at an item two rules fit → The reason is the upper rule; the order is kept after a restart
- [ ] **18.5** Start minato with the inbox view → Cards are listed at the left (title, repository, reason) and the detail at the right; the list is not blank

## 19. New issue

- [ ] **19.1** In the inbox or the list press `N` → The writing box opens titled "New issue" with the repository of the selected item
- [ ] **19.2** Type a title, a new line and a body, then `Ctrl+Enter` → A toast says the issue was created; it is on GitHub with that title and body and appears in the list soon
- [ ] **19.3** Press `N`, leave it empty, `Ctrl+Enter` → A toast says the title is empty and the box stays open

## 20. Repository settings

- [ ] **20.1** In the inbox or the list press `S` → A box titled with the repository shows description, visibility, default branch, topics, the three merge methods, delete-branch-after-merge, issues, discussions and archived
- [ ] **20.2** Press `Esc` → The box closes

## 21. More saved filters

- [ ] **21.1** In the list move to the filter column, choose "Mentions me" under saved → Only items that mention you stay, and the query over the list shows `mentions:@me`
- [ ] **21.2** Choose "Findings" under saved → Only items with the `finding` label stay, and the query shows `label:finding`

## 22. Discussion answer

- [ ] **22.1** Select a discussion in the list that has a comment from someone other than its author, wait for the detail, press `M` → A toast says it was marked; on GitHub that comment shows as the answer
- [ ] **22.2** Press `M` on an issue or a PR → A toast says only a discussion can have an answer

## 23. Saved searches

- [ ] **23.1** In the settings (`,`), minato page, type `is:open label:bug` in "Saved searches" → It stays after closing and reopening the settings and the app (`minato-searches.txt`)
- [ ] **23.2** In the list move to the filter column and choose that search under saved → The header shows `user:<you> is:open label:bug sort:updated-desc` and the list soon shows what GitHub finds
- [ ] **23.3** Choose "All" under saved, or switch to the inbox → The ordinary list comes back

## 24. Columns, mouse and arrows

- [ ] **24.1** In the inbox, list and Actions, drag the line between two columns → The pointer turns to a resize arrow over the line and the columns follow; nothing scrolls; each column keeps a usable width
- [ ] **24.2** Click a row in each column (filter, item, run, card) → The row is selected and that column gets the focus (a lit line on top); the detail follows
- [ ] **24.3** In each view press `←` `↓` `↑` `→` instead of `h j k l` → They do the same as `h j k l`, also in the notifications list and the filters
- [x] **24.4** Open the palette and look at the commands tsumugi also has → They read as in tsumugi: "Settings", "Notifications", "Keys: every key and what it does" (Japanese: 設定, 通知, キー: すべてのキーとその働き)
- [ ] **24.5** Compare the inbox, list, Actions and notifications with `docs/mock/minato.html` → The same layout: the inbox's tag chips, "Your turn" / "Waiting" / "Done today" with their hints, the detail's "Next" box and keys, the list's filter column with counts, the Actions run rows and detail
- [ ] **24.6** Select an item that is a stub for a red main CI (no thread) → The detail shows its title and the next step only; no "opened by" line, no "the item is gone" toast and no endless "Loading…"

## 25. Comment box, Markdown, kept widths and the cache

- [ ] **25.1** Select an issue and look under the detail → A box like GitHub's "Add a comment": Write / Preview tabs, a toolbar (H B I quote code link, lists, @ #), about five lines for the text, the hint at the left of the foot, "Close issue" and a green "Comment"
- [ ] **25.2** Click the text area, type two lines, then click another row → The box takes the cursor in place; after the click the box leaves and the text stays; back on the item, clicking the box shows the text again
- [ ] **25.3** Select a word and click B, I, the link and the list buttons → The word becomes `**word**`, `_word_`, `[word](url)`, and the line a list item; hovering a button shows its name
- [ ] **25.4** Click Preview, then Write → The text shows as Markdown (headings, bold, lists, quotes, code), then the field comes back with the text
- [ ] **25.5** With nothing written click "Comment", then "Close issue" → "Comment" is dim and does nothing; "Close issue" asks first, as `x` does
- [ ] **25.6** Type a comment and click "Close with comment" (the close button's name once there is text) → The comment is sent and the issue is closed
- [ ] **25.7** Look at an item whose body and comments use Markdown → Headings, bold, italic, code, lists, quotes and links show as such in the detail, not as raw marks
- [ ] **25.8** Drag a column line in the inbox, list and Actions, close minato and start it again → The columns come back at the widths they were left at (state.toml in minato's state folder)
- [ ] **25.9** Start minato while offline after a run with the connection up → The inbox, list, Actions and Board show the last answers at once; a selected item shows its last detail
- [ ] **25.10** Online, select an item, another, and the first again → The first detail shows at once and is not asked again unless the item changed on GitHub

## 26. HTML in Markdown, the wheel, the scale, the hover and the messages

- [ ] **26.1** Look at an item whose body has `<details><summary>…</summary>` and `<b>`, `<code>`, `<br>`, `<a href>` → The summary is a bar with a triangle pointing down and what the details hold under it; bold, code, the line break and the link show as Markdown does; no raw tags are left
- [ ] **26.2** Turn the wheel over the inbox cards, the list's filters and rows, the Actions runs, a board column and the notifications → The selection (or the filter cursor) moves one a notch, down for down; the column under the pointer takes the focus
- [ ] **26.3** Turn the wheel over a long detail in the inbox, the list and Actions → It scrolls about three lines a notch; in the inbox and the list it stops once its end shows, and at the top
- [ ] **26.4** Press Ctrl+=, Ctrl++ (with Shift), Ctrl+- and Ctrl+0, also on the settings screen, and pick "Zoom in" in the palette → The window grows or shrinks a step, a message says the size (as kura's), Ctrl+0 brings it back to 100%; kura and tsumugi take the same size from common.toml
- [ ] **26.5** Rest the pointer on an inbox card, a list row and a board card → A box with the first lines of the item's body in monospace shows, as tsumugi's peek of a session; none for an item without a body or while a card is dragged
- [ ] **26.6** Press `a` on an item, then do other things that leave a message (`d`, Ctrl+=) → Each message shows at the top right under the title bar, over the palette and the settings too, and goes after about four seconds

## 27. Larger details, the ends, F1 and the palette

- [ ] **27.1** Look at the detail of an item in the inbox and the list, and of a run in Actions → The title, the meta line, the next box, the checks, the body and the comments read a step larger than in 0.23.0; nothing overlaps
- [ ] **27.2** In an inbox with 30 cards and a short window, select one in the middle → The selected card and as many whole cards as fit around it show; the others hide behind "⋯" lines in their own sections, each saying how many of that section are hidden
- [ ] **27.3** Focus a long detail (`l` in the list, Actions' detail) and hold `j`, then press `k` once; turn the wheel down over it too → The scroll stops once the end shows; one `k` moves it up at once
- [ ] **27.4** Press `G`, then `gg`, in the inbox, the list's filters, rows and detail, Actions' columns, a board column and the notifications → `G` goes to the last (a detail to its end), `gg` to the first (a detail to its top); one `g` then another key does nothing odd
- [ ] **27.5** Press `F1` in each view, with the palette open and in the notifications → The key list opens (the palette closes); `F1` again closes it; the keybar ends with `F1`; the list has "gg G"
- [x] **27.6** Open the palette → There are no lines to go to Inbox, List, Actions or Board

## 28. Search, Tab and the board's speed

- [ ] **28.1** In the inbox press `/`, type `font kura` → A field opens in the keybar after a lit `/` chip (the `/` itself is not typed in); the cards narrow as you type to those with both words in the title, #number, repository, labels, author or body start; the keybar shows "N 件" (red at 0) and "Enter 決定 · Esc 消す"; the words are marked in the card titles and in the detail's body and comments
- [ ] **28.2** Type a Japanese word with the IME in the search field → The IME's candidate window shows by the field; the committed word filters and is marked
- [ ] **28.3** Search `preview`, then `Preview` → Lower case finds "Preview" and "preview"; once a capital is typed only "Preview" is found
- [x] **28.4** With the field open press `Down`/`Up`, then `Enter`, then `j k`, then `Esc` → The arrows move the selection; `Enter` closes the field but keeps the filter and the keybar line; `j k` move again; `Esc` clears the words and every card comes back
- [x] **28.5** Search in the inbox, press `2`, then `/` again → The list is filtered by the same words; `/` reopens the field with them, the cursor at the end
- [ ] **28.6** With the field open click a view tab for Actions, open the palette, press `n` (bell) by mouse → The field closes each time; in Actions the keybar shows its keys again
- [x] **28.7** Press `Tab` five times from the inbox, then `Shift+Tab` → Inbox → List → Actions → Board → Inbox → List; `Shift+Tab` goes back; nothing happens with the writing box, the palette, the notifications or the help open
- [ ] **28.8** Press `4` on a fresh start (no cache), then again after `1` and back → The board shows within a few seconds (it asks for the one Project); while it loads, `Enter` on an inbox card shows its detail at once
- [ ] **28.9** Select a run in Actions while the network is off for 15 seconds, then turn it on → Its jobs come within about 10 seconds of the network coming back, not after the next refresh
- [ ] **28.10** In Actions select a finished run, move away and back; quit minato, start it and select it again → The second time and after the restart its jobs, artifacts and graph show at once, with no "loading"
- [ ] **28.11** Select a running run and wait for two refreshes of the runs (about a minute) → Its jobs' marks and durations follow the run; when it finishes the detail shows the final jobs and artifacts
- [ ] **28.12** Hold `j` across many runs in Actions, then let go → The detail of the run you stop on comes within a few seconds, not after those of the runs passed
- [ ] **28.13** Press `4` on a Project with more than 100 cards → Every card shows (up to 500); the cards appear together, not 100 first and the rest later
- [ ] **28.14** Rest the pointer on a board card for the first time, then again → The first time the box with the body's start comes after a moment; the second time at once
- [ ] **28.15** Start minato, stay in the inbox for about 30 seconds, then press `3` and `4` → Actions shows its runs and the board its cards at once, with no "loading"
- [ ] **28.16** Stay in the inbox for a minute after a start, then in Actions select a finished run never selected before → Its jobs, artifacts and graph show at once; the runs next to it too
- [ ] **28.17** Quit minato and start it again; note the API counter in the status bar, wait two minutes in the inbox → The counter goes down by only a few (the unchanged runs cost nothing; the notifications one each 90 seconds)

## 29. The built-in browser

- [ ] **29.1** On Windows, press `o` on an inbox card → A panel at the window's right under the title bar, about 560 wide, with ← (address) ⟳ ↗ ✕ and the GitHub page below; the view narrows to its left; the keybar stays across the whole window
- [ ] **29.2** Press `o` on another card, then in the list, Actions (`o`, `e`, `p`) and the board → The same panel goes to each page; no second panel or window
- [ ] **29.3** Press `O` on a card → The system's browser opens the page; the panel does not change
- [ ] **29.4** Sign in to GitHub in the panel, quit minato, start it and press `o` → Still signed in (the cookies are in `%LOCALAPPDATA%\uchmk\minato\webview`, or the state folder of the OS); nothing new beside `minato.exe`
- [ ] **29.5** Follow a link in the page, then press ← → The address field shows each page's address as it loads; ← goes back; ↗ opens the page shown in the system's browser
- [ ] **29.6** Click a link that opens a new window (`target="_blank"`, e.g. a file in a PR's "Files changed" opened in a new tab) → It loads in the same panel; no other window appears
- [ ] **29.7** Click in the page, then click a card in the view and press `j` `k`; click in the address field and type → The keys move the selection again (not the page's); the field takes the typing
- [ ] **29.8** Drag the line left of the panel left and right; make the window narrow → The panel follows the pointer and the page with it; it stays at least 320 wide and leaves the view 300 while it can; the window's right edge still resizes the window
- [ ] **29.9** With the panel open press `Ctrl+Shift+P`, `n`, `?`, `,`, `N`, and `m` on a PR → The page hides while the palette, the notifications, the help, the settings, the new-issue box and the merge question are up, and comes back when each closes
- [ ] **29.10** Press `Esc` with nothing else open, then open the panel again and click ✕ → `Esc` closes the panel and the view takes the whole width; ✕ does the same
- [ ] **29.11** Make a message show (e.g. `d` then `u`) with the panel open → The message is at the top right of the view, left of the panel, not under it
- [ ] **29.12** On Linux press `o` → The system's browser opens the page; no panel
