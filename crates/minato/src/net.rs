//! The link between the window and `minato-core`'s GitHub worker: find the
//! token off the UI thread, ask who it is, and keep the rate limit fresh.
//! Each frame `pump` takes what has arrived; it never waits.

use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use minato_core::github::token::Source;
use minato_core::github::types::{parse_rate_limit, parse_viewer};
use minato_core::github::worker::{self, Call, Job, Worker};
use minato_core::github::{Client, Rate, Token, UreqTransport};

use crate::app::App;

/// How often the rate limit is asked while plenty is left.
const POLL: Duration = Duration::from_secs(60);

type Found = Option<(Token, Source)>;

pub struct Net {
    token_rx: Receiver<Found>,
    worker: Option<Worker>,
    next_poll: Instant,
    rate: Option<Rate>,
}

impl Net {
    /// Start looking for the token (it may run `gh`).
    pub fn start(ctx: &egui::Context) -> Net {
        let (tx, token_rx) = mpsc::channel();
        let ctx = ctx.clone();
        worker::find_token(move |found| {
            let _ = tx.send(found);
            ctx.request_repaint();
        });
        Net { token_rx, worker: None, next_poll: Instant::now(), rate: None }
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
                            Ok(v) => app.viewer = Some(v.login),
                            Err(e) => app.say(e),
                        },
                        "rate" => {
                            if let Ok(r) = parse_rate_limit(&f.body) {
                                self.rate = Some(r);
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
        ctx.request_repaint_after(self.next_poll.saturating_duration_since(now));
    }
}

fn get(key: &str, path: &str) -> Job {
    Job { key: key.into(), call: Call::Get(path.into()) }
}
