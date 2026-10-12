//! The pure parts of the writing box: wrapping a selection for Ctrl+B / I /
//! K and the toolbar, a small Markdown reader for the preview and the
//! detail, and the picker for labels and assignees.

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

/// A button of the writing box's toolbar (GitHub's, less the slash).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Heading,
    Bold,
    Italic,
    Quote,
    Code,
    Link,
    Bullets,
    Numbers,
    Tasks,
    Mention,
    Reference,
}

impl Tool {
    pub const ALL: [Tool; 11] = [Tool::Heading, Tool::Bold, Tool::Italic, Tool::Quote, Tool::Code, Tool::Link, Tool::Bullets, Tool::Numbers, Tool::Tasks, Tool::Mention, Tool::Reference];
}

/// What a toolbar button does to `text` with the characters `a..b`
/// selected: the new text and selection.
pub fn tool(text: &str, a: usize, b: usize, t: Tool) -> (String, usize, usize) {
    let (a, b) = (a.min(b), a.max(b));
    match t {
        Tool::Bold => wrap(text, a, b, "**", "**"),
        Tool::Italic => wrap(text, a, b, "_", "_"),
        Tool::Link => link(text, a, b),
        Tool::Code if text.chars().skip(a).take(b - a).any(|c| c == '\n') => wrap(text, a, b, "```\n", "\n```"),
        Tool::Code => wrap(text, a, b, "`", "`"),
        Tool::Mention => wrap(text, b, b, "@", ""),
        Tool::Reference => wrap(text, b, b, "#", ""),
        Tool::Heading => lines(text, a, b, |_| "### ".into()),
        Tool::Quote => lines(text, a, b, |_| "> ".into()),
        Tool::Bullets => lines(text, a, b, |_| "- ".into()),
        Tool::Numbers => lines(text, a, b, |k| format!("{}. ", k + 1)),
        Tool::Tasks => lines(text, a, b, |_| "- [ ] ".into()),
    }
}

/// `mark(k)` put at the start of each line the selection touches (the k-th
/// of them); the selection moves to keep the same text.
fn lines(text: &str, a: usize, b: usize, mark: impl Fn(usize) -> String) -> (String, usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    let b = b.min(chars.len());
    let a = a.min(b);
    let start = chars[..a].iter().rposition(|&c| c == '\n').map_or(0, |i| i + 1);
    // The starts of the lines from `start` to the one `b` is on.
    let mut starts = vec![start];
    starts.extend((start..b).filter(|&i| chars[i] == '\n').map(|i| i + 1));
    let mut out = String::new();
    let (mut na, mut nb) = (a, b);
    let mut from = 0;
    for (k, &at) in starts.iter().enumerate() {
        out.extend(&chars[from..at]);
        let m = mark(k);
        let n = m.chars().count();
        out.push_str(&m);
        if at <= a {
            na += n;
        }
        nb += n;
        from = at;
    }
    out.extend(&chars[from..]);
    (out, na, nb)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Heading(u8, String),
    Bullet(String),
    /// A numbered item: its number as written, and the text.
    Numbered(String, String),
    /// A task: whether it is ticked, and the text.
    Task(bool, String),
    Quote(String),
    Code(String),
    /// A table's rows, the cells of each (the line under the head dropped).
    Table(Vec<Vec<String>>),
    /// `---`.
    Rule,
    Para(String),
}

/// A small reading of Markdown: headings, bullets, numbered items, tasks,
/// quotes, fenced code, tables, rules and paragraphs. HTML comments (issue
/// templates leave them) and lines that are only a tag are dropped. Inline
/// marks stay as typed; [`spans`] reads them.
pub fn blocks(text: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut code: Option<Vec<&str>> = None;
    let mut para: Vec<&str> = Vec::new();
    let mut table: Vec<Vec<String>> = Vec::new();
    fn flush(para: &mut Vec<&str>, table: &mut Vec<Vec<String>>, out: &mut Vec<Block>) {
        if !para.is_empty() {
            out.push(Block::Para(para.join(" ")));
            para.clear();
        }
        if !table.is_empty() {
            out.push(Block::Table(std::mem::take(table)));
        }
    }
    let text = without_comments(text);
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
        let t = line.trim();
        if t.starts_with('|') {
            if !para.is_empty() {
                out.push(Block::Para(para.join(" ")));
                para.clear();
            }
            let cells: Vec<String> = t.trim_matches('|').split('|').map(|c| c.trim().to_string()).collect();
            // The line under the head (`|---|:--:|`) only says how to align.
            if !cells.iter().all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':'))) {
                table.push(cells);
            }
            continue;
        }
        if t.starts_with("```") {
            flush(&mut para, &mut table, &mut out);
            code = Some(Vec::new());
        } else if t.is_empty() || is_tag(t) {
            flush(&mut para, &mut table, &mut out);
        } else if let Some(h) = heading(t) {
            flush(&mut para, &mut table, &mut out);
            out.push(h);
        } else if t.len() >= 3 && ['-', '*', '_'].iter().any(|&m| t.chars().all(|c| c == m)) {
            flush(&mut para, &mut table, &mut out);
            out.push(Block::Rule);
        } else if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).or_else(|| t.strip_prefix("+ ")) {
            flush(&mut para, &mut table, &mut out);
            let rest = rest.trim();
            let task = [("[ ] ", false), ("[x] ", true), ("[X] ", true)].into_iter().find_map(|(m, on)| rest.strip_prefix(m).map(|r| (on, r)));
            out.push(match task {
                Some((on, r)) => Block::Task(on, r.trim().to_string()),
                None => Block::Bullet(rest.to_string()),
            });
        } else if let Some((n, rest)) = numbered(t) {
            flush(&mut para, &mut table, &mut out);
            out.push(Block::Numbered(n, rest));
        } else if let Some(rest) = t.strip_prefix('>') {
            flush(&mut para, &mut table, &mut out);
            out.push(Block::Quote(rest.trim().to_string()));
        } else {
            if !table.is_empty() {
                out.push(Block::Table(std::mem::take(&mut table)));
            }
            para.push(t);
        }
    }
    if let Some(buf) = code {
        out.push(Block::Code(buf.join("\n")));
    }
    flush(&mut para, &mut table, &mut out);
    out
}

/// `text` without its `<!-- … -->`.
fn without_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("<!--") {
        out.push_str(&rest[..i]);
        match rest[i..].find("-->") {
            Some(j) => rest = &rest[i + j + 3..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// A line that is only an HTML tag (`<details>`, `</summary>`, `<br/>`).
fn is_tag(t: &str) -> bool {
    t.starts_with('<') && t.ends_with('>') && t.matches('<').count() == 1 && t[1..].starts_with(|c: char| c.is_ascii_alphabetic() || c == '/')
}

/// `12. text` as its number and text.
fn numbered(t: &str) -> Option<(String, String)> {
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    let rest = t[digits..].strip_prefix(". ").or_else(|| t[digits..].strip_prefix(") "))?;
    (digits > 0 && digits < 10).then(|| (t[..digits].to_string(), rest.trim().to_string()))
}

/// A run of text with the same inline marks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    /// Where a link goes.
    pub link: Option<String>,
}

/// The inline marks of one block: `**bold**` / `__bold__`, `_italic_` /
/// `*italic*`, `~~struck~~`, `` `code` ``, `[text](url)` and `![alt](url)`
/// (shown as its alt). A mark that is never closed stays as typed.
pub fn spans(s: &str) -> Vec<Span> {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let mut out: Vec<Span> = Vec::new();
    let mut cur = Span::default();
    fn push(out: &mut Vec<Span>, cur: &mut Span) {
        if !cur.text.is_empty() {
            let next = Span { text: String::new(), ..cur.clone() };
            out.push(std::mem::replace(cur, next));
        }
    }
    let closes = |from: usize, mark: &[char]| (from..n).any(|j| chars[j..].starts_with(mark));
    let mut i = 0;
    while i < n {
        let c = chars[i];
        let rest = &chars[i..];
        if c == '\\' && i + 1 < n && chars[i + 1].is_ascii_punctuation() {
            cur.text.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if c == '`' {
            if let Some(j) = (i + 1..n).find(|&j| chars[j] == '`') {
                push(&mut out, &mut cur);
                out.push(Span { text: chars[i + 1..j].iter().collect(), code: true, ..cur.clone() });
                i = j + 1;
                continue;
            }
        }
        let image = c == '!' && rest.get(1) == Some(&'[');
        if c == '[' || image {
            let open = i + usize::from(image);
            let close = (open + 1..n).find(|&j| chars[j] == ']');
            let end = close.filter(|&k| chars.get(k + 1) == Some(&'(')).and_then(|k| (k + 2..n).find(|&j| chars[j] == ')').map(|e| (k, e)));
            if let Some((close, end)) = end {
                push(&mut out, &mut cur);
                let label: String = chars[open + 1..close].iter().collect();
                let url: String = chars[close + 2..end].iter().collect();
                let text = if image { format!("[{}]", if label.is_empty() { "image" } else { &label }) } else { label };
                out.push(Span { text, link: Some(url), ..cur.clone() });
                i = end + 1;
                continue;
            }
        }
        let pair = rest.get(..2).filter(|p| p[0] == p[1] && matches!(p[0], '*' | '_' | '~'));
        if let Some(pair) = pair {
            let strike = pair[0] == '~';
            let on = if strike { cur.strike } else { cur.bold };
            if on || closes(i + 2, pair) {
                push(&mut out, &mut cur);
                if strike {
                    cur.strike = !on;
                } else {
                    cur.bold = !on;
                }
            } else {
                cur.text.extend(pair);
            }
            i += 2;
            continue;
        }
        if c == '*' || c == '_' {
            let inside = c == '_' && i > 0 && chars[i - 1].is_alphanumeric() && chars.get(i + 1).is_some_and(|x| x.is_alphanumeric());
            let opens = chars.get(i + 1).is_some_and(|x| !x.is_whitespace()) && closes(i + 1, &[c]);
            if !inside && (cur.italic || opens) {
                push(&mut out, &mut cur);
                cur.italic = !cur.italic;
                i += 1;
                continue;
            }
        }
        cur.text.push(c);
        i += 1;
    }
    push(&mut out, &mut cur);
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
    /// Ticking a name unticks the others (a Project's single-select field).
    pub single: bool,
    pub query: String,
    pub cursor: usize,
}

impl Picker {
    pub fn new(options: Vec<String>, chosen: Vec<String>) -> Self {
        Self { options, original: chosen.clone(), chosen, single: false, query: String::new(), cursor: 0 }
    }

    pub fn single(options: Vec<String>, chosen: Vec<String>) -> Self {
        Self { single: true, ..Self::new(options, chosen) }
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
        if self.single {
            self.chosen = vec![name];
            return;
        }
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
    fn markdown_lists_tables_and_comments() {
        let got = blocks("<!-- template\nhint -->\n1. one\n2) two\n- [ ] todo\n- [x] done\n---\n<details>\n| a | b |\n|---|:-:|\n| 1 | 2 |\nafter");
        assert_eq!(
            got,
            vec![
                Block::Numbered("1".into(), "one".into()),
                Block::Numbered("2".into(), "two".into()),
                Block::Task(false, "todo".into()),
                Block::Task(true, "done".into()),
                Block::Rule,
                Block::Table(vec![vec!["a".into(), "b".into()], vec!["1".into(), "2".into()]]),
                Block::Para("after".into()),
            ]
        );
        assert_eq!(blocks("a <!-- open"), vec![Block::Para("a".into())]);
    }

    #[test]
    fn inline_marks_are_read() {
        let s = spans("say **bold** and _it_ with `co*de*` and [link](https://x.test) ~~no~~");
        let plain: Vec<(&str, bool, bool, bool, bool)> = s.iter().map(|x| (x.text.as_str(), x.bold, x.italic, x.code, x.strike)).collect();
        assert_eq!(
            plain,
            vec![
                ("say ", false, false, false, false),
                ("bold", true, false, false, false),
                (" and ", false, false, false, false),
                ("it", false, true, false, false),
                (" with ", false, false, false, false),
                ("co*de*", false, false, true, false),
                (" and ", false, false, false, false),
                ("link", false, false, false, false),
                (" ", false, false, false, false),
                ("no", false, false, false, true),
            ]
        );
        assert_eq!(s[7].link.as_deref(), Some("https://x.test"));
        // Unclosed marks and snake_case stay as typed.
        let t: String = spans("a ** b and snake_case_name and 2 * 3").iter().map(|x| x.text.clone()).collect();
        assert_eq!(t, "a ** b and snake_case_name and 2 * 3");
        assert_eq!(spans("![shot](u.png)")[0].text, "[shot]");
        assert_eq!(spans("\\*not\\*")[0].text, "*not*");
    }

    #[test]
    fn the_toolbar_marks_lines_and_words() {
        assert_eq!(tool("ab", 0, 2, Tool::Bold).0, "**ab**");
        assert_eq!(tool("ab", 1, 1, Tool::Mention), ("a@b".to_string(), 2, 2));
        assert_eq!(tool("a\nb", 0, 3, Tool::Code).0, "```\na\nb\n```");
        assert_eq!(tool("one\ntwo\nthree", 1, 5, Tool::Numbers), ("1. one\n2. two\nthree".to_string(), 4, 11));
        assert_eq!(tool("x\ny", 2, 2, Tool::Quote), ("x\n> y".to_string(), 4, 4));
        assert_eq!(tool("", 0, 0, Tool::Tasks), ("- [ ] ".to_string(), 6, 6));
        assert_eq!(tool("hi", 2, 2, Tool::Heading).0, "### hi");
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
