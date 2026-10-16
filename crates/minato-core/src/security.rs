//! The security alerts of the watched repositories (docs/design.md, 「6 安全」):
//! code scanning, Dependabot and secret scanning. Read-only parsing, the order
//! the tab shows them in, the dismissal bodies, and the stub items that bring
//! critical and high alerts into the inbox (`id` starts with `sec:`).

use serde_json::{json, Value};

use crate::list::{Item, Kind};

/// Which scanner raised an alert.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    Code,
    Dependabot,
    Secret,
}

pub const SOURCES: [Source; 3] = [Source::Code, Source::Dependabot, Source::Secret];

impl Source {
    /// The word in ids and in the cache keys.
    pub fn word(self) -> &'static str {
        match self {
            Source::Code => "code",
            Source::Dependabot => "dep",
            Source::Secret => "secret",
        }
    }

    pub fn from_word(w: &str) -> Option<Source> {
        SOURCES.into_iter().find(|s| s.word() == w)
    }

    /// The i18n key of the short name.
    pub fn label(self) -> &'static str {
        match self {
            Source::Code => "sec.code",
            Source::Dependabot => "sec.dep",
            Source::Secret => "sec.secret",
        }
    }

    /// The REST path of the open alerts of `repo`.
    pub fn list_path(self, repo: &str) -> String {
        format!("/repos/{repo}/{}?state=open&per_page=100", self.segment())
    }

    fn segment(self) -> &'static str {
        match self {
            Source::Code => "code-scanning/alerts",
            Source::Dependabot => "dependabot/alerts",
            Source::Secret => "secret-scanning/alerts",
        }
    }

    /// The REST path of one alert, for the write.
    pub fn alert_path(self, repo: &str, number: u64) -> String {
        format!("/repos/{repo}/{}/{number}", self.segment())
    }
}

/// How bad, the worst first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Unknown,
}

impl Severity {
    fn parse(s: &str) -> Severity {
        match s {
            "critical" => Severity::Critical,
            "high" => Severity::High,
            "medium" | "moderate" | "error" => Severity::Medium,
            "low" | "warning" | "note" => Severity::Low,
            _ => Severity::Unknown,
        }
    }

    /// The English name, for the search box.
    pub fn word(self) -> &'static str {
        match self {
            Severity::Critical => "critical",
            Severity::High => "high",
            Severity::Medium => "medium",
            Severity::Low => "low",
            Severity::Unknown => "unknown",
        }
    }

    /// The i18n key of the name.
    pub fn label(self) -> &'static str {
        match self {
            Severity::Critical => "sec.critical",
            Severity::High => "sec.high",
            Severity::Medium => "sec.medium",
            Severity::Low => "sec.low",
            Severity::Unknown => "sec.unknown",
        }
    }

    /// Critical and high are the ones that go to the inbox and light the tab.
    pub fn urgent(self) -> bool {
        self <= Severity::High
    }
}

/// One open alert.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alert {
    pub repo: String,
    pub source: Source,
    pub number: u64,
    pub severity: Severity,
    /// The rule, the package or the kind of secret.
    pub what: String,
    /// A one-line explanation (may be empty).
    pub summary: String,
    /// Where: a file and line, or a manifest.
    pub place: String,
    pub created_at: String,
    pub url: String,
}

impl Alert {
    /// The id of the alert (`code:owner/repo#3`), also the stub item's tail.
    pub fn key(&self) -> String {
        format!("{}:{}#{}", self.source.word(), self.repo, self.number)
    }
}

fn text(v: &Value, p: &str) -> String {
    v.pointer(p).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn number(v: &Value) -> Option<u64> {
    v.get("number").and_then(Value::as_u64)
}

/// The alerts in the answer of one list call. An answer that is not a list
/// (a message such as "Advanced Security must be enabled") is an error.
pub fn parse(source: Source, repo: &str, body: &str) -> Result<Vec<Alert>, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let list = v.as_array().ok_or_else(|| text(&v, "/message"))?;
    Ok(list.iter().filter_map(|a| one(source, repo, a)).collect())
}

fn one(source: Source, repo: &str, a: &Value) -> Option<Alert> {
    let n = number(a)?;
    let (severity, what, summary, place) = match source {
        Source::Code => {
            let level = text(a, "/rule/security_severity_level");
            let sev = if level.is_empty() { text(a, "/rule/severity") } else { level };
            let path = text(a, "/most_recent_instance/location/path");
            let line = a.pointer("/most_recent_instance/location/start_line").and_then(Value::as_u64);
            let place = match line {
                Some(l) if !path.is_empty() => format!("{path}:{l}"),
                _ => path,
            };
            let id = text(a, "/rule/id");
            let desc = text(a, "/rule/description");
            (Severity::parse(&sev), if id.is_empty() { desc.clone() } else { id }, desc, place)
        }
        Source::Dependabot => {
            let name = text(a, "/dependency/package/name");
            let name = if name.is_empty() { text(a, "/security_vulnerability/package/name") } else { name };
            (
                Severity::parse(&text(a, "/security_advisory/severity")),
                name,
                text(a, "/security_advisory/summary"),
                text(a, "/dependency/manifest_path"),
            )
        }
        Source::Secret => {
            let name = text(a, "/secret_type_display_name");
            let name = if name.is_empty() { text(a, "/secret_type") } else { name };
            // A leaked secret has no score; treat it as high.
            (Severity::High, name, String::new(), String::new())
        }
    };
    Some(Alert {
        repo: repo.to_string(),
        source,
        number: n,
        severity,
        what,
        summary,
        place,
        created_at: text(a, "/created_at"),
        url: text(a, "/html_url"),
    })
}

/// The order the tab shows: worst first, then the oldest, then by repository.
pub fn sort(alerts: &mut [Alert]) {
    alerts.sort_by(|a, b| {
        a.severity
            .cmp(&b.severity)
            .then_with(|| a.created_at.cmp(&b.created_at))
            .then_with(|| a.repo.cmp(&b.repo))
            .then_with(|| a.source.cmp(&b.source))
            .then_with(|| a.number.cmp(&b.number))
    });
}

/// The places (indexes into `alerts`) of the alerts the search box's words
/// leave: each word in the repository, the rule or package, the summary, the
/// place, the severity or the kind.
pub fn shown(alerts: &[Alert], q: &crate::find::Query) -> Vec<usize> {
    (0..alerts.len())
        .filter(|&i| {
            let a = &alerts[i];
            q.takes_fields(&[&a.repo, &a.what, &a.summary, &a.place, a.severity.word(), a.source.word(), &format!("#{}", a.number)])
        })
        .collect()
}

/// How many alerts per severity, for the band at the top: critical, high, medium, low (unknown counts as low).
pub fn counts(alerts: &[Alert]) -> [usize; 4] {
    let mut c = [0; 4];
    for a in alerts {
        c[(a.severity as usize).min(3)] += 1;
    }
    c
}

/// The repositories that have alerts, with their number, the most first.
pub fn per_repo(alerts: &[Alert]) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for a in alerts {
        match out.iter_mut().find(|(r, _)| *r == a.repo) {
            Some((_, n)) => *n += 1,
            None => out.push((a.repo.clone(), 1)),
        }
    }
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    out
}

/// Whether any alert is critical or high (the tab's red dot).
pub fn any_urgent(alerts: &[Alert]) -> bool {
    alerts.iter().any(|a| a.severity.urgent())
}

/// Whether `item` stands for an alert, not for something on GitHub.
pub fn is_stub(item: &Item) -> bool {
    item.id.starts_with("sec:")
}

/// The alert key a stub stands for (`code:owner/repo#3`).
pub fn stub_key(item: &Item) -> Option<&str> {
    item.id.strip_prefix("sec:").filter(|_| is_stub(item))
}

/// A stub item for each critical or high alert, for the inbox.
pub fn stubs(alerts: &[Alert]) -> Vec<Item> {
    alerts
        .iter()
        .filter(|a| a.severity.urgent())
        .map(|a| {
            let sev = match a.severity {
                Severity::Critical => "Critical",
                _ => "High",
            };
            let mut title = format!("{sev} · {}", a.what);
            if !a.summary.is_empty() && a.summary != a.what {
                title.push_str(" — ");
                title.push_str(&a.summary);
            }
            Item {
                id: format!("sec:{}", a.key()),
                kind: Kind::Issue,
                number: 0,
                title,
                url: a.url.clone(),
                author: String::new(),
                labels: Vec::new(),
                assignees: Vec::new(),
                milestone: String::new(),
                reviewers: Vec::new(),
                comments: 0,
                created_at: a.created_at.clone(),
                updated_at: a.created_at.clone(),
                draft: false,
                merged: false,
                closed: false,
                conflict: false,
                mentioned: false,
                peek: a.place.clone(),
                head: String::new(),
                ci: None,
                repo: a.repo.clone(),
            }
        })
        .collect()
}

/// The reasons a person can give for dismissing an alert, as (i18n key, API word).
/// The first is the neutral "accept it" reason the confirm box names.
pub fn reasons(source: Source) -> &'static [(&'static str, &'static str)] {
    match source {
        Source::Code => &[("sec.r.wontfix", "won't fix"), ("sec.r.false", "false positive"), ("sec.r.tests", "used in tests")],
        Source::Dependabot => &[
            ("sec.r.tolerable", "tolerable_risk"),
            ("sec.r.false", "inaccurate"),
            ("sec.r.unused", "not_used"),
            ("sec.r.nobandwidth", "no_bandwidth"),
            ("sec.r.started", "fix_started"),
        ],
        Source::Secret => &[
            ("sec.r.wontfix", "wont_fix"),
            ("sec.r.false", "false_positive"),
            ("sec.r.revoked", "revoked"),
            ("sec.r.tests", "used_in_tests"),
        ],
    }
}

/// The reason dismissing with one key uses (the first of `reasons`).
pub fn default_reason(source: Source) -> (&'static str, &'static str) {
    reasons(source)[0]
}

/// The body of the PATCH that dismisses (or resolves) an alert for `reason` (an API word).
pub fn dismiss_body(source: Source, reason: &str) -> String {
    match source {
        Source::Secret => json!({"state": "resolved", "resolution": reason}),
        _ => json!({"state": "dismissed", "dismissed_reason": reason}),
    }
    .to_string()
}

/// The body of the PATCH that brings a dismissed alert back (the undo).
pub fn reopen_body() -> String {
    json!({"state": "open"}).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODE: &str = include_str!("../tests/fixtures/code_scanning_alerts.json");
    const DEP: &str = include_str!("../tests/fixtures/dependabot_alerts.json");
    const SECRET: &str = include_str!("../tests/fixtures/secret_scanning_alerts.json");

    #[test]
    fn code_scanning_alerts_are_read() {
        let v = parse(Source::Code, "u/r", CODE).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].number, 3);
        assert_eq!(v[0].severity, Severity::High);
        assert_eq!(v[0].what, "js/sql-injection");
        assert_eq!(v[0].place, "src/db.js:42");
        // No security level: the rule's own severity (warning) is low.
        assert_eq!(v[1].severity, Severity::Low);
        assert_eq!(v[1].what, "actions/missing-workflow-permissions");
        assert_eq!(v[1].url, "https://github.com/u/r/security/code-scanning/4");
    }

    #[test]
    fn dependabot_alerts_are_read() {
        let v = parse(Source::Dependabot, "u/r", DEP).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].severity, Severity::Critical);
        assert_eq!(v[0].what, "lodash");
        assert_eq!(v[0].place, "package-lock.json");
        assert_eq!(v[1].severity, Severity::Medium);
        assert_eq!(v[1].what, "left-pad");
    }

    #[test]
    fn secret_alerts_are_high() {
        let v = parse(Source::Secret, "u/r", SECRET).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].severity, Severity::High);
        assert_eq!(v[0].what, "GitHub Personal Access Token");
        assert_eq!(v[0].key(), "secret:u/r#1");
    }

    #[test]
    fn a_message_instead_of_a_list_is_an_error() {
        let e = parse(Source::Code, "u/r", r#"{"message":"Code scanning is not enabled for this repository."}"#).unwrap_err();
        assert!(e.contains("not enabled"));
        assert!(parse(Source::Code, "u/r", "nope").is_err());
        assert_eq!(parse(Source::Dependabot, "u/r", "[]").unwrap(), vec![]);
    }

    #[test]
    fn worst_first_then_oldest() {
        let mut v = parse(Source::Code, "u/r", CODE).unwrap();
        v.extend(parse(Source::Dependabot, "u/r", DEP).unwrap());
        v.extend(parse(Source::Secret, "u/a", SECRET).unwrap());
        sort(&mut v);
        let order: Vec<_> = v.iter().map(|a| a.key()).collect();
        assert_eq!(order[0], "dep:u/r#7");
        // The two high ones: the older code alert (2026-09) before the secret (2026-10).
        assert_eq!(order[1], "code:u/r#3");
        assert_eq!(order[2], "secret:u/a#1");
        assert_eq!(counts(&v), [1, 2, 1, 1]);
        assert!(any_urgent(&v));
        assert_eq!(per_repo(&v), vec![("u/r".to_string(), 4), ("u/a".to_string(), 1)]);
    }

    #[test]
    fn only_urgent_alerts_become_stubs() {
        let mut v = parse(Source::Dependabot, "u/r", DEP).unwrap();
        v.extend(parse(Source::Code, "u/r", CODE).unwrap());
        let s = stubs(&v);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].id, "sec:dep:u/r#7");
        assert!(s[0].title.starts_with("Critical · lodash — "));
        assert!(is_stub(&s[0]));
        assert_eq!(stub_key(&s[1]), Some("code:u/r#3"));
        assert_eq!(s[1].repo, "u/r");
        assert_eq!(s[1].url, "https://github.com/u/r/security/code-scanning/3");
    }

    #[test]
    fn bodies_follow_the_api() {
        assert_eq!(dismiss_body(Source::Code, "won't fix"), r#"{"dismissed_reason":"won't fix","state":"dismissed"}"#);
        assert_eq!(dismiss_body(Source::Secret, "revoked"), r#"{"resolution":"revoked","state":"resolved"}"#);
        assert_eq!(reopen_body(), r#"{"state":"open"}"#);
        for s in SOURCES {
            assert!(!reasons(s).is_empty());
            assert_eq!(Source::from_word(s.word()), Some(s));
            assert!(s.list_path("u/r").starts_with("/repos/u/r/"));
        }
    }
}
