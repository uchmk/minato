//! The inbox (docs/design.md, 「1 受信箱」): what is your turn. Seven rules
//! pick items out of the list's data, three groups sort them, and `d` `s`
//! `u` are local marks that never touch GitHub. No egui; tested on its own.

use std::collections::HashMap;

use crate::list::{Ci, Item, Kind};
use crate::notes::Tone;

/// The rules that bring an item into the inbox, in the design's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// My PR whose CI is red.
    RedCi,
    /// My PR that is green and ready to merge.
    Mergeable,
    /// A question (label `question`) that is mine or asked of me.
    Question,
    /// Label `needs-owner`.
    NeedsOwner,
    /// A `finding` nobody has taken.
    Finding,
    /// Label `lane-stalled`.
    LaneStalled,
    /// A PR from dependabot (off by default).
    Dependabot,
    /// Someone else's issue or PR that mentions me.
    Mention,
    /// A repository whose default branch has a red CI (a stub item, id `main:...`).
    MainRed,
}

pub const RULES: [Rule; 9] = [Rule::RedCi, Rule::Mergeable, Rule::Question, Rule::NeedsOwner, Rule::Finding, Rule::LaneStalled, Rule::Dependabot, Rule::Mention, Rule::MainRed];

impl Rule {
    pub fn index(self) -> usize {
        RULES.iter().position(|r| *r == self).unwrap_or(0)
    }

    /// The i18n key of the one-line reason.
    pub fn why(self) -> &'static str {
        match self {
            Rule::RedCi => "why.red_ci",
            Rule::Mergeable => "why.mergeable",
            Rule::Question => "why.question",
            Rule::NeedsOwner => "why.needs_owner",
            Rule::Finding => "why.finding",
            Rule::LaneStalled => "why.lane_stalled",
            Rule::Dependabot => "why.dependabot",
            Rule::Mention => "why.mention",
            Rule::MainRed => "why.main_red",
        }
    }

    /// The key to press next and the i18n key of its name.
    pub fn next(self) -> (&'static str, &'static str) {
        match self {
            Rule::RedCi => ("o", "next.open"),
            Rule::Mergeable => ("m", "next.merge"),
            Rule::Question => ("r", "next.reply"),
            Rule::NeedsOwner => ("r", "next.answer"),
            Rule::Finding => ("a", "next.take"),
            Rule::LaneStalled => ("o", "next.open"),
            Rule::Dependabot => ("o", "next.open"),
            Rule::Mention => ("r", "next.reply"),
            Rule::MainRed => ("o", "next.open"),
        }
    }

    /// The i18n key of the note under the next step: what to look at.
    pub fn note(self) -> &'static str {
        match self {
            Rule::RedCi => "note.red_ci",
            Rule::Mergeable => "note.mergeable",
            Rule::Question => "note.question",
            Rule::NeedsOwner => "note.needs_owner",
            Rule::Finding => "note.finding",
            Rule::LaneStalled => "note.lane_stalled",
            Rule::Dependabot => "note.dependabot",
            Rule::Mention => "note.mention",
            Rule::MainRed => "note.main_red",
        }
    }

    /// The colour of the card's edge and the next box.
    pub fn tone(self) -> Tone {
        match self {
            Rule::RedCi | Rule::MainRed => Tone::Bad,
            Rule::Mergeable => Tone::Ok,
            Rule::Question | Rule::NeedsOwner => Tone::Ask,
            Rule::LaneStalled => Tone::Run,
            Rule::Dependabot => Tone::Idle,
            Rule::Finding | Rule::Mention => Tone::Accent,
        }
    }
}

/// Which rules are on (`.0`) and their priority (`.1`: rule indexes, the
/// first that fits an item is its reason).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules(pub [bool; 9], pub [usize; 9]);

impl Default for Rules {
    fn default() -> Self {
        Rules([true, true, true, true, true, true, false, true, true], [0, 1, 2, 3, 4, 5, 6, 7, 8])
    }
}

impl Rules {
    /// Move the rule at place `at` of the priority one place (`-1` up, `1` down).
    pub fn shift(&mut self, at: usize, step: isize) {
        let to = at as isize + step;
        if (0..9).contains(&to) {
            self.1.swap(at, to as usize);
        }
    }
}

fn has_label(item: &Item, name: &str) -> bool {
    item.labels.iter().any(|l| l.name == name)
}

/// The first rule that takes `item` in, if any.
pub fn why(item: &Item, me: &str, rules: &Rules) -> Option<Rule> {
    let mine = item.author == me;
    let pr = item.kind == Kind::Pr;
    rules.1.iter().map(|&k| RULES[k]).filter(|r| rules.0[r.index()]).find(|r| match r {
        Rule::RedCi => pr && mine && item.ci == Some(Ci::Bad),
        Rule::Mergeable => pr && mine && !item.draft && !item.merged && !item.conflict && item.ci == Some(Ci::Ok),
        Rule::Question => has_label(item, "question") && (mine || item.assignees.iter().any(|a| a == me)),
        Rule::NeedsOwner => has_label(item, "needs-owner"),
        Rule::Finding => has_label(item, "finding") && item.assignees.is_empty(),
        Rule::LaneStalled => has_label(item, "lane-stalled"),
        Rule::Dependabot => pr && item.author.starts_with("dependabot"),
        Rule::Mention => item.mentioned && !mine && item.kind != Kind::Discussion,
        Rule::MainRed => item.id.starts_with("main:"),
    })
}

/// The three groups of the left column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    /// Your turn.
    Mine,
    /// Yours, but waiting for someone else.
    Wait,
    /// Marked done today.
    Done,
}

/// A local mark on an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// Done at this time (seconds since the epoch).
    Done(i64),
    /// Hidden until this time.
    Later(i64),
}

/// The marks as JSON, for the file (`{"id":["done",t]}`).
pub fn marks_json(marks: &HashMap<String, Mark>) -> String {
    let mut m = serde_json::Map::new();
    for (id, mark) in marks {
        let (word, t) = match mark {
            Mark::Done(t) => ("done", t),
            Mark::Later(t) => ("later", t),
        };
        m.insert(id.clone(), serde_json::json!([word, t]));
    }
    serde_json::Value::Object(m).to_string()
}

/// The marks in `text`; what cannot be read is left out.
pub fn marks_parse(text: &str) -> HashMap<String, Mark> {
    let Ok(serde_json::Value::Object(m)) = serde_json::from_str(text) else { return HashMap::new() };
    m.iter()
        .filter_map(|(id, v)| {
            let t = v.get(1)?.as_i64()?;
            match v.get(0)?.as_str()? {
                "done" => Some((id.clone(), Mark::Done(t))),
                "later" => Some((id.clone(), Mark::Later(t))),
                _ => None,
            }
        })
        .collect()
}

/// One card of the inbox.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Card {
    /// Index into the items.
    pub at: usize,
    pub group: Group,
    pub rule: Option<Rule>,
}

/// A day, for "done today".
const DAY: i64 = 24 * 3600;

/// Sort the items into cards: Mine, then Wait, then Done.
pub fn cards(items: &[Item], me: &str, rules: &Rules, marks: &HashMap<String, Mark>, now: i64) -> Vec<Card> {
    let mut out = Vec::new();
    for (at, item) in items.iter().enumerate() {
        let rule = why(item, me, rules);
        let group = match marks.get(&item.id) {
            Some(Mark::Later(until)) if *until > now => continue,
            Some(Mark::Done(t)) => {
                if now - *t < DAY {
                    Group::Done
                } else {
                    continue;
                }
            }
            _ => {
                if rule.is_some() {
                    Group::Mine
                } else if item.author == me && item.kind != Kind::Discussion {
                    Group::Wait
                } else {
                    continue;
                }
            }
        };
        out.push(Card { at, group, rule });
    }
    out.sort_by_key(|c| match c.group {
        Group::Mine => 0,
        Group::Wait => 1,
        Group::Done => 2,
    });
    out
}

/// The status filter of the lower-left chips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    All,
    Mine,
    Wait,
    Done,
}

impl Status {
    fn next(self) -> Status {
        match self {
            Status::All => Status::Mine,
            Status::Mine => Status::Wait,
            Status::Wait => Status::Done,
            Status::Done => Status::All,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Status::All => "inbox.all",
            Status::Mine => "inbox.mine",
            Status::Wait => "inbox.wait",
            Status::Done => "inbox.done",
        }
    }

    /// Whether a card of group `g` passes this filter.
    pub fn takes(self, g: Group) -> bool {
        matches!((self, g), (Status::All, _) | (Status::Mine, Group::Mine) | (Status::Wait, Group::Wait) | (Status::Done, Group::Done))
    }
}

/// What the inbox has selected and filtered.
#[derive(Clone, Debug, PartialEq)]
pub struct InboxState {
    pub sel: usize,
    pub status: Status,
    pub repo: Option<String>,
    pub tag: Option<String>,
    /// For `u`: the item and the mark it had before.
    pub undo: Vec<(String, Option<Mark>)>,
    /// Lines the detail is scrolled by.
    pub scroll: f32,
    /// The words of the search box (`/`).
    pub find: crate::find::Query,
}

impl Default for InboxState {
    fn default() -> Self {
        InboxState { sel: 0, status: Status::All, repo: None, tag: None, undo: Vec::new(), scroll: 0.0, find: Default::default() }
    }
}

/// The next value after `cur` in `all` (and back to `None` after the last).
fn cycle(all: &[String], cur: &Option<String>) -> Option<String> {
    match cur {
        None => all.first().cloned(),
        Some(c) => all.iter().position(|x| x == c).and_then(|k| all.get(k + 1)).cloned(),
    }
}

impl InboxState {
    /// The cards that pass the status, repository and tag filters and the
    /// search.
    pub fn shown(&self, all: &[Card], items: &[Item]) -> Vec<Card> {
        all.iter()
            .copied()
            .filter(|c| {
                let i = &items[c.at];
                self.status.takes(c.group)
                    && self.repo.as_ref().is_none_or(|r| *r == i.repo)
                    && self.tag.as_ref().is_none_or(|t| i.labels.iter().any(|l| l.name == *t))
                    && self.find.takes(i)
            })
            .collect()
    }

    /// The repositories and tags of the cards (before the repo/tag filters).
    pub fn repos_and_tags(all: &[Card], items: &[Item]) -> (Vec<String>, Vec<String>) {
        let mut repos: Vec<String> = all.iter().map(|c| items[c.at].repo.clone()).collect();
        let mut tags: Vec<String> = all.iter().flat_map(|c| items[c.at].labels.iter().map(|l| l.name.clone())).collect();
        repos.sort();
        repos.dedup();
        tags.sort();
        tags.dedup();
        (repos, tags)
    }

    /// `j` `k` `f` `F` `t` and the detail's `Space`; true if the key was used.
    pub fn key(&mut self, c: char, all: &[Card], items: &[Item]) -> bool {
        let n = self.shown(all, items).len();
        match c {
            'j' => self.sel = (self.sel + 1).min(n.saturating_sub(1)),
            'k' => self.sel = self.sel.saturating_sub(1),
            ' ' => {
                self.scroll += 6.0;
                return true;
            }
            'f' => self.status = self.status.next(),
            'F' => {
                let (repos, _) = Self::repos_and_tags(all, items);
                self.repo = cycle(&repos, &self.repo);
            }
            't' => {
                let (_, tags) = Self::repos_and_tags(all, items);
                self.tag = cycle(&tags, &self.tag);
            }
            _ => return false,
        }
        self.scroll = 0.0;
        self.clamp(all, items);
        true
    }

    pub fn clamp(&mut self, all: &[Card], items: &[Item]) {
        let n = self.shown(all, items).len();
        self.sel = self.sel.min(n.saturating_sub(1));
    }

    /// `gg` and `G`: the first card or the last.
    pub fn jump(&mut self, end: bool, all: &[Card], items: &[Item]) {
        self.sel = if end { usize::MAX } else { 0 };
        self.scroll = 0.0;
        self.clamp(all, items);
    }

    /// The selected card, if any.
    pub fn selected(&self, all: &[Card], items: &[Item]) -> Option<Card> {
        self.shown(all, items).get(self.sel).copied()
    }

    /// Put `mark` on `id`, remembering the old one for `u`.
    pub fn mark(&mut self, marks: &mut HashMap<String, Mark>, id: &str, mark: Mark) {
        self.undo.push((id.to_string(), marks.insert(id.to_string(), mark)));
    }

    /// Take back the last mark; the id it was on.
    pub fn undo_last(&mut self, marks: &mut HashMap<String, Mark>) -> Option<String> {
        let (id, prev) = self.undo.pop()?;
        match prev {
            Some(m) => marks.insert(id.clone(), m),
            None => marks.remove(&id),
        };
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_survive_a_round_trip() {
        let mut m = HashMap::new();
        m.insert("a".to_string(), Mark::Done(5));
        m.insert("b".to_string(), Mark::Later(9));
        assert_eq!(marks_parse(&marks_json(&m)), m);
        assert!(marks_parse("garbage").is_empty());
        assert_eq!(marks_parse(r#"{"a":["x",1],"b":["done",2]}"#).len(), 1);
    }
    use crate::list::Label;

    fn item(id: &str, kind: Kind, author: &str) -> Item {
        Item {
            id: id.into(),
            kind,
            repo: "uchmk/kura".into(),
            number: 1,
            title: id.into(),
            url: String::new(),
            author: author.into(),
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

    fn labelled(mut i: Item, l: &str) -> Item {
        i.labels.push(Label { name: l.into(), color: "000000".into() });
        i
    }

    #[test]
    fn rules_pick_the_right_items() {
        let r = Rules::default();
        let mut red = item("a", Kind::Pr, "me");
        red.ci = Some(Ci::Bad);
        assert_eq!(why(&red, "me", &r), Some(Rule::RedCi));
        assert_eq!(why(&red, "you", &r), None, "someone else's red PR is not my turn");
        let mut green = item("b", Kind::Pr, "me");
        green.ci = Some(Ci::Ok);
        assert_eq!(why(&green, "me", &r), Some(Rule::Mergeable));
        green.draft = true;
        assert_eq!(why(&green, "me", &r), None);
        green.draft = false;
        green.conflict = true;
        assert_eq!(why(&green, "me", &r), None, "a PR with conflicts is not ready to merge");
        assert_eq!(why(&labelled(item("c", Kind::Issue, "x"), "needs-owner"), "me", &r), Some(Rule::NeedsOwner));
        let f = labelled(item("d", Kind::Issue, "x"), "finding");
        assert_eq!(why(&f, "me", &r), Some(Rule::Finding));
        let mut taken = f.clone();
        taken.assignees.push("y".into());
        assert_eq!(why(&taken, "me", &r), None);
        assert_eq!(why(&labelled(item("e", Kind::Issue, "x"), "lane-stalled"), "me", &r), Some(Rule::LaneStalled));
    }

    #[test]
    fn mentions_and_a_red_main_count_and_the_priority_decides() {
        let r = Rules::default();
        let mut m = item("m", Kind::Issue, "x");
        m.mentioned = true;
        assert_eq!(why(&m, "me", &r), Some(Rule::Mention));
        m.author = "me".into();
        assert_eq!(why(&m, "me", &r), None, "my own item is not a mention");
        assert_eq!(why(&item("main:uchmk/kura", Kind::Issue, ""), "me", &r), Some(Rule::MainRed));
        let mut both = labelled(item("b", Kind::Issue, "x"), "finding");
        both.mentioned = true;
        assert_eq!(why(&both, "me", &r), Some(Rule::Finding));
        let mut up = r;
        for at in (1..=7).rev() {
            up.shift(at, -1);
        }
        assert_eq!(up.1[0], 7);
        assert_eq!(why(&both, "me", &up), Some(Rule::Mention));
    }

    #[test]
    fn dependabot_is_off_until_turned_on() {
        let d = item("d", Kind::Pr, "dependabot[bot]");
        assert_eq!(why(&d, "me", &Rules::default()), None);
        let mut on = Rules::default();
        on.0[Rule::Dependabot.index()] = true;
        assert_eq!(why(&d, "me", &on), Some(Rule::Dependabot));
    }

    #[test]
    fn a_rule_switched_off_does_not_fire() {
        let mut off = Rules::default();
        off.0[Rule::NeedsOwner.index()] = false;
        assert_eq!(why(&labelled(item("c", Kind::Issue, "x"), "needs-owner"), "me", &off), None);
    }

    #[test]
    fn groups_come_in_order_and_marks_move_cards() {
        let mut red = item("red", Kind::Pr, "me");
        red.ci = Some(Ci::Bad);
        let waiting = item("wait", Kind::Pr, "me");
        let other = item("other", Kind::Issue, "x");
        let items = vec![waiting, red, other];
        let mut marks = HashMap::new();
        let c = cards(&items, "me", &Rules::default(), &marks, 1000);
        assert_eq!(c.iter().map(|c| (items[c.at].id.as_str(), c.group)).collect::<Vec<_>>(), [("red", Group::Mine), ("wait", Group::Wait)]);
        marks.insert("red".to_string(), Mark::Done(900));
        let c = cards(&items, "me", &Rules::default(), &marks, 1000);
        assert_eq!(items[c[1].at].id, "red");
        assert_eq!(c[1].group, Group::Done);
        // A day later it is gone.
        let c = cards(&items, "me", &Rules::default(), &marks, 900 + DAY + 1);
        assert!(c.iter().all(|c| items[c.at].id != "red"));
    }

    #[test]
    fn later_hides_until_its_time() {
        let mut red = item("red", Kind::Pr, "me");
        red.ci = Some(Ci::Bad);
        let items = vec![red];
        let mut marks = HashMap::new();
        marks.insert("red".to_string(), Mark::Later(2000));
        assert!(cards(&items, "me", &Rules::default(), &marks, 1000).is_empty());
        assert_eq!(cards(&items, "me", &Rules::default(), &marks, 2000).len(), 1);
    }

    #[test]
    fn undo_restores_the_old_mark() {
        let mut st = InboxState::default();
        let mut marks = HashMap::new();
        st.mark(&mut marks, "a", Mark::Later(5));
        st.mark(&mut marks, "a", Mark::Done(6));
        assert_eq!(st.undo_last(&mut marks).as_deref(), Some("a"));
        assert_eq!(marks.get("a"), Some(&Mark::Later(5)));
        st.undo_last(&mut marks);
        assert!(marks.is_empty());
        assert_eq!(st.undo_last(&mut marks), None);
    }

    #[test]
    fn filters_cycle_and_narrow() {
        let mut a = labelled(item("a", Kind::Pr, "me"), "bug");
        a.ci = Some(Ci::Bad);
        let mut b = item("b", Kind::Pr, "me");
        b.repo = "uchmk/ito".into();
        b.ci = Some(Ci::Bad);
        let items = vec![a, b];
        let all = cards(&items, "me", &Rules::default(), &HashMap::new(), 0);
        let mut st = InboxState::default();
        assert_eq!(st.shown(&all, &items).len(), 2);
        st.key('F', &all, &items);
        assert_eq!(st.repo.as_deref(), Some("uchmk/ito"));
        assert_eq!(st.shown(&all, &items).len(), 1);
        st.key('F', &all, &items);
        st.key('F', &all, &items);
        assert_eq!(st.repo, None, "after the last it goes back to all");
        st.key('t', &all, &items);
        assert_eq!(st.shown(&all, &items).len(), 1);
        st.key('f', &all, &items);
        assert_eq!(st.status, Status::Mine);
        st.key('f', &all, &items);
        assert_eq!(st.shown(&all, &items).len(), 0, "nothing is waiting");
        assert_eq!(st.sel, 0);
    }
}
