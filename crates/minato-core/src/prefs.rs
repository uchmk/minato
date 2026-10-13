//! minato's own settings: whose repositories are shown, which inbox rules
//! are on and which fields the writing box offers. Kept as a small JSON
//! file beside common.toml.

use serde_json::{json, Value};

use crate::inbox::Rules;

/// The writing box's fields, in the order of the keys `Alt+L A S M P R`.
pub const FIELDS: usize = 6;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// The user or organization whose repositories, searches and Project
    /// are shown (a login); empty for the viewer.
    pub owner: String,
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
            project: crate::board::TITLE.into(),
            rules: Rules::default(),
            fields: [true, true, true, false, false, false],
            order: [0, 1, 2, 3, 4, 5],
        }
    }
}

impl Settings {
    pub fn to_json(&self) -> String {
        json!({ "owner": self.owner, "project": self.project, "rules": self.rules.0, "rule_order": self.rules.1, "fields": self.fields, "order": self.order }).to_string()
    }

    /// What `text` says; anything missing or unreadable keeps its default.
    pub fn parse(text: &str) -> Settings {
        let mut s = Settings::default();
        let Ok(v) = serde_json::from_str::<Value>(text) else { return s };
        if let Some(o) = v.get("owner").and_then(Value::as_str) {
            s.owner = login(o);
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
        let s = Settings { owner: "rust-lang".into(), project: "Roadmap".into(), ..Settings::default() };
        assert_eq!(Settings::parse(&s.to_json()), s);
        // Before they were settings: the viewer and "uchmk".
        let old = Settings::parse(r#"{"fields":[true]}"#);
        assert_eq!((old.owner.as_str(), old.project.as_str()), ("", "uchmk"));
        assert_eq!(Settings::parse(r#"{"owner":" @Foo\"bar "}"#).owner, "Foobar");
        assert_eq!(login("https://github.com/rust-lang/"), "rust-lang");
        assert_eq!(login("  "), "");
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
