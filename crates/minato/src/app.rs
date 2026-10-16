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
    Notes,
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

    /// A key in the security view: `j k` move, `o` opens the alert on
    /// GitHub, `y` copies its link, `Enter` opens it too, `x` asks to dismiss
    /// it, `u` reopens the one dismissed last.
    fn security_key(&mut self, c: char) -> bool {
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

    /// The alerts the search box leaves, as places in `alerts`.
    pub fn sec_shown(&self) -> Vec<usize> {
        security::shown(&self.alerts, &self.inbox.find)
    }

    /// Dismiss the alert whose key is `key` (confirmed), and forget it here
    /// at once; `u` reopens it.
    fn dismiss(&mut self, key: &str) {
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
            set_ratios(&mut self.security_split, &st.security);
            set_ratios(&mut self.status_split, &st.status);
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

    /// `W`: keep the list's search (the filter, or the saved search in
    /// force) with the search box's words as a new saved search.
    fn save_search(&mut self) {
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
    fn forget_search(&mut self) {
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

#[cfg(test)]
mod tests;
