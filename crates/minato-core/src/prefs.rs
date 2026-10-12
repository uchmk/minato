//! minato's own settings: which inbox rules are on and which fields the
//! writing box offers. Kept as a small JSON file beside common.toml.

use serde_json::{Value, json};

use crate::inbox::Rules;

/// The writing box's fields, in the order of the keys `Alt+L A S M P R`.
pub const FIELDS: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub rules: Rules,
    /// Labels, Assignees, Status, Milestone, Priority, Reviewers.
    pub fields: [bool; FIELDS],
}

impl Default for Settings {
    fn default() -> Self {
        Settings { rules: Rules::default(), fields: [true, true, true, false, false, false] }
    }
}

impl Settings {
    pub fn to_json(&self) -> String {
        json!({ "rules": self.rules.0, "fields": self.fields }).to_string()
    }

    /// What `text` says; anything missing or unreadable keeps its default.
    pub fn parse(text: &str) -> Settings {
        let mut s = Settings::default();
        let Ok(v) = serde_json::from_str::<Value>(text) else { return s };
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
        s
    }
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
    fn milestone_priority_and_reviewers_are_off_by_default() {
        assert_eq!(Settings::default().fields, [true, true, true, false, false, false]);
    }
}
