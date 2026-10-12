//! The thread the UI asks GitHub through. The UI sends a `Job` and later
//! takes `Reply`s with `try_recv`; it never waits. A job queued behind a
//! newer one with the same key is dropped (the old answer would be stale).

use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use super::client::{Client, Fetched};
use super::http::Transport;
use super::token::Token;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    Get(String),
    Graphql { query: String, variables: String },
    /// A write; its key must be its own (same key = dropped as stale).
    Send { method: super::http::Method, path: String, body: String },
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
    jobs: Sender<Job>,
    replies: Receiver<Reply>,
}

impl Worker {
    /// Start the thread. `wake` is called after each reply (the UI repaints).
    pub fn spawn<T: Transport>(client: Client<T>, wake: impl Fn() + Send + 'static) -> Worker {
        let (jobs, job_rx) = mpsc::channel::<Job>();
        let (reply_tx, replies) = mpsc::channel::<Reply>();
        let _ = thread::Builder::new().name("github".into()).spawn(move || run(client, job_rx, reply_tx, wake));
        Worker { jobs, replies }
    }

    pub fn send(&self, job: Job) {
        let _ = self.jobs.send(job);
    }

    pub fn try_recv(&self) -> Option<Reply> {
        self.replies.try_recv().ok()
    }
}

/// Of the jobs waiting, keep the newest of each key, in order.
fn latest(mut waiting: Vec<Job>) -> Vec<Job> {
    let mut seen = HashSet::new();
    waiting.reverse();
    waiting.retain(|j| seen.insert(j.key.clone()));
    waiting.reverse();
    waiting
}

fn run<T: Transport>(mut client: Client<T>, jobs: Receiver<Job>, replies: Sender<Reply>, wake: impl Fn()) {
    while let Ok(first) = jobs.recv() {
        let mut waiting = vec![first];
        waiting.extend(jobs.try_iter());
        for job in latest(waiting) {
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

    #[test]
    fn stale_jobs_with_the_same_key_are_dropped() {
        let kept = latest(vec![job("a", "/1"), job("b", "/2"), job("a", "/3")]);
        assert_eq!(kept, vec![job("b", "/2"), job("a", "/3")]);
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
