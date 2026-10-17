//! minato's own settings: whose repositories are shown, which inbox rules
//! are on, which fields the writing box offers and how a watched run's end
//! is told. Kept in config.toml beside common.toml (minato-settings.json before
//! v0.61.0; [`Settings::parse`] still reads that, for the move).

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
pub const FORMATS: [&str; 4] = ["ntfy", "slack", "discord", "json"];

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
    /// The places a message goes to, each with the ends it is told of.
    pub hooks: Vec<Hook>,
}

/// One webhook: an ntfy topic, a Slack or Discord webhook, or any address
/// that takes JSON.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hook {
    /// Nothing is sent while it is not an http(s) address.
    pub url: String,
    /// The form the address takes (a word of [`FORMATS`]).
    pub format: String,
    /// Told of a run that passed, failed, was cancelled.
    pub on: [bool; 3],
}

impl Hook {
    /// A hook just added: ntfy, told of every end.
    pub fn new() -> Hook {
        Hook { url: String::new(), format: "ntfy".into(), on: [true; 3] }
    }

    fn to_json(&self) -> Value {
        json!({ "url": self.url, "format": self.format, "on": self.on })
    }

    fn parse(v: &Value) -> Hook {
        let mut h = Hook::new();
        if let Some(s) = v.get("url").and_then(Value::as_str) {
            h.url = s.trim().to_string();
        }
        if let Some(s) = v.get("format").and_then(Value::as_str).filter(|s| FORMATS.contains(s)) {
            h.format = s.to_string();
        }
        if let Some(list) = v.get("on").and_then(Value::as_array) {
            for (slot, x) in h.on.iter_mut().zip(list) {
                if let Some(b) = x.as_bool() {
                    *slot = b;
                }
            }
        }
        h
    }
}

impl Default for Hook {
    fn default() -> Self {
        Hook::new()
    }
}

impl Default for Notify {
    fn default() -> Self {
        Notify {
            system: [true; 3],
            sound: [true; 3],
            sounds: ["chime", "alert", "chime"].map(String::from),
            focus: true,
            hooks: Vec::new(),
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

    /// The webhooks' addresses and forms for a run that ended in `slot`:
    /// those told of it that have an http(s) address.
    pub fn hooks_for(&self, slot: usize) -> Vec<(&str, &str)> {
        self.hooks.iter().filter(|h| h.on[slot] && is_http(&h.url)).map(|h| (h.url.trim(), h.format.as_str())).collect()
    }

    fn to_json(&self) -> Value {
        json!({
            "system": self.system,
            "sound": self.sound,
            "sounds": self.sounds,
            "focus": self.focus,
            "webhooks": self.hooks.iter().map(Hook::to_json).collect::<Vec<_>>(),
        })
    }

    fn parse(v: &Value) -> Notify {
        let mut n = Notify::default();
        for (key, out) in [("system", &mut n.system), ("sound", &mut n.sound)] {
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
        // The single webhook of before (`webhook`, `webhook_url`, `webhook_format`) is the first.
        if v.get("webhook_url").and_then(Value::as_str).is_some_and(|u| !u.trim().is_empty()) {
            n.hooks.push(Hook::parse(&json!({ "url": v["webhook_url"], "format": v.get("webhook_format"), "on": v.get("webhook") })));
        }
        if let Some(list) = v.get("webhooks").and_then(Value::as_array) {
            n.hooks.extend(list.iter().map(Hook::parse));
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
    /// Mark an inbox item done (locally) once you have commented on it.
    pub auto_done: bool,
    /// Labels, Assignees, Status, Milestone, Priority, Reviewers.
    pub fields: [bool; FIELDS],
    /// The fields in the order the writing box lists them (indexes into `fields`).
    pub order: [usize; FIELDS],
    pub notify: Notify,
    /// The saved searches of the list, one query each.
    pub searches: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            owner: String::new(),
            owners: Vec::new(),
            also: Vec::new(),
            project: crate::board::TITLE.into(),
            rules: Rules::default(),
            auto_done: false,
            fields: [true, true, true, false, false, false],
            order: [0, 1, 2, 3, 4, 5],
            notify: Notify::default(),
            searches: Vec::new(),
        }
    }
}

/// The run ends as config.toml names them, in the order of the slots.
pub const EVENTS: [&str; 3] = ["passed", "failed", "cancelled"];

impl Settings {
    pub fn to_json(&self) -> String {
        json!({ "owner": self.owner, "owners": self.owners, "also": self.also, "project": self.project, "rules": self.rules.0, "rule_order": self.rules.1, "auto_done": self.auto_done, "fields": self.fields, "order": self.order, "notify": self.notify.to_json() }).to_string()
    }

    /// What `text` (the old minato-settings.json) says; anything missing or
    /// unreadable keeps its default.
    pub fn parse(text: &str) -> Settings {
        match serde_json::from_str::<Value>(text) {
            Ok(v) => Settings::from_value(&v),
            Err(_) => Settings::default(),
        }
    }

    /// What `text` (config.toml) says; anything missing or unreadable keeps
    /// its default.
    pub fn parse_toml(text: &str) -> Settings {
        let Ok(mut v) = toml::from_str::<Value>(text) else { return Settings::default() };
        if let Some(n) = v.get("notify") {
            v["notify"] = notify_from_toml(n);
        }
        let mut s = Settings::from_value(&v);
        if let Some(list) = v.get("searches").and_then(Value::as_array) {
            s.searches = list.iter().filter_map(Value::as_str).map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect();
        }
        s
    }

    /// The keys of config.toml as `(table, key, value written as TOML)`.
    pub fn toml_keys(&self) -> Vec<(Option<&'static str>, &'static str, String)> {
        let n = &self.notify;
        let names = |on: &[bool; 3]| EVENTS.iter().zip(on).filter(|(_, b)| **b).map(|(e, _)| *e).collect::<Vec<_>>();
        let mut out = vec![
            (None, "owner", lit(&self.owner)),
            (None, "owners", lit(&self.owners)),
            (None, "also", lit(&self.also)),
            (None, "project", lit(&self.project)),
            (None, "rules", lit(self.rules.0)),
            (None, "rule_order", lit(self.rules.1)),
            (None, "auto_done", lit(self.auto_done)),
            (None, "fields", lit(self.fields)),
            (None, "order", lit(self.order)),
            (None, "searches", lit(&self.searches)),
            (Some("notify"), "system", lit(names(&n.system))),
            (Some("notify"), "sound", lit(names(&n.sound))),
            (Some("notify"), "focus_mode", lit(n.focus)),
        ];
        for (key, sound) in ["sound_passed", "sound_failed", "sound_cancelled"].into_iter().zip(&n.sounds) {
            out.push((Some("notify"), key, lit(sound)));
        }
        out
    }

    /// The `[[notify.webhooks]]` tables, each as `(key, value written as TOML)`.
    pub fn toml_hooks(&self) -> Vec<Vec<(&'static str, String)>> {
        let states = |on: &[bool; 3]| EVENTS.iter().zip(on).filter(|(_, b)| **b).map(|(e, _)| *e).collect::<Vec<_>>();
        self.notify.hooks.iter().map(|h| vec![("url", lit(&h.url)), ("format", lit(&h.format)), ("states", lit(states(&h.on)))]).collect()
    }

    fn from_value(v: &Value) -> Settings {
        let mut s = Settings::default();
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
        if let Some(b) = v.get("auto_done").and_then(Value::as_bool) {
            s.auto_done = b;
        }
        fill("fields", &mut s.fields);
        // `grow`: an order saved before a rule was added is shorter; the new ones go last.
        let perm = |key: &str, out: &mut [usize], grow: bool| {
            let Some(list) = v.get(key).and_then(Value::as_array) else { return };
            let mut order: Vec<usize> = list.iter().filter_map(|x| x.as_u64().map(|n| n as usize)).collect();
            let mut seen = vec![false; out.len()];
            let fits = (order.len() == out.len() || (grow && !order.is_empty() && order.len() < out.len()))
                && order.iter().all(|&k| k < out.len() && !std::mem::replace(&mut seen[k], true));
            if fits {
                order.extend((0..out.len()).filter(|&k| !seen[k]));
                out.copy_from_slice(&order);
            }
        };
        perm("order", &mut s.order, false);
        perm("rule_order", &mut s.rules.1, true);
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

/// `value` written as TOML.
fn lit(value: impl serde::Serialize) -> String {
    toml::Value::try_from(value).map(|v| v.to_string()).unwrap_or_default()
}

/// The `[notify]` table of config.toml in the shape [`Notify::parse`] reads.
fn notify_from_toml(n: &Value) -> Value {
    let on = |v: &Value| match v {
        Value::Bool(b) => Some(json!([b, b, b])),
        Value::Array(list) => Some(json!(EVENTS.map(|e| list.iter().any(|x| x.as_str() == Some(e))))),
        _ => None,
    };
    let mut out = serde_json::Map::new();
    for key in ["system", "sound"] {
        if let Some(v) = n.get(key).and_then(on) {
            out.insert(key.into(), v);
        }
    }
    out.insert(
        "sounds".into(),
        json!(["sound_passed", "sound_failed", "sound_cancelled"].map(|k| n.get(k).cloned().unwrap_or(Value::Null))),
    );
    if let Some(b) = n.get("focus_mode") {
        out.insert("focus".into(), b.clone());
    }
    let mut hooks = Vec::new();
    // The one address of before (`webhook`, `webhook_format`, `webhook_states`) is the first.
    if let Some(url) = n.get("webhook").filter(|u| u.as_str().is_some_and(|u| !u.trim().is_empty())) {
        hooks.push(json!({ "url": url, "format": n.get("webhook_format"), "on": n.get("webhook_states").and_then(on) }));
    }
    if let Some(list) = n.get("webhooks").and_then(Value::as_array) {
        hooks.extend(list.iter().map(|h| json!({ "url": h.get("url"), "format": h.get("format"), "on": h.get("states").and_then(on) })));
    }
    out.insert("webhooks".into(), Value::Array(hooks));
    Value::Object(out)
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
        // A rule order saved with nine rules keeps its places; the newer ones go last.
        let old = Settings::parse(r#"{"rule_order":[1,0,2,3,4,5,6,7,8],"rules":[true,true,true,true,true,true,false,true,true]}"#);
        assert_eq!(old.rules.1, [1, 0, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        assert!(old.rules.0[9] && old.rules.0[10] && !old.rules.0[6]);
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
    fn auto_done_is_off_by_default_and_round_trips() {
        assert!(!Settings::default().auto_done);
        assert!(!Settings::parse("{}").auto_done);
        let s = Settings { auto_done: true, ..Settings::default() };
        assert!(Settings::parse(&s.to_json()).auto_done);
    }

    #[test]
    fn webhooks_round_trip_and_are_sent_only_to_an_http_address() {
        let mut s = Settings::default();
        // Nothing until there is a hook with an address.
        assert!(s.notify.hooks.is_empty() && s.notify.hooks_for(0).is_empty());
        let mut a = Hook::new();
        assert_eq!((a.on, a.format.as_str()), ([true; 3], "ntfy"));
        a.url = "https://ntfy.sh/my-topic".into();
        a.on = [false, true, true];
        a.format = "slack".into();
        let mut b = Hook::new();
        b.url = "ntfy.sh/no-scheme".into();
        s.notify.hooks = vec![a, b, Hook { url: "http://127.0.0.1:9/x".into(), format: "json".into(), on: [true; 3] }];
        assert_eq!(Settings::parse(&s.to_json()), s);
        assert_eq!(s.notify.hooks_for(0), [("http://127.0.0.1:9/x", "json")]);
        assert_eq!(s.notify.hooks_for(1), [("https://ntfy.sh/my-topic", "slack"), ("http://127.0.0.1:9/x", "json")]);
        // The one webhook of before is the first.
        let old = Settings::parse(
            r#"{"notify":{"webhook":[1,false],"webhook_url":" http://127.0.0.1:9/x ","webhook_format":"xml","webhooks":[{"url":"https://b/","format":"discord"}]}}"#,
        )
        .notify;
        assert_eq!(old.hooks.len(), 2);
        assert_eq!(
            (old.hooks[0].url.as_str(), old.hooks[0].format.as_str(), old.hooks[0].on),
            ("http://127.0.0.1:9/x", "ntfy", [true, false, true])
        );
        assert_eq!((old.hooks[1].url.as_str(), old.hooks[1].format.as_str()), ("https://b/", "discord"));
        // An old empty address is no hook.
        assert!(Settings::parse(r#"{"notify":{"webhook_url":" "}}"#).notify.hooks.is_empty());
        assert!(is_http(" https://a") && !is_http("ftp://a") && !is_http(""));
    }

    #[test]
    fn config_toml_reads_notify_in_the_shared_keys() {
        let s = Settings::parse_toml(
            r#"
owner = "@Foo"
searches = ["is:open", " ", "label:bug"]
[notify]
system = ["failed"]
sound = false
sound_failed = "low"
focus_mode = false
webhook = "https://old/"
webhook_states = ["passed"]
[[notify.webhooks]]
url = "https://b/"
format = "slack"
states = ["failed", "cancelled"]
[[notify.webhooks]]
url = "https://c/"
"#,
        );
        assert_eq!((s.owner.as_str(), s.searches.as_slice()), ("Foo", ["is:open".to_string(), "label:bug".to_string()].as_slice()));
        assert_eq!((s.notify.system, s.notify.sound, s.notify.focus), ([false, true, false], [false; 3], false));
        assert_eq!(s.notify.sounds, ["chime", "low", "chime"]);
        let h = &s.notify.hooks;
        assert_eq!((h[0].url.as_str(), h[0].format.as_str(), h[0].on), ("https://old/", "ntfy", [true, false, false]));
        assert_eq!((h[1].format.as_str(), h[1].on), ("slack", [false, true, true]));
        assert_eq!(h[2].on, [true; 3]);
        assert_eq!(Settings::parse_toml("not = [toml"), Settings::default());
    }

    #[test]
    fn toml_keys_read_back_as_written() {
        let mut s = Settings { owner: "foo".into(), searches: vec!["a b".into()], ..Settings::default() };
        s.notify.system = [true, false, true];
        s.notify.sounds[1] = "low".into();
        s.notify.hooks = vec![Hook { url: "https://x/".into(), format: "json".into(), on: [false, true, false] }];
        let mut text = String::new();
        for (table, key, value) in s.toml_keys() {
            text.push_str(&format!(
                "{}{key} = {value}
",
                table.map_or(String::new(), |t| format!(
                    "# {t}
"
                ))
            ));
        }
        // Flat check of the values: each is a TOML value that parses back.
        for (_, _, value) in s.toml_keys() {
            assert!(format!("v = {value}").parse::<toml::Table>().is_ok(), "{value}");
        }
        assert!(text.contains("system = [\"passed\", \"cancelled\"]"), "{text}");
        assert_eq!(s.toml_hooks()[0][2], ("states", "[\"failed\"]".to_string()));
    }

    #[test]
    fn milestone_priority_and_reviewers_are_off_by_default() {
        assert_eq!(Settings::default().fields, [true, true, true, false, false, false]);
    }
}
