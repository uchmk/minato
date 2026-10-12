//! The list view's data and what its keys do (docs/design.md, 「2 一覧」):
//! the items, the filter column, the selection. No egui, so it is tested
//! without a window.

/// What an item is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Issue,
    Pr,
    Discussion,
}

impl Kind {
    /// The `search` qualifier for this kind of item.
    pub fn qualifier(self) -> &'static str {
        match self {
            Kind::Issue => "is:issue",
            Kind::Pr => "is:pr",
            Kind::Discussion => "",
        }
    }
}

/// A check's or a rollup's outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ci {
    Ok,
    Bad,
    Pending,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub name: String,
    /// Hex without `#`.
    pub color: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// GraphQL node id (for the detail).
    pub id: String,
    pub kind: Kind,
    /// `owner/name`
    pub repo: String,
    pub number: u64,
    pub title: String,
    pub url: String,
    pub author: String,
    pub labels: Vec<Label>,
    pub assignees: Vec<String>,
    pub comments: u32,
    /// ISO 8601 UTC, as GitHub sends it.
    pub created_at: String,
    pub updated_at: String,
    pub draft: bool,
    pub merged: bool,
    pub ci: Option<Ci>,
}

impl Item {
    /// The repository's name without the owner.
    pub fn repo_name(&self) -> &str {
        self.repo.rsplit('/').next().unwrap_or(&self.repo)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub name: String,
    pub ci: Ci,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub author: String,
    pub created_at: String,
    pub body: String,
}

/// The selected item's body, checks and conversation.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Detail {
    pub body: String,
    pub checks: Vec<Check>,
    pub timeline: Vec<Entry>,
}

/// The saved filters of the filter column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Saved {
    None,
    /// Assigned to me.
    Assigned,
    /// A PR whose CI is red.
    Red,
}

/// What the filter column has chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    pub repo: Option<String>,
    pub kind: Option<Kind>,
    pub author: Option<String>,
    pub saved: Saved,
}

impl Default for Filter {
    fn default() -> Self {
        Filter { repo: None, kind: None, author: None, saved: Saved::None }
    }
}

impl Filter {
    /// Whether `item` passes (`me` is the viewer's login, for `Assigned`).
    pub fn matches(&self, item: &Item, me: &str) -> bool {
        self.repo.as_ref().is_none_or(|r| *r == item.repo)
            && self.kind.is_none_or(|k| k == item.kind)
            && self.author.as_ref().is_none_or(|a| *a == item.author)
            && match self.saved {
                Saved::None => true,
                Saved::Assigned => item.assignees.iter().any(|a| a == me),
                Saved::Red => item.kind == Kind::Pr && item.ci == Some(Ci::Bad),
            }
    }

    /// The GitHub search this filter stands for, as shown over the list.
    pub fn query(&self, owner: &str) -> String {
        let mut q = vec!["is:open".to_string()];
        match &self.repo {
            Some(r) => q.push(format!("repo:{r}")),
            None => q.push(format!("user:{owner}")),
        }
        if let Some(k) = self.kind.map(Kind::qualifier).filter(|s| !s.is_empty()) {
            q.push(k.into());
        }
        if let Some(a) = &self.author {
            q.push(format!("author:{a}"));
        }
        match self.saved {
            Saved::None => {}
            Saved::Assigned => q.push("assignee:@me".into()),
            Saved::Red => {
                q.push("is:pr".into());
                q.push("status:failure".into());
            }
        }
        q.join(" ")
    }
}

/// One option of the filter column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    Repo(Option<String>),
    Kind(Option<Kind>),
    Author(Option<String>),
    Saved(Saved),
}

/// The options in the column's order, grouped; `None`/"all" first in each.
pub fn options(items: &[Item]) -> Vec<(&'static str, Vec<Pick>)> {
    let distinct = |f: &dyn Fn(&Item) -> String| {
        let mut v: Vec<String> = items.iter().map(f).collect();
        v.sort();
        v.dedup();
        v
    };
    let mut repos = vec![Pick::Repo(None)];
    repos.extend(distinct(&|i| i.repo.clone()).into_iter().map(|r| Pick::Repo(Some(r))));
    let mut authors = vec![Pick::Author(None)];
    authors.extend(distinct(&|i| i.author.clone()).into_iter().map(|a| Pick::Author(Some(a))));
    vec![
        ("repo", repos),
        ("kind", vec![Pick::Kind(None), Pick::Kind(Some(Kind::Issue)), Pick::Kind(Some(Kind::Pr)), Pick::Kind(Some(Kind::Discussion))]),
        ("author", authors),
        ("saved", vec![Pick::Saved(Saved::None), Pick::Saved(Saved::Assigned), Pick::Saved(Saved::Red)]),
    ]
}

/// The columns `h` and `l` move between.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Col {
    Filters,
    Items,
    Detail,
}

/// The list's selection state.
#[derive(Clone, Debug, PartialEq)]
pub struct ListState {
    pub col: Col,
    pub filter: Filter,
    /// Cursor in the filter column (over the flattened options).
    pub cursor: usize,
    /// Selected row among the filtered items.
    pub sel: usize,
    /// Lines the detail is scrolled by.
    pub scroll: f32,
}

impl Default for ListState {
    fn default() -> Self {
        ListState { col: Col::Items, filter: Filter::default(), cursor: 0, sel: 0, scroll: 0.0 }
    }
}

impl ListState {
    /// The items that pass the filter, in the order given.
    pub fn shown<'a>(&self, items: &'a [Item], me: &str) -> Vec<&'a Item> {
        items.iter().filter(|i| self.filter.matches(i, me)).collect()
    }

    /// The selected item's node id, if any.
    pub fn selected<'a>(&self, items: &'a [Item], me: &str) -> Option<&'a Item> {
        self.shown(items, me).get(self.sel).copied()
    }

    /// A key typed in the list. `n` is the filtered count and `opts` the
    /// flattened options. Returns whether it was used.
    pub fn key(&mut self, c: char, items: &[Item], me: &str) -> bool {
        let n = self.shown(items, me).len();
        let flat: Vec<Pick> = options(items).into_iter().flat_map(|(_, v)| v).collect();
        match (self.col, c) {
            (Col::Filters, 'j') => self.cursor = (self.cursor + 1).min(flat.len().saturating_sub(1)),
            (Col::Filters, 'k') => self.cursor = self.cursor.saturating_sub(1),
            (Col::Filters, ' ') => {
                if let Some(p) = flat.get(self.cursor) {
                    self.apply(p.clone());
                }
            }
            (Col::Items, 'j') => self.sel = (self.sel + 1).min(n.saturating_sub(1)),
            (Col::Items, 'k') => self.sel = self.sel.saturating_sub(1),
            (Col::Detail, 'j') => self.scroll += 1.0,
            (Col::Detail, 'k') => self.scroll = (self.scroll - 1.0).max(0.0),
            (_, 'h') => {
                self.col = match self.col {
                    Col::Detail => Col::Items,
                    _ => Col::Filters,
                }
            }
            (_, 'l') => {
                self.col = match self.col {
                    Col::Filters => Col::Items,
                    _ => Col::Detail,
                }
            }
            _ => return false,
        }
        if matches!(c, 'j' | 'k') && self.col == Col::Items {
            self.scroll = 0.0;
        }
        true
    }

    /// Choose a filter option; the selection goes back to the top.
    pub fn apply(&mut self, p: Pick) {
        match p {
            Pick::Repo(r) => self.filter.repo = r,
            Pick::Kind(k) => self.filter.kind = k,
            Pick::Author(a) => self.filter.author = a,
            Pick::Saved(s) => self.filter.saved = s,
        }
        self.sel = 0;
        self.scroll = 0.0;
    }

    /// Whether `p` is the one chosen in its group.
    pub fn is_chosen(&self, p: &Pick) -> bool {
        match p {
            Pick::Repo(r) => self.filter.repo == *r,
            Pick::Kind(k) => self.filter.kind == *k,
            Pick::Author(a) => self.filter.author == *a,
            Pick::Saved(s) => self.filter.saved == *s,
        }
    }

    /// Keep the selection inside the list after the items changed.
    pub fn clamp(&mut self, items: &[Item], me: &str) {
        let n = self.shown(items, me).len();
        self.sel = self.sel.min(n.saturating_sub(1));
    }
}

/// Seconds since the Unix epoch of an ISO 8601 UTC time like
/// `2026-10-12T09:02:36Z`; `None` if it does not read.
pub fn epoch(iso: &str) -> Option<i64> {
    let b = iso.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' {
        return None;
    }
    let n = |r: std::ops::Range<usize>| iso.get(r)?.parse::<i64>().ok();
    let (y, m, d) = (n(0..4)?, n(5..7)?, n(8..10)?);
    let (h, mi, s) = (n(11..13)?, n(14..16)?, n(17..19)?);
    // Days from civil (H. Hinnant).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3600 + mi * 60 + s)
}

/// "3h" / "12m" / "2d": how long before `now` (Unix seconds) the time was.
pub fn age(iso: &str, now: i64) -> String {
    let Some(t) = epoch(iso) else { return "—".into() };
    let s = (now - t).max(0);
    match s {
        0..=59 => format!("{s}s"),
        60..=3599 => format!("{}m", s / 60),
        3600..=86_399 => format!("{}h", s / 3600),
        _ => format!("{}d", s / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(repo: &str, kind: Kind, author: &str) -> Item {
        Item {
            id: format!("{repo}{author}"),
            kind,
            repo: repo.into(),
            number: 1,
            title: "t".into(),
            url: String::new(),
            author: author.into(),
            labels: vec![],
            assignees: vec![],
            comments: 0,
            created_at: String::new(),
            updated_at: String::new(),
            draft: false,
            merged: false,
            ci: None,
        }
    }

    fn items() -> Vec<Item> {
        let mut red = item("uchmk/kura", Kind::Pr, "uchmk");
        red.ci = Some(Ci::Bad);
        let mut mine = item("uchmk/tsumugi", Kind::Issue, "bot");
        mine.assignees = vec!["me".into()];
        vec![item("uchmk/tsumugi", Kind::Pr, "uchmk"), red, mine]
    }

    #[test]
    fn the_filter_matches_and_writes_its_query() {
        let it = items();
        let mut f = Filter::default();
        assert_eq!(f.query("uchmk"), "is:open user:uchmk");
        assert!(it.iter().all(|i| f.matches(i, "me")));
        f.saved = Saved::Red;
        assert_eq!(it.iter().filter(|i| f.matches(i, "me")).count(), 1);
        assert_eq!(f.query("uchmk"), "is:open user:uchmk is:pr status:failure");
        let f = Filter { repo: Some("uchmk/kura".into()), kind: Some(Kind::Pr), author: Some("uchmk".into()), saved: Saved::Assigned };
        assert_eq!(f.query("uchmk"), "is:open repo:uchmk/kura is:pr author:uchmk assignee:@me");
        assert!(!f.matches(&it[1], "me"));
    }

    #[test]
    fn j_k_h_l_move_between_and_within_columns() {
        let it = items();
        let mut s = ListState::default();
        assert!(s.key('j', &it, "me"));
        assert_eq!(s.sel, 1);
        s.key('j', &it, "me");
        s.key('j', &it, "me");
        assert_eq!(s.sel, 2, "stops at the last row");
        assert!(s.key('h', &it, "me"));
        assert_eq!(s.col, Col::Filters);
        s.key('j', &it, "me");
        s.key('j', &it, "me");
        assert_eq!(s.cursor, 2);
        assert!(s.key('l', &it, "me"));
        assert_eq!(s.col, Col::Items);
        assert!(s.key('l', &it, "me"));
        assert_eq!(s.col, Col::Detail);
        s.key('j', &it, "me");
        assert_eq!(s.scroll, 1.0);
        s.key('h', &it, "me");
        s.key('k', &it, "me");
        assert_eq!(s.sel, 1);
        assert_eq!(s.scroll, 0.0);
        assert!(!s.key('x', &it, "me"));
    }

    #[test]
    fn choosing_in_the_filter_column_narrows_the_list() {
        let it = items();
        let mut s = ListState { col: Col::Filters, ..Default::default() };
        // options: repo (all, kura, tsumugi), kind (4), author, saved
        s.key('j', &it, "me");
        s.key(' ', &it, "me");
        assert_eq!(s.filter.repo.as_deref(), Some("uchmk/kura"));
        assert_eq!(s.shown(&it, "me").len(), 1);
        assert!(s.is_chosen(&Pick::Repo(Some("uchmk/kura".into()))));
        assert!(!s.is_chosen(&Pick::Repo(None)));
        s.sel = 5;
        s.clamp(&it, "me");
        assert_eq!(s.sel, 0);
    }

    #[test]
    fn times_are_read_and_aged() {
        assert_eq!(epoch("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(epoch("2026-10-12T09:02:36Z"), Some(1_791_795_756));
        assert_eq!(epoch("garbage"), None);
        let now = epoch("2026-10-12T12:00:00Z").unwrap();
        assert_eq!(age("2026-10-12T09:00:00Z", now), "3h");
        assert_eq!(age("2026-10-12T11:48:00Z", now), "12m");
        assert_eq!(age("2026-10-10T12:00:00Z", now), "2d");
        assert_eq!(age("", now), "—");
    }

    #[test]
    fn repo_name_drops_the_owner() {
        assert_eq!(item("uchmk/kura", Kind::Pr, "a").repo_name(), "kura");
    }
}
