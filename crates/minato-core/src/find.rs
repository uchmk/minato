//! The search box (`/`): words split by spaces, all of which must be found
//! (AND), each in the title, `#number`, repository, labels, author or the
//! start of the body. Smart case: the case counts only once a capital is
//! typed. The same words are lit where they show.

use std::ops::Range;

use crate::list::Item;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    /// The words as typed (lowered when `case` is off).
    words: Vec<String>,
    /// A capital was typed: the case counts.
    case: bool,
}

impl Query {
    pub fn new(text: &str) -> Query {
        let case = text.chars().any(char::is_uppercase);
        let words = text.split_whitespace().map(|w| if case { w.to_string() } else { w.to_lowercase() }).collect();
        Query { words, case }
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    fn found(&self, hay: &str, word: &str) -> bool {
        if self.case {
            hay.contains(word)
        } else {
            hay.to_lowercase().contains(word)
        }
    }

    /// Whether every word is in one of the item's fields (an empty query
    /// takes everything).
    pub fn takes(&self, item: &Item) -> bool {
        if self.is_empty() {
            return true;
        }
        let number = format!("#{}", item.number);
        let fields = [item.title.as_str(), &number, &item.repo, &item.author, &item.peek];
        self.words.iter().all(|w| fields.iter().any(|f| self.found(f, w)) || item.labels.iter().any(|l| self.found(&l.name, w)))
    }

    /// Whether every word is in one of `fields` (for what is not an item).
    pub fn takes_fields(&self, fields: &[&str]) -> bool {
        self.words.iter().all(|w| fields.iter().any(|f| self.found(f, w)))
    }

    /// Where the words are in `text`, as byte ranges in order, joined where
    /// they touch or overlap.
    pub fn ranges(&self, text: &str) -> Vec<Range<usize>> {
        let mut out: Vec<Range<usize>> = Vec::new();
        if self.is_empty() {
            return out;
        }
        for (at, _) in text.char_indices() {
            for w in &self.words {
                if let Some(len) = self.starts(&text[at..], w) {
                    out.push(at..at + len);
                }
            }
        }
        out.sort_by_key(|r| (r.start, r.end));
        let mut joined: Vec<Range<usize>> = Vec::new();
        for r in out {
            match joined.last_mut() {
                Some(last) if r.start <= last.end => last.end = last.end.max(r.end),
                _ => joined.push(r),
            }
        }
        joined
    }

    /// The bytes of `hay`'s start that make `word`, if it starts with it.
    fn starts(&self, hay: &str, word: &str) -> Option<usize> {
        if self.case {
            return hay.starts_with(word).then_some(word.len());
        }
        let mut want = word.chars().peekable();
        for (at, c) in hay.char_indices() {
            for l in c.to_lowercase() {
                if want.next() != Some(l) {
                    return None;
                }
            }
            if want.peek().is_none() {
                return Some(at + c.len_utf8());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::list::{Kind, Label};

    fn item() -> Item {
        Item {
            id: "N1".into(),
            kind: Kind::Issue,
            repo: "uchmk/kura".into(),
            number: 42,
            title: "Font size in the Preview".into(),
            url: String::new(),
            author: "octocat".into(),
            labels: vec![Label { name: "finding".into(), color: "fff".into() }],
            assignees: Vec::new(),
            milestone: String::new(),
            reviewers: Vec::new(),
            comments: 0,
            last_by: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
            draft: false,
            merged: false,
            closed: false,
            conflict: false,
            mentioned: false,
            ci: None,
            peek: "Reads it from common.toml.".into(),
            head: String::new(),
            decision: Default::default(),
        }
    }

    #[test]
    fn every_word_must_be_found_somewhere() {
        let i = item();
        assert!(Query::new("").takes(&i));
        assert!(Query::new("font kura").takes(&i));
        assert!(Query::new("#42 finding octocat common.toml").takes(&i));
        assert!(!Query::new("font tsumugi").takes(&i));
        assert!(!Query::new("#43").takes(&i));
    }

    #[test]
    fn the_case_counts_once_a_capital_is_typed() {
        let i = item();
        assert!(Query::new("preview").takes(&i));
        assert!(Query::new("Preview").takes(&i));
        assert!(!Query::new("PREVIEW").takes(&i));
        assert!(!Query::new("Font kURA").takes(&i));
    }

    #[test]
    fn the_words_are_lit_where_they_are() {
        let q = Query::new("the pre");
        assert_eq!(q.ranges("Font size in the Preview"), [13..16, 17..20]);
        // Overlapping and touching hits join.
        assert_eq!(Query::new("ab bc").ranges("xabcx"), vec![1..4]);
        assert_eq!(Query::new("Ab").ranges("ab Ab"), vec![3..5]);
        // Non-ASCII text keeps its byte offsets.
        assert_eq!(Query::new("ä").ranges("ÄäA x ä"), [0..4, 8..10]);
        assert_eq!(Query::new("検索").ranges("全文検索です"), vec![6..12]);
        assert!(Query::new("").ranges("abc").is_empty());
    }
}
