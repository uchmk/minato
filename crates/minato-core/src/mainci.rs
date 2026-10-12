//! Which of my repositories have a red CI on the default branch. The inbox
//! shows each as a stub item (`id` starts with `main:`, `number` is 0), so the
//! rule machinery stays the same; writes never go to a stub.

use serde_json::Value;

use crate::list::{Ci, Item, Kind};

pub fn query() -> String {
    "query { viewer { repositories(first: 30, ownerAffiliations: OWNER, isFork: false, orderBy: {field: PUSHED_AT, direction: DESC}) { nodes { nameWithOwner url isArchived defaultBranchRef { name target { ... on Commit { statusCheckRollup { state } } } } } } } }".into()
}

/// Whether `item` stands for a repository, not for something on GitHub.
pub fn is_stub(item: &Item) -> bool {
    item.id.starts_with("main:")
}

/// A stub for each repository whose default branch is red.
pub fn parse(body: &str) -> Result<Vec<Item>, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let nodes = v.pointer("/data/viewer/repositories/nodes").and_then(Value::as_array).ok_or_else(|| v.pointer("/errors/0/message").and_then(Value::as_str).unwrap_or("no repositories in the answer").to_string())?;
    let text = |n: &Value, p: &str| n.pointer(p).and_then(Value::as_str).unwrap_or_default().to_string();
    Ok(nodes
        .iter()
        .filter(|n| n.pointer("/isArchived").and_then(Value::as_bool) != Some(true))
        .filter(|n| matches!(text(n, "/defaultBranchRef/target/statusCheckRollup/state").as_str(), "FAILURE" | "ERROR"))
        .map(|n| {
            let repo = text(n, "/nameWithOwner");
            Item {
                id: format!("main:{repo}"),
                kind: Kind::Issue,
                number: 0,
                title: format!("CI is red on {}", text(n, "/defaultBranchRef/name")),
                url: format!("{}/actions", text(n, "/url")),
                author: String::new(),
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
                ci: Some(Ci::Bad),
                repo,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_red_default_branches_become_stubs() {
        let body = r#"{"data":{"viewer":{"repositories":{"nodes":[
{"nameWithOwner":"u/a","url":"https://github.com/u/a","isArchived":false,"defaultBranchRef":{"name":"main","target":{"statusCheckRollup":{"state":"FAILURE"}}}},
{"nameWithOwner":"u/b","url":"https://github.com/u/b","isArchived":false,"defaultBranchRef":{"name":"main","target":{"statusCheckRollup":{"state":"SUCCESS"}}}},
{"nameWithOwner":"u/c","url":"https://github.com/u/c","isArchived":true,"defaultBranchRef":{"name":"main","target":{"statusCheckRollup":{"state":"FAILURE"}}}},
{"nameWithOwner":"u/d","url":"https://github.com/u/d","isArchived":false,"defaultBranchRef":null}]}}}}"#;
        let v = parse(body).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, "main:u/a");
        assert_eq!(v[0].url, "https://github.com/u/a/actions");
        assert!(is_stub(&v[0]));
        assert!(parse(r#"{"errors":[{"message":"nope"}]}"#).is_err());
    }
}
