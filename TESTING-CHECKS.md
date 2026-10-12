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

## 5. GitHub connection

- [ ] **5.1** Start `minato` where `gh auth login` is done → Within a few seconds the account mark shows your initial (hover: your login) and the keybar shows `API 4,xxx/5,000` and `⟳ 0s ago`
- [ ] **5.2** Start it with no `gh` login and no `GITHUB_TOKEN` → A toast says there is no token and how to get one; the account mark stays `?`
- [ ] **5.3** Start it with `GITHUB_TOKEN=bad` → A toast says GitHub refused the token (401); the token text appears nowhere
- [ ] **5.4** Leave it open for 3 minutes → The `⟳` age counts up and drops back to a few seconds after each poll; the window stays idle otherwise

## 6. List view

- [ ] **6.1** Press `2` after the connection is up → Within a few seconds your open issues, PRs and discussions list in three lines each; the CI mark and the comment count stand at the right
- [ ] **6.2** Press `j` / `k` → The selected row moves; the detail at the right follows within a second
- [ ] **6.3** Shrink the window height until the rows do not fit → Far rows fold to one line, then hide with "⋯ 上に/下に N 件"; no scrollbar appears
- [ ] **6.4** Press `h`, then `j` and `Space` on a repository → The list narrows to it and the query line over the list changes; the chosen option is filled
- [ ] **6.5** Pick "CI is red" → Only PRs with a red mark remain
- [ ] **6.6** Press `l` twice and `j` → The detail scrolls by lines, with no scrollbar; moving the row resets it
- [ ] **6.7** Press `o` → The item opens in the browser
- [ ] **6.8** Look at a PR with checks → The detail lists each check with ✓ ✗ ◐ in its colour, then the conversation

## 7. Inbox view

- [ ] **7.1** Press `1` after the connection is up → Cards of your turn (red or ready PRs, `question` / `needs-owner` / `lane-stalled` items, unassigned findings) stand under "Your turn"; each shows its reason in colour
- [ ] **7.2** Select a card with `j` / `k` → The detail at the right follows; its top line shows the reason and the next key
- [ ] **7.3** Press `d`, then `u` → The card moves to "Done today" with a toast, and `u` brings it back
- [ ] **7.4** Press `s` → The card disappears (back tomorrow 9:00) with a toast; `u` brings it back
- [ ] **7.5** Press `f`, `F`, `t` → The chips at the lower left change and the cards narrow; after the last choice they return to All
- [ ] **7.6** Shrink the window height → Far cards fold to their title, then hide with "⋯" lines; no scrollbar
- [ ] **7.7** Press `o` → The item opens in the browser

## 8. Writing box and fields

- [ ] **8.1** Select an item and press `r` → A box opens at the bottom with the item's name on top and the key hints below; the cursor is in the field
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

## 9. Actions

- [ ] **9.1** Press `3` → The runs of your repositories appear, newest first, each with a mark (✓ ✗ ◐ ◷ ⊘), its workflow, event, branch, repository, number, actor and age
- [ ] **9.2** Press `h`, move with `j k`, press `Space` on "Failed" → Only failed runs stay; the query line and the count change; the filter shows counts
- [ ] **9.3** Select a run → The detail shows the status, the title, the meta line, the next step, the jobs with durations, the last 10 results and artifacts
- [ ] **9.4** Select a failed run and press `R` → A toast says the failed jobs are re-running; after a moment the run shows as running
- [ ] **9.5** Select a running run and press `X` → A toast says it is stopping; the run turns to cancelled
- [ ] **9.6** Press `D` on a run of a workflow with `workflow_dispatch` → A toast says the workflow started; a new run appears
- [ ] **9.7** Press `e` on a failed run → The browser opens the failed job's page
- [ ] **9.8** Press `p` on a run of a pull request, then `o` → The PR opens; then the run opens
- [ ] **9.9** Press `R` on a passed run → A toast says only a failed run can be re-run; nothing is sent
- [ ] **9.10** Fill the list with many runs → No scrollbar; far rows fold and "more" shows

## 10. Board

- [ ] **10.1** Press `4` → Project "uchmk" shows its Status options as columns with counts; each card shows repo, number, kind, title and labels; a "No status" column leads if some card has none
- [ ] **10.2** Move with `h l j k` → The column header and the card are highlighted; the cursor stays inside the columns
- [ ] **10.3** Press `L` on a card → The card moves to the next column at once, a toast says so, and the card's Status changes in the GitHub Project
- [ ] **10.4** Press `H` on a card in the first Status column → Nothing moves (no write)
- [ ] **10.5** Drag a card onto another column with the mouse → The card follows the pointer, the column under it is outlined, and on release the card moves and GitHub changes
- [ ] **10.6** Press `u` after a move → The card goes back and GitHub changes back
- [ ] **10.7** Press `Enter` on a card that is in the list, then on one that is not → The list opens on it; otherwise a toast says it is not in the list
- [ ] **10.8** Press `o` on a card → The browser opens the issue or PR
- [ ] **10.9** Use a token without the `project` scope and press `L` → An error toast appears and, after a moment, the card returns to its column
- [ ] **10.10** Put many cards in one column → No scrollbar; far cards fold and "more" shows

## 11. Notifications and palette

- [ ] **11.1** Press `n` with unread GitHub notifications → Unread threads list with repo, kind, reason, age and title; the bell count matches
- [ ] **11.2** Press `Enter` on a thread → The browser opens its issue, PR or commit; the row goes and the count drops by one
- [ ] **11.3** Press `d` on a thread → The row goes without opening anything; after a refresh it stays gone on GitHub
- [ ] **11.4** Press `A` → All rows go, a toast says so, and the bell shows none; GitHub shows them read
- [ ] **11.5** Press `n` again → The view comes back
- [ ] **11.6** Press `Ctrl+Shift+P` → A palette opens over the window with a search field
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

## 14. Settings key, palette field and bell

- [ ] **14.1** Look at the title bar → There is no `,` button; the account mark, the bell and the palette field remain
- [ ] **14.2** Press `Ctrl+,` → The settings screen opens; `Esc` returns
- [ ] **14.3** Click the palette field in the title bar → The command palette opens, the same as `Ctrl+Shift+P`
- [ ] **14.4** With no unread notification look at the bell, then with some → Dim cup without a badge; with unread, a gold badge with the count (`9+` past nine), the same look as tsumugi's bell
- [ ] **14.5** Click the bell → The notifications panel opens

## 15. The minato page of the settings

- [ ] **15.1** `Ctrl+,`, open the "minato" page → Two cards: the seven inbox rules and the six writing-box fields, each with a switch; Milestone, Priority, Reviewers and the dependabot rule are off
- [ ] **15.2** Switch the dependabot rule on and go to the inbox → A dependabot PR now appears with its reason
- [ ] **15.3** Switch Milestone on, press `r` then `Alt+M` on an item → The milestone picker opens; with it off, a toast says the field is hidden
- [ ] **15.4** Change a switch, quit and start again → The switches keep their state; `minato-settings.json` sits beside common.toml

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
