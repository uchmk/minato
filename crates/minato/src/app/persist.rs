//! What is kept between runs and how it is queued for the writers.

use super::*;

impl App {
    /// Keep the marks in `path` (marks.toml): read them now (at start-up;
    /// from `old`, the marks.json of before, when there is no `path` yet),
    /// and write every change on a thread of its own. `on` is false for a
    /// run that keeps nothing.
    pub fn keep_marks(&mut self, path: std::path::PathBuf, old: &std::path::Path, on: bool) {
        let kept = ito_common::kept::read::<minato_core::kept::MarksFile>(&path);
        let from_old = kept.is_none() && !path.exists();
        self.marks = match kept {
            Some(f) => f.marks(),
            None if from_old => {
                std::fs::read_to_string(old).map(|t| minato_core::kept::MarksFile::from_old(&t).marks()).unwrap_or_default()
            }
            None => HashMap::new(),
        };
        let w = ito_common::kept::Writer::new(path, on);
        // The old file stays; the new one takes over from here.
        if from_old && !self.marks.is_empty() {
            w.save_toml(&minato_core::kept::MarksFile::of(&self.marks));
        }
        self.marks_tx = Some(w);
    }

    /// Keep the runs seen in Actions in `path` (seen-runs.toml), the same
    /// way (`old` is the seen-runs.txt of before).
    pub fn keep_seen(&mut self, path: std::path::PathBuf, old: &std::path::Path, on: bool) {
        let kept = ito_common::kept::read::<minato_core::kept::SeenFile>(&path);
        let from_old = kept.is_none() && !path.exists();
        let file = match kept {
            Some(f) => f,
            None if from_old => std::fs::read_to_string(old).map(|t| minato_core::kept::SeenFile::from_old(&t)).unwrap_or_default(),
            None => Default::default(),
        };
        self.acts.seen = file.set();
        let w = ito_common::kept::Writer::new(path, on);
        if from_old && !file.recent.is_empty() {
            w.save_toml(&file);
        }
        self.seen_tx = Some(w);
    }

    /// Each frame: the run selected in Actions becomes seen after a while
    /// on screen (written when it does).
    pub fn settle_runs(&mut self, now: f64) {
        let shown = self.view == View::Actions && self.mode == Mode::Main;
        if self.acts.settle(&self.runs, now, shown) {
            let mut ids: Vec<u64> = self.acts.seen.iter().copied().collect();
            // The newest runs have the largest ids; the oldest are let go.
            ids.sort_unstable_by(|a, b| b.cmp(a));
            ids.truncate(SEEN_KEPT);
            self.acts.seen = ids.iter().copied().collect();
            if let Some(w) = &self.seen_tx {
                w.save_toml(&minato_core::kept::SeenFile { recent: ids });
            }
        }
    }

    /// Keep the columns' widths in `path` (state.toml), the same way.
    pub fn keep_state(&mut self, path: std::path::PathBuf) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            let st = minato_core::state::parse(&text);
            set_ratios(&mut self.inbox_split, &st.inbox);
            set_ratios(&mut self.list_split, &st.list);
            set_ratios(&mut self.acts_split, &st.actions);
            set_ratios(&mut self.security_split, &st.security);
            set_ratios(&mut self.status_split, &st.status);
        }
        self.state_tx = Some(writer(path));
    }

    /// Keep minato's settings and the saved searches in config.toml at
    /// `path`: read them now, and write every change on a thread of its own.
    pub fn keep_settings(&mut self, path: std::path::PathBuf) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            self.settings = Settings::parse_toml(&text);
            self.scope_text = [self.settings.owners_line(), self.settings.project.clone()];
            self.searches_text = self
                .settings
                .searches
                .iter()
                .map(|l| {
                    format!(
                        "{l}
"
                    )
                })
                .collect();
            self.sync_searches();
        }
        self.settings_tx = Some(crate::config::writer(path));
    }

    /// Take the saved searches from the text box; a search that is gone
    /// stops being the list's filter.
    pub fn sync_searches(&mut self) {
        self.list.searches = self.searches_text.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect();
        self.settings.searches = self.list.searches.clone();
        if self.list.filter.search.is_some_and(|i| i >= self.list.searches.len()) {
            self.list.filter.search = None;
        }
    }

    /// Write the saved searches after the screen changed them (they are
    /// kept in config.toml with the settings).
    pub fn save_searches(&self) {
        self.save_settings();
    }

    /// Write the settings after the screen changed them.
    pub fn save_settings(&self) {
        if let Some(tx) = &self.settings_tx {
            let _ = tx.send(self.settings.clone());
        }
    }

    /// Whether the writing box offers field `f` (0 Labels .. 5 Reviewers);
    /// says so in a toast when the settings hide it. Reviewers on anything
    /// but a PR say that first, hidden or not.
    pub fn field_shown(&mut self, f: usize) -> bool {
        let pr = self.compose.as_ref().is_some_and(|c| self.item(&c.id).is_some_and(|i| i.kind == Kind::Pr));
        if f == 5 && !pr {
            self.say_key("write.not_pr");
            return false;
        }
        let on = self.settings.fields[f];
        if !on {
            self.say_key("pick.hidden");
        }
        on
    }

    pub(super) fn save_marks(&self) {
        if let Some(w) = &self.marks_tx {
            w.save_toml(&minato_core::kept::MarksFile::of(&self.marks));
        }
    }

    pub(super) fn queue(&mut self, key: &str, call: Call, done: Option<&'static str>) {
        self.outbox.push(Outgoing { key: key.to_string(), call, done });
    }

    pub(super) fn item(&self, id: &str) -> Option<Item> {
        self.items.iter().find(|i| i.id == id).cloned()
    }

    /// The selected item, unless it is a stub for a repository (red main CI):
    /// nothing can be written to that.
    pub(super) fn writable(&mut self) -> Option<Item> {
        let item = self.selected().cloned()?;
        if minato_core::mainci::is_stub(&item) {
            self.say(self.tr("write.not_real").to_string());
            return None;
        }
        Some(item)
    }

    /// `M`: mark the newest comment of the selected discussion that is not
    /// the asker's as its answer.
    pub(super) fn mark_answer(&mut self) {
        let Some(item) = self.selected().cloned() else { return };
        if item.kind != Kind::Discussion || minato_core::mainci::is_stub(&item) {
            self.say_key("write.not_discussion");
            return;
        }
        let entry = self
            .detail
            .as_ref()
            .filter(|(id, _)| *id == item.id)
            .and_then(|(_, d)| d.timeline.iter().rev().find(|e| e.author != item.author && !e.id.is_empty()));
        match entry {
            Some(e) if e.answer => self.say_key("write.already_answer"),
            Some(e) => {
                let n = self.outbox.len();
                let call = write::mark_answer(&e.id);
                self.queue(&format!("write:{n}:answer"), call, Some("write.answered"));
            }
            None => self.say_key("write.no_answer"),
        }
    }

    /// `W`: keep the list's search (the filter, or the saved search in
    /// force) with the search box's words as a new saved search.
    pub(super) fn save_search(&mut self) {
        let owners = self.owners_shown();
        let query = match self.list.active_search() {
            Some(t) => t.to_string(),
            None => self.list.filter.query(&owners),
        };
        let words = if self.find_text.trim().is_empty() {
            minato_core::list::split_search(self.list.filter.search.and_then(|i| self.list.searches.get(i)).map_or("", String::as_str))
                .1
                .to_string()
        } else {
            self.find_text.clone()
        };
        let line = minato_core::list::saved_line(&query, &words);
        if self.list.searches.contains(&line) {
            self.say(self.tr("search.already"));
            return;
        }
        if !self.searches_text.is_empty() && !self.searches_text.ends_with('\n') {
            self.searches_text.push('\n');
        }
        self.searches_text.push_str(&line);
        self.searches_text.push('\n');
        self.sync_searches();
        self.save_searches();
        self.say(format!("{} {line}", self.tr("search.saved")));
    }

    /// `Z`: take the saved search in force out of the saved searches.
    pub(super) fn forget_search(&mut self) {
        let Some(line) = self.list.filter.search.and_then(|i| self.list.searches.get(i)).cloned() else {
            self.say_key("search.none_in_force");
            return;
        };
        let mut gone = false;
        let kept: Vec<&str> = self
            .searches_text
            .lines()
            .filter(|l| {
                let drop = !gone && l.trim() == line;
                gone |= drop;
                !drop
            })
            .collect();
        self.searches_text = kept
            .iter()
            .map(|l| {
                format!(
                    "{l}
"
                )
            })
            .collect();
        self.list.filter.search = None;
        self.sync_searches();
        self.save_searches();
        self.say(format!("{} {line}", self.tr("search.forgot")));
    }
}
