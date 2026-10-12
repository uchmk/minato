//! The board without drawing: Project "uchmk" read from GraphQL, its Status
//! options as columns, the cards in them, the cursor, and the move of a
//! card to the next column (which becomes a Project field write).

use serde_json::Value;

use crate::github::worker::Call;
use crate::github::write::project_field;
use crate::list::Label;

/// The Project the board shows, by title (the first one when none matches).
pub const TITLE: &str = "uchmk";

/// The Project, its Status field and the cards. No variables.
pub fn query() -> String {
    let content = "__typename ... on Issue { id title number url repository { nameWithOwner } labels(first: 5) { nodes { name color } } } \
... on PullRequest { id title number url repository { nameWithOwner } labels(first: 5) { nodes { name color } } }";
    format!(
        "query {{ viewer {{ projectsV2(first: 20) {{ nodes {{ id title \
field(name: \"Status\") {{ ... on ProjectV2SingleSelectField {{ id options {{ id name }} }} }} priority: field(name: \"Priority\") {{ ... on ProjectV2SingleSelectField {{ id options {{ id name }} }} }} \
items(first: 100) {{ nodes {{ id content {{ {content} }} \
fieldValueByName(name: \"Status\") {{ ... on ProjectV2ItemFieldSingleSelectValue {{ optionId }} }} priority: fieldValueByName(name: \"Priority\") {{ ... on ProjectV2ItemFieldSingleSelectValue {{ optionId }} }} }} }} }} }} }} }}"
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    /// The Project item's id (what the write names).
    pub item: String,
    /// The issue's or PR's node id, as in the list.
    pub id: String,
    pub repo: String,
    pub number: u64,
    pub title: String,
    pub url: String,
    pub pr: bool,
    pub labels: Vec<Label>,
    /// The Status option it stands in, if any.
    pub option: Option<String>,
    /// The Priority option it stands in, if the Project has that field.
    pub priority: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    /// `None` is the column of cards with no Status.
    pub option: Option<String>,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub field: String,
    pub options: Vec<Column>,
    /// The Priority field's id and options (empty when there is none).
    pub priority_field: String,
    pub priority_options: Vec<Column>,
    pub cards: Vec<Card>,
}

fn text(v: &Value, pointer: &str) -> String {
    v.pointer(pointer).and_then(Value::as_str).unwrap_or_default().to_string()
}

pub fn parse(body: &str) -> Result<Project, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    // A Project without a Priority field makes GitHub add an error beside
    // the data; only an answer with no data is a failure.
    if v.pointer("/data/viewer").is_none() {
        if let Some(e) = v.pointer("/errors/0/message").and_then(Value::as_str) {
            return Err(e.to_string());
        }
    }
    let nodes = v.pointer("/data/viewer/projectsV2/nodes").and_then(Value::as_array).ok_or("no projects in the answer")?;
    let p = nodes.iter().find(|p| text(p, "/title") == TITLE).or_else(|| nodes.first()).ok_or("no Project found")?;
    let field = text(p, "/field/id");
    if field.is_empty() {
        return Err("the Project has no Status field".into());
    }
    let options = p
        .pointer("/field/options")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(|o| Column { option: Some(text(o, "/id")), name: text(o, "/name") }).collect())
        .unwrap_or_default();
    let priority_options = p
        .pointer("/priority/options")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(|o| Column { option: Some(text(o, "/id")), name: text(o, "/name") }).collect())
        .unwrap_or_default();
    let cards = p
        .pointer("/items/nodes")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter(|i| matches!(text(i, "/content/__typename").as_str(), "Issue" | "PullRequest"))
                .map(|i| Card {
                    item: text(i, "/id"),
                    id: text(i, "/content/id"),
                    repo: text(i, "/content/repository/nameWithOwner"),
                    number: i.pointer("/content/number").and_then(Value::as_u64).unwrap_or_default(),
                    title: text(i, "/content/title"),
                    url: text(i, "/content/url"),
                    pr: text(i, "/content/__typename") == "PullRequest",
                    labels: i
                        .pointer("/content/labels/nodes")
                        .and_then(Value::as_array)
                        .map(|l| l.iter().map(|l| Label { name: text(l, "/name"), color: text(l, "/color") }).collect())
                        .unwrap_or_default(),
                    option: Some(text(i, "/fieldValueByName/optionId")).filter(|s| !s.is_empty()),
                    priority: Some(text(i, "/priority/optionId")).filter(|s| !s.is_empty()),
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Project { id: text(p, "/id"), title: text(p, "/title"), field, options, priority_field: text(p, "/priority/id"), priority_options, cards })
}

impl Project {
    /// The columns: the cards with no Status first (only if there are
    /// some), then the Status options in GitHub's order.
    pub fn columns(&self) -> Vec<Column> {
        let known = |c: &Card| c.option.as_ref().is_some_and(|o| self.options.iter().any(|x| x.option.as_ref() == Some(o)));
        let mut cols = Vec::new();
        if self.cards.iter().any(|c| !known(c)) {
            cols.push(Column { option: None, name: String::new() });
        }
        cols.extend(self.options.iter().cloned());
        cols
    }

    /// The indices of the cards of column `col`.
    pub fn in_column(&self, col: usize) -> Vec<usize> {
        let cols = self.columns();
        let Some(c) = cols.get(col) else { return Vec::new() };
        self.cards
            .iter()
            .enumerate()
            .filter(|(_, k)| match &c.option {
                Some(o) => k.option.as_ref() == Some(o),
                None => !self.options.iter().any(|x| x.option == k.option),
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// The column a card stands in.
    pub fn column_of(&self, card: usize) -> Option<usize> {
        (0..self.columns().len()).find(|&c| self.in_column(c).contains(&card))
    }

    /// The card of an issue or PR (by node id).
    pub fn card_of(&self, id: &str) -> Option<usize> {
        self.cards.iter().position(|c| c.id == id)
    }

    /// The names of a field's options, and the one a card stands in.
    pub fn choices(&self, card: usize, priority: bool) -> (Vec<String>, Vec<String>) {
        let (opts, cur) = if priority { (&self.priority_options, self.cards.get(card).and_then(|c| c.priority.as_ref())) } else { (&self.options, self.cards.get(card).and_then(|c| c.option.as_ref())) };
        let names = opts.iter().map(|o| o.name.clone()).collect();
        let chosen = opts.iter().filter(|o| o.option.as_ref() == cur).map(|o| o.name.clone()).collect();
        (names, chosen)
    }

    /// Set a card's Status or Priority by option name; the write for GitHub.
    pub fn set_by_name(&mut self, card: usize, priority: bool, name: &str) -> Option<Call> {
        let opts = if priority { &self.priority_options } else { &self.options };
        let option = opts.iter().find(|o| o.name == name)?.option.clone()?;
        if !priority {
            return self.set_option(card, &option);
        }
        let k = self.cards.get_mut(card)?;
        k.priority = Some(option.clone());
        Some(project_field(&self.id, &k.item, &self.priority_field, &option))
    }

    /// Set a card's option and return the write that tells GitHub.
    pub fn set_option(&mut self, card: usize, option: &str) -> Option<Call> {
        let k = self.cards.get_mut(card)?;
        k.option = Some(option.to_string());
        Some(project_field(&self.id, &k.item, &self.field, option))
    }
}

/// Which column and row the cursor is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BoardState {
    pub col: usize,
    pub row: usize,
}

impl BoardState {
    /// `h l j k`: returns whether the key was used.
    pub fn key(&mut self, c: char, p: &Project) -> bool {
        let cols = p.columns().len();
        match c {
            'h' => self.col = self.col.saturating_sub(1),
            'l' => self.col = (self.col + 1).min(cols.saturating_sub(1)),
            'j' => self.row += 1,
            'k' => self.row = self.row.saturating_sub(1),
            _ => return false,
        }
        self.clamp(p);
        true
    }

    pub fn clamp(&mut self, p: &Project) {
        self.col = self.col.min(p.columns().len().saturating_sub(1));
        self.row = self.row.min(p.in_column(self.col).len().saturating_sub(1));
    }

    /// The card under the cursor (an index into `p.cards`).
    pub fn selected(&self, p: &Project) -> Option<usize> {
        p.in_column(self.col).get(self.row).copied()
    }

    /// Put the cursor on `card`.
    pub fn follow(&mut self, p: &Project, card: usize) {
        if let Some(col) = p.column_of(card) {
            self.col = col;
            self.row = p.in_column(col).iter().position(|&i| i == card).unwrap_or(0);
        }
    }
}

/// Move the selected card by `dir` columns (`H` is -1, `L` is +1). Returns
/// the write and the option it came from (for undo), or `None` when it can
/// not go (the edge, or into the no-Status column).
pub fn shift(p: &mut Project, s: &mut BoardState, dir: i32) -> Option<(String, Call, Option<String>)> {
    let card = s.selected(p)?;
    let from = p.column_of(card)?;
    let to = from.checked_add_signed(dir as isize)?;
    let option = p.columns().get(to)?.option.clone()?;
    let before = p.cards[card].option.clone();
    let item = p.cards[card].item.clone();
    let call = p.set_option(card, &option)?;
    s.follow(p, card);
    Some((item, call, before))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::worker::Call;

    const BODY: &str = r#"{"data":{"viewer":{"projectsV2":{"nodes":[
      {"id":"PX","title":"other","field":{"id":"F0","options":[]},"items":{"nodes":[]}},
      {"id":"P1","title":"uchmk","field":{"id":"F1","options":[{"id":"o1","name":"Todo"},{"id":"o2","name":"In progress"},{"id":"o3","name":"Done"}]},
       "items":{"nodes":[
        {"id":"I1","content":{"__typename":"Issue","id":"N1","title":"A","number":1,"url":"u1","repository":{"nameWithOwner":"uchmk/kura"},"labels":{"nodes":[{"name":"bug","color":"f00"}]}},"fieldValueByName":{"optionId":"o1"}},
        {"id":"I2","content":{"__typename":"PullRequest","id":"N2","title":"B","number":2,"url":"u2","repository":{"nameWithOwner":"uchmk/kura"},"labels":{"nodes":[]}},"fieldValueByName":{"optionId":"o1"}},
        {"id":"I3","content":{"__typename":"Issue","id":"N3","title":"C","number":3,"url":"u3","repository":{"nameWithOwner":"uchmk/tsumugi"},"labels":{"nodes":[]}},"fieldValueByName":{}},
        {"id":"I4","content":{"__typename":"DraftIssue"},"fieldValueByName":{"optionId":"o2"}}]}}]}}}}"#;

    fn project() -> Project {
        parse(BODY).unwrap()
    }

    #[test]
    fn the_project_is_found_by_title() {
        let p = project();
        assert_eq!((p.id.as_str(), p.field.as_str(), p.options.len(), p.cards.len()), ("P1", "F1", 3, 3));
        assert!(p.cards[1].pr && p.cards[0].labels[0].name == "bug");
        assert_eq!(p.cards[2].option, None);
    }

    #[test]
    fn cards_with_no_status_get_their_own_first_column() {
        let p = project();
        let cols = p.columns();
        assert_eq!(cols.len(), 4);
        assert_eq!(cols[0].option, None);
        assert_eq!(p.in_column(0), [2]);
        assert_eq!(p.in_column(1), [0, 1]);
        assert_eq!(p.column_of(1), Some(1));
    }

    #[test]
    fn errors_and_missing_fields_are_reported() {
        assert_eq!(parse(r#"{"errors":[{"message":"Your token has not been granted the required scopes"}]}"#), Err("Your token has not been granted the required scopes".into()));
        assert!(parse(r#"{"data":{"viewer":{"projectsV2":{"nodes":[]}}}}"#).is_err());
    }

    #[test]
    fn priority_is_read_and_set_by_name() {
        let body = BODY.replace(r#""field":{"id":"F1","#, r#""priority":{"id":"PF","options":[{"id":"p1","name":"High"},{"id":"p2","name":"Low"}]},"field":{"id":"F1","#).replace(r#""fieldValueByName":{"optionId":"o1"}},
        {"id":"I2""#, r#""fieldValueByName":{"optionId":"o1"},"priority":{"optionId":"p2"}},
        {"id":"I2""#);
        let mut p = parse(&body).unwrap();
        assert_eq!(p.card_of("N1"), Some(0));
        assert_eq!(p.choices(0, true), (vec!["High".to_string(), "Low".to_string()], vec!["Low".to_string()]));
        let Some(Call::Graphql { variables, .. }) = p.set_by_name(0, true, "High") else { panic!() };
        assert!(variables.contains("PF") && variables.contains("p1"));
        assert_eq!(p.cards[0].priority.as_deref(), Some("p1"));
        assert!(p.set_by_name(0, true, "Nope").is_none());
        assert!(p.set_by_name(0, false, "Done").is_some());
        assert_eq!(p.cards[0].option.as_deref(), Some("o3"));
    }

    #[test]
    fn the_cursor_moves_and_stays_inside() {
        let p = project();
        let mut s = BoardState { col: 1, row: 0 };
        assert!(s.key('j', &p));
        assert!(s.key('j', &p));
        assert_eq!(s.row, 1);
        assert!(s.key('l', &p));
        assert_eq!(s, BoardState { col: 2, row: 0 });
        assert!(!s.key('x', &p));
        assert_eq!(s.selected(&p), None);
    }

    #[test]
    fn a_card_moves_to_the_next_column_and_the_cursor_follows() {
        let mut p = project();
        let mut s = BoardState { col: 1, row: 1 };
        let (item, call, before) = shift(&mut p, &mut s, 1).unwrap();
        assert_eq!((item.as_str(), before.as_deref()), ("I2", Some("o1")));
        let Call::Graphql { variables, .. } = call else { panic!() };
        assert!(variables.contains("\"o\":\"o2\"") && variables.contains("\"i\":\"I2\""));
        assert_eq!(s, BoardState { col: 2, row: 0 });
        // Back, and past the edges.
        assert!(shift(&mut p, &mut s, -1).is_some());
        let mut s = BoardState { col: 4, row: 0 };
        assert!(shift(&mut p, &mut s, 1).is_none());
        // Not into the no-Status column.
        let mut s = BoardState { col: 1, row: 0 };
        assert!(shift(&mut p, &mut s, -1).is_none());
    }
}
