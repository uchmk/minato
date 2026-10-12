//! The link between the window and `minato-core`'s GitHub worker: find the
//! token off the UI thread, ask who it is, and keep the rate limit fresh.
//! Each frame `pump` takes what has arrived; it never waits. The answers
//! are kept on disk (`minato_core::cache`), so a start shows the last ones
//! at once and a detail whose item has not moved is not asked again.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use minato_core::actions::{self, Run};
use minato_core::board;
use minato_core::cache;
use minato_core::notes;
use minato_core::workflow;
use minato_core::github::token::Source;
use minato_core::github::types::{parse_rate_limit, parse_viewer};
use minato_core::github::search::{detail_query, discussions_query, items_query, parse_detail, parse_items};
use minato_core::github::worker::{self, Call, Job, Worker};
use minato_core::github::write::parse_names;
use minato_core::github::{Client, Rate, Token, UreqTransport};

use minato_core::list::Item;
use minato_core::mainci;

use crate::app::App;
use crate::overlay::Overlay;

/// How often the rate limit is asked while plenty is left.
const POLL: Duration = Duration::from_secs(60);

type Found = Option<(Token, Source)>;

pub struct Net {
    token_rx: Receiver<Found>,
    worker: Option<Worker>,
    next_poll: Instant,
    rate: Option<Rate>,
    next_search: Instant,
    /// Whether the last search was for closed items.
    searched_closed: bool,
    /// The last answers of the two searches, merged into `app.items`.
    found: [Vec<Item>; 2],
    /// The saved search the items were last searched with (the list view's).
    searched_custom: Option<String>,
    /// Ids the `mentions:` search found, and the repositories whose main CI is red.
    mentioned: std::collections::HashSet<String>,
    stubs: Vec<Item>,
    /// The detail last asked for (its node id and the item's `updatedAt`).
    asked: Option<(String, String)>,
    /// Ask the next detail from GitHub even if the cache has it (after a write).
    refetch: bool,
    /// What to say when a write (by its job key) has gone through.
    pending: std::collections::HashMap<String, &'static str>,
    /// The repositories whose runs are asked for (none until GitHub says).
    repos: Vec<String>,
    /// The last runs of each repository.
    run_found: std::collections::HashMap<String, Vec<Run>>,
    next_runs: Instant,
    next_board: Instant,
    next_notes: Instant,
    /// The run whose jobs and artifacts were last asked for.
    asked_run: Option<u64>,
    /// The kept answers, until the thread has read them; where to write.
    cache_rx: Option<Receiver<Vec<cache::Entry>>>,
    cache_tx: Option<mpsc::Sender<cache::Entry>>,
    /// The kept details by node id: the item's `updatedAt` then and the body.
    kept: HashMap<String, (String, String)>,
    /// The keys GitHub has answered this run (the cache does not undo them).
    live: HashSet<String>,
}

/// How often the runs are asked again, while the Actions view is open.
const RUNS: Duration = Duration::from_secs(45);

/// How often the board is asked again, while it is open.
const BOARD: Duration = Duration::from_secs(60);

/// How often the notifications are asked again.
const NOTES: Duration = Duration::from_secs(90);

/// How many repositories' runs are shown.
const REPOS: usize = 8;

/// How often the lists are searched again.
const SEARCH: Duration = Duration::from_secs(120);

impl Net {
    /// Merge what the searches found into `app.items`: mentions marked, stubs
    /// for red main branches added (not over closed items).
    fn rebuild(&self, app: &mut App) {
        let mut all: Vec<Item> = self.found.concat();
        for i in &mut all {
            i.mentioned = self.mentioned.contains(&i.id);
        }
        all.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        if !app.show_closed {
            all.extend(self.stubs.iter().cloned());
        }
        app.items = all;
        app.list.clamp(&app.items, app.viewer.as_deref().unwrap_or(""));
    }

    /// Start looking for the token (it may run `gh`), and reading the
    /// answers kept in `cache` (a folder) on a thread of its own.
    pub fn start(ctx: &egui::Context, cache: Option<PathBuf>) -> Net {
        let (tx, token_rx) = mpsc::channel();
        let wake = ctx.clone();
        worker::find_token(move |found| {
            let _ = tx.send(found);
            wake.request_repaint();
        });
        let (cache_rx, cache_tx) = match cache {
            Some(dir) => {
                let (load_tx, load_rx) = mpsc::channel();
                let (dir2, wake) = (dir.clone(), ctx.clone());
                std::thread::spawn(move || {
                    let _ = load_tx.send(cache::load(&dir2, SystemTime::now()));
                    wake.request_repaint();
                });
                (Some(load_rx), Some(cache_writer(dir)))
            }
            None => (None, None),
        };
        Net { token_rx, worker: None, next_poll: Instant::now(), rate: None, next_search: Instant::now(), searched_closed: false, searched_custom: None, found: [Vec::new(), Vec::new()], mentioned: Default::default(), stubs: Vec::new(), asked: None, refetch: false, pending: Default::default(), repos: Vec::new(), run_found: Default::default(), next_runs: Instant::now(), next_board: Instant::now(), next_notes: Instant::now(), asked_run: None, cache_rx, cache_tx, kept: Default::default(), live: Default::default() }
    }

    /// The kept answers, once the thread has read them: each one GitHub has
    /// not answered yet this run is taken as if it had just come; the
    /// details wait until their item is selected.
    fn load_cache(&mut self, app: &mut App) {
        let Some(rx) = &self.cache_rx else { return };
        let Ok(entries) = rx.try_recv() else { return };
        self.cache_rx = None;
        for e in entries {
            if let Some(id) = e.key.strip_prefix("detail:") {
                self.kept.insert(id.to_string(), (e.stamp, e.body));
            } else if cached(&e.key, false) && !self.live.contains(&e.key) {
                self.take(app, &e.key, &e.body, false);
            }
        }
    }

    /// Write an answer to the cache (on the writer's thread).
    fn keep(&self, key: &str, stamp: &str, body: &str) {
        if let Some(tx) = &self.cache_tx {
            let _ = tx.send(cache::Entry { key: key.into(), stamp: stamp.into(), body: body.into() });
        }
    }

    /// An answer, come from GitHub (`live`) or from the cache.
    fn take(&mut self, app: &mut App, key: &str, body: &str, live: bool) {
        match key {
            "me" => match parse_viewer(body) {
                Ok(v) => {
                    app.viewer = Some(v.login);
                    self.next_search = Instant::now();
                }
                Err(e) => app.say(e),
            },
            "rate" => {
                if let Ok(r) = parse_rate_limit(body) {
                    self.rate = Some(r);
                }
            }
            k @ ("items" | "discussions") => {
                let parsed = parse_items(body);
                match parsed {
                    Ok(v) => {
                        self.found[usize::from(k == "discussions")] = v;
                        self.rebuild(app);
                    }
                    Err(e) => app.say(e),
                }
            }
            "mentions" => {
                if let Ok(v) = parse_items(body) {
                    self.mentioned = v.into_iter().map(|i| i.id).collect();
                    self.rebuild(app);
                }
            }
            "mainci" => {
                if let Ok(v) = mainci::parse(body) {
                    self.stubs = v;
                    self.rebuild(app);
                }
            }
            k if k.starts_with("detail:") => {
                let id = &k["detail:".len()..];
                if live {
                    let stamp = app.items.iter().find(|i| i.id == id).map(|i| i.updated_at.clone()).unwrap_or_default();
                    self.kept.insert(id.to_string(), (stamp.clone(), body.to_string()));
                    self.keep(k, &stamp, body);
                }
                if app.selected().is_some_and(|i| i.id == id) {
                    match parse_detail(body) {
                        Ok(d) => app.detail = Some((id.to_string(), d)),
                        Err(e) => app.say(e),
                    }
                }
            }
            k if k.starts_with("labels:") || k.starts_with("people:") => {
                let labels = k.starts_with("labels:");
                let repo = &k[7..];
                let names = parse_names(body, if labels { "name" } else { "login" });
                let open = app.compose.as_ref().and_then(|c| app.items.iter().find(|i| i.id == c.id)).is_some_and(|i| i.repo == repo);
                match (names, open) {
                    (Ok(v), true) => {
                        if let Some(c) = app.compose.as_mut() {
                            *(if labels { &mut c.labels } else { &mut c.people }) = v;
                        }
                    }
                    (Err(e), true) => app.say(e),
                    _ => {}
                }
            }
            k if k.starts_with("miles:") => {
                let repo = &k[6..];
                let open = app.compose.as_ref().and_then(|c| app.items.iter().find(|i| i.id == c.id)).is_some_and(|i| i.repo == repo);
                match (minato_core::github::write::parse_milestones(body), open) {
                    (Ok(v), true) => {
                        if let Some(c) = app.compose.as_mut() {
                            c.milestones = v;
                        }
                    }
                    (Err(e), true) => app.say(e),
                    _ => {}
                }
            }
            "notes" => match notes::parse(body) {
                Ok(v) => {
                    app.unread = v.len() as u32;
                    app.notes = v;
                    app.ns.clamp(app.notes.len());
                }
                Err(e) => app.say(e),
            },
            k if k.starts_with("note:") => {
                if let Some(done) = self.pending.remove(k) {
                    let words = app.tr(done);
                    app.say(words);
                }
                self.next_notes = Instant::now();
            }
            "board" => match board::parse(body) {
                Ok(p) => {
                    app.bs.clamp(&p);
                    app.board = Some(p);
                }
                Err(e) => app.say(e),
            },
            k if k.starts_with("board:") => {
                if body.contains("\"errors\"") {
                    app.say(body.chars().take(160).collect::<String>());
                    self.pending.remove(k);
                } else if let Some(done) = self.pending.remove(k) {
                    let words = app.tr(done);
                    app.say(words);
                }
                // Read the truth back: it undoes a move GitHub refused.
                self.next_board = Instant::now();
            }
            "a-repos" => match actions::parse_repos(body) {
                Ok(v) => {
                    self.repos = v.into_iter().take(REPOS).collect();
                    self.next_runs = Instant::now();
                }
                Err(e) => app.say(e),
            },
            k if k.starts_with("a-runs:") => match actions::parse_runs(body) {
                Ok(v) => {
                    self.run_found.insert(k["a-runs:".len()..].to_string(), v);
                    let mut all: Vec<Run> = self.run_found.values().flatten().cloned().collect();
                    all.sort_by(|a, b| b.started_at.cmp(&a.started_at));
                    app.runs = all;
                    app.acts.clamp(&app.runs);
                }
                Err(e) => app.say(e),
            },
            k if k.starts_with("a-jobs:") || k.starts_with("a-arts:") => {
                let jobs = k.starts_with("a-jobs:");
                let id: u64 = k[7..].parse().unwrap_or(0);
                let d = app.run_detail.get_or_insert(crate::app::RunDetail { id, ..Default::default() });
                if d.id != id {
                    *d = crate::app::RunDetail { id, ..Default::default() };
                }
                if jobs {
                    match actions::parse_jobs(body) {
                        Ok(v) => d.jobs = Some(v),
                        Err(e) => app.say(e),
                    }
                } else {
                    match actions::parse_artifacts(body) {
                        Ok(v) => d.artifacts = Some(v),
                        Err(e) => app.say(e),
                    }
                }
            }
            k if k.starts_with("a-yaml:") => {
                let id: u64 = k[7..].parse().unwrap_or(0);
                let d = app.run_detail.get_or_insert(crate::app::RunDetail { id, ..Default::default() });
                if d.id != id {
                    *d = crate::app::RunDetail { id, ..Default::default() };
                }
                // A workflow that cannot be read just has no graph.
                if let Ok(text) = workflow::parse_contents(body) {
                    d.workflow = Some(workflow::parse(&text));
                }
            }
            k if k.starts_with("a-repo:") => {
                if let Some(Overlay::Log(l)) = app.overlay.as_mut() {
                    if l.job_id == 0 && l.name.starts_with(&k[7..]) {
                        l.lines = Some(minato_core::reposettings::lines(body).unwrap_or_else(|e| vec![e]));
                    }
                }
            }
            k if k.starts_with("a-log:") => {
                let id: u64 = k[6..].parse().unwrap_or(0);
                if let Some(Overlay::Log(l)) = app.overlay.as_mut() {
                    if l.job_id == id {
                        l.scroll = 0;
                        l.lines = Some(workflow::log_tail(body, 400));
                    }
                }
            }
            k if k.starts_with("write:") => {
                if body.contains("\"errors\"") {
                    app.say(body.chars().take(160).collect::<String>());
                } else if let Some(done) = self.pending.remove(k) {
                    let words = app.tr(done);
                    app.say(words);
                }
                // Look again at once, and re-ask the detail (not from the cache).
                self.next_search = Instant::now();
                self.next_runs = Instant::now();
                self.asked = None;
                self.refetch = true;
                self.asked_run = None;
            }
            _ => {}
        }
    }

    pub fn pump(&mut self, app: &mut App, ctx: &egui::Context) {
        if self.worker.is_none() {
            match self.token_rx.try_recv() {
                Ok(Some((token, _))) => {
                    let wake = ctx.clone();
                    let worker = Worker::spawn(Client::new(UreqTransport::new(), token), move || wake.request_repaint());
                    worker.send(get("me", "/user"));
                    worker.send(get("rate", "/rate_limit"));
                    self.worker = Some(worker);
                    self.next_poll = Instant::now() + POLL;
                }
                Ok(None) => {
                    let words = app.tr("net.no_token");
                    app.say(words);
                    // Do not look again; a restart does.
                    let (_, rx) = mpsc::channel();
                    self.token_rx = rx;
                }
                Err(_) => {}
            }
        }
        self.load_cache(app);
        let Some(worker) = &self.worker else { return };
        let replies: Vec<_> = std::iter::from_fn(|| worker.try_recv()).collect();
        for reply in replies {
            match reply.result {
                Ok(f) => {
                    app.synced = Some(Instant::now());
                    if let Some(r) = f.rate {
                        self.rate = Some(r);
                    }
                    let key = reply.key.clone();
                    self.live.insert(key.clone());
                    if cached(&key, self.searched_custom.is_some() || self.searched_closed) && !f.body.contains("\"errors\"") {
                        self.keep(&key, "", &f.body);
                    }
                    self.take(app, &key, &f.body, true);
                }
                Err(e) => {
                    self.pending.remove(&reply.key);
                    if let Some(id) = reply.key.strip_prefix("a-log:").and_then(|n| n.parse::<u64>().ok()) {
                        if let Some(Overlay::Log(l)) = app.overlay.as_mut() {
                            if l.job_id == id {
                                l.lines = Some(vec![e.clone()]);
                            }
                        }
                    }
                    if reply.key.starts_with("board:") {
                        self.next_board = Instant::now();
                    }
                    if reply.key.starts_with("note:") {
                        self.next_notes = Instant::now();
                    }
                    app.say(e);
                }
            }
        }
        let Some(worker) = &self.worker else { return };
        for o in app.outbox.drain(..) {
            if let Some(done) = o.done {
                self.pending.insert(o.key.clone(), done);
            }
            worker.send(Job { key: o.key, call: o.call });
        }
        app.api = self.rate.map(|r| (r.remaining, r.limit));
        let now = Instant::now();
        if now >= self.next_poll {
            worker.send(get("rate", "/rate_limit"));
            let unix = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let wait = self.rate.map_or(POLL, |r| r.interval(POLL, unix));
            self.next_poll = now + wait;
        }
        if let Some(me) = app.viewer.clone() {
            let custom = if app.view == crate::app::View::List { app.list.active_search().map(|t| minato_core::list::search_query(t, &me)) } else { None };
            if app.show_closed != self.searched_closed || custom != self.searched_custom {
                self.searched_closed = app.show_closed;
                self.searched_custom = custom.clone();
                self.next_search = now;
            }
            if now >= self.next_search {
                let state = if app.show_closed { "is:closed" } else { "is:open" };
                let q = format!("user:{me} {state} sort:updated-desc");
                let vars = format!("{{\"q\":{},\"n\":50}}", json_str(&q));
                worker.send(Job { key: "items".into(), call: Call::Graphql { query: items_query(), variables: vars.clone() } });
                if let Some(q) = custom {
                    let cvars = format!("{{\"q\":{},\"n\":50}}", json_str(&q));
                    worker.send(Job { key: "items".into(), call: Call::Graphql { query: items_query(), variables: cvars } });
                    self.found[1].clear();
                    self.mentioned.clear();
                    self.stubs.clear();
                } else if app.show_closed {
                    self.found[1].clear();
                } else {
                    let q = format!("user:{me} is:open mentions:{me} -author:{me} sort:updated-desc");
                    let mvars = format!("{{\"q\":{},\"n\":50}}", json_str(&q));
                    worker.send(Job { key: "mentions".into(), call: Call::Graphql { query: items_query(), variables: mvars } });
                    worker.send(Job { key: "mainci".into(), call: Call::Graphql { query: mainci::query(), variables: "{}".into() } });
                    worker.send(Job { key: "discussions".into(), call: Call::Graphql { query: discussions_query(), variables: vars } });
                }
                self.next_search = now + SEARCH;
            }
        }
        if app.viewer.is_some() && now >= self.next_notes {
            worker.send(get("notes", notes::PATH));
            self.next_notes = now + NOTES;
        }
        if app.viewer.is_some() && app.view == crate::app::View::Board && now >= self.next_board {
            worker.send(Job { key: "board".into(), call: Call::Graphql { query: board::query(), variables: "{}".into() } });
            self.next_board = now + BOARD;
        }
        if app.viewer.is_some() && app.view == crate::app::View::Actions {
            if self.repos.is_empty() {
                if now >= self.next_runs {
                    worker.send(get("a-repos", actions::repos_path()));
                    self.next_runs = now + RUNS;
                }
            } else if now >= self.next_runs {
                for r in &self.repos {
                    worker.send(Job { key: format!("a-runs:{r}"), call: Call::Get(actions::runs_path(r)) });
                }
                self.next_runs = now + RUNS;
                self.asked_run = None;
            }
            let run = app.run().cloned();
            let want = run.as_ref().map(|r| r.id);
            if want != self.asked_run {
                if let Some(r) = &run {
                    worker.send(Job { key: format!("a-jobs:{}", r.id), call: Call::Get(actions::jobs_path(r)) });
                    worker.send(Job { key: format!("a-arts:{}", r.id), call: Call::Get(actions::artifacts_path(r)) });
                    if let Some(path) = workflow::contents_path(r) {
                        worker.send(Job { key: format!("a-yaml:{}", r.id), call: Call::Get(path) });
                    }
                }
                self.asked_run = want;
            }
        }
        // A stub for a repository (red main CI) has no thread to read. A
        // detail kept from when the item was last updated is shown and not
        // asked again; an older one is shown while the new one comes.
        let want = app.selected().filter(|i| !minato_core::mainci::is_stub(i)).map(|i| (i.id.clone(), i.updated_at.clone()));
        if want != self.asked {
            if let Some((id, stamp)) = &want {
                let kept = self.kept.get(id).and_then(|(s, b)| parse_detail(b).ok().map(|d| (s == stamp && !stamp.is_empty(), d)));
                let fresh = kept.as_ref().is_some_and(|(same, _)| *same) && !self.refetch;
                if let Some((_, d)) = kept {
                    if app.detail.as_ref().is_none_or(|(i, _)| i != id) || fresh {
                        app.detail = Some((id.clone(), d));
                    }
                }
                if !fresh {
                    let vars = format!("{{\"id\":{}}}", json_str(id));
                    worker.send(Job { key: format!("detail:{id}"), call: Call::Graphql { query: detail_query().into(), variables: vars } });
                }
            }
            self.asked = want;
            self.refetch = false;
        }
        let wake = self.next_poll.min(self.next_search).min(self.next_notes).min(match app.view {
            crate::app::View::Actions => self.next_runs,
            crate::app::View::Board => self.next_board,
            _ => self.next_search,
        });
        ctx.request_repaint_after(wake.saturating_duration_since(now));
    }
}

/// Whether the answer to `key` is kept (a detail is kept apart, with its
/// stamp). `narrowed`: the items are of a saved search or the closed ones,
/// which a start would not show.
fn cached(key: &str, narrowed: bool) -> bool {
    match key {
        "items" => !narrowed,
        "me" | "discussions" | "mentions" | "mainci" | "notes" | "board" | "a-repos" => true,
        k => k.starts_with("a-runs:"),
    }
}

/// A thread that writes each answer it is sent to the cache in `dir`.
fn cache_writer(dir: PathBuf) -> mpsc::Sender<cache::Entry> {
    let (tx, rx) = mpsc::channel::<cache::Entry>();
    std::thread::spawn(move || {
        let _ = std::fs::create_dir_all(&dir);
        while let Ok(e) = rx.recv() {
            let _ = ito_common::write_atomic(&cache::path(&dir, &e.key), cache::encode(&e));
        }
    });
    tx
}

fn get(key: &str, path: &str) -> Job {
    Job { key: key.into(), call: Call::Get(path.into()) }
}

/// A string as a JSON literal.
fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
