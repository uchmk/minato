//! What to do with an answer once it has come: kept, parsed and handed to the app.

use super::*;

impl Net {
    /// The runs found of the repositories looked at, newest first.
    pub(super) fn show_runs(&self, app: &mut App) {
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
    pub(super) fn keep(&self, key: &str, stamp: &str, body: &str) {
        if let Some(tx) = &self.cache_tx {
            let _ = tx.send(cache::Entry { key: key.into(), stamp: stamp.into(), body: body.into() });
        }
    }

    /// An answer, come from GitHub (`live`) or from the cache.
    pub(super) fn take(&mut self, app: &mut App, key: &str, body: &str, live: bool) {
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
            k if k.starts_with("past:") => match parse_items(body) {
                Ok(v) => {
                    let gone = v.into_iter().filter(|i| i.merged || i.closed).collect();
                    app.past.insert(k["past:".len()..].to_string(), gone);
                }
                Err(e) => app.say(e),
            },
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
    pub(super) fn show_security(&mut self, app: &mut App) {
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
    pub(super) fn show_board(&mut self, app: &mut App, body: &str, live: bool) {
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
    pub(super) fn search_page(&mut self, app: &mut App, base: &'static str, key: &str, body: String) {
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
    pub(super) fn search_done(&mut self, app: &mut App, base: &'static str) {
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
    pub(super) fn board_page(&mut self, app: &mut App, key: &str, body: &str) {
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
    pub(super) fn take_run(&mut self, app: &mut App, key: &str, body: &str, kept: Option<&str>) {
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
    pub(super) fn known_yaml(&mut self, r: &Run) -> bool {
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
    pub(super) fn keep_rate(&mut self, r: Rate) {
        match r.bucket {
            Bucket::Core => self.rates[0] = Some(r),
            Bucket::Graphql => self.rates[1] = Some(r),
            Bucket::Other => {}
        }
    }
}
