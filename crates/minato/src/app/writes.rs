//! The things that change GitHub (compose, comment, merge, close, re-run) and their pickers.

use super::*;

impl App {
    /// `r`: open the writing box on the selected item with the repository's
    /// labels and people known so far, and ask for them unless just asked.
    pub fn open_compose(&mut self) {
        let Some(item) = self.writable() else { return };
        if self.compose.as_ref().is_some_and(|c| c.id == item.id) {
            return;
        }
        self.close_compose();
        for (key, call) in self.ask_lists(&item.repo) {
            self.queue(&key, call, None);
        }
        let text = self.drafts.remove(&item.id).unwrap_or_default();
        let Lists { labels, people, milestones, came } = self.lists.get(&item.repo).cloned().unwrap_or_default();
        self.compose = Some(Compose { id: item.id, text, preview: false, picker: None, labels, people, milestones, came, tool: None });
    }

    /// The jobs that ask for `repo`'s labels, people and milestones, unless
    /// they were asked in the last [`LISTS_AGE`] (they are then marked asked).
    pub fn ask_lists(&mut self, repo: &str) -> Vec<(String, Call)> {
        let now = Instant::now();
        if repo.is_empty() || self.lists_asked.get(repo).is_some_and(|t| now.duration_since(*t) < LISTS_AGE) {
            return Vec::new();
        }
        self.lists_asked.insert(repo.to_string(), now);
        vec![
            (format!("labels:{repo}"), Call::Get(write::labels_path(repo))),
            (format!("people:{repo}"), Call::Get(write::assignees_path(repo))),
            (format!("miles:{repo}"), Call::Get(write::milestones_path(repo))),
        ]
    }

    /// A list of `repo`'s did not come: the next look asks again.
    pub fn lists_failed(&mut self, repo: &str) {
        self.lists_asked.remove(repo);
    }

    /// Whether the writing box is open on an item of `repo`.
    pub fn composing_in(&self, repo: &str) -> bool {
        self.compose.as_ref().and_then(|c| self.items.iter().find(|i| i.id == c.id)).is_some_and(|i| i.repo == repo)
    }

    /// One of `repo`'s lists came (`set` puts it in): kept, and given to the
    /// box open on the repository and to its picker, which may have opened
    /// while the list was on its way.
    pub fn got_lists(&mut self, repo: &str, set: impl FnOnce(&mut Lists)) {
        let lists = self.lists.entry(repo.to_string()).or_default();
        set(lists);
        let Lists { labels, people, milestones, came } = lists.clone();
        if !self.composing_in(repo) {
            return;
        }
        let Some(c) = self.compose.as_mut() else { return };
        (c.labels, c.people, c.milestones, c.came) = (labels, people, milestones, came);
        if let Some((kind, pk)) = c.picker.as_mut() {
            pk.options = match kind {
                Pick::Labels => c.labels.clone(),
                Pick::Assignees | Pick::Reviewers => c.people.clone(),
                Pick::Milestone => c.milestones.iter().map(|(_, t)| t.clone()).collect(),
                _ => return,
            };
            pk.cursor = pk.cursor.min(pk.shown().len().saturating_sub(1));
        }
    }

    /// Close the box, keeping what was written on an item as its draft (a
    /// new issue's text is not kept). An open picker writes what changed.
    pub fn close_compose(&mut self) {
        if self.compose.as_ref().is_some_and(|c| c.picker.is_some()) {
            self.apply_pick();
        }
        let Some(c) = self.compose.take() else { return };
        if !c.id.starts_with("new:") && !c.text.trim().is_empty() {
            self.drafts.insert(c.id, c.text);
        }
    }

    /// `S`: show the settings of the selected item's repository (read only).
    pub(super) fn repo_settings(&mut self) {
        let Some(repo) = self.selected().or(self.items.first()).map(|i| i.repo.clone()) else { return };
        self.queue(&format!("a-repo:{repo}"), Call::Get(minato_core::reposettings::path(&repo)), None);
        self.overlay = Some(Overlay::Log(LogView::new(0, repo)));
    }

    /// `N`: open the writing box for a new issue in the selected item's
    /// repository (the first one listed when nothing is selected).
    pub(super) fn new_issue(&mut self) {
        let Some(repo) = self.selected().or(self.items.first()).map(|i| i.repo.clone()) else { return };
        self.close_compose();
        self.compose = Some(Compose {
            id: format!("new:{repo}"),
            text: String::new(),
            preview: false,
            picker: None,
            labels: Vec::new(),
            people: Vec::new(),
            milestones: Vec::new(),
            came: [false; 3],
            tool: None,
        });
    }

    /// Send the comment; `close` also closes the item.
    pub fn send_comment(&mut self, close: bool) {
        let Some(c) = self.compose.take() else { return };
        if let Some(repo) = c.id.strip_prefix("new:") {
            match write::create_issue(repo, &c.text) {
                Some(call) => {
                    let n = self.outbox.len();
                    self.queue(&format!("write:{n}:new"), call, Some("write.created"));
                }
                None => {
                    self.compose = Some(c);
                    self.say_key("write.no_title");
                }
            }
            return;
        }
        let Some(item) = self.item(&c.id) else { return };
        // Comment with nothing written: the box stays as it is.
        if !close && c.text.trim().is_empty() {
            self.compose = Some(c);
            return;
        }
        let n = self.outbox.len();
        if !c.text.trim().is_empty() {
            // Auto-done: a comment on an item that is in the inbox now settles it (a local mark).
            if self.settings.auto_done && self.inbox_all().iter().any(|k| self.items[k.at].id == item.id && k.group != Group::Done) {
                self.inbox.mark(&mut self.marks, &item.id, Mark::Done(now_epoch()));
                self.save_marks();
            }
            self.queue(&format!("write:{n}:comment"), write::comment(&item, &c.text), Some("write.commented"));
        }
        if close && item.kind != Kind::Discussion {
            self.queue(&format!("write:{n}:state"), write::set_state(&item, false), Some("write.closed"));
        }
    }

    /// The picker was closed: write what changed.
    pub fn apply_pick(&mut self) {
        let Some((kind, picker)) = self.compose.as_mut().and_then(|c| c.picker.take()) else { return };
        if !picker.changed() {
            return;
        }
        let Some(item) = self.compose.as_ref().and_then(|c| self.item(&c.id)) else { return };
        let n = self.outbox.len();
        match kind {
            Pick::Labels => self.queue(&format!("write:{n}:labels"), write::set_labels(&item, &picker.chosen), Some("write.labels")),
            Pick::Assignees => {
                self.queue(&format!("write:{n}:people"), write::set_assignees(&item, &picker.chosen), Some("write.assignees"))
            }
            Pick::Milestone => {
                let number =
                    picker.chosen.first().and_then(|t| self.compose.as_ref()?.milestones.iter().find(|(_, m)| m == t).map(|(n, _)| *n));
                self.queue(&format!("write:{n}:milestone"), write::set_milestone(&item, number), Some("write.milestone"));
            }
            Pick::Reviewers => {
                // The ones ticked are asked; the ones unticked, taken off.
                let (added, removed) = (picker.added(), picker.removed());
                if !added.is_empty() {
                    self.queue(&format!("write:{n}:reviewers"), write::request_reviewers(&item, &added), Some("write.reviewers"));
                }
                if !removed.is_empty() {
                    self.queue(&format!("write:{n}:unreview"), write::remove_reviewers(&item, &removed), Some("write.unreviewed"));
                }
            }
            Pick::Status | Pick::Priority => {
                let priority = kind == Pick::Priority;
                let (Some(name), Some(p)) = (picker.chosen.first(), self.board.as_mut()) else { return };
                let Some(card) = p.card_of(&item.id) else { return };
                if let Some(call) = p.set_by_name(card, priority, name) {
                    let key = format!("board:{}:{n}", p.cards[card].item);
                    self.queue(&key, call, Some("write.field"));
                }
            }
        }
    }

    /// The picker of field `k` (0 Labels .. 5 Reviewers), from Alt+L and the
    /// rest or a click on its chip; one open before writes what changed.
    pub fn open_pick(&mut self, k: usize) {
        self.apply_pick();
        match k {
            0 | 1 => {
                let Some(c) = self.compose.as_ref() else { return };
                let item = self.item(&c.id);
                let Some(c) = self.compose.as_mut() else { return };
                c.picker = Some(if k == 0 {
                    (
                        Pick::Labels,
                        Picker::new(c.labels.clone(), item.map(|i| i.labels.iter().map(|l| l.name.clone()).collect()).unwrap_or_default()),
                    )
                } else {
                    (Pick::Assignees, Picker::new(c.people.clone(), item.map(|i| i.assignees).unwrap_or_default()))
                });
            }
            2 | 4 => self.open_field_picker(k == 4),
            _ => self.open_extra_picker(k == 5),
        }
    }

    /// Alt+M (milestone, one choice) or Alt+R (reviewers, PRs only).
    pub fn open_extra_picker(&mut self, reviewers: bool) {
        let Some(item) = self.compose.as_ref().and_then(|c| self.item(&c.id)) else { return };
        if reviewers && item.kind != Kind::Pr {
            self.say_key("write.not_pr");
            return;
        }
        let Some(c) = self.compose.as_mut() else { return };
        c.picker = Some(if reviewers {
            (Pick::Reviewers, Picker::new(c.people.clone(), item.reviewers))
        } else {
            let now = [item.milestone].into_iter().filter(|m| !m.is_empty()).collect();
            (Pick::Milestone, Picker::single(c.milestones.iter().map(|(_, t)| t.clone()).collect(), now))
        });
    }

    /// Alt+S / Alt+P in the writing box: the picker for the item's Project
    /// Status or Priority. Says so when the item is not in the Project.
    pub fn open_field_picker(&mut self, priority: bool) {
        let Some(id) = self.compose.as_ref().map(|c| c.id.clone()) else { return };
        let found = self.board.as_ref().and_then(|p| p.card_of(&id).map(|k| p.choices(k, priority)));
        match found {
            Some((names, chosen)) if !names.is_empty() => {
                let kind = if priority { Pick::Priority } else { Pick::Status };
                if let Some(c) = self.compose.as_mut() {
                    c.picker = Some((kind, Picker::single(names, chosen)));
                }
            }
            _ => {
                let words = self.tr("write.no_field");
                self.say(words);
            }
        }
    }

    /// `m` in the inbox: ask before merging an open PR.
    pub(super) fn ask_merge(&mut self) {
        match self.selected() {
            Some(i) if i.kind == Kind::Pr && !i.merged && !i.draft && !minato_core::mainci::is_stub(i) => {
                self.confirm = Some(i.id.clone());
                self.confirm_act = Confirm::Merge;
            }
            Some(_) => {
                let words = self.tr("write.not_mergeable");
                self.say(words);
            }
            None => {}
        }
    }

    /// `x`: ask before closing the selected issue or PR.
    pub fn ask_close(&mut self) {
        match self.selected() {
            Some(i) if i.kind != Kind::Discussion && !i.merged && !minato_core::mainci::is_stub(i) => {
                self.confirm = Some(i.id.clone());
                self.confirm_act = Confirm::Close;
            }
            Some(_) => self.say_key("write.not_closable"),
            None => {}
        }
    }

    /// `x` on a closed item: ask before opening it again.
    pub(super) fn ask_reopen(&mut self) {
        match self.selected() {
            Some(i) if i.kind != Kind::Discussion && !i.merged && !minato_core::mainci::is_stub(i) => {
                self.confirm = Some(i.id.clone());
                self.confirm_act = Confirm::Reopen;
            }
            Some(_) => self.say_key("write.not_closable"),
            None => {}
        }
    }

    pub(super) fn reopen(&mut self, id: &str) {
        if let Some(item) = self.item(id) {
            let n = self.outbox.len();
            self.queue(&format!("write:{n}:state"), write::set_state(&item, true), Some("write.reopened"));
        }
    }

    pub(super) fn close(&mut self, id: &str) {
        if let Some(item) = self.item(id) {
            let n = self.outbox.len();
            self.queue(&format!("write:{n}:state"), write::set_state(&item, false), Some("write.closed"));
        }
    }

    pub(super) fn merge(&mut self, id: &str) {
        if let Some(item) = self.item(id) {
            let n = self.outbox.len();
            self.queue(&format!("write:{n}:merge"), write::merge(&item), Some("write.merged"));
        }
    }

    /// `R` in the inbox: run the failed jobs of the selected PR's CI again.
    /// Its failed runs come from the Actions data (same repository, the PR's
    /// number or branch), the newest of each workflow, the mended ones left out.
    pub(super) fn rerun_ci(&mut self) {
        let all = self.inbox_all();
        let Some(card) = self.inbox.selected(&all, &self.items) else { return };
        let item = &self.items[card.at];
        if item.kind != Kind::Pr {
            return self.say_key("run.not_pr");
        }
        let mended = actions::mended(&self.runs);
        let mut newest: std::collections::HashMap<u64, &Run> = std::collections::HashMap::new();
        for r in self.runs.iter().filter(|r| {
            r.status == actions::Status::Bad
                && !mended.contains(&r.id)
                && r.repo == item.repo
                && (r.pr == Some(item.number) || (!item.head.is_empty() && r.branch == item.head))
        }) {
            let e = newest.entry(r.workflow_id).or_insert(r);
            if r.id > e.id {
                *e = r;
            }
        }
        let mut runs: Vec<Run> = newest.into_values().cloned().collect();
        runs.sort_by_key(|r| r.id);
        if runs.is_empty() {
            return self.say_key("run.none_failed");
        }
        for run in runs {
            self.queue(&format!("write:{}:rerun", run.id), actions::rerun_failed(&run), Some("run.reran"));
        }
    }

    /// `a`: assign the selected item to the viewer.
    pub(super) fn assign_self(&mut self) {
        let Some(item) = self.writable() else { return };
        let Some(me) = self.viewer.clone() else { return };
        if item.kind == Kind::Discussion {
            return;
        }
        let n = self.outbox.len();
        self.queue(&format!("write:{n}:assign"), write::assign(&item, &me), Some("write.assigned"));
    }
}
