//! The access token: from `GITHUB_TOKEN` / `GH_TOKEN`, else `gh auth token`.
//! It is never written to a file or a log: `Debug` and `Display` hide it.

use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct Token(String);

impl Token {
    /// A token from text, trimmed; `None` when nothing is left.
    pub fn new(text: &str) -> Option<Token> {
        let t = text.trim();
        (!t.is_empty()).then(|| Token(t.to_string()))
    }

    /// The `Authorization` header's value. The only way out.
    pub fn bearer(&self) -> String {
        format!("Bearer {}", self.0)
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token(***)")
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

/// Where a token was found, for the settings screen to say (never the value).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Env,
    Gh,
}

/// The first token of `GITHUB_TOKEN`, `GH_TOKEN`, then `gh`'s. `env` and `gh`
/// are passed in so the order can be tested.
pub fn resolve(env: impl Fn(&str) -> Option<String>, gh: impl FnOnce() -> Option<String>) -> Option<(Token, Source)> {
    for name in ["GITHUB_TOKEN", "GH_TOKEN"] {
        if let Some(t) = env(name).and_then(|v| Token::new(&v)) {
            return Some((t, Source::Env));
        }
    }
    gh().and_then(|v| Token::new(&v)).map(|t| (t, Source::Gh))
}

/// The real lookup: the environment, then `gh auth token`. Runs a process, so
/// call it from a worker thread.
pub fn find() -> Option<(Token, Source)> {
    resolve(
        |n| std::env::var(n).ok(),
        || {
            let out = std::process::Command::new("gh").args(["auth", "token"]).stdin(std::process::Stdio::null()).output().ok()?;
            out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_and_display_hide_the_value() {
        let t = Token::new(" ghp_secret\n").unwrap();
        assert!(!format!("{t:?}{t}").contains("secret"));
        assert_eq!(t.bearer(), "Bearer ghp_secret");
        assert!(Token::new("  \n").is_none());
    }

    #[test]
    fn the_environment_wins_over_gh() {
        let env = |n: &str| (n == "GH_TOKEN").then(|| "from_env".to_string());
        let (t, s) = resolve(env, || Some("from_gh".into())).unwrap();
        assert_eq!((t.bearer().as_str(), s), ("Bearer from_env", Source::Env));
        let (t, s) = resolve(|_| None, || Some("from_gh\n".into())).unwrap();
        assert_eq!((t.bearer().as_str(), s), ("Bearer from_gh", Source::Gh));
        assert!(resolve(|_| Some(" ".into()), || None).is_none());
    }
}
