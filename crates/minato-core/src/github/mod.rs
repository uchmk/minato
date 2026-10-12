//! Talking to GitHub, apart from egui: the token, the rate limit, the HTTP
//! transport, the ETag cache and the worker thread the UI asks through.
//! Nothing here is tested against the real service; the tests use recorded
//! JSON in `tests/fixtures/` and a fake transport.

pub mod client;
pub mod http;
pub mod rate;
pub mod search;
pub mod token;
pub mod types;
pub mod worker;

pub use client::{Client, Fetched};
pub use http::{Request, Response, Transport, UreqTransport};
pub use rate::Rate;
pub use token::Token;
