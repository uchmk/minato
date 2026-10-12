//! The link between the window and `minato-core`'s GitHub worker: find the
//! token off the UI thread, ask who it is, and keep the rate limit fresh.
//! Each frame `pump` takes what has arrived; it never waits. The answers
//! are kept on disk (`minato_core::cache`), so a start shows the last ones
//! at once and a detail whose item has not moved is not asked again.
//!
//! Two workers, each one request at a time: `worker` for the polls (the
//! searches, the board, the runs; those of the open view go first), `quick`
//! for what a key asks for (a detail, a run's jobs, a log, a write), so it
//! never waits behind a poll.
//!
//! A run's jobs, artifacts and workflow are kept by run with its
//! `updated_at`: a finished run's are shown from there (and from disk after
//! a restart) and not asked again; a running or queued one's are asked with
//! each refresh of the runs.

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

use crate::app::{App, RunDetail};
use crate::overlay::Overlay;

/// How often the rate limit is asked while plenty is left.
const POLL: Duration = Duration::from_secs(60);

type Found = Option<(Token, Source)>;

pub struct Net {
    token_rx: Receiver<Found>,
    worker: Option<Worker>,
    quick: Option<Worker>,
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
    /// The number of the Project the board shows, once `board-pick` has said.
    board_no: Option<u64>,
    next_notes: Instant,
    /// The run whose jobs and artifacts were last asked for, and its `updated_at`.
    asked_run: Option<(u64, String)>,
    /// After a run's jobs failed to come, when to ask again.
    run_retry: Instant,
    /// What GitHub said about the runs looked at, by run id.
    run_kept: HashMap<u64, RunMemo>,
    /// Ask the selected run again even if it is kept (after a write).
    run_refetch: bool,
    /// The board's pages come so far (the board changes once all have).
    board_pages: Vec<String>,
    /// The card whose body was last asked for.
    peek_asked: Option<String>,
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

/// How long to wait before asking again for a run's jobs that failed.
const RETRY: Duration = Duration::from_secs(10);

/// At most so many pages of 100 cards are read for the board.
const PAGES: usize = 5;

/// What GitHub said about one run, and the run's `updated_at` then.
struct RunMemo {
    stamp: String,
    detail: RunDetail,
    /// Whether the jobs, the artifacts and the workflow have come since
    /// (the workflow is of the run's commit, so it stays).
    got: [bool; 3],
}

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
        let now = Instant::now();
        Net { token_rx, worker: None, quick: None, next_poll: now, rate: None, next_search: now, searched_closed: false, searched_custom: None, found: [Vec::new(), Vec::new()], mentioned: Default::default(), stubs: Vec::new(), asked: None, refetch: false, pending: Default::default(), repos: Vec::new(), run_found: Default::default(), next_runs: now, next_board: now, board_no: None, next_notes: now, asked_run: None, run_retry: now, run_kept: Default::default(), run_refetch: false, board_pages: Vec::new(), peek_asked: None, cache_rx, cache_tx, kept: Default::default(), live: Default::default() }
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
            } else if self.live.contains(&e.key) {
                continue;
            } else if run_part(&e.key) {
                self.take_run(app, &e.key, &e.body, Some(&e.stamp));
            } else if cached(&e.key, false) || e.key == "board" {
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
            "board-pick" => match board::pick(body) {
                Ok(n) => {
                    if live && self.board_no != Some(n) {
                        self.next_board = Instant::now();
                    }
                    self.board_no = Some(n);
                }
                Err(e) => app.say(e),
            },
            k @ ("board" | "board-more") if live => self.board_page(app, k, body),
            "board" => self.show_board(app, body, false),
            k if k.starts_with("peek:") => {
                app.peeks.insert(k["peek:".len()..].to_string(), board::parse_peek(body));
            }
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
            k if run_part(k) => self.take_run(app, k, body, None),
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
                self.run_refetch = true;
            }
            _ => {}
        }
    }

    /// The board from an answer (all its pages merged, or kept).
    fn show_board(&mut self, app: &mut App, body: &str, live: bool) {
        match board::parse(body) {
            Ok(p) => {
                app.bs.clamp(&p);
                app.board = Some(p);
            }
            Err(e) => {
                // The Project may be gone or renumbered: pick again.
                if live {
                    self.board_no = None;
                }
                app.say(e);
            }
        }
    }

    /// One page of the board from GitHub: the next is asked for, and once
    /// the last has come the board shows them all (and they are kept).
    fn board_page(&mut self, app: &mut App, key: &str, body: &str) {
        if key == "board" {
            self.board_pages.clear();
        } else if self.board_pages.is_empty() {
            return;
        }
        self.board_pages.push(body.to_string());
        let next = board::next_page(body).filter(|_| self.board_pages.len() < PAGES);
        if let (Some(after), Some(n), Some(w)) = (next, self.board_no, &self.worker) {
            w.send_first(Job { key: "board-more".into(), call: Call::Graphql { query: board::query(), variables: board::vars(n, Some(&after)) } });
            return;
        }
        let pages = std::mem::take(&mut self.board_pages);
        let all = board::merge(&pages).unwrap_or_else(|_| body.to_string());
        if !all.contains("\"errors\"") {
            self.keep("board", "", &all);
        }
        self.show_board(app, &all, true);
    }

    /// A run's jobs, artifacts or workflow (by `key`), from GitHub (`kept`
    /// is `None`; the stamp is the run's `updated_at` now) or from the cache
    /// with its stamp. Kept by run; shown only while the run is selected. A
    /// finished run's answers go to disk.
    fn take_run(&mut self, app: &mut App, key: &str, body: &str, kept: Option<&str>) {
        let Some((part, id)) = key.split_once(':') else { return };
        let Ok(id) = id.parse::<u64>() else { return };
        let run = app.runs.iter().find(|r| r.id == id);
        let done = run.is_some_and(|r| !r.status.active());
        let stamp = match kept {
            Some(s) => s.to_string(),
            None => run.map(|r| r.updated_at.clone()).unwrap_or_default(),
        };
        if kept.is_none() && done && !stamp.is_empty() {
            self.keep(key, &stamp, body);
        }
        let m = self.run_kept.entry(id).or_insert_with(|| RunMemo { stamp: stamp.clone(), detail: RunDetail { id, ..Default::default() }, got: [false; 3] });
        // A newer state of the run: what is kept shows until the new comes.
        if m.stamp != stamp {
            m.stamp = stamp;
            m.got[0] = false;
            m.got[1] = false;
        }
        match part {
            "a-jobs" => match actions::parse_jobs(body) {
                Ok(v) => (m.detail.jobs, m.got[0]) = (Some(v), true),
                Err(e) => app.say(e),
            },
            "a-arts" => match actions::parse_artifacts(body) {
                Ok(v) => (m.detail.artifacts, m.got[1]) = (Some(v), true),
                Err(e) => app.say(e),
            },
            _ => {
                // A workflow that cannot be read just has no graph.
                if let Ok(text) = workflow::parse_contents(body) {
                    m.detail.workflow = Some(workflow::parse(&text));
                }
                m.got[2] = true;
            }
        }
        if app.run().is_some_and(|r| r.id == id) {
            app.run_detail = Some(m.detail.clone());
        }
    }

    pub fn pump(&mut self, app: &mut App, ctx: &egui::Context) {
        if self.worker.is_none() {
            match self.token_rx.try_recv() {
                Ok(Some((token, _))) => {
                    let (wake, wake2) = (ctx.clone(), ctx.clone());
                    self.quick = Some(Worker::spawn(Client::new(UreqTransport::new(), token.clone()), move || wake2.request_repaint()));
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
        let (Some(worker), Some(quick)) = (&self.worker, &self.quick) else { return };
        let replies: Vec<_> = std::iter::from_fn(|| worker.try_recv().or_else(|| quick.try_recv())).collect();
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
                    // A run's jobs that did not come are asked again soon,
                    // not left "loading" until the next refresh.
                    if reply.key.starts_with("a-jobs:") {
                        self.asked_run = None;
                        self.run_retry = Instant::now() + RETRY;
                    }
                    app.say(e);
                }
            }
        }
        let (Some(worker), Some(quick)) = (&self.worker, &self.quick) else { return };
        for o in app.outbox.drain(..) {
            if let Some(done) = o.done {
                self.pending.insert(o.key.clone(), done);
            }
            quick.send(Job { key: o.key, call: o.call });
        }
        // The body of the card under the mouse; only the last one waits.
        if let Some(id) = app.peek_wanted.take() {
            if self.peek_asked.as_ref() != Some(&id) && !app.peeks.contains_key(&id) {
                let vars = format!("{{\"id\":{}}}", json_str(&id));
                quick.send_in("peek", &id, Job { key: format!("peek:{id}"), call: Call::Graphql { query: board::peek_query().into(), variables: vars } });
                self.peek_asked = Some(id);
            }
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
            match self.board_no {
                None => worker.send_first(Job { key: "board-pick".into(), call: Call::Graphql { query: board::pick_query().into(), variables: "{}".into() } }),
                Some(n) => worker.send_first(Job { key: "board".into(), call: Call::Graphql { query: board::query(), variables: board::vars(n, None) } }),
            }
            self.next_board = now + BOARD;
        }
        if app.viewer.is_some() && app.view == crate::app::View::Actions {
            if self.repos.is_empty() {
                if now >= self.next_runs {
                    worker.send_first(get("a-repos", actions::repos_path()));
                    self.next_runs = now + RUNS;
                }
            } else if now >= self.next_runs {
                for r in &self.repos {
                    worker.send_first(Job { key: format!("a-runs:{r}"), call: Call::Get(actions::runs_path(r)) });
                }
                self.next_runs = now + RUNS;
                // A running or queued run's jobs are asked again with the runs.
                if app.run().is_some_and(|r| r.status.active()) {
                    self.asked_run = None;
                }
            }
            // The run selected: what is kept shows at once. A finished run's
            // parts kept since it finished are not asked again; the rest are,
            // and those of a run passed over (`j` `k`) are dropped unasked.
            let run = app.run().cloned();
            let want = run.as_ref().map(|r| (r.id, r.updated_at.clone()));
            if want != self.asked_run && now >= self.run_retry {
                if let Some(r) = &run {
                    let memo = self.run_kept.get(&r.id);
                    if let Some(m) = memo {
                        app.run_detail = Some(m.detail.clone());
                    }
                    let same = !self.run_refetch && memo.is_some_and(|m| m.stamp == r.updated_at && !r.status.active());
                    let got = |k: usize| !self.run_refetch && memo.is_some_and(|m| m.got[k]);
                    let tag = r.id.to_string();
                    if !(same && got(0)) {
                        quick.send_in("run", &tag, Job { key: format!("a-jobs:{}", r.id), call: Call::Get(actions::jobs_path(r)) });
                    }
                    if !(same && got(1)) {
                        quick.send_in("run", &tag, Job { key: format!("a-arts:{}", r.id), call: Call::Get(actions::artifacts_path(r)) });
                    }
                    if let Some(path) = workflow::contents_path(r).filter(|_| !got(2)) {
                        quick.send_in("run", &tag, Job { key: format!("a-yaml:{}", r.id), call: Call::Get(path) });
                    }
                }
                self.asked_run = want;
                self.run_refetch = false;
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
                    quick.send(Job { key: format!("detail:{id}"), call: Call::Graphql { query: detail_query().into(), variables: vars } });
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
        let wake = if self.run_retry > now { wake.min(self.run_retry) } else { wake };
        ctx.request_repaint_after(wake.saturating_duration_since(now));
    }
}

/// Whether the answer to `key` is kept as it comes (a detail and a run's
/// parts are kept apart, with their stamp; the board once all its pages
/// have come). `narrowed`: the items are of a saved search or the closed
/// ones, which a start would not show.
fn cached(key: &str, narrowed: bool) -> bool {
    match key {
        "items" => !narrowed,
        "me" | "discussions" | "mentions" | "mainci" | "notes" | "board-pick" | "a-repos" => true,
        k => k.starts_with("a-runs:"),
    }
}

/// Whether `key` is for a run's jobs, artifacts or workflow.
fn run_part(key: &str) -> bool {
    key.starts_with("a-jobs:") || key.starts_with("a-arts:") || key.starts_with("a-yaml:")
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
