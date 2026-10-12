//! GET with an ETag cache: a 304 hands back the copy we hold (and GitHub
//! does not count it against the limit); a 200 replaces the copy. The
//! clients of one app share the cache, and the app may fill it from disk so
//! a start's first ask can be a 304 too.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use super::http::{API, Request, Transport};
use super::rate::Rate;
use super::token::Token;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fetched {
    pub body: String,
    /// True when GitHub said 304 and the body is our copy.
    pub cached: bool,
    pub rate: Option<Rate>,
    /// The ETag the body came with (a GET's), to keep beside it.
    pub etag: Option<String>,
}

/// The ETag cache: url -> (etag, body).
pub type Etags = Arc<Mutex<HashMap<String, (String, String)>>>;

pub struct Client<T: Transport> {
    transport: T,
    token: Token,
    cache: Etags,
}

/// The URL of `path` ("/user" or a full URL).
pub fn url(path: &str) -> String {
    if path.starts_with("http") { path.to_string() } else { format!("{API}{path}") }
}

impl<T: Transport> Client<T> {
    pub fn new(transport: T, token: Token) -> Self {
        Self::sharing(transport, token, Etags::default())
    }

    /// A client whose ETag cache is `cache` (shared with others).
    pub fn sharing(transport: T, token: Token, cache: Etags) -> Self {
        Self { transport, token, cache }
    }

    fn held(&self, url: &str) -> Option<(String, String)> {
        self.cache.lock().unwrap_or_else(PoisonError::into_inner).get(url).cloned()
    }

    /// `path` is "/user" or a full URL.
    pub fn get(&mut self, path: &str) -> Result<Fetched, String> {
        let url = url(path);
        let etag = self.held(&url).map(|(e, _)| e);
        let r = self.transport.send(&self.token, &Request::Get { url: url.clone(), etag })?;
        match r.status {
            304 => match self.held(&url) {
                Some((etag, body)) => Ok(Fetched { body, cached: true, rate: r.rate, etag: Some(etag) }),
                None => Err("304 without a cached copy".into()),
            },
            200..=299 => {
                if let Some(e) = &r.etag {
                    self.cache.lock().unwrap_or_else(PoisonError::into_inner).insert(url, (e.clone(), r.body.clone()));
                }
                Ok(Fetched { body: r.body, cached: false, rate: r.rate, etag: r.etag })
            }
            401 => Err("GitHub refused the token (401)".into()),
            s => Err(format!("GitHub answered {s}: {}", first_line(&r.body))),
        }
    }

    /// Change something. `path` is "/repos/..." or a full URL; `body` is JSON.
    pub fn send(&mut self, method: super::http::Method, path: &str, body: &str) -> Result<Fetched, String> {
        let r = self.transport.send(&self.token, &Request::Send { method, url: url(path), body: body.to_string() })?;
        match r.status {
            200..=299 => Ok(Fetched { body: r.body, cached: false, rate: r.rate, etag: None }),
            401 => Err("GitHub refused the token (401)".into()),
            403 => Err(format!("GitHub said no (403): {}", first_line(&r.body))),
            s => Err(format!("GitHub answered {s}: {}", first_line(&r.body))),
        }
    }

    /// A GraphQL query; `variables` is a JSON object text (`{}` for none).
    pub fn graphql(&mut self, query: &str, variables: &str) -> Result<Fetched, String> {
        let vars: serde_json::Value = serde_json::from_str(variables).map_err(|e| e.to_string())?;
        let body = serde_json::json!({ "query": query, "variables": vars }).to_string();
        let r = self.transport.send(&self.token, &Request::Graphql { body })?;
        if !(200..=299).contains(&r.status) {
            return Err(format!("GitHub answered {}: {}", r.status, first_line(&r.body)));
        }
        Ok(Fetched { body: r.body, cached: false, rate: r.rate, etag: None })
    }
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").chars().take(120).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::Response;

    /// Answers from a script and remembers what it was asked.
    struct Fake {
        answers: Vec<Response>,
        asked: std::sync::Arc<std::sync::Mutex<Vec<Request>>>,
    }

    impl Transport for Fake {
        fn send(&mut self, _: &Token, req: &Request) -> Result<Response, String> {
            self.asked.lock().unwrap().push(req.clone());
            Ok(self.answers.remove(0))
        }
    }

    fn resp(status: u16, etag: Option<&str>, body: &str) -> Response {
        Response { status, etag: etag.map(str::to_string), body: body.into(), rate: None }
    }

    fn client(answers: Vec<Response>) -> (Client<Fake>, std::sync::Arc<std::sync::Mutex<Vec<Request>>>) {
        let asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        (Client::new(Fake { answers, asked: asked.clone() }, Token::new("t").unwrap()), asked)
    }

    #[test]
    fn a_304_hands_back_the_copy_and_sends_the_etag() {
        let (mut c, asked) = client(vec![resp(200, Some("\"a\""), "one"), resp(304, None, "")]);
        let first = c.get("/user").unwrap();
        assert_eq!((first.body.as_str(), first.cached), ("one", false));
        let second = c.get("/user").unwrap();
        assert_eq!((second.body.as_str(), second.cached), ("one", true));
        let asked = asked.lock().unwrap();
        assert_eq!(asked[0], Request::Get { url: "https://api.github.com/user".into(), etag: None });
        assert_eq!(asked[1], Request::Get { url: "https://api.github.com/user".into(), etag: Some("\"a\"".into()) });
        assert_eq!((first.etag.as_deref(), second.etag.as_deref()), (Some("\"a\""), Some("\"a\"")));
    }

    #[test]
    fn a_cache_filled_from_disk_is_shared_and_asked_with() {
        let etags = Etags::default();
        etags.lock().unwrap().insert(url("/user"), ("\"d\"".into(), "kept".into()));
        let asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut c = Client::sharing(Fake { answers: vec![resp(304, None, "")], asked: asked.clone() }, Token::new("t").unwrap(), etags.clone());
        let got = c.get("/user").unwrap();
        assert_eq!((got.body.as_str(), got.cached), ("kept", true));
        assert_eq!(asked.lock().unwrap()[0], Request::Get { url: "https://api.github.com/user".into(), etag: Some("\"d\"".into()) });
        let mut other = Client::sharing(Fake { answers: vec![resp(200, Some("\"e\""), "new")], asked }, Token::new("t").unwrap(), etags.clone());
        other.get("/user").unwrap();
        assert_eq!(etags.lock().unwrap()[&url("/user")], ("\"e\"".into(), "new".into()));
    }

    #[test]
    fn errors_say_the_status_without_the_token() {
        let (mut c, _) = client(vec![resp(401, None, "{}"), resp(404, None, "Not Found\nmore")]);
        assert_eq!(c.get("/user").unwrap_err(), "GitHub refused the token (401)");
        assert_eq!(c.get("/x").unwrap_err(), "GitHub answered 404: Not Found");
    }

    #[test]
    fn graphql_posts_the_query_and_variables() {
        let (mut c, asked) = client(vec![resp(200, None, "{}")]);
        c.graphql("query { viewer { login } }", "{\"n\":1}").unwrap();
        let Request::Graphql { body } = asked.lock().unwrap()[0].clone() else { panic!() };
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["variables"]["n"], 1);
        assert_eq!(v["query"], "query { viewer { login } }");
        assert!(c.graphql("q", "not json").is_err());
    }
}
