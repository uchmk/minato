//! The API's rate limit, read from the `x-ratelimit-*` headers.

use std::time::Duration;

/// Which of GitHub's limits a reply counts against: REST and GraphQL each
/// have their own (search has a third, by the minute).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Bucket {
    #[default]
    Core,
    Graphql,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rate {
    pub remaining: u32,
    pub limit: u32,
    /// Unix seconds when the window resets.
    pub reset: u64,
    pub bucket: Bucket,
}

impl Rate {
    /// From a header lookup (names in lower case); `None` if any is missing.
    /// A reply without `x-ratelimit-resource` counts against REST's.
    pub fn from_headers(get: impl Fn(&str) -> Option<String>) -> Option<Rate> {
        let n = |k: &str| get(k)?.trim().parse::<u64>().ok();
        let bucket = match get("x-ratelimit-resource").as_deref().map(str::trim) {
            None | Some("core") => Bucket::Core,
            Some("graphql") => Bucket::Graphql,
            Some(_) => Bucket::Other,
        };
        Some(Rate { remaining: n("x-ratelimit-remaining")? as u32, limit: n("x-ratelimit-limit")? as u32, reset: n("x-ratelimit-reset")?, bucket })
    }

    /// Of REST's and GraphQL's, the one with the smaller share left: the one
    /// that runs out first.
    pub fn tighter(rates: &[Option<Rate>; 2]) -> Option<Rate> {
        let left = |r: &Rate| r.remaining as f64 / r.limit.max(1) as f64;
        rates.iter().flatten().copied().min_by(|a, b| left(a).total_cmp(&left(b)))
    }

    /// How long to wait between polls: `base` while plenty is left, longer as
    /// the remainder falls (below 20% twice, below 5% eight times), and until
    /// the reset when it is empty.
    pub fn interval(self, base: Duration, now: u64) -> Duration {
        if self.remaining == 0 {
            return Duration::from_secs(self.reset.saturating_sub(now).max(1));
        }
        let left = self.remaining as f64 / self.limit.max(1) as f64;
        if left < 0.05 {
            base * 8
        } else if left < 0.20 {
            base * 2
        } else {
            base
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rate(remaining: u32) -> Rate {
        Rate { remaining, limit: 5000, reset: 1000, bucket: Bucket::Core }
    }

    #[test]
    fn headers_are_read_or_none() {
        let h = |k: &str| match k {
            "x-ratelimit-remaining" => Some("4812".to_string()),
            "x-ratelimit-limit" => Some("5000".to_string()),
            "x-ratelimit-reset" => Some("1760000000".to_string()),
            _ => None,
        };
        assert_eq!(Rate::from_headers(h), Some(Rate { remaining: 4812, limit: 5000, reset: 1_760_000_000, bucket: Bucket::Core }));
        let g = |k: &str| if k == "x-ratelimit-resource" { Some("graphql".to_string()) } else { h(k) };
        assert_eq!(Rate::from_headers(g).map(|r| r.bucket), Some(Bucket::Graphql));
        let s = |k: &str| if k == "x-ratelimit-resource" { Some("search".to_string()) } else { h(k) };
        assert_eq!(Rate::from_headers(s).map(|r| r.bucket), Some(Bucket::Other));
        assert_eq!(Rate::from_headers(|_| None), None);
        assert_eq!(Rate::from_headers(|_| Some("x".into())), None);
    }

    #[test]
    fn the_tighter_of_rest_and_graphql_is_shown() {
        let g = Rate { bucket: Bucket::Graphql, ..rate(4600) };
        assert_eq!(Rate::tighter(&[Some(rate(4949)), Some(g)]), Some(g));
        assert_eq!(Rate::tighter(&[Some(rate(100)), Some(g)]), Some(rate(100)));
        assert_eq!(Rate::tighter(&[None, Some(g)]), Some(g));
        assert_eq!(Rate::tighter(&[None, None]), None);
    }

    #[test]
    fn the_interval_widens_as_the_limit_runs_out() {
        let base = Duration::from_secs(60);
        assert_eq!(rate(4000).interval(base, 0), base);
        assert_eq!(rate(900).interval(base, 0), base * 2);
        assert_eq!(rate(100).interval(base, 0), base * 8);
        assert_eq!(rate(0).interval(base, 940), Duration::from_secs(60));
        assert_eq!(rate(0).interval(base, 5000), Duration::from_secs(1));
    }
}
