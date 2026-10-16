//! The status view's data (docs/design.md, 「5 状況」): the wintest lanes
//! as their status issues tell, each repository's last release and the
//! commits since it, and the open items that wait on the owner. No egui,
//! so it is tested without a window.

use serde_json::Value;

use crate::list::{epoch, Item};

/// The label of a lane's status issue, and the one `Merge lanes` adds when
/// it has not been written for three hours.
pub const LANE_LABEL: &str = "lane-status";
pub const STALLED_LABEL: &str = "lane-stalled";

/// The labels of the items that wait on the owner, in the order shown.
pub const ATTENTION: [&str; 3] = ["needs-pc", "question", "lane-review"];

/// A lane whose status issue is older than this is shown as late.
pub const LATE: i64 = 3 * 3600;

/// How a lane is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Health {
    Ok,
    /// Its status issue has not been written for a while.
    Late,
    /// Stalled, or its last runs failed.
    Bad,
}

/// One wintest lane, as its status issue tells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lane {
    /// The repository and the lane's name ("win", "arm").
    pub repo: String,
    pub name: String,
    /// When the lane's script last ran (Unix seconds), if the issue says.
    pub fired: Option<i64>,
    /// What that run did ("This firing:").
    pub outcome: String,
    /// The runs that failed in a row.
    pub failed: u32,
    pub stalled: bool,
    /// The script's version and the worktree's state, as the issue tells.
    pub script: String,
    pub worktree: String,
    /// The status issue's page.
    pub url: String,
}

impl Lane {
    pub fn health(&self, now: i64) -> Health {
        if self.stalled || self.failed > 0 {
            Health::Bad
        } else if self.fired.is_none_or(|t| now - t > LATE) {
            Health::Late
        } else {
            Health::Ok
        }
    }
}

/// The lanes among `items`, by repository and name.
pub fn lanes(items: &[Item]) -> Vec<Lane> {
    let mut v: Vec<Lane> = items.iter().filter(|i| has(i, LANE_LABEL)).filter_map(lane).collect();
    v.sort_by(|a, b| (&a.repo, &a.name).cmp(&(&b.repo, &b.name)));
    v.dedup_by(|a, b| a.repo == b.repo && a.name == b.name);
    v
}

fn has(i: &Item, label: &str) -> bool {
    i.labels.iter().any(|l| l.name == label)
}

/// A lane from its status issue: "Lane status: win", and a body (as plain
/// text) like "Lane win · Last firing 2026-10-14 14:23 +09:00 · …",
/// "This firing: Done …", "Worktree: clean · Failed runs in a row: 0".
fn lane(i: &Item) -> Option<Lane> {
    let name = i.title.strip_prefix("Lane status:")?.trim().to_string();
    let text = i.peek.replace(['*', '`'], "");
    let after = |key: &str| text.lines().find_map(|l| l.split(" · ").find_map(|p| p.trim().strip_prefix(key)).map(str::trim));
    Some(Lane {
        repo: i.repo.clone(),
        name,
        fired: after("Last firing").and_then(local_epoch),
        outcome: after("This firing:").unwrap_or_default().to_string(),
        failed: after("Failed runs in a row:").and_then(|n| n.parse().ok()).unwrap_or(0),
        stalled: has(i, STALLED_LABEL),
        script: after("Script").unwrap_or_default().to_string(),
        worktree: after("Worktree:").unwrap_or_default().to_string(),
        url: i.url.clone(),
    })
}

/// Unix seconds of a local time with its offset: "2026-10-14 14:23 +09:00".
pub fn local_epoch(text: &str) -> Option<i64> {
    let mut w = text.split_whitespace();
    let (day, time, off) = (w.next()?, w.next()?, w.next()?);
    let utc = epoch(&format!("{day}T{time}:00Z"))?;
    let sign = match off.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let (h, m) = off[1..].split_once(':')?;
    Some(utc - sign * (h.parse::<i64>().ok()? * 3600 + m.parse::<i64>().ok()? * 60))
}

/// The open items that wait on the owner: those with one of the
/// `ATTENTION` labels (in that order, newest first in each), not the
/// lanes' own status issues.
pub fn attention(items: &[Item]) -> Vec<(usize, &'static str)> {
    let mut out = Vec::new();
    for label in ATTENTION {
        let mut v: Vec<usize> =
            (0..items.len()).filter(|&k| !items[k].closed && has(&items[k], label) && !has(&items[k], LANE_LABEL)).collect();
        v.sort_by(|a, b| items[*b].updated_at.cmp(&items[*a].updated_at));
        out.extend(v.into_iter().filter(|k| !out.iter().any(|(x, _)| x == k)).map(|k| (k, label)).collect::<Vec<_>>());
    }
    out
}

/// A repository's last release.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shipped {
    pub tag: String,
    /// The release's title, who published it, and whether it is a pre-release.
    pub name: String,
    pub author: String,
    pub prerelease: bool,
    /// Its notes (the first part) and its files with their sizes.
    pub notes: String,
    pub assets: Vec<(String, u64)>,
    pub published_at: String,
    /// How many files it has, and whether its notes carry the SHA-256 table.
    pub files: usize,
    pub sums: bool,
    pub url: String,
}

impl Shipped {
    /// Files without the SHA-256 table in the notes. A release with no
    /// files (a library's) has nothing to sum.
    pub fn lacks_sums(&self) -> bool {
        self.files > 0 && !self.sums
    }
}

/// The releases of a repository, newest first (a few are enough to pass
/// over drafts).
pub fn releases_path(repo: &str) -> String {
    format!("/repos/{repo}/releases?per_page=5")
}

/// The newest published release of the answer; `None` when there is none.
pub fn parse_latest(body: &str) -> Result<Option<Shipped>, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let Some(a) = v.as_array() else {
        return Err(v.get("message").and_then(Value::as_str).map_or_else(|| "not a list".into(), str::to_string));
    };
    let text = |r: &Value, k: &str| r.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    Ok(a.iter().find(|r| !r.get("draft").and_then(Value::as_bool).unwrap_or(false)).map(|r| Shipped {
        tag: text(r, "tag_name"),
        name: text(r, "name"),
        author: r.pointer("/author/login").and_then(Value::as_str).unwrap_or_default().to_string(),
        prerelease: r.get("prerelease").and_then(Value::as_bool).unwrap_or(false),
        notes: text(r, "body").replace('\r', "").chars().take(2400).collect(),
        assets: r
            .get("assets")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|f| (text(f, "name"), f.get("size").and_then(Value::as_u64).unwrap_or(0))).collect())
            .unwrap_or_default(),
        published_at: text(r, "published_at"),
        files: r.get("assets").and_then(Value::as_array).map_or(0, Vec::len),
        sums: text(r, "body").contains("SHA-256"),
        url: text(r, "html_url"),
    }))
}

/// The commits on the default branch since `tag`.
pub fn compare_path(repo: &str, tag: &str) -> String {
    format!("/repos/{repo}/compare/{tag}...HEAD")
}

/// How many commits the branch is ahead of the tag.
pub fn parse_ahead(body: &str) -> Result<u32, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    match v.get("ahead_by").and_then(Value::as_u64) {
        Some(n) => Ok(n as u32),
        None => Err(v.get("message").and_then(Value::as_str).unwrap_or("no ahead_by").to_string()),
    }
}

/// A line of the status view, in the order shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    Lane(Lane),
    /// A repository with its last release (none yet: `None`) and the
    /// commits since, once GitHub has said.
    Release {
        repo: String,
        shipped: Option<Shipped>,
        ahead: Option<u32>,
    },
    /// An item waiting on the owner (its place in the items) and the label
    /// it is there for.
    Item(usize, &'static str),
}

impl Row {
    /// The page `o` opens.
    pub fn url(&self, items: &[Item]) -> String {
        match self {
            Row::Lane(l) => l.url.clone(),
            Row::Release { shipped: Some(s), .. } if !s.url.is_empty() => s.url.clone(),
            Row::Release { repo, .. } => format!("https://github.com/{repo}/releases"),
            Row::Item(k, _) => items.get(*k).map(|i| i.url.clone()).unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::list::{Kind, Label};

    fn item(repo: &str, title: &str, labels: &[&str], peek: &str) -> Item {
        Item {
            id: format!("{repo}:{title}"),
            kind: Kind::Issue,
            repo: repo.into(),
            number: 13,
            title: title.into(),
            url: format!("https://github.com/{repo}/issues/13"),
            author: "uchmk".into(),
            labels: labels.iter().map(|l| Label { name: (*l).into(), color: "ededed".into() }).collect(),
            assignees: Vec::new(),
            milestone: String::new(),
            reviewers: Vec::new(),
            comments: 0,
            created_at: "2026-10-01T00:00:00Z".into(),
            updated_at: "2026-10-14T05:00:00Z".into(),
            draft: false,
            merged: false,
            closed: false,
            conflict: false,
            mentioned: false,
            ci: None,
            peek: peek.into(),
            head: String::new(),
        }
    }

    const BODY: &str = "Lane win · Last firing 2026-10-14 14:23 +09:00 · Script v0.107.26 (fc76e83, main's copy)\n\
This firing: Done (test/win-5-10): WINTEST_DONE #119\n\
Worktree: clean · Failed runs in a row: 0\n\
If Last firing is more than two hours old …";

    #[test]
    fn a_lane_is_read_from_its_status_issue() {
        let items = [
            item("uchmk/tsumugi", "Lane status: win", &["lane-status"], BODY),
            item("uchmk/kura", "Lane status: arm", &["lane-status", "lane-stalled"], &BODY.replace("in a row: 0", "in a row: 2")),
            item("uchmk/kura", "Something else", &[], BODY),
        ];
        let v = lanes(&items);
        assert_eq!(v.len(), 2);
        assert_eq!((v[0].repo.as_str(), v[0].name.as_str()), ("uchmk/kura", "arm"));
        let t = &v[1];
        assert_eq!(t.fired, epoch("2026-10-14T05:23:00Z"));
        assert_eq!(t.outcome, "Done (test/win-5-10): WINTEST_DONE #119");
        assert_eq!((t.failed, t.stalled), (0, false));
        assert_eq!(t.worktree, "clean");
        assert!(!t.script.is_empty(), "the script line is kept");
        assert_eq!((v[0].failed, v[0].stalled), (2, true));
        let at = t.fired.unwrap();
        assert_eq!(t.health(at + 600), Health::Ok);
        assert_eq!(t.health(at + LATE + 1), Health::Late);
        assert_eq!(v[0].health(at), Health::Bad);
        // Markdown left in a body is passed over.
        let md = item("o/r", "Lane status: win", &["lane-status"], "**Lane** `win` · **Last firing** 2026-10-14 14:23 +09:00");
        assert_eq!(lanes(&[md])[0].fired, t.fired);
        // A body that says nothing is late, not a failure.
        let bare = item("o/r", "Lane status: win", &["lane-status"], "");
        assert_eq!(lanes(&[bare])[0].health(at), Health::Late);
    }

    #[test]
    fn local_times_read_with_their_offset() {
        assert_eq!(local_epoch("2026-10-14 09:00 +09:00"), epoch("2026-10-14T00:00:00Z"));
        assert_eq!(local_epoch("2026-10-14 09:00 -01:30"), epoch("2026-10-14T10:30:00Z"));
        assert_eq!(local_epoch("2026-10-14"), None);
        assert_eq!(local_epoch("x y z"), None);
    }

    #[test]
    fn items_waiting_on_the_owner_come_by_label() {
        let mut a = item("o/r", "pc", &["needs-pc"], "");
        a.updated_at = "2026-10-14T01:00:00Z".into();
        let mut b = item("o/r", "pc newer", &["needs-pc", "question"], "");
        b.updated_at = "2026-10-14T02:00:00Z".into();
        let c = item("o/r", "q", &["question"], "");
        let mut d = item("o/r", "closed", &["question"], "");
        d.closed = true;
        let e = item("o/r", "Lane status: win", &["lane-status", "lane-review"], "");
        let f = item("o/r", "review", &["lane-review"], "");
        let items = [a, b, c, d, e, f];
        assert_eq!(attention(&items), [(1, "needs-pc"), (0, "needs-pc"), (2, "question"), (5, "lane-review")]);
    }

    #[test]
    fn the_last_release_and_the_commits_since() {
        let body = r#"[{"tag_name":"v2","draft":true},{"tag_name":"v1","draft":false,"published_at":"2026-10-14T02:38:59Z",
            "assets":[{},{},{}],"body":"notes\n\nSHA-256 | file","html_url":"https://github.com/o/r/releases/tag/v1"}]"#;
        let s = parse_latest(body).unwrap().unwrap();
        assert_eq!((s.tag.as_str(), s.files, s.sums), ("v1", 3, true));
        assert!(!s.lacks_sums());
        assert!(Shipped { sums: false, ..s.clone() }.lacks_sums());
        assert!(!Shipped { sums: false, files: 0, ..s.clone() }.lacks_sums(), "no files, nothing to sum");
        // The title, the author, the files with their sizes and the notes.
        let full = r#"[{"tag_name":"v3","name":"Third","author":{"login":"me"},"prerelease":true,"body":"a\r\nb",
            "assets":[{"name":"x.zip","size":10},{"name":"y.zip"}]}]"#;
        let t = parse_latest(full).unwrap().unwrap();
        assert_eq!((t.name.as_str(), t.author.as_str(), t.prerelease, t.notes.as_str()), ("Third", "me", true, "a\nb"));
        assert_eq!(t.assets, [("x.zip".to_string(), 10), ("y.zip".to_string(), 0)]);
        assert_eq!(parse_latest("[]").unwrap(), None);
        assert_eq!(parse_latest(r#"{"message":"Not Found"}"#), Err("Not Found".into()));
        assert_eq!(releases_path("o/r"), "/repos/o/r/releases?per_page=5");
        assert_eq!(compare_path("o/r", "v1"), "/repos/o/r/compare/v1...HEAD");
        assert_eq!(parse_ahead(r#"{"ahead_by":8,"status":"ahead"}"#), Ok(8));
        assert!(parse_ahead(r#"{"message":"Not Found"}"#).is_err());
        let r = Row::Release { repo: "o/r".into(), shipped: Some(s), ahead: Some(8) };
        assert_eq!(r.url(&[]), "https://github.com/o/r/releases/tag/v1");
        let none = Row::Release { repo: "o/r".into(), shipped: None, ahead: None };
        assert_eq!(none.url(&[]), "https://github.com/o/r/releases");
    }
}
