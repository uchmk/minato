//! The Actions, notes and find sides of the app: runs, the log, the notes and the find bar.

use super::*;

impl App {
    /// The workflow file of the run, once read.
    pub fn workflow_of(&self, run: &Run) -> Option<&Workflow> {
        self.run_detail.as_ref().filter(|d| d.id == run.id).and_then(|d| d.workflow.as_ref())
    }

    /// `L`: the log of the failed job (else the last one), in the app.
    pub(super) fn open_log(&mut self, run: &Run) {
        let jobs = self.run_detail.as_ref().filter(|d| d.id == run.id).and_then(|d| d.jobs.as_ref());
        let Some(job) = jobs.and_then(|j| j.iter().find(|j| j.status == actions::Status::Bad).or(j.last())).cloned() else {
            return self.say_key("run.no_jobs");
        };
        self.queue(&format!("a-log:{}", job.id), Call::Get(workflow::log_path(run, &job)), None);
        self.overlay = Some(Overlay::Log(LogView::new(job.id, job.name)));
    }

    /// `y`: the page's link on the clipboard.
    pub(super) fn copy_link(&mut self, url: String) {
        if url.is_empty() {
            return;
        }
        self.copy = Some(url);
        self.say_key("copied.link");
    }

    /// `w`: watch the run (a toast of the system and a sound when it ends),
    /// or stop watching it.
    pub(super) fn watch(&mut self, run: &Run) {
        if let Some(at) = self.watched.iter().position(|w| w.id == run.id) {
            self.watched.remove(at);
            self.say_key("watch.off");
        } else if run.status.active() {
            self.watched.push(run.clone());
            self.say_key("watch.on");
        } else {
            self.say_key("watch.done");
        }
    }

    /// Whether the run `id` is watched (`w`).
    pub fn watching(&self, id: u64) -> bool {
        self.watched.iter().any(|w| w.id == id)
    }

    /// The watched runs that newer ones have pushed out of `runs`: each is
    /// asked for apart, or it would never be seen to end.
    pub fn watched_away(&self) -> Vec<&Run> {
        self.watched.iter().filter(|w| !self.runs.iter().any(|r| r.id == w.id)).collect()
    }

    /// The runs came again (`more`: watched runs asked for apart): the
    /// watched ones that have ended ring.
    pub fn runs_came(&mut self, more: &[Run]) {
        let mut ended = Vec::new();
        for w in &mut self.watched {
            if let Some(r) = self.runs.iter().chain(more).find(|r| r.id == w.id) {
                *w = r.clone();
            }
        }
        self.watched.retain(|w| {
            if w.status.active() {
                return true;
            }
            ended.push(w.clone());
            false
        });
        for r in ended {
            let k = minato_core::prefs::Notify::slot(r.status);
            let title = self.tr(["watch.finished", "watch.failed", "watch.cancelled"][k]).to_string();
            let body = format!("{} · {}: {}", r.repo, r.workflow, r.title);
            self.say(format!("{title}: {body}"));
            let n = &self.settings.notify;
            let sound = n.sound[k].then(|| n.sounds[k].clone());
            let level = if k == 1 { ito_notify::webhook::Level::Error } else { ito_notify::webhook::Level::Done };
            let hooks = n
                .hooks_for(k)
                .into_iter()
                .map(|(url, format)| Hook { url: url.into(), format: format.into(), level, folder: r.repo.clone() })
                .collect();
            self.rings.push(Ring { title, body, system: n.system[k], sound, focus: n.focus, hooks });
        }
    }

    /// The lines of the status view: the lanes, the repositories' last
    /// releases, then the items that wait on the owner.
    pub fn status_rows(&self) -> Vec<overview::Row> {
        let mut rows: Vec<overview::Row> = overview::lanes(&self.items).into_iter().map(overview::Row::Lane).collect();
        for repo in &self.status_repos {
            if let Some(Some(s)) = self.shipped.get(repo) {
                let ahead = self.ahead.get(repo).filter(|(tag, _)| *tag == s.tag).map(|(_, n)| *n);
                rows.push(overview::Row::Release { repo: repo.clone(), shipped: Some(s.clone()), ahead });
            }
        }
        rows.extend(overview::attention(&self.items).into_iter().map(|(k, label)| overview::Row::Item(k, label)));
        rows
    }

    /// The alerts the search box leaves, as places in `alerts`.
    pub fn sec_shown(&self) -> Vec<usize> {
        security::shown(&self.alerts, &self.inbox.find)
    }

    /// Dismiss the alert whose key is `key` (confirmed), and forget it here
    /// at once; `u` reopens it.
    pub(super) fn dismiss(&mut self, key: &str) {
        let Some(at) = self.alerts.iter().position(|a| a.key() == key) else { return };
        let a = self.alerts.remove(at);
        self.security_sel = self.security_sel.min(self.sec_shown().len().saturating_sub(1));
        let call = Call::Send {
            method: minato_core::github::http::Method::Patch,
            path: a.source.alert_path(&a.repo, a.number),
            body: security::dismiss_body(a.source, security::default_reason(a.source).1),
        };
        self.sec_undo.push(a);
        self.queue(&format!("write:sec:{}", self.outbox.len()), call, Some("sec.dismissed"));
        self.alerts_changed = true;
    }

    /// A row of the notifications clicked, or the header's button.
    pub fn note_click(&mut self, c: NoteClick) {
        match c {
            NoteClick::Open(k) if k < self.notes.len() => self.read_note(k, '\n'),
            NoteClick::Open(_) => {}
            NoteClick::ReadAll => self.mark_all_read(),
            NoteClick::Close => self.mode = Mode::Main,
        }
    }

    /// The unread notifications as GitHub listed them, less those marked
    /// read here since.
    pub fn take_notes(&mut self, v: Vec<Note>) {
        self.notes = self.notes_read.unread(v);
        self.unread = self.notes.len() as u32;
        self.ns.clamp(self.notes.len());
    }

    /// Mark the notification at `k` read; `how` is the key (`d` `o` `Enter`).
    pub(super) fn read_note(&mut self, k: usize, how: char) {
        let note = self.notes.remove(k);
        self.notes_read.mark(&note);
        self.queue(&format!("note:{}", note.id), notes::mark_read(&note.id), None);
        self.unread = self.notes.len() as u32;
        self.ns.clamp(self.notes.len());
        let page = self.note_run(&note).map_or_else(|| note.url.clone(), |k| self.runs[k].url.clone());
        match how {
            'o' => self.open_url = Some(page),
            '\n' => {
                self.mode = Mode::Main;
                if self.open_item(&note.url) || self.open_red_main(&note) || self.open_run(&note) {
                    return;
                }
                if notes::number(&note.url).is_some() {
                    // Not one the searches list (merged, closed): fetched
                    // and shown in the list (see `show_fetched`).
                    self.queue(
                        &format!("fetch:{}", note.url),
                        Call::Graphql { query: search::item_query(), variables: search::item_vars(&note.url) },
                        None,
                    );
                    self.say_key("notes.fetching");
                } else {
                    self.open_url = Some(page);
                }
            }
            _ => {}
        }
    }

    /// The run a notification of a workflow run is about: the newest of
    /// its workflow on its branch done by the time it came (else the
    /// newest).
    pub(super) fn note_run(&self, note: &Note) -> Option<usize> {
        let (workflow, branch) = notes::workflow_run(note)?;
        let of = |r: &&Run| r.repo == note.repo && r.workflow == workflow && r.branch == branch;
        let newest = |done: bool| {
            self.runs
                .iter()
                .enumerate()
                .filter(|(_, r)| of(r) && (!done || r.updated_at <= note.updated_at))
                .max_by(|a, b| a.1.started_at.cmp(&b.1.started_at))
                .map(|(k, _)| k)
        };
        newest(true).or_else(|| newest(false))
    }

    /// The inbox card of the red CI of a notification's repository, when
    /// the notification is of a run on the branch the card says is red.
    pub(super) fn open_red_main(&mut self, note: &Note) -> bool {
        let Some((_, branch)) = notes::workflow_run(note) else { return false };
        let id = format!("main:{}", note.repo);
        let Some(at) = self.items.iter().position(|i| i.id == id && mainci::branch(i) == Some(branch)) else { return false };
        let Some(card) = self.inbox_all().into_iter().find(|c| c.at == at) else { return false };
        self.open_card(card);
        true
    }

    /// Show the run a notification is about in Actions, the filters
    /// cleared if they hide it. False if minato does not have it.
    pub(super) fn open_run(&mut self, note: &Note) -> bool {
        let Some(k) = self.note_run(note) else { return false };
        let id = self.runs[k].id;
        // Shown even if seen, as the selected run.
        self.acts.keep = Some(id);
        if !self.acts.matches(&self.runs[k], &self.runs) {
            let seen = std::mem::take(&mut self.acts.seen);
            self.acts = ActState::default();
            self.acts.state = None;
            self.acts.seen = seen;
            self.acts.keep = Some(id);
        }
        self.acts.col = actions::Col::Runs;
        self.acts.sel = self.acts.shown(&self.runs).iter().position(|r| r.id == id).unwrap_or(0);
        self.acts.scroll = 0.0;
        self.view = View::Actions;
        true
    }

    /// Where a notification's thread is in minato, by the page it opens:
    /// its row in the items, and its inbox card unless that is done.
    pub fn note_place(&self, url: &str) -> (Option<usize>, Option<Card>) {
        let (at, card) = self.place(url);
        (at, card.filter(|c| c.group != Group::Done))
    }

    /// The row of the item whose page is `url`, and its inbox card.
    pub(super) fn place(&self, url: &str) -> (Option<usize>, Option<Card>) {
        let at = self.items.iter().position(|i| i.url == url);
        let card = at.and_then(|at| self.inbox_all().into_iter().find(|c| c.at == at));
        (at, card)
    }

    /// `/`: the search box takes the keys, with the words it had.
    pub(super) fn find_start(&mut self) {
        self.find_open = true;
        self.find_fresh = true;
    }

    /// The search's words as typed: the inbox and the list show only what
    /// has them all, from their first row.
    pub fn set_find(&mut self, text: &str) {
        self.find_text = text.to_string();
        let q = minato_core::find::Query::new(text);
        if q != self.inbox.find {
            self.inbox.find = q.clone();
            self.list.find = q;
            (self.inbox.sel, self.inbox.scroll) = (0, 0.0);
            (self.list.sel, self.list.scroll) = (0, 0.0);
            self.security_sel = 0;
        }
    }

    /// Esc: the search box closed and its words gone.
    pub fn find_clear(&mut self) {
        self.find_open = false;
        self.set_find("");
    }

    /// Whether the search filters the view on screen.
    pub fn find_on(&self) -> bool {
        self.mode == Mode::Main
            && matches!(self.view, View::Inbox | View::List | View::Security)
            && (self.find_open || !self.inbox.find.is_empty())
    }

    /// The search's words for what the view shows.
    pub fn lit(&self) -> &minato_core::find::Query {
        &self.inbox.find
    }

    /// How many cards or rows the search leaves in the view.
    pub fn find_count(&self) -> usize {
        if self.view == View::Security {
            return self.sec_shown().len();
        }
        if self.view == View::Inbox {
            return self.inbox.shown(&self.inbox_all(), &self.items).len();
        }
        self.list.shown(&self.items, self.viewer.as_deref().unwrap_or("")).len()
    }

    /// The box gives the keys back once anything is over it or the view
    /// has no search (its words stay).
    pub fn find_check(&mut self) {
        let free = self.compose.is_none() && self.confirm.is_none() && self.palette.is_none() && self.overlay.is_none();
        if !(free && self.mode == Mode::Main && matches!(self.view, View::Inbox | View::List)) {
            self.find_open = false;
        }
    }
}
