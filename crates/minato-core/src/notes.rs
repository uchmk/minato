//! GitHub notifications without drawing: the unread threads of
//! `/notifications`, the cursor, and the two writes (one thread read, all
//! read).

use serde_json::{Value, json};

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
        _ => format!("https://github.com/{owner}/{name}"),
    }
}

pub fn parse(body: &str) -> Result<Vec<Note>, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    if let Some(m) = v.get("message").and_then(Value::as_str) {
        return Err(m.to_string());
    }
    let a = v.as_array().ok_or("notifications: not a list")?;
    Ok(a
        .iter()
        .filter(|n| n.get("unread").and_then(Value::as_bool).unwrap_or(true))
        .map(|n| {
            let repo = text(n, "/repository/full_name");
            Note {
                id: text(n, "/id"),
                title: text(n, "/subject/title"),
                kind: text(n, "/subject/type"),
                reason: text(n, "/reason"),
                updated_at: text(n, "/updated_at"),
                url: web_url(&text(n, "/subject/url"), &repo),
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
       "subject":{"title":"CI","type":"CheckSuite","url":null},
       "repository":{"full_name":"uchmk/tsumugi"}}]"#;

    #[test]
    fn only_unread_threads_are_kept_with_their_web_pages() {
        let n = parse(BODY).unwrap();
        assert_eq!(n.len(), 2);
        assert_eq!((n[0].id.as_str(), n[0].kind.as_str(), n[0].reason.as_str()), ("1", "PullRequest", "review_requested"));
        assert_eq!(n[0].url, "https://github.com/uchmk/kura/pull/5");
        assert_eq!(n[1].url, "https://github.com/uchmk/tsumugi");
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
