//! F12: open a bug report with the version, the OS and what minato knew
//! already filled in. The lines are `ito-bugreport`'s, the same in every
//! uchmk app; this file gives it minato's name, repository and rows.

use std::path::{Path, PathBuf};

#[cfg(test)]
use ito_bugreport::Field;
use ito_bugreport::{Context, Spec};

/// A fork should point this at its own tracker.
const REPO: &str = "https://github.com/uchmk/minato";

/// The template's filename, from `.github/ISSUE_TEMPLATE/`.
const TEMPLATE: &str = "bug_report.yml";

fn spec() -> Spec<'static> {
    Spec { app: "minato", version: env!("CARGO_PKG_VERSION"), repo: REPO, template: TEMPLATE }
}

/// The link that opens the report: the `version`, `os` and `context` fields
/// of the form filled in. `base` is the `uchmk` folder, `scale` common.toml's.
pub fn url(base: Option<&Path>, scale: f32) -> String {
    ito_bugreport::url(&spec(), &[("context", context(base, scale))])
}

fn context(base: Option<&Path>, scale: f32) -> String {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from);
    let files: Vec<PathBuf> =
        base.map(|b| [ito_common::common_path(b), crate::prefs::keymap_path(b)]).into_iter().flatten().filter(|p| p.is_file()).collect();
    ito_bugreport::context(&Context {
        home: home.as_deref(),
        scale: Some(scale),
        config: &files,
        anchors: &["uchmk", "minato"],
        ..Default::default()
    })
}

/// minato's own fields on the shared form: none yet.
#[cfg(test)]
const FIELDS: &[Field] = &[];

/// The whole `.github/ISSUE_TEMPLATE/bug_report.yml`, made from ito's shared form.
#[cfg(test)]
fn form() -> String {
    ito_bugreport::template("minato", FIELDS)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The form on disk is the one ito generates. To rewrite it after a
    /// change: `cargo test write_the_form -- --ignored`.
    const FORM: &str = include_str!("../../../.github/ISSUE_TEMPLATE/bug_report.yml");

    #[test]
    #[ignore = "rewrites .github/ISSUE_TEMPLATE/bug_report.yml"]
    fn write_the_form() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/ISSUE_TEMPLATE").join(TEMPLATE);
        std::fs::write(path, form()).unwrap();
    }

    #[test]
    fn the_form_on_disk_is_the_generated_one() {
        assert_eq!(FORM.replace("\r\n", "\n"), form(), "run `cargo test write_the_form -- --ignored`");
    }

    /// The ids `url` fills must exist in the form, or GitHub drops them silently.
    #[test]
    fn the_form_has_the_fields_the_link_fills() {
        let ids = ito_bugreport::field_ids(FORM);
        for id in ["version", "os", "context"] {
            assert!(ids.iter().any(|i| i == id), "bug_report.yml has no field `{id}`");
        }
        assert!(!FORM.contains("{{app}}"), "the template's placeholder was not replaced");
    }

    #[test]
    fn the_link_names_the_app_and_never_a_path() {
        let dir = std::env::temp_dir().join("minato-bugreport-test");
        let u = url(Some(&dir), 1.0);
        assert!(u.starts_with("https://github.com/uchmk/minato/issues/new?template=bug_report.yml&version=minato%20"), "{u}");
        assert!(u.contains("&context=") && u.contains("Config%3A%20defaults%20only"), "{u}");
        assert!(!u.contains("minato-bugreport-test"), "{u}");
    }
}
