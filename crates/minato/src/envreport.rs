//! `minato env`: what a bug report would otherwise have to ask for (the other apps' `env`).
//! Printed, not opened in a browser, and it never opens a window.

use ito_common::report::{files_rows, finish, row, section, variables, version_rows, Rows};

/// The whole report.
pub fn text() -> String {
    let mut out = String::new();
    section(&mut out, "Minato", &version_rows(env!("CARGO_PKG_VERSION"), &ito_bugreport::os_line()));
    section(&mut out, "Files", &files());
    section(&mut out, "Last run", &last_run());
    section(&mut out, "GitHub", &github());
    section(&mut out, "Variables", &variables(&["UCHMK_CONFIG_DIR", "GITHUB_TOKEN", "GH_TOKEN", "WGPU_BACKEND", "LANG"], "(not set)"));
    finish(&mut out);
    out
}

fn files() -> Rows {
    let mut rows = files_rows("minato");
    if let Some(dir) = ito_common::app_dir("minato") {
        rows.push(row("config.toml", ito_common::report::presence(&crate::config::path(&dir))));
    }
    rows
}

fn last_run() -> Rows {
    let Some(run) = ito_common::runinfo::load("minato") else {
        return vec![row("Last run", "no record (minato has not opened a window here)")];
    };
    let mut rows = vec![row("Version", run.version.clone())];
    if let Some([adapter, backend, device]) = run.rendering() {
        rows.push(row("Adapter", adapter));
        rows.push(row("Backend", backend));
        rows.push(row("Device", device));
    }
    if let Some(line) = run.window_line() {
        rows.push(row("Window", line));
    }
    rows
}

/// Where the token comes from, never the token itself.
fn github() -> Rows {
    let said = match minato_core::github::token::find() {
        Some((_, minato_core::github::token::Source::Env)) => "GITHUB_TOKEN or GH_TOKEN",
        Some((_, minato_core::github::token::Source::Gh)) => "gh auth token",
        None => "none (set GITHUB_TOKEN or run `gh auth login`)",
    };
    vec![row("Token", said)]
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_report_is_long_enough_and_never_holds_a_token() {
        let t = super::text();
        assert!(t.lines().count() >= 10, "{t}");
        assert!(t.starts_with("Minato\n") && t.contains("Version") && t.ends_with("\n") && !t.ends_with("\n\n"));
        for v in ["GITHUB_TOKEN", "GH_TOKEN"] {
            if let Some(tok) = std::env::var(v).ok().filter(|t| t.len() > 8) {
                assert!(!t.contains(&tok));
            }
        }
    }
}
