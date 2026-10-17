//! config.toml: minato's own settings (`Settings`) in `app_dir("minato")`,
//! written key by key so what a person wrote around them stays, and the
//! one-time move from the files minato used before v0.61.0
//! (`minato-settings.json`, `minato-searches.txt`, `minato-marks.json`
//! beside common.toml).

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use minato_core::prefs::Settings;

/// config.toml in `app_dir`.
pub fn path(app_dir: &Path) -> PathBuf {
    app_dir.join("config.toml")
}

/// marks.json in the state folder: written by minato alone, so state.
pub fn marks_path(state_dir: &Path) -> PathBuf {
    state_dir.join("marks.json")
}

/// `text` with `s` written into it: only the keys minato owns change, so
/// comments, order and other keys stay. An error, and nothing to write,
/// when `text` is not TOML.
pub fn apply(text: &str, s: &Settings) -> Result<String, String> {
    let mut t = text.to_owned();
    for (table, key, value) in s.toml_keys() {
        t = ito_common::set_key(&t, table, key, &value)?;
    }
    // The single webhook of before is the first table now.
    for key in ["webhook", "webhook_format", "webhook_states"] {
        t = ito_common::remove_key(&t, Some("notify"), key)?;
    }
    ito_common::set_tables(&t, "notify.webhooks", &s.toml_hooks())
}

/// A thread that writes each `Settings` it is sent into config.toml at
/// `path`, on what is there; the last one of a burst. A file that does not
/// read as TOML is left alone.
pub fn writer(path: PathBuf) -> Sender<Settings> {
    let (tx, rx) = std::sync::mpsc::channel::<Settings>();
    std::thread::spawn(move || {
        while let Ok(mut s) = rx.recv() {
            while let Ok(newer) = rx.try_recv() {
                s = newer;
            }
            if let Ok(next) = ito_common::read_or_empty(&path).and_then(|old| apply(&old, &s)) {
                let _ = ito_common::write_atomic(&path, next);
            }
        }
    });
    tx
}

/// What config.toml at `path` says; the defaults when it is not there.
#[cfg(test)]
pub fn read(path: &Path) -> Settings {
    std::fs::read_to_string(path).map(|t| Settings::parse_toml(&t)).unwrap_or_default()
}

/// `<name>.bak-<time>` beside `p`.
pub fn aside(p: &Path, stamp: u64) -> PathBuf {
    let mut name = p.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(format!(".bak-{stamp}"));
    p.with_file_name(name)
}

/// Seconds since the epoch, for `.bak-<time>`.
pub fn stamp() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Move the files of before v0.61.0 to where they are now, when the new
/// ones are not there (nothing new is overwritten):
/// - `minato-settings.json` and `minato-searches.txt` (in `base`) become
///   `config.toml` in `app_dir`, and are renamed `.bak-<time>`;
/// - `minato-marks.json` becomes `marks.json` in `state_dir`.
///
/// What was moved, in a line to toast; `None` when there was nothing to do
/// or it could not be done (the old files then stay).
pub fn migrate(base: &Path, app_dir: &Path, state_dir: Option<&Path>) -> Option<String> {
    let mut moved = Vec::new();
    let new = path(app_dir);
    let json = base.join("minato-settings.json");
    let searches = base.join("minato-searches.txt");
    if !new.exists() && (json.exists() || searches.exists()) {
        let mut s = std::fs::read_to_string(&json).map(|t| Settings::parse(&t)).unwrap_or_default();
        if let Ok(t) = std::fs::read_to_string(&searches) {
            s.searches = t.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect();
        }
        if apply("", &s).and_then(|text| ito_common::write_atomic(&new, text)).is_ok() {
            let stamp = stamp();
            for old in [json, searches] {
                if old.exists() && std::fs::rename(&old, aside(&old, stamp)).is_ok() {
                    moved.push(old.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
                }
            }
        }
    }
    let marks = base.join("minato-marks.json");
    if let Some(to) = state_dir.map(marks_path) {
        if marks.exists() && !to.exists() {
            let _ = std::fs::create_dir_all(to.parent()?);
            if std::fs::rename(&marks, &to).is_ok() {
                moved.push("minato-marks.json".into());
            }
        }
    }
    (!moved.is_empty()).then(|| moved.join(", "))
}

/// The webhook addresses of config.toml `text`, in order.
fn urls(text: &str) -> Vec<String> {
    Settings::parse_toml(text).notify.hooks.into_iter().map(|h| h.url).collect()
}

/// config.toml `text` with every webhook address emptied: those are as good
/// as a password (anyone with one can post), so an export leaves them out.
/// Comments are lost in what comes back. The same text when it is not TOML.
pub fn blank_urls(text: &str) -> String {
    with_urls(text, |_, _| String::new())
}

/// config.toml `text` (just imported) with the webhook addresses that are
/// empty in it filled from `old`'s, place by place.
pub fn restore_urls(text: &str, old: &str) -> String {
    let old = urls(old);
    with_urls(text, |k, url| if url.trim().is_empty() { old.get(k).cloned().unwrap_or_default() } else { url.to_owned() })
}

fn with_urls(text: &str, url: impl Fn(usize, &str) -> String) -> String {
    let Ok(mut doc) = text.parse::<toml::Table>() else { return text.to_owned() };
    if let Some(hooks) = doc.get_mut("notify").and_then(|n| n.get_mut("webhooks")).and_then(toml::Value::as_array_mut) {
        for (k, h) in hooks.iter_mut().enumerate() {
            if let Some(t) = h.as_table_mut() {
                let now = t.get("url").and_then(toml::Value::as_str).unwrap_or_default().to_owned();
                t.insert("url".into(), toml::Value::String(url(k, &now)));
            }
        }
    }
    toml::to_string(&doc).unwrap_or_else(|_| text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use minato_core::prefs::Hook;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("minato-config-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn sample() -> Settings {
        let mut s =
            Settings { owner: "foo".into(), owners: vec!["foo".into()], searches: vec!["is:open label:bug".into()], ..Settings::default() };
        s.rules.0[6] = true;
        s.notify.system = [true, false, true];
        s.notify.sounds[1] = "low".into();
        s.notify.hooks = vec![Hook { url: "https://hooks.example/a".into(), format: "slack".into(), on: [false, true, true] }];
        s
    }

    #[test]
    fn settings_written_read_back_and_a_hand_edit_survives() {
        let s = sample();
        let text = apply("# mine\nextra = 1\n", &s).unwrap();
        assert_eq!(Settings::parse_toml(&text), s);
        assert!(text.starts_with("# mine\n") && text.contains("extra = 1"), "{text}");
        assert!(text.contains("[notify]") && text.contains("[[notify.webhooks]]"), "{text}");
        assert!(text.contains("system = [\"passed\", \"cancelled\"]") && text.contains("focus_mode = true"), "{text}");
        assert!(text.contains("sound_failed = \"low\"") && text.contains("states = [\"failed\", \"cancelled\"]"), "{text}");
        // Written again with a change: the comment and the other key stay.
        let mut s2 = s.clone();
        s2.notify.hooks.clear();
        s2.auto_done = true;
        let next = apply(&text, &s2).unwrap();
        assert_eq!(Settings::parse_toml(&next), s2);
        assert!(next.starts_with("# mine\n") && !next.contains("notify.webhooks"), "{next}");
        assert!(apply("not = [toml", &s).is_err());
    }

    #[test]
    fn the_old_files_move_once_and_nothing_new_is_overwritten() {
        let base = temp("migrate");
        let (app, state) = (base.join("minato"), base.join("state"));
        let old = sample();
        std::fs::write(base.join("minato-settings.json"), old.to_json()).unwrap();
        std::fs::write(base.join("minato-searches.txt"), "is:open\n\nlabel:bug\n").unwrap();
        std::fs::write(base.join("minato-marks.json"), "{}").unwrap();
        let said = migrate(&base, &app, Some(&state)).unwrap();
        assert!(said.contains("minato-settings.json") && said.contains("minato-marks.json"), "{said}");
        let got = read(&path(&app));
        assert_eq!(got.notify, old.notify);
        assert_eq!((got.owner.as_str(), got.searches.as_slice()), ("foo", ["is:open".to_string(), "label:bug".to_string()].as_slice()));
        assert!(marks_path(&state).exists() && !base.join("minato-marks.json").exists() && !base.join("minato-settings.json").exists());
        let baks = std::fs::read_dir(&base).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().contains(".bak-")).count();
        assert_eq!(baks, 2);
        // The second time there is nothing to do, and a new file is never replaced.
        assert_eq!(migrate(&base, &app, Some(&state)), None);
        std::fs::write(base.join("minato-settings.json"), "{\"owner\":\"other\"}").unwrap();
        assert_eq!(migrate(&base, &app, Some(&state)), None);
        assert_eq!(read(&path(&app)).owner, "foo");
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn export_leaves_the_webhook_addresses_out_and_import_keeps_the_ones_here() {
        let mut s = sample();
        s.notify.hooks.push(Hook { url: "https://hooks.example/b".into(), format: "json".into(), on: [true; 3] });
        let text = apply("", &s).unwrap();
        let blank = blank_urls(&text);
        assert!(!blank.contains("hooks.example"), "{blank}");
        let back = Settings::parse_toml(&blank);
        assert_eq!(back.notify.hooks.len(), 2);
        assert_eq!(back.notify.hooks[1].format, "json");
        // Imported on top of what is here: the addresses come back place by place.
        let kept = Settings::parse_toml(&restore_urls(&blank, &text));
        assert_eq!(kept, s);
        // An address the bundle does have is the bundle's.
        let other = text.replace("hooks.example/a", "elsewhere.example/a");
        assert_eq!(Settings::parse_toml(&restore_urls(&other, &text)).notify.hooks[0].url, "https://elsewhere.example/a");
        assert_eq!(blank_urls("not = [toml"), "not = [toml");
    }
}
