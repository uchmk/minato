//! What minato answers through its `ito-ipc` door (minato#67): the open PRs
//! of a repository with their branch, review and CI state, read from the
//! items already in memory. No egui and no sockets, so it is tested apart.
//! michi reads the branch's PR state from here instead of asking GitHub.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::list::{Ci, Item, Kind};

/// Bumped when [`Rq`] or the reply's JSON change shape.
pub const PROTO: u32 = 1;

/// The environment variable that names another door address.
pub const ENV: &str = "MINATO_ADDRESS";

/// A request to the window. Postcard numbers the variants, so new ones go last.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rq {
    /// The open PRs of `repo` (`owner/name`).
    PrState { repo: String },
}

/// The JSON for `rq`, or why there is none (the repository is not one minato
/// has searched, so the caller shows nothing).
///
/// ```json
/// {"repo":"o/n","prs":[{"branch":"x","number":3,"url":"…","title":"…","draft":false,
///   "conflict":false,"review":"requested","reviewers":["me"],"ci":"failure"}]}
/// ```
///
/// Added at the end since v0.70.1: `state` (`open`; merged and closed PRs are
/// not fetched) and `decision` (`approved`, `changes_requested`, `none`: GitHub's
/// `reviewDecision`). `review` is `requested` when `viewer` is among the people asked, `waiting`
/// when others are asked, and `none` when nobody is. `ci` is `pending`,
/// `success`, `failure`, or `none` (no checks).
pub fn answer(items: &[Item], viewer: &str, rq: &Rq) -> Result<String, String> {
    let Rq::PrState { repo } = rq;
    let here = |i: &&Item| i.repo.eq_ignore_ascii_case(repo);
    if !items.iter().any(|i| here(&i)) {
        return Err(format!("minato has no items of {repo}"));
    }
    let prs: Vec<_> = items
        .iter()
        .filter(here)
        .filter(|i| i.kind == Kind::Pr && !i.merged && !i.closed)
        .map(|i| {
            let review = if i.reviewers.iter().any(|r| r.eq_ignore_ascii_case(viewer)) {
                "requested"
            } else if i.reviewers.is_empty() {
                "none"
            } else {
                "waiting"
            };
            let ci = match i.ci {
                Some(Ci::Ok) => "success",
                Some(Ci::Bad) => "failure",
                Some(Ci::Pending) => "pending",
                None => "none",
            };
            json!({
                "branch": i.head, "number": i.number, "url": i.url, "title": i.title, "draft": i.draft,
                "conflict": i.conflict, "review": review, "reviewers": i.reviewers, "ci": ci,
                "state": "open", "decision": i.decision.name(),
            })
        })
        .collect();
    Ok(json!({ "repo": repo, "prs": prs }).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(repo: &str, number: u64, kind: Kind) -> Item {
        Item {
            id: String::new(),
            kind,
            repo: repo.into(),
            number,
            title: format!("t{number}"),
            url: format!("https://github.com/{repo}/pull/{number}"),
            author: "a".into(),
            labels: Vec::new(),
            assignees: Vec::new(),
            milestone: String::new(),
            reviewers: Vec::new(),
            comments: 0,
            last_by: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
            draft: false,
            merged: false,
            closed: false,
            conflict: false,
            mentioned: false,
            ci: None,
            peek: String::new(),
            head: format!("b{number}"),
            decision: Default::default(),
        }
    }

    fn ask(items: &[Item]) -> Result<serde_json::Value, String> {
        answer(items, "me", &Rq::PrState { repo: "o/n".into() }).map(|t| serde_json::from_str(&t).unwrap())
    }

    #[test]
    fn open_prs_of_the_repo_with_review_and_ci() {
        let mut red = item("o/n", 1, Kind::Pr);
        red.ci = Some(Ci::Bad);
        red.reviewers = vec!["Me".into()];
        let mut wait = item("o/n", 2, Kind::Pr);
        wait.reviewers = vec!["x".into()];
        wait.ci = Some(Ci::Pending);
        wait.draft = true;
        let mut merged = item("o/n", 3, Kind::Pr);
        merged.merged = true;
        let items = [red, wait, merged, item("o/n", 4, Kind::Issue), item("o/other", 5, Kind::Pr)];
        let v = ask(&items).unwrap();
        let prs = v["prs"].as_array().unwrap();
        assert_eq!(prs.len(), 2);
        assert_eq!((prs[0]["state"].as_str(), prs[0]["decision"].as_str()), (Some("open"), Some("none")));
        assert_eq!(
            (prs[0]["branch"].as_str(), prs[0]["review"].as_str(), prs[0]["ci"].as_str()),
            (Some("b1"), Some("requested"), Some("failure"))
        );
        assert_eq!(
            (prs[1]["review"].as_str(), prs[1]["ci"].as_str(), prs[1]["draft"].as_bool()),
            (Some("waiting"), Some("pending"), Some(true))
        );
    }

    #[test]
    fn an_unknown_repo_has_no_answer_and_a_known_one_without_prs_is_empty() {
        assert!(ask(&[item("o/other", 1, Kind::Pr)]).is_err());
        let v = ask(&[item("o/n", 1, Kind::Issue)]).unwrap();
        assert!(v["prs"].as_array().unwrap().is_empty());
    }
}
