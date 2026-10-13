//! What minato writes to GitHub, as ready-to-send calls. Nothing here sends;
//! the tests check the paths and the JSON.

use serde_json::{json, Value};

use super::http::Method;
use super::worker::Call;
use crate::list::{Item, Kind};

fn send(method: Method, path: String, body: Value) -> Call {
    Call::Send { method, path, body: body.to_string() }
}

/// A comment on an issue, a PR or a discussion.
pub fn comment(item: &Item, body: &str) -> Call {
    match item.kind {
        Kind::Discussion => Call::Graphql {
            query: "mutation($id:ID!,$body:String!){addDiscussionComment(input:{discussionId:$id,body:$body}){comment{id}}}".into(),
            variables: json!({ "id": item.id, "body": body }).to_string(),
        },
        _ => send(Method::Post, format!("/repos/{}/issues/{}/comments", item.repo, item.number), json!({ "body": body })),
    }
}

/// Mark a discussion comment as the answer.
pub fn mark_answer(comment_id: &str) -> Call {
    Call::Graphql {
        query: "mutation($id:ID!){markDiscussionCommentAsAnswer(input:{id:$id}){discussion{id}}}".into(),
        variables: json!({ "id": comment_id }).to_string(),
    }
}

/// A new issue; the first line of `text` is the title, the rest the body.
pub fn create_issue(repo: &str, text: &str) -> Option<Call> {
    let text = text.trim_start();
    let (title, body) = text.split_once('\n').unwrap_or((text, ""));
    let title = title.trim();
    if title.is_empty() {
        return None;
    }
    Some(send(Method::Post, format!("/repos/{repo}/issues"), json!({ "title": title, "body": body.trim() })))
}

/// Merge a PR with a merge commit.
pub fn merge(item: &Item) -> Call {
    send(Method::Put, format!("/repos/{}/pulls/{}/merge", item.repo, item.number), json!({ "merge_method": "merge" }))
}

/// Add `login` to the assignees (the others stay).
pub fn assign(item: &Item, login: &str) -> Call {
    send(Method::Post, format!("/repos/{}/issues/{}/assignees", item.repo, item.number), json!({ "assignees": [login] }))
}

/// Replace the assignees with exactly `logins`.
pub fn set_assignees(item: &Item, logins: &[String]) -> Call {
    send(Method::Patch, format!("/repos/{}/issues/{}", item.repo, item.number), json!({ "assignees": logins }))
}

/// Replace the labels with exactly `names`.
pub fn set_labels(item: &Item, names: &[String]) -> Call {
    send(Method::Patch, format!("/repos/{}/issues/{}", item.repo, item.number), json!({ "labels": names }))
}

/// Close or reopen an issue or a PR.
pub fn set_state(item: &Item, open: bool) -> Call {
    send(Method::Patch, format!("/repos/{}/issues/{}", item.repo, item.number), json!({ "state": if open { "open" } else { "closed" } }))
}

/// Move a Project item to another option of a single-select field
/// (Status, Priority). The ids come from the Project's own fields.
pub fn project_field(project: &str, item: &str, field: &str, option: &str) -> Call {
    Call::Graphql {
        query: "mutation($p:ID!,$i:ID!,$f:ID!,$o:String!){updateProjectV2ItemFieldValue(input:{projectId:$p,itemId:$i,fieldId:$f,value:{singleSelectOptionId:$o}}){projectV2Item{id}}}".into(),
        variables: json!({ "p": project, "i": item, "f": field, "o": option }).to_string(),
    }
}

/// Set the milestone by its number (`None` clears it).
pub fn set_milestone(item: &Item, number: Option<u64>) -> Call {
    send(Method::Patch, format!("/repos/{}/issues/{}", item.repo, item.number), json!({ "milestone": number }))
}

/// Ask `logins` to review a PR.
pub fn request_reviewers(item: &Item, logins: &[String]) -> Call {
    send(Method::Post, format!("/repos/{}/pulls/{}/requested_reviewers", item.repo, item.number), json!({ "reviewers": logins }))
}

/// Take `logins` off the review requests of a PR.
pub fn remove_reviewers(item: &Item, logins: &[String]) -> Call {
    send(Method::Delete, format!("/repos/{}/pulls/{}/requested_reviewers", item.repo, item.number), json!({ "reviewers": logins }))
}

/// The path of the repository's open milestones.
pub fn milestones_path(repo: &str) -> String {
    format!("/repos/{repo}/milestones?per_page=100")
}

/// The milestones as `(number, title)`, in the order sent.
pub fn parse_milestones(body: &str) -> Result<Vec<(u64, String)>, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let list = v.as_array().ok_or("not a list")?;
    Ok(list.iter().filter_map(|x| Some((x.get("number")?.as_u64()?, x.get("title")?.as_str()?.to_string()))).collect())
}

/// The names in a REST list (`key` is `name` for labels, `login` for
/// assignees), in the order sent.
pub fn parse_names(body: &str, key: &str) -> Result<Vec<String>, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let list = v.as_array().ok_or("not a list")?;
    Ok(list.iter().filter_map(|x| x.get(key).and_then(Value::as_str)).map(str::to_string).collect())
}

/// The path of the repository's labels (up to 100).
pub fn labels_path(repo: &str) -> String {
    format!("/repos/{repo}/labels?per_page=100")
}

/// The path of the people who can be assigned in the repository.
pub fn assignees_path(repo: &str) -> String {
    format!("/repos/{repo}/assignees?per_page=100")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(kind: Kind) -> Item {
        Item {
            id: "D_1".into(),
            kind,
            repo: "uchmk/kura".into(),
            number: 7,
            title: String::new(),
            url: String::new(),
            author: String::new(),
            labels: Vec::new(),
            assignees: Vec::new(),
            milestone: String::new(),
            reviewers: Vec::new(),
            comments: 0,
            created_at: String::new(),
            updated_at: String::new(),
            draft: false,
            merged: false,
            closed: false,
            conflict: false,
            mentioned: false,
            peek: String::new(),
            head: String::new(),
            ci: None,
        }
    }

    #[test]
    fn a_new_issue_takes_its_title_from_the_first_line() {
        let (m, path, body) = parts(
            create_issue(
                "uchmk/kura",
                "
 Title here 
Body
more
",
            )
            .unwrap(),
        );
        assert!(matches!(m, Method::Post));
        assert_eq!(path, "/repos/uchmk/kura/issues");
        assert_eq!(
            body,
            json!({ "title": "Title here", "body": "Body
more" })
        );
        assert!(create_issue(
            "uchmk/kura",
            " 
 "
        )
        .is_none());
    }

    fn parts(c: Call) -> (Method, String, Value) {
        let Call::Send { method, path, body } = c else { panic!("not a REST write") };
        (method, path, serde_json::from_str(&body).unwrap())
    }

    #[test]
    fn milestone_and_reviewers_use_their_endpoints() {
        let (m, p, b) = parts(set_milestone(&item(Kind::Issue), Some(3)));
        assert_eq!((m, p.as_str(), b["milestone"].as_u64()), (Method::Patch, "/repos/uchmk/kura/issues/7", Some(3)));
        assert!(parts(set_milestone(&item(Kind::Issue), None)).2["milestone"].is_null());
        let (m, p, b) = parts(request_reviewers(&item(Kind::Pr), &["a".into()]));
        assert_eq!((m, p.as_str(), b["reviewers"][0].as_str()), (Method::Post, "/repos/uchmk/kura/pulls/7/requested_reviewers", Some("a")));
        assert_eq!(parts(remove_reviewers(&item(Kind::Pr), &["a".into()])).0, Method::Delete);
        assert_eq!(parse_milestones(r#"[{"number":2,"title":"v1"},{"x":1}]"#).unwrap(), [(2, "v1".to_string())]);
    }

    #[test]
    fn a_comment_goes_to_the_issues_endpoint() {
        let (m, p, b) = parts(comment(&item(Kind::Pr), "hi \"there\"\n"));
        assert_eq!((m, p.as_str()), (Method::Post, "/repos/uchmk/kura/issues/7/comments"));
        assert_eq!(b["body"], "hi \"there\"\n");
    }

    #[test]
    fn the_answer_mark_is_a_mutation_on_the_comment() {
        let Call::Graphql { query, variables } = mark_answer("DC_1") else { panic!() };
        assert!(query.contains("markDiscussionCommentAsAnswer"));
        assert_eq!(serde_json::from_str::<Value>(&variables).unwrap()["id"], "DC_1");
    }

    #[test]
    fn a_discussion_comment_is_a_mutation() {
        let Call::Graphql { query, variables } = comment(&item(Kind::Discussion), "x") else { panic!() };
        assert!(query.contains("addDiscussionComment"));
        let v: Value = serde_json::from_str(&variables).unwrap();
        assert_eq!((v["id"].as_str(), v["body"].as_str()), (Some("D_1"), Some("x")));
    }

    #[test]
    fn merge_uses_a_merge_commit() {
        let (m, p, b) = parts(merge(&item(Kind::Pr)));
        assert_eq!((m, p.as_str()), (Method::Put, "/repos/uchmk/kura/pulls/7/merge"));
        assert_eq!(b["merge_method"], "merge");
    }

    #[test]
    fn assignees_labels_and_state() {
        let (m, p, b) = parts(assign(&item(Kind::Issue), "uchmk"));
        assert_eq!((m, p.as_str(), &b["assignees"][0]), (Method::Post, "/repos/uchmk/kura/issues/7/assignees", &json!("uchmk")));
        let (m, p, b) = parts(set_labels(&item(Kind::Issue), &["bug".into(), "lane:win".into()]));
        assert_eq!((m, p.as_str()), (Method::Patch, "/repos/uchmk/kura/issues/7"));
        assert_eq!(b["labels"], json!(["bug", "lane:win"]));
        let (_, _, b) = parts(set_assignees(&item(Kind::Issue), &[]));
        assert_eq!(b["assignees"], json!([]));
        let (_, _, b) = parts(set_state(&item(Kind::Issue), false));
        assert_eq!(b["state"], "closed");
    }

    #[test]
    fn the_project_mutation_carries_the_four_ids() {
        let Call::Graphql { query, variables } = project_field("P", "I", "F", "O") else { panic!() };
        assert!(query.contains("updateProjectV2ItemFieldValue"));
        let v: Value = serde_json::from_str(&variables).unwrap();
        assert_eq!((v["p"].as_str(), v["i"].as_str(), v["f"].as_str(), v["o"].as_str()), (Some("P"), Some("I"), Some("F"), Some("O")));
    }

    #[test]
    fn names_are_read_from_a_list() {
        assert_eq!(parse_names(r#"[{"name":"bug","color":"f00"},{"name":"x"}]"#, "name").unwrap(), ["bug", "x"]);
        assert_eq!(parse_names(r#"[{"login":"uchmk"}]"#, "login").unwrap(), ["uchmk"]);
        assert!(parse_names(r#"{"message":"Not Found"}"#, "name").is_err());
    }
}
