//! minato's own settings: whose repositories are shown, which inbox rules
//! are on and which fields the writing box offers. Kept as a small JSON
//! file beside common.toml.

use serde_json::{json, Value};

use crate::inbox::Rules;

/// The writing box's fields, in the order of the keys `Alt+L A S M P R`.
pub const FIELDS: usize = 6;

/// How many owners shown before are kept for switching back.
pub const OWNERS: usize = 8;

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
        }
    }
}

impl Settings {
    pub fn to_json(&self) -> String {
        json!({ "owner": self.owner, "owners": self.owners, "also": self.also, "project": self.project, "rules": self.rules.0, "rule_order": self.rules.1, "fields": self.fields, "order": self.order }).to_string()
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
    fn milestone_priority_and_reviewers_are_off_by_default() {
        assert_eq!(Settings::default().fields, [true, true, true, false, false, false]);
    }
}
