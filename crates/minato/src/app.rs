//! The app's state and what the keys do to it. Nothing here draws or reads
//! a file, so the keys can be tested without a window.

use std::time::{Duration, Instant};

use std::collections::HashMap;

use minato_core::actions::{self, ActState, Artifact, Job, Run};
use minato_core::board::{self, BoardState, Project};
use minato_core::composer::{Picker, Tool};
use minato_core::github::worker::Call;
use minato_core::github::write;
use minato_core::inbox::{Card, Group, InboxState, Mark, Status};
use minato_core::prefs::Settings;
use minato_core::list::{Detail, Item, Kind, ListState};
use minato_core::notes::{self, Note, NotesState};

use minato_core::workflow::{self, Workflow};

use ito_layout::{Dir, Node};

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

    /// The next view in 1 2 3 4 order (the one before with `back`), round
    /// from the last to the first.
    pub fn next(self, back: bool) -> View {
        let at = View::ALL.iter().position(|&v| v == self).unwrap_or(0);
        let n = View::ALL.len();
        View::ALL[if back { (at + n - 1) % n } else { (at + 1) % n }]
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
                ("O", "key.outside"),
                ("f F t", "key.filter"),
                ("/", "key.find"),
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
                ("O", "key.outside"),
                ("/", "key.find"),
            ],
            View::Actions => &[
                ("j k", "key.next"),
                ("R", "key.rerun"),
                ("X", "key.cancel"),
                ("D", "key.dispatch"),
                ("e", "key.logs"),
                ("L", "key.log"),
                ("o", "key.browser"),
                ("O", "key.outside"),
            ],
            View::Board => &[("h l", "key.columns"), ("j k", "key.next"), ("H L", "key.card"), ("Enter", "key.open"), ("u", "key.undo"), ("o", "key.browser"), ("O", "key.outside")],
        }
    }
}

/// The keys that work in every view.
pub const GLOBAL_KEYS: [(&str, &str); 9] = [
    ("1 2 3 4", "key.views"),
    ("Tab S-Tab", "key.cycle"),
    ("gg G", "key.ends"),
    ("Ctrl+,", "key.settings"),
    ("Ctrl+= - 0", "key.scale"),
    ("Ctrl+Shift+P", "key.palette"),
    ("n", "key.notes"),
    ("u", "key.undo"),
    ("F1 ?", "key.help"),
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

/// A column of a view, for the dividers the mouse drags (ito-layout).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pane {
    Left,
    Mid,
    Right,
}

/// Two columns: the left one's share of the width.
pub fn two(left: f32) -> Node<Pane> {
    Node::Split { dir: Dir::Right, ratio: left, first: Box::new(Node::Leaf(Pane::Left)), second: Box::new(Node::Leaf(Pane::Right)) }
}

/// Three columns: the left one's share, then the middle one's of the rest.
pub fn three(left: f32, mid: f32) -> Node<Pane> {
    let rest = Node::Split { dir: Dir::Right, ratio: mid, first: Box::new(Node::Leaf(Pane::Mid)), second: Box::new(Node::Leaf(Pane::Right)) };
    Node::Split { dir: Dir::Right, ratio: left, first: Box::new(Node::Leaf(Pane::Left)), second: Box::new(rest) }
}

/// A split's ratios, the outer one first (as state.toml keeps them).
pub fn ratios(n: &Node<Pane>) -> Vec<f32> {
    match n {
        Node::Split { ratio, first, second, .. } => {
            let mut out = vec![*ratio];
            out.extend(ratios(first));
            out.extend(ratios(second));
            out
        }
        Node::Leaf(_) => Vec::new(),
    }
}

/// Put `rs` back into a split in the same order; a list of another length
/// (from another version of the views) is left alone.
fn set_ratios(n: &mut Node<Pane>, rs: &[f32]) {
    fn walk(n: &mut Node<Pane>, rs: &mut std::slice::Iter<f32>) {
        if let Node::Split { ratio, first, second, .. } = n {
            if let Some(r) = rs.next() {
                *ratio = *r;
            }
            walk(first, rs);
            walk(second, rs);
        }
    }
    if ratios(n).len() == rs.len() {
        walk(n, &mut rs.iter());
    }
}

/// What the mouse clicked in a view: it does what the keys that get there do.
#[derive(Clone, Debug, PartialEq)]
pub enum Click {
    /// A line of the list's filters (its place among all of them).
    ListFilter(usize),
    /// A row of the list (its place among those shown).
    ListItem(usize),
    /// Somewhere else in a column of the list.
    ListCol(minato_core::list::Col),
    InboxCard(usize),
    InboxStatus(Status),
    InboxRepo(Option<String>),
    InboxTag(Option<String>),
    ActFilter(usize),
    ActRun(usize),
    ActCol(actions::Col),
    /// The writing box's strip under the detail.
    Write,
    /// A button of the box's toolbar.
    Tool(Tool),
    /// The box's Write (`false`) or Preview (`true`) tab.
    Tab(bool),
    /// Comment (`false`), or Close (with the comment if there is one).
    Send(bool),
}

impl Click {
    /// Whether it is the writing box's own, so the box stays open for it.
    pub fn is_compose(&self) -> bool {
        matches!(self, Click::Write | Click::Tool(_) | Click::Tab(_) | Click::Send(_))
    }
}

/// What the mouse wheel turned over: a list moves its selection (as `j k`),
/// a detail scrolls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wheel {
    /// A column of the list view (not its detail).
    List(minato_core::list::Col),
    /// The inbox's cards.
    Inbox,
    /// The detail of the inbox or the list.
    Detail,
    Acts(actions::Col),
    /// A column of the board.
    Board(usize),
    Notes,
}

/// What the mouse did in the notifications.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteClick {
    Open(usize),
    ReadAll,
    Close,
}

/// What picking a line of the palette does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cmd {
    Settings,
    Help,
    Notes,
    MarkAll,
    /// The common scale (Ctrl+= Ctrl+- Ctrl+0).
    Scale(ito_common::ScaleTo),
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
    /// A message at the top right, until the time with it.
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
    /// The text box of the saved searches in the settings (one per line).
    pub searches_text: String,
    searches_tx: Option<std::sync::mpsc::Sender<String>>,
    /// The list shows closed issues and PRs (`C`), so `x` opens them again.
    pub show_closed: bool,
    /// The list view's selection.
    pub list: ListState,
    /// The detail of one item (its node id), once fetched.
    pub detail: Option<(String, Detail)>,
    /// A URL to open in the browser, taken by the window: the built-in one
    /// beside the view (Windows, macOS), or the system's with `open_outside`
    /// (`O`) and on Linux.
    pub open_url: Option<String>,
    pub open_outside: bool,
    /// The built-in browser's width while it is open, as asked (the frame
    /// keeps it between its bounds).
    pub browser_w: Option<f32>,
    /// The writing box, while it is open.
    pub compose: Option<Compose>,
    /// What was written in the box on an item and left there (by a click
    /// elsewhere or Esc), by node id; it comes back when the box opens again.
    pub drafts: HashMap<String, String>,
    state_tx: Option<std::sync::mpsc::Sender<String>>,
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
    /// The start of the bodies of the board's cards (by node id), asked for
    /// when the mouse is first over one; the card the mouse wants it for.
    pub peeks: HashMap<String, String>,
    pub peek_wanted: Option<String>,
    /// Calls the window has asked for; the net takes them.
    pub outbox: Vec<Outgoing>,
    pub quit: bool,
    /// The unread notifications.
    pub notes: Vec<Note>,
    pub ns: NotesState,
    /// The wheel's turn not yet a whole step (a touchpad's small ones).
    pub wheel_acc: f32,
    /// How far the detail drawn last can scroll, in lines: the keys and the
    /// wheel stop there.
    pub detail_max: Option<f32>,
    /// `g` typed once: a second makes `gg`.
    pub pending_g: bool,
    /// The palette, while it is open.
    pub palette: Option<ito_palette::View>,
    /// The search box's text (`/`), which filters the inbox and the list.
    pub find_text: String,
    /// The search box has the keys.
    pub find_open: bool,
    /// The box opened in this frame: its `/` is still in the input.
    pub find_fresh: bool,
    /// The columns of the inbox, the list and the Actions view, as the mouse left them.
    pub inbox_split: Node<Pane>,
    pub list_split: Node<Pane>,
    pub acts_split: Node<Pane>,
}

/// What a confirmation box asks about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confirm {
    Merge,
    Close,
}

/// What GitHub said about one run.
#[derive(Clone, Default)]
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
    /// A toolbar button clicked, for the box to apply to its selection.
    pub tool: Option<Tool>,
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
            wheel_acc: 0.0,
            detail_max: None,
            pending_g: false,
            find_text: String::new(),
            find_open: false,
            find_fresh: false,
            viewer: None,
            items: Vec::new(),
            inbox: InboxState::default(),
            marks: HashMap::new(),
            marks_tx: None,
            settings: Settings::default(),
            settings_tx: None,
            searches_text: String::new(),
            searches_tx: None,
            show_closed: false,
            list: ListState::default(),
            detail: None,
            open_url: None,
            open_outside: false,
            browser_w: None,
            compose: None,
            drafts: HashMap::new(),
            state_tx: None,
            confirm: None,
            confirm_act: Confirm::Merge,
            runs: Vec::new(),
            acts: ActState::default(),
            run_detail: None,
            overlay: None,
            board: None,
            bs: BoardState::default(),
            board_undo: Vec::new(),
            peeks: HashMap::new(),
            peek_wanted: None,
            outbox: Vec::new(),
            quit: false,
            // The mock's widths at 1280 px: 400 | rest, and 210 | 470 | rest.
            inbox_split: two(0.31),
            list_split: three(0.165, 0.44),
            acts_split: three(0.165, 0.44),
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

    /// How long until the toast goes; `None` when there is none or its
    /// time is up.
    pub fn toast_left(&self) -> Option<Duration> {
        self.toast.as_ref().map(|(_, until)| until.saturating_duration_since(Instant::now())).filter(|d| !d.is_zero())
    }

    /// The common scale a step up or down, or back to 100%, written to
    /// common.toml and said in a toast (as kura does).
    pub fn scale(&mut self, to: ito_common::ScaleTo) {
        let next = ito_common::scale_step(self.prefs.common.scale_or_one(), to);
        let no_folder = self.tr("settings.no_folder");
        self.prefs.set_scale(next, no_folder);
        let said = ito_common::scale_text(next, &self.lang);
        self.say(said);
    }

    /// The wheel turned `notches` (up is plus) over `at`: a step a notch,
    /// `j` down and `k` up in a list (the column under the pointer takes the
    /// focus), three lines a notch in a detail, which stops at `max` lines.
    pub fn wheel(&mut self, at: Wheel, notches: f32, max: Option<f32>) {
        if self.wheel_acc * notches > 0.0 {
            self.wheel_acc = 0.0;
        }
        self.wheel_acc -= notches;
        let steps = self.wheel_acc.trunc();
        self.wheel_acc -= steps;
        if steps == 0.0 {
            return;
        }
        let key = if steps > 0.0 { 'j' } else { 'k' };
        let me = self.viewer.clone().unwrap_or_default();
        let detail = matches!(at, Wheel::Acts(actions::Col::Detail));
        if at == Wheel::Detail {
            let s = if self.view == View::Inbox { &mut self.inbox.scroll } else { &mut self.list.scroll };
            *s = (*s + steps * 3.0).min(max.unwrap_or(f32::MAX)).max(0.0);
            return;
        }
        for _ in 0..steps.abs() as usize * if detail { 3 } else { 1 } {
            match at {
                Wheel::List(col) => {
                    self.list.col = col;
                    self.list.key(key, &self.items, &me);
                }
                Wheel::Inbox => {
                    let all = self.inbox_all();
                    self.inbox.key(key, &all, &self.items);
                }
                Wheel::Acts(col) => {
                    self.acts.col = col;
                    self.acts.key(key, &self.runs);
                }
                Wheel::Board(col) => {
                    if let Some(p) = self.board.as_ref() {
                        self.bs.col = col;
                        self.bs.key(key, p);
                    }
                }
                Wheel::Notes => {
                    self.ns.key(key, self.notes.len());
                }
                Wheel::Detail => {}
            }
        }
        self.clamp_scroll();
    }

    /// Keep the scroll of the view's detail at its end or above.
    pub fn clamp_scroll(&mut self) {
        let Some(max) = self.detail_max else { return };
        let s = match self.view {
            View::Inbox => &mut self.inbox.scroll,
            View::List => &mut self.list.scroll,
            View::Actions => &mut self.acts.scroll,
            View::Board => return,
        };
        *s = s.min(max);
    }

    /// F1: the list of the keys, or back from it.
    pub fn toggle_help(&mut self) {
        if self.compose.is_some() || self.confirm.is_some() || self.overlay.is_some() {
            return;
        }
        self.palette = None;
        self.mode = if self.mode == Mode::Help { Mode::Main } else { Mode::Help };
    }

    /// `gg` (`end` false) and `G`: the first or the last of the column in
    /// focus, as in kura; the top or the end of a detail.
    fn jump(&mut self, end: bool) {
        let me = self.viewer.clone().unwrap_or_default();
        match (self.mode, self.view) {
            (Mode::Notes, _) => self.ns.jump(end, self.notes.len()),
            (_, View::Inbox) => {
                let all = self.inbox_all();
                self.inbox.jump(end, &all, &self.items);
            }
            (_, View::List) => self.list.jump(end, &self.items, &me),
            (_, View::Actions) => self.acts.jump(end, &self.runs),
            (_, View::Board) => {
                if let Some(p) = self.board.as_ref() {
                    self.bs.jump(end, p);
                }
            }
        }
    }

    /// A character typed (not a key with Ctrl or Alt: those are the
    /// shortcuts). Returns whether it was used.
    pub fn feed(&mut self, c: char) -> bool {
        let g = std::mem::take(&mut self.pending_g);
        let free = self.compose.is_none() && self.confirm.is_none() && self.palette.is_none() && self.overlay.is_none() && matches!(self.mode, Mode::Main | Mode::Notes);
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

    /// Tab and Shift+Tab: the next or the previous view, as yagura's.
    /// Only on the main screen with nothing over it.
    pub fn cycle_view(&mut self, back: bool) -> bool {
        let free = self.compose.is_none() && self.confirm.is_none() && self.palette.is_none() && self.overlay.is_none();
        if !free || self.mode != Mode::Main {
            return false;
        }
        self.pending_g = false;
        self.view = self.view.next(back);
        true
    }

    fn feed_key(&mut self, c: char) -> bool {
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
            (Mode::Main, '/') if matches!(self.view, View::Inbox | View::List) => self.find_start(),
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

    /// A key over the notifications: `j k` move, `Enter` opens the thread in
    /// minato (in the browser if minato does not have it) and marks it read,
    /// `o` opens it in the browser, `d` only marks it read, `A` marks all,
    /// `n` closes.
    fn notes_key(&mut self, c: char) -> bool {
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

    /// A row of the notifications clicked, or the header's button.
    pub fn note_click(&mut self, c: NoteClick) {
        match c {
            NoteClick::Open(k) if k < self.notes.len() => self.read_note(k, '\n'),
            NoteClick::Open(_) => {}
            NoteClick::ReadAll => self.mark_all_read(),
            NoteClick::Close => self.mode = Mode::Main,
        }
    }

    /// Mark the notification at `k` read; `how` is the key (`d` `o` `Enter`).
    fn read_note(&mut self, k: usize, how: char) {
        let note = self.notes.remove(k);
        self.queue(&format!("note:{}", note.id), notes::mark_read(&note.id), None);
        self.unread = self.notes.len() as u32;
        self.ns.clamp(self.notes.len());
        match how {
            'o' => self.open_url = Some(note.url),
            '\n' => {
                self.mode = Mode::Main;
                if !self.open_item(&note.url) {
                    self.open_url = Some(note.url);
                }
            }
            _ => {}
        }
    }

    /// Where a notification's thread is in minato, by the page it opens:
    /// its row in the items, and its inbox card unless that is done.
    pub fn note_place(&self, url: &str) -> (Option<usize>, Option<Card>) {
        let at = self.items.iter().position(|i| i.url == url);
        let card = at.and_then(|at| self.inbox_all().into_iter().find(|c| c.at == at && c.group != Group::Done));
        (at, card)
    }

    /// `/`: the search box takes the keys, with the words it had.
    fn find_start(&mut self) {
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
        }
    }

    /// Esc: the search box closed and its words gone.
    pub fn find_clear(&mut self) {
        self.find_open = false;
        self.set_find("");
    }

    /// Whether the search filters the view on screen.
    pub fn find_on(&self) -> bool {
        self.mode == Mode::Main && matches!(self.view, View::Inbox | View::List) && (self.find_open || !self.inbox.find.is_empty())
    }

    /// The search's words for what the view shows.
    pub fn lit(&self) -> &minato_core::find::Query {
        &self.inbox.find
    }

    /// How many cards or rows the search leaves in the view.
    pub fn find_count(&self) -> usize {
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

    /// Open the item whose page is `url`: in the inbox if it is a card
    /// there, else in the list. False if minato does not have it.
    fn open_item(&mut self, url: &str) -> bool {
        let (Some(at), card) = self.note_place(url) else { return false };
        self.find_clear();
        if let Some(card) = card {
            self.inbox = InboxState { undo: std::mem::take(&mut self.inbox.undo), ..InboxState::default() };
            let all = self.inbox_all();
            self.inbox.sel = self.inbox.shown(&all, &self.items).iter().position(|c| *c == card).unwrap_or(0);
            self.view = View::Inbox;
            return true;
        }
        let id = self.items[at].id.clone();
        self.show_in_list(&id);
        self.view == View::List
    }

    /// A click in a view.
    pub fn click(&mut self, c: Click) {
        use minato_core::list::Col;
        let me = self.viewer.clone().unwrap_or_default();
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
            Click::Write => self.open_compose(),
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

    pub fn set_split(&mut self, v: View, n: Node<Pane>) {
        match v {
            View::Inbox => self.inbox_split = n,
            View::List => self.list_split = n,
            View::Actions => self.acts_split = n,
            View::Board => {}
        }
        if let Some(tx) = &self.state_tx {
            let _ = tx.send(minato_core::state::format(&self.columns()));
        }
    }

    /// The columns' widths as state.toml keeps them.
    pub fn columns(&self) -> minato_core::state::State {
        minato_core::state::State { inbox: ratios(&self.inbox_split), list: ratios(&self.list_split), actions: ratios(&self.acts_split) }
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
        // tsumugi's words for the same commands; the views are not here, as
        // the bar shows them with their keys.
        let mut e = vec![
            Entry::new(self.tr("cmd.notes"), "n", Cmd::Notes),
            Entry::new(self.tr("cmd.mark_all"), "A", Cmd::MarkAll),
            Entry::new(self.tr("cmd.settings"), "Ctrl+,", Cmd::Settings),
            Entry::new(self.tr("cmd.zoom_in"), "Ctrl+=", Cmd::Scale(ito_common::ScaleTo::In)),
            Entry::new(self.tr("cmd.zoom_out"), "Ctrl+-", Cmd::Scale(ito_common::ScaleTo::Out)),
            Entry::new(self.tr("cmd.zoom_reset"), "Ctrl+0", Cmd::Scale(ito_common::ScaleTo::Reset)),
            Entry::new(self.tr("cmd.help"), "F1", Cmd::Help),
        ];
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
            Cmd::Help => self.mode = Mode::Help,
            Cmd::Notes => self.mode = Mode::Notes,
            Cmd::MarkAll => self.mark_all_read(),
            Cmd::Scale(to) => self.scale(to),
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
        self.find_clear();
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

    /// Keep the columns' widths in `path` (state.toml), the same way.
    pub fn keep_state(&mut self, path: std::path::PathBuf) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            let st = minato_core::state::parse(&text);
            set_ratios(&mut self.inbox_split, &st.inbox);
            set_ratios(&mut self.list_split, &st.list);
            set_ratios(&mut self.acts_split, &st.actions);
        }
        self.state_tx = Some(writer(path));
    }

    /// Keep minato's settings in `path`, the same way.
    pub fn keep_settings(&mut self, path: std::path::PathBuf) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            self.settings = Settings::parse(&text);
        }
        self.settings_tx = Some(writer(path));
    }

    /// Keep the saved searches (one per line) in `path`, the same way.
    pub fn keep_searches(&mut self, path: std::path::PathBuf) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            self.searches_text = text;
            self.sync_searches();
        }
        self.searches_tx = Some(writer(path));
    }

    /// Take the saved searches from the text box; a search that is gone
    /// stops being the list's filter.
    pub fn sync_searches(&mut self) {
        self.list.searches = self.searches_text.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect();
        if self.list.filter.search.is_some_and(|i| i >= self.list.searches.len()) {
            self.list.filter.search = None;
        }
    }

    /// Write the saved searches after the screen changed them.
    pub fn save_searches(&self) {
        if let Some(tx) = &self.searches_tx {
            let _ = tx.send(self.searches_text.clone());
        }
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
        if self.compose.as_ref().is_some_and(|c| c.id == item.id) {
            return;
        }
        self.close_compose();
        self.queue(&format!("labels:{}", item.repo), Call::Get(write::labels_path(&item.repo)), None);
        self.queue(&format!("people:{}", item.repo), Call::Get(write::assignees_path(&item.repo)), None);
        self.queue(&format!("miles:{}", item.repo), Call::Get(write::milestones_path(&item.repo)), None);
        let text = self.drafts.remove(&item.id).unwrap_or_default();
        self.compose = Some(Compose { id: item.id, text, preview: false, picker: None, labels: Vec::new(), people: Vec::new(), milestones: Vec::new(), tool: None });
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
        self.close_compose();
        self.compose = Some(Compose { id: format!("new:{repo}"), text: String::new(), preview: false, picker: None, labels: Vec::new(), people: Vec::new(), milestones: Vec::new(), tool: None });
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
        if self.find_on() {
            self.find_clear();
            return true;
        }
        let was = self.mode;
        self.mode = Mode::Main;
        was != Mode::Main
    }

    /// Whether the bar's tab for `v` is the one in force (also with the
    /// notifications open over it).
    pub fn is_on(&self, v: View) -> bool {
        matches!(self.mode, Mode::Main | Mode::Notes) && self.view == v
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
        assert!(!a.open_outside);
        // `O` is `o` in the system's browser.
        assert!(a.feed('O'));
        assert!(a.open_url.take().unwrap().ends_with("/runs/2"));
        assert!(a.open_outside);
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
        // Enter closes the list, as a click on a row does.
        assert_eq!(a.mode, Mode::Main);
        assert!(a.feed('n'));
        a.note_click(NoteClick::Close);
        assert_eq!(a.mode, Mode::Main);
    }

    #[test]
    fn a_note_on_a_thread_minato_has_opens_it_there() {
        let mut a = with_pr();
        a.items[0].url = "https://example.test/P1".into();
        a.view = View::Board;
        a.notes = vec![Note { url: "https://example.test/P1".into(), ..note("1") }];
        a.unread = 1;
        a.feed('n');
        assert!(a.is_on(View::Board));
        a.note_click(NoteClick::Open(0));
        assert_eq!(a.open_url, None);
        assert_eq!(a.mode, Mode::Main);
        assert!(matches!(a.view, View::Inbox | View::List));
        assert_eq!(a.selected().map(|i| i.id.as_str()), Some("P1"));
        assert!(a.notes.is_empty());
    }

    #[test]
    fn clicks_pick_rows_and_columns() {
        let mut a = with_pr();
        a.items.push(Item { id: "P2".into(), number: 4, ..pr("P2") });
        a.click(Click::ListItem(1));
        assert_eq!((a.list.col, a.list.sel), (minato_core::list::Col::Items, 1));
        a.click(Click::ListCol(minato_core::list::Col::Detail));
        assert_eq!(a.list.col, minato_core::list::Col::Detail);
        a.runs = vec![run(1, actions::Status::Ok, None), run(2, actions::Status::Bad, None)];
        a.click(Click::ActRun(1));
        assert_eq!((a.acts.col, a.acts.sel), (actions::Col::Runs, 1));
        a.click(Click::ActCol(actions::Col::Detail));
        assert_eq!(a.acts.col, actions::Col::Detail);
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
        assert!(!e.iter().any(|x| x.key == "1"), "no lines for the views");
        // The names of the commands tsumugi has too are tsumugi's.
        let name = |c: Cmd| e.iter().find(|x| x.pick == c).map(|x| x.title.clone());
        assert_eq!(name(Cmd::Settings).as_deref(), Some(a.tr("cmd.settings")));
        assert_eq!(name(Cmd::Notes).as_deref(), Some(a.tr("cmd.notes")));
        assert_eq!(name(Cmd::Help).as_deref(), Some(a.tr("cmd.help")));
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
    fn tab_cycles_the_views_round() {
        let mut a = app();
        for v in [View::List, View::Actions, View::Board, View::Inbox] {
            assert!(a.cycle_view(false));
            assert_eq!(a.view, v);
        }
        assert!(a.cycle_view(true));
        assert_eq!(a.view, View::Board);
        assert!(a.cycle_view(true));
        assert_eq!(a.view, View::Actions);
        a.mode = Mode::Notes;
        assert!(!a.cycle_view(false));
        assert_eq!(a.view, View::Actions);
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
            peek: String::new(),
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
    fn slash_searches_the_inbox_and_the_list_and_esc_clears() {
        let mut a = with_pr();
        let mut other = pr("P2");
        other.title = "Font size in the Preview".into();
        other.number = 8;
        a.items.push(other);
        a.list.sel = 1;
        assert!(a.feed('/'));
        assert!(a.find_open && a.find_fresh);
        a.set_find("font PREV");
        assert_eq!(a.find_count(), 0);
        a.set_find("font prev");
        assert_eq!(a.find_count(), 1);
        assert_eq!((a.list.sel, a.selected().map(|i| i.number)), (0, Some(8)));
        // The inbox is searched with the same words.
        assert_eq!(a.inbox.find, a.list.find);
        // Enter keeps the words; Esc then clears them.
        a.find_open = false;
        assert!(a.find_on());
        assert!(a.escape());
        assert!(!a.find_on() && a.find_text.is_empty());
        assert_eq!(a.find_count(), 2);
        // Not in the views without a search, nor under the palette.
        a.view = View::Actions;
        a.feed('/');
        assert!(!a.find_open);
        a.view = View::List;
        a.feed('/');
        a.palette = Some(ito_palette::View::new());
        a.find_check();
        assert!(!a.find_open);
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
    fn leaving_the_box_keeps_the_text_as_a_draft() {
        let mut a = with_pr();
        a.feed('r');
        a.compose.as_mut().unwrap().text = "half".into();
        a.close_compose();
        assert!(a.compose.is_none());
        assert_eq!(a.drafts.get("P1").map(String::as_str), Some("half"));
        a.click(Click::Write);
        assert_eq!(a.compose.as_ref().unwrap().text, "half");
        assert!(a.drafts.is_empty());
        // Opening it again on the same item keeps what is there.
        a.compose.as_mut().unwrap().text = "half more".into();
        a.click(Click::Write);
        assert_eq!(a.compose.as_ref().unwrap().text, "half more");
        // An empty box leaves no draft.
        a.compose.as_mut().unwrap().text = "  ".into();
        a.close_compose();
        assert!(a.drafts.is_empty());
    }

    #[test]
    fn the_strip_buttons_open_the_box_and_send() {
        let mut a = with_pr();
        a.click(Click::Tool(Tool::Bold));
        let c = a.compose.as_ref().unwrap();
        assert!(c.tool == Some(Tool::Bold) && !c.preview);
        a.click(Click::Tab(true));
        assert!(a.compose.as_ref().unwrap().preview);
        a.outbox.clear();
        // Comment with nothing written does nothing and keeps the box.
        a.click(Click::Send(false));
        assert!(a.outbox.is_empty() && a.compose.is_some());
        // Close with nothing written asks first.
        a.click(Click::Send(true));
        assert!(a.compose.is_none());
        assert_eq!(a.confirm.as_deref(), Some("P1"));
        assert!(a.confirm_act == Confirm::Close);
        a.confirm = None;
        a.click(Click::Write);
        a.outbox.clear();
        a.compose.as_mut().unwrap().text = "done".into();
        a.click(Click::Send(true));
        let keys: Vec<&str> = a.outbox.iter().map(|o| o.key.as_str()).collect();
        assert_eq!(keys, ["write:0:comment", "write:0:state"]);
    }

    #[test]
    fn the_widths_come_back_from_state_toml() {
        let dir = std::env::temp_dir().join(format!("minato-state-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let f = dir.join("state.toml");
        let mut a = app();
        let mut st = a.columns();
        assert!(!st.inbox.is_empty());
        for r in st.inbox.iter_mut().chain(st.list.iter_mut()).chain(st.actions.iter_mut()) {
            *r = 0.31;
        }
        std::fs::write(&f, minato_core::state::format(&st)).unwrap();
        a.keep_state(f.clone());
        assert_eq!(a.columns(), st);
        // A list of another length is left alone.
        let mut short = st.clone();
        short.inbox.push(0.5);
        let mut b = app();
        std::fs::write(&f, minato_core::state::format(&short)).unwrap();
        let before = b.columns().inbox;
        b.keep_state(f);
        assert_eq!(b.columns().inbox, before);
        let _ = std::fs::remove_dir_all(&dir);
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

    #[test]
    fn gg_and_g_go_to_the_ends() {
        let mut a = with_pr();
        a.items.push(pr("P2"));
        a.items.push(pr("P3"));
        a.view = View::List;
        a.list.col = minato_core::list::Col::Items;
        a.feed('G');
        assert_eq!(a.list.sel, 2);
        a.feed('g');
        assert_eq!(a.list.sel, 2, "one g waits for the second");
        a.feed('g');
        assert_eq!(a.list.sel, 0);
        // A key between the two g's starts over.
        a.feed('G');
        a.feed('g');
        a.feed('k');
        a.feed('g');
        assert_eq!(a.list.sel, 1);
    }

    #[test]
    fn the_detail_stops_at_its_end() {
        let mut a = with_pr();
        a.view = View::List;
        a.list.col = minato_core::list::Col::Detail;
        a.detail_max = Some(4.0);
        for _ in 0..10 {
            a.feed('j');
        }
        assert_eq!(a.list.scroll, 4.0);
        a.feed('k');
        assert_eq!(a.list.scroll, 3.0, "one up from the end shows at once");
        a.feed('G');
        assert_eq!(a.list.scroll, 4.0);
        a.feed('g');
        a.feed('g');
        assert_eq!(a.list.scroll, 0.0);
    }

    #[test]
    fn f1_opens_and_closes_the_keys() {
        let mut a = app();
        a.palette = Some(ito_palette::View::new());
        a.toggle_help();
        assert!(a.mode == Mode::Help && a.palette.is_none());
        a.toggle_help();
        assert_eq!(a.mode, Mode::Main);
    }

    #[test]
    fn a_toast_goes_when_its_time_is_up() {
        let mut a = app();
        a.say("Assigned to you");
        assert!(a.toast_left().is_some());
        a.toast = Some(("Assigned to you".into(), Instant::now()));
        assert_eq!(a.toast_left(), None, "an expired toast is not drawn");
    }

    #[test]
    fn the_scale_keys_step_and_reset() {
        use ito_common::ScaleTo;
        let mut a = app();
        a.scale(ScaleTo::In);
        let up = a.prefs.common.scale_or_one();
        assert!(up > 1.0);
        assert!(a.toast.as_ref().is_some_and(|(t, _)| t.contains('%')));
        a.scale(ScaleTo::Out);
        a.scale(ScaleTo::Out);
        assert!(a.prefs.common.scale_or_one() < 1.0);
        a.scale(ScaleTo::Reset);
        assert_eq!(a.prefs.common.scale_or_one(), 1.0);
    }

    #[test]
    fn the_wheel_moves_a_list_and_scrolls_a_detail() {
        let mut a = with_pr();
        a.items.push(pr("P2"));
        a.items.push(pr("P3"));
        // Down a notch: the next row, and the column under the pointer.
        a.wheel(Wheel::List(minato_core::list::Col::Items), -1.0, None);
        assert_eq!((a.list.col, a.list.sel), (minato_core::list::Col::Items, 1));
        // A touchpad's small turns add up to a step.
        a.wheel(Wheel::List(minato_core::list::Col::Items), -0.5, None);
        assert_eq!(a.list.sel, 1);
        a.wheel(Wheel::List(minato_core::list::Col::Items), -0.5, None);
        assert_eq!(a.list.sel, 2);
        a.wheel(Wheel::List(minato_core::list::Col::Items), 2.0, None);
        assert_eq!(a.list.sel, 0);
        // The detail: three lines a notch, stopping at its end and the top.
        a.wheel(Wheel::Detail, -1.0, Some(10.0));
        assert_eq!(a.list.scroll, 3.0);
        a.wheel(Wheel::Detail, -5.0, Some(10.0));
        assert_eq!(a.list.scroll, 10.0);
        a.wheel(Wheel::Detail, 9.0, Some(10.0));
        assert_eq!(a.list.scroll, 0.0);
    }
}
