//! The state files that are TOML (common-spec: state files are TOML too):
//! `marks.toml` and `seen-runs.toml`. The older `marks.json` and
//! `seen-runs.txt` are read once when the new file is not there yet.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::inbox::{marks_parse, Mark};

/// `marks.toml`: one `[[entries]]` per marked item.
#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
pub struct MarksFile {
    #[serde(default)]
    pub entries: Vec<MarkEntry>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct MarkEntry {
    pub id: String,
    /// `done` or `later`.
    pub mark: String,
    /// Seconds since the epoch.
    pub at: i64,
}

impl MarksFile {
    /// The file for `marks`, ordered by id so it does not shuffle between writes.
    pub fn of(marks: &HashMap<String, Mark>) -> Self {
        let mut entries: Vec<MarkEntry> = marks
            .iter()
            .map(|(id, m)| match m {
                Mark::Done(t) => MarkEntry { id: id.clone(), mark: "done".into(), at: *t },
                Mark::Later(t) => MarkEntry { id: id.clone(), mark: "later".into(), at: *t },
            })
            .collect();
        entries.sort_by(|a, b| a.id.cmp(&b.id));
        Self { entries }
    }

    /// The marks; an entry with a mark that is not known is left out.
    pub fn marks(&self) -> HashMap<String, Mark> {
        self.entries
            .iter()
            .filter_map(|e| match e.mark.as_str() {
                "done" => Some((e.id.clone(), Mark::Done(e.at))),
                "later" => Some((e.id.clone(), Mark::Later(e.at))),
                _ => None,
            })
            .collect()
    }

    /// The marks of the older `marks.json` text.
    pub fn from_old(text: &str) -> Self {
        Self::of(&marks_parse(text))
    }
}

/// `seen-runs.toml`: the ids of the runs seen in Actions, newest first.
#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
pub struct SeenFile {
    #[serde(default)]
    pub recent: Vec<u64>,
}

impl SeenFile {
    pub fn set(&self) -> HashSet<u64> {
        self.recent.iter().copied().collect()
    }

    /// The ids of the older `seen-runs.txt` (one per line).
    pub fn from_old(text: &str) -> Self {
        Self { recent: text.lines().filter_map(|l| l.trim().parse().ok()).collect() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_round_trip_through_toml_and_read_the_old_json() {
        let mut m = HashMap::new();
        m.insert("a".to_string(), Mark::Done(5));
        m.insert("b".to_string(), Mark::Later(9));
        let text = toml::to_string(&MarksFile::of(&m)).unwrap();
        assert!(text.starts_with("[[entries]]\nid = \"a\""), "{text}");
        assert_eq!(toml::from_str::<MarksFile>(&text).unwrap().marks(), m);
        assert_eq!(MarksFile::from_old(r#"{"a":["done",5],"b":["later",9],"c":["x",1]}"#).marks(), m);
        assert_eq!(toml::from_str::<MarksFile>("").unwrap(), MarksFile::default());
    }

    #[test]
    fn seen_runs_round_trip_and_read_the_old_lines() {
        let s = SeenFile { recent: vec![9, 4] };
        let text = toml::to_string(&s).unwrap();
        assert_eq!(text, "recent = [9, 4]\n");
        assert_eq!(toml::from_str::<SeenFile>(&text).unwrap(), s);
        assert_eq!(SeenFile::from_old("9\n 4 \nx\n"), s);
    }
}
