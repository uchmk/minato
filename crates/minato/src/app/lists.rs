//! Owners, cards and the list view: which cards show where.

use super::*;

impl App {
    /// Open the item whose page is `url`: in the inbox if it is a card
    /// there (done today too), else in the list. False if minato does not
    /// have it.
    pub(super) fn open_item(&mut self, url: &str) -> bool {
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
    pub(super) fn open_card(&mut self, card: Card) {
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
    pub(super) fn show_in_list(&mut self, id: &str) {
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
}
