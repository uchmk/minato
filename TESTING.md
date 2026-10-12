# minato: real-machine checks

Things a headless machine cannot see: the window, the title bar, the icon, the keys, the clock and the look.
`TESTING-CHECKS.md` is generated from the tables below (`cargo run -p minato --example make-testcheck`); this file is the source of truth.
`[x]` is set only by the owner or a real-machine session; `[~]` means judged from a screenshot.

Rows whose behaviour changes lose their mark before the list is regenerated.

## 1. Window and title bar

| # | Do | Expect |
| --- | --- | --- |
| 1.1 | Start `minato` | A 1280x800 window with no system title bar; the logo, `minato` (no 港), the tabs `1 Inbox` `2 List` `3 Actions` `4 Board`, a palette field in the middle, the bell, the account mark and three window buttons on the right |
| 1.2 | Drag the empty part of the title bar | The window moves; double-click maximises and restores |
| 1.3 | Click minimise, maximise, close | Each does what it says; close shows a red button under the pointer |
| 1.4 | Drag each edge and corner | The window resizes with the matching pointer; nothing resizes while maximised |
| 1.5 | Shrink the window | It stops at 720x420; nothing scrolls and no scrollbar appears anywhere |
| 1.6 | Look at the exe in Explorer and the taskbar | The same logo as in the title bar (tile, cyan and gold walls, blue wave, two lights) |

## 2. Views and keys

| # | Do | Expect |
| --- | --- | --- |
| 2.1 | Press `1`, `2`, `3`, `4` | The matching tab lights with an underline and the empty view's name and text change |
| 2.2 | Click each tab | Same as the digit |
| 2.3 | Press `?` | A card lists the global keys and the current view's keys; `?` or `Esc` closes it; digits do nothing while it is open |
| 2.4 | Press `,` or click the comma button | The settings screen opens; `Esc` closes it |
| 2.5 | Press `Ctrl+Shift+P` | A toast says it is not connected yet (the palette comes later) |
| 2.6 | Look at the keybar in each view | The keys of the view at the left, the first one lit, ending with `?`; no overflow at 720 px |

## 3. Keybar status and clock

| # | Do | Expect |
| --- | --- | --- |
| 3.1 | Look at the right of the keybar | `⟳ —`, `API —` and the clock `2026/10/12 (Mon) 17:40`-style text; the minute changes on its own |
| 3.2 | Switch the language to Japanese in the settings | Tabs, empty texts, help and the weekday (月) change; Japanese glyphs show (a system font loads in the background) |
| 3.3 | Turn the clock off in the settings | The clock disappears from the keybar |

## 4. Settings

| # | Do | Expect |
| --- | --- | --- |
| 4.1 | Open the settings | The common screen of the uchmk apps: General (language, size, clock) and Theme pages, a search, `Ctrl+Tab` moves between pages |
| 4.2 | Change the scale to 150% | The whole window is drawn larger at once and stays so after a restart |
| 4.3 | Pick another theme | The window's colours change at once; the sample card follows the theme |
| 4.4 | Edit `%APPDATA%\uchmk\common.toml` by hand while the app runs | The change shows within a few seconds |

## 5. GitHub connection

| # | Do | Expect |
| --- | --- | --- |
| 5.1 | Start `minato` where `gh auth login` is done | Within a few seconds the account mark shows your initial (hover: your login) and the keybar shows `API 4,xxx/5,000` and `⟳ 0s ago` |
| 5.2 | Start it with no `gh` login and no `GITHUB_TOKEN` | A toast says there is no token and how to get one; the account mark stays `?` |
| 5.3 | Start it with `GITHUB_TOKEN=bad` | A toast says GitHub refused the token (401); the token text appears nowhere |
| 5.4 | Leave it open for 3 minutes | The `⟳` age counts up and drops back to a few seconds after each poll; the window stays idle otherwise |

## 6. List view

| # | Do | Expect |
| --- | --- | --- |
| 6.1 | Press `2` after the connection is up | Within a few seconds your open issues, PRs and discussions list in three lines each; the CI mark and the comment count stand at the right |
| 6.2 | Press `j` / `k` | The selected row moves; the detail at the right follows within a second |
| 6.3 | Shrink the window height until the rows do not fit | Far rows fold to one line, then hide with "⋯ 上に/下に N 件"; no scrollbar appears |
| 6.4 | Press `h`, then `j` and `Space` on a repository | The list narrows to it and the query line over the list changes; the chosen option is filled |
| 6.5 | Pick "CI is red" | Only PRs with a red mark remain |
| 6.6 | Press `l` twice and `j` | The detail scrolls by lines, with no scrollbar; moving the row resets it |
| 6.7 | Press `o` | The item opens in the browser |
| 6.8 | Look at a PR with checks | The detail lists each check with ✓ ✗ ◐ in its colour, then the conversation |

## 7. Inbox view

| # | Do | Expect |
| --- | --- | --- |
| 7.1 | Press `1` after the connection is up | Cards of your turn (red or ready PRs, `question` / `needs-owner` / `lane-stalled` items, unassigned findings) stand under "Your turn"; each shows its reason in colour |
| 7.2 | Select a card with `j` / `k` | The detail at the right follows; its top line shows the reason and the next key |
| 7.3 | Press `d`, then `u` | The card moves to "Done today" with a toast, and `u` brings it back |
| 7.4 | Press `s` | The card disappears (back tomorrow 9:00) with a toast; `u` brings it back |
| 7.5 | Press `f`, `F`, `t` | The chips at the lower left change and the cards narrow; after the last choice they return to All |
| 7.6 | Shrink the window height | Far cards fold to their title, then hide with "⋯" lines; no scrollbar |
| 7.7 | Press `o` | The item opens in the browser |

## 8. Writing box and fields

| # | Do | Expect |
| --- | --- | --- |
| 8.1 | Select an item and press `r` | A box opens at the bottom with the item's name on top and the key hints below; the cursor is in the field |
| 8.2 | Type text, press `Ctrl+P`, then `Esc` | The preview shows headings, bullets, quotes and code as blocks; `Esc` returns to the field |
| 8.3 | Select a word and press `Ctrl+B`, `Ctrl+I`, `Ctrl+K` | `**word**`, `_word_` and `[word](url)` (with `url` selected) |
| 8.4 | Press `Alt+L`, type part of a label, `Down`, `Enter`, `Esc` | The label is ticked `[x]`; on `Esc` a toast says the labels were updated and GitHub shows the change |
| 8.5 | Press `Alt+A`, tick yourself, `Esc` | The assignee is set; the detail shows it after the refresh |
| 8.6 | Type a comment and press `Ctrl+Enter` | The box closes, a toast says it was sent, and the comment appears in the detail |
| 8.7 | On an open issue press `r`, type, `Ctrl+Shift+Enter` | The comment is sent and the issue is closed |
| 8.8 | On a Discussion press `r` and send | The comment appears in the Discussion |
| 8.9 | In the inbox, select a green PR and press `m`, then `Esc` | A confirmation shows the PR; `Esc` leaves it unmerged |
| 8.10 | Press `m`, then `Enter` on a PR you may merge | The PR is merged and the card leaves "Your turn" after the refresh |
| 8.11 | Press `a` on an issue | You are assigned; a toast says so |
| 8.12 | While the box is open, press `1`..`4` and letters | They are typed into the field; the views do not change |

## 9. Actions

| # | Do | Expect |
| --- | --- | --- |
| 9.1 | Press `3` | The runs of your repositories appear, newest first, each with a mark (✓ ✗ ◐ ◷ ⊘), its workflow, event, branch, repository, number, actor and age |
| 9.2 | Press `h`, move with `j k`, press `Space` on "Failed" | Only failed runs stay; the query line and the count change; the filter shows counts |
| 9.3 | Select a run | The detail shows the status, the title, the meta line, the next step, the jobs with durations, the last 10 results and artifacts |
| 9.4 | Select a failed run and press `R` | A toast says the failed jobs are re-running; after a moment the run shows as running |
| 9.5 | Select a running run and press `X` | A toast says it is stopping; the run turns to cancelled |
| 9.6 | Press `D` on a run of a workflow with `workflow_dispatch` | A toast says the workflow started; a new run appears |
| 9.7 | Press `e` on a failed run | The browser opens the failed job's page |
| 9.8 | Press `p` on a run of a pull request, then `o` | The PR opens; then the run opens |
| 9.9 | Press `R` on a passed run | A toast says only a failed run can be re-run; nothing is sent |
| 9.10 | Fill the list with many runs | No scrollbar; far rows fold and "more" shows |
