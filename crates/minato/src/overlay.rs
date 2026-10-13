//! The two boxes the Actions view opens over itself: the dispatch form
//! (the inputs of `workflow_dispatch`) and the log of a job. They take the
//! keys themselves, as `K`, so they can be tested without a window.

use std::cell::Cell;

use minato_core::actions::Run;
use minato_core::workflow::Input;

/// A key as the boxes see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum K {
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Enter,
    /// Ctrl+Enter (Cmd+Enter): send.
    Send,
    Back,
    Esc,
    Char(char),
}

pub enum Overlay {
    Form(Box<Form>),
    Log(LogView),
}

pub struct Form {
    pub run: Run,
    pub inputs: Vec<Input>,
    pub values: Vec<String>,
    pub sel: usize,
}

pub struct LogView {
    pub job_id: u64,
    pub name: String,
    /// `None` while it is being fetched.
    pub lines: Option<Vec<String>>,
    /// The first row shown, the long lines wrapped.
    pub scroll: usize,
    /// How far it scrolls before its end shows, in rows; set as it is drawn.
    pub last: Cell<usize>,
}

/// What a key did to the box.
#[derive(Debug, PartialEq, Eq)]
pub enum Out {
    Nothing,
    Close,
    /// Send the dispatch with these inputs.
    Send(Vec<(String, String)>),
    /// A required input is empty (its name).
    Missing(String),
}

impl Form {
    pub fn new(run: Run, inputs: Vec<Input>) -> Form {
        let values = inputs.iter().map(|i| i.default.clone()).collect();
        Form { run, inputs, values, sel: 0 }
    }

    /// Whether the input is chosen from a short list rather than typed.
    pub fn picks(i: &Input) -> bool {
        i.kind == "boolean" || (i.kind == "choice" && !i.options.is_empty())
    }

    fn cycle(&mut self, step: isize) {
        let i = &self.inputs[self.sel];
        let all: Vec<String> = if i.kind == "boolean" { vec!["true".into(), "false".into()] } else { i.options.clone() };
        let at = all.iter().position(|o| *o == self.values[self.sel]).map_or(0, |p| p as isize + step);
        self.values[self.sel] = all[at.rem_euclid(all.len() as isize) as usize].clone();
    }

    pub fn key(&mut self, k: K) -> Out {
        if self.inputs.is_empty() {
            return if k == K::Esc { Out::Close } else { Out::Nothing };
        }
        let n = self.inputs.len();
        let pick = Self::picks(&self.inputs[self.sel]);
        match k {
            K::Esc => return Out::Close,
            K::Up => self.sel = (self.sel + n - 1) % n,
            K::Down => self.sel = (self.sel + 1) % n,
            K::Left if pick => self.cycle(-1),
            K::Right if pick => self.cycle(1),
            K::Char(' ') if pick => self.cycle(1),
            K::Enter if pick => self.cycle(1),
            K::Enter => self.sel = (self.sel + 1) % n,
            K::Back if !pick => {
                self.values[self.sel].pop();
            }
            K::Char(c) if !pick && !c.is_control() => self.values[self.sel].push(c),
            K::Send => {
                if let Some(i) = self.inputs.iter().zip(&self.values).find(|(i, v)| i.required && v.is_empty()) {
                    return Out::Missing(i.0.name.clone());
                }
                let all = self
                    .inputs
                    .iter()
                    .zip(&self.values)
                    .filter(|(i, v)| i.required || !v.is_empty() || !i.default.is_empty())
                    .map(|(i, v)| (i.name.clone(), v.clone()))
                    .collect();
                return Out::Send(all);
            }
            _ => {}
        }
        Out::Nothing
    }
}

impl LogView {
    pub fn new(job_id: u64, name: String) -> LogView {
        LogView { job_id, name, lines: None, scroll: 0, last: Cell::new(0) }
    }

    pub fn key(&mut self, k: K) -> Out {
        let last = self.last.get();
        self.scroll = self.scroll.min(last);
        match k {
            K::Esc | K::Char('q') => return Out::Close,
            K::Down | K::Char('j') => self.scroll = (self.scroll + 1).min(last),
            K::Up | K::Char('k') => self.scroll = self.scroll.saturating_sub(1),
            K::PageDown | K::Char(' ') => self.scroll = (self.scroll + 20).min(last),
            K::PageUp => self.scroll = self.scroll.saturating_sub(20),
            K::Char('g') => self.scroll = 0,
            K::Char('G') => self.scroll = last,
            _ => {}
        }
        Out::Nothing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use minato_core::actions::Status;

    fn run() -> Run {
        Run {
            id: 1,
            repo: "o/r".into(),
            workflow_id: 5,
            workflow: "w".into(),
            number: 1,
            title: "t".into(),
            event: "push".into(),
            branch: "main".into(),
            sha: "abc".into(),
            actor: "a".into(),
            status: Status::Ok,
            started_at: String::new(),
            updated_at: String::new(),
            url: String::new(),
            pr: None,
            path: String::new(),
            head_sha: String::new(),
        }
    }

    fn input(name: &str, kind: &str, default: &str, required: bool, options: &[&str]) -> Input {
        Input {
            name: name.into(),
            description: String::new(),
            required,
            kind: kind.into(),
            default: default.into(),
            options: options.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn form() -> Form {
        Form::new(
            run(),
            vec![
                input("level", "choice", "info", true, &["info", "debug"]),
                input("dry", "boolean", "false", false, &[]),
                input("note", "string", "", false, &[]),
                input("who", "string", "", true, &[]),
            ],
        )
    }

    #[test]
    fn choices_cycle_and_text_is_typed() {
        let mut f = form();
        f.key(K::Right);
        assert_eq!(f.values[0], "debug");
        f.key(K::Right);
        assert_eq!(f.values[0], "info");
        f.key(K::Down);
        f.key(K::Enter);
        assert_eq!(f.values[1], "true");
        f.key(K::Down);
        for c in "hi".chars() {
            f.key(K::Char(c));
        }
        f.key(K::Back);
        assert_eq!(f.values[2], "h");
    }

    #[test]
    fn send_needs_the_required_inputs() {
        let mut f = form();
        assert_eq!(f.key(K::Send), Out::Missing("who".into()));
        f.sel = 3;
        f.key(K::Char('x'));
        let Out::Send(v) = f.key(K::Send) else { panic!() };
        assert_eq!(
            v,
            [("level".to_string(), "info".to_string()), ("dry".to_string(), "false".to_string()), ("who".to_string(), "x".to_string())]
        );
        assert_eq!(f.key(K::Esc), Out::Close);
    }

    #[test]
    fn the_log_scrolls_within_its_lines() {
        let mut l = LogView::new(1, "t".into());
        l.key(K::Char('j'));
        assert_eq!(l.scroll, 0);
        l.lines = Some((0..50).map(|i| i.to_string()).collect());
        l.last.set(30);
        l.key(K::Char('G'));
        assert_eq!(l.scroll, 30);
        l.key(K::PageUp);
        assert_eq!(l.scroll, 10);
        // A taller window shows more, and the next key starts from its end.
        l.key(K::Char('G'));
        l.last.set(20);
        l.key(K::Char('k'));
        assert_eq!(l.scroll, 19);
        l.key(K::Char('g'));
        assert_eq!(l.scroll, 0);
        assert_eq!(l.key(K::Esc), Out::Close);
    }
}
