//! The app's state and what the keys do to it. Nothing here draws or reads
//! a file, so the keys can be tested without a window.

use std::time::{Duration, Instant};

use std::collections::HashMap;

use minato_core::actions::{self, ActState, Artifact, Job, Run};
use minato_core::board::{self, BoardState, Project};
use minato_core::composer::Picker;
use minato_core::github::worker::Call;
use minato_core::github::write;
use minato_core::inbox::{Card, InboxState, Mark, Rules};
use minato_core::list::{Detail, Item, Kind, ListState};

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
                ("d", "key.done"),
                ("s", "key.later"),
                ("u", "key.undo"),
                ("m", "key.merge"),
                ("a", "key.assign"),
                ("o", "key.browser"),
                ("f F t", "key.filter"),
            ],
            View::List => &[
                ("j k", "key.next"),
                ("h l", "key.columns"),
                ("r", "key.write"),
                ("a", "key.assign"),
                ("i", "key.inbox_add"),
                ("o", "key.browser"),
            ],
            View::Actions => &[
                ("j k", "key.next"),
                ("R", "key.rerun"),
                ("X", "key.cancel"),
                ("D", "key.dispatch"),
                ("e", "key.logs"),
                ("o", "key.browser"),
            ],
            View::Board => &[("h l", "key.columns"), ("j k", "key.next"), ("H L", "key.card"), ("Enter", "key.open"), ("u", "key.undo"), ("o", "key.browser")],
        }
    }
}

/// The keys that work in every view.
pub const GLOBAL_KEYS: [(&str, &str); 6] = [
    ("1 2 3 4", "key.views"),
    (",", "key.settings"),
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
    /// Which inbox rules are on.
    pub rules: Rules,
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
    /// The workflow runs of the repositories, newest first.
    pub runs: Vec<Run>,
    /// The Actions view's selection and filters.
    pub acts: ActState,
    /// The jobs and artifacts of one run, once fetched.
    pub run_detail: Option<RunDetail>,
    /// Project "uchmk" with its Status columns, once fetched.
    pub board: Option<Project>,
    pub bs: BoardState,
    /// Moves that can be undone: the Project item and the option it came from.
    pub board_undo: Vec<(String, String)>,
    /// Set when the board should be fetched again at once.
    /// Calls the window has asked for; the net takes them.
    pub outbox: Vec<Outgoing>,
    pub quit: bool,
}

/// What GitHub said about one run.
pub struct RunDetail {
    pub id: u64,
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
}

impl App {
    pub fn new(prefs: Prefs) -> Self {
        let mut app = Self {
            view: View::Inbox,
            mode: Mode::Main,
            prefs,
            lang: "en".into(),
            unread: 0,
            synced: None,
            api: None,
            toast: None,
            viewer: None,
            items: Vec::new(),
            inbox: InboxState::default(),
            marks: HashMap::new(),
            rules: Rules::default(),
            list: ListState::default(),
            detail: None,
            open_url: None,
            compose: None,
            confirm: None,
            runs: Vec::new(),
            acts: ActState::default(),
            run_detail: None,
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
        if self.compose.is_some() || self.confirm.is_some() {
            return true;
        }
        match (self.mode, c) {
            (Mode::Help, '?') => self.mode = Mode::Main,
            (Mode::Settings, _) | (Mode::Help, _) => return false,
            (Mode::Main, '?') => self.mode = Mode::Help,
            (Mode::Main, ',') => self.mode = Mode::Settings,
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
            self.merge(&id);
            return true;
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
            'D' => self.queue(&format!("write:{}:dispatch", run.id), actions::dispatch(&run), Some("run.dispatched")),
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
        minato_core::inbox::cards(&self.items, me, &self.rules, &self.marks, now_epoch())
    }

    /// A key in the inbox: `d` done, `s` later (tomorrow 9:00), `u` undo,
    /// `o` the browser, and the filters and the cursor.
    fn inbox_key(&mut self, c: char) -> bool {
        let all = self.inbox_all();
        let card = self.inbox.selected(&all, &self.items);
        match c {
            'r' => self.open_compose(),
            'm' => self.ask_merge(),
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
                let all = self.inbox_all();
                self.inbox.clamp(&all, &self.items);
                let words = self.tr(if c == 'd' { "inbox.marked_done" } else { "inbox.marked_later" });
                self.say(words);
            }
            'u' => {
                if self.inbox.undo_last(&mut self.marks).is_some() {
                    let words = self.tr("inbox.undone");
                    self.say(words);
                }
            }
            _ => return self.inbox.key(c, &all, &self.items),
        }
        true
    }

    fn queue(&mut self, key: &str, call: Call, done: Option<&'static str>) {
        self.outbox.push(Outgoing { key: key.to_string(), call, done });
    }

    fn item(&self, id: &str) -> Option<Item> {
        self.items.iter().find(|i| i.id == id).cloned()
    }

    /// `r`: open the writing box on the selected item and ask for the
    /// repository's labels and people.
    pub fn open_compose(&mut self) {
        let Some(item) = self.selected().cloned() else { return };
        self.queue(&format!("labels:{}", item.repo), Call::Get(write::labels_path(&item.repo)), None);
        self.queue(&format!("people:{}", item.repo), Call::Get(write::assignees_path(&item.repo)), None);
        self.compose = Some(Compose { id: item.id, text: String::new(), preview: false, picker: None, labels: Vec::new(), people: Vec::new() });
    }

    /// Send the comment; `close` also closes the item.
    pub fn send_comment(&mut self, close: bool) {
        let Some(c) = self.compose.take() else { return };
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
        }
    }

    /// `m` in the inbox: ask before merging an open PR.
    fn ask_merge(&mut self) {
        match self.selected() {
            Some(i) if i.kind == Kind::Pr && !i.merged && !i.draft => self.confirm = Some(i.id.clone()),
            Some(_) => {
                let words = self.tr("write.not_mergeable");
                self.say(words);
            }
            None => {}
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
        };
        board::Project { id: "P".into(), title: "uchmk".into(), field: "F".into(), options: vec![col("o1", "Todo"), col("o2", "Doing")], cards: vec![card("I1", "o1"), card("I2", "o1")] }
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

    #[test]
    fn digits_open_the_views() {
        let mut a = app();
        assert_eq!(a.view, View::Inbox);
        for (c, v) in [('2', View::List), ('3', View::Actions), ('4', View::Board), ('1', View::Inbox)] {
            assert!(a.feed(c));
            assert_eq!(a.view, v);
        }
        assert!(!a.feed('5'));
        assert!(!a.feed('x'));
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
        assert_eq!(keys, ["labels:uchmk/kura", "people:uchmk/kura"]);
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
}
