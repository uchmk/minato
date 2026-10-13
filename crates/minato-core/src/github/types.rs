//! The few answers stage 2 reads: who the token belongs to, and the rate
//! limit. Parsed from the JSON GitHub sends (fixtures in `tests/fixtures/`).

use serde::Deserialize;

use super::rate::Rate;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Viewer {
    pub login: String,
    #[serde(default)]
    pub avatar_url: String,
}

pub fn parse_viewer(body: &str) -> Result<Viewer, String> {
    serde_json::from_str(body).map_err(|e| format!("unexpected /user answer: {e}"))
}

/// `GET /rate_limit`: the core REST bucket.
pub fn parse_rate_limit(body: &str) -> Result<Rate, String> {
    #[derive(Deserialize)]
    struct Bucket {
        limit: u32,
        remaining: u32,
        reset: u64,
    }
    #[derive(Deserialize)]
    struct Resources {
        core: Bucket,
    }
    #[derive(Deserialize)]
    struct Answer {
        resources: Resources,
    }
    let a: Answer = serde_json::from_str(body).map_err(|e| format!("unexpected /rate_limit answer: {e}"))?;
    let b = a.resources.core;
    Ok(Rate { remaining: b.remaining, limit: b.limit, reset: b.reset, bucket: crate::github::Bucket::Core })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_viewer_is_read_from_a_recorded_answer() {
        let v = parse_viewer(include_str!("../../tests/fixtures/user.json")).unwrap();
        assert_eq!(v.login, "octocat");
        assert!(v.avatar_url.ends_with(".png") || v.avatar_url.contains("avatars"));
        assert!(parse_viewer("{}").is_err());
    }

    #[test]
    fn the_core_bucket_is_the_rate() {
        let r = parse_rate_limit(include_str!("../../tests/fixtures/rate_limit.json")).unwrap();
        assert_eq!(r, Rate { remaining: 4812, limit: 5000, reset: 1_760_000_000, bucket: crate::github::Bucket::Core });
        assert!(parse_rate_limit("{}").is_err());
    }
}
