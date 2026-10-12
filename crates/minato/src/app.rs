//! The app's state and what the keys do to it. Nothing here draws or reads
//! a file, so the keys can be tested without a window.

use std::time::{Duration, Instant};

use std::collections::HashMap;

use minato_core::actions::{self, ActState, Artifact, Job, Run};
use minato_core::board::{self, BoardState, Project};
use minato_core::composer::Picker;
use minato_core::github::worker::Call;
use minato_core::github::write;
use minato_core::inbox::{Card, InboxState, Mark};
use minato_core::prefs::Settings;
use minato_core::list::{Detail, Item, Kind, ListState};
use minato_core::notes::{self, Note, NotesState};

use minato_core::workflow::{self, Workflow};

use crate::overlay::{Form, K, LogView, Out, Overlay};
use crate::prefs::Prefs;

/// The four views, opened by `1` to `4` (docs/design.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Inbox,
    List,
    Actions,
    Board,
}

impl View {
    pub const ALL: [View; 4] = [View::Inbox, View::List, View::Actions, View::Board];

    /// The digit that opens it.
    pub fn digit(self) -> char {
        match self {
            View::Inbox => '1',
            View::List => '2',
            View::Actions => '3',
            View::Board => '4',
        }
    }

    pub fn from_digit(c: char) -> Option<View> {
        View::ALL.into_iter().find(|v| v.digit() == c)
    }

    /// The word-table key of its name.
    pub fn label(self) -> &'static str {
        match self {
            View::Inbox => "view.inbox",
            View::List => "view.list",
            View::Actions => "view.actions",
            View::Board => "view.board",
        }
    }

    /// The word-table key of the line saying what will be here.
    pub fn empty(self) -> &'static str {
        match self {
            View::Inbox => "empty.inbox",
            View::List => "empty.list",
            View::Actions => "empty.actions",
            View::Board => "empty.board",
        }
    }

    /// The keys that work in this view: `(keys, word-table key)`, the one to
    /// recommend first. The keybar shows the first few; `?` shows them all.
    pub fn keys(self) -> &'static [(&'static str, &'static str)] {
        match self {
            View::Inbox => &[
                ("j k", "key.next"),
                ("Space", "key.details"),
                ("r", "key.write"),
                ("N", "key.new"),
                ("S", "key.repo"),
("M", "key.answer"),
                ("d", "key.done"),
                ("s", "key.later"),
                ("u", "key.undo"),
                ("m", "key.merge"),
                ("x", "key.close"),
                ("a", "key.assign"),
                ("o", "key.browser"),
                ("f F t", "key.filter"),
            ],
            View::List => &[
                ("j k", "key.next"),
                ("h l", "key.columns"),
                ("r", "key.write"),
                ("N", "key.new"),
                ("S", "key.repo"),
                ("a", "key.assign"),
                ("m", "key.merge"),
                ("x", "key.close"),
                ("C", "key.closed"),
                ("i", "key.inbox_add"),
                ("o", "key.browser"),
            ],
            View::Actions => &[
                ("j k", "key.next"),
                ("R", "key.rerun"),
                ("X", "key.cancel"),
                ("D", "key.dispatch"),
                ("e", "key.logs"),
                ("L", "key.log"),
                ("o", "key.browser"),
            ],
            View::Board => &[("h l", "key.columns"), ("j k", "key.next"), ("H L", "key.card"), ("Enter", "key.open"), ("u", "key.undo"), ("o", "key.browser")],
        }
    }
}

/// The keys that work in every view.
pub const GLOBAL_KEYS: [(&str, &str); 6] = [
    ("1 2 3 4", "key.views"),
    ("Ctrl+,", "key.settings"),
    ("Ctrl+Shift+P", "key.palette"),
    ("n", "key.notes"),
    ("u", "key.undo"),
    ("?", "key.help"),
];

/// What the window shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Main,
    Settings,
    Help,
    /// The notifications, over the view.
    Notes,
}

/// What picking a line of the palette does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cmd {
    Go(View),
    Settings,
    Help,
    Notes,
    MarkAll,
    /// An issue, PR or discussion (its node id).
    Item(String),
}

pub struct App {
    pub view: View,
    pub mode: Mode,
    pub prefs: Prefs,
    /// The language in force (`en`, `ja`), from common.toml or the OS.
    pub lang: String,
    /// Unread notifications, on the bell.
    pub unread: u32,
    /// When GitHub was last asked; `None` until the first sync.
    pub synced: Option<Instant>,
    /// The rate limit's remaining and total, from the last answer.
    pub api: Option<(u32, u32)>,
    /// One line at the foot, until `toast_until`.
    pub toast: Option<(String, Instant)>,
    /// The login the token belongs to, once GitHub has said.
    pub viewer: Option<String>,
    /// The open issues, PRs and discussions of the last search.
    pub items: Vec<Item>,
    /// The inbox's selection and filters.
    pub inbox: InboxState,
    /// The local marks (done, later) of the inbox; they never reach GitHub.
    pub marks: HashMap<String, Mark>,
    /// Where changed marks go to be written (see `keep_marks`).
    marks_tx: Option<std::sync::mpsc::Sender<String>>,
    /// minato's own settings: the inbox rules and the writing box's fields.
    pub settings: Settings,
    settings_tx: Option<std::sync::mpsc::Sender<String>>,
    /// The list shows closed issues and PRs (`C`), so `x` opens them again.
    pub show_closed: bool,
    /// The list view's selection.
    pub list: ListState,
    /// The detail of one item (its node id), once fetched.
    pub detail: Option<(String, Detail)>,
    /// A URL to open in the browser, taken by the window.
    pub open_url: Option<String>,
    /// The writing box, while it is open.
    pub compose: Option<Compose>,
    /// The item (node id) a merge waits to be confirmed for.
    pub confirm: Option<String>,
    /// What the confirmation is for.
    pub confirm_act: Confirm,
    /// The workflow runs of the repositories, newest first.
    pub runs: Vec<Run>,
    /// The Actions view's selection and filters.
    pub acts: ActState,
    /// The jobs and artifacts of one run, once fetched.
    pub run_detail: Option<RunDetail>,
    /// The dispatch form or a job's log, while one is open.
    pub overlay: Option<Overlay>,
    /// Project "uchmk" with its Status columns, once fetched.
    pub board: Option<Project>,
    pub bs: BoardState,
    /// Moves that can be undone: the Project item and the option it came from.
    pub board_undo: Vec<(String, String)>,
    /// Set when the board should be fetched again at once.
    /// Calls the window has asked for; the net takes them.
    pub outbox: Vec<Outgoing>,
    pub quit: bool,
    /// The unread notifications.
    pub notes: Vec<Note>,
    pub ns: NotesState,
    /// The palette, while it is open.
    pub palette: Option<ito_palette::View>,
}

/// What a confirmation box asks about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confirm {
    Merge,
    Close,
}

/// What GitHub said about one run.
#[derive(Default)]
pub struct RunDetail {
    pub id: u64,
    /// The workflow file at the run's commit (the `needs:` graph, the inputs).
    pub workflow: Option<Workflow>,
    pub jobs: Option<Vec<Job>>,
    pub artifacts: Option<Vec<Artifact>>,
}

/// One call for the worker, and the words to say when it has gone through.
pub struct Outgoing {
    pub key: String,
    pub call: Call,
    pub done: Option<&'static str>,
}

/// Which list the picker is choosing from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    Labels,
    Assignees,
    /// The Project's Status or Priority of the item (one choice).
    Status,
    Priority,
    Milestone,
    Reviewers,
}

/// A thread that writes the last text it is sent to `path`.
fn writer(path: std::path::PathBuf) -> std::sync::mpsc::Sender<String> {
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        while let Ok(mut text) = rx.recv() {
            while let Ok(newer) = rx.try_recv() {
                text = newer;
            }
            let _ = ito_common::write_atomic(&path, text);
        }
    });
    tx
}

/// The writing box: a comment on one item, with the labels and the
/// assignees to change from the same place.
pub struct Compose {
    pub id: String,
    pub text: String,
    pub preview: bool,
    pub picker: Option<(Pick, Picker)>,
    /// The repository's labels and assignable people, once GitHub has said.
    pub labels: Vec<String>,
    pub people: Vec<String>,
    /// The repository's open milestones, `(number, title)`.
    pub milestones: Vec<(u64, String)>,
}

impl App {
    pub fn new(prefs: Prefs) -> Self {
        let mut app = Self {
            view: View::Inbox,
            mode: Mode::Main,
            prefs,
            lang: "en".into(),
            unread: 0,
            notes: Vec::new(),
            ns: NotesState::default(),
            palette: None,
            synced: None,
            api: None,
            toast: None,
            viewer: None,
            items: Vec::new(),
            inbox: InboxState::default(),
            marks: HashMap::new(),
            marks_tx: None,
            settings: Settings::default(),
            settings_tx: None,
            show_closed: false,
            list: ListState::default(),
            detail: None,
            open_url: None,
            compose: None,
            confirm: None,
            confirm_act: Confirm::Merge,
            runs: Vec::new(),
            acts: ActState::default(),
            run_detail: None,
            overlay: None,
            board: None,
            bs: BoardState::default(),
            board_undo: Vec::new(),
            outbox: Vec::new(),
            quit: false,
        };
        app.set_language();
        app
    }

    /// The language common.toml names, else the OS's.
    pub fn set_language(&mut self) {
        let os = ito_common::os_language();
        self.lang = ito_i18n::resolve(None, self.prefs.common.language.as_deref(), os.as_deref(), &crate::i18n::AVAILABLE);
    }

    pub fn ja(&self) -> bool {
        self.lang == "ja"
    }

    /// The words for `key` in the language in force.
    pub fn tr(&self, key: &'static str) -> &'static str {
        crate::i18n::tr(&self.lang, key)
    }

    pub fn say(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), Instant::now() + Duration::from_secs(4)));
    }

    /// How long until the toast goes, if there is one.
    pub fn toast_left(&self) -> Option<Duration> {
        self.toast.as_ref().map(|(_, until)| until.saturating_duration_since(Instant::now()))
    }

    /// A character typed (not a key with Ctrl or Alt: those are the
    /// shortcuts). Returns whether it was used.
    pub fn feed(&mut self, c: char) -> bool {
        if self.compose.is_some() || self.confirm.is_some() || self.palette.is_some() || self.overlay.is_some() {
            return true;
        }
        match (self.mode, c) {
            (Mode::Notes, c) => return self.notes_key(c),
            (Mode::Help, '?') => self.mode = Mode::Main,
            (Mode::Settings, _) | (Mode::Help, _) => return false,
            (Mode::Main, '?') => self.mode = Mode::Help,
            (Mode::Main, ',') => self.mode = Mode::Settings,
            (Mode::Main, 'n') => self.mode = Mode::Notes,
            (Mode::Main, 'S') if matches!(self.view, View::Inbox | View::List) => self.repo_settings(),
            (Mode::Main, 'M') if matches!(self.view, View::Inbox | View::List) => self.mark_answer(),
            (Mode::Main, 'N') if matches!(self.view, View::Inbox | View::List) => self.new_issue(),
            (Mode::Main, c) => match View::from_digit(c) {
                Some(v) => self.view = v,
                None if self.view == View::List => return self.list_key(c),
                None if self.view == View::Inbox => return self.inbox_key(c),
                None if self.view == View::Actions => return self.actions_key(c),
                None if self.view == View::Board => return self.board_key(c),
                None => return false,
            },
        }
        true
    }

    /// A key in the list view: the columns' own, and `o` for the browser.
    fn list_key(&mut self, c: char) -> bool {
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
            'x' if self.show_closed => {
                self.reopen();
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
        if c == 'o' {
            if let Some(i) = self.list.selected(&self.items, &me) {
                self.open_url = Some(i.url.clone());
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
            }
            return true;
        }
        if self.mode == Mode::Notes {
            return self.notes_key('\n');
        }
        self.mode == Mode::Main && match self.view {
            View::List => self.list_key(' '),
            View::Actions => self.actions_key(' '),
            View::Board => self.board_key('\n'),
            _ => false,
        }
    }

    /// The run under the cursor of the Actions view.
    pub fn run(&self) -> Option<&Run> {
        self.acts.selected(&self.runs)
    }

    /// A key in the Actions view: `R` runs the failed jobs again, `X` stops,
    /// `D` starts the workflow by hand, `e` opens the failed job's log, `p`
    /// the pull request, `o` the run; the rest moves the cursor.
    fn actions_key(&mut self, c: char) -> bool {
        let Some(run) = self.run().cloned() else { return self.acts.key(c, &self.runs) };
        if self.acts.col == actions::Col::Filters && c != 'e' && c != 'o' && c != 'p' {
            return self.acts.key(c, &self.runs);
        }
        match c {
            'R' if run.status == actions::Status::Bad => self.queue(&format!("write:{}:rerun", run.id), actions::rerun_failed(&run), Some("run.reran")),
            'R' => self.say_key("run.not_failed"),
            'X' if run.status.active() => self.queue(&format!("write:{}:cancel", run.id), actions::cancel(&run), Some("run.cancelled")),
            'X' => self.say_key("run.not_active"),
            'D' => {
                let inputs = self.workflow_of(&run).map(|w| w.inputs.clone()).unwrap_or_default();
                if inputs.is_empty() {
                    self.queue(&format!("write:{}:dispatch", run.id), actions::dispatch(&run), Some("run.dispatched"));
                } else {
                    self.overlay = Some(Overlay::Form(Box::new(Form::new(run, inputs))));
                }
            }
            'L' => self.open_log(&run),
            'e' => {
                let failed = self.run_detail.as_ref().filter(|d| d.id == run.id).and_then(|d| d.jobs.as_ref()).and_then(|j| j.iter().find(|j| j.status == actions::Status::Bad));
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

    /// The workflow file of the run, once read.
    pub fn workflow_of(&self, run: &Run) -> Option<&Workflow> {
        self.run_detail.as_ref().filter(|d| d.id == run.id).and_then(|d| d.workflow.as_ref())
    }

    /// `L`: the log of the failed job (else the last one), in the app.
    fn open_log(&mut self, run: &Run) {
        let jobs = self.run_detail.as_ref().filter(|d| d.id == run.id).and_then(|d| d.jobs.as_ref());
        let Some(job) = jobs.and_then(|j| j.iter().find(|j| j.status == actions::Status::Bad).or(j.last())).cloned() else {
            return self.say_key("run.no_jobs");
        };
        self.queue(&format!("a-log:{}", job.id), Call::Get(workflow::log_path(run, &job)), None);
        self.overlay = Some(Overlay::Log(LogView::new(job.id, job.name)));
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
    fn board_key(&mut self, c: char) -> bool {
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
            _ => return self.bs.key(c, p),
        }
        true
    }

    /// A key over the notifications: `j k` move, `Enter` opens the thread and
    /// marks it read, `d` marks it read, `A` marks all, `n` closes.
    fn notes_key(&mut self, c: char) -> bool {
        match c {
            'n' => self.mode = Mode::Main,
            'A' => self.mark_all_read(),
            'd' | 'o' | '\n' => {
                if self.ns.sel < self.notes.len() {
                    let note = self.notes.remove(self.ns.sel);
                    if c != 'd' {
                        self.open_url = Some(note.url);
                    }
                    self.queue(&format!("note:{}", note.id), notes::mark_read(&note.id), None);
                    self.unread = self.notes.len() as u32;
                    self.ns.clamp(self.notes.len());
                }
            }
            _ => return self.ns.key(c, self.notes.len()),
        }
        true
    }

    pub fn mark_all_read(&mut self) {
        if self.notes.is_empty() {
            return;
        }
        self.notes.clear();
        self.unread = 0;
        self.ns = NotesState::default();
        self.queue("note:all", notes::mark_all(), Some("notes.all_read"));
    }

    /// The lines of the palette: the commands, then the items (`#12 title`).
    pub fn palette_entries(&self) -> Vec<ito_palette::Entry<Cmd>> {
        use ito_palette::Entry;
        let mut e = Vec::new();
        for v in View::ALL {
            e.push(Entry::new(format!("{} {}", self.tr("cmd.go"), self.tr(v.label())), v.digit().to_string(), Cmd::Go(v)));
        }
        e.push(Entry::new(self.tr("key.notes"), "n", Cmd::Notes));
        e.push(Entry::new(self.tr("cmd.mark_all"), "A", Cmd::MarkAll));
        e.push(Entry::new(self.tr("key.settings"), "Ctrl+,", Cmd::Settings));
        e.push(Entry::new(self.tr("key.help"), "?", Cmd::Help));
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
            Cmd::Go(v) => {
                self.mode = Mode::Main;
                self.view = v;
            }
            Cmd::Settings => self.mode = Mode::Settings,
            Cmd::Help => self.mode = Mode::Help,
            Cmd::Notes => self.mode = Mode::Notes,
            Cmd::MarkAll => self.mark_all_read(),
            Cmd::Item(id) => {
                self.mode = Mode::Main;
                self.show_in_list(&id);
            }
        }
    }

    /// A card dropped on a column (the mouse): the same write as `H` `L`.
    pub fn drop_card(&mut self, card: usize, col: usize) {
        let Some(p) = self.board.as_mut() else { return };
        let Some(option) = p.columns().get(col).and_then(|c| c.option.clone()) else { return };
        if p.cards.get(card).is_none_or(|k| k.option.as_ref() == Some(&option)) {
            return;
        }
        let (item, before) = (p.cards[card].item.clone(), p.cards[card].option.clone());
        if let Some(call) = p.set_option(card, &option) {
            self.bs.follow(p, card);
            if let Some(b) = before {
                self.board_undo.push((item.clone(), b));
            }
            self.queue(&format!("board:{}:{}", item, self.outbox.len()), call, Some("board.moved"));
        }
    }

    /// The cursor put on a card by a click.
    pub fn pick_card(&mut self, card: usize) {
        if let Some(p) = self.board.as_ref() {
            self.bs.follow(p, card);
        }
    }

    /// Open the list view on one item.
    fn show_in_list(&mut self, id: &str) {
        let me = self.viewer.clone().unwrap_or_default();
        self.list.filter = Default::default();
        self.list.col = minato_core::list::Col::Items;
        if let Some(k) = self.list.shown(&self.items, &me).iter().position(|i| i.id == id) {
            self.list.sel = k;
            self.view = View::List;
        } else {
            self.say_key("board.not_listed");
        }
    }

    fn say_key(&mut self, key: &'static str) {
        let words = self.tr(key);
        self.say(words);
    }

    /// Every card of the inbox now, before its filters.
    pub fn inbox_all(&self) -> Vec<Card> {
        let me = self.viewer.as_deref().unwrap_or("");
        minato_core::inbox::cards(&self.items, me, &self.settings.rules, &self.marks, now_epoch())
    }

    /// A key in the inbox: `d` done, `s` later (tomorrow 9:00), `u` undo,
    /// `o` the browser, and the filters and the cursor.
    fn inbox_key(&mut self, c: char) -> bool {
        let all = self.inbox_all();
        let card = self.inbox.selected(&all, &self.items);
        match c {
            'r' => self.open_compose(),
            'm' => self.ask_merge(),
            'x' => self.ask_close(),
            'a' => self.assign_self(),
            'o' => {
                if let Some(card) = card {
                    self.open_url = Some(self.items[card.at].url.clone());
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

    /// Keep the marks in `path`: read them now (at start-up), and write
    /// every change on a thread of its own.
    pub fn keep_marks(&mut self, path: std::path::PathBuf) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            self.marks = minato_core::inbox::marks_parse(&text);
        }
        self.marks_tx = Some(writer(path));
    }

    /// Keep minato's settings in `path`, the same way.
    pub fn keep_settings(&mut self, path: std::path::PathBuf) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            self.settings = Settings::parse(&text);
        }
        self.settings_tx = Some(writer(path));
    }

    /// Write the settings after the screen changed them.
    pub fn save_settings(&self) {
        if let Some(tx) = &self.settings_tx {
            let _ = tx.send(self.settings.to_json());
        }
    }

    /// Whether the writing box offers field `f` (0 Labels .. 5 Reviewers);
    /// says so in a toast when the settings hide it.
    pub fn field_shown(&mut self, f: usize) -> bool {
        let on = self.settings.fields[f];
        if !on {
            self.say_key("pick.hidden");
        }
        on
    }

    fn save_marks(&self) {
        if let Some(tx) = &self.marks_tx {
            let _ = tx.send(minato_core::inbox::marks_json(&self.marks));
        }
    }

    fn queue(&mut self, key: &str, call: Call, done: Option<&'static str>) {
        self.outbox.push(Outgoing { key: key.to_string(), call, done });
    }

    fn item(&self, id: &str) -> Option<Item> {
        self.items.iter().find(|i| i.id == id).cloned()
    }

    /// The selected item, unless it is a stub for a repository (red main CI):
    /// nothing can be written to that.
    fn writable(&mut self) -> Option<Item> {
        let item = self.selected().cloned()?;
        if minato_core::mainci::is_stub(&item) {
            self.say(self.tr("write.not_real").to_string());
            return None;
        }
        Some(item)
    }

    /// `r`: open the writing box on the selected item and ask for the
    /// repository's labels and people.
    pub fn open_compose(&mut self) {
        let Some(item) = self.writable() else { return };
        self.queue(&format!("labels:{}", item.repo), Call::Get(write::labels_path(&item.repo)), None);
        self.queue(&format!("people:{}", item.repo), Call::Get(write::assignees_path(&item.repo)), None);
        self.queue(&format!("miles:{}", item.repo), Call::Get(write::milestones_path(&item.repo)), None);
        self.compose = Some(Compose { id: item.id, text: String::new(), preview: false, picker: None, labels: Vec::new(), people: Vec::new(), milestones: Vec::new() });
    }

    /// `M`: mark the newest comment of the selected discussion that is not
    /// the asker's as its answer.
    fn mark_answer(&mut self) {
        let Some(item) = self.selected().cloned() else { return };
        if item.kind != Kind::Discussion || minato_core::mainci::is_stub(&item) {
            self.say_key("write.not_discussion");
            return;
        }
        let entry = self.detail.as_ref().filter(|(id, _)| *id == item.id).and_then(|(_, d)| d.timeline.iter().rev().find(|e| e.author != item.author && !e.id.is_empty()));
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

    /// `S`: show the settings of the selected item's repository (read only).
    fn repo_settings(&mut self) {
        let Some(repo) = self.selected().or(self.items.first()).map(|i| i.repo.clone()) else { return };
        self.queue(&format!("a-repo:{repo}"), Call::Get(minato_core::reposettings::path(&repo)), None);
        self.overlay = Some(Overlay::Log(LogView::new(0, format!("{repo}  settings"))));
    }

    /// `N`: open the writing box for a new issue in the selected item's
    /// repository (the first one listed when nothing is selected).
    fn new_issue(&mut self) {
        let Some(repo) = self.selected().or(self.items.first()).map(|i| i.repo.clone()) else { return };
        self.compose = Some(Compose { id: format!("new:{repo}"), text: String::new(), preview: false, picker: None, labels: Vec::new(), people: Vec::new(), milestones: Vec::new() });
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
        let n = self.outbox.len();
        if !c.text.trim().is_empty() {
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
            Pick::Assignees => self.queue(&format!("write:{n}:people"), write::set_assignees(&item, &picker.chosen), Some("write.assignees")),
            Pick::Milestone => {
                let number = picker.chosen.first().and_then(|t| self.compose.as_ref()?.milestones.iter().find(|(_, m)| m == t).map(|(n, _)| *n));
                self.queue(&format!("write:{n}:milestone"), write::set_milestone(&item, number), Some("write.milestone"));
            }
            Pick::Reviewers => self.queue(&format!("write:{n}:reviewers"), write::request_reviewers(&item, &picker.chosen), Some("write.reviewers")),
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

    /// Alt+M (milestone, one choice) or Alt+R (reviewers, PRs only).
    pub fn open_extra_picker(&mut self, reviewers: bool) {
        let Some(c) = self.compose.as_ref() else { return };
        let is_pr = self.item(&c.id).is_some_and(|i| i.kind == Kind::Pr);
        if reviewers && !is_pr {
            self.say_key("write.not_pr");
            return;
        }
        let Some(c) = self.compose.as_mut() else { return };
        c.picker = Some(if reviewers {
            (Pick::Reviewers, Picker::new(c.people.clone(), Vec::new()))
        } else {
            (Pick::Milestone, Picker::single(c.milestones.iter().map(|(_, t)| t.clone()).collect(), Vec::new()))
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
    fn ask_merge(&mut self) {
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
    fn ask_close(&mut self) {
        match self.selected() {
            Some(i) if i.kind != Kind::Discussion && !i.merged && !minato_core::mainci::is_stub(i) => {
                self.confirm = Some(i.id.clone());
                self.confirm_act = Confirm::Close;
            }
            Some(_) => self.say_key("write.not_closable"),
            None => {}
        }
    }

    /// `x` over closed items: open the selected issue or PR again.
    fn reopen(&mut self) {
        match self.selected() {
            Some(i) if i.kind != Kind::Discussion && !i.merged && !minato_core::mainci::is_stub(i) => {
                let n = self.outbox.len();
                self.queue(&format!("write:{n}:state"), write::set_state(i, true), Some("write.reopened"));
            }
            Some(_) => self.say_key("write.not_closable"),
            None => {}
        }
    }

    fn close(&mut self, id: &str) {
        if let Some(item) = self.item(id) {
            let n = self.outbox.len();
            self.queue(&format!("write:{n}:state"), write::set_state(&item, false), Some("write.closed"));
        }
    }

    fn merge(&mut self, id: &str) {
        if let Some(item) = self.item(id) {
            let n = self.outbox.len();
            self.queue(&format!("write:{n}:merge"), write::merge(&item), Some("write.merged"));
        }
    }

    /// `a`: assign the selected item to the viewer.
    fn assign_self(&mut self) {
        let (Some(item), Some(me)) = (self.selected().cloned(), self.viewer.clone()) else { return };
        if item.kind == Kind::Discussion || minato_core::mainci::is_stub(&item) {
            return;
        }
        let n = self.outbox.len();
        self.queue(&format!("write:{n}:assign"), write::assign(&item, &me), Some("write.assigned"));
    }

    /// The item shown in the detail: the list's or the inbox's selection.
    pub fn selected(&self) -> Option<&Item> {
        if self.view == View::Inbox {
            let all = self.inbox_all();
            return self.inbox.selected(&all, &self.items).map(|c| &self.items[c.at]);
        }
        self.list.selected(&self.items, self.viewer.as_deref().unwrap_or(""))
    }

    /// Escape: close the help or the settings.
    pub fn escape(&mut self) -> bool {
        if self.confirm.take().is_some() {
            return true;
        }
        if self.overlay.take().is_some() {
            return true;
        }
        let was = self.mode;
        self.mode = Mode::Main;
        was != Mode::Main
    }

    /// Whether the bar's tab for `v` is the one in force.
    pub fn is_on(&self, v: View) -> bool {
        self.mode == Mode::Main && self.view == v
    }

    /// Seconds since the last sync.
    pub fn since_sync(&self) -> Option<u64> {
        self.synced.map(|t| t.elapsed().as_secs())
    }
}

/// Seconds since the Unix epoch.
pub fn now_epoch() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Tomorrow 9:00 local time, as seconds since the epoch.
fn tomorrow_nine() -> i64 {
    use chrono::{Duration, Local, TimeZone};
    let day = Local::now().date_naive() + Duration::days(1);
    let at = day.and_hms_opt(9, 0, 0).unwrap_or_default();
    Local.from_local_datetime(&at).earliest().map_or_else(|| now_epoch() + 12 * 3600, |t| t.timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        App::new(Prefs::empty())
    }

    fn run(id: u64, status: actions::Status, pr: Option<u64>) -> Run {
        Run {
            id,
            repo: "uchmk/kura".into(),
            workflow_id: 5,
            workflow: "check".into(),
            number: id,
            title: "t".into(),
            event: "push".into(),
            branch: "main".into(),
            sha: "abc".into(),
            actor: "u".into(),
            status,
            started_at: "2026-10-12T09:00:00Z".into(),
            updated_at: "2026-10-12T09:01:00Z".into(),
            url: format!("https://github.com/uchmk/kura/actions/runs/{id}"),
            pr,
            path: ".github/workflows/check.yml".into(),
            head_sha: "abc0000".into(),
        }
    }

    #[test]
    fn actions_keys_rerun_cancel_and_open() {
        let mut a = app();
        a.feed('3');
        a.runs = vec![run(1, actions::Status::Bad, Some(9)), run(2, actions::Status::Running, None)];
        assert!(a.feed('R'));
        assert_eq!(a.outbox.len(), 1);
        assert_eq!(a.outbox[0].key, "write:1:rerun");
        // Only a going run can be stopped.
        assert!(a.feed('X'));
        assert_eq!(a.outbox.len(), 1);
        assert!(a.feed('p'));
        assert_eq!(a.open_url.take().as_deref(), Some("https://github.com/uchmk/kura/pull/9"));
        assert!(a.feed('j'));
        assert!(a.feed('X'));
        assert_eq!(a.outbox[1].key, "write:2:cancel");
        assert!(a.feed('R'));
        assert_eq!(a.outbox.len(), 2, "a running run is not re-run");
        assert!(a.feed('D'));
        assert_eq!(a.outbox[2].key, "write:2:dispatch");
        assert!(a.feed('e'));
        assert!(a.open_url.take().unwrap().ends_with("/runs/2"));
        assert!(a.feed('p'));
        assert!(a.open_url.is_none(), "no pull request to open");
    }

    #[test]
    fn actions_filters_take_the_keys_in_their_column() {
        let mut a = app();
        a.feed('3');
        a.runs = vec![run(1, actions::Status::Bad, None)];
        assert!(a.feed('h'));
        assert!(!a.feed('R'), "the filter column ignores it");
        assert!(a.outbox.is_empty(), "R does nothing in the filter column");
    }

    fn board_with_two_cards() -> board::Project {
        let col = |id: &str, name: &str| board::Column { option: Some(id.into()), name: name.into() };
        let card = |item: &str, option: &str| board::Card {
            item: item.into(),
            id: format!("N-{item}"),
            repo: "uchmk/kura".into(),
            number: 1,
            title: "t".into(),
            url: format!("https://example.test/{item}"),
            pr: false,
            labels: Vec::new(),
            option: Some(option.into()),
            priority: None,
        };
        board::Project { id: "P".into(), title: "uchmk".into(), field: "F".into(), options: vec![col("o1", "Todo"), col("o2", "Doing")], priority_field: "PF".into(), priority_options: vec![col("p1", "High")], cards: vec![card("I1", "o1"), card("I2", "o1")] }
    }

    #[test]
    fn the_board_moves_a_card_writes_and_undoes() {
        let mut a = app();
        a.feed('4');
        assert!(!a.feed('L'), "no board yet");
        a.board = Some(board_with_two_cards());
        assert!(a.feed('L'));
        assert_eq!(a.board.as_ref().unwrap().cards[0].option.as_deref(), Some("o2"));
        assert_eq!(a.outbox.len(), 1);
        assert!(a.outbox[0].key.starts_with("board:I1:"));
        assert_eq!(a.bs.col, 1);
        assert!(a.feed('u'));
        assert_eq!(a.board.as_ref().unwrap().cards[0].option.as_deref(), Some("o1"));
        assert_eq!(a.outbox.len(), 2);
        assert!(a.feed('u'), "nothing left to undo is still the board's key");
        assert_eq!(a.outbox.len(), 2);
    }

    #[test]
    fn a_dropped_card_goes_to_its_column_once() {
        let mut a = app();
        a.feed('4');
        a.board = Some(board_with_two_cards());
        a.drop_card(1, 1);
        assert_eq!(a.board.as_ref().unwrap().cards[1].option.as_deref(), Some("o2"));
        assert_eq!(a.outbox.len(), 1);
        a.drop_card(1, 1);
        assert_eq!(a.outbox.len(), 1, "dropping on its own column writes nothing");
        a.drop_card(9, 0);
        assert_eq!(a.outbox.len(), 1);
    }

    #[test]
    fn board_o_opens_the_card_in_the_browser() {
        let mut a = app();
        a.feed('4');
        a.board = Some(board_with_two_cards());
        assert!(a.feed('o'));
        assert_eq!(a.open_url.as_deref(), Some("https://example.test/I1"));
    }

    fn note(id: &str) -> Note {
        Note { id: id.into(), repo: "o/r".into(), title: "t".into(), kind: "Issue".into(), reason: "mention".into(), updated_at: String::new(), url: format!("https://example.test/{id}") }
    }

    #[test]
    fn n_opens_the_notifications_and_enter_reads_one() {
        let mut a = app();
        a.notes = vec![note("1"), note("2")];
        a.unread = 2;
        assert!(a.feed('n'));
        assert_eq!(a.mode, Mode::Notes);
        assert!(a.feed('j'));
        assert!(a.enter());
        assert_eq!(a.open_url.as_deref(), Some("https://example.test/2"));
        assert_eq!((a.notes.len(), a.unread), (1, 1));
        assert!(a.outbox[0].key.ends_with("note:2"));
        assert!(a.feed('n'));
        assert_eq!(a.mode, Mode::Main);
    }

    #[test]
    fn capital_a_marks_everything_read_once() {
        let mut a = app();
        a.notes = vec![note("1")];
        a.unread = 1;
        a.feed('n');
        a.feed('A');
        a.feed('A');
        assert!(a.notes.is_empty());
        assert_eq!((a.unread, a.outbox.len()), (0, 1));
    }

    #[test]
    fn the_palette_lists_commands_and_runs_one() {
        let mut a = app();
        let e = a.palette_entries();
        assert!(e.iter().any(|x| x.pick == Cmd::Go(View::Board)));
        a.run_cmd(Cmd::Go(View::Board));
        assert_eq!(a.view, View::Board);
        a.run_cmd(Cmd::Notes);
        assert_eq!(a.mode, Mode::Notes);
        a.palette = Some(ito_palette::View::new());
        assert!(a.feed('q'));
        assert!(!a.quit);
    }

    #[test]
    fn alt_s_picks_a_status_for_an_item_in_the_project() {
        let mut a = with_pr();
        a.board = Some(board_with_two_cards());
        a.board.as_mut().unwrap().cards[0].id = a.items[0].id.clone();
        a.feed('r');
        a.outbox.clear();
        a.open_field_picker(false);
        let c = a.compose.as_mut().unwrap();
        let (kind, pk) = c.picker.as_mut().unwrap();
        assert_eq!(*kind, Pick::Status);
        pk.step(true);
        pk.toggle();
        a.apply_pick();
        assert!(a.outbox[0].key.starts_with("board:I1:"));
        assert_eq!(a.board.as_ref().unwrap().cards[0].option.as_deref(), Some("o2"));
    }

    #[test]
    fn alt_p_says_so_when_the_item_is_not_in_the_project() {
        let mut a = with_pr();
        a.board = Some(board_with_two_cards());
        a.feed('r');
        a.open_field_picker(true);
        assert!(a.compose.as_ref().unwrap().picker.is_none() && a.toast.is_some());
    }

    #[test]
    fn milestone_and_reviewers_are_written_from_their_pickers() {
        let mut a = with_pr();
        a.feed('r');
        let c = a.compose.as_mut().unwrap();
        c.milestones = vec![(4, "v1".into()), (5, "v2".into())];
        c.people = vec!["amy".into(), "bob".into()];
        a.outbox.clear();
        a.open_extra_picker(false);
        let (kind, pk) = a.compose.as_mut().unwrap().picker.as_mut().unwrap();
        assert_eq!(*kind, Pick::Milestone);
        pk.step(true);
        pk.toggle();
        a.apply_pick();
        let Call::Send { path, body, .. } = &a.outbox[0].call else { panic!() };
        assert!(path.ends_with("/issues/3") && body.contains("\"milestone\":5"), "{path} {body}");
        a.open_extra_picker(true);
        let (kind, pk) = a.compose.as_mut().unwrap().picker.as_mut().unwrap();
        assert_eq!(*kind, Pick::Reviewers);
        pk.toggle();
        a.apply_pick();
        let Call::Send { path, body, .. } = &a.outbox[1].call else { panic!() };
        assert!(path.ends_with("/pulls/3/requested_reviewers") && body.contains("amy"), "{path} {body}");
    }

    #[test]
    fn hidden_fields_say_so_and_settings_are_read_back() {
        let mut a = app();
        assert!(a.field_shown(0));
        assert!(!a.field_shown(3) && a.toast.is_some());
        let dir = std::env::temp_dir().join(format!("minato-settings-{}", std::process::id()));
        let file = dir.join("s.json");
        a.keep_settings(file.clone());
        a.settings.fields[3] = true;
        a.settings.rules.0[6] = true;
        a.save_settings();
        for _ in 0..100 {
            if file.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let mut b = app();
        b.keep_settings(file);
        assert_eq!(b.settings, a.settings);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn x_reopens_over_closed_items() {
        let mut a = with_pr();
        a.feed('2');
        assert!(a.feed('C') && a.show_closed);
        assert!(a.feed('x'));
        assert!(a.confirm.is_none(), "reopening asks nothing");
        assert!(a.outbox.last().is_some_and(|o| o.key.ends_with(":state")));
        assert!(a.feed('C') && !a.show_closed);
    }

    #[test]
    fn reviewers_are_for_prs_only() {
        let mut a = with_pr();
        a.items[0].kind = Kind::Issue;
        a.feed('r');
        a.open_extra_picker(true);
        assert!(a.compose.as_ref().unwrap().picker.is_none() && a.toast.is_some());
    }

    #[test]
    fn marks_are_read_back_after_a_restart() {
        let dir = std::env::temp_dir().join(format!("minato-marks-{}", std::process::id()));
        let file = dir.join("m.json");
        let mut a = with_pr();
        a.view = View::Inbox;
        a.keep_marks(file.clone());
        a.feed('d');
        let want = a.marks.clone();
        assert_eq!(want.len(), 1);
        for _ in 0..100 {
            if file.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let mut b = app();
        b.keep_marks(file);
        assert_eq!(b.marks, want);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn digits_open_the_views() {
        let mut a = app();
        assert_eq!(a.view, View::Inbox);
        for (c, v) in [('2', View::List), ('3', View::Actions), ('4', View::Board), ('1', View::Inbox)] {
            assert!(a.feed(c));
            assert_eq!(a.view, v);
        }
        assert!(!a.feed('5'));
        assert!(!a.feed('z'));
    }

    #[test]
    fn comma_and_question_mark_open_and_escape_closes() {
        let mut a = app();
        assert!(a.feed(','));
        assert_eq!(a.mode, Mode::Settings);
        // In the settings the screen's own controls take the characters.
        assert!(!a.feed('3'));
        assert_eq!(a.view, View::Inbox);
        assert!(a.escape());
        assert_eq!(a.mode, Mode::Main);
        assert!(!a.escape());
        assert!(a.feed('?'));
        assert_eq!(a.mode, Mode::Help);
        assert!(!a.feed('3'), "the help swallows the views' digits");
        assert!(a.feed('?'));
        assert_eq!(a.mode, Mode::Main);
    }

    #[test]
    fn list_keys_work_only_in_the_list_view() {
        let mut a = app();
        assert!(!a.feed('h'), "no list keys in the inbox");
        a.feed('2');
        assert!(a.feed('h'));
        assert_eq!(a.list.col, minato_core::list::Col::Filters);
        assert!(a.feed('l'));
        assert!(a.feed('o'), "o with nothing selected is used and does nothing");
        assert!(a.open_url.is_none());
    }

    #[test]
    fn the_tab_in_force_is_the_view_only_in_the_main_mode() {
        let mut a = app();
        assert!(a.is_on(View::Inbox));
        a.feed(',');
        assert!(!a.is_on(View::Inbox));
    }

    #[test]
    fn every_view_has_keys_and_words_for_them() {
        for v in View::ALL {
            assert!(!v.keys().is_empty());
            for (_, w) in v.keys() {
                assert_ne!(crate::i18n::tr("en", w), *w, "no words for {w}");
            }
            assert_ne!(crate::i18n::tr("en", v.label()), v.label());
            assert_ne!(crate::i18n::tr("en", v.empty()), v.empty());
        }
        for (_, w) in GLOBAL_KEYS {
            assert_ne!(crate::i18n::tr("en", w), w, "no words for {w}");
        }
    }

    fn pr(id: &str) -> Item {
        Item {
            id: id.into(),
            kind: Kind::Pr,
            repo: "uchmk/kura".into(),
            number: 3,
            title: "t".into(),
            url: String::new(),
            author: "uchmk".into(),
            labels: Vec::new(),
            assignees: Vec::new(),
            comments: 0,
            created_at: String::new(),
            updated_at: String::new(),
            draft: false,
            merged: false,
            conflict: false,
            mentioned: false,
            ci: None,
        }
    }

    fn with_pr() -> App {
        let mut a = app();
        a.view = View::List;
        a.viewer = Some("uchmk".into());
        a.items = vec![pr("P1")];
        a
    }

    #[test]
    fn r_opens_the_box_and_asks_for_labels_and_people() {
        let mut a = with_pr();
        assert!(a.feed('r'));
        assert!(a.compose.is_some());
        let keys: Vec<&str> = a.outbox.iter().map(|o| o.key.as_str()).collect();
        assert_eq!(keys, ["labels:uchmk/kura", "people:uchmk/kura", "miles:uchmk/kura"]);
        // Typed characters do not reach the views while the box is open.
        assert!(a.feed('2'));
        assert_eq!(a.view, View::List);
    }

    #[test]
    fn sending_queues_a_comment_and_close_but_not_an_empty_one() {
        let mut a = with_pr();
        a.feed('r');
        a.outbox.clear();
        a.compose.as_mut().unwrap().text = "ok".into();
        a.send_comment(true);
        let keys: Vec<&str> = a.outbox.iter().map(|o| o.key.as_str()).collect();
        assert_eq!(keys, ["write:0:comment", "write:0:state"]);
        assert!(a.compose.is_none());
        a.outbox.clear();
        a.feed('r');
        a.outbox.clear();
        a.send_comment(false);
        assert!(a.outbox.is_empty());
    }

    #[test]
    fn merge_waits_for_enter_and_escape_cancels() {
        let mut a = with_pr();
        a.view = View::List;
        a.ask_merge();
        assert_eq!(a.confirm.as_deref(), Some("P1"));
        assert!(a.escape());
        assert!(a.outbox.is_empty());
        a.ask_merge();
        assert!(a.enter());
        assert_eq!(a.outbox.len(), 1);
        assert!(a.outbox[0].key.ends_with(":merge"));
    }

    #[test]
    fn a_draft_or_an_issue_is_not_merged() {
        let mut a = with_pr();
        a.items[0].draft = true;
        a.ask_merge();
        assert!(a.confirm.is_none() && a.toast.is_some());
    }

    #[test]
    fn a_applies_the_picker_only_when_it_changed() {
        let mut a = with_pr();
        a.feed('r');
        a.outbox.clear();
        let c = a.compose.as_mut().unwrap();
        c.picker = Some((Pick::Labels, Picker::new(vec!["bug".into()], Vec::new())));
        a.apply_pick();
        assert!(a.outbox.is_empty());
        let c = a.compose.as_mut().unwrap();
        let mut pk = Picker::new(vec!["bug".into()], Vec::new());
        pk.toggle();
        c.picker = Some((Pick::Labels, pk));
        a.apply_pick();
        assert!(a.outbox[0].key.ends_with(":labels"));
    }

    fn with_detail(a: &mut App, yaml: &str) {
        a.feed('3');
        a.runs = vec![run(1, actions::Status::Bad, None)];
        a.acts.col = actions::Col::Runs;
        let job = |id, name: &str, status| actions::Job { id, name: name.into(), status, started_at: String::new(), completed_at: String::new(), url: String::new() };
        a.run_detail = Some(RunDetail { id: 1, jobs: Some(vec![job(10, "lint", actions::Status::Ok), job(11, "Test (linux)", actions::Status::Bad)]), workflow: Some(workflow::parse(yaml)), ..Default::default() });
    }

    #[test]
    fn d_opens_the_form_when_the_workflow_has_inputs_and_sends_them() {
        let mut a = app();
        with_detail(&mut a, "on:
  workflow_dispatch:
    inputs:
      who:
        required: true
jobs:
  lint:
    runs-on: x
");
        a.feed('D');
        assert!(matches!(a.overlay, Some(Overlay::Form(_))));
        assert!(a.outbox.is_empty());
        a.overlay_key(K::Send);
        assert!(a.overlay.is_some() && a.outbox.is_empty());
        a.overlay_key(K::Char('x'));
        a.overlay_key(K::Send);
        assert!(a.overlay.is_none());
        let Call::Send { body, .. } = &a.outbox[0].call else { panic!() };
        assert!(body.contains("\"inputs\"") && body.contains("\"who\":\"x\""), "{body}");
    }

    #[test]
    fn d_starts_at_once_without_inputs_and_l_opens_the_failed_log() {
        let mut a = app();
        with_detail(&mut a, "jobs:
  lint:
    runs-on: x
");
        a.feed('D');
        assert!(a.overlay.is_none());
        assert_eq!(a.outbox.len(), 1);
        a.feed('L');
        let Some(Overlay::Log(l)) = &a.overlay else { panic!() };
        assert_eq!(l.job_id, 11);
        assert_eq!(a.outbox[1].key, "a-log:11");
        assert!(a.escape() && a.overlay.is_none());
    }

    #[test]
    fn x_closes_only_after_enter_and_m_works_in_the_list() {
        let mut a = with_pr();
        assert!(a.feed('x'));
        assert_eq!((a.confirm.as_deref(), a.confirm_act), (Some("P1"), Confirm::Close));
        assert!(a.escape() && a.outbox.is_empty());
        a.feed('x');
        assert!(a.enter());
        assert_eq!(a.outbox.len(), 1);
        assert!(a.outbox[0].key.ends_with(":state"));
        a.feed('m');
        assert_eq!(a.confirm_act, Confirm::Merge);
        a.enter();
        assert!(a.outbox[1].key.ends_with(":merge"));
    }

    #[test]
    fn a_merged_item_is_not_closed() {
        let mut a = with_pr();
        a.items[0].merged = true;
        a.feed('x');
        assert!(a.confirm.is_none() && a.toast.is_some());
    }
}
