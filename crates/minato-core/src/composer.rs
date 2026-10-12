//! The pure parts of the writing box: wrapping a selection for Ctrl+B / I /
//! K, a small Markdown reader for the preview, and the picker for labels and
//! assignees.

/// `text` with `left` and `right` put around the characters `a..b` (counted
/// in chars). Returns the new text and the selection, still on the same
/// characters.
pub fn wrap(text: &str, a: usize, b: usize, left: &str, right: &str) -> (String, usize, usize) {
    let (a, b) = (a.min(b), a.max(b));
    let chars: Vec<char> = text.chars().collect();
    let b = b.min(chars.len());
    let a = a.min(b);
    let mut out: String = chars[..a].iter().collect();
    out.push_str(left);
    out.extend(&chars[a..b]);
    out.push_str(right);
    out.extend(&chars[b..]);
    let n = left.chars().count();
    (out, a + n, b + n)
}

/// A link around the selection; the selection moves to the empty `()`
/// so the address can be typed.
pub fn link(text: &str, a: usize, b: usize) -> (String, usize, usize) {
    let (out, _, e) = wrap(text, a, b, "[", "](url)");
    // Select the word `url`.
    (out, e + 2, e + 5)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Heading(u8, String),
    Bullet(String),
    Quote(String),
    Code(String),
    Para(String),
}

/// A small reading of Markdown for the preview: headings, bullets, quotes,
/// fenced code and paragraphs. Inline marks stay as typed.
pub fn blocks(text: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut code: Option<Vec<&str>> = None;
    let mut para: Vec<&str> = Vec::new();
    fn flush(para: &mut Vec<&str>, out: &mut Vec<Block>) {
        if !para.is_empty() {
            out.push(Block::Para(para.join(" ")));
            para.clear();
        }
    }
    for line in text.lines() {
        if let Some(buf) = &mut code {
            if line.trim_start().starts_with("```") {
                out.push(Block::Code(buf.join("\n")));
                code = None;
            } else {
                buf.push(line);
            }
            continue;
        }
        let t = line.trim_start();
        if t.starts_with("```") {
            flush(&mut para, &mut out);
            code = Some(Vec::new());
        } else if t.is_empty() {
            flush(&mut para, &mut out);
        } else if let Some(h) = heading(t) {
            flush(&mut para, &mut out);
            out.push(h);
        } else if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            flush(&mut para, &mut out);
            out.push(Block::Bullet(rest.trim().to_string()));
        } else if let Some(rest) = t.strip_prefix('>') {
            flush(&mut para, &mut out);
            out.push(Block::Quote(rest.trim().to_string()));
        } else {
            para.push(t);
        }
    }
    if let Some(buf) = code {
        out.push(Block::Code(buf.join("\n")));
    }
    flush(&mut para, &mut out);
    out
}

fn heading(t: &str) -> Option<Block> {
    let n = t.chars().take_while(|&c| c == '#').count();
    let rest = t[n..].strip_prefix(' ')?;
    (1..=6).contains(&n).then(|| Block::Heading(n as u8, rest.trim().to_string()))
}

/// A box of names to tick: the repository's labels or assignees.
#[derive(Clone, Debug, Default)]
pub struct Picker {
    pub options: Vec<String>,
    pub chosen: Vec<String>,
    original: Vec<String>,
    pub query: String,
    pub cursor: usize,
}

impl Picker {
    pub fn new(options: Vec<String>, chosen: Vec<String>) -> Self {
        Self { options, original: chosen.clone(), chosen, query: String::new(), cursor: 0 }
    }

    /// The options that contain the query, ignoring case.
    pub fn shown(&self) -> Vec<&str> {
        let q = self.query.to_lowercase();
        self.options.iter().map(String::as_str).filter(|o| o.to_lowercase().contains(&q)).collect()
    }

    pub fn type_char(&mut self, c: char) {
        self.query.push(c);
        self.cursor = 0;
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.cursor = 0;
    }

    pub fn step(&mut self, down: bool) {
        let n = self.shown().len();
        if n == 0 {
            return;
        }
        self.cursor = if down { (self.cursor + 1).min(n - 1) } else { self.cursor.saturating_sub(1) };
    }

    /// Tick or untick the name under the cursor.
    pub fn toggle(&mut self) {
        let Some(name) = self.shown().get(self.cursor).map(|s| s.to_string()) else { return };
        match self.chosen.iter().position(|c| *c == name) {
            Some(i) => {
                self.chosen.remove(i);
            }
            None => self.chosen.push(name),
        }
    }

    pub fn is_chosen(&self, name: &str) -> bool {
        self.chosen.iter().any(|c| c == name)
    }

    /// Whether the set differs from what it began as.
    pub fn changed(&self) -> bool {
        let mut a = self.chosen.clone();
        let mut b = self.original.clone();
        a.sort();
        b.sort();
        a != b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_keeps_the_selection_on_the_same_characters() {
        let (t, a, b) = wrap("say hello now", 4, 9, "**", "**");
        assert_eq!(t, "say **hello** now");
        assert_eq!(&t[a..b], "hello");
        let (t, a, b) = wrap("日本語", 1, 2, "_", "_");
        assert_eq!(t, "日_本_語");
        assert_eq!((a, b), (2, 3));
    }

    #[test]
    fn wrap_with_nothing_selected_puts_the_marks_together() {
        assert_eq!(wrap("ab", 1, 1, "*", "*"), ("a**b".to_string(), 2, 2));
        assert_eq!(wrap("ab", 9, 9, "*", "*").0, "ab**");
    }

    #[test]
    fn a_link_selects_the_address_word() {
        let (t, a, b) = link("see docs", 4, 8);
        assert_eq!(t, "see [docs](url)");
        assert_eq!(t.chars().skip(a).take(b - a).collect::<String>(), "url");
    }

    #[test]
    fn markdown_blocks_are_read() {
        let got = blocks("# Title\nsome\ntext\n\n- one\n* two\n> said\n```\nlet a;\n\nb\n```\nend");
        assert_eq!(
            got,
            vec![
                Block::Heading(1, "Title".into()),
                Block::Para("some text".into()),
                Block::Bullet("one".into()),
                Block::Bullet("two".into()),
                Block::Quote("said".into()),
                Block::Code("let a;\n\nb".into()),
                Block::Para("end".into()),
            ]
        );
        assert_eq!(blocks("#nospace"), vec![Block::Para("#nospace".into())]);
        assert_eq!(blocks("```\nopen"), vec![Block::Code("open".into())]);
    }

    #[test]
    fn the_picker_filters_toggles_and_reports_change() {
        let mut p = Picker::new(vec!["bug".into(), "Bugfix".into(), "lane:win".into()], vec!["lane:win".into()]);
        assert!(!p.changed());
        for c in "BUG".chars() {
            p.type_char(c);
        }
        assert_eq!(p.shown(), ["bug", "Bugfix"]);
        p.step(true);
        p.step(true);
        assert_eq!(p.cursor, 1);
        p.toggle();
        assert!(p.is_chosen("Bugfix") && p.changed());
        p.toggle();
        assert!(!p.changed());
        p.backspace();
        p.backspace();
        p.backspace();
        p.step(true);
        p.step(true);
        p.toggle();
        assert!(!p.is_chosen("lane:win") && p.changed());
    }
}
