//! GitHub notifications without drawing: the unread threads of
//! `/notifications`, the cursor, and the two writes (one thread read, all
//! read).

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::github::http::Method;
use crate::github::worker::Call;

/// The unread threads, newest first.
pub const PATH: &str = "/notifications?per_page=50";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub id: String,
    pub repo: String,
    pub title: String,
    /// `Issue`, `PullRequest`, `Discussion`, `CheckSuite`, …
    pub kind: String,
    /// Why it came: `mention`, `review_requested`, `assign`, …
    pub reason: String,
    pub updated_at: String,
    /// Where it opens in the browser.
    pub url: String,
}

fn text(v: &Value, pointer: &str) -> String {
    v.pointer(pointer).and_then(Value::as_str).unwrap_or_default().to_string()
}

/// The page of a thread: the API address of its subject turned into the
/// web one (`/repos/o/r/pulls/5` → `/o/r/pull/5`); the repository when
/// the subject has no page of its own.
pub fn web_url(subject_api: &str, repo: &str) -> String {
    let Some((_, rest)) = subject_api.split_once("/repos/") else { return format!("https://github.com/{repo}") };
    let mut parts = rest.split('/');
    let (Some(owner), Some(name), Some(kind), Some(num)) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
        return format!("https://github.com/{repo}");
    };
    match kind {
        "pulls" => format!("https://github.com/{owner}/{name}/pull/{num}"),
        "issues" => format!("https://github.com/{owner}/{name}/issues/{num}"),
        "commits" => format!("https://github.com/{owner}/{name}/commit/{num}"),
        "releases" => format!("https://github.com/{owner}/{name}/releases"),
        _ => format!("https://github.com/{owner}/{name}"),
    }
}

/// The page of a thread of `kind` whose subject has no API address: a
/// workflow run's is the repository's Actions, a discussion's its
/// discussions.
fn page(kind: &str, subject_api: &str, repo: &str) -> String {
    match kind {
        _ if !subject_api.is_empty() => web_url(subject_api, repo),
        "CheckSuite" => format!("https://github.com/{repo}/actions"),
        "Discussion" => format!("https://github.com/{repo}/discussions"),
        _ => web_url(subject_api, repo),
    }
}

/// The workflow and the branch a run's notification is about, from its
/// title (`CI workflow run failed for main branch`).
pub fn workflow_run(note: &Note) -> Option<(&str, &str)> {
    if note.kind != "CheckSuite" {
        return None;
    }
    let (workflow, rest) = note.title.split_once(" workflow run ")?;
    let (_, branch) = rest.rsplit_once(" for ")?;
    Some((workflow, branch.strip_suffix(" branch")?))
}

/// The colour a line is drawn in, by what it is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Bad,
    Ok,
    Ask,
    Run,
    Idle,
    Accent,
}

/// What happened, as an i18n key, and its tone: why GitHub sent the thread.
pub fn event(reason: &str) -> (&'static str, Tone) {
    match reason {
        "ci_activity" => ("event.ci", Tone::Run),
        "review_requested" => ("event.review", Tone::Ask),
        "mention" | "team_mention" => ("event.mention", Tone::Accent),
        "comment" => ("event.comment", Tone::Accent),
        "assign" => ("event.assign", Tone::Accent),
        "state_change" => ("event.state", Tone::Ok),
        "author" => ("event.author", Tone::Accent),
        _ => ("event.other", Tone::Idle),
    }
}

/// The number at the end of a thread's page (`/pull/5`, `/issues/5`,
/// `/discussions/5`), if it has one.
pub fn number(url: &str) -> Option<u64> {
    let (head, n) = url.rsplit_once('/')?;
    let kind = head.rsplit('/').next()?;
    if !matches!(kind, "pull" | "issues" | "discussions") {
        return None;
    }
    n.parse().ok()
}

pub fn parse(body: &str) -> Result<Vec<Note>, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    if let Some(m) = v.get("message").and_then(Value::as_str) {
        return Err(m.to_string());
    }
    let a = v.as_array().ok_or("notifications: not a list")?;
    Ok(a.iter()
        .filter(|n| n.get("unread").and_then(Value::as_bool).unwrap_or(true))
        .map(|n| {
            let repo = text(n, "/repository/full_name");
            Note {
                id: text(n, "/id"),
                title: text(n, "/subject/title"),
                kind: text(n, "/subject/type"),
                reason: text(n, "/reason"),
                updated_at: text(n, "/updated_at"),
                url: page(&text(n, "/subject/type"), &text(n, "/subject/url"), &repo),
                repo,
            }
        })
        .collect())
}

/// Mark one thread read.
pub fn mark_read(id: &str) -> Call {
    Call::Send { method: Method::Patch, path: format!("/notifications/threads/{id}"), body: "{}".into() }
}

/// Mark every notification read.
pub fn mark_all() -> Call {
    Call::Send { method: Method::Put, path: "/notifications".into(), body: json!({}).to_string() }
}

/// The threads marked read here, by when they were last updated: GitHub may
/// still list them for a while (a list asked before the mark, or one it
/// had not made again), and they are not shown again unless something new
/// happens in them.
#[derive(Clone, Debug, Default)]
pub struct Read(HashMap<String, String>);

impl Read {
    pub fn mark(&mut self, note: &Note) {
        self.0.insert(note.id.clone(), note.updated_at.clone());
    }

    /// The threads of `notes` still unread; a thread GitHub no longer
    /// lists is forgotten.
    pub fn unread(&mut self, notes: Vec<Note>) -> Vec<Note> {
        self.0.retain(|id, _| notes.iter().any(|n| n.id == *id));
        notes.into_iter().filter(|n| self.0.get(&n.id) != Some(&n.updated_at)).collect()
    }
}

/// The cursor on the list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NotesState {
    pub sel: usize,
}

impl NotesState {
    /// `j k`: returns whether the key was used.
    pub fn key(&mut self, c: char, len: usize) -> bool {
        match c {
            'j' => self.sel += 1,
            'k' => self.sel = self.sel.saturating_sub(1),
            _ => return false,
        }
        self.clamp(len);
        true
    }

    pub fn clamp(&mut self, len: usize) {
        self.sel = self.sel.min(len.saturating_sub(1));
    }

    /// `gg` and `G`: the first or the last.
    pub fn jump(&mut self, end: bool, len: usize) {
        self.sel = if end { usize::MAX } else { 0 };
        self.clamp(len);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = r#"[
      {"id":"1","unread":true,"reason":"review_requested","updated_at":"2026-10-12T01:00:00Z",
       "subject":{"title":"Fix it","type":"PullRequest","url":"https://api.github.com/repos/uchmk/kura/pulls/5"},
       "repository":{"full_name":"uchmk/kura"}},
      {"id":"2","unread":false,"reason":"mention","updated_at":"2026-10-12T00:00:00Z",
       "subject":{"title":"Old","type":"Issue","url":"https://api.github.com/repos/uchmk/kura/issues/1"},
       "repository":{"full_name":"uchmk/kura"}},
      {"id":"3","unread":true,"reason":"ci_activity","updated_at":"2026-10-11T00:00:00Z",
       "subject":{"title":"CI workflow run failed for test/win-17 branch","type":"CheckSuite","url":null},
       "repository":{"full_name":"uchmk/tsumugi"}},
      {"id":"4","unread":true,"reason":"subscribed","updated_at":"2026-10-10T00:00:00Z",
       "subject":{"title":"Hi","type":"Discussion","url":null},
       "repository":{"full_name":"uchmk/kura"}},
      {"id":"5","unread":true,"reason":"subscribed","updated_at":"2026-10-10T00:00:00Z",
       "subject":{"title":"v1","type":"Release","url":"https://api.github.com/repos/uchmk/kura/releases/77"},
       "repository":{"full_name":"uchmk/kura"}}]"#;

    #[test]
    fn only_unread_threads_are_kept_with_their_web_pages() {
        let n = parse(BODY).unwrap();
        assert_eq!(n.len(), 4);
        assert_eq!((n[0].id.as_str(), n[0].kind.as_str(), n[0].reason.as_str()), ("1", "PullRequest", "review_requested"));
        assert_eq!(n[0].url, "https://github.com/uchmk/kura/pull/5");
        assert_eq!(n[1].url, "https://github.com/uchmk/tsumugi/actions");
        assert_eq!(n[2].url, "https://github.com/uchmk/kura/discussions");
        assert_eq!(n[3].url, "https://github.com/uchmk/kura/releases");
    }

    #[test]
    fn a_runs_notification_names_its_workflow_and_branch() {
        let n = parse(BODY).unwrap();
        assert_eq!(workflow_run(&n[1]), Some(("CI", "test/win-17")));
        assert_eq!(workflow_run(&n[0]), None);
        let odd = Note { title: "Deploy".into(), ..n[1].clone() };
        assert_eq!(workflow_run(&odd), None);
    }

    #[test]
    fn a_thread_marked_read_stays_away_until_it_changes() {
        let n = parse(BODY).unwrap();
        let mut read = Read::default();
        read.mark(&n[0]);
        assert_eq!(read.unread(n.clone()).len(), 3, "a list made before the mark");
        let newer = Note { updated_at: "2026-10-12T02:00:00Z".into(), ..n[0].clone() };
        assert_eq!(read.unread(vec![newer]).len(), 1, "something new happened in it");
        read.mark(&n[0]);
        assert_eq!(read.unread(n[1..].to_vec()).len(), 3);
        assert_eq!(read.unread(n.clone()).len(), 4, "forgotten once GitHub stopped listing it");
    }

    #[test]
    fn an_error_message_is_reported() {
        assert_eq!(parse(r#"{"message":"Bad credentials"}"#), Err("Bad credentials".into()));
    }

    #[test]
    fn the_writes_name_the_thread_or_everything() {
        assert!(matches!(mark_read("9"), Call::Send { method: Method::Patch, path, .. } if path == "/notifications/threads/9"));
        assert!(matches!(mark_all(), Call::Send { method: Method::Put, path, .. } if path == "/notifications"));
    }

    #[test]
    fn events_have_words_and_numbers_come_from_the_page() {
        assert_eq!(event("review_requested"), ("event.review", Tone::Ask));
        assert_eq!(event("ci_activity").1, Tone::Run);
        assert_eq!(event("something_new"), ("event.other", Tone::Idle));
        assert_eq!(number("https://github.com/uchmk/kura/pull/5"), Some(5));
        assert_eq!(number("https://github.com/uchmk/kura/issues/12"), Some(12));
        assert_eq!(number("https://github.com/uchmk/kura/discussions/3"), Some(3));
        assert_eq!(number("https://github.com/uchmk/kura/commit/abc"), None);
        assert_eq!(number("https://github.com/uchmk/tsumugi"), None);
    }

    #[test]
    fn the_cursor_stays_inside() {
        let mut s = NotesState::default();
        assert!(s.key('j', 2));
        assert!(s.key('j', 2));
        assert_eq!(s.sel, 1);
        assert!(s.key('k', 2));
        assert!(!s.key('x', 2));
        s.clamp(0);
        assert_eq!(s.sel, 0);
    }
}
