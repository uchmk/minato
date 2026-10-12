//! The link between the window and `minato-core`'s GitHub worker: find the
//! token off the UI thread, ask who it is, and keep the rate limit fresh.
//! Each frame `pump` takes what has arrived; it never waits.

use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use minato_core::github::token::Source;
use minato_core::github::types::{parse_rate_limit, parse_viewer};
use minato_core::github::search::{detail_query, discussions_query, items_query, parse_detail, parse_items};
use minato_core::github::worker::{self, Call, Job, Worker};
use minato_core::github::{Client, Rate, Token, UreqTransport};

use minato_core::list::Item;

use crate::app::App;

/// How often the rate limit is asked while plenty is left.
const POLL: Duration = Duration::from_secs(60);

type Found = Option<(Token, Source)>;

pub struct Net {
    token_rx: Receiver<Found>,
    worker: Option<Worker>,
    next_poll: Instant,
    rate: Option<Rate>,
    next_search: Instant,
    /// The last answers of the two searches, merged into `app.items`.
    found: [Vec<Item>; 2],
    /// The detail last asked for (its node id).
    asked: Option<String>,
}

/// How often the lists are searched again.
const SEARCH: Duration = Duration::from_secs(120);

impl Net {
    /// Start looking for the token (it may run `gh`).
    pub fn start(ctx: &egui::Context) -> Net {
        let (tx, token_rx) = mpsc::channel();
        let ctx = ctx.clone();
        worker::find_token(move |found| {
            let _ = tx.send(found);
            ctx.request_repaint();
        });
        Net { token_rx, worker: None, next_poll: Instant::now(), rate: None, next_search: Instant::now(), found: [Vec::new(), Vec::new()], asked: None }
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
        let Some(worker) = &self.worker else { return };
        while let Some(reply) = worker.try_recv() {
            match reply.result {
                Ok(f) => {
                    app.synced = Some(Instant::now());
                    if let Some(r) = f.rate {
                        self.rate = Some(r);
                    }
                    match reply.key.as_str() {
                        "me" => match parse_viewer(&f.body) {
                            Ok(v) => {
                                app.viewer = Some(v.login);
                                self.next_search = Instant::now();
                            }
                            Err(e) => app.say(e),
                        },
                        "rate" => {
                            if let Ok(r) = parse_rate_limit(&f.body) {
                                self.rate = Some(r);
                            }
                        }
                        k @ ("items" | "discussions") => {
                            let parsed = parse_items(&f.body);
                            match parsed {
                                Ok(v) => {
                                    self.found[usize::from(k == "discussions")] = v;
                                    let mut all: Vec<Item> = self.found.concat();
                                    all.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
                                    app.items = all;
                                    app.list.clamp(&app.items, app.viewer.as_deref().unwrap_or(""));
                                }
                                Err(e) => app.say(e),
                            }
                        }
                        k if k.starts_with("detail:") => {
                            let id = &k["detail:".len()..];
                            if app.selected().is_some_and(|i| i.id == id) {
                                match parse_detail(&f.body) {
                                    Ok(d) => app.detail = Some((id.to_string(), d)),
                                    Err(e) => app.say(e),
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Err(e) => app.say(e),
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
            if now >= self.next_search {
                let q = format!("user:{me} is:open sort:updated-desc");
                let vars = format!("{{\"q\":{},\"n\":50}}", json_str(&q));
                worker.send(Job { key: "items".into(), call: Call::Graphql { query: items_query(), variables: vars.clone() } });
                worker.send(Job { key: "discussions".into(), call: Call::Graphql { query: discussions_query(), variables: vars } });
                self.next_search = now + SEARCH;
            }
        }
        let want = app.selected().map(|i| i.id.clone());
        if want != self.asked {
            if let Some(id) = &want {
                let vars = format!("{{\"id\":{}}}", json_str(id));
                worker.send(Job { key: format!("detail:{id}"), call: Call::Graphql { query: detail_query().into(), variables: vars } });
            }
            self.asked = want;
        }
        let wake = self.next_poll.min(self.next_search);
        ctx.request_repaint_after(wake.saturating_duration_since(now));
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
