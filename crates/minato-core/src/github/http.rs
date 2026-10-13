//! One HTTP request and its answer, behind a trait so the client can be
//! tested without a network. `UreqTransport` is the real one (rustls).

use std::time::Duration;

use super::rate::Rate;
use super::token::Token;

pub const API: &str = "https://api.github.com";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// GET `url`, with the ETag of the copy we hold, if any.
    Get { url: String, etag: Option<String> },
    /// Change something: `method` `url` with a JSON `body`.
    Send { method: Method, url: String, body: String },
    /// POST a GraphQL query (`body` is the JSON document).
    Graphql { body: String },
}

/// The verbs minato writes with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Post,
    Put,
    Patch,
    Delete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub etag: Option<String>,
    pub body: String,
    pub rate: Option<Rate>,
}

pub trait Transport: Send + 'static {
    fn send(&mut self, token: &Token, req: &Request) -> Result<Response, String>;
}

pub struct UreqTransport {
    agent: ureq::Agent,
}

impl UreqTransport {
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            .user_agent(concat!("minato/", env!("CARGO_PKG_VERSION")))
            .build();
        Self { agent: config.into() }
    }
}

impl Default for UreqTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for UreqTransport {
    fn send(&mut self, token: &Token, req: &Request) -> Result<Response, String> {
        let auth = token.bearer();
        let result = match req {
            Request::Get { url, etag } => {
                let mut r = self.agent.get(url).header("Authorization", &auth).header("Accept", "application/vnd.github+json");
                if let Some(e) = etag {
                    r = r.header("If-None-Match", e);
                }
                r.call()
            }
            Request::Send { method, url, body } => {
                let json = |r: ureq::RequestBuilder<ureq::typestate::WithBody>| {
                    r.header("Authorization", &auth)
                        .header("Accept", "application/vnd.github+json")
                        .header("Content-Type", "application/json")
                        .send(body.as_bytes())
                };
                json(match method {
                    Method::Post => self.agent.post(url),
                    Method::Put => self.agent.put(url),
                    Method::Patch => self.agent.patch(url),
                    Method::Delete => self.agent.delete(url).force_send_body(),
                })
            }
            Request::Graphql { body } => self
                .agent
                .post(format!("{API}/graphql"))
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
                .send(body.as_bytes()),
        };
        // A transport failure's text names the host, never the header.
        let mut resp = result.map_err(|e| e.to_string())?;
        let header = |k: &str| resp.headers().get(k).and_then(|v| v.to_str().ok()).map(str::to_string);
        let etag = header("etag");
        let rate = Rate::from_headers(header);
        let status = resp.status().as_u16();
        let body = resp.body_mut().read_to_string().map_err(|e| e.to_string())?;
        Ok(Response { status, etag, body, rate })
    }
}
