//! The thread the UI asks GitHub through. The UI sends a `Job` and later
//! takes `Reply`s with `try_recv`; it never waits. A job queued behind a
//! newer one with the same key is dropped (the old answer would be stale).
//! A job sent `first` goes before the others waiting; jobs sent into a slot
//! for one thing (a run, a card) are dropped once the slot is asked for
//! another while they still wait.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use super::client::{Client, Fetched};
use super::http::Transport;
use super::token::Token;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    Get(String),
    Graphql {
        query: String,
        variables: String,
    },
    /// A write; its key must be its own (same key = dropped as stale).
    Send {
        method: super::http::Method,
        path: String,
        body: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Job {
    /// What the answer is for; the UI picks it. Same key = same question.
    pub key: String,
    pub call: Call,
}

#[derive(Debug)]
pub struct Reply {
    pub key: String,
    pub result: Result<Fetched, String>,
}

pub struct Worker {
    jobs: Sender<Queued>,
    replies: Receiver<Reply>,
}

/// A job as it waits: whether it goes first, and its slot and what for.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Queued {
    job: Job,
    first: bool,
    slot: Option<(String, String)>,
}

impl Worker {
    /// Start the thread. `wake` is called after each reply (the UI repaints).
    pub fn spawn<T: Transport>(client: Client<T>, wake: impl Fn() + Send + 'static) -> Worker {
        let (jobs, job_rx) = mpsc::channel::<Queued>();
        let (reply_tx, replies) = mpsc::channel::<Reply>();
        let _ = thread::Builder::new().name("github".into()).spawn(move || run(client, job_rx, reply_tx, wake));
        Worker { jobs, replies }
    }

    pub fn send(&self, job: Job) {
        let _ = self.jobs.send(Queued { job, first: false, slot: None });
    }

    /// Before the jobs already waiting (what the open view shows).
    pub fn send_first(&self, job: Job) {
        let _ = self.jobs.send(Queued { job, first: true, slot: None });
    }

    /// Into `slot`, for `tag`: the jobs still waiting in it for another tag
    /// are dropped (the run or card they were for is no longer looked at).
    pub fn send_in(&self, slot: &str, tag: &str, job: Job) {
        let _ = self.jobs.send(Queued { job, first: false, slot: Some((slot.into(), tag.into())) });
    }

    pub fn try_recv(&self) -> Option<Reply> {
        self.replies.try_recv().ok()
    }
}

/// Of the jobs waiting, keep the newest of each key and, in each slot, the
/// ones for the slot's newest tag; those sent first go first, else in order.
fn latest(mut waiting: Vec<Queued>) -> Vec<Queued> {
    let mut seen = HashSet::new();
    let mut slots: HashMap<String, String> = HashMap::new();
    waiting.reverse();
    waiting.retain(|q| {
        let mine = q.slot.as_ref().is_none_or(|(s, t)| slots.entry(s.clone()).or_insert_with(|| t.clone()) == t);
        mine && seen.insert(q.job.key.clone())
    });
    waiting.reverse();
    waiting.sort_by_key(|q| !q.first);
    waiting
}

fn run<T: Transport>(mut client: Client<T>, jobs: Receiver<Queued>, replies: Sender<Reply>, wake: impl Fn()) {
    let mut waiting: Vec<Queued> = Vec::new();
    loop {
        // What came while the last job ran is sorted in before the next.
        if waiting.is_empty() {
            let Ok(q) = jobs.recv() else { return };
            waiting.push(q);
        }
        waiting.extend(jobs.try_iter());
        waiting = latest(waiting);
        let job = waiting.remove(0).job;
        let result = match &job.call {
            Call::Get(path) => client.get(path),
            Call::Graphql { query, variables } => client.graphql(query, variables),
            Call::Send { method, path, body } => client.send(*method, path, body),
        };
        if replies.send(Reply { key: job.key, result }).is_err() {
            return;
        }
        wake();
    }
}

/// Look for the token without blocking the caller (it may run `gh`).
pub fn find_token(done: impl FnOnce(Option<(Token, super::token::Source)>) + Send + 'static) {
    let _ = thread::Builder::new().name("token".into()).spawn(move || done(super::token::find()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{Request, Response};
    use std::time::Duration;

    struct Echo;

    impl Transport for Echo {
        fn send(&mut self, _: &Token, req: &Request) -> Result<Response, String> {
            let body = match req {
                Request::Get { url, .. } => url.clone(),
                Request::Graphql { .. } => "g".into(),
                Request::Send { url, .. } => url.clone(),
            };
            Ok(Response { status: 200, etag: None, body, rate: None })
        }
    }

    fn job(key: &str, path: &str) -> Job {
        Job { key: key.into(), call: Call::Get(path.into()) }
    }

    fn q(key: &str, path: &str) -> Queued {
        Queued { job: job(key, path), first: false, slot: None }
    }

    fn keys(v: Vec<Queued>) -> Vec<String> {
        v.into_iter().map(|q| q.job.key).collect()
    }

    #[test]
    fn stale_jobs_with_the_same_key_are_dropped() {
        let kept = latest(vec![q("a", "/1"), q("b", "/2"), q("a", "/3")]);
        assert_eq!(kept, vec![q("b", "/2"), q("a", "/3")]);
    }

    #[test]
    fn jobs_sent_first_go_before_the_others_in_order() {
        let first = |k: &str| Queued { first: true, ..q(k, "/") };
        assert_eq!(keys(latest(vec![q("a", "/"), first("b"), q("c", "/"), first("d")])), ["b", "d", "a", "c"]);
    }

    #[test]
    fn a_slot_keeps_only_the_jobs_for_its_newest_tag() {
        let slot = |k: &str, t: &str| Queued { slot: Some(("run".into(), t.into())), ..q(k, "/") };
        let waiting =
            vec![slot("jobs:1", "1"), slot("arts:1", "1"), q("x", "/"), slot("jobs:2", "2"), slot("jobs:3", "3"), slot("arts:3", "3")];
        assert_eq!(keys(latest(waiting)), ["x", "jobs:3", "arts:3"]);
    }

    #[test]
    fn an_answer_comes_back_through_the_channel() {
        let w = Worker::spawn(Client::new(Echo, Token::new("t").unwrap()), || {});
        w.send(job("me", "/user"));
        let mut got = None;
        for _ in 0..200 {
            if let Some(r) = w.try_recv() {
                got = Some(r);
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let r = got.expect("no reply");
        assert_eq!(r.key, "me");
        assert_eq!(r.result.unwrap().body, "https://api.github.com/user");
    }
}
