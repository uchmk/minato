# TODO

`【人】` is the owner's work; `（要確認: #N）` waits for an answer (go on with the recommendation).

## Next stages (docs/dev-prompt.md)

- [x] 2. GitHub client in `minato-core` (v0.2.0)
- [ ] 2b. Token entry in the settings screen and the OS credential store (今は `GITHUB_TOKEN` と `gh auth token` だけ）（要確認: #3）
- [x] 3. Inbox view (v0.4.0; rules judged from the list data; marks are not saved to disk yet; m a r R e g wait for stage 5 and 6)（要確認: #5）
- [x] 4. List view (three columns) (v0.3.0; read only)
- [x] 5. Writing box and fields (v0.5.0: comment, labels, assignees, merge, assign self)
- [ ] 5b. Pickers for Status / Priority / Milestone / Reviewers (the `updateProjectV2ItemFieldValue` builder exists), and showing / ordering the fields in the settings screen
- [x] 5c. Actions view (v0.6.0: runs, filters, jobs, last 10, artifacts, R X D e p o; `D` sends no inputs; logs open in the browser)
- [ ] 5d. Dispatch inputs from the workflow YAML, logs inside the app, a `needs:` graph of the jobs
- [ ] 6. Board view, palette (Ctrl+Shift+P)
- [ ] 7. Writes (comment, close, label, merge) with confirmation
- [ ] 8. Notifications, release workflow

## Owner

- 【人】 Run the rows of TESTING-CHECKS.md on the real machine (window, title bar, icon, fonts); see the `needs-pc` issue for v0.1.0.
