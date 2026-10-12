//! A repository's settings, read only: the lines minato shows in a box.

use serde_json::Value;

pub fn path(repo: &str) -> String {
    format!("/repos/{repo}")
}

/// `name: value` lines from the answer of `GET /repos/{repo}`.
pub fn lines(body: &str) -> Result<Vec<String>, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    if let Some(m) = v.get("message").and_then(Value::as_str) {
        return Err(m.to_string());
    }
    let text = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("-").to_string();
    let flag = |k: &str| if v.get(k).and_then(Value::as_bool) == Some(true) { "on" } else { "off" };
    let topics: Vec<&str> = v.get("topics").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    Ok(vec![
        format!("description: {}", text("description")),
        format!("visibility: {}", text("visibility")),
        format!("default branch: {}", text("default_branch")),
        format!("open issues and PRs: {}", v.get("open_issues_count").and_then(Value::as_u64).unwrap_or(0)),
        format!("topics: {}", if topics.is_empty() { "-".to_string() } else { topics.join(", ") }),
        String::new(),
        format!("merge commit: {}", flag("allow_merge_commit")),
        format!("squash merge: {}", flag("allow_squash_merge")),
        format!("rebase merge: {}", flag("allow_rebase_merge")),
        format!("delete branch after merge: {}", flag("delete_branch_on_merge")),
        String::new(),
        format!("issues: {}", flag("has_issues")),
        format!("discussions: {}", flag("has_discussions")),
        format!("archived: {}", flag("archived")),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_become_lines() {
        let body = r#"{"description":"d","visibility":"private","default_branch":"main","open_issues_count":3,"topics":["a","b"],"allow_merge_commit":true,"allow_squash_merge":false,"delete_branch_on_merge":true,"has_issues":true}"#;
        let l = lines(body).unwrap();
        assert_eq!(l[1], "visibility: private");
        assert_eq!(l[4], "topics: a, b");
        assert!(l.contains(&"merge commit: on".to_string()));
        assert!(l.contains(&"squash merge: off".to_string()));
        assert!(l.contains(&"rebase merge: off".to_string()));
        assert_eq!(lines(r#"{"message":"Not Found"}"#), Err("Not Found".to_string()));
    }
}
