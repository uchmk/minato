//! The link between the window and `minato-core`'s GitHub worker: find the
//! token off the UI thread, ask who it is, and keep the rate limit fresh.
//! Each frame `pump` takes what has arrived; it never waits. The answers
//! are kept on disk (`minato_core::cache`), so a start shows the last ones
//! at once and a detail whose item has not moved is not asked again.
//!
//! Three workers, each one request at a time: `worker` for the polls (the
//! searches, the board, the runs; those of the open view go first, the
//! others' less often), `quick` for what a key asks for (a detail, a run's
//! jobs, a log, a write), so it never waits behind a poll, and `back` for
//! reading the finished runs' jobs ahead. The repositories and the runs of
//! each are asked on `runs`, a few at once, so Actions does not wait for
//! them one by one or behind a search. They share one ETag cache, filled from disk at a start, so an
//! answer that has not changed costs a 304.
//!
//! F5 starts a round: everything shown is asked at once (with the ETags,
//! so what has not changed is a free 304), and the keybar counts the
//! answers of the round as they come.
//!
//! A run's jobs, artifacts, workflow and release are kept by run with its
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
use minato_core::github::search::{self, detail_query, discussions_query, items_query, parse_detail, parse_item, parse_items, search_vars};
use minato_core::github::token::Source;
use minato_core::github::types::{parse_rate_limit, parse_viewer};
use minato_core::github::worker::{self, Call, Job, Worker};
use minato_core::github::write::{self, parse_names};
use minato_core::github::{client, Bucket, Client, Etags, Rate, Token, UreqTransport};
use minato_core::notes;
use minato_core::overview;
use minato_core::security;
use minato_core::workflow;

use minato_core::list::Item;
use minato_core::mainci;

use crate::app::{App, RunDetail, View};
use crate::overlay::Overlay;

/// How often the rate limit is asked while plenty is left.
const POLL: Duration = Duration::from_secs(60);

/// How many repositories' runs are asked at once.
const RUN_LANES: usize = 4;

type Found = Option<(Token, Source)>;

/// The open items' answers: the two searches', the mentioned ids and the stubs.
type Answers = ([Vec<Item>; 2], HashSet<String>, Vec<Item>);

pub struct Net {
    token_rx: Receiver<Found>,
    worker: Option<Worker>,
    quick: Option<Worker>,
    back: Option<Worker>,
    /// The workers the runs of the repositories are spread over.
    runs: Vec<Worker>,
    /// The ETags and bodies the workers' GETs were last answered with.
    etags: Etags,
    /// Jobs sent to `back` and not answered yet.
    back_busy: usize,
    /// The runs (id and `updated_at`) `back` has been asked about this start.
    back_tried: HashSet<(u64, String)>,
    next_poll: Instant,
    /// The calls left in REST's limit and in GraphQL's: they are counted
    /// apart, and the tighter one is shown and paces the polls.
    rates: [Option<Rate>; 2],
    next_search: Instant,
    /// Whether the last search was for closed items.
    searched_closed: bool,
    /// The last answers of the two searches, merged into `app.items`.
    found: [Vec<Item>; 2],
    /// The same searches of each owner shown alongside (searched apart, so
    /// the first owner's items are never crowded out), by login; and the
    /// logins they were last asked for.
    extra: HashMap<String, [Vec<Item>; 2]>,
    extra_for: Vec<String>,
    /// The saved search the items were last searched with (the list view's).
    searched_custom: Option<String>,
    /// The open items' answers (`found`, `mentioned`, `stubs`) while the
    /// list shows closed items or a saved search's.
    stash: Option<Answers>,
    /// Ids the `mentions:` search found, and the repositories whose main CI is red.
    mentioned: std::collections::HashSet<String>,
    stubs: Vec<Item>,
    /// The detail last asked for (its node id and the item's `updatedAt`).
    asked: Option<(String, String)>,
    /// Ask the next detail from GitHub even if the cache has it (after a write).
    refetch: bool,
    /// What to say when a write (by its job key) has gone through.
    pending: std::collections::HashMap<String, &'static str>,
    /// The repositories whose runs are asked for (none until GitHub says):
    /// those of each owner shown, the first owner's first.
    repos: Vec<String>,
    /// Each owner's repositories as GitHub said, by login ("" for the
    /// first owner, the viewer when the settings name none).
    repo_sets: HashMap<String, Vec<String>>,
    /// The last runs of each repository.
    run_found: std::collections::HashMap<String, Vec<Run>>,
    /// When the runs and the board were last asked for (`None`: ask now).
    runs_asked: Option<Instant>,
    board_asked: Option<Instant>,
    /// Whether the runs and the board were last asked behind the searches
    /// (another view was open).
    runs_away: bool,
    board_away: bool,
    /// The number of the Project the board shows, once `board-pick` has said.
    board_no: Option<u64>,
    next_notes: Instant,
    /// The run whose jobs and artifacts were last asked for, and its `updated_at`.
    asked_run: Option<(u64, String)>,
    /// After a run's jobs failed to come, when to ask again.
    run_retry: Instant,
    /// What GitHub said about the runs looked at, by run id.
    run_kept: HashMap<u64, RunMemo>,
    /// The workflows read, by the path they were read from (a file at a
    /// commit, the same for the runs of one commit).
    yaml_by: HashMap<String, workflow::Workflow>,
    /// Ask the selected run again even if it is kept (after a write).
    run_refetch: bool,
    /// The board's pages come so far (the board changes once all have).
    board_pages: Vec<String>,
    /// The query each search ("items", "discussions") was last sent with,
    /// and the pages of its answer come so far (the items change once all
    /// have). The next page's key names the round it is of.
    searched_q: HashMap<&'static str, String>,
    search_pages: HashMap<&'static str, (u64, Vec<String>)>,
    /// The query whose answer, all its pages, the items are of.
    shown_q: HashMap<&'static str, String>,
    search_round: u64,
    /// The card whose body was last asked for.
    peek_asked: Option<String>,
    /// The kept answers, until the thread has read them; where to write.
    cache_rx: Option<Receiver<Vec<cache::Entry>>>,
    cache_tx: Option<mpsc::Sender<cache::Entry>>,
    /// The kept details by node id: the item's `updatedAt` then and the body.
    kept: HashMap<String, (String, String)>,
    /// The keys GitHub has answered this run (the cache does not undo them).
    live: HashSet<String>,
    /// The settings' owner (empty: the viewer) and board Project the polls
    /// follow; `None` until the first frame.
    scope: Option<(String, String)>,
    /// The sync F5 asked for, while its answers come.
    round: Option<Round>,
    /// When the status view last asked for the releases, and the tag each
    /// repository's commits since were last asked against.
    status_asked: Option<Instant>,
    cmp_asked: HashMap<String, String>,
    /// When the security alerts were last asked for, the alerts each
    /// ("word:repo") answered with, and the ones that could not be read.
    sec_asked: Option<Instant>,
    sec_found: HashMap<String, Vec<security::Alert>>,
    sec_failed: HashSet<String>,
}

/// A sync asked for by hand: the keys asked in it and not answered yet,
/// how many were asked, and when it started.
struct Round {
    want: HashSet<String>,
    asked: usize,
    since: Instant,
}

/// Count `key` in the round, if one runs (a free function: the workers
/// are borrowed from the same `Net` while jobs are sent).
fn track(round: &mut Option<Round>, key: &str) {
    if let Some(r) = round {
        if r.want.insert(key.to_string()) {
            r.asked += 1;
        }
    }
}

/// A round that has not ended by then ends anyway (an answer lost to a
/// newer question with the same key never comes).
const ROUND_MAX: Duration = Duration::from_secs(90);

/// How often the runs are asked again, while the Actions view is open, and
/// while it is not (so it opens on fresh runs).
const RUNS: Duration = Duration::from_secs(45);
const RUNS_AWAY: Duration = Duration::from_secs(180);

/// How often the board is asked again, while it is open and while not.
const BOARD: Duration = Duration::from_secs(60);
const BOARD_AWAY: Duration = Duration::from_secs(300);

/// Below so many calls left, the runs' jobs are not read ahead.
const BACK_FLOOR: u32 = 1000;

/// How often the notifications are asked again.
const NOTES: Duration = Duration::from_secs(90);

/// How many repositories' runs are shown, of each owner.
const REPOS: usize = 8;

/// How often the status view asks for the releases again while it is open.
const STATUS: Duration = Duration::from_secs(300);

/// How often the security alerts are asked again, while their view is open
/// and while it is not (the inbox shows the critical and high ones).
const SECURITY: Duration = Duration::from_secs(300);
const SECURITY_AWAY: Duration = Duration::from_secs(900);

/// How often the lists are searched again.
const SEARCH: Duration = Duration::from_secs(120);

/// How long to wait before asking again for a run's jobs that failed.
const RETRY: Duration = Duration::from_secs(10);

/// At most so many pages of 100 cards are read for the board.
const PAGES: usize = 5;

/// At most so many pages of 100 items are read for a search (GitHub gives
/// no more than 1000).
const SEARCH_PAGES: usize = 10;

/// What GitHub said about one run, and the run's `updated_at` then.
struct RunMemo {
    stamp: String,
    detail: RunDetail,
    /// Whether the jobs, the artifacts, the workflow and the release have
    /// come since (the workflow is of the run's commit, so it stays).
    got: [bool; 4],
}

impl Net {
    /// Merge what the searches found into `app.items`: mentions marked, the
    /// items fetched for notifications added (until a search lists them),
    /// stubs for red main branches added (not over closed items).
    fn rebuild(&self, app: &mut App) {
        let mut all = minato_core::list::join_found(self.found.concat(), self.extra.values().flatten().flatten());
        app.fetched.retain(|f| !all.iter().any(|i| i.id == f.id));
        all.extend(app.fetched.iter().cloned());
        for i in &mut all {
            i.mentioned = self.mentioned.contains(&i.id);
        }
        all.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        if !self.searched_closed {
            all.extend(self.stubs.iter().cloned());
            all.extend(security::stubs(&app.alerts));
        }
        let card = app.inbox.selected(&app.inbox_all(), &app.items).map(|c| app.items[c.at].id.clone());
        let before = std::mem::replace(&mut app.items, all);
        app.list.follow(&before, &app.items);
        app.list.stay(&before, &app.items, app.viewer.as_deref().unwrap_or(""));
        // The inbox's card stays on its item too (cards point into the items).
        if let Some(id) = card {
            let all = app.inbox_all();
            if let Some(k) = app.inbox.shown(&all, &app.items).iter().position(|c| app.items[c.at].id == id) {
                app.inbox.sel = k;
            }
        }
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
        Net {
            token_rx,
            worker: None,
            quick: None,
            back: None,
            runs: Vec::new(),
            etags: Etags::default(),
            back_busy: 0,
            back_tried: HashSet::new(),
            next_poll: now,
            rates: [None; 2],
            next_search: now,
            searched_closed: false,
            searched_custom: None,
            stash: None,
            found: [Vec::new(), Vec::new()],
            extra: HashMap::new(),
            extra_for: Vec::new(),
            mentioned: Default::default(),
            stubs: Vec::new(),
            asked: None,
            refetch: false,
            pending: Default::default(),
            repos: Vec::new(),
            repo_sets: HashMap::new(),
            run_found: Default::default(),
            runs_asked: None,
            board_asked: None,
            runs_away: false,
            board_away: false,
            board_no: None,
            next_notes: now,
            asked_run: None,
            run_retry: now,
            run_kept: Default::default(),
            yaml_by: HashMap::new(),
            run_refetch: false,
            board_pages: Vec::new(),
            searched_q: HashMap::new(),
            search_pages: HashMap::new(),
            shown_q: HashMap::new(),
            search_round: 0,
            peek_asked: None,
            cache_rx,
            cache_tx,
            kept: Default::default(),
            live: Default::default(),
            scope: None,
            round: None,
            status_asked: None,
            sec_asked: None,
            sec_found: HashMap::new(),
            sec_failed: HashSet::new(),
            cmp_asked: HashMap::new(),
        }
    }

    /// The kept answers, once the thread has read them: each one GitHub has
    /// not answered yet this run is taken as if it had just come; the
    /// details wait until their item is selected.
    fn load_cache(&mut self, app: &mut App) {
        let Some(rx) = &self.cache_rx else { return };
        let Ok(entries) = rx.try_recv() else { return };
        self.cache_rx = None;
        // What was kept for another owner or Project is not shown; kept
        // with no mark, it is of the viewer and "uchmk".
        let line = scope_line(self.scope.as_ref());
        let kept = entries.iter().find(|e| e.key == "scope").map_or_else(|| scope_line(None), |e| e.body.clone());
        let same = kept == line;
        if !same {
            self.keep("scope", "", &line);
        }
        let other = self.other().map(str::to_string);
        for e in entries {
            // A GET kept with its ETag: the first ask may be a 304.
            if let Some(path) = rest_path(&e.key, other.as_deref()).filter(|_| !e.stamp.is_empty()) {
                self.etags
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .entry(client::url(&path))
                    .or_insert_with(|| (e.stamp.clone(), e.body.clone()));
            }
            if !same && scoped(&e.key) {
                continue;
            }
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

    /// A list for the writing box did not come: asked again on the next
    /// look; said only when the box is open on its repository (it may have
    /// been read ahead).
    fn list_failed(&self, app: &mut App, repo: &str, e: String) {
        app.lists_failed(repo);
        if app.composing_in(repo) {
            app.say(e);
        }
    }

    /// The owner the settings name, when it is not the viewer.
    fn other(&self) -> Option<&str> {
        self.scope.as_ref().and_then(|s| s.0.split_whitespace().next())
    }

    /// The owners the settings show alongside the first.
    fn also(&self) -> Vec<String> {
        self.scope.as_ref().map(|s| s.0.split_whitespace().skip(1).map(str::to_string).collect()).unwrap_or_default()
    }

    /// One owner's repositories have come (`owner`: "" for the first): the
    /// runs are of all those known, the first owner's first, and a run of a
    /// repository no longer among them goes once every owner's have come.
    fn set_repos(&mut self, app: &mut App, owner: String, v: Vec<String>) {
        self.repo_sets.insert(owner, v.into_iter().take(REPOS).collect());
        let keys: Vec<String> = std::iter::once(String::new()).chain(self.also()).collect();
        let mut repos: Vec<String> = Vec::new();
        for r in keys.iter().filter_map(|k| self.repo_sets.get(k)).flatten() {
            if !repos.contains(r) {
                repos.push(r.clone());
            }
        }
        self.repos = repos;
        if keys.iter().all(|k| self.repo_sets.contains_key(k)) {
            let repos = &self.repos;
            self.run_found.retain(|r, _| repos.contains(r));
        }
        app.status_repos = self.repos.clone();
        self.status_asked = None;
        self.sec_asked = None;
        self.show_runs(app);
        self.runs_asked = None;
    }

    /// F5: ask for everything shown now. The polls come due at once, the
    /// repositories are asked again, and the open item and run are read
    /// again past what is kept; a round counts their answers.
    fn start_round(&mut self, app: &mut App) {
        let now = Instant::now();
        (self.next_poll, self.next_search, self.next_notes, self.run_retry) = (now, now, now, now);
        (self.board_asked, self.runs_asked) = (None, None);
        self.repos.clear();
        self.repo_sets.clear();
        (self.status_asked, self.cmp_asked) = (None, HashMap::new());
        self.sec_asked = None;
        (self.asked, self.refetch) = (None, true);
        (self.asked_run, self.run_refetch) = (None, true);
        self.round = Some(Round { want: HashSet::new(), asked: 0, since: now });
        app.syncing = Some((0, 0));
    }

    /// The round's count for the keybar, and its end once all is answered
    /// (or it has run too long).
    fn end_round(&mut self, app: &mut App, now: Instant) {
        let Some(r) = &self.round else { return };
        app.syncing = Some((r.asked - r.want.len(), r.asked));
        if r.want.is_empty() || now.duration_since(r.since) >= ROUND_MAX {
            self.round = None;
            app.syncing = None;
            let words = app.tr("net.synced");
            app.say(words);
        }
    }

    /// Follow the settings' owner and board Project: when one changes, what
    /// was found for the old is forgotten and asked for again at once.
    fn rescope(&mut self, app: &mut App) {
        // The first owner, then those shown alongside, apart by spaces.
        let owners = std::iter::once(&app.settings.owner).chain(&app.settings.also).filter(|o| !o.is_empty());
        let want = (owners.cloned().collect::<Vec<_>>().join(" "), app.settings.project.clone());
        let Some(old) = self.scope.replace(want.clone()) else { return };
        if old == want {
            return;
        }
        self.keep("scope", "", &scope_line(Some(&want)));
        if old.0 != want.0 {
            self.found = Default::default();
            self.extra.clear();
            self.search_pages.clear();
            self.stash = None;
            self.mentioned.clear();
            self.stubs.clear();
            self.rebuild(app);
            self.next_search = Instant::now();
            self.repos.clear();
            self.repo_sets.clear();
            app.status_repos.clear();
            self.status_asked = None;
            (self.sec_asked, self.sec_found, self.sec_failed) = (None, HashMap::new(), HashSet::new());
            app.alerts.clear();
            app.sec_loaded = false;
            app.sec_unreadable = 0;
            self.run_found.clear();
            self.show_runs(app);
            self.runs_asked = None;
            self.asked_run = None;
            app.run_detail = None;
        }
        self.board_no = None;
        self.board_asked = None;
        self.board_pages.clear();
        app.board = None;
    }

    /// The runs found of the repositories looked at, newest first.
    fn show_runs(&self, app: &mut App) {
        let mut all: Vec<Run> = self.run_found.values().flatten().cloned().collect();
        all.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        let before = std::mem::replace(&mut app.runs, all);
        // While the first answers come in, a cursor at the top stays there:
        // the runs found later are often newer.
        if self.run_found.len() < self.repos.len() && app.acts.sel == 0 {
            app.acts.clamp(&app.runs);
        } else {
            app.acts.stay(&before, &app.runs);
        }
        // A run watched (`w`) that has finished says so.
        app.runs_came(&[]);
        ring(app);
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
                    self.keep_rate(r);
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
            k if k.starts_with("x-items:") || k.starts_with("x-disc:") => {
                let Some((part, login)) = k.split_once(':') else { return };
                // Asked before the owners or the search changed.
                if !self.extra_for.iter().any(|o| o == login) {
                    return;
                }
                if let Ok(v) = parse_items(body) {
                    self.extra.entry(login.to_string()).or_default()[usize::from(part == "x-disc")] = v;
                    self.rebuild(app);
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
                match parse_names(body, if labels { "name" } else { "login" }) {
                    Ok(v) if labels => app.got_lists(repo, |l| (l.labels, l.came[0]) = (v, true)),
                    Ok(v) => app.got_lists(repo, |l| (l.people, l.came[1]) = (v, true)),
                    Err(e) => self.list_failed(app, repo, e),
                }
            }
            k if k.starts_with("miles:") => {
                let repo = &k[6..];
                match minato_core::github::write::parse_milestones(body) {
                    Ok(v) => app.got_lists(repo, |l| (l.milestones, l.came[2]) = (v, true)),
                    Err(e) => self.list_failed(app, repo, e),
                }
            }
            "notes" => match notes::parse(body) {
                Ok(v) => app.take_notes(v),
                Err(e) => app.say(e),
            },
            k if k.starts_with("fetch:") => match parse_item(body) {
                Ok(Some(item)) => {
                    let id = item.id.clone();
                    app.fetched.retain(|f| f.id != id);
                    app.fetched.push(item);
                    self.rebuild(app);
                    app.show_fetched(&id);
                }
                // Not something the list can show: its page.
                _ => app.open_url = Some(k["fetch:".len()..].to_string()),
            },
            k if k.starts_with("note:") => {
                if let Some(done) = self.pending.remove(k) {
                    let words = app.tr(done);
                    app.say(words);
                }
                self.next_notes = Instant::now();
            }
            "board-pick" => match board::pick(body, &app.settings.project) {
                Ok(n) => {
                    // The pick was asked instead of the board: the board now.
                    if live {
                        self.board_asked = None;
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
                self.board_asked = None;
            }
            "a-repos" => match actions::parse_repos(body) {
                Ok(v) => self.set_repos(app, String::new(), v),
                Err(e) => app.say(e),
            },
            k if k.starts_with("a-repos:") => {
                let login = &k["a-repos:".len()..];
                // Kept from before, or asked before the owners changed.
                if !self.also().iter().any(|o| o == login) {
                    return;
                }
                match actions::parse_repos(body) {
                    Ok(v) => self.set_repos(app, login.to_string(), v),
                    Err(e) => app.say(e),
                }
            }
            k if k.starts_with("s-rel:") => {
                if let Ok(s) = overview::parse_latest(body) {
                    app.shipped.insert(k["s-rel:".len()..].to_string(), s);
                }
            }
            k if k.starts_with("s-cmp:") => {
                // "s-cmp:owner/name tag": the commits since that tag.
                if let (Some((repo, tag)), Ok(n)) = (k["s-cmp:".len()..].split_once(' '), overview::parse_ahead(body)) {
                    app.ahead.insert(repo.to_string(), (tag.to_string(), n));
                }
            }
            k if k.starts_with("a-runs:") => {
                let repo = &k["a-runs:".len()..];
                // Kept from before, or asked before the owner changed.
                if !self.repos.is_empty() && !self.repos.iter().any(|r| r == repo) {
                    return;
                }
                match actions::parse_runs(body) {
                    Ok(v) => {
                        self.run_found.insert(repo.to_string(), v);
                        self.show_runs(app);
                    }
                    Err(e) => app.say(e),
                }
            }
            k if k.starts_with("a-watch:") => {
                // A watched run newer ones pushed out of the runs.
                if let Ok(r) = actions::parse_run(body) {
                    app.runs_came(&[r]);
                    ring(app);
                }
            }
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
            k if k.starts_with("s-sec:") => {
                // "s-sec:word:owner/name": one kind of alert of one repository.
                let id = &k["s-sec:".len()..];
                let repo = id.split_once(':').map_or("", |(_, r)| r);
                if !self.repos.iter().any(|r| r == repo) {
                    return;
                }
                let source = id.split_once(':').and_then(|(w, _)| security::Source::from_word(w));
                if let Some(source) = source {
                    match security::parse(source, repo, body) {
                        Ok(v) => {
                            self.sec_failed.remove(id);
                            self.sec_found.insert(id.to_string(), v);
                        }
                        Err(_) => {
                            self.sec_found.remove(id);
                            self.sec_failed.insert(id.to_string());
                        }
                    }
                    self.show_security(app);
                }
            }
            k if k.starts_with("write:") => {
                if k.starts_with("write:sec:") {
                    self.sec_asked = None;
                }
                if body.contains("\"errors\"") {
                    app.say(body.chars().take(160).collect::<String>());
                } else if let Some(done) = self.pending.remove(k) {
                    let words = app.tr(done);
                    app.say(words);
                }
                // Look again at once, and re-ask the detail (not from the cache).
                self.next_search = Instant::now();
                self.runs_asked = None;
                self.asked = None;
                self.refetch = true;
                self.asked_run = None;
                self.run_refetch = true;
            }
            _ => {}
        }
    }

    /// The alerts found so far, worst first, in the security view and (the
    /// critical and high ones) the inbox.
    fn show_security(&mut self, app: &mut App) {
        let repos = &self.repos;
        let mut all: Vec<security::Alert> = self
            .sec_found
            .iter()
            .filter(|(k, _)| k.split_once(':').is_some_and(|(_, r)| repos.iter().any(|x| x == r)))
            .flat_map(|(_, v)| v.iter().cloned())
            .collect();
        security::sort(&mut all);
        let asked = repos.len() * 3;
        let answered = self.sec_found.len() + self.sec_failed.len();
        // A repository none of whose three kinds can be read (no access, or
        // the features are off) is only counted.
        app.sec_unreadable =
            repos.iter().filter(|r| ["code", "dependabot", "secret"].iter().all(|w| self.sec_failed.contains(&format!("{w}:{r}")))).count();
        app.sec_loaded = asked > 0 && answered >= asked;
        app.alerts = all;
        app.security_sel = app.security_sel.min(app.alerts.len().saturating_sub(1));
        self.rebuild(app);
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

    /// One page of a search (`base`) from GitHub: the next is asked for,
    /// and once the last has come the items change (and are kept). A page
    /// of a round a newer first page has ended is dropped. The first page
    /// of a search other than the one shown (closed items, a saved search)
    /// shows at once, not the old search's items until all have come.
    fn search_page(&mut self, app: &mut App, base: &'static str, key: &str, body: String) {
        if key == base {
            self.search_round += 1;
            self.search_pages.insert(base, (self.search_round, Vec::new()));
            if self.shown_q.get(base) != self.searched_q.get(base) && search::next_page(&body).is_some() {
                self.take(app, base, &body, true);
            }
        } else if !self.search_pages.get(base).is_some_and(|(round, _)| key.ends_with(&format!("-more:{round}"))) {
            return;
        }
        let Some((round, pages)) = self.search_pages.get_mut(base) else { return };
        let next = search::next_page(&body).filter(|_| pages.len() + 1 < SEARCH_PAGES);
        pages.push(body);
        if let (Some(after), Some(q), Some(w)) = (next, self.searched_q.get(base), &self.worker) {
            let query = if base == "items" { items_query() } else { discussions_query() };
            let job = Job { key: format!("{base}-more:{round}"), call: Call::Graphql { query, variables: search_vars(q, Some(&after)) } };
            track(&mut self.round, &job.key);
            if matches!(app.view, View::Inbox | View::List) {
                w.send_first(job);
            } else {
                w.send(job);
            }
            return;
        }
        self.search_done(app, base);
    }

    /// The pages of a search come so far, as its answer.
    fn search_done(&mut self, app: &mut App, base: &'static str) {
        let Some((_, pages)) = self.search_pages.remove(base) else { return };
        let Ok(all) = search::merge(&pages) else { return };
        if cached(base, self.searched_custom.is_some() || self.searched_closed) && !all.contains("\"errors\"") {
            self.keep(base, "", &all);
        }
        if let Some(q) = self.searched_q.get(base) {
            self.shown_q.insert(base, q.clone());
        }
        self.take(app, base, &all, true);
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
            let job = Job {
                key: "board-more".into(),
                call: Call::Graphql { query: board::query(self.other()), variables: board::vars(n, Some(&after)) },
            };
            track(&mut self.round, &job.key);
            if app.view == View::Board {
                w.send_first(job);
            } else {
                w.send(job);
            }
            return;
        }
        let pages = std::mem::take(&mut self.board_pages);
        let all = board::merge(&pages).unwrap_or_else(|_| body.to_string());
        // An error beside the data (a Project with no Priority field) is
        // kept; an answer with no Project is not.
        if board::parse(&all).is_ok() {
            self.keep("board", "", &all);
        }
        self.show_board(app, &all, true);
    }

    /// A run's jobs, artifacts, workflow or release (by `key`), from GitHub (`kept`
    /// is `None`; the stamp is the run's `updated_at` now) or from the cache
    /// with its stamp. Kept by run; shown only while the run is selected. A
    /// finished run's answers go to disk.
    fn take_run(&mut self, app: &mut App, key: &str, body: &str, kept: Option<&str>) {
        let Some((part, id)) = key.split_once(':') else { return };
        let Ok(id) = id.parse::<u64>() else { return };
        let run = app.runs.iter().find(|r| r.id == id);
        let path = run.and_then(workflow::contents_path);
        let done = run.is_some_and(|r| !r.status.active());
        let stamp = match kept {
            Some(s) => s.to_string(),
            None => run.map(|r| r.updated_at.clone()).unwrap_or_default(),
        };
        // The releases are matched to the run, so it must be known; what is
        // kept is only the release it made.
        let narrowed;
        let body = match (part, run) {
            ("a-rels", None) => return,
            ("a-rels", Some(r)) if kept.is_none() => match actions::narrow_releases(body, r) {
                Ok(b) => {
                    narrowed = b;
                    &narrowed
                }
                Err(e) => return app.say(e),
            },
            _ => body,
        };
        let release = match (part, run) {
            ("a-rels", Some(r)) => Some(actions::parse_release(body, r)),
            _ => None,
        };
        if kept.is_none() && done && !stamp.is_empty() {
            self.keep(key, &stamp, body);
        }
        let m = self.run_kept.entry(id).or_insert_with(|| RunMemo {
            stamp: stamp.clone(),
            detail: RunDetail { id, ..Default::default() },
            got: [false; 4],
        });
        // A newer state of the run: what is kept shows until the new comes.
        if m.stamp != stamp {
            m.stamp = stamp;
            m.got[0] = false;
            m.got[1] = false;
            m.got[3] = false;
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
            "a-rels" => match release {
                Some(Ok(v)) => (m.detail.release, m.got[3]) = (v, true),
                Some(Err(e)) => app.say(e),
                None => {}
            },
            _ => {
                // A workflow that cannot be read just has no graph.
                if let Ok(text) = workflow::parse_contents(body) {
                    let w = workflow::parse(&text);
                    if let Some(p) = path {
                        self.yaml_by.insert(p, w.clone());
                    }
                    m.detail.workflow = Some(w);
                }
                m.got[2] = true;
                m.detail.yaml_tried = true;
            }
        }
        if app.run().is_some_and(|r| r.id == id) {
            app.run_detail = Some(m.detail.clone());
        }
    }

    /// Whether the run's workflow is known: kept for it, or read for
    /// another run of the same file at the same commit (then kept for it).
    fn known_yaml(&mut self, r: &Run) -> bool {
        if self.run_kept.get(&r.id).is_some_and(|m| m.got[2]) {
            return true;
        }
        let Some(w) = workflow::contents_path(r).and_then(|p| self.yaml_by.get(&p)) else { return false };
        let m = self.run_kept.entry(r.id).or_insert_with(|| RunMemo {
            stamp: r.updated_at.clone(),
            detail: RunDetail { id: r.id, ..Default::default() },
            got: [false; 4],
        });
        m.detail.workflow = Some(w.clone());
        m.got[2] = true;
        m.detail.yaml_tried = true;
        true
    }

    /// Keep a reply's count in its own limit's place; search's is not kept.
    fn keep_rate(&mut self, r: Rate) {
        match r.bucket {
            Bucket::Core => self.rates[0] = Some(r),
            Bucket::Graphql => self.rates[1] = Some(r),
            Bucket::Other => {}
        }
    }

    /// Read a finished run's jobs, artifacts and workflow ahead on `back`,
    /// one run at a time: the runs near the selected one first, then the
    /// rest. What is kept is not asked; a run tried is not tried again this
    /// start (unless it changes); nothing while the calls left are few.
    fn read_ahead(&mut self, app: &App) {
        if self.back.is_none() || self.back_busy > 0 || Rate::tighter(&self.rates).is_some_and(|r| r.remaining < BACK_FLOOR) {
            return;
        }
        let order: Vec<Run> = app.acts.near_first(&app.runs).into_iter().filter(|r| !r.status.active()).cloned().collect();
        for r in order {
            let tried = (r.id, r.updated_at.clone());
            if self.back_tried.contains(&tried) {
                continue;
            }
            let yaml = self.known_yaml(&r);
            let memo = self.run_kept.get(&r.id);
            let same = memo.is_some_and(|m| m.stamp == r.updated_at);
            let mut jobs = Vec::new();
            if !(same && memo.is_some_and(|m| m.got[0])) {
                jobs.push(Job { key: format!("a-jobs:{}", r.id), call: Call::Get(actions::jobs_path(&r)) });
            }
            if !(same && memo.is_some_and(|m| m.got[1])) {
                jobs.push(Job { key: format!("a-arts:{}", r.id), call: Call::Get(actions::artifacts_path(&r)) });
            }
            if actions::makes_release(&r) && !(same && memo.is_some_and(|m| m.got[3])) {
                jobs.push(Job { key: format!("a-rels:{}", r.id), call: Call::Get(actions::releases_path(&r)) });
            }
            if let Some(path) = workflow::contents_path(&r).filter(|_| !yaml) {
                jobs.push(Job { key: format!("a-yaml:{}", r.id), call: Call::Get(path) });
            }
            self.back_tried.insert(tried);
            if jobs.is_empty() {
                continue;
            }
            self.back_busy = jobs.len();
            if let Some(back) = &self.back {
                for j in jobs {
                    back.send(j);
                }
            }
            return;
        }
    }

    pub fn pump(&mut self, app: &mut App, ctx: &egui::Context) {
        if self.worker.is_none() {
            match self.token_rx.try_recv() {
                Ok(Some((token, _))) => {
                    let (wake, wake2, wake3) = (ctx.clone(), ctx.clone(), ctx.clone());
                    let client = |token: Token| Client::sharing(UreqTransport::new(), token, self.etags.clone());
                    self.quick = Some(Worker::spawn(client(token.clone()), move || wake2.request_repaint()));
                    self.back = Some(Worker::spawn(client(token.clone()), move || wake3.request_repaint()));
                    self.runs = (0..RUN_LANES)
                        .map(|_| {
                            let wake = ctx.clone();
                            Worker::spawn(client(token.clone()), move || wake.request_repaint())
                        })
                        .collect();
                    let worker = Worker::spawn(client(token), move || wake.request_repaint());
                    worker.send(get("me", "/user"));
                    worker.send(get("rate", "/rate_limit"));
                    self.worker = Some(worker);
                    self.next_poll = Instant::now() + POLL;
                }
                Ok(None) => {
                    app.signed_out = true;
                    let words = app.tr("net.no_token");
                    app.say(words);
                    // Do not look again; a restart does.
                    let (_, rx) = mpsc::channel();
                    self.token_rx = rx;
                }
                Err(_) => {}
            }
        }
        self.rescope(app);
        self.load_cache(app);
        let (Some(worker), Some(quick)) = (&self.worker, &self.quick) else { return };
        let mut replies: Vec<_> = std::iter::from_fn(|| worker.try_recv().or_else(|| quick.try_recv())).map(|r| (r, false)).collect();
        for w in &self.runs {
            replies.extend(std::iter::from_fn(|| w.try_recv()).map(|r| (r, false)));
        }
        if let Some(back) = &self.back {
            replies.extend(std::iter::from_fn(|| back.try_recv()).map(|r| (r, true)));
        }
        for (reply, ahead) in replies {
            if let Some(r) = &mut self.round {
                r.want.remove(&reply.key);
            }
            if ahead {
                self.back_busy = self.back_busy.saturating_sub(1);
            }
            match reply.result {
                Ok(f) => {
                    app.synced = Some(Instant::now());
                    if let Some(r) = f.rate {
                        self.keep_rate(r);
                    }
                    let key = reply.key.clone();
                    if let Some(base) = search_of(&key) {
                        self.live.insert(base.to_string());
                        self.search_page(app, base, &key, f.body);
                        continue;
                    }
                    self.live.insert(key.clone());
                    // A 304's body is the one already kept.
                    if !f.cached && cached(&key, self.searched_custom.is_some() || self.searched_closed) && !f.body.contains("\"errors\"") {
                        self.keep(&key, f.etag.as_deref().unwrap_or(""), &f.body);
                    }
                    self.take(app, &key, &f.body, true);
                }
                // What is read ahead and does not come is left to a selection.
                Err(_) if ahead => {}
                Err(e) => {
                    self.pending.remove(&reply.key);
                    // A repository with no release to read, or none to
                    // compare with: the status view just shows none.
                    if reply.key.starts_with("s-rel:") || reply.key.starts_with("s-cmp:") {
                        continue;
                    }
                    // Alerts of a kind that cannot be read (not enabled, or no
                    // access) are counted, not said.
                    if let Some(id) = reply.key.strip_prefix("s-sec:") {
                        self.sec_found.remove(id);
                        self.sec_failed.insert(id.to_string());
                        self.show_security(app);
                        continue;
                    }
                    // An owner not found is not asked again until F5.
                    if let Some(login) = reply.key.strip_prefix("a-repos:") {
                        self.repo_sets.insert(login.to_string(), Vec::new());
                    }
                    if let Some(repo) = list_repo(&reply.key) {
                        self.list_failed(app, repo, e);
                        continue;
                    }
                    if let Some(id) = reply.key.strip_prefix("a-log:").and_then(|n| n.parse::<u64>().ok()) {
                        if let Some(Overlay::Log(l)) = app.overlay.as_mut() {
                            if l.job_id == id {
                                l.lines = Some(vec![e.clone()]);
                            }
                        }
                    }
                    if reply.key.starts_with("board:") {
                        self.board_asked = None;
                    }
                    if let Some(base) = search_of(&reply.key).filter(|_| reply.key.contains("-more:")) {
                        if self.search_pages.get(base).is_some_and(|(round, _)| reply.key.ends_with(&format!(":{round}"))) {
                            self.search_done(app, base);
                        }
                    }
                    if let Some(url) = reply.key.strip_prefix("fetch:") {
                        app.open_url = Some(url.to_string());
                        continue;
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
                    // A workflow file that does not come is not waited for:
                    // `D` then starts the run with no inputs.
                    if let Some(id) = reply.key.strip_prefix("a-yaml:").and_then(|n| n.parse::<u64>().ok()) {
                        let stamp = app.runs.iter().find(|r| r.id == id).map(|r| r.updated_at.clone()).unwrap_or_default();
                        let m = self.run_kept.entry(id).or_insert_with(|| RunMemo {
                            stamp,
                            detail: RunDetail { id, ..Default::default() },
                            got: [false; 4],
                        });
                        m.detail.yaml_tried = true;
                        if let Some(d) = app.run_detail.as_mut().filter(|d| d.id == id) {
                            d.yaml_tried = true;
                        }
                    }
                    if e == client::REFUSED {
                        app.signed_out = true;
                        let words = app.tr("net.refused");
                        app.say(words);
                    } else {
                        app.say(e);
                    }
                }
            }
        }
        if std::mem::take(&mut app.sync_wanted) {
            self.start_round(app);
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
                quick.send_in(
                    "peek",
                    &id,
                    Job { key: format!("peek:{id}"), call: Call::Graphql { query: board::peek_query().into(), variables: vars } },
                );
                self.peek_asked = Some(id);
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
                    track(&mut self.round, &format!("detail:{id}"));
                    quick.send(Job { key: format!("detail:{id}"), call: Call::Graphql { query: detail_query().into(), variables: vars } });
                }
                // The writing box's lists are read ahead, after the detail:
                // `r` then opens with them.
                let repo = app.selected().map(|i| i.repo.clone()).unwrap_or_default();
                for (key, call) in app.ask_lists(&repo) {
                    quick.send(Job { key, call });
                }
            }
            self.asked = want;
            self.refetch = false;
        }
        app.api = Rate::tighter(&self.rates).map(|r| (r.remaining, r.limit));
        let now = Instant::now();
        if now >= self.next_poll {
            worker.send(get("rate", "/rate_limit"));
            track(&mut self.round, "rate");
            let unix = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let wait = Rate::tighter(&self.rates).map_or(POLL, |r| r.interval(POLL, unix));
            self.next_poll = now + wait;
        }
        if let Some(me) = app.viewer.clone() {
            // The searches are of the owner's repositories; the mentions
            // and the authors stay the viewer's.
            let owners = Some(app.owners_shown()).filter(|o| !o.is_empty()).unwrap_or_else(|| me.clone());
            let users = minato_core::list::users(&owners);
            let custom = if app.view == crate::app::View::List {
                app.list.active_search().map(|t| minato_core::list::search_query(t, &owners))
            } else {
                None
            };
            let closed = app.closed_shown();
            if closed != self.searched_closed || custom != self.searched_custom {
                // The open items' answers are kept while the list shows
                // others, and shown again at once on coming back.
                let plain = |c: bool, q: &Option<String>| !c && q.is_none();
                if plain(self.searched_closed, &self.searched_custom) {
                    self.stash = Some((self.found.clone(), self.mentioned.clone(), self.stubs.clone()));
                } else if plain(closed, &custom) {
                    if let Some((found, mentioned, stubs)) = self.stash.take() {
                        (self.found, self.mentioned, self.stubs) = (found, mentioned, stubs);
                    }
                }
                self.searched_closed = closed;
                self.searched_custom = custom.clone();
                self.search_pages.clear();
                // The owners alongside are asked again with the rest.
                self.extra.clear();
                self.extra_for.clear();
                self.next_search = now;
                if plain(closed, &custom) {
                    self.rebuild(app);
                }
            }
            if now >= self.next_search {
                let state = if closed { "is:closed" } else { "is:open" };
                let mut logins = owners.split_whitespace();
                let plain = minato_core::list::owner_search(logins.next().unwrap_or(&me), state);
                let others: Vec<String> = logins.map(str::to_string).collect();
                let q = custom.clone().unwrap_or_else(|| plain.clone());
                worker.send(Job { key: "items".into(), call: Call::Graphql { query: items_query(), variables: search_vars(&q, None) } });
                track(&mut self.round, "items");
                self.searched_q.insert("items", q);
                if custom.is_some() {
                    self.found[1].clear();
                    self.mentioned.clear();
                    self.stubs.clear();
                } else if closed {
                    self.found[1].clear();
                } else {
                    let q = format!("{users} is:open mentions:{me} -author:{me} sort:updated-desc");
                    worker.send(Job {
                        key: "mentions".into(),
                        call: Call::Graphql { query: items_query(), variables: search_vars(&q, None) },
                    });
                    worker.send(Job {
                        key: "mainci".into(),
                        call: Call::Graphql { query: mainci::query(self.other()), variables: "{}".into() },
                    });
                    let vars = search_vars(&plain, None);
                    worker.send(Job { key: "discussions".into(), call: Call::Graphql { query: discussions_query(), variables: vars } });
                    for k in ["mentions", "mainci", "discussions"] {
                        track(&mut self.round, k);
                    }
                    self.searched_q.insert("discussions", plain);
                }
                // Each owner shown alongside: its first page, apart. A saved
                // search names them all in its own query.
                self.extra.retain(|k, _| others.contains(k));
                self.extra_for = if custom.is_some() { Vec::new() } else { others };
                for o in &self.extra_for {
                    let vars = search_vars(&minato_core::list::owner_search(o, state), None);
                    let mut jobs =
                        vec![Job { key: format!("x-items:{o}"), call: Call::Graphql { query: items_query(), variables: vars.clone() } }];
                    if !closed {
                        jobs.push(Job { key: format!("x-disc:{o}"), call: Call::Graphql { query: discussions_query(), variables: vars } });
                    }
                    for job in jobs {
                        track(&mut self.round, &job.key);
                        worker.send(job);
                    }
                }
                self.next_search = now + SEARCH;
            }
        }
        if app.viewer.is_some() && now >= self.next_notes {
            // Asked without its ETag: GitHub answers 304 while no thread is
            // new, even after some were marked read.
            self.etags.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(&client::url(notes::PATH));
            worker.send(get("notes", notes::PATH));
            track(&mut self.round, "notes");
            self.next_notes = now + NOTES;
        }
        // The board and the runs: those of the open view first and often,
        // the other's behind the searches and less often, so the view opens
        // on them.
        let send = |job: Job, open: bool| if open { worker.send_first(job) } else { worker.send(job) };
        let open = app.view == View::Board;
        // Asked behind the searches and not answered yet (the owner changed
        // in the settings): the view opening on it asks again, first.
        if open && self.board_away && app.board.is_none() {
            self.board_asked = None;
        }
        if app.viewer.is_some() && due(self.board_asked, if open { BOARD } else { BOARD_AWAY }, now) <= now {
            track(&mut self.round, if self.board_no.is_some() { "board" } else { "board-pick" });
            match self.board_no {
                None => send(
                    Job {
                        key: "board-pick".into(),
                        call: Call::Graphql { query: board::pick_query(self.other()), variables: "{}".into() },
                    },
                    open,
                ),
                Some(n) => send(
                    Job { key: "board".into(), call: Call::Graphql { query: board::query(self.other()), variables: board::vars(n, None) } },
                    open,
                ),
            }
            self.board_asked = Some(now);
            self.board_away = !open;
        }
        let open = app.view == View::Actions;
        if open && self.runs_away && self.run_found.is_empty() {
            self.runs_asked = None;
        }
        // A run watched (`w`) is looked at as often as the open view's.
        let often = open || !app.watched.is_empty();
        if app.viewer.is_some() && due(self.runs_asked, if often { RUNS } else { RUNS_AWAY }, now) <= now {
            // Each owner's repositories not known yet, on a runs lane: not
            // behind a slow search of a new owner.
            let missing: Vec<String> =
                std::iter::once(String::new()).chain(self.also()).filter(|k| !self.repo_sets.contains_key(k)).collect();
            for k in &missing {
                let job = if k.is_empty() {
                    get("a-repos", &actions::repos_path(self.other()))
                } else {
                    get(&format!("a-repos:{k}"), &actions::repos_path(Some(k)))
                };
                track(&mut self.round, &job.key);
                match self.runs.first() {
                    Some(w) if open => w.send_first(job),
                    Some(w) => w.send(job),
                    None => send(job, open),
                }
            }
            if !self.repos.is_empty() {
                for (k, r) in self.repos.iter().enumerate() {
                    let job = Job { key: format!("a-runs:{r}"), call: Call::Get(actions::runs_path(r)) };
                    track(&mut self.round, &job.key);
                    match self.runs.get(k % RUN_LANES) {
                        Some(w) if open => w.send_first(job),
                        Some(w) => w.send(job),
                        None => send(job, open),
                    }
                }
                // Each watched run newer ones have pushed out of the runs.
                for r in app.watched_away() {
                    let job = Job { key: format!("a-watch:{}", r.id), call: Call::Get(actions::run_path(&r.repo, r.id)) };
                    track(&mut self.round, &job.key);
                    match self.runs.first() {
                        Some(w) => w.send(job),
                        None => send(job, open),
                    }
                }
                // A running or queued run's jobs are asked again with the runs.
                if open && app.run().is_some_and(|r| r.status.active()) {
                    self.asked_run = None;
                }
            }
            self.runs_asked = Some(now);
            self.runs_away = !open;
        }
        if app.viewer.is_some() && open {
            // The run selected: what is kept shows at once. A finished run's
            // parts kept since it finished are not asked again; the rest are,
            // and those of a run passed over (`j` `k`) are dropped unasked.
            let run = app.run().cloned();
            let want = run.as_ref().map(|r| (r.id, r.updated_at.clone()));
            if want != self.asked_run && now >= self.run_retry {
                if let Some(r) = &run {
                    let yaml = !self.run_refetch && self.known_yaml(r);
                    let Some(quick) = &self.quick else { return };
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
                    if actions::makes_release(r) && !(same && got(3)) {
                        quick.send_in("run", &tag, Job { key: format!("a-rels:{}", r.id), call: Call::Get(actions::releases_path(r)) });
                    }
                    if let Some(path) = workflow::contents_path(r).filter(|_| !yaml) {
                        quick.send_in("run", &tag, Job { key: format!("a-yaml:{}", r.id), call: Call::Get(path) });
                    }
                }
                self.asked_run = want;
                self.run_refetch = false;
            }
        }
        // The status view: each repository's last release, then the commits
        // since it, asked while the view is open.
        if app.viewer.is_some() && app.view == View::Status && !self.repos.is_empty() {
            if due(self.status_asked, STATUS, now) <= now {
                for (k, r) in self.repos.iter().enumerate() {
                    let job = get(&format!("s-rel:{r}"), &overview::releases_path(r));
                    track(&mut self.round, &job.key);
                    if let Some(w) = self.runs.get(k % RUN_LANES).or(self.worker.as_ref()) {
                        w.send_first(job);
                    }
                }
                self.status_asked = Some(now);
                self.cmp_asked.clear();
            }
            for (repo, s) in &app.shipped {
                let Some(s) = s.as_ref().filter(|s| !s.tag.is_empty() && self.repos.contains(repo)) else { continue };
                if self.cmp_asked.get(repo) == Some(&s.tag) {
                    continue;
                }
                let job = get(&format!("s-cmp:{repo} {}", s.tag), &overview::compare_path(repo, &s.tag));
                track(&mut self.round, &job.key);
                if let Some(q) = &self.quick {
                    q.send(job);
                }
                self.cmp_asked.insert(repo.clone(), s.tag.clone());
            }
        }
        // The security alerts: asked in every view (the inbox shows the
        // urgent ones), more often while their view is open.
        if app.viewer.is_some() && !self.repos.is_empty() {
            if std::mem::take(&mut app.alerts_changed) {
                let keep: HashSet<String> = app.alerts.iter().map(|a| a.key()).collect();
                for v in self.sec_found.values_mut() {
                    v.retain(|a| keep.contains(&a.key()));
                }
                self.rebuild(app);
            }
            let every = if app.view == View::Security { SECURITY } else { SECURITY_AWAY };
            if due(self.sec_asked, every, now) <= now {
                let mut n = 0;
                for r in &self.repos {
                    for source in [security::Source::Code, security::Source::Dependabot, security::Source::Secret] {
                        let job = get(&format!("s-sec:{}:{r}", source.word()), &source.list_path(r));
                        track(&mut self.round, &job.key);
                        if let Some(w) = self.runs.get(n % RUN_LANES).or(self.worker.as_ref()) {
                            w.send_first(job);
                        }
                        n += 1;
                    }
                }
                self.sec_asked = Some(now);
            }
        }
        if app.viewer.is_some() {
            self.read_ahead(app);
        }
        self.end_round(app, now);
        let wake = self.next_poll.min(self.next_search).min(self.next_notes);
        let wake = match (&app.viewer, app.view) {
            (None, _) => wake,
            (Some(_), View::Board) => wake.min(due(self.board_asked, BOARD, now)).min(due(self.runs_asked, RUNS_AWAY, now)),
            (Some(_), View::Actions) => wake.min(due(self.runs_asked, RUNS, now)).min(due(self.board_asked, BOARD_AWAY, now)),
            (Some(_), _) => wake.min(due(self.runs_asked, RUNS_AWAY, now)).min(due(self.board_asked, BOARD_AWAY, now)),
        };
        let wake = if self.run_retry > now { wake.min(self.run_retry) } else { wake };
        let wake = match &app.viewer {
            Some(_) if app.view == View::Status => {
                wake.min(due(self.status_asked, STATUS, now)).min(due(self.sec_asked, SECURITY_AWAY, now))
            }
            Some(_) if app.view == View::Security => wake.min(due(self.sec_asked, SECURITY, now)),
            Some(_) if !app.watched.is_empty() => wake.min(due(self.runs_asked, RUNS, now)).min(due(self.sec_asked, SECURITY_AWAY, now)),
            Some(_) => wake.min(due(self.sec_asked, SECURITY_AWAY, now)),
            _ => wake,
        };
        ctx.request_repaint_after(wake.saturating_duration_since(now));
    }
}

/// Whether the answer to `key` is kept as it comes (a detail and a run's
/// parts are kept apart, with their stamp; the board once all its pages
/// have come). `narrowed`: the items are of a saved search or the closed
/// ones, which a start would not show.
/// The search (`items`, `discussions`) a key's answer is a page of.
fn search_of(key: &str) -> Option<&'static str> {
    ["items", "discussions"].into_iter().find(|b| key == *b || key.strip_prefix(b).is_some_and(|r| r.starts_with("-more:")))
}

fn cached(key: &str, narrowed: bool) -> bool {
    match key {
        "items" => !narrowed,
        "me" | "discussions" | "mentions" | "mainci" | "board-pick" | "a-repos" => true,
        k => ["a-runs:", "a-repos:", "s-rel:"].iter().any(|p| k.starts_with(p)) || list_repo(k).is_some(),
    }
}

/// Whether `key`'s answer is of the owner or the Project the settings name
/// (not shown from the cache once they have changed).
fn scoped(key: &str) -> bool {
    matches!(key, "items" | "discussions" | "mentions" | "mainci" | "board-pick" | "board" | "a-repos")
        || key.starts_with("a-runs:")
        || key.starts_with("a-repos:")
}

/// The owner and the Project as kept in the cache (`None`: the viewer and
/// the default Project).
fn scope_line(scope: Option<&(String, String)>) -> String {
    match scope {
        Some((owner, project)) => format!("{owner}\n{project}"),
        None => format!("\n{}", board::TITLE),
    }
}

/// When a poll last asked at `asked` (`None`: never, or to be asked at once:
/// `now`) is due again, asked every `every`.
fn due(asked: Option<Instant>, every: Duration, now: Instant) -> Instant {
    asked.map_or(now, |t| t + every)
}

/// The path of a kept GET, whose ETag it is kept with (`other`: the owner
/// the settings name, if not the viewer).
fn rest_path(key: &str, other: Option<&str>) -> Option<String> {
    match key {
        "me" => Some("/user".into()),
        "a-repos" => Some(actions::repos_path(other)),
        k if k.starts_with("a-repos:") => Some(actions::repos_path(Some(&k["a-repos:".len()..]))),
        k if k.starts_with("s-rel:") => Some(overview::releases_path(&k["s-rel:".len()..])),
        k if k.starts_with("labels:") => list_repo(k).map(write::labels_path),
        k if k.starts_with("people:") => list_repo(k).map(write::assignees_path),
        k if k.starts_with("miles:") => list_repo(k).map(write::milestones_path),
        k => k.strip_prefix("a-runs:").map(actions::runs_path),
    }
}

/// The repository of a key asking for a list for the writing box.
fn list_repo(key: &str) -> Option<&str> {
    ["labels:", "people:", "miles:"].iter().find_map(|p| key.strip_prefix(p))
}

/// Whether `key` is for a run's jobs, artifacts, workflow or release.
fn run_part(key: &str) -> bool {
    ["a-jobs:", "a-arts:", "a-yaml:", "a-rels:"].iter().any(|p| key.starts_with(p))
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

/// Tell of the watched runs that finished as the settings say: the
/// webhook, the system's notification and a sound (none in focus mode, if
/// so set).
pub fn ring(app: &mut App) {
    for r in app.rings.drain(..) {
        if let Some(h) = r.hook {
            ito_notify::webhook::send(h.url, h.format, h.level, r.title.clone(), r.body.clone(), h.folder);
        }
        if r.system {
            ito_notify::toast::show_with_icon(
                ito_notify::toast::App { id: "uchmk.minato", name: "minato" },
                crate::icon::pixels,
                &r.title,
                &r.body,
            );
        }
        if let Some(s) = r.sound.filter(|_| !(r.focus && ito_notify::sound::quiet_time())) {
            ito_notify::sound::play(&s);
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_poll_never_asked_is_due_at_once() {
        let now = Instant::now();
        assert!(due(None, RUNS, now) <= now);
        assert!(due(Some(now), RUNS, now) > now);
        assert!(due(Some(now - BOARD), BOARD, now) <= now);
    }

    #[test]
    fn a_cache_with_no_mark_is_of_the_viewer_and_uchmk() {
        assert_eq!(scope_line(None), scope_line(Some(&(String::new(), "uchmk".into()))));
        assert_ne!(scope_line(None), scope_line(Some(&("rust-lang".into(), "uchmk".into()))));
        assert!(scoped("a-runs:o/r") && scoped("board") && !scoped("detail:x") && !scoped("notes"));
        assert_eq!(rest_path("a-repos", Some("o")).as_deref(), Some("/users/o/repos?type=owner&sort=pushed&per_page=12"));
        // Another owner's repositories, and a repository's releases.
        assert!(scoped("a-repos:p") && cached("a-repos:p", false) && cached("s-rel:o/r", false) && !scoped("s-rel:o/r"));
        assert_eq!(rest_path("a-repos:p", Some("o")).as_deref(), Some("/users/p/repos?type=owner&sort=pushed&per_page=12"));
        assert_eq!(rest_path("s-rel:o/r", None).as_deref(), Some("/repos/o/r/releases?per_page=5"));
    }

    #[test]
    fn the_writing_boxs_lists_are_kept_with_their_etags() {
        assert_eq!(list_repo("people:o/r"), Some("o/r"));
        assert_eq!(list_repo("detail:x"), None);
        assert!(cached("labels:o/r", true) && cached("miles:o/r", false) && !scoped("people:o/r"));
        assert_eq!(rest_path("people:o/r", None).as_deref(), Some("/repos/o/r/assignees?per_page=100"));
    }

    #[test]
    fn a_searchs_pages_are_known_by_their_key() {
        assert_eq!(search_of("items"), Some("items"));
        assert_eq!(search_of("items-more:3"), Some("items"));
        assert_eq!(search_of("discussions-more:12"), Some("discussions"));
        assert_eq!(search_of("mentions"), None);
        assert_eq!(search_of("items-x"), None);
        assert_eq!(search_of("detail:items"), None);
    }
}
