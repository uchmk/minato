# TODO

`【人】` is the owner's work; `（要確認: #N）` waits for an answer (go on with the recommendation).

## Next stages (docs/dev-prompt.md)

- [x] 2. GitHub client in `minato-core` (v0.2.0)
- [-] 2b. Token entry in the settings screen and the OS credential store: skipped on purpose, `GITHUB_TOKEN` and `gh auth token` cover the owner (#3, answer A)
- [x] 3. Inbox view (v0.4.0; rules judged from the list data; marks are saved to `minato-marks.json` since v0.10.0; m a r R e g wait for stage 5 and 6)（要確認: #5）
- [x] 4. List view (three columns) (v0.3.0; read only)
- [x] 5. Writing box and fields (v0.5.0: comment, labels, assignees, merge, assign self)
- [x] 5b. Pickers for Status / Priority (v0.9.0: `Alt+S` / `Alt+P`) and Milestone / Reviewers (v0.10.0: `Alt+M` / `Alt+R`; reviewers only add requests). v0.12.0: the settings screen has a "minato" page that shows or hides the fields and the inbox rules ; v0.14.0: the order of the fields can be changed there too
- [x] 5c. Actions view (v0.6.0: runs, filters, jobs, last 10, artifacts, R X D e p o; `D` sends no inputs; logs open in the browser)
- [x] 5d. Dispatch inputs from the workflow YAML, logs inside the app, a `needs:` graph of the jobs (v0.9.0: `D` form, `L` log, graph in the run detail)
- [x] 6. Board view (v0.7.0: Project uchmk Status columns, H L and drag write to GitHub, u undo)
- [x] 6b. Palette (v0.8.0: Ctrl+Shift+P, commands and `#number` items)
- [x] 7. Writes (comment, close, label, merge) with confirmation (v0.9.1: `x` close and `m` merge ask first, in the list and the inbox; comment and labels were v0.5.0)
- [x] 11. Closed items in the list (v0.13.0: `C` toggles, `x` opens one again; discussions are not shown while it is on)
- [x] 12. New issue (v0.16.0: `N` in the inbox and the list)
- [x] 13. Repository settings, read only (v0.17.0: `S` in the inbox and the list)
- [x] 14. Saved filters for mentions and findings (v0.18.0)
- [x] 8. Notifications panel (v0.8.0: `n`, mark read, `A` all), release workflow (v0.8.0; run by hand with the tag)

## Owner

- 【人】 Run the rows of TESTING-CHECKS.md on the real machine (window, title bar, icon, fonts); see the `needs-pc` issue for v0.1.0.
