//! The list's two reads: a search for the items (issues, PRs, discussions)
//! and, for the one selected, its body, comments and checks. The queries are
//! here with the parsers; the parsers are tested on recorded answers.

use serde::Deserialize;

use crate::list::{Check, Ci, Detail, Entry, Item, Kind, Label, peek_of};

const ITEM_COMMON: &str = "id number title url createdAt updatedAt author { login } repository { nameWithOwner } \
labels(first: 10) { nodes { name color } } comments { totalCount } bodyText";

/// Who a PR asks for a review: people by login, teams by name.
const REVIEWERS: &str = "reviewRequests(first: 5) { nodes { requestedReviewer { __typename ... on User { login } ... on Team { name } } } }";

/// The search for issues and PRs. Variables: `q` (the query text), `n`.
pub fn items_query() -> String {
    format!(
        "query($q: String!, $n: Int!) {{ search(query: $q, type: ISSUE, first: $n) {{ nodes {{ __typename \
... on Issue {{ {ITEM_COMMON} state milestone {{ title }} assignees(first: 5) {{ nodes {{ login }} }} }} \
... on PullRequest {{ {ITEM_COMMON} state isDraft mergeable headRefName milestone {{ title }} assignees(first: 5) {{ nodes {{ login }} }} {REVIEWERS} \
commits(last: 1) {{ nodes {{ commit {{ statusCheckRollup {{ state }} }} }} }} }} }} }} }}"
    )
}

/// One issue or PR by its page, open or closed (for a notification of
/// one the searches do not list). A discussion's page is no resource
/// there: its answer is null and the page opens. Variables: `url`.
pub fn item_query() -> String {
    format!(
        "query($url: URI!) {{ resource(url: $url) {{ __typename \
... on Issue {{ {ITEM_COMMON} state milestone {{ title }} assignees(first: 5) {{ nodes {{ login }} }} }} \
... on PullRequest {{ {ITEM_COMMON} state isDraft mergeable headRefName milestone {{ title }} assignees(first: 5) {{ nodes {{ login }} }} {REVIEWERS} \
commits(last: 1) {{ nodes {{ commit {{ statusCheckRollup {{ state }} }} }} }} }} }} }}"
    )
}

/// The variables of `item_query`.
pub fn item_vars(url: &str) -> String {
    serde_json::json!({ "url": url }).to_string()
}

/// The search for discussions. Variables: `q`, `n`.
pub fn discussions_query() -> String {
    format!(
        "query($q: String!, $n: Int!) {{ search(query: $q, type: DISCUSSION, first: $n) {{ nodes {{ __typename \
... on Discussion {{ {ITEM_COMMON} }} }} }} }}"
    )
}

/// One item's body, comments and checks. Variables: `id`.
pub fn detail_query() -> &'static str {
    "query($id: ID!) { node(id: $id) { __typename \
... on Issue { body comments(last: 20) { nodes { author { login } createdAt body } } } \
... on Discussion { body comments(last: 20) { nodes { id isAnswer author { login } createdAt body } } } \
... on PullRequest { body comments(last: 20) { nodes { author { login } createdAt body } } \
commits(last: 1) { nodes { commit { statusCheckRollup { contexts(first: 30) { nodes { __typename \
... on CheckRun { name conclusion status } ... on StatusContext { context state } } } } } } } } } }"
}

#[derive(Deserialize)]
struct Data<T> {
    data: Option<T>,
    #[serde(default)]
    errors: Vec<GqlError>,
}

#[derive(Deserialize)]
struct GqlError {
    message: String,
}

fn unwrap<T>(d: Data<T>) -> Result<T, String> {
    match d.data {
        Some(t) => Ok(t),
        None => Err(d.errors.into_iter().next().map_or("GitHub sent no data".to_string(), |e| e.message)),
    }
}

#[derive(Deserialize)]
struct Nodes<T> {
    #[serde(default = "Vec::new")]
    nodes: Vec<T>,
}

#[derive(Deserialize)]
struct Login {
    login: String,
}

#[derive(Deserialize)]
struct Count {
    #[serde(rename = "totalCount")]
    total_count: u32,
}

#[derive(Deserialize)]
struct Repo {
    #[serde(rename = "nameWithOwner")]
    name_with_owner: String,
}

#[derive(Deserialize)]
struct RawLabel {
    name: String,
    color: String,
}

#[derive(Deserialize)]
struct Rollup {
    state: String,
}

#[derive(Deserialize)]
struct CommitNode {
    commit: CommitInner,
}

#[derive(Deserialize)]
struct CommitInner {
    #[serde(rename = "statusCheckRollup")]
    rollup: Option<Rollup>,
}

#[derive(Deserialize)]
struct Title {
    title: String,
}

#[derive(Deserialize)]
struct ReviewRequest {
    #[serde(rename = "requestedReviewer")]
    reviewer: Option<Reviewer>,
}

#[derive(Deserialize)]
struct Reviewer {
    login: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct RawItem {
    #[serde(rename = "__typename")]
    typename: String,
    id: Option<String>,
    number: Option<u64>,
    title: Option<String>,
    url: Option<String>,
    #[serde(rename = "createdAt")]
    created_at: Option<String>,
    #[serde(rename = "updatedAt")]
    updated_at: Option<String>,
    author: Option<Login>,
    repository: Option<Repo>,
    labels: Option<Nodes<RawLabel>>,
    comments: Option<Count>,
    state: Option<String>,
    #[serde(rename = "isDraft", default)]
    is_draft: bool,
    mergeable: Option<String>,
    #[serde(rename = "headRefName")]
    head: Option<String>,
    assignees: Option<Nodes<Login>>,
    milestone: Option<Title>,
    #[serde(rename = "reviewRequests")]
    review_requests: Option<Nodes<ReviewRequest>>,
    commits: Option<Nodes<CommitNode>>,
    #[serde(rename = "bodyText")]
    body_text: Option<String>,
}

#[derive(Deserialize)]
struct Search {
    search: Nodes<RawItem>,
}

#[derive(Deserialize)]
struct Resource {
    resource: Option<RawItem>,
}

/// Rollup state to a CI mark.
pub fn ci_of(state: &str) -> Ci {
    match state {
        "SUCCESS" | "NEUTRAL" | "SKIPPED" => Ci::Ok,
        "PENDING" | "EXPECTED" | "QUEUED" | "IN_PROGRESS" | "WAITING" | "REQUESTED" => Ci::Pending,
        _ => Ci::Bad,
    }
}

/// The items of a search answer. Nodes of a kind we do not know (the search
/// returns whatever matched) are skipped.
pub fn parse_items(body: &str) -> Result<Vec<Item>, String> {
    let d: Data<Search> = serde_json::from_str(body).map_err(|e| format!("unexpected search answer: {e}"))?;
    Ok(unwrap(d)?.search.nodes.into_iter().filter_map(item_of).collect())
}

/// The item of an `item_query` answer; `None` when the page is not an
/// issue, PR or discussion.
pub fn parse_item(body: &str) -> Result<Option<Item>, String> {
    let d: Data<Resource> = serde_json::from_str(body).map_err(|e| format!("unexpected item answer: {e}"))?;
    Ok(unwrap(d)?.resource.and_then(item_of))
}

fn item_of(n: RawItem) -> Option<Item> {
    let kind = match n.typename.as_str() {
        "Issue" => Kind::Issue,
        "PullRequest" => Kind::Pr,
        "Discussion" => Kind::Discussion,
        _ => return None,
    };
    let (Some(id), Some(number), Some(title), Some(repo)) = (n.id, n.number, n.title, n.repository) else { return None };
    let ci = n
        .commits
        .and_then(|c| c.nodes.into_iter().next())
        .and_then(|c| c.commit.rollup)
        .map(|r| ci_of(&r.state));
    Some(Item {
        id,
        kind,
        repo: repo.name_with_owner,
        number,
        title,
        url: n.url.unwrap_or_default(),
        author: n.author.map_or_else(|| "ghost".to_string(), |a| a.login),
        labels: n.labels.map_or_else(Vec::new, |l| l.nodes.into_iter().map(|l| Label { name: l.name, color: l.color }).collect()),
        assignees: n.assignees.map_or_else(Vec::new, |a| a.nodes.into_iter().map(|a| a.login).collect()),
        milestone: n.milestone.map(|m| m.title).unwrap_or_default(),
        reviewers: n.review_requests.map_or_else(Vec::new, |r| r.nodes.into_iter().filter_map(|r| r.reviewer.and_then(|v| v.login.or(v.name))).collect()),
        comments: n.comments.map_or(0, |c| c.total_count),
        created_at: n.created_at.unwrap_or_default(),
        updated_at: n.updated_at.unwrap_or_default(),
        draft: n.is_draft,
        merged: n.state.as_deref() == Some("MERGED"),
        closed: n.state.as_deref() == Some("CLOSED"),
        conflict: n.mergeable.as_deref() == Some("CONFLICTING"),
        mentioned: false,
        ci,
        peek: n.body_text.as_deref().map(peek_of).unwrap_or_default(),
        head: n.head.unwrap_or_default(),
    })
}

#[derive(Deserialize)]
struct RawComment {
    #[serde(default)]
    id: String,
    #[serde(default, rename = "isAnswer")]
    is_answer: bool,
    author: Option<Login>,
    #[serde(rename = "createdAt")]
    created_at: String,
    body: String,
}

#[derive(Deserialize)]
struct RawContext {
    #[serde(rename = "__typename")]
    typename: String,
    name: Option<String>,
    conclusion: Option<String>,
    status: Option<String>,
    context: Option<String>,
    state: Option<String>,
}

#[derive(Deserialize)]
struct RawRollup {
    contexts: Nodes<RawContext>,
}

#[derive(Deserialize)]
struct RawCommit {
    commit: RawCommitInner,
}

#[derive(Deserialize)]
struct RawCommitInner {
    #[serde(rename = "statusCheckRollup")]
    rollup: Option<RawRollup>,
}

#[derive(Deserialize)]
struct RawDetail {
    body: Option<String>,
    comments: Option<Nodes<RawComment>>,
    commits: Option<Nodes<RawCommit>>,
}

#[derive(Deserialize)]
struct NodeAnswer {
    node: Option<RawDetail>,
}

/// The selected item's body, comments and checks.
pub fn parse_detail(body: &str) -> Result<Detail, String> {
    let d: Data<NodeAnswer> = serde_json::from_str(body).map_err(|e| format!("unexpected detail answer: {e}"))?;
    let raw = unwrap(d)?.node.ok_or("the item is gone")?;
    let checks = raw
        .commits
        .and_then(|c| c.nodes.into_iter().next())
        .and_then(|c| c.commit.rollup)
        .map(|r| {
            r.contexts
                .nodes
                .into_iter()
                .filter_map(|c| match c.typename.as_str() {
                    "CheckRun" => {
                        let ci = match (c.status.as_deref(), c.conclusion.as_deref()) {
                            (Some("COMPLETED"), Some(k)) => ci_of(k),
                            _ => Ci::Pending,
                        };
                        Some(Check { name: c.name?, ci })
                    }
                    "StatusContext" => Some(Check { name: c.context?, ci: ci_of(c.state.as_deref()?) }),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    let timeline = raw
        .comments
        .map(|c| {
            c.nodes
                .into_iter()
                .map(|c| Entry { id: c.id, answer: c.is_answer, author: c.author.map_or_else(|| "ghost".to_string(), |a| a.login), created_at: c.created_at, body: c.body })
                .collect()
        })
        .unwrap_or_default();
    Ok(Detail { body: raw.body.unwrap_or_default(), checks, timeline })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issues_prs_and_discussions_are_read() {
        let items = parse_items(include_str!("../../tests/fixtures/search.json")).unwrap();
        assert_eq!(items.len(), 3);
        let pr = &items[0];
        assert_eq!((pr.kind, pr.repo.as_str(), pr.number), (Kind::Pr, "uchmk/tsumugi", 66));
        assert_eq!(pr.ci, Some(Ci::Bad));
        assert_eq!(pr.labels[0].name, "lane:win");
        assert_eq!(pr.assignees, vec!["uchmk".to_string()]);
        assert_eq!(pr.comments, 2);
        assert_eq!((pr.milestone.as_str(), pr.reviewers.clone()), ("v1", vec!["kura-bot".to_string(), "core".to_string()]));
        assert_eq!(pr.peek, "Fixes the font.\nThe UI now reads it from common.toml.");
        let issue = &items[1];
        assert_eq!((issue.kind, issue.ci, issue.author.as_str()), (Kind::Issue, None, "ghost"));
        assert!(issue.milestone.is_empty() && issue.reviewers.is_empty());
        assert_eq!(items[2].kind, Kind::Discussion);
        assert_eq!(items[2].peek, "");
    }

    #[test]
    fn an_error_answer_gives_its_message() {
        let e = parse_items("{\"data\":null,\"errors\":[{\"message\":\"Bad credentials\"}]}").unwrap_err();
        assert_eq!(e, "Bad credentials");
        assert!(parse_items("nope").is_err());
    }

    #[test]
    fn the_detail_has_body_checks_and_comments() {
        let d = parse_detail(include_str!("../../tests/fixtures/detail.json")).unwrap();
        assert!(d.body.starts_with("Takes the UI font"));
        assert_eq!(d.checks.len(), 3);
        assert_eq!((d.checks[0].name.as_str(), d.checks[0].ci), ("test (windows)", Ci::Bad));
        assert_eq!(d.checks[1].ci, Ci::Ok);
        assert_eq!(d.checks[2].ci, Ci::Pending);
        assert_eq!(d.timeline.len(), 2);
        assert_eq!(d.timeline[0].author, "uchmk");
        assert!(parse_detail("{\"data\":{\"node\":null}}").is_err());
    }

    #[test]
    fn discussion_comments_carry_their_id_and_answer_mark() {
        let body = r#"{"data":{"node":{"body":"q","comments":{"nodes":[{"id":"DC_1","isAnswer":true,"author":{"login":"a"},"createdAt":"2026-01-01T00:00:00Z","body":"x"},{"author":null,"createdAt":"2026-01-02T00:00:00Z","body":"y"}]}}}}"#;
        let d = parse_detail(body).unwrap();
        assert_eq!((d.timeline[0].id.as_str(), d.timeline[0].answer), ("DC_1", true));
        assert_eq!((d.timeline[1].id.as_str(), d.timeline[1].answer), ("", false));
    }

    #[test]
    fn one_item_is_read_by_its_page() {
        let body = r#"{"data":{"resource":{"__typename":"PullRequest","id":"PR_9","number":9,"title":"Fix","url":"https://github.com/u/r/pull/9","createdAt":"2026-10-01T00:00:00Z","updatedAt":"2026-10-02T00:00:00Z","author":{"login":"u"},"repository":{"nameWithOwner":"u/r"},"labels":{"nodes":[]},"comments":{"totalCount":1},"bodyText":"b","state":"MERGED","isDraft":false,"mergeable":"UNKNOWN","assignees":{"nodes":[]},"commits":{"nodes":[]}}}}"#;
        let i = parse_item(body).unwrap().unwrap();
        assert_eq!((i.id.as_str(), i.number, i.merged, i.closed, i.kind), ("PR_9", 9, true, false, Kind::Pr));
        let i = parse_item(&body.replace("MERGED", "CLOSED")).unwrap().unwrap();
        assert!(i.closed && !i.merged);
        assert_eq!(parse_item(r#"{"data":{"resource":{"__typename":"Commit"}}}"#), Ok(None));
        assert_eq!(parse_item(r#"{"data":{"resource":null}}"#), Ok(None));
        assert!(parse_item(r#"{"data":null,"errors":[{"message":"nope"}]}"#).is_err());
        assert_eq!(item_vars("https://github.com/u/r/pull/9"), r#"{"url":"https://github.com/u/r/pull/9"}"#);
    }

    #[test]
    fn the_queries_name_their_variables() {
        assert!(items_query().contains("$q: String!"));
        assert!(items_query().contains("milestone { title }") && item_query().contains("reviewRequests(first: 5)"));
        assert!(item_query().contains("resource(url: $url)"));
        // A discussion is no `UniformResourceLocatable`: GitHub refuses
        // the whole query with a fragment on it.
        assert!(!item_query().contains("on Discussion"));
        assert!(discussions_query().contains("type: DISCUSSION"));
        assert!(detail_query().contains("$id: ID!"));
    }
}
