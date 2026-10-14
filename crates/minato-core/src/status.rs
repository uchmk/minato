//! The words of the keybar's right side: when the last sync was, and how
//! much of the API's rate limit is left.

/// "12s 前" / "12s ago": how long since the last sync, or "—" before the
/// first.
pub fn ago(secs: Option<u64>, ja: bool) -> String {
    let Some(s) = secs else { return "—".into() };
    let (n, unit) = match s {
        0..=59 => (s, "s"),
        60..=3599 => (s / 60, "m"),
        3600..=86399 => (s / 3600, "h"),
        _ => (s / 86400, "d"),
    };
    if ja {
        format!("{n}{unit} 前")
    } else {
        format!("{n}{unit} ago")
    }
}

/// "取得中 3 / 12" / "Fetching 3 / 12": the answers of a sync asked for by
/// hand (F5) that have come, of those asked.
pub fn fetching(done: usize, total: usize, ja: bool) -> String {
    format!("{} {done} / {total}", if ja { "取得中" } else { "Fetching" })
}

/// "API 4,812/5,000": the rate limit's remaining and total, or "API —".
pub fn api(limit: Option<(u32, u32)>) -> String {
    match limit {
        Some((left, total)) => format!("API {}/{}", thousands(left), thousands(total)),
        None => "API —".into(),
    }
}

/// `4812` as "4,812".
pub fn thousands(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ago_picks_the_unit() {
        assert_eq!(ago(Some(12), true), "12s 前");
        assert_eq!(ago(Some(12), false), "12s ago");
        assert_eq!(ago(Some(59), true), "59s 前");
        assert_eq!(ago(Some(60), true), "1m 前");
        assert_eq!(ago(Some(7200), false), "2h ago");
        assert_eq!(ago(Some(200_000), false), "2d ago");
        assert_eq!(ago(None, true), "—");
    }

    #[test]
    fn fetching_counts_the_answers() {
        assert_eq!(fetching(3, 12, true), "取得中 3 / 12");
        assert_eq!(fetching(0, 0, false), "Fetching 0 / 0");
    }

    #[test]
    fn api_has_thousands_separators() {
        assert_eq!(api(Some((4812, 5000))), "API 4,812/5,000");
        assert_eq!(api(Some((7, 60))), "API 7/60");
        assert_eq!(api(None), "API —");
        assert_eq!(thousands(1_234_567), "1,234,567");
        assert_eq!(thousands(0), "0");
    }
}
