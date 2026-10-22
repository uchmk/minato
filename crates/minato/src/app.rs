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
use minato_core::security;

use minato_core::workflow::{self, Workflow};

use ito_keymap::Match;
use ito_keys::Key;
use ito_layout::{Dir, Node};

use crate::keymap::{Cmd as KeyCmd, Ctx, Fire, Keys};
use crate::overlay::{Form, LogView, Out, Overlay, K};
use crate::prefs::Prefs;

/// The six views, opened by `1` to `6` (docs/design.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Inbox,
    List,
    Actions,
    Board,
    Status,
    Security,
}

impl View {
    pub const ALL: [View; 6] = [View::Inbox, View::List, View::Actions, View::Board, View::Status, View::Security];

    /// The digit that opens it.
    pub fn digit(self) -> char {
        match self {
            View::Inbox => '1',
            View::List => '2',
            View::Actions => '3',
            View::Board => '4',
            View::Status => '5',
            View::Security => '6',
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
            View::Security => "view.security",
        }
    }

    /// Its section of keymap.toml.
    pub fn layer(self) -> &'static str {
        match self {
            View::Inbox => "inbox",
            View::List => "list",
            View::Actions => "actions",
            View::Board => "board",
            View::Status => "status",
            View::Security => "security",
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
            View::Security => "empty.security",
        }
    }

    /// The keys that work in this view: `(keys, word-table key)`, the one to
    /// recommend first. The keybar shows as many as fit; `?` shows them all.
    /// `{action}` is the keys keymap.toml gives the action ([`Keys::fill`]).
    pub fn keys(self) -> &'static [(&'static str, &'static str)] {
        match self {
            View::Inbox => &[
                ("{down} {up}", "key.next"),
                ("{details}", "key.details"),
                ("{reply}", "key.write"),
                ("{new_issue}", "key.new"),
                ("{repo_settings}", "key.repo"),
                ("{answer}", "key.answer"),
                ("{done}", "key.done"),
                ("{later}", "key.later"),
                ("{undo}", "key.undo"),
                ("{merge}", "key.merge"),
                ("{close}", "key.close"),
                ("{assign}", "key.assign"),
                ("{open}", "key.browser"),
                ("{open_outside}", "key.outside"),
                ("{copy_link}", "key.copy_link"),
                ("{prompt}", "key.prompt"),
                ("{filter_status} {filter_repo} {filter_tag}", "key.filter"),
                ("{find}", "key.find"),
            ],
            View::List => &[
                ("{down} {up}", "key.next"),
                ("{left} {right}", "key.columns"),
                ("{reply}", "key.write"),
                ("{new_issue}", "key.new"),
                ("{repo_settings}", "key.repo"),
                ("{answer}", "key.answer"),
                ("{assign}", "key.assign"),
                ("{merge}", "key.merge"),
                ("{close}", "key.close"),
                ("{closed}", "key.closed"),
                ("{save_search}", "key.save_search"),
                ("{forget_search}", "key.forget_search"),
                ("i", "key.inbox_add"),
                ("{open}", "key.browser"),
                ("{open_outside}", "key.outside"),
                ("{copy_link}", "key.copy_link"),
                ("{find}", "key.find"),
            ],
            View::Actions => &[
                ("{down} {up}", "key.next"),
                ("{watch}", "key.watch"),
                ("{rerun}", "key.rerun"),
                ("{cancel}", "key.cancel"),
                ("{dispatch}", "key.dispatch"),
                ("{logs}", "key.logs"),
                ("{log}", "key.log"),
                ("{open_pr}", "run.open_pr"),
                ("{open}", "key.browser"),
                ("{open_outside}", "key.outside"),
                ("{copy_link}", "key.copy_link"),
            ],
            View::Board => &[
                ("{left} {right}", "key.columns"),
                ("{down} {up}", "key.next"),
                ("{card_left} {card_right}", "key.card"),
                ("{enter}", "key.open"),
                ("{undo}", "key.undo"),
                ("{open}", "key.browser"),
                ("{open_outside}", "key.outside"),
                ("{copy_link}", "key.copy_link"),
            ],
            View::Status => &[
                ("{down} {up}", "key.next"),
                ("{enter}", "key.open"),
                ("{open}", "key.browser"),
                ("{open_outside}", "key.outside"),
                ("{copy_link}", "key.copy_link"),
            ],
            View::Security => &[
                ("{down} {up}", "key.next"),
                ("{enter}", "key.open"),
                ("{open}", "key.browser"),
                ("{open_outside}", "key.outside"),
                ("{copy_link}", "key.copy_link"),
                ("{dismiss}", "key.dismiss"),
                ("{undo}", "key.undo"),
            ],
        }
    }
}

/// The keys that work in every view, `{action}` as in [`View::keys`].
pub const GLOBAL_KEYS: [(&str, &str); 11] = [
    ("{view inbox} {view list} {view actions} {view board} {view status} {view security}", "key.views"),
    ("{next_view} {prev_view}", "key.cycle"),
    ("{top} {bottom}", "key.ends"),
    ("Ctrl+,", "key.settings"),
    ("Ctrl+= - 0", "key.scale"),
    ("Ctrl+Shift+P", "key.palette"),
    ("{notes}", "key.notes"),
    ("{undo}", "key.undo"),
    ("F5", "key.sync"),
    ("{owner}", "key.owner"),
    ("F1 {help}", "key.help"),
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
    /// A row of the security view (its place among `alerts`); opened on a double click.
    SecurityRow(usize, bool),
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
    /// The status view's lines.
    Status,
    /// The security view's alerts.
    Security,
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
    marks_tx: Option<ito_common::kept::Writer>,
    /// minato's own settings: whose repositories, the inbox rules and the
    /// writing box's fields.
    pub settings: Settings,
    /// The settings' one-line fields while they are typed in.
    pub fields_typed: ito_prefs::Drafts,
    /// The text boxes of the owner and the board's Project in the settings
    /// (taken into `settings` on Enter).
    pub scope_text: [String; 2],
    /// The Startup card: whether minato starts at sign-in as the OS says
    /// (`None` until it answers) and the answer on its way.
    pub startup: Option<bool>,
    pub startup_rx: Option<std::sync::mpsc::Receiver<bool>>,
    settings_tx: Option<std::sync::mpsc::Sender<Settings>>,
    /// The text box of the saved searches in the settings (one per line).
    pub searches_text: String,
    seen_tx: Option<ito_common::kept::Writer>,
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
    /// The recent merged and closed PRs of the repositories the door was asked
    /// about (by lower-case `owner/name`), and the ones it was asked about
    /// since the net last looked.
    pub past: HashMap<String, Vec<Item>>,
    pub past_wanted: std::collections::HashSet<String>,
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
    /// The keys in force (keymap.toml over the built-in ones).
    pub keys: Keys,
    /// The keys of a chord typed so far (`g` of `g g`).
    pub key_pending: Vec<Key>,
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
    /// The security and status views: the rows at the left, the detail at the right (not kept).
    pub security_split: Node<Pane>,
    pub status_split: Node<Pane>,
    pub list_split: Node<Pane>,
    pub acts_split: Node<Pane>,
    /// The runs watched with `w`, as last seen: a toast and a sound when one
    /// ends.
    pub watched: Vec<Run>,
    /// The toasts of the system that wait to be shown (the network takes them).
    pub rings: Vec<Ring>,
    /// The status view's selection (a row of `status_rows`).
    pub status_sel: usize,
    /// The open security alerts of the watched repositories, sorted; the
    /// security view's selection; the alerts dismissed here (to reopen with
    /// `u`); how many repositories could not be read; whether any answer came.
    pub alerts: Vec<security::Alert>,
    pub security_sel: usize,
    pub sec_undo: Vec<security::Alert>,
    pub sec_unreadable: usize,
    pub sec_loaded: bool,
    /// Set when `alerts` changed here, so the network rebuilds the inbox's stubs.
    pub alerts_changed: bool,
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
    /// The messages for the webhooks that are told of it.
    pub hooks: Vec<Hook>,
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
    /// Dismiss a security alert (the confirm is the alert's key).
    Dismiss,
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
            keys: Keys::default(),
            key_pending: Vec::new(),
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
            startup: None,
            startup_rx: None,
            settings_tx: None,
            searches_text: String::new(),
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
            past: HashMap::new(),
            past_wanted: std::collections::HashSet::new(),
            outbox: Vec::new(),
            quit: false,
            // The mock's widths at 1280 px: 400 | rest, and 210 | 470 | rest.
            inbox_split: two(0.31),
            security_split: two(0.58),
            status_split: two(0.58),
            list_split: three(0.165, 0.44),
            acts_split: three(0.165, 0.44),
            watched: Vec::new(),
            rings: Vec::new(),
            status_sel: 0,
            alerts: Vec::new(),
            security_sel: 0,
            sec_undo: Vec::new(),
            sec_unreadable: 0,
            sec_loaded: false,
            alerts_changed: false,
            shipped: HashMap::new(),
            ahead: HashMap::new(),
            status_repos: Vec::new(),
        };
        app.set_language();
        app.load_keys();
        app
    }

    /// The keys again from keymap.toml as last read, what in it does not
    /// work said in a toast.
    pub fn load_keys(&mut self) {
        let path = self.prefs.base.as_deref().map(|b| crate::prefs::keymap_path(b).display().to_string());
        let path = path.unwrap_or_else(|| crate::keymap::FILE.to_owned());
        let user = self.prefs.keymap.as_deref().map(|t| (path.as_str(), t));
        let (keys, warnings) = Keys::load(user);
        self.keys = keys;
        self.key_pending.clear();
        if let Some(first) = warnings.first() {
            let more = warnings.len() - 1;
            let words = if more == 0 { first.clone() } else { format!("{first} (+{more})") };
            self.say(words);
        }
    }

    /// Where the keys are read now.
    pub fn key_ctx(&self) -> Ctx {
        match self.mode {
            Mode::Help => Ctx::Help,
            Mode::Notes => Ctx::Notes,
            _ => Ctx::View(self.view),
        }
    }

    /// A key pressed with nothing over the view (no box, no search): run
    /// what keymap.toml binds to it, or wait for the rest of a chord
    /// (`g g`). Returns whether it was used.
    pub fn press(&mut self, k: Key) -> bool {
        self.key_pending.push(k);
        let ctx = self.key_ctx();
        let run = match ito_keymap::resolve(self.keys.bindings(ctx), &self.key_pending) {
            Match::Exact(b) => b.run.clone(),
            Match::Pending(_) => return true,
            Match::None => {
                // A key that ends no chord may start one of its own.
                let alone = self.key_pending.len() > 1;
                self.key_pending.clear();
                return alone && self.press(k);
            }
        };
        self.key_pending.clear();
        for cmd in run {
            let KeyCmd::Run(name) = cmd else { continue };
            match crate::keymap::fire(name) {
                Some(Fire::Char(c)) => _ = self.feed(c),
                Some(Fire::Enter) => _ = self.enter(),
                Some(Fire::Tab(back)) => _ = self.cycle_view(back),
                Some(Fire::Top) => self.top(),
                None => {}
            }
        }
        true
    }

    /// `gg`: the first of the column in focus, the top of a detail.
    pub fn top(&mut self) {
        self.pending_g = false;
        let free = self.compose.is_none()
            && self.confirm.is_none()
            && self.palette.is_none()
            && self.overlay.is_none()
            && matches!(self.mode, Mode::Main | Mode::Notes);
        if free {
            self.jump(false);
            self.clamp_scroll();
        }
    }

    /// `rows` with the keys of `ctx` filled in; a row no key does is left out.
    pub fn key_rows(&self, ctx: Ctx, rows: &[(&str, &'static str)]) -> Vec<(String, &'static str)> {
        rows.iter().map(|(spec, w)| (self.keys.fill(ctx, spec), *w)).filter(|(k, _)| !k.is_empty()).collect()
    }

    /// The keys that do now in `ctx` what the character `key` did before the
    /// keys could be changed (`key` itself when it is no action; empty when
    /// no key does it).
    pub fn char_label(&self, ctx: Ctx, key: &str) -> String {
        let mut chars = key.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => match Keys::name_of(ctx, c) {
                Some(name) => self.keys.label(ctx, name),
                None => key.to_owned(),
            },
            _ => key.to_owned(),
        }
    }

    /// `keys` (`(character, words)`) as [`App::char_label`] shows them in
    /// the view, those no key does left out.
    pub fn alt_labels<'a>(&self, keys: &[(&str, &'a str)]) -> Vec<(String, &'a str)> {
        let ctx = Ctx::View(self.view);
        keys.iter().map(|(k, w)| (self.char_label(ctx, k), *w)).filter(|(k, _)| !k.is_empty()).collect()
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

    /// F12: the bug report form in the system's browser, with the version, the
    /// OS and what minato knew filled in (by name, never by path).
    pub fn bug_report(&mut self) {
        self.open_url = Some(crate::bugreport::url(
            self.prefs.base.as_deref(),
            self.prefs.common.scale_or_one(),
            ito_common::runinfo::load_from(&ito_common::runinfo::path("minato").unwrap_or_default()).as_ref(),
        ));
        self.open_outside = true;
        let text = self.tr("bugreport.opened");
        self.say(text);
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
                Wheel::Status => {
                    self.status_key(key);
                }
                Wheel::Security => {
                    self.security_key(key);
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
            View::Board | View::Status | View::Security => return,
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
            (_, View::Security) => self.security_sel = if end { self.sec_shown().len().saturating_sub(1) } else { 0 },
        }
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

    pub fn set_split(&mut self, v: View, n: Node<Pane>) {
        match v {
            View::Inbox => self.inbox_split = n,
            View::List => self.list_split = n,
            View::Actions => self.acts_split = n,
            View::Security => self.security_split = n,
            View::Status => self.status_split = n,
            View::Board => {}
        }
        if let Some(tx) = &self.state_tx {
            let _ = tx.send(minato_core::state::format(&self.columns()));
        }
    }

    /// The columns' widths as state.toml keeps them.
    pub fn columns(&self) -> minato_core::state::State {
        minato_core::state::State {
            inbox: ratios(&self.inbox_split),
            list: ratios(&self.list_split),
            actions: ratios(&self.acts_split),
            security: ratios(&self.security_split),
            status: ratios(&self.status_split),
        }
    }

    /// F5: fetch everything shown again now, not at the next turn.
    pub fn sync_now(&mut self) {
        self.sync_wanted = true;
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
        // A chord begun (`g`, `z`): Esc lets go of it, and only that.
        let chord = !self.key_pending.is_empty() || self.pending_g;
        if chord && self.confirm.is_none() && self.overlay.is_none() {
            self.key_pending.clear();
            self.pending_g = false;
            return true;
        }
        self.key_pending.clear();
        self.pending_g = false;
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

mod keys;
mod lists;
mod panes;
mod persist;
mod writes;

#[cfg(test)]
mod tests;
