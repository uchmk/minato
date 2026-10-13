//! The Actions view without drawing: workflow runs, their jobs and artifacts
//! as GitHub sends them, the filters and the selection, and the calls that
//! change a run (re-run, cancel, dispatch).

use serde_json::{Value, json};

use crate::github::Method;
use crate::github::worker::Call;
use crate::list::epoch;

/// How a run stands, for the mark and the colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ok,
    Bad,
    Running,
    Queued,
    /// Cancelled, skipped or anything else that is not a result.
    Idle,
}

impl Status {
    /// From GitHub's `status` and `conclusion`.
    pub fn from(status: &str, conclusion: Option<&str>) -> Status {
        match (status, conclusion) {
            ("completed", Some("success")) => Status::Ok,
            ("completed", Some("failure" | "timed_out" | "startup_failure" | "action_required")) => Status::Bad,
            ("completed", _) => Status::Idle,
            ("in_progress", _) => Status::Running,
            _ => Status::Queued,
        }
    }

    pub fn mark(self) -> &'static str {
        match self {
            Status::Ok => "✓",
            Status::Bad => "✗",
            Status::Running => "◐",
            Status::Queued => "◷",
            Status::Idle => "⊘",
        }
    }

    /// The word-table key.
    pub fn word(self) -> &'static str {
        match self {
            Status::Ok => "run.ok",
            Status::Bad => "run.bad",
            Status::Running => "run.running",
            Status::Queued => "run.queued",
            Status::Idle => "run.idle",
        }
    }

    pub fn active(self) -> bool {
        matches!(self, Status::Running | Status::Queued)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub id: u64,
    /// `owner/name`
    pub repo: String,
    pub workflow_id: u64,
    pub workflow: String,
    pub number: u64,
    pub title: String,
    pub event: String,
    pub branch: String,
    /// The first 7 characters of the commit.
    pub sha: String,
    pub actor: String,
    pub status: Status,
    pub started_at: String,
    pub updated_at: String,
    pub url: String,
    /// The pull request the run belongs to, if any.
    pub pr: Option<u64>,
    /// The workflow file, e.g. `.github/workflows/ci.yml`.
    pub path: String,
    /// The whole commit id (`sha` is its first 7 characters).
    pub head_sha: String,
}

impl Run {
    /// Seconds the run took, or has taken so far (`now` is Unix seconds).
    pub fn seconds(&self, now: i64) -> Option<i64> {
        let from = epoch(&self.started_at)?;
        let to = if self.status.active() { now } else { epoch(&self.updated_at)? };
        Some((to - from).max(0))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Job {
    pub id: u64,
    pub name: String,
    pub status: Status,
    pub started_at: String,
    pub completed_at: String,
    pub url: String,
}

impl Job {
    pub fn seconds(&self, now: i64) -> Option<i64> {
        let from = epoch(&self.started_at)?;
        let to = if self.status.active() { now } else { epoch(&self.completed_at)? };
        Some((to - from).max(0))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Artifact {
    pub name: String,
    pub bytes: u64,
}

/// "42s", "2m10s", "1h05m".
pub fn duration(s: i64) -> String {
    match s {
        0..=59 => format!("{s}s"),
        60..=3599 => format!("{}m{:02}s", s / 60, s % 60),
        _ => format!("{}h{:02}m", s / 3600, s % 3600 / 60),
    }
}

/// "9.8 MB".
pub fn size(bytes: u64) -> String {
    match bytes {
        0..=1023 => format!("{bytes} B"),
        1024..=1_048_575 => format!("{:.1} KB", bytes as f64 / 1024.0),
        _ => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
    }
}

fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn num(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or_default()
}

fn list(body: &str, key: &str) -> Result<Vec<Value>, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    match v.get(key).and_then(Value::as_array) {
        Some(a) => Ok(a.clone()),
        None => Err(v.get("message").and_then(Value::as_str).map_or_else(|| format!("no `{key}` in the answer"), str::to_string)),
    }
}

/// The repositories to look at: not archived, newest push first (as sent).
pub fn parse_repos(body: &str) -> Result<Vec<String>, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let a = v.as_array().ok_or("not a list")?;
    Ok(a.iter().filter(|r| !r.get("archived").and_then(Value::as_bool).unwrap_or(false)).filter_map(|r| r.get("full_name")?.as_str().map(str::to_string)).collect())
}

pub fn parse_runs(body: &str) -> Result<Vec<Run>, String> {
    Ok(list(body, "workflow_runs")?
        .iter()
        .map(|r| {
            let conclusion = r.get("conclusion").and_then(Value::as_str);
            Run {
                id: num(r, "id"),
                repo: r.pointer("/repository/full_name").and_then(Value::as_str).unwrap_or_default().to_string(),
                workflow_id: num(r, "workflow_id"),
                workflow: text(r, "name"),
                number: num(r, "run_number"),
                title: text(r, "display_title"),
                event: text(r, "event"),
                branch: text(r, "head_branch"),
                sha: text(r, "head_sha").chars().take(7).collect(),
                actor: r.pointer("/actor/login").and_then(Value::as_str).unwrap_or_default().to_string(),
                status: Status::from(&text(r, "status"), conclusion),
                started_at: {
                    let s = text(r, "run_started_at");
                    if s.is_empty() { text(r, "created_at") } else { s }
                },
                updated_at: text(r, "updated_at"),
                url: text(r, "html_url"),
                pr: r.pointer("/pull_requests/0/number").and_then(Value::as_u64),
                path: text(r, "path"),
                head_sha: text(r, "head_sha"),
            }
        })
        .collect())
}

pub fn parse_jobs(body: &str) -> Result<Vec<Job>, String> {
    Ok(list(body, "jobs")?
        .iter()
        .map(|j| Job {
            id: num(j, "id"),
            name: text(j, "name"),
            status: Status::from(&text(j, "status"), j.get("conclusion").and_then(Value::as_str)),
            started_at: text(j, "started_at"),
            completed_at: text(j, "completed_at"),
            url: text(j, "html_url"),
        })
        .collect())
}

pub fn parse_artifacts(body: &str) -> Result<Vec<Artifact>, String> {
    Ok(list(body, "artifacts")?.iter().map(|a| Artifact { name: text(a, "name"), bytes: num(a, "size_in_bytes") }).collect())
}

/// The viewer's own repositories, or those of `owner` (a user or
/// organization), newest push first.
pub fn repos_path(owner: Option<&str>) -> String {
    match owner {
        None => "/user/repos?affiliation=owner&sort=pushed&per_page=12".into(),
        Some(o) => format!("/users/{o}/repos?type=owner&sort=pushed&per_page=12"),
    }
}

pub fn runs_path(repo: &str) -> String {
    format!("/repos/{repo}/actions/runs?per_page=10")
}

pub fn jobs_path(run: &Run) -> String {
    format!("/repos/{}/actions/runs/{}/jobs?per_page=50", run.repo, run.id)
}

pub fn artifacts_path(run: &Run) -> String {
    format!("/repos/{}/actions/runs/{}/artifacts?per_page=20", run.repo, run.id)
}

fn post(path: String, body: Value) -> Call {
    Call::Send { method: Method::Post, path, body: body.to_string() }
}

/// Run only the failed jobs again.
pub fn rerun_failed(run: &Run) -> Call {
    post(format!("/repos/{}/actions/runs/{}/rerun-failed-jobs", run.repo, run.id), json!({}))
}

/// Stop a run that is going or waiting.
pub fn cancel(run: &Run) -> Call {
    post(format!("/repos/{}/actions/runs/{}/cancel", run.repo, run.id), json!({}))
}

/// Start the run's workflow by hand on the run's branch (no inputs).
pub fn dispatch(run: &Run) -> Call {
    post(format!("/repos/{}/actions/workflows/{}/dispatches", run.repo, run.workflow_id), json!({ "ref": run.branch }))
}

/// Start the run's workflow on the run's branch with the given inputs.
pub fn dispatch_with(run: &Run, inputs: &[(String, String)]) -> Call {
    let map: serde_json::Map<String, Value> = inputs.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect();
    post(format!("/repos/{}/actions/workflows/{}/dispatches", run.repo, run.workflow_id), json!({ "ref": run.branch, "inputs": map }))
}

/// The page of the pull request a run belongs to. GitHub leaves the number out
/// once the PR is merged or when it comes from a fork, so a `pull_request` run
/// without one opens the search for its branch.
pub fn pr_url(run: &Run) -> Option<String> {
    match run.pr {
        Some(n) => Some(format!("https://github.com/{}/pull/{n}", run.repo)),
        None if has_pr(run) => Some(format!("https://github.com/{}/pulls?q=is%3Apr+head%3A{}", run.repo, run.branch)),
        None => None,
    }
}

/// Whether the run belongs to a pull request, known or not.
pub fn has_pr(run: &Run) -> bool {
    run.pr.is_some() || run.event.starts_with("pull_request")
}

/// What the filter column chooses from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    Repo(Option<String>),
    State(Option<Bucket>),
    Workflow(Option<String>),
}

/// The state filter's groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bucket {
    Bad,
    Active,
    Ok,
    Idle,
}

impl Bucket {
    pub fn word(self) -> &'static str {
        match self {
            Bucket::Bad => "run.bad",
            Bucket::Active => "run.active",
            Bucket::Ok => "run.ok",
            Bucket::Idle => "run.idle",
        }
    }

    fn holds(self, s: Status) -> bool {
        match self {
            Bucket::Bad => s == Status::Bad,
            Bucket::Active => s.active(),
            Bucket::Ok => s == Status::Ok,
            Bucket::Idle => s == Status::Idle,
        }
    }
}

/// The columns `h` and `l` move between.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Col {
    Filters,
    Runs,
    Detail,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActState {
    pub col: Col,
    pub repo: Option<String>,
    pub state: Option<Bucket>,
    pub workflow: Option<String>,
    pub cursor: usize,
    pub sel: usize,
    pub scroll: f32,
}

impl Default for ActState {
    fn default() -> Self {
        ActState { col: Col::Runs, repo: None, state: None, workflow: None, cursor: 0, sel: 0, scroll: 0.0 }
    }
}

/// The filter column: repositories, states and workflows present in `runs`.
pub fn options(runs: &[Run]) -> Vec<(&'static str, Vec<Pick>)> {
    fn distinct(it: impl Iterator<Item = String>) -> Vec<Option<String>> {
        let mut v: Vec<String> = it.collect();
        v.sort();
        v.dedup();
        std::iter::once(None).chain(v.into_iter().map(Some)).collect()
    }
    vec![
        ("repo", distinct(runs.iter().map(|r| r.repo.clone())).into_iter().map(Pick::Repo).collect()),
        ("state", [None, Some(Bucket::Bad), Some(Bucket::Active), Some(Bucket::Ok), Some(Bucket::Idle)].into_iter().map(Pick::State).collect()),
        ("workflow", distinct(runs.iter().map(|r| r.workflow.clone())).into_iter().map(Pick::Workflow).collect()),
    ]
}

impl ActState {
    pub fn matches(&self, r: &Run) -> bool {
        self.repo.as_ref().is_none_or(|x| *x == r.repo) && self.state.is_none_or(|b| b.holds(r.status)) && self.workflow.as_ref().is_none_or(|w| *w == r.workflow)
    }

    pub fn shown<'a>(&self, runs: &'a [Run]) -> Vec<&'a Run> {
        runs.iter().filter(|r| self.matches(r)).collect()
    }

    pub fn selected<'a>(&self, runs: &'a [Run]) -> Option<&'a Run> {
        self.shown(runs).get(self.sel).copied()
    }

    /// Every run, those near the selected one first (the next, the one
    /// before, and on outward in the view's order), then the hidden ones as
    /// they are: the order the runs' jobs are read ahead in.
    pub fn near_first<'a>(&self, runs: &'a [Run]) -> Vec<&'a Run> {
        let shown = self.shown(runs);
        let sel = self.sel.min(shown.len().saturating_sub(1));
        let mut out = Vec::with_capacity(runs.len());
        for d in 0..shown.len() {
            out.extend(shown.get(sel + d));
            if d > 0 && d <= sel {
                out.push(shown[sel - d]);
            }
        }
        out.extend(runs.iter().filter(|r| !self.matches(r)));
        out
    }

    pub fn is_chosen(&self, p: &Pick) -> bool {
        match p {
            Pick::Repo(r) => self.repo == *r,
            Pick::State(s) => self.state == *s,
            Pick::Workflow(w) => self.workflow == *w,
        }
    }

    pub fn apply(&mut self, p: Pick) {
        match p {
            Pick::Repo(r) => self.repo = r,
            Pick::State(s) => self.state = s,
            Pick::Workflow(w) => self.workflow = w,
        }
        self.sel = 0;
        self.scroll = 0.0;
    }

    /// A key typed in the view: the cursor and the columns. Returns whether
    /// it was used.
    pub fn key(&mut self, c: char, runs: &[Run]) -> bool {
        let n = self.shown(runs).len();
        let flat: Vec<Pick> = options(runs).into_iter().flat_map(|(_, v)| v).collect();
        match (self.col, c) {
            (Col::Filters, 'j') => self.cursor = (self.cursor + 1).min(flat.len().saturating_sub(1)),
            (Col::Filters, 'k') => self.cursor = self.cursor.saturating_sub(1),
            (Col::Filters, ' ') => {
                if let Some(p) = flat.get(self.cursor) {
                    self.apply(p.clone());
                }
            }
            (Col::Runs, 'j') => self.sel = (self.sel + 1).min(n.saturating_sub(1)),
            (Col::Runs, 'k') => self.sel = self.sel.saturating_sub(1),
            (Col::Detail, 'j') => self.scroll += 1.0,
            (Col::Detail, 'k') => self.scroll = (self.scroll - 1.0).max(0.0),
            (_, 'h') => self.col = if self.col == Col::Detail { Col::Runs } else { Col::Filters },
            (_, 'l') => self.col = if self.col == Col::Filters { Col::Runs } else { Col::Detail },
            _ => return false,
        }
        if matches!(c, 'j' | 'k') && self.col == Col::Runs {
            self.scroll = 0.0;
        }
        true
    }

    pub fn clamp(&mut self, runs: &[Run]) {
        self.sel = self.sel.min(self.shown(runs).len().saturating_sub(1));
    }

    /// Keep the selection on the same run after the runs changed (a new run
    /// above it, or a re-run moving it to the top); clamped when it is gone.
    pub fn stay(&mut self, before: &[Run], after: &[Run]) {
        let id = self.selected(before).map(|r| r.id);
        if let Some(k) = id.and_then(|id| self.shown(after).iter().position(|r| r.id == id)) {
            self.sel = k;
        }
        self.clamp(after);
    }

    /// `gg` and `G`, as the list's.
    pub fn jump(&mut self, end: bool, runs: &[Run]) {
        match self.col {
            Col::Filters => {
                let n: usize = options(runs).iter().map(|(_, v)| v.len()).sum();
                self.cursor = if end { n.saturating_sub(1) } else { 0 };
            }
            Col::Runs => {
                self.sel = if end { self.shown(runs).len().saturating_sub(1) } else { 0 };
                self.scroll = 0.0;
            }
            Col::Detail => self.scroll = if end { f32::MAX } else { 0.0 },
        }
    }
}

/// The last `n` finished results of the run's workflow, newest first.
pub fn history<'a>(runs: &'a [Run], of: &Run, n: usize) -> Vec<&'a Run> {
    runs.iter().filter(|r| r.repo == of.repo && r.workflow_id == of.workflow_id && !r.status.active()).take(n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUNS: &str = r#"{"total_count":3,"workflow_runs":[
      {"id":11,"name":"check","display_title":"Fix it","run_number":7,"event":"pull_request","head_branch":"feat/x","head_sha":"a3f9c21deadbeef","status":"completed","conclusion":"failure","workflow_id":5,"run_started_at":"2026-10-12T09:00:00Z","created_at":"2026-10-12T08:59:00Z","updated_at":"2026-10-12T09:02:10Z","html_url":"https://github.com/uchmk/kura/actions/runs/11","actor":{"login":"uchmk"},"pull_requests":[{"number":361}],"repository":{"full_name":"uchmk/kura"}},
      {"id":12,"name":"release","display_title":"v1","run_number":3,"event":"workflow_dispatch","head_branch":"main","head_sha":"4a9619d","status":"in_progress","conclusion":null,"workflow_id":6,"created_at":"2026-10-12T09:10:00Z","updated_at":"2026-10-12T09:10:30Z","html_url":"u","actor":{"login":"uchmk"},"pull_requests":[],"repository":{"full_name":"uchmk/tsumugi"}},
      {"id":13,"name":"check","display_title":"Ok","run_number":6,"event":"push","head_branch":"main","head_sha":"b","status":"completed","conclusion":"success","workflow_id":5,"run_started_at":"2026-10-11T09:00:00Z","updated_at":"2026-10-11T09:03:00Z","html_url":"u","actor":{"login":"x"},"pull_requests":[],"repository":{"full_name":"uchmk/kura"}}]}"#;

    fn runs() -> Vec<Run> {
        parse_runs(RUNS).unwrap()
    }

    #[test]
    fn runs_are_read() {
        let r = runs();
        assert_eq!(r.len(), 3);
        assert_eq!((r[0].status, r[0].sha.as_str(), r[0].pr, r[0].repo.as_str()), (Status::Bad, "a3f9c21", Some(361), "uchmk/kura"));
        assert_eq!(r[1].status, Status::Running);
        // No `run_started_at`: the creation time stands in.
        assert_eq!(r[1].started_at, "2026-10-12T09:10:00Z");
        assert_eq!(r[0].seconds(0), Some(130));
        assert_eq!(duration(130), "2m10s");
        assert_eq!(r[1].seconds(epoch("2026-10-12T09:11:00Z").unwrap()), Some(60));
    }

    #[test]
    fn a_message_instead_of_runs_is_an_error() {
        assert_eq!(parse_runs(r#"{"message":"Not Found"}"#), Err("Not Found".into()));
    }

    #[test]
    fn statuses_come_from_status_and_conclusion() {
        assert_eq!(Status::from("completed", Some("cancelled")), Status::Idle);
        assert_eq!(Status::from("completed", Some("timed_out")), Status::Bad);
        assert_eq!(Status::from("queued", None), Status::Queued);
        assert_eq!(Status::from("waiting", None), Status::Queued);
    }

    #[test]
    fn another_owners_repos_are_their_public_ones() {
        assert_eq!(repos_path(None), "/user/repos?affiliation=owner&sort=pushed&per_page=12");
        assert_eq!(repos_path(Some("rust-lang")), "/users/rust-lang/repos?type=owner&sort=pushed&per_page=12");
    }

    #[test]
    fn jobs_artifacts_and_repos_are_read() {
        let j = parse_jobs(r#"{"jobs":[{"id":1,"name":"test","status":"completed","conclusion":"failure","started_at":"2026-10-12T09:00:00Z","completed_at":"2026-10-12T09:00:14Z","html_url":"u"}]}"#).unwrap();
        assert_eq!((j[0].status, j[0].seconds(0)), (Status::Bad, Some(14)));
        let a = parse_artifacts(r#"{"artifacts":[{"name":"x.zip","size_in_bytes":10276044}]}"#).unwrap();
        assert_eq!(size(a[0].bytes), "9.8 MB");
        let r = parse_repos(r#"[{"full_name":"uchmk/a","archived":false},{"full_name":"uchmk/old","archived":true}]"#).unwrap();
        assert_eq!(r, ["uchmk/a"]);
    }

    #[test]
    fn filters_narrow_and_keys_move() {
        let runs = runs();
        let mut s = ActState::default();
        assert_eq!(s.shown(&runs).len(), 3);
        s.apply(Pick::State(Some(Bucket::Active)));
        assert_eq!(s.shown(&runs).len(), 1);
        s.apply(Pick::State(None));
        s.apply(Pick::Workflow(Some("check".into())));
        assert_eq!(s.shown(&runs).len(), 2);
        assert!(s.key('j', &runs));
        assert_eq!(s.sel, 1);
        assert!(s.key('h', &runs) && s.col == Col::Filters);
        assert!(s.key('j', &runs) && s.cursor == 1);
        assert!(!s.key('x', &runs));
        let o = options(&runs);
        assert_eq!(o[0].1.len(), 3); // All + two repositories
        assert_eq!(o[2].1.len(), 3);
    }

    #[test]
    fn the_runs_near_the_selected_one_come_first() {
        let runs = runs();
        let ids = |s: &ActState| s.near_first(&runs).iter().map(|r| r.id).collect::<Vec<_>>();
        let mut s = ActState::default();
        assert_eq!(ids(&s), [11, 12, 13]);
        s.sel = 1;
        assert_eq!(ids(&s), [12, 13, 11]);
        s.sel = 2;
        assert_eq!(ids(&s), [13, 12, 11]);
        // The hidden run comes last.
        s.apply(Pick::Workflow(Some("check".into())));
        s.sel = 1;
        assert_eq!(ids(&s), [13, 11, 12]);
        assert!(ActState::default().near_first(&[]).is_empty());
    }

    #[test]
    fn the_selection_stays_on_its_run_when_one_comes_above_it() {
        let runs = runs();
        let mut s = ActState { sel: 1, ..Default::default() };
        let mut more = runs.clone();
        more.insert(0, Run { id: 99, ..runs[2].clone() });
        s.stay(&runs, &more);
        assert_eq!(s.selected(&more).map(|r| r.id), Some(12));
        // Gone: the row is kept, inside the list.
        s.stay(&more, &more[..2]);
        assert_eq!(s.sel, 1);
    }

    #[test]
    fn history_skips_running_and_other_workflows() {
        let runs = runs();
        let h = history(&runs, &runs[0], 10);
        assert_eq!(h.iter().map(|r| r.id).collect::<Vec<_>>(), [11, 13]);
    }

    #[test]
    fn the_calls_name_the_run() {
        let r = &runs()[0];
        let Call::Send { method, path, .. } = rerun_failed(r) else { panic!() };
        assert_eq!((method, path.as_str()), (Method::Post, "/repos/uchmk/kura/actions/runs/11/rerun-failed-jobs"));
        let Call::Send { path, .. } = cancel(r) else { panic!() };
        assert!(path.ends_with("/runs/11/cancel"));
        let Call::Send { path, body, .. } = dispatch(r) else { panic!() };
        assert_eq!(path, "/repos/uchmk/kura/actions/workflows/5/dispatches");
        assert_eq!(body, r#"{"ref":"feat/x"}"#);
        assert_eq!(pr_url(r).as_deref(), Some("https://github.com/uchmk/kura/pull/361"));
        assert_eq!(pr_url(&runs()[1]), None);
        let merged = Run { pr: None, event: "pull_request".into(), ..r.clone() };
        assert!(has_pr(&merged));
        assert_eq!(pr_url(&merged).as_deref(), Some("https://github.com/uchmk/kura/pulls?q=is%3Apr+head%3Afeat/x"));
    }
}
