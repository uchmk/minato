//! The app's state and what the keys do to it. Nothing here draws or reads
//! a file, so the keys can be tested without a window.

use std::time::{Duration, Instant};

use std::collections::HashMap;

use minato_core::inbox::{Card, InboxState, Mark, Rules};
use minato_core::list::{Detail, Item, ListState};

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
                ("o", "key.browser"),
                ("f F t", "key.filter"),
            ],
            View::List => &[
                ("j k", "key.next"),
                ("h l", "key.columns"),
                ("r", "key.write"),
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
            View::Board => &[("h l", "key.columns"), ("j k", "key.next"), ("H L", "key.card"), ("Enter", "key.open")],
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
    pub quit: bool,
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
        match (self.mode, c) {
            (Mode::Help, '?') => self.mode = Mode::Main,
            (Mode::Settings, _) | (Mode::Help, _) => return false,
            (Mode::Main, '?') => self.mode = Mode::Help,
            (Mode::Main, ',') => self.mode = Mode::Settings,
            (Mode::Main, c) => match View::from_digit(c) {
                Some(v) => self.view = v,
                None if self.view == View::List => return self.list_key(c),
                None if self.view == View::Inbox => return self.inbox_key(c),
                None => return false,
            },
        }
        true
    }

    /// A key in the list view: the columns' own, and `o` for the browser.
    fn list_key(&mut self, c: char) -> bool {
        let me = self.viewer.clone().unwrap_or_default();
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
        self.mode == Mode::Main && self.view == View::List && self.list_key(' ')
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
}
