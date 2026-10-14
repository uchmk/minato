//! The app's state and what the keys do to it. Nothing here draws or reads
//! a file, so the keys can be tested without a window.

use std::time::{Duration, Instant};

use std::collections::HashMap;

use minato_core::actions::{self, ActState, Artifact, Job, Release, Run};
use minato_core::board::{self, BoardState, Project};
use minato_core::composer::{Picker, Tool};
use minato_core::github::search;
use minato_core::github::worker::Call;
use minato_core::github::write;
use minato_core::inbox::{Card, Group, InboxState, Mark, Rule, Status};
use minato_core::list::{Detail, Item, Kind, ListState};
use minato_core::mainci;
use minato_core::notes::{self, Note, NotesState};
use minato_core::overview;
use minato_core::prefs::Settings;

use minato_core::workflow::{self, Workflow};

use ito_layout::{Dir, Node};

use crate::overlay::{Form, LogView, Out, Overlay, K};
use crate::prefs::Prefs;

/// The five views, opened by `1` to `5` (docs/design.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Inbox,
    List,
    Actions,
    Board,
    Status,
}

impl View {
    pub const ALL: [View; 5] = [View::Inbox, View::List, View::Actions, View::Board, View::Status];

    /// The digit that opens it.
    pub fn digit(self) -> char {
        match self {
            View::Inbox => '1',
            View::List => '2',
            View::Actions => '3',
            View::Board => '4',
            View::Status => '5',
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
            View::Status => "view.status",
        }
    }

    /// The word-table key of the line saying what will be here.
    pub fn empty(self) -> &'static str {
        match self {
            View::Inbox => "empty.inbox",
            View::List => "empty.list",
            View::Actions => "empty.actions",
            View::Board => "empty.board",
            View::Status => "empty.status",
        }
    }

    /// The keys that work in this view: `(keys, word-table key)`, the one to
    /// recommend first. The keybar shows as many as fit; `?` shows them all.
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
                ("y", "key.copy_link"),
                ("c", "key.prompt"),
                ("f F t", "key.filter"),
                ("/", "key.find"),
            ],
            View::List => &[
                ("j k", "key.next"),
                ("h l", "key.columns"),
                ("r", "key.write"),
                ("N", "key.new"),
                ("S", "key.repo"),
                ("M", "key.answer"),
                ("a", "key.assign"),
                ("m", "key.merge"),
                ("x", "key.close"),
                ("C", "key.closed"),
                ("i", "key.inbox_add"),
                ("o", "key.browser"),
                ("O", "key.outside"),
                ("y", "key.copy_link"),
                ("/", "key.find"),
            ],
            View::Actions => &[
                ("j k", "key.next"),
                ("w", "key.watch"),
                ("R", "key.rerun"),
                ("X", "key.cancel"),
                ("D", "key.dispatch"),
                ("e", "key.logs"),
                ("L", "key.log"),
                ("p", "run.open_pr"),
                ("o", "key.browser"),
                ("O", "key.outside"),
                ("y", "key.copy_link"),
            ],
            View::Board => &[
                ("h l", "key.columns"),
                ("j k", "key.next"),
                ("H L", "key.card"),
                ("Enter", "key.open"),
                ("u", "key.undo"),
                ("o", "key.browser"),
                ("O", "key.outside"),
                ("y", "key.copy_link"),
            ],
            View::Status => {
                &[("j k", "key.next"), ("Enter", "key.open"), ("o", "key.browser"), ("O", "key.outside"), ("y", "key.copy_link")]
            }
        }
    }
}

/// The keys that work in every view.
pub const GLOBAL_KEYS: [(&str, &str); 11] = [
    ("1 2 3 4 5", "key.views"),
    ("Tab S-Tab", "key.cycle"),
    ("gg G", "key.ends"),
    ("Ctrl+,", "key.settings"),
    ("Ctrl+= - 0", "key.scale"),
    ("Ctrl+Shift+P", "key.palette"),
    ("n", "key.notes"),
    ("u", "key.undo"),
    ("F5", "key.sync"),
    ("@", "key.owner"),
    ("F1 ?", "key.help"),
];

/// The palette opened with `@`, to switch whose repositories are shown.
fn owner_palette() -> ito_palette::View {
    let mut v = ito_palette::View::new();
    v.query = "@".into();
    v
}

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
    let rest =
        Node::Split { dir: Dir::Right, ratio: mid, first: Box::new(Node::Leaf(Pane::Mid)), second: Box::new(Node::Leaf(Pane::Right)) };
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
    /// A line of the status view (a row of `status_rows`); opened on a
    /// double click, as `Enter` does.
    StatusRow(usize, bool),
    /// The writing box's strip under the detail.
    Write,
    /// A button of the box's toolbar.
    Tool(Tool),
    /// The box's Write (`false`) or Preview (`true`) tab.
    Tab(bool),
    /// Comment (`false`), or Close (with the comment if there is one).
    Send(bool),
    /// A field chip of the strip (0 Labels .. 5 Reviewers): its picker.
    Field(usize),
    /// A button that stands for a key (the next box's): that key.
    Key(char),
}

impl Click {
    /// Whether it is the writing box's own, so the box stays open for it.
    pub fn is_compose(&self) -> bool {
        matches!(self, Click::Write | Click::Tool(_) | Click::Tab(_) | Click::Send(_) | Click::Field(_))
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
    /// The status view's lines.
    Status,
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
    /// Ask GitHub for everything shown now (F5).
    Sync,
    /// Show this owner's repositories (a login; empty for the viewer).
    Owner(String),
    /// Show this owner's alongside the owner's (a login).
    AddOwner(String),
    /// Stop showing this owner's alongside the owner's (a login).
    DropOwner(String),
    /// The common scale (Ctrl+= Ctrl+- Ctrl+0).
    Scale(ito_common::ScaleTo),
    /// An issue, PR or discussion (its node id).
    Item(String),
}

pub struct App {
    pub view: View,
    pub mode: Mode,
    /// The help's second page (the inbox's table), when it does not fit
    /// beside the keys; Tab turns it.
    pub help_rules: bool,
    /// The key rows the help's columns are moved up by (`j` `k`) when they
    /// do not fit, and the most that is of use (set as the help is drawn).
    pub help_top: usize,
    pub help_last: std::cell::Cell<usize>,
    pub prefs: Prefs,
    /// The language in force (`en`, `ja`), from common.toml or the OS.
    pub lang: String,
    /// Unread notifications, on the bell.
    pub unread: u32,
    /// When GitHub was last asked; `None` until the first sync.
    pub synced: Option<Instant>,
    /// F5 asked for everything shown to be fetched again; the network
    /// takes it.
    pub sync_wanted: bool,
    /// The answers of that sync that came back and the ones asked, while
    /// it runs (the keybar shows them).
    pub syncing: Option<(usize, usize)>,
    /// The rate limit's remaining and total, from the last answer.
    pub api: Option<(u32, u32)>,
    /// A message at the top right, until the time with it.
    pub toast: Option<(String, Instant)>,
    /// The login the token belongs to, once GitHub has said.
    pub viewer: Option<String>,
    /// No token was found, or GitHub refused it: the account mark is `?`
    /// though the kept answers still show whose they were.
    pub signed_out: bool,
    /// The open issues, PRs and discussions of the last search.
    pub items: Vec<Item>,
    /// The inbox's selection and filters.
    pub inbox: InboxState,
    /// The local marks (done, later) of the inbox; they never reach GitHub.
    pub marks: HashMap<String, Mark>,
    /// Where changed marks go to be written (see `keep_marks`).
    marks_tx: Option<std::sync::mpsc::Sender<String>>,
    /// minato's own settings: whose repositories, the inbox rules and the
    /// writing box's fields.
    pub settings: Settings,
    /// The settings' one-line fields while they are typed in.
    pub fields_typed: ito_prefs::Drafts,
    /// The text boxes of the owner and the board's Project in the settings
    /// (taken into `settings` on Enter).
    pub scope_text: [String; 2],
    settings_tx: Option<std::sync::mpsc::Sender<String>>,
    /// The text box of the saved searches in the settings (one per line).
    pub searches_text: String,
    searches_tx: Option<std::sync::mpsc::Sender<String>>,
    seen_tx: Option<std::sync::mpsc::Sender<String>>,
    /// The list shows closed issues and PRs (`C`), so `x` opens them again.
    pub show_closed: bool,
    /// Items fetched one by one for a notification the searches do not
    /// list (a merged PR, a closed issue): shown in the list, not the inbox.
    pub fetched: Vec<Item>,
    /// The list view's selection.
    pub list: ListState,
    /// The detail of one item (its node id), once fetched.
    pub detail: Option<(String, Detail)>,
    /// A URL to open in the browser, taken by the window: the built-in one
    /// beside the view (Windows, macOS), or the system's with `open_outside`
    /// (`O`) and on Linux.
    pub open_url: Option<String>,
    pub open_outside: bool,
    /// Words to put on the clipboard, taken by the window.
    pub copy: Option<String>,
    /// The built-in browser's width while it is open, as asked (the frame
    /// keeps it between its bounds).
    pub browser_w: Option<f32>,
    /// The writing box, while it is open.
    pub compose: Option<Compose>,
    /// What was written in the box on an item and left there (by a click
    /// elsewhere or Esc), by node id; it comes back when the box opens again.
    pub drafts: HashMap<String, String>,
    /// Each repository's lists for the writing box, by `owner/name`, as
    /// GitHub last said (kept across the box's openings and in the cache).
    pub lists: HashMap<String, Lists>,
    /// When each repository's lists were last asked for.
    lists_asked: HashMap<String, Instant>,
    state_tx: Option<std::sync::mpsc::Sender<String>>,
    /// What waits to be confirmed: the item's node id, or the run's id to
    /// start its workflow again or stop it.
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
    /// The notifications marked read here.
    notes_read: notes::Read,
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
    /// The runs watched with `w`, as last seen: a toast and a sound when one
    /// ends.
    pub watched: Vec<Run>,
    /// The toasts of the system that wait to be shown (the network takes them).
    pub rings: Vec<Ring>,
    /// The status view's selection (a row of `status_rows`).
    pub status_sel: usize,
    /// The latest release of each repository (`None`: it has none), as GitHub last said.
    pub shipped: HashMap<String, Option<overview::Shipped>>,
    /// The commits on the default branch since a repository's latest
    /// release: the tag they were counted from and how many.
    pub ahead: HashMap<String, (String, u32)>,
    /// The repositories whose releases the status view shows (the owners').
    pub status_repos: Vec<String>,
}

/// A toast of the system: a watched run ended (or the settings' test).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ring {
    pub title: String,
    pub body: String,
    /// Shown as the system's notification.
    pub system: bool,
    /// The sound to play, if any.
    pub sound: Option<String>,
    /// No sound in Windows' focus mode.
    pub focus: bool,
    /// A message for the webhook, if one is to be sent.
    pub hook: Option<Hook>,
}

/// A message for a webhook (`ito_notify::webhook::send`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hook {
    pub url: String,
    pub format: String,
    pub level: ito_notify::webhook::Level,
    /// The repository.
    pub folder: String,
}

/// What a confirmation box asks about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confirm {
    Merge,
    Close,
    Reopen,
    /// Start a run's workflow by hand (one without inputs).
    Dispatch,
    /// Stop a run that is going.
    Cancel,
}

/// What GitHub said about one run.
#[derive(Clone, Default)]
pub struct RunDetail {
    pub id: u64,
    /// The workflow file at the run's commit (the `needs:` graph, the inputs).
    pub workflow: Option<Workflow>,
    /// Whether the workflow file has been asked for and answered (read, or
    /// not to be read): till then `D` cannot know the inputs.
    pub yaml_tried: bool,
    pub jobs: Option<Vec<Job>>,
    pub artifacts: Option<Vec<Artifact>>,
    /// The release the run made (a release workflow's run), once read.
    pub release: Option<Release>,
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

/// How many seen runs are written (the newest).
const SEEN_KEPT: usize = 1000;

/// How long a repository's lists for the writing box are not asked again.
const LISTS_AGE: Duration = Duration::from_secs(600);

/// A repository's labels, assignable people and open milestones
/// (`(number, title)`).
#[derive(Clone, Debug, Default)]
pub struct Lists {
    pub labels: Vec<String>,
    pub people: Vec<String>,
    pub milestones: Vec<(u64, String)>,
    /// Which of the three have come (an empty list that came is not one
    /// on its way).
    pub came: [bool; 3],
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
    /// Which of the labels, people and milestones have come.
    pub came: [bool; 3],
    /// A toolbar button clicked, for the box to apply to its selection.
    pub tool: Option<Tool>,
}

impl Compose {
    /// Whether the open picker's list has come (Status and Priority come
    /// with the Project).
    pub fn picker_ready(&self) -> bool {
        match self.picker.as_ref().map(|(k, _)| *k) {
            Some(Pick::Labels) => self.came[0],
            Some(Pick::Assignees | Pick::Reviewers) => self.came[1],
            Some(Pick::Milestone) => self.came[2],
            _ => true,
        }
    }
}

impl App {
    pub fn new(prefs: Prefs) -> Self {
        let mut app = Self {
            view: View::Inbox,
            mode: Mode::Main,
            help_rules: false,
            help_top: 0,
            help_last: Default::default(),
            prefs,
            lang: "en".into(),
            unread: 0,
            notes: Vec::new(),
            ns: NotesState::default(),
            notes_read: notes::Read::default(),
            palette: None,
            synced: None,
            sync_wanted: false,
            syncing: None,
            api: None,
            toast: None,
            wheel_acc: 0.0,
            detail_max: None,
            pending_g: false,
            find_text: String::new(),
            find_open: false,
            find_fresh: false,
            viewer: None,
            signed_out: false,
            items: Vec::new(),
            inbox: InboxState::default(),
            marks: HashMap::new(),
            marks_tx: None,
            settings: Settings::default(),
            fields_typed: ito_prefs::Drafts::new(),
            scope_text: [String::new(), board::TITLE.into()],
            settings_tx: None,
            searches_text: String::new(),
            searches_tx: None,
            seen_tx: None,
            show_closed: false,
            fetched: Vec::new(),
            list: ListState::default(),
            detail: None,
            open_url: None,
            copy: None,
            open_outside: false,
            browser_w: None,
            compose: None,
            drafts: HashMap::new(),
            lists: HashMap::new(),
            lists_asked: HashMap::new(),
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
            watched: Vec::new(),
            rings: Vec::new(),
            status_sel: 0,
            shipped: HashMap::new(),
            ahead: HashMap::new(),
            status_repos: Vec::new(),
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
                Wheel::Status => {
                    self.status_key(key);
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
            View::Board | View::Status => return,
        };
        *s = s.min(max);
    }

    /// F1: the list of the keys, or back from it.
    pub fn toggle_help(&mut self) {
        if self.compose.is_some() || self.confirm.is_some() || self.overlay.is_some() {
            return;
        }
        self.palette = None;
        self.help_top = 0;
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
            (_, View::Status) => self.status_sel = if end { self.status_rows().len().saturating_sub(1) } else { 0 },
        }
    }

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

    /// Tab and Shift+Tab: the next or the previous view, as yagura's.
    /// Only on the main screen with nothing over it.
    pub fn cycle_view(&mut self, back: bool) -> bool {
        let free = self.compose.is_none() && self.confirm.is_none() && self.palette.is_none() && self.overlay.is_none();
        if free && self.mode == Mode::Help {
            self.help_rules = !self.help_rules;
            return true;
        }
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
            (Mode::Help, 'j') => self.help_top = (self.help_top + 1).min(self.help_last.get()),
            (Mode::Help, 'k') => self.help_top = self.help_top.saturating_sub(1),
            (Mode::Settings, _) | (Mode::Help, _) => return false,
            (Mode::Main, '?') => (self.mode, self.help_top) = (Mode::Help, 0),
            (Mode::Main, ',') => self.mode = Mode::Settings,
            (Mode::Main, 'n') => self.mode = Mode::Notes,
            (Mode::Main, '@') => self.palette = Some(owner_palette()),
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
                None if self.view == View::Status => return self.status_key(c),
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
    pub fn keys_now(&self) -> Vec<(&'static str, &'static str)> {
        let run = self.run();
        self.view
            .keys()
            .iter()
            .copied()
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
    pub fn bar_keys(&self) -> Vec<(&'static str, &'static str)> {
        if self.confirm.is_some() {
            return match self.confirm_act {
                Confirm::Merge => vec![("Enter", "key.merge"), ("Esc", "bar.no")],
                Confirm::Close => vec![("Enter", "key.close"), ("Esc", "bar.no")],
                Confirm::Reopen => vec![("Enter", "bar.reopen"), ("Esc", "bar.no")],
                Confirm::Dispatch => vec![("Enter", "bar.run"), ("Esc", "bar.no")],
                Confirm::Cancel => vec![("Enter", "bar.stop"), ("Esc", "bar.keep")],
            };
        }
        if let Some(c) = &self.compose {
            return self.compose_keys(c);
        }
        if self.palette.is_some() {
            return vec![("Enter", "bar.do"), ("↑ ↓", "bar.choose"), ("Esc", "key.close")];
        }
        match &self.overlay {
            Some(Overlay::Form(_)) => {
                return vec![("Ctrl+Enter", "bar.start"), ("↑ ↓", "bar.input"), ("Enter", "bar.change"), ("Esc", "bar.no")]
            }
            Some(Overlay::Log(_)) => return vec![("j k", "bar.scroll"), ("Space", "bar.page"), ("g G", "bar.ends"), ("Esc", "key.close")],
            None => {}
        }
        match self.mode {
            Mode::Settings => vec![("Esc", "key.back"), ("Ctrl+Tab", "settings.theme")],
            Mode::Help if self.help_last.get() > 0 => vec![("j k", "help.more"), ("F1 Esc", "key.back")],
            Mode::Help => vec![("F1 Esc", "key.back")],
            Mode::Notes => vec![("j k", "notes.pick"), ("Enter", "key.open"), ("Shift+A", "notes.read_all"), ("Esc", "notes.close")],
            Mode::Main if self.find_on() => Vec::new(),
            Mode::Main => self.keys_now().into_iter().chain([("F5", "key.sync"), ("@", "key.owner"), ("F1", "key.help")]).collect(),
        }
    }

    /// A key in the Actions view: `R` runs the failed jobs again, `X` stops,
    /// `D` starts the workflow by hand, `e` opens the failed job's log, `p`
    /// the pull request, `o` the run; the rest moves the cursor.
    fn actions_key(&mut self, c: char) -> bool {
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
            'y' => {
                if let Some(url) = self.bs.selected(p).map(|i| p.cards[i].url.clone()) {
                    self.copy_link(url);
                }
            }
            _ => return self.bs.key(c, p),
        }
        true
    }

    /// `y`: the page's link on the clipboard.
    fn copy_link(&mut self, url: String) {
        if url.is_empty() {
            return;
        }
        self.copy = Some(url);
        self.say_key("copied.link");
    }

    /// `w`: watch the run (a toast of the system and a sound when it ends),
    /// or stop watching it.
    fn watch(&mut self, run: &Run) {
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
            let hook = n.hook(k).map(|(url, format)| Hook { url: url.into(), format: format.into(), level, folder: r.repo.clone() });
            self.rings.push(Ring { title, body, system: n.system[k], sound, focus: n.focus, hook });
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

    /// A key in the status view: `j k` move, `o` opens the line's page, `y`
    /// copies it, `Enter` opens an item in the list (other lines in the browser).
    fn status_key(&mut self, c: char) -> bool {
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

    /// The unread notifications as GitHub listed them, less those marked
    /// read here since.
    pub fn take_notes(&mut self, v: Vec<Note>) {
        self.notes = self.notes_read.unread(v);
        self.unread = self.notes.len() as u32;
        self.ns.clamp(self.notes.len());
    }

    /// Mark the notification at `k` read; `how` is the key (`d` `o` `Enter`).
    fn read_note(&mut self, k: usize, how: char) {
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
    fn note_run(&self, note: &Note) -> Option<usize> {
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
    fn open_red_main(&mut self, note: &Note) -> bool {
        let Some((_, branch)) = notes::workflow_run(note) else { return false };
        let id = format!("main:{}", note.repo);
        let Some(at) = self.items.iter().position(|i| i.id == id && mainci::branch(i) == Some(branch)) else { return false };
        let Some(card) = self.inbox_all().into_iter().find(|c| c.at == at) else { return false };
        self.open_card(card);
        true
    }

    /// Show the run a notification is about in Actions, the filters
    /// cleared if they hide it. False if minato does not have it.
    fn open_run(&mut self, note: &Note) -> bool {
        let Some(k) = self.note_run(note) else { return false };
        let id = self.runs[k].id;
        // Shown even if seen, as the selected run.
        self.acts.keep = Some(id);
        if !self.acts.matches(&self.runs[k]) {
            let seen = std::mem::take(&mut self.acts.seen);
            self.acts = ActState::default();
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
    fn place(&self, url: &str) -> (Option<usize>, Option<Card>) {
        let at = self.items.iter().position(|i| i.url == url);
        let card = at.and_then(|at| self.inbox_all().into_iter().find(|c| c.at == at));
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
    /// there (done today too), else in the list. False if minato does not
    /// have it.
    fn open_item(&mut self, url: &str) -> bool {
        let (Some(at), card) = self.place(url) else { return false };
        if let Some(card) = card {
            self.open_card(card);
            return true;
        }
        let id = self.items[at].id.clone();
        self.show_in_list(&id);
        self.view == View::List
    }

    /// The inbox with `card` selected, its filters cleared.
    fn open_card(&mut self, card: Card) {
        self.find_clear();
        self.inbox = InboxState { undo: std::mem::take(&mut self.inbox.undo), ..InboxState::default() };
        let all = self.inbox_all();
        self.inbox.sel = self.inbox.shown(&all, &self.items).iter().position(|c| *c == card).unwrap_or(0);
        self.view = View::Inbox;
    }

    /// An item fetched for a notification has come (it is in the items
    /// now): the list shows it.
    pub fn show_fetched(&mut self, id: &str) {
        self.mode = Mode::Main;
        self.show_in_list(id);
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

    pub fn set_split(&mut self, v: View, n: Node<Pane>) {
        match v {
            View::Inbox => self.inbox_split = n,
            View::List => self.list_split = n,
            View::Actions => self.acts_split = n,
            View::Board | View::Status => {}
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
        for n in std::mem::take(&mut self.notes) {
            self.notes_read.mark(&n);
        }
        self.unread = 0;
        self.ns = NotesState::default();
        self.queue("note:all", notes::mark_all(), Some("notes.all_read"));
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
            Entry::new(self.tr("cmd.notes"), "n", Cmd::Notes),
            Entry::new(self.tr("cmd.mark_all"), "A", Cmd::MarkAll),
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

    /// F5: fetch everything shown again now, not at the next turn.
    pub fn sync_now(&mut self) {
        self.sync_wanted = true;
    }

    /// Show `login`'s repositories, searches and Project (the viewer's when
    /// it is the viewer); the settings keep it, as the settings screen does.
    pub fn show_owner(&mut self, login: &str) {
        let me = self.viewer.as_deref().is_some_and(|v| v.eq_ignore_ascii_case(login));
        let was = self.settings.clone();
        self.settings.set_owner(if me { "" } else { login });
        self.scope_text[0] = self.settings.owners_line();
        if self.settings != was {
            self.save_settings();
        }
        self.mode = Mode::Main;
        let who = self.owner().unwrap_or(login).to_string();
        let words = self.tr("owner.switched").replace("{}", &who);
        self.say(words);
    }

    /// `+@name`: show `login`'s repositories, items and runs alongside the
    /// owner's (the viewer becomes the owner named, as only a named owner
    /// has others beside it).
    pub fn add_owner(&mut self, login: &str) {
        self.mode = Mode::Main;
        if self.settings.owner.is_empty() {
            let Some(me) = self.viewer.clone() else { return self.say_key("owner.not_added") };
            self.settings.set_owner(&me);
        }
        if !self.settings.add_also(login) {
            return self.say_key("owner.not_added");
        }
        self.scope_text[0] = self.settings.owners_line();
        self.save_settings();
        let words = self.tr("owner.added").replace("{}", &minato_core::prefs::login(login));
        self.say(words);
    }

    /// `-@name`: stop showing `login`'s alongside the owner's.
    pub fn drop_owner(&mut self, login: &str) {
        self.mode = Mode::Main;
        let before = self.settings.also.len();
        self.settings.also.retain(|o| !o.eq_ignore_ascii_case(login));
        if self.settings.also.len() == before {
            return;
        }
        self.scope_text[0] = self.settings.owners_line();
        self.save_settings();
        let words = self.tr("owner.dropped").replace("{}", login);
        self.say(words);
    }

    /// Every owner shown, space-separated: the owner (else the viewer) and
    /// the others beside it; empty before GitHub has said who the viewer is.
    pub fn owners_shown(&self) -> String {
        self.owner().into_iter().chain(self.settings.also.iter().map(String::as_str)).collect::<Vec<_>>().join(" ")
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

    /// Whether the searches are of closed items: `C` while the list is
    /// open. The inbox and the rest keep the open ones.
    pub fn closed_shown(&self) -> bool {
        self.show_closed && self.view == View::List
    }

    /// Whose repositories are shown: the owner the settings name, else the
    /// viewer.
    pub fn owner(&self) -> Option<&str> {
        Some(self.settings.owner.as_str()).filter(|o| !o.is_empty()).or(self.viewer.as_deref())
    }

    /// Every card of the inbox now, before its filters (none of an item
    /// fetched for a notification).
    pub fn inbox_all(&self) -> Vec<Card> {
        let me = self.viewer.as_deref().unwrap_or("");
        let mut all = minato_core::inbox::cards(&self.items, me, &self.settings.rules, &self.marks, now_epoch());
        if !self.fetched.is_empty() {
            all.retain(|c| !self.fetched.iter().any(|f| f.id == self.items[c.at].id));
        }
        all
    }

    /// A key in the inbox: `d` done, `s` later (tomorrow 9:00), `u` undo,
    /// `o` the browser, `c` a prompt for Claude, and the filters and the
    /// cursor.
    fn inbox_key(&mut self, c: char) -> bool {
        let all = self.inbox_all();
        let card = self.inbox.selected(&all, &self.items);
        match c {
            'r' => self.open_compose(),
            'm' => self.ask_merge(),
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
    fn prompt(&self, card: Card) -> String {
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

    /// Keep the marks in `path`: read them now (at start-up), and write
    /// every change on a thread of its own.
    pub fn keep_marks(&mut self, path: std::path::PathBuf) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            self.marks = minato_core::inbox::marks_parse(&text);
        }
        self.marks_tx = Some(writer(path));
    }

    /// Keep the runs seen in Actions (one id per line) in `path`, the same way.
    pub fn keep_seen(&mut self, path: std::path::PathBuf) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            self.acts.seen = text.lines().filter_map(|l| l.trim().parse().ok()).collect();
        }
        self.seen_tx = Some(writer(path));
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
            if let Some(tx) = &self.seen_tx {
                let _ = tx.send(
                    ids.iter()
                        .map(|i| {
                            format!(
                                "{i}
"
                            )
                        })
                        .collect(),
                );
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
        }
        self.state_tx = Some(writer(path));
    }

    /// Keep minato's settings in `path`, the same way.
    pub fn keep_settings(&mut self, path: std::path::PathBuf) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            self.settings = Settings::parse(&text);
            self.scope_text = [self.settings.owners_line(), self.settings.project.clone()];
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

    /// `M`: mark the newest comment of the selected discussion that is not
    /// the asker's as its answer.
    fn mark_answer(&mut self) {
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

    /// `S`: show the settings of the selected item's repository (read only).
    fn repo_settings(&mut self) {
        let Some(repo) = self.selected().or(self.items.first()).map(|i| i.repo.clone()) else { return };
        self.queue(&format!("a-repo:{repo}"), Call::Get(minato_core::reposettings::path(&repo)), None);
        self.overlay = Some(Overlay::Log(LogView::new(0, repo)));
    }

    /// `N`: open the writing box for a new issue in the selected item's
    /// repository (the first one listed when nothing is selected).
    fn new_issue(&mut self) {
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

    /// `x` on a closed item: ask before opening it again.
    fn ask_reopen(&mut self) {
        match self.selected() {
            Some(i) if i.kind != Kind::Discussion && !i.merged && !minato_core::mainci::is_stub(i) => {
                self.confirm = Some(i.id.clone());
                self.confirm_act = Confirm::Reopen;
            }
            Some(_) => self.say_key("write.not_closable"),
            None => {}
        }
    }

    fn reopen(&mut self, id: &str) {
        if let Some(item) = self.item(id) {
            let n = self.outbox.len();
            self.queue(&format!("write:{n}:state"), write::set_state(&item, true), Some("write.reopened"));
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
        let Some(item) = self.writable() else { return };
        let Some(me) = self.viewer.clone() else { return };
        if item.kind == Kind::Discussion {
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
    fn the_keybar_shows_the_run_keys_that_apply() {
        let mut a = app();
        a.feed('3');
        a.runs = vec![run(1, actions::Status::Bad, Some(9)), run(2, actions::Status::Running, None), run(3, actions::Status::Ok, None)];
        let words = |a: &App| a.keys_now().into_iter().map(|(_, w)| w).collect::<Vec<_>>();
        let w = words(&a);
        assert!(w.contains(&"key.rerun") && !w.contains(&"key.cancel") && w.contains(&"run.open_pr"));
        a.feed('j');
        let w = words(&a);
        assert!(!w.contains(&"key.rerun") && w.contains(&"key.cancel") && !w.contains(&"run.open_pr"));
        a.feed('j');
        let w = words(&a);
        assert!(!w.contains(&"key.rerun") && !w.contains(&"key.cancel") && w.contains(&"key.dispatch"));
    }

    #[test]
    fn the_keybar_follows_what_is_open() {
        let mut a = with_pr();
        let bar = |a: &App| a.bar_keys().into_iter().map(|(_, w)| w).collect::<Vec<_>>();
        assert_eq!(bar(&a).last(), Some(&"key.help"));
        a.feed('r');
        let w = bar(&a);
        assert_eq!((w[0], w.last()), ("bar.send", Some(&"bar.leave")));
        a.compose.as_mut().unwrap().preview = true;
        assert_eq!(bar(&a).last(), Some(&"bar.edit"));
        a.open_pick(0);
        assert_eq!(bar(&a), ["bar.tick", "bar.move", "bar.apply"]);
        a.close_compose();
        a.new_issue();
        let w = bar(&a);
        assert_eq!((w[0], w.last()), ("bar.create", Some(&"bar.no")));
        assert!(!w.contains(&"bar.send_close") && !w.contains(&"pick.labels"), "a new issue has no fields");
        a.close_compose();
        a.feed('x');
        assert_eq!(bar(&a), ["key.close", "bar.no"]);
        a.escape();
        a.palette = Some(ito_palette::View::new());
        assert_eq!(bar(&a).last(), Some(&"key.close"));
        a.palette = None;
        a.toggle_help();
        assert_eq!(a.bar_keys().last(), Some(&("F1 Esc", "key.back")));
    }

    #[test]
    fn f5_asks_for_a_sync_and_the_keybar_offers_it() {
        let mut a = app();
        let words = a.bar_keys().into_iter().map(|(_, w)| w).collect::<Vec<_>>();
        assert!(words.ends_with(&["key.sync", "key.owner", "key.help"]));
        a.run_cmd(Cmd::Sync);
        assert!(a.sync_wanted);
        assert!(a.palette_entries("").iter().any(|x| x.pick == Cmd::Sync && x.key == "F5"));
    }

    #[test]
    fn at_switches_the_owner_and_back() {
        let mut a = app();
        a.viewer = Some("me".into());
        assert!(a.feed('@'));
        assert_eq!(a.palette.as_ref().map(|v| v.query.as_str()), Some("@"));
        let owners = |a: &App, q: &str| {
            a.palette_entries(q).into_iter().filter_map(|x| if let Cmd::Owner(o) = x.pick { Some(o) } else { None }).collect::<Vec<_>>()
        };
        assert!(owners(&a, "@").is_empty(), "nobody to switch to yet");
        assert_eq!(owners(&a, "@rust-lang"), ["rust-lang"]);
        a.palette = None;
        a.run_cmd(Cmd::Owner("rust-lang".into()));
        assert_eq!((a.settings.owner.as_str(), a.scope_text[0].as_str()), ("rust-lang", "rust-lang"));
        assert_eq!(a.owner(), Some("rust-lang"));
        // Back to the viewer, who is offered first; the other owner is kept.
        assert_eq!(owners(&a, "@"), ["me"]);
        a.show_owner("me");
        assert_eq!(a.settings.owner, "");
        assert_eq!(a.owner(), Some("me"));
        assert_eq!(owners(&a, "@"), ["rust-lang"]);
        assert_eq!(owners(&a, "@tokio-rs"), ["tokio-rs", "rust-lang"]);
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
        assert_eq!((a.confirm.as_deref(), a.confirm_act), (Some("2"), Confirm::Cancel));
        assert_eq!(a.outbox.len(), 1, "nothing stops before Enter");
        assert!(a.enter());
        assert_eq!(a.outbox[1].key, "write:2:cancel");
        assert!(a.feed('R'));
        assert_eq!(a.outbox.len(), 2, "a running run is not re-run");
        // Not before its workflow file is read: the inputs may want the form.
        assert!(a.feed('D'));
        assert_eq!(a.outbox.len(), 2);
        assert_eq!(a.toast.as_ref().map(|t| t.0.clone()), Some(a.tr("run.reading").to_string()));
        a.run_detail = Some(RunDetail { id: 2, yaml_tried: true, ..Default::default() });
        assert!(a.feed('D') && a.enter());
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
        board::Project {
            id: "P".into(),
            title: "uchmk".into(),
            field: "F".into(),
            options: vec![col("o1", "Todo"), col("o2", "Doing")],
            priority_field: "PF".into(),
            priority_options: vec![col("p1", "High")],
            cards: vec![card("I1", "o1"), card("I2", "o1")],
        }
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
        Note {
            id: id.into(),
            repo: "o/r".into(),
            title: "t".into(),
            kind: "Issue".into(),
            reason: "mention".into(),
            updated_at: String::new(),
            url: format!("https://example.test/{id}"),
        }
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
    fn a_runs_note_opens_its_run_in_actions() {
        let mut a = app();
        let mut old = run(1, actions::Status::Bad, None);
        old.started_at = "2026-10-12T08:00:00Z".into();
        let later = Run { updated_at: "2026-10-12T10:00:00Z".into(), ..run(3, actions::Status::Running, None) };
        let other = Run { branch: "dev".into(), ..run(4, actions::Status::Bad, None) };
        a.runs = vec![run(2, actions::Status::Ok, None), later, old, other];
        a.acts.state = Some(actions::Bucket::Bad);
        // Seen, and still shown when a notification opens it.
        a.acts.seen.insert(2);
        let ci = Note {
            repo: "uchmk/kura".into(),
            kind: "CheckSuite".into(),
            title: "check workflow run failed for main branch".into(),
            updated_at: "2026-10-12T09:01:00Z".into(),
            url: "https://github.com/uchmk/kura/actions".into(),
            ..note("1")
        };
        a.notes = vec![ci.clone(), Note { title: "gone workflow run failed for main branch".into(), ..ci }];
        a.feed('n');
        a.note_click(NoteClick::Open(0));
        assert_eq!(a.view, View::Actions);
        assert_eq!(a.run().map(|r| r.id), Some(2), "the newest done by then, the filters cleared");
        assert_eq!(a.acts.state, None);
        assert!(a.acts.seen.contains(&2));
        assert_eq!(a.open_url, None);
        // A run minato does not have opens the repository's Actions.
        a.feed('n');
        a.note_click(NoteClick::Open(0));
        assert_eq!(a.open_url.as_deref(), Some("https://github.com/uchmk/kura/actions"));
    }

    #[test]
    fn a_note_on_a_card_done_today_opens_it_in_the_inbox() {
        let mut a = with_pr();
        a.items[0].url = "https://github.com/uchmk/kura/pull/3".into();
        a.marks.insert("P1".into(), Mark::Done(now_epoch()));
        a.notes = vec![Note { url: a.items[0].url.clone(), ..note("1") }];
        assert_eq!(a.note_place(&a.items[0].url).1, None, "no badge for a done card");
        a.feed('n');
        a.note_click(NoteClick::Open(0));
        assert_eq!(a.view, View::Inbox);
        assert_eq!(a.selected().map(|i| i.id.as_str()), Some("P1"));
        assert!(a.outbox.iter().all(|o| !o.key.starts_with("fetch:")));
    }

    #[test]
    fn a_runs_note_opens_the_red_main_card_before_actions() {
        let mut a = with_pr();
        a.items.push(Item {
            id: "main:uchmk/kura".into(),
            title: "CI is red on main".into(),
            number: 0,
            author: String::new(),
            ..pr("main:uchmk/kura")
        });
        a.runs = vec![run(1, actions::Status::Bad, None)];
        let ci = Note {
            repo: "uchmk/kura".into(),
            kind: "CheckSuite".into(),
            title: "check workflow run failed for main branch".into(),
            url: "https://github.com/uchmk/kura/actions".into(),
            ..note("1")
        };
        a.notes = vec![Note { title: "check workflow run failed for dev branch".into(), ..ci.clone() }, ci];
        a.feed('n');
        a.note_click(NoteClick::Open(1));
        assert_eq!(a.view, View::Inbox);
        assert_eq!(a.selected().map(|i| i.id.as_str()), Some("main:uchmk/kura"));
        // Another branch's run is not the red main: Actions.
        a.runs[0].branch = "dev".into();
        a.feed('n');
        a.note_click(NoteClick::Open(0));
        assert_eq!(a.view, View::Actions);
    }

    #[test]
    fn a_note_on_a_closed_pr_fetches_it_into_the_list() {
        let mut a = with_pr();
        let url = "https://github.com/uchmk/kura/pull/7";
        a.notes = vec![Note { url: url.into(), kind: "PullRequest".into(), ..note("1") }];
        a.feed('n');
        a.note_click(NoteClick::Open(0));
        assert_eq!(a.open_url, None, "not the browser");
        assert!(a.outbox.iter().any(|o| o.key == format!("fetch:{url}")));
        // What Net does when it comes.
        let got = Item { id: "P7".into(), number: 7, url: url.into(), merged: true, ..pr("P7") };
        a.fetched.push(got.clone());
        a.items.push(got);
        a.view = View::Board;
        a.show_fetched("P7");
        assert_eq!(a.view, View::List);
        assert_eq!(a.selected().map(|i| i.id.as_str()), Some("P7"));
        assert!(a.inbox_all().iter().all(|c| a.items[c.at].id != "P7"), "not in the inbox");
    }

    #[test]
    fn a_note_read_here_is_not_listed_again_by_an_old_list() {
        let mut a = app();
        a.take_notes(vec![note("1"), note("2")]);
        a.feed('n');
        a.feed('d');
        assert_eq!(a.unread, 1);
        a.take_notes(vec![note("1"), note("2")]);
        assert_eq!((a.notes.len(), a.notes[0].id.as_str()), (1, "2"));
        a.feed('A');
        a.take_notes(vec![note("1"), note("2")]);
        assert_eq!(a.unread, 0);
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
        let e = a.palette_entries("");
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
    fn x_reopens_a_closed_item_after_enter() {
        let mut a = with_pr();
        a.feed('2');
        assert!(a.feed('C') && a.show_closed);
        for i in &mut a.items {
            i.closed = true;
        }
        let before = a.outbox.len();
        assert!(a.feed('x'));
        assert_eq!(a.confirm_act, Confirm::Reopen);
        assert_eq!(a.outbox.len(), before, "nothing is sent before Enter");
        assert!(a.escape() && a.confirm.is_none());
        assert_eq!(a.outbox.len(), before);
        assert!(a.feed('x') && a.enter());
        let Call::Send { body, .. } = &a.outbox.last().unwrap().call else { panic!() };
        assert!(a.outbox.last().is_some_and(|o| o.key.ends_with(":state")) && body.contains(r#""state":"open""#), "{body}");
        assert!(a.feed('C') && !a.show_closed);
        // Only the list searches closed ones; the inbox keeps the open.
        a.show_closed = true;
        assert!(a.closed_shown());
        a.view = View::Inbox;
        assert!(!a.closed_shown());
    }

    #[test]
    fn reviewers_are_for_prs_only() {
        let mut a = with_pr();
        a.items[0].kind = Kind::Issue;
        a.feed('r');
        a.open_extra_picker(true);
        assert!(a.compose.as_ref().unwrap().picker.is_none() && a.toast.is_some());
        // Hidden in the settings, it still says it is for PRs.
        a.settings.fields[5] = false;
        a.toast = None;
        assert!(!a.field_shown(5));
        assert_eq!(a.toast.as_ref().map(|t| t.0.clone()), Some(a.tr("write.not_pr").to_string()));
    }

    #[test]
    fn seen_runs_drop_out_and_are_read_back_after_a_restart() {
        let dir = std::env::temp_dir().join(format!("minato-seen-{}", std::process::id()));
        let file = dir.join("seen.txt");
        let mut a = app();
        a.keep_seen(file.clone());
        a.view = View::Actions;
        a.runs = vec![run(1, actions::Status::Ok, None), run(2, actions::Status::Bad, None)];
        a.settle_runs(1.0);
        a.settle_runs(2.5);
        assert!(a.feed('j'));
        a.settle_runs(3.0);
        assert_eq!(a.acts.shown(&a.runs).len(), 1);
        assert_eq!(a.run().map(|r| r.id), Some(2));
        for _ in 0..100 {
            if file.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let mut b = app();
        b.keep_seen(file);
        assert_eq!(b.acts.seen, [1].into());
        let _ = std::fs::remove_dir_all(dir);
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
    fn c_copies_a_prompt_for_claude() {
        let mut a = with_pr();
        a.view = View::Inbox;
        a.items[0].url = "https://github.com/uchmk/kura/pull/3".into();
        assert!(a.feed('c'));
        let text = a.copy.take().unwrap();
        assert!(text.starts_with("uchmk/kura PR #3: t\nhttps://github.com/uchmk/kura/pull/3\n\n"), "{text}");
        assert!(text.contains(a.tr("prompt.why_wait")) && text.contains(a.tr("prompt.ask_wait")), "{text}");
        assert!(a.toast.is_some());
        // A card a rule took in asks what the rule wants.
        a.items[0].ci = Some(minato_core::list::Ci::Bad);
        a.feed('c');
        assert!(a.copy.take().unwrap().contains(a.tr("ask.red_ci")));
    }

    #[test]
    fn digits_open_the_views() {
        let mut a = app();
        assert_eq!(a.view, View::Inbox);
        for (c, v) in [('2', View::List), ('3', View::Actions), ('4', View::Board), ('5', View::Status), ('1', View::Inbox)] {
            assert!(a.feed(c));
            assert_eq!(a.view, v);
        }
        assert!(!a.feed('6'));
        assert!(!a.feed('z'));
    }

    #[test]
    fn tab_cycles_the_views_round() {
        let mut a = app();
        for v in [View::List, View::Actions, View::Board, View::Status, View::Inbox] {
            assert!(a.cycle_view(false));
            assert_eq!(a.view, v);
        }
        assert!(a.cycle_view(true));
        assert_eq!(a.view, View::Status);
        assert!(a.cycle_view(true));
        assert_eq!(a.view, View::Board);
        a.mode = Mode::Notes;
        assert!(!a.cycle_view(false));
        assert_eq!(a.view, View::Board);
    }

    #[test]
    fn w_watches_a_run_and_it_rings_when_it_ends() {
        let mut a = app();
        a.feed('3');
        a.runs = vec![run(1, actions::Status::Running, None), run(2, actions::Status::Ok, None)];
        let words = |a: &App| a.keys_now().into_iter().map(|(_, w)| w).collect::<Vec<_>>();
        assert!(words(&a).contains(&"key.watch"));
        assert!(a.feed('w'));
        assert!(a.watching(1) && a.watched.len() == 1);
        // Watching again stops it; a run that has ended is not watched.
        a.feed('w');
        assert!(a.watched.is_empty());
        a.feed('j');
        assert!(!words(&a).contains(&"key.watch"));
        a.feed('w');
        assert!(a.watched.is_empty());
        a.feed('k');
        a.feed('w');
        assert!(a.watching(1));
        // Still going: nothing rings.
        a.runs_came(&[]);
        assert!(a.rings.is_empty() && a.watching(1));
        a.runs[0].status = actions::Status::Bad;
        a.runs_came(&[]);
        assert!(a.watched.is_empty());
        assert_eq!(a.rings.len(), 1);
        assert!(a.rings[0].sound.as_deref() == Some("alert") && a.rings[0].title == a.tr("watch.failed"));
        assert!(a.rings[0].system && a.rings[0].focus);
        assert!(a.rings[0].body.contains("uchmk/kura") && a.rings[0].body.contains("check"));
    }

    #[test]
    fn a_watched_run_rings_as_the_settings_say() {
        assert_eq!(minato_core::prefs::SOUNDS, ito_notify::SOUNDS);
        let mut a = app();
        a.feed('3');
        a.runs = vec![run(1, actions::Status::Running, None), run(2, actions::Status::Running, None)];
        a.feed('w');
        a.feed('j');
        a.feed('w');
        let n = &mut a.settings.notify;
        n.system[2] = false;
        n.sound[0] = false;
        n.sounds[2] = "low".into();
        n.focus = false;
        a.runs[0].status = actions::Status::Ok;
        a.runs[1].status = actions::Status::Idle;
        a.runs_came(&[]);
        let r = &a.rings;
        assert_eq!(r.len(), 2);
        assert!(r[0].system && r[0].sound.is_none() && r[0].title == a.tr("watch.finished"));
        assert!(!r[1].system && r[1].sound.as_deref() == Some("low") && r[1].title == a.tr("watch.cancelled"));
        assert!(!r[1].focus);
        // No address, no webhook.
        assert!(r.iter().all(|r| r.hook.is_none()));
        // Said in the window either way.
        assert!(a.toast.as_ref().is_some_and(|t| t.0.starts_with(a.tr("watch.cancelled"))));
    }

    #[test]
    fn a_watched_run_goes_to_the_webhook_as_the_settings_say() {
        use ito_notify::webhook::Level;
        assert_eq!(minato_core::prefs::FORMATS, ito_notify::FORMATS);
        let mut a = app();
        a.feed('3');
        a.runs = (1..=3).map(|id| run(id, actions::Status::Running, None)).collect();
        for _ in 0..3 {
            a.feed('w');
            a.feed('j');
        }
        let n = &mut a.settings.notify;
        n.webhook_url = "http://127.0.0.1:9/hook".into();
        n.webhook_format = "json".into();
        n.webhook[2] = false;
        a.runs[0].status = actions::Status::Ok;
        a.runs[1].status = actions::Status::Bad;
        a.runs[2].status = actions::Status::Idle;
        a.runs_came(&[]);
        let hooks: Vec<_> = a.rings.iter().map(|r| r.hook.as_ref().map(|h| (h.url.as_str(), h.format.as_str(), h.level))).collect();
        let url = "http://127.0.0.1:9/hook";
        assert_eq!(hooks, [Some((url, "json", Level::Done)), Some((url, "json", Level::Error)), None]);
        assert_eq!(a.rings[1].hook.as_ref().map(|h| h.folder.as_str()), Some(a.runs[1].repo.as_str()));
    }

    #[test]
    fn a_watched_run_pushed_out_of_the_runs_is_asked_apart_and_rings() {
        let mut a = app();
        a.feed('3');
        a.runs = vec![run(1, actions::Status::Running, None)];
        a.feed('w');
        assert!(a.watched_away().is_empty());
        // Newer runs push it out of the ten asked for: it is still watched.
        a.runs = vec![run(2, actions::Status::Running, None)];
        a.runs_came(&[]);
        assert!(a.watching(1) && a.rings.is_empty());
        assert_eq!(a.watched_away().iter().map(|r| r.id).collect::<Vec<_>>(), [1]);
        // Asked apart, it has ended.
        a.runs_came(&[run(1, actions::Status::Ok, None)]);
        assert!(a.watched.is_empty());
        assert!(a.rings.len() == 1 && a.rings[0].sound.as_deref() == Some("chime") && a.rings[0].title == a.tr("watch.finished"));
    }

    #[test]
    fn y_copies_the_link_in_every_view() {
        let mut a = with_pr();
        a.items[0].url = "https://github.com/uchmk/kura/pull/3".into();
        assert!(a.feed('y'));
        assert_eq!(a.copy.take().as_deref(), Some("https://github.com/uchmk/kura/pull/3"));
        assert!(a.toast.is_some());
        a.view = View::Inbox;
        a.feed('y');
        assert_eq!(a.copy.take().as_deref(), Some("https://github.com/uchmk/kura/pull/3"));
        a.feed('3');
        a.runs = vec![run(7, actions::Status::Ok, None)];
        a.feed('y');
        assert_eq!(a.copy.take().as_deref(), Some("https://github.com/uchmk/kura/actions/runs/7"));
        // No link, nothing copied.
        a.copy_link(String::new());
        assert!(a.copy.is_none());
    }

    fn labelled(id: &str, title: &str, labels: &[&str]) -> Item {
        let mut i = pr(id);
        i.kind = Kind::Issue;
        i.title = title.into();
        i.url = format!("https://github.com/uchmk/kura/issues/{id}");
        i.labels = labels.iter().map(|l| minato_core::list::Label { name: (*l).into(), color: "ededed".into() }).collect();
        i
    }

    #[test]
    fn the_status_view_lists_lanes_releases_and_what_waits() {
        let mut a = app();
        a.viewer = Some("uchmk".into());
        a.items = vec![
            labelled("1", "Lane status: win", &["lane-status"]),
            labelled("2", "Which way?", &["question"]),
            labelled("3", "Run on the PC", &["needs-pc"]),
        ];
        a.status_repos = vec!["uchmk/kura".into(), "uchmk/ito".into()];
        let shipped = overview::Shipped {
            tag: "v1".into(),
            published_at: "2026-10-14T00:00:00Z".into(),
            files: 6,
            sums: true,
            url: "https://github.com/uchmk/kura/releases/tag/v1".into(),
        };
        a.shipped.insert("uchmk/kura".into(), Some(shipped));
        a.shipped.insert("uchmk/ito".into(), None);
        a.ahead.insert("uchmk/kura".into(), ("v1".into(), 4));
        let rows = a.status_rows();
        assert_eq!(rows.len(), 4, "a lane, one release (ito has none), two items");
        assert!(matches!(&rows[0], overview::Row::Lane(l) if l.name == "win"));
        assert!(matches!(&rows[1], overview::Row::Release { ahead: Some(4), .. }));
        assert_eq!(rows[2], overview::Row::Item(2, "needs-pc"));
        assert_eq!(rows[3], overview::Row::Item(1, "question"));
        // Counted from an older tag: not shown.
        a.ahead.insert("uchmk/kura".into(), ("v0".into(), 9));
        assert!(matches!(&a.status_rows()[1], overview::Row::Release { ahead: None, .. }));

        assert!(a.feed('5'));
        assert!(a.feed('k'), "k at the top stays");
        assert_eq!(a.status_sel, 0);
        a.feed('o');
        assert_eq!(a.open_url.take().as_deref(), Some("https://github.com/uchmk/kura/issues/1"));
        a.feed('j');
        a.feed('y');
        assert_eq!(a.copy.take().as_deref(), Some("https://github.com/uchmk/kura/releases/tag/v1"));
        a.enter();
        assert_eq!(a.open_url.take().as_deref(), Some("https://github.com/uchmk/kura/releases/tag/v1"));
        for _ in 0..9 {
            a.feed('j');
        }
        assert_eq!(a.status_sel, 3, "j stops at the last line");
        assert!(!a.feed('z'));
        // Enter on an item shows it in the list.
        a.enter();
        assert_eq!(a.view, View::List);
        assert!(a.open_url.is_none());
        // A click picks a line; a double click opens it.
        a.feed('5');
        a.click(Click::StatusRow(0, false));
        assert_eq!(a.status_sel, 0);
        assert!(a.open_url.is_none());
        a.click(Click::StatusRow(1, true));
        assert_eq!((a.status_sel, a.open_url.take().is_some()), (1, true));
    }

    #[test]
    fn other_owners_come_alongside_and_go() {
        let mut a = app();
        let entries = |a: &App, q: &str| a.palette_entries(q).into_iter().map(|x| x.pick).collect::<Vec<_>>();
        // Before GitHub says who the viewer is, there is nobody to stand beside.
        a.run_cmd(Cmd::AddOwner("rust-lang".into()));
        assert!(a.settings.also.is_empty());
        a.viewer = Some("me".into());
        assert_eq!(a.owners_shown(), "me");
        assert!(entries(&a, "+@rust-lang").contains(&Cmd::AddOwner("rust-lang".into())));
        a.run_cmd(Cmd::AddOwner("rust-lang".into()));
        assert_eq!((a.settings.owner.as_str(), a.settings.also.as_slice()), ("me", &["rust-lang".to_string()][..]));
        assert_eq!(a.owners_shown(), "me rust-lang");
        assert_eq!(a.scope_text[0], a.settings.owners_line());
        // Shown already: not offered again; the way back is.
        let e = entries(&a, "+@Rust-Lang");
        assert!(!e.iter().any(|c| matches!(c, Cmd::AddOwner(_))));
        assert!(e.contains(&Cmd::DropOwner("rust-lang".into())));
        a.run_cmd(Cmd::AddOwner("me".into()));
        assert_eq!(a.settings.also.len(), 1, "the owner is not beside itself");
        a.run_cmd(Cmd::DropOwner("RUST-LANG".into()));
        assert!(a.settings.also.is_empty());
        assert_eq!(a.owners_shown(), "me");
        assert!(!entries(&a, "").iter().any(|c| matches!(c, Cmd::DropOwner(_))));
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
            milestone: String::new(),
            reviewers: Vec::new(),
            comments: 0,
            created_at: String::new(),
            updated_at: String::new(),
            draft: false,
            merged: false,
            closed: false,
            conflict: false,
            mentioned: false,
            peek: String::new(),
            head: String::new(),
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
    fn a_field_chip_opens_its_picker_and_writes_the_one_before() {
        let mut a = with_pr();
        a.click(Click::Field(0));
        assert!(matches!(a.compose.as_ref().unwrap().picker, Some((Pick::Labels, _))));
        let c = a.compose.as_mut().unwrap();
        c.people = vec!["uchmk".into()];
        if let Some((_, pk)) = c.picker.as_mut() {
            pk.options = vec!["bug".into()];
            pk.toggle();
        }
        a.outbox.clear();
        a.click(Click::Field(1));
        assert!(matches!(a.compose.as_ref().unwrap().picker, Some((Pick::Assignees, _))));
        let keys: Vec<&str> = a.outbox.iter().map(|o| o.key.as_str()).collect();
        assert_eq!(keys, ["write:0:labels"]);
        // Status needs the item in the Project: it only says so.
        a.click(Click::Field(2));
        assert!(a.compose.as_ref().unwrap().picker.is_none());
    }

    #[test]
    fn the_lists_are_asked_once_and_reach_a_picker_opened_before_them() {
        let mut a = with_pr();
        let repo = a.selected().unwrap().repo.clone();
        a.feed('r');
        let keys: Vec<String> = a.outbox.iter().map(|o| o.key.clone()).collect();
        assert_eq!(keys, [format!("labels:{repo}"), format!("people:{repo}"), format!("miles:{repo}")]);
        a.open_pick(1);
        assert!(a.compose.as_ref().unwrap().picker.as_ref().unwrap().1.options.is_empty());
        a.got_lists(&repo, |l| l.people = vec!["amy".into(), "bob".into()]);
        assert_eq!(a.compose.as_ref().unwrap().picker.as_ref().unwrap().1.options, ["amy", "bob"]);
        // Opened again: what came is there at once and not asked again.
        a.close_compose();
        a.outbox.clear();
        a.feed('r');
        assert!(a.outbox.is_empty());
        assert_eq!(a.compose.as_ref().unwrap().people, ["amy", "bob"]);
        // Another repository's list is kept but not put in this box.
        a.got_lists("o/other", |l| l.labels = vec!["x".into()]);
        assert!(a.compose.as_ref().unwrap().labels.is_empty() && a.lists["o/other"].labels == ["x"]);
        a.lists_failed(&repo);
        assert_eq!(a.ask_lists(&repo).len(), 3);
    }

    #[test]
    fn an_empty_list_that_came_is_not_loading_and_pickers_start_from_the_item() {
        let mut a = with_pr();
        let repo = a.selected().unwrap().repo.clone();
        let id = a.selected().unwrap().id.clone();
        let it = a.items.iter_mut().find(|i| i.id == id).unwrap();
        (it.milestone, it.reviewers) = ("v1".into(), vec!["amy".into()]);
        a.feed('r');
        a.open_pick(3);
        assert!(!a.compose.as_ref().unwrap().picker_ready());
        a.got_lists(&repo, |l| (l.milestones, l.came[2]) = (Vec::new(), true));
        assert!(a.compose.as_ref().unwrap().picker_ready());
        assert_eq!(a.compose.as_ref().unwrap().picker.as_ref().unwrap().1.chosen, ["v1"]);
        a.got_lists(&repo, |l| (l.people, l.came[1]) = (vec!["amy".into(), "bob".into()], true));
        // Reviewers: bob ticked is asked, amy unticked is taken off.
        a.outbox.clear();
        a.open_pick(5);
        let pk = &mut a.compose.as_mut().unwrap().picker.as_mut().unwrap().1;
        assert!(pk.is_chosen("amy"));
        pk.toggle();
        pk.step(true);
        pk.toggle();
        a.apply_pick();
        let keys: Vec<&str> = a.outbox.iter().map(|o| o.key.as_str()).collect();
        assert_eq!(keys, ["write:0:reviewers", "write:0:unreview"]);
    }

    #[test]
    fn the_owner_is_the_settings_one_else_the_viewer() {
        let mut a = app();
        assert_eq!(a.owner(), None);
        a.viewer = Some("uchmk".into());
        assert_eq!(a.owner(), Some("uchmk"));
        a.settings.owner = "rust-lang".into();
        assert_eq!(a.owner(), Some("rust-lang"));
    }

    #[test]
    fn the_next_box_button_is_its_key() {
        let mut a = app();
        a.feed('3');
        a.runs = vec![run(1, actions::Status::Bad, None)];
        a.click(Click::Key('R'));
        assert_eq!(a.outbox.len(), 1);
        assert_eq!(a.outbox[0].key, "write:1:rerun");
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
        let job = |id, name: &str, status| actions::Job {
            id,
            name: name.into(),
            status,
            started_at: String::new(),
            completed_at: String::new(),
            url: String::new(),
        };
        a.run_detail = Some(RunDetail {
            id: 1,
            jobs: Some(vec![job(10, "lint", actions::Status::Ok), job(11, "Test (linux)", actions::Status::Bad)]),
            workflow: Some(workflow::parse(yaml)),
            yaml_tried: true,
            ..Default::default()
        });
    }

    #[test]
    fn d_opens_the_form_when_the_workflow_has_inputs_and_sends_them() {
        let mut a = app();
        with_detail(
            &mut a,
            "on:
  workflow_dispatch:
    inputs:
      who:
        required: true
jobs:
  lint:
    runs-on: x
",
        );
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
    fn d_asks_before_starting_without_inputs_and_l_opens_the_failed_log() {
        let mut a = app();
        with_detail(
            &mut a,
            "jobs:
  lint:
    runs-on: x
",
        );
        a.feed('D');
        assert!(a.overlay.is_none());
        assert_eq!((a.confirm.as_deref(), a.confirm_act), (Some("1"), Confirm::Dispatch));
        assert!(a.outbox.is_empty(), "nothing starts before Enter");
        assert!(a.escape() && a.confirm.is_none() && a.outbox.is_empty());
        a.feed('D');
        assert!(a.enter() && a.confirm.is_none());
        assert_eq!(a.outbox.len(), 1);
        assert_eq!(a.outbox[0].key, "write:1:dispatch");
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
        // Tab turns the help's page, not the view.
        assert!(a.cycle_view(false));
        assert!(a.help_rules && a.view == View::Inbox);
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
