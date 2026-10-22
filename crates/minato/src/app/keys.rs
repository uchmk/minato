//! What the keys do: the dispatch by view and mode, the bars and the palette.

use super::*;

impl App {
    /// A character typed (not a key with Ctrl or Alt: those are the
    /// shortcuts). Returns whether it was used.
    pub fn feed(&mut self, c: char) -> bool {
        let g = std::mem::take(&mut self.pending_g);
        let free = self.compose.is_none()
            && self.confirm.is_none()
            && self.palette.is_none()
            && self.overlay.is_none()
            && matches!(self.mode, Mode::Main | Mode::Notes);
        let used = match c {
            'g' if free => {
                if g {
                    self.jump(false);
                } else {
                    self.pending_g = true;
                }
                true
            }
            'G' if free => {
                self.jump(true);
                true
            }
            // `o`'s page, in the system's browser.
            'O' if free => {
                let used = self.feed_key('o');
                self.open_outside = self.open_url.is_some();
                used
            }
            _ => self.feed_key(c),
        };
        self.clamp_scroll();
        used
    }

    pub(super) fn feed_key(&mut self, c: char) -> bool {
        if self.compose.is_some() || self.confirm.is_some() || self.palette.is_some() || self.overlay.is_some() {
            return true;
        }
        match (self.mode, c) {
            (Mode::Notes, c) => return self.notes_key(c),
            (Mode::Help, '?') => self.mode = Mode::Main,
            (Mode::Help, 'j') => self.help_top = (self.help_top + 1).min(self.help_last.get()),
            (Mode::Help, 'k') => self.help_top = self.help_top.saturating_sub(1),
            (Mode::Settings, _) | (Mode::Help, _) => return false,
            (Mode::Main, '?') => (self.mode, self.help_top) = (Mode::Help, 0),
            (Mode::Main, ',') => self.mode = Mode::Settings,
            (Mode::Main, 'n') => self.mode = Mode::Notes,
            (Mode::Main, '@') => self.palette = Some(owner_palette()),
            (Mode::Main, 'W') if self.view == View::List => self.save_search(),
            (Mode::Main, 'Z') if self.view == View::List => self.forget_search(),
            (Mode::Main, 'S') if matches!(self.view, View::Inbox | View::List) => self.repo_settings(),
            (Mode::Main, 'M') if matches!(self.view, View::Inbox | View::List) => self.mark_answer(),
            (Mode::Main, 'N') if matches!(self.view, View::Inbox | View::List) => self.new_issue(),
            (Mode::Main, '/') if matches!(self.view, View::Inbox | View::List | View::Security) => self.find_start(),
            (Mode::Main, c) => match View::from_digit(c) {
                Some(v) => self.view = v,
                None if self.view == View::List => return self.list_key(c),
                None if self.view == View::Inbox => return self.inbox_key(c),
                None if self.view == View::Actions => return self.actions_key(c),
                None if self.view == View::Board => return self.board_key(c),
                None if self.view == View::Status => return self.status_key(c),
                None if self.view == View::Security => return self.security_key(c),
                None => return false,
            },
        }
        true
    }

    /// A key in the list view: the columns' own, and `o` for the browser.
    pub(super) fn list_key(&mut self, c: char) -> bool {
        let me = self.viewer.clone().unwrap_or_default();
        match c {
            'r' => {
                self.open_compose();
                return true;
            }
            'a' => {
                self.assign_self();
                return true;
            }
            'm' => {
                self.ask_merge();
                return true;
            }
            'x' if self.selected().is_some_and(|i| i.closed) => {
                self.ask_reopen();
                return true;
            }
            'x' => {
                self.ask_close();
                return true;
            }
            'C' => {
                self.show_closed = !self.show_closed;
                self.say_key(if self.show_closed { "list.closed_on" } else { "list.closed_off" });
                return true;
            }
            _ => {}
        }
        if c == 'o' || c == 'y' {
            if let Some(url) = self.list.selected(&self.items, &me).map(|i| i.url.clone()) {
                if c == 'o' {
                    self.open_url = Some(url);
                } else {
                    self.copy_link(url);
                }
            }
            return true;
        }
        self.list.key(c, &self.items, &me)
    }

    /// Enter: the same as Space (choose a filter).
    pub fn enter(&mut self) -> bool {
        if let Some(id) = self.confirm.take() {
            match self.confirm_act {
                Confirm::Merge => self.merge(&id),
                Confirm::Close => self.close(&id),
                Confirm::Reopen => self.reopen(&id),
                Confirm::Dismiss => self.dismiss(&id),
                Confirm::Dispatch | Confirm::Cancel => {
                    if let Some(run) = self.runs.iter().find(|r| r.id.to_string() == id).cloned() {
                        if self.confirm_act == Confirm::Cancel {
                            self.queue(&format!("write:{}:cancel", run.id), actions::cancel(&run), Some("run.cancelled"));
                        } else {
                            self.queue(&format!("write:{}:dispatch", run.id), actions::dispatch(&run), Some("run.dispatched"));
                        }
                    }
                }
            }
            return true;
        }
        if self.mode == Mode::Notes {
            return self.notes_key('\n');
        }
        self.mode == Mode::Main
            && match self.view {
                View::List => self.list_key(' '),
                View::Actions => self.actions_key(' '),
                View::Board => self.board_key('\n'),
                View::Status => self.status_key('\n'),
                View::Security => self.security_key('\n'),
                _ => false,
            }
    }

    /// The run under the cursor of the Actions view.
    pub fn run(&self) -> Option<&Run> {
        self.acts.selected(&self.runs)
    }

    /// The view's keys that do something now, for the keybar: in Actions,
    /// `R` only on a failed run, `X` on one still going, `p` on one from a
    /// pull request.
    pub fn keys_now(&self) -> Vec<(String, &'static str)> {
        let run = self.run();
        self.key_rows(Ctx::View(self.view), self.view.keys())
            .into_iter()
            .filter(|(_, w)| match *w {
                "key.rerun" => run.is_some_and(|r| r.status == actions::Status::Bad),
                "key.cancel" => run.is_some_and(|r| r.status.active()),
                "run.open_pr" => run.is_some_and(actions::has_pr),
                "key.watch" => run.is_some_and(|r| r.status.active() || self.watching(r.id)),
                _ => true,
            })
            .collect()
    }

    /// The keys of the writing box `c` (or its picker's): for a new issue
    /// create, else send and the fields' `Alt` keys; Esc last.
    pub fn compose_keys(&self, c: &Compose) -> Vec<(&'static str, &'static str)> {
        if c.picker.is_some() {
            return vec![("Enter", "bar.tick"), ("↑ ↓", "bar.move"), ("Esc", "bar.apply")];
        }
        if c.id.starts_with("new:") {
            return vec![
                ("Ctrl+Enter", "bar.create"),
                ("Ctrl+P", "compose.preview"),
                ("Ctrl+B I K", "bar.format"),
                ("Esc", if c.preview { "bar.edit" } else { "bar.no" }),
            ];
        }
        let mut keys = vec![("Ctrl+Enter", "bar.send"), ("Ctrl+P", "compose.preview"), ("Ctrl+Shift+Enter", "bar.send_close")];
        for k in self.settings.order {
            if self.settings.fields[k] {
                keys.push((
                    ["Alt+L", "Alt+A", "Alt+S", "Alt+M", "Alt+P", "Alt+R"][k],
                    ["pick.labels", "pick.assignees", "pick.status", "pick.milestone", "pick.priority", "pick.reviewers"][k],
                ));
            }
        }
        keys.extend([("Ctrl+B I K", "bar.format"), ("Esc", if c.preview { "bar.edit" } else { "bar.leave" })]);
        keys
    }

    /// The keys for the keybar as things stand: a confirmation's, the writing
    /// box's (or its picker's), the palette's, a form's or a log's, else the
    /// mode's (none while the search has the bar). The bar keeps the way out
    /// (Esc, or F1 for the list of keys) to the last.
    pub fn bar_keys(&self) -> Vec<(String, &'static str)> {
        let own = |keys: Vec<(&str, &'static str)>| keys.into_iter().map(|(k, w)| (k.to_owned(), w)).collect();
        if self.confirm.is_some() {
            return own(match self.confirm_act {
                Confirm::Merge => vec![("Enter", "key.merge"), ("Esc", "bar.no")],
                Confirm::Close => vec![("Enter", "key.close"), ("Esc", "bar.no")],
                Confirm::Reopen => vec![("Enter", "bar.reopen"), ("Esc", "bar.no")],
                Confirm::Dispatch => vec![("Enter", "bar.run"), ("Esc", "bar.no")],
                Confirm::Cancel => vec![("Enter", "bar.stop"), ("Esc", "bar.keep")],
                Confirm::Dismiss => vec![("Enter", "bar.dismiss"), ("Esc", "bar.no")],
            });
        }
        if let Some(c) = &self.compose {
            return own(self.compose_keys(c));
        }
        if self.palette.is_some() {
            return own(vec![("Enter", "bar.do"), ("↑ ↓", "bar.choose"), ("Esc", "key.close")]);
        }
        match &self.overlay {
            Some(Overlay::Form(_)) => {
                return own(vec![("Ctrl+Enter", "bar.start"), ("↑ ↓", "bar.input"), ("Enter", "bar.change"), ("Esc", "bar.no")])
            }
            Some(Overlay::Log(_)) => {
                return own(vec![("j k", "bar.scroll"), ("Space", "bar.page"), ("g G", "bar.ends"), ("Esc", "key.close")])
            }
            None => {}
        }
        match self.mode {
            Mode::Settings => own(vec![("Esc", "key.back"), ("Ctrl+Tab", "settings.theme")]),
            Mode::Help if self.help_last.get() > 0 => self.key_rows(Ctx::Help, &[("{down} {up}", "help.more"), ("F1 Esc", "key.back")]),
            Mode::Help => own(vec![("F1 Esc", "key.back")]),
            Mode::Notes => self.key_rows(
                Ctx::Notes,
                &[("{down} {up}", "notes.pick"), ("{enter}", "key.open"), ("{read_all}", "notes.read_all"), ("Esc", "notes.close")],
            ),
            Mode::Main if self.find_on() => Vec::new(),
            Mode::Main => {
                let tail = self.key_rows(Ctx::View(self.view), &[("F5", "key.sync"), ("{owner}", "key.owner"), ("F1", "key.help")]);
                self.keys_now().into_iter().chain(tail).collect()
            }
        }
    }

    /// A key in the Actions view: `R` runs the failed jobs again, `X` stops,
    /// `D` starts the workflow by hand, `e` opens the failed job's log, `p`
    /// the pull request, `o` the run; the rest moves the cursor.
    pub(super) fn actions_key(&mut self, c: char) -> bool {
        let Some(run) = self.run().cloned() else { return self.acts.key(c, &self.runs) };
        if self.acts.col == actions::Col::Filters && !matches!(c, 'e' | 'o' | 'p' | 'w' | 'y') {
            return self.acts.key(c, &self.runs);
        }
        match c {
            'w' => self.watch(&run),
            'y' => self.copy_link(run.url.clone()),
            'R' if run.status == actions::Status::Bad => {
                self.queue(&format!("write:{}:rerun", run.id), actions::rerun_failed(&run), Some("run.reran"))
            }
            'R' => self.say_key("run.not_failed"),
            'X' if run.status.active() => {
                self.confirm = Some(run.id.to_string());
                self.confirm_act = Confirm::Cancel;
            }
            'X' => self.say_key("run.not_active"),
            'D' => {
                // Its inputs come with the workflow file: not before it.
                let tried = self.run_detail.as_ref().is_some_and(|d| d.id == run.id && d.yaml_tried);
                if !tried && workflow::contents_path(&run).is_some() {
                    self.say_key("run.reading");
                    return true;
                }
                let inputs = self.workflow_of(&run).map(|w| w.inputs.clone()).unwrap_or_default();
                if inputs.is_empty() {
                    // Nothing to fill in: ask before it starts, as closing does.
                    self.confirm = Some(run.id.to_string());
                    self.confirm_act = Confirm::Dispatch;
                } else {
                    self.overlay = Some(Overlay::Form(Box::new(Form::new(run, inputs))));
                }
            }
            'L' => self.open_log(&run),
            'e' => {
                let failed = self
                    .run_detail
                    .as_ref()
                    .filter(|d| d.id == run.id)
                    .and_then(|d| d.jobs.as_ref())
                    .and_then(|j| j.iter().find(|j| j.status == actions::Status::Bad));
                self.open_url = Some(failed.map_or_else(|| run.url.clone(), |j| j.url.clone()));
            }
            'p' => match actions::pr_url(&run) {
                Some(u) => self.open_url = Some(u),
                None => self.say_key("run.no_pr"),
            },
            'o' => self.open_url = Some(run.url.clone()),
            _ => return self.acts.key(c, &self.runs),
        }
        true
    }

    /// A key for the open box.
    pub fn overlay_key(&mut self, k: K) {
        let out = match self.overlay.as_mut() {
            Some(Overlay::Form(f)) => f.key(k),
            Some(Overlay::Log(l)) => l.key(k),
            None => return,
        };
        match out {
            Out::Nothing => {}
            Out::Close => self.overlay = None,
            Out::Missing(name) => {
                let words = format!("{}: {name}", self.tr("run.input_missing"));
                self.say(words);
            }
            Out::Send(inputs) => {
                if let Some(Overlay::Form(f)) = self.overlay.take() {
                    self.queue(&format!("write:{}:dispatch", f.run.id), actions::dispatch_with(&f.run, &inputs), Some("run.dispatched"));
                }
            }
        }
    }

    /// A key on the board: `h l j k` move the cursor, `H L` move the card to
    /// the next column (and tell GitHub), `u` undoes the last move, `Enter`
    /// opens the card in the list, `o` in the browser.
    pub(super) fn board_key(&mut self, c: char) -> bool {
        let Some(p) = self.board.as_mut() else { return false };
        match c {
            'H' | 'L' => {
                if let Some((item, call, before)) = board::shift(p, &mut self.bs, if c == 'H' { -1 } else { 1 }) {
                    if let Some(b) = before {
                        self.board_undo.push((item.clone(), b));
                    }
                    self.queue(&format!("board:{}:{}", item, self.outbox.len()), call, Some("board.moved"));
                }
            }
            'u' => {
                let Some((item, option)) = self.board_undo.pop() else { return true };
                if let Some(i) = p.cards.iter().position(|k| k.item == item) {
                    if let Some(call) = p.set_option(i, &option) {
                        self.bs.follow(p, i);
                        self.queue(&format!("board:{}:{}", item, self.outbox.len()), call, Some("board.undone"));
                    }
                }
            }
            'o' | '\n' => {
                if let Some(card) = self.bs.selected(p).map(|i| p.cards[i].clone()) {
                    if c == 'o' {
                        self.open_url = Some(card.url);
                    } else {
                        self.show_in_list(&card.id);
                    }
                }
            }
            'y' => {
                if let Some(url) = self.bs.selected(p).map(|i| p.cards[i].url.clone()) {
                    self.copy_link(url);
                }
            }
            _ => return self.bs.key(c, p),
        }
        true
    }

    /// A key in the status view: `j k` move, `o` opens the line's page, `y`
    /// copies it, `Enter` opens an item in the list (other lines in the browser).
    pub(super) fn status_key(&mut self, c: char) -> bool {
        let rows = self.status_rows();
        let last = rows.len().saturating_sub(1);
        self.status_sel = self.status_sel.min(last);
        match c {
            'j' => self.status_sel = (self.status_sel + 1).min(last),
            'k' => self.status_sel = self.status_sel.saturating_sub(1),
            'o' | 'y' | '\n' => {
                let Some(row) = rows.get(self.status_sel) else { return true };
                match (c, row) {
                    ('\n', overview::Row::Item(k, _)) => {
                        let id = self.items[*k].id.clone();
                        self.show_in_list(&id);
                    }
                    ('y', _) => self.copy_link(row.url(&self.items)),
                    _ => self.open_url = Some(row.url(&self.items)).filter(|u| !u.is_empty()),
                }
            }
            _ => return false,
        }
        true
    }

    /// A key in the security view: `j k` move, `o` opens the alert on
    /// GitHub, `y` copies its link, `Enter` opens it too, `x` asks to dismiss
    /// it, `u` reopens the one dismissed last.
    pub(super) fn security_key(&mut self, c: char) -> bool {
        let shown = self.sec_shown();
        let last = shown.len().saturating_sub(1);
        self.security_sel = self.security_sel.min(last);
        let at = shown.get(self.security_sel).copied();
        match c {
            'j' => self.security_sel = (self.security_sel + 1).min(last),
            'k' => self.security_sel = self.security_sel.saturating_sub(1),
            'o' | '\n' | 'y' => {
                let Some(url) = at.map(|i| self.alerts[i].url.clone()) else { return true };
                if c == 'y' {
                    self.copy_link(url);
                } else {
                    self.open_url = Some(url).filter(|u| !u.is_empty());
                }
            }
            'x' => {
                if let Some(key) = at.map(|i| self.alerts[i].key()) {
                    self.confirm = Some(key);
                    self.confirm_act = Confirm::Dismiss;
                }
            }
            'u' => {
                let Some(a) = self.sec_undo.pop() else { return true };
                let key = format!("write:sec:{}", self.outbox.len());
                let call = Call::Send {
                    method: minato_core::github::http::Method::Patch,
                    path: a.source.alert_path(&a.repo, a.number),
                    body: security::reopen_body(),
                };
                self.queue(&key, call, Some("sec.reopened"));
            }
            _ => return false,
        }
        true
    }

    /// A key over the notifications: `j k` move, `Enter` opens the thread in
    /// minato (in the browser if minato does not have it) and marks it read,
    /// `o` opens it in the browser, `d` only marks it read, `A` marks all,
    /// `n` closes.
    pub(super) fn notes_key(&mut self, c: char) -> bool {
        match c {
            'n' => self.mode = Mode::Main,
            'A' => self.mark_all_read(),
            'd' | 'o' | '\n' => {
                if self.ns.sel < self.notes.len() {
                    self.read_note(self.ns.sel, c);
                }
            }
            _ => return self.ns.key(c, self.notes.len()),
        }
        true
    }

    /// A click in a view.
    pub fn click(&mut self, c: Click) {
        use minato_core::list::Col;
        let me = self.viewer.clone().unwrap_or_default();
        // A click on the box beside an open picker writes what it changed.
        if c.is_compose() {
            self.apply_pick();
        }
        match c {
            Click::ListFilter(k) => {
                self.list.col = Col::Filters;
                self.list.cursor = k;
                self.list.key(' ', &self.items, &me);
            }
            Click::ListItem(k) => {
                self.list.col = Col::Items;
                if self.list.sel != k {
                    self.list.sel = k;
                    self.list.scroll = 0.0;
                }
            }
            Click::ListCol(col) => self.list.col = col,
            Click::InboxCard(k) => {
                if self.inbox.sel != k {
                    self.inbox.sel = k;
                    self.inbox.scroll = 0.0;
                }
            }
            Click::InboxStatus(s) => self.inbox.status = s,
            Click::InboxRepo(r) => self.inbox.repo = r,
            Click::InboxTag(t) => self.inbox.tag = if self.inbox.tag == t { None } else { t },
            Click::ActFilter(k) => {
                self.acts.col = actions::Col::Filters;
                self.acts.cursor = k;
                self.acts.key(' ', &self.runs);
            }
            Click::ActRun(k) => {
                self.acts.col = actions::Col::Runs;
                if self.acts.sel != k {
                    self.acts.sel = k;
                    self.acts.scroll = 0.0;
                }
            }
            Click::ActCol(col) => self.acts.col = col,
            Click::SecurityRow(k, open) => {
                self.security_sel = k;
                if open {
                    self.security_key('\n');
                }
            }
            Click::StatusRow(k, open) => {
                self.status_sel = k;
                if open {
                    self.status_key('\n');
                }
            }
            Click::Write => self.open_compose(),
            Click::Field(k) => {
                self.open_compose();
                self.open_pick(k);
            }
            Click::Key(k) => {
                self.feed(k);
            }
            Click::Tool(t) => {
                self.open_compose();
                if let Some(c) = self.compose.as_mut() {
                    c.preview = false;
                    c.tool = Some(t);
                }
            }
            Click::Tab(on) => {
                self.open_compose();
                if let Some(c) = self.compose.as_mut() {
                    c.preview = on;
                }
            }
            Click::Send(close) => {
                self.open_compose();
                let empty = self.compose.as_ref().is_none_or(|c| c.text.trim().is_empty());
                match (close, empty) {
                    // Close with nothing written asks first, as `x` does.
                    (true, true) => {
                        self.close_compose();
                        self.ask_close();
                    }
                    (false, true) => {}
                    _ => self.send_comment(close),
                }
            }
        }
        let all = self.inbox_all();
        self.inbox.clamp(&all, &self.items);
    }

    /// The lines of the palette: the commands, the owners to switch to
    /// (`@login`; what `query` names after `@` too), then the items
    /// (`#12 title`).
    pub fn palette_entries(&self, query: &str) -> Vec<ito_palette::Entry<Cmd>> {
        use ito_palette::Entry;
        // tsumugi's words for the same commands; the views are not here, as
        // the bar shows them with their keys.
        let mut e = vec![
            Entry::new(self.tr("cmd.sync"), "F5", Cmd::Sync),
            Entry::new(self.tr("cmd.notes"), self.char_label(Ctx::View(self.view), "n"), Cmd::Notes),
            Entry::new(self.tr("cmd.mark_all"), self.char_label(Ctx::Notes, "A"), Cmd::MarkAll),
            Entry::new(self.tr("cmd.settings"), "Ctrl+,", Cmd::Settings),
            Entry::new(self.tr("cmd.zoom_in"), "Ctrl+=", Cmd::Scale(ito_common::ScaleTo::In)),
            Entry::new(self.tr("cmd.zoom_out"), "Ctrl+-", Cmd::Scale(ito_common::ScaleTo::Out)),
            Entry::new(self.tr("cmd.zoom_reset"), "Ctrl+0", Cmd::Scale(ito_common::ScaleTo::Reset)),
            Entry::new(self.tr("cmd.help"), "F1", Cmd::Help),
        ];
        let now = self.settings.owner.as_str();
        let mut owner = |login: &str, detail: &str| {
            if !login.is_empty() && !e.iter().any(|x| matches!(&x.pick, Cmd::Owner(o) if o.eq_ignore_ascii_case(login))) {
                let mut x = Entry::new(format!("@{login}"), "", Cmd::Owner(login.to_string()));
                x.detail = detail.to_string();
                e.push(x);
            }
        };
        let typed = minato_core::prefs::login(query.trim().strip_prefix('@').unwrap_or(""));
        if !typed.eq_ignore_ascii_case(now) {
            owner(&typed, self.tr("cmd.owner"));
        }
        if !now.is_empty() {
            if let Some(me) = self.viewer.as_deref() {
                owner(me, self.tr("cmd.owner_me"));
            }
        }
        for o in self.settings.owners.iter().filter(|o| !o.eq_ignore_ascii_case(now)) {
            owner(o, self.tr("cmd.owner"));
        }
        // `+@name` shows another owner's alongside; `-@name` stops it.
        let added = minato_core::prefs::login(query.trim().strip_prefix("+@").unwrap_or(""));
        let shown = |o: &str| {
            self.owner().is_some_and(|w| w.eq_ignore_ascii_case(o)) || self.settings.also.iter().any(|x| x.eq_ignore_ascii_case(o))
        };
        if !added.is_empty() && !shown(&added) {
            let mut x = Entry::new(format!("+@{added}"), "", Cmd::AddOwner(added.clone()));
            x.detail = self.tr("cmd.owner_add").to_string();
            e.push(x);
        }
        for o in &self.settings.also {
            let mut x = Entry::new(format!("-@{o}"), "", Cmd::DropOwner(o.clone()));
            x.detail = self.tr("cmd.owner_drop").to_string();
            e.push(x);
        }
        for i in self.items.iter().take(200) {
            let mut x = Entry::new(format!("#{} {}", i.number, i.title), "", Cmd::Item(i.id.clone()));
            x.detail = i.repo.clone();
            e.push(x);
        }
        e
    }

    /// Do what a palette line says.
    pub fn run_cmd(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Settings => self.mode = Mode::Settings,
            Cmd::Help => (self.mode, self.help_top) = (Mode::Help, 0),
            Cmd::Notes => self.mode = Mode::Notes,
            Cmd::MarkAll => self.mark_all_read(),
            Cmd::Sync => self.sync_now(),
            Cmd::Owner(o) => self.show_owner(&o),
            Cmd::AddOwner(o) => self.add_owner(&o),
            Cmd::DropOwner(o) => self.drop_owner(&o),
            Cmd::Scale(to) => self.scale(to),
            Cmd::Item(id) => {
                self.mode = Mode::Main;
                self.show_in_list(&id);
            }
        }
    }

    pub(super) fn say_key(&mut self, key: &'static str) {
        let words = self.tr(key);
        self.say(words);
    }

    /// A key in the inbox: `d` done, `s` later (tomorrow 9:00), `u` undo,
    /// `o` the browser, `c` a prompt for Claude, and the filters and the
    /// cursor.
    pub(super) fn inbox_key(&mut self, c: char) -> bool {
        let all = self.inbox_all();
        let card = self.inbox.selected(&all, &self.items);
        match c {
            'r' => self.open_compose(),
            'm' => self.ask_merge(),
            'R' => self.rerun_ci(),
            'x' => self.ask_close(),
            'a' => self.assign_self(),
            'c' => {
                if let Some(card) = card {
                    self.copy = Some(self.prompt(card));
                    self.say_key("prompt.copied");
                }
            }
            'o' => {
                if let Some(card) = card {
                    self.open_url = Some(self.items[card.at].url.clone());
                }
            }
            'y' => {
                if let Some(card) = card {
                    self.copy_link(self.items[card.at].url.clone());
                }
            }
            'd' | 's' => {
                let Some(card) = card else { return true };
                let id = self.items[card.at].id.clone();
                let mark = if c == 'd' { Mark::Done(now_epoch()) } else { Mark::Later(tomorrow_nine()) };
                self.inbox.mark(&mut self.marks, &id, mark);
                self.save_marks();
                let all = self.inbox_all();
                self.inbox.clamp(&all, &self.items);
                let words = self.tr(if c == 'd' { "inbox.marked_done" } else { "inbox.marked_later" });
                self.say(words);
            }
            'u' => {
                if self.inbox.undo_last(&mut self.marks).is_some() {
                    self.save_marks();
                    let words = self.tr("inbox.undone");
                    self.say(words);
                }
            }
            _ => return self.inbox.key(c, &all, &self.items),
        }
        true
    }

    /// What to paste into a Claude Code session about `card`: the item, why
    /// it is in the inbox and what to do (the rule's ask).
    pub(super) fn prompt(&self, card: Card) -> String {
        let item = &self.items[card.at];
        let head = match (item.number, item.kind) {
            (0, _) => format!("{}: {}", item.repo, item.title),
            (n, Kind::Pr) => format!("{} PR #{n}: {}", item.repo, item.title),
            (n, Kind::Issue) => format!("{} issue #{n}: {}", item.repo, item.title),
            (n, Kind::Discussion) => format!("{} discussion #{n}: {}", item.repo, item.title),
        };
        let why = self.tr(card.rule.map_or("prompt.why_wait", Rule::why));
        let ask = self.tr(card.rule.map_or("prompt.ask_wait", Rule::ask));
        format!("{head}\n{}\n\n{}: {why}\n{}\n{ask}\n", item.url, self.tr("prompt.why"), self.tr("prompt.read"))
    }
}
