//! minato's own settings: whose repositories are shown, which inbox rules
//! are on, which fields the writing box offers and how a watched run's end
//! is told. Kept as a small JSON file beside common.toml.

use serde_json::{json, Value};

use crate::actions::Status;
use crate::inbox::Rules;

/// The writing box's fields, in the order of the keys `Alt+L A S M P R`.
pub const FIELDS: usize = 6;

/// How many owners shown before are kept for switching back.
pub const OWNERS: usize = 8;

/// The sounds that can be picked, by their words (`ito_notify::SOUNDS`).
pub const SOUNDS: [&str; 4] = ["chime", "low", "alert", "default"];

/// The forms a webhook's address takes, by their words (`ito_notify::FORMATS`).
pub const FORMATS: [&str; 3] = ["ntfy", "slack", "json"];

/// Whether `url` is an address a webhook may be sent to (`ito_notify::is_http`).
pub fn is_http(url: &str) -> bool {
    let u = url.trim();
    u.starts_with("http://") || u.starts_with("https://")
}

/// How a run watched with `w` tells that it has ended, by how it ended:
/// passed, failed, cancelled (see [`Notify::slot`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notify {
    /// The system's notification.
    pub system: [bool; 3],
    /// A sound.
    pub sound: [bool; 3],
    /// Which sound (a word of [`SOUNDS`]).
    pub sounds: [String; 3],
    /// No sound in Windows' focus mode, a presentation or a full-screen game.
    pub focus: bool,
    /// A message to `webhook_url`.
    pub webhook: [bool; 3],
    /// An ntfy topic, a Slack webhook or any address that takes JSON;
    /// nothing is sent while it is not an http(s) address.
    pub webhook_url: String,
    /// The form the address takes (a word of [`FORMATS`]).
    pub webhook_format: String,
}

impl Default for Notify {
    fn default() -> Self {
        Notify {
            system: [true; 3],
            sound: [true; 3],
            sounds: ["chime", "alert", "chime"].map(String::from),
            focus: true,
            webhook: [true; 3],
            webhook_url: String::new(),
            webhook_format: "ntfy".into(),
        }
    }
}

impl Notify {
    /// The column a run's end is in: 0 passed, 1 failed, 2 cancelled
    /// (skipped and the like too).
    pub fn slot(s: Status) -> usize {
        match s {
            Status::Ok => 0,
            Status::Bad => 1,
            _ => 2,
        }
    }

    /// The webhook's address and form for a run that ended in `slot`, when
    /// one is to be sent there.
    pub fn hook(&self, slot: usize) -> Option<(&str, &str)> {
        (self.webhook[slot] && is_http(&self.webhook_url)).then(|| (self.webhook_url.trim(), self.webhook_format.as_str()))
    }

    fn to_json(&self) -> Value {
        json!({
            "system": self.system,
            "sound": self.sound,
            "sounds": self.sounds,
            "focus": self.focus,
            "webhook": self.webhook,
            "webhook_url": self.webhook_url,
            "webhook_format": self.webhook_format,
        })
    }

    fn parse(v: &Value) -> Notify {
        let mut n = Notify::default();
        for (key, out) in [("system", &mut n.system), ("sound", &mut n.sound), ("webhook", &mut n.webhook)] {
            if let Some(list) = v.get(key).and_then(Value::as_array) {
                for (slot, x) in out.iter_mut().zip(list) {
                    if let Some(b) = x.as_bool() {
                        *slot = b;
                    }
                }
            }
        }
        if let Some(list) = v.get("sounds").and_then(Value::as_array) {
            for (slot, x) in n.sounds.iter_mut().zip(list) {
                if let Some(s) = x.as_str().filter(|s| SOUNDS.contains(s)) {
                    *slot = s.to_string();
                }
            }
        }
        if let Some(b) = v.get("focus").and_then(Value::as_bool) {
            n.focus = b;
        }
        if let Some(s) = v.get("webhook_url").and_then(Value::as_str) {
            n.webhook_url = s.trim().to_string();
        }
        if let Some(s) = v.get("webhook_format").and_then(Value::as_str).filter(|s| FORMATS.contains(s)) {
            n.webhook_format = s.to_string();
        }
        n
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// The user or organization whose repositories, searches and Project
    /// are shown (a login); empty for the viewer.
    pub owner: String,
    /// The owners shown before (logins, the latest first), for switching
    /// back from the palette; the viewer is not among them.
    pub owners: Vec<String>,
    /// Other owners shown alongside `owner` (logins; none when `owner` is
    /// empty): their repositories, items and runs join the owner's.
    pub also: Vec<String>,
    /// The title of the Project the board shows (the first when none has it).
    pub project: String,
    pub rules: Rules,
    /// Labels, Assignees, Status, Milestone, Priority, Reviewers.
    pub fields: [bool; FIELDS],
    /// The fields in the order the writing box lists them (indexes into `fields`).
    pub order: [usize; FIELDS],
    pub notify: Notify,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            owner: String::new(),
            owners: Vec::new(),
            also: Vec::new(),
            project: crate::board::TITLE.into(),
            rules: Rules::default(),
            fields: [true, true, true, false, false, false],
            order: [0, 1, 2, 3, 4, 5],
            notify: Notify::default(),
        }
    }
}

impl Settings {
    pub fn to_json(&self) -> String {
        json!({ "owner": self.owner, "owners": self.owners, "also": self.also, "project": self.project, "rules": self.rules.0, "rule_order": self.rules.1, "fields": self.fields, "order": self.order, "notify": self.notify.to_json() }).to_string()
    }

    /// What `text` says; anything missing or unreadable keeps its default.
    pub fn parse(text: &str) -> Settings {
        let mut s = Settings::default();
        let Ok(v) = serde_json::from_str::<Value>(text) else { return s };
        if let Some(o) = v.get("owner").and_then(Value::as_str) {
            s.owner = login(o);
        }
        if let Some(list) = v.get("owners").and_then(Value::as_array) {
            for o in list.iter().filter_map(Value::as_str).map(login) {
                if !o.is_empty() && !s.owners.iter().any(|x| x.eq_ignore_ascii_case(&o)) && s.owners.len() < OWNERS {
                    s.owners.push(o);
                }
            }
        }
        if !s.owner.is_empty() && !s.owners.iter().any(|x| x.eq_ignore_ascii_case(&s.owner)) {
            s.owners.insert(0, s.owner.clone());
            s.owners.truncate(OWNERS);
        }
        if let Some(list) = v.get("also").and_then(Value::as_array).filter(|_| !s.owner.is_empty()) {
            for o in list.iter().filter_map(Value::as_str) {
                s.add_also(o);
            }
        }
        if let Some(p) = v.get("project").and_then(Value::as_str) {
            s.project = p.trim().to_string();
        }
        let fill = |key: &str, out: &mut [bool]| {
            if let Some(list) = v.get(key).and_then(Value::as_array) {
                for (slot, x) in out.iter_mut().zip(list) {
                    if let Some(b) = x.as_bool() {
                        *slot = b;
                    }
                }
            }
        };
        fill("rules", &mut s.rules.0);
        fill("fields", &mut s.fields);
        let perm = |key: &str, out: &mut [usize]| {
            let Some(list) = v.get(key).and_then(Value::as_array) else { return };
            let order: Vec<usize> = list.iter().filter_map(|x| x.as_u64().map(|n| n as usize)).collect();
            let mut seen = vec![false; out.len()];
            if order.len() == out.len() && order.iter().all(|&k| k < out.len() && !std::mem::replace(&mut seen[k], true)) {
                out.copy_from_slice(&order);
            }
        };
        perm("order", &mut s.order);
        perm("rule_order", &mut s.rules.1);
        if let Some(n) = v.get("notify") {
            s.notify = Notify::parse(n);
        }
        s
    }

    /// Show `text`'s owners (empty for the viewer): logins apart by commas
    /// or spaces, the first the owner (to the front of the owners shown
    /// before), the others shown alongside it.
    pub fn set_owner(&mut self, text: &str) {
        let mut names = text.split([',', ' ', '\t']).map(login).filter(|o| !o.is_empty());
        self.owner = names.next().unwrap_or_default();
        self.also.clear();
        if !self.owner.is_empty() {
            self.owners.retain(|x| !x.eq_ignore_ascii_case(&self.owner));
            self.owners.insert(0, self.owner.clone());
            self.owners.truncate(OWNERS);
        }
        for o in names {
            self.add_also(&o);
        }
    }

    /// Show `text`'s owner alongside the owner (not the owner itself, not
    /// twice). Whether it was added.
    pub fn add_also(&mut self, text: &str) -> bool {
        let o = login(text);
        if o.is_empty() || o.eq_ignore_ascii_case(&self.owner) || self.also.iter().any(|x| x.eq_ignore_ascii_case(&o)) {
            return false;
        }
        self.also.push(o);
        true
    }

    /// The owners as the settings' box shows them: "uchmk, rust-lang".
    pub fn owners_line(&self) -> String {
        std::iter::once(&self.owner).chain(&self.also).filter(|o| !o.is_empty()).cloned().collect::<Vec<_>>().join(", ")
    }

    /// Move the field at place `at` of the order one place (`-1` up, `1` down).
    pub fn shift(&mut self, at: usize, step: isize) {
        let to = at as isize + step;
        if (0..FIELDS as isize).contains(&to) {
            self.order.swap(at, to as usize);
        }
    }
}

/// A login from what was typed: `@name`, `name` or the account's URL; what
/// a login cannot hold is dropped.
pub fn login(text: &str) -> String {
    let last = text.trim().trim_end_matches('/').rsplit('/').next().unwrap_or_default();
    last.trim_start_matches('@').chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_keep_defaults_for_the_unreadable() {
        let mut s = Settings::default();
        s.rules.0[6] = true;
        s.fields[4] = true;
        assert_eq!(Settings::parse(&s.to_json()), s);
        assert_eq!(Settings::parse("nonsense"), Settings::default());
        let odd = Settings::parse(r#"{"fields":[false,"x"],"rules":[]}"#);
        assert!(!odd.fields[0] && odd.fields[1]);
    }

    #[test]
    fn the_owner_and_the_project_are_kept_and_the_owner_is_a_login() {
        let s = Settings { owner: "rust-lang".into(), owners: vec!["rust-lang".into()], project: "Roadmap".into(), ..Settings::default() };
        assert_eq!(Settings::parse(&s.to_json()), s);
        // Before they were settings: the viewer and "uchmk".
        let old = Settings::parse(r#"{"fields":[true]}"#);
        assert_eq!((old.owner.as_str(), old.project.as_str()), ("", "uchmk"));
        assert_eq!(Settings::parse(r#"{"owner":" @Foo\"bar "}"#).owner, "Foobar");
        assert_eq!(login("https://github.com/rust-lang/"), "rust-lang");
        assert_eq!(login("  "), "");
    }

    #[test]
    fn the_owners_shown_before_are_kept_latest_first() {
        let mut s = Settings::default();
        s.set_owner("@rust-lang");
        s.set_owner("https://github.com/uchmk");
        s.set_owner("Rust-Lang");
        assert_eq!(s.owner, "Rust-Lang");
        assert_eq!(s.owners, ["Rust-Lang", "uchmk"]);
        s.set_owner("");
        assert_eq!((s.owner.as_str(), s.owners.len()), ("", 2));
        assert_eq!(Settings::parse(&s.to_json()), s);
        for i in 0..OWNERS + 2 {
            s.set_owner(&format!("o{i}"));
        }
        assert_eq!(s.owners.len(), OWNERS);
        assert_eq!(s.owners[0], format!("o{}", OWNERS + 1));
        // An owner set before the list existed joins it; doubles and blanks do not.
        let old = Settings::parse(r#"{"owner":"foo","owners":["bar","BAR"," ",""]}"#);
        assert_eq!(old.owners, ["foo", "bar"]);
    }

    #[test]
    fn other_owners_are_shown_alongside_the_first() {
        let mut s = Settings::default();
        s.set_owner("uchmk, @rust-lang  https://github.com/tokio-rs/ UCHMK,rust-lang");
        assert_eq!(s.owner, "uchmk");
        assert_eq!(s.also, ["rust-lang", "tokio-rs"]);
        assert_eq!(s.owners, ["uchmk"]);
        assert_eq!(s.owners_line(), "uchmk, rust-lang, tokio-rs");
        assert_eq!(Settings::parse(&s.to_json()), s);
        assert!(!s.add_also("Tokio-RS") && !s.add_also("uchmk") && s.add_also("serde-rs"));
        s.set_owner("rust-lang");
        assert!(s.also.is_empty());
        s.set_owner("");
        assert_eq!(s.owners_line(), "");
        // No owner, none alongside.
        assert!(Settings::parse(r#"{"also":["x"]}"#).also.is_empty());
    }

    #[test]
    fn the_order_moves_and_only_a_permutation_is_read() {
        let mut s = Settings::default();
        s.shift(0, -1);
        s.shift(5, 1);
        assert_eq!(s.order, [0, 1, 2, 3, 4, 5]);
        s.shift(0, 1);
        assert_eq!(s.order, [1, 0, 2, 3, 4, 5]);
        s.rules.shift(0, 1);
        assert_eq!(Settings::parse(&s.to_json()), s);
        assert_eq!(Settings::parse(&s.to_json()).rules.1[0], 1);
        assert_eq!(Settings::parse(r#"{"order":[0,0,1,2,3,4]}"#).order, [0, 1, 2, 3, 4, 5]);
        assert_eq!(Settings::parse(r#"{"order":[0,1,2]}"#).order, [0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn the_notifications_round_trip_and_keep_known_sounds_only() {
        let mut s = Settings::default();
        assert_eq!(s.notify.sounds, ["chime", "alert", "chime"]);
        s.notify.system[2] = false;
        s.notify.sound = [false, true, false];
        s.notify.sounds[1] = "low".into();
        s.notify.focus = false;
        assert_eq!(Settings::parse(&s.to_json()), s);
        // Before there were any: on, as it was.
        assert_eq!(Settings::parse(r#"{"fields":[true]}"#).notify, Notify::default());
        let odd = Settings::parse(r#"{"notify":{"system":[false,"x"],"sounds":["boom","low"],"focus":1}}"#).notify;
        assert_eq!(odd.system, [false, true, true]);
        assert_eq!(odd.sounds, ["chime", "low", "chime"]);
        assert!(odd.focus);
        assert_eq!([Status::Ok, Status::Bad, Status::Idle, Status::Running].map(Notify::slot), [0, 1, 2, 2]);
    }

    #[test]
    fn the_webhook_round_trips_and_is_sent_only_to_an_http_address() {
        let mut s = Settings::default();
        assert_eq!((s.notify.webhook, s.notify.webhook_format.as_str()), ([true; 3], "ntfy"));
        // Nothing until there is an address.
        assert_eq!(s.notify.hook(0), None);
        s.notify.webhook = [false, true, true];
        s.notify.webhook_url = "https://ntfy.sh/my-topic".into();
        s.notify.webhook_format = "slack".into();
        assert_eq!(Settings::parse(&s.to_json()), s);
        assert_eq!(s.notify.hook(0), None);
        assert_eq!(s.notify.hook(1), Some(("https://ntfy.sh/my-topic", "slack")));
        s.notify.webhook_url = "ntfy.sh/my-topic".into();
        assert_eq!(s.notify.hook(1), None);
        let odd =
            Settings::parse(r#"{"notify":{"webhook":[1,false],"webhook_url":" http://127.0.0.1:9/x ","webhook_format":"xml"}}"#).notify;
        assert_eq!(odd.webhook, [true, false, true]);
        assert_eq!((odd.webhook_url.as_str(), odd.webhook_format.as_str()), ("http://127.0.0.1:9/x", "ntfy"));
        assert!(is_http(" https://a") && !is_http("ftp://a") && !is_http(""));
    }

    #[test]
    fn milestone_priority_and_reviewers_are_off_by_default() {
        assert_eq!(Settings::default().fields, [true, true, true, false, false, false]);
    }
}
