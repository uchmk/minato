//! minato's keys from keymap.toml (ito's `docs/common-spec.md`): the
//! built-in `defaults/keymap.toml` with `<config>/uchmk/minato/keymap.toml`
//! laid over it by `ito-keymap`. A binding runs actions by name; each action
//! comes down to the key it was before the keys could be changed ([`Fire`]),
//! so `App` still reads one character at a time.

use ito_keymap::{Binding, Layer, Match, Spec};
use ito_keys::Key;

use crate::app::View;

/// The built-in keymap, in the form a user's file has.
pub const DEFAULTS: &str = include_str!("defaults/keymap.toml");

/// The user's file, in the `minato` folder of the `uchmk` one.
pub const FILE: &str = "keymap.toml";

const LAYERS: [Layer; 7] = [
    Layer::new("main"),
    Layer::new("inbox"),
    Layer::new("list"),
    Layer::new("actions"),
    Layer::new("board"),
    Layer::new("status"),
    Layer::new("notes"),
];

pub const SPEC: Spec = Spec { layers: &LAYERS, ignored: &[], defaults: DEFAULTS };

/// The command that does nothing: a built-in key taken away.
pub const NOOP: &str = "none";

/// Keys read before the keymap is (`main.rs`): bound in it, they never run.
const FIXED: [&str; 8] = ["<Esc>", "<F1>", "<F5>", "<C-,>", "<C-S-p>", "<C-=>", "<C-->", "<C-0>"];

/// What an action does: the character it was, Enter, Tab (Shift+Tab with
/// `true`), or `gg`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fire {
    Char(char),
    Enter,
    Tab(bool),
    Top,
}

/// The actions: the name a keymap runs, the layer it belongs to (`main`'s
/// work in every view and over the notifications) and what it does.
const ACTIONS: &[(&str, &str, Fire)] = &[
    ("view inbox", "main", Fire::Char('1')),
    ("view list", "main", Fire::Char('2')),
    ("view actions", "main", Fire::Char('3')),
    ("view board", "main", Fire::Char('4')),
    ("view status", "main", Fire::Char('5')),
    ("next_view", "main", Fire::Tab(false)),
    ("prev_view", "main", Fire::Tab(true)),
    ("down", "main", Fire::Char('j')),
    ("up", "main", Fire::Char('k')),
    ("left", "main", Fire::Char('h')),
    ("right", "main", Fire::Char('l')),
    ("top", "main", Fire::Top),
    ("bottom", "main", Fire::Char('G')),
    ("enter", "main", Fire::Enter),
    ("open", "main", Fire::Char('o')),
    ("open_outside", "main", Fire::Char('O')),
    ("copy_link", "main", Fire::Char('y')),
    ("undo", "main", Fire::Char('u')),
    ("notes", "main", Fire::Char('n')),
    ("owner", "main", Fire::Char('@')),
    ("settings", "main", Fire::Char(',')),
    ("help", "main", Fire::Char('?')),
    ("details", "inbox", Fire::Char(' ')),
    ("reply", "inbox", Fire::Char('r')),
    ("new_issue", "inbox", Fire::Char('N')),
    ("repo_settings", "inbox", Fire::Char('S')),
    ("answer", "inbox", Fire::Char('M')),
    ("done", "inbox", Fire::Char('d')),
    ("later", "inbox", Fire::Char('s')),
    ("merge", "inbox", Fire::Char('m')),
    ("close", "inbox", Fire::Char('x')),
    ("assign", "inbox", Fire::Char('a')),
    ("prompt", "inbox", Fire::Char('c')),
    ("filter_status", "inbox", Fire::Char('f')),
    ("filter_repo", "inbox", Fire::Char('F')),
    ("filter_tag", "inbox", Fire::Char('t')),
    ("find", "inbox", Fire::Char('/')),
    ("select", "list", Fire::Char(' ')),
    ("reply", "list", Fire::Char('r')),
    ("new_issue", "list", Fire::Char('N')),
    ("repo_settings", "list", Fire::Char('S')),
    ("answer", "list", Fire::Char('M')),
    ("assign", "list", Fire::Char('a')),
    ("merge", "list", Fire::Char('m')),
    ("close", "list", Fire::Char('x')),
    ("closed", "list", Fire::Char('C')),
    ("find", "list", Fire::Char('/')),
    ("select", "actions", Fire::Char(' ')),
    ("watch", "actions", Fire::Char('w')),
    ("rerun", "actions", Fire::Char('R')),
    ("cancel", "actions", Fire::Char('X')),
    ("dispatch", "actions", Fire::Char('D')),
    ("logs", "actions", Fire::Char('e')),
    ("log", "actions", Fire::Char('L')),
    ("open_pr", "actions", Fire::Char('p')),
    ("card_left", "board", Fire::Char('H')),
    ("card_right", "board", Fire::Char('L')),
    ("read", "notes", Fire::Char('d')),
    ("read_all", "notes", Fire::Char('A')),
    ("close_notes", "notes", Fire::Char('n')),
];

/// A binding's command: an action by name, or `none` (the key does nothing).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    Run(&'static str),
    None,
}

/// What an action does (`None` for a name that is no action).
pub fn fire(name: &str) -> Option<Fire> {
    ACTIONS.iter().find(|(n, _, _)| *n == name).map(|(_, _, f)| *f)
}

/// Whether the action `name` may be bound in `layer`.
fn belongs(name: &str, layer: &str) -> bool {
    ACTIONS.iter().any(|(n, l, _)| *n == name && (*l == layer || *l == "main"))
}

/// Read one `run` string.
fn command(text: &str, warnings: &mut Vec<String>) -> Option<Cmd> {
    let name = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if name == NOOP {
        return Some(Cmd::None);
    }
    match ACTIONS.iter().find(|(n, _, _)| *n == name) {
        Some((n, _, _)) => Some(Cmd::Run(n)),
        None => {
            warnings.push(format!("keymap: `{text}` is no action of minato"));
            None
        }
    }
}

/// Where the keys are read: a view, the notifications over it, or the list
/// of keys (F1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ctx {
    View(View),
    Notes,
    Help,
}

/// The bindings in force in each [`Ctx`]: its own layer's, then [main]'s.
pub struct Keys {
    main: Vec<Binding<Cmd>>,
    views: [Vec<Binding<Cmd>>; 5],
    notes: Vec<Binding<Cmd>>,
    /// Each layer's own bindings, in [`SPEC`]'s order: what the settings
    /// screen's Keys page lists.
    layers: Vec<(String, Vec<Binding<Cmd>>)>,
}

impl Default for Keys {
    fn default() -> Self {
        Self::load(None).0
    }
}

impl Keys {
    /// The built-in keys with the user's file (its path, for the warnings,
    /// and its text) over them, and what in it does not work.
    pub fn load(user: Option<(&str, &str)>) -> (Self, Vec<String>) {
        let files: Vec<(&str, &str)> = user.into_iter().collect();
        let mut km = ito_keymap::load(&SPEC, &files, &mut command);
        let mut warnings = std::mem::take(&mut km.warnings);
        let mut take = |layer: &str| {
            let mut b = km.take(layer);
            b.retain(|b| {
                if let Some(k) = Self::fixed(&b.on) {
                    warnings.push(format!("{}: [{layer}] `{k}` is fixed and cannot be bound", b.from));
                    return false;
                }
                let wrong: Vec<&str> =
                    b.run.iter().filter_map(|c| if let Cmd::Run(n) = c { Some(*n) } else { None }).filter(|n| !belongs(n, layer)).collect();
                for n in &wrong {
                    warnings.push(format!("{}: [{layer}] `{n}` is no action of [{layer}]", b.from));
                }
                wrong.is_empty()
            });
            b
        };
        let layers: Vec<(String, Vec<Binding<Cmd>>)> = LAYERS.iter().map(|l| (l.name.to_owned(), take(l.name))).collect();
        let main = layers[0].1.clone();
        let own = |k: usize| {
            let mut b = layers[k].1.clone();
            b.extend(main.iter().cloned());
            b
        };
        let views = [own(1), own(2), own(3), own(4), own(5)];
        let notes = own(6);
        // A key of a view's section in front of [main]'s chords that start
        // with it: there those chords can no longer be pressed.
        let sections = ["inbox", "list", "actions", "board", "status", "notes"];
        for (layer, list) in sections.iter().zip(views.iter().chain([&notes])) {
            for b in &list[..list.len() - main.len()] {
                for m in main.iter().filter(|m| m.on.len() > b.on.len() && m.on.starts_with(&b.on)) {
                    if *b.from != *ito_keymap::BUILT_IN || *m.from != *ito_keymap::BUILT_IN {
                        let (k, chord) = (ito_keys::render_seq(&b.on), ito_keys::render_seq(&m.on));
                        warnings.push(format!("{}: [{layer}] `{k}` hides [main]'s `{chord}` there", b.from));
                    }
                }
            }
        }
        (Self { main, views, notes, layers }, warnings)
    }

    /// Each layer's own bindings, by name in [`SPEC`]'s order.
    pub fn layers(&self) -> &[(String, Vec<Binding<Cmd>>)] {
        &self.layers
    }

    /// The built-in keys alone, layer by layer, read once.
    pub fn built_in_layers() -> &'static [(String, Vec<Binding<Cmd>>)] {
        static BUILT_IN: std::sync::OnceLock<Vec<(String, Vec<Binding<Cmd>>)>> = std::sync::OnceLock::new();
        BUILT_IN.get_or_init(|| Self::load(None).0.layers)
    }

    /// Whether `keys` hold one that minato reads before the keymap (Esc, F1,
    /// F5, the settings, the palette, the size): bound, it never runs.
    pub fn fixed(keys: &[Key]) -> Option<&Key> {
        keys.iter().find(|k| FIXED.iter().any(|f| Key::parse(f).as_ref() == Some(*k)))
    }

    /// The bindings read in `ctx`, the first that matches winning.
    pub fn bindings(&self, ctx: Ctx) -> &[Binding<Cmd>] {
        match ctx {
            Ctx::View(v) => &self.views[View::ALL.iter().position(|&w| w == v).unwrap_or(0)],
            Ctx::Notes => &self.notes,
            Ctx::Help => &self.main,
        }
    }

    /// The first keys that run `name` alone in `ctx` (and that no binding
    /// before them takes), as the screen shows them; empty when none do.
    pub fn label(&self, ctx: Ctx, name: &str) -> String {
        let list = self.bindings(ctx);
        let found = list.iter().find(|b| {
            matches!(b.run.as_slice(), [Cmd::Run(n)] if *n == name)
                && matches!(ito_keymap::resolve(list, &b.on), Match::Exact(x) if std::ptr::eq(x, *b))
        });
        found.map(|b| human(&b.on)).unwrap_or_default()
    }

    /// `spec` with each `{action}` in it replaced by [`Keys::label`], the
    /// spaces left by an unbound one dropped: `"{top} {bottom}"` is `gg G`.
    pub fn fill(&self, ctx: Ctx, spec: &str) -> String {
        let mut out = String::new();
        let mut rest = spec;
        while let Some(at) = rest.find('{') {
            out.push_str(&rest[..at]);
            let Some(end) = rest[at..].find('}') else { break };
            out.push_str(&self.label(ctx, &rest[at + 1..at + end]));
            rest = &rest[at + end + 1..];
        }
        out.push_str(rest);
        out.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// The action that is the character `c` in `ctx`'s layer or [main]'s.
    pub fn name_of(ctx: Ctx, c: char) -> Option<&'static str> {
        let layer = match ctx {
            Ctx::View(v) => v.layer(),
            Ctx::Notes => "notes",
            Ctx::Help => "main",
        };
        let of = |l: &str| ACTIONS.iter().find(|(_, al, f)| *al == l && *f == Fire::Char(c)).map(|(n, _, _)| *n);
        of(layer).or_else(|| of("main"))
    }
}

/// Keys as the screen shows them; a chord of plain characters without the
/// spaces (`gg`).
fn human(keys: &[Key]) -> String {
    if keys.len() > 1 && keys.iter().all(|k| k.is_bare_char() && !k.shift && k.code != ito_keys::Code::Char(' ')) {
        keys.iter().map(Key::human).collect()
    } else {
        ito_keys::human_seq(keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(text: &str) -> (Keys, Vec<String>) {
        Keys::load(Some(("keymap.toml", text)))
    }

    fn run(k: &Keys, ctx: Ctx, on: &str) -> Option<Vec<Cmd>> {
        let on: Vec<Key> = on.split_whitespace().map(|t| Key::parse(t).unwrap()).collect();
        match ito_keymap::resolve(k.bindings(ctx), &on) {
            Match::Exact(b) => Some(b.run.clone()),
            _ => None,
        }
    }

    #[test]
    fn the_defaults_load_without_a_word() {
        let (k, warnings) = Keys::load(None);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(run(&k, Ctx::View(View::Inbox), "d"), Some(vec![Cmd::Run("done")]));
        assert_eq!(run(&k, Ctx::View(View::List), "d"), None);
        assert_eq!(run(&k, Ctx::Notes, "d"), Some(vec![Cmd::Run("read")]));
        assert_eq!(run(&k, Ctx::Help, "j"), Some(vec![Cmd::Run("down")]));
        assert_eq!(run(&k, Ctx::View(View::Board), "<S-Right>"), Some(vec![Cmd::Run("card_right")]));
        assert_eq!(run(&k, Ctx::View(View::Actions), "L"), Some(vec![Cmd::Run("log")]));
        assert_eq!(run(&k, Ctx::View(View::Status), "g g"), Some(vec![Cmd::Run("top")]));
        // Every action is bound somewhere, and each runs as something.
        for (name, _, _) in ACTIONS {
            assert!(fire(name).is_some());
            let bound = std::iter::once(Ctx::Notes)
                .chain(View::ALL.map(Ctx::View))
                .any(|c| k.bindings(c).iter().any(|b| b.run == [Cmd::Run(name)]));
            assert!(bound, "{name}");
        }
    }

    #[test]
    fn layers_follow_the_spec_and_the_defaults_stay_built_in() {
        let (k, _) = keys(
            r#"[main]
prepend_keymap = [{ on = "<F2>", run = "notes" }]"#,
        );
        let names: Vec<&str> = k.layers().iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["main", "inbox", "list", "actions", "board", "status", "notes"]);
        assert_eq!(k.layers()[0].1.len(), Keys::built_in_layers()[0].1.len() + 1);
        assert_eq!(Keys::fixed(&[Key::parse("<F1>").unwrap()]), Key::parse("<F1>").as_ref());
        assert_eq!(Keys::fixed(&[Key::parse("<F2>").unwrap()]), None);
    }

    #[test]
    fn a_users_key_goes_first_and_none_swallows_one() {
        let (k, warnings) = keys(
            r#"
            [inbox]
            prepend_keymap = [
                { on = "<C-d>", run = "done" },
                { on = "d", run = "none" },
            ]
            [main]
            prepend_keymap = [{ on = "<C-n>", run = "down" }]
            "#,
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        let inbox = Ctx::View(View::Inbox);
        assert_eq!(run(&k, inbox, "<C-d>"), Some(vec![Cmd::Run("done")]));
        assert_eq!(run(&k, inbox, "d"), Some(vec![Cmd::None]));
        assert_eq!(run(&k, Ctx::Notes, "<C-n>"), Some(vec![Cmd::Run("down")]));
        // The labels follow: `d` no longer does it.
        assert_eq!(k.label(inbox, "done"), if cfg!(target_os = "macos") { "⌘D" } else { "Ctrl+D" });
        assert_eq!(k.label(inbox, "down"), if cfg!(target_os = "macos") { "⌘N" } else { "Ctrl+N" });
    }

    #[test]
    fn an_action_of_another_layer_is_left_out_and_said() {
        let (k, warnings) = keys(
            r#"
            [main]
            prepend_keymap = [{ on = "<C-d>", run = "done" }, { on = "<C-q>", run = "fly" }]
            "#,
        );
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings.iter().any(|w| w.contains("`fly` is no action")), "{warnings:?}");
        assert!(warnings.iter().any(|w| w.contains("[main] `done` is no action of [main]")), "{warnings:?}");
        assert_eq!(run(&k, Ctx::View(View::Inbox), "<C-d>"), None);
        // The rest still works.
        assert_eq!(run(&k, Ctx::View(View::Inbox), "d"), Some(vec![Cmd::Run("done")]));
    }

    #[test]
    fn a_views_key_before_a_chord_of_main_is_said() {
        let (k, warnings) = keys(
            "[inbox]
prepend_keymap = [{ on = \"g\", run = \"later\" }]",
        );
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("[inbox] `g` hides [main]'s `gg`"), "{warnings:?}");
        assert_eq!(run(&k, Ctx::View(View::Inbox), "g"), Some(vec![Cmd::Run("later")]));
        assert_eq!(run(&k, Ctx::View(View::List), "g g"), Some(vec![Cmd::Run("top")]));
    }

    #[test]
    fn a_fixed_key_is_said_and_left_out() {
        let (k, warnings) = keys(
            r#"
            [main]
            prepend_keymap = [{ on = "<Esc>", run = "top" }, { on = "<F5>", run = "none" }, { on = "<C-c>", run = "copy_link" }]
            "#,
        );
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings.iter().all(|w| w.contains("is fixed")), "{warnings:?}");
        assert_eq!(run(&k, Ctx::Help, "<Esc>"), None);
        assert_eq!(run(&k, Ctx::Help, "<C-c>"), Some(vec![Cmd::Run("copy_link")]));
        // What main.rs presses for egui's Copy event.
        assert_eq!(Key::parse("<C-c>"), Some(Key::ctrl('c')));
    }

    #[test]
    fn the_labels_are_the_keys_in_force() {
        let k = Keys::default();
        let inbox = Ctx::View(View::Inbox);
        assert_eq!(k.fill(inbox, "{down} {up}"), "j k");
        assert_eq!(k.fill(inbox, "{top} {bottom}"), "gg G");
        assert_eq!(k.fill(Ctx::Help, "F1 {help}"), "F1 ?");
        assert_eq!(k.fill(inbox, "{details}"), "Space");
        assert_eq!(k.fill(inbox, "{next_view} {prev_view}"), if cfg!(target_os = "macos") { "Tab ⇧Tab" } else { "Tab Shift+Tab" });
        // Not bound in the list: nothing.
        assert_eq!(k.fill(Ctx::View(View::List), "{done}"), "");
        // A key taken by a binding before it is not shown.
        let (k, _) = keys("[inbox]\nprepend_keymap = [{ on = \"j\", run = \"later\" }]");
        assert_eq!(k.label(inbox, "down"), "Down");
        assert_eq!(k.label(Ctx::View(View::List), "down"), "j");
    }

    #[test]
    fn a_character_names_its_action() {
        assert_eq!(Keys::name_of(Ctx::View(View::Inbox), 'd'), Some("done"));
        assert_eq!(Keys::name_of(Ctx::View(View::Actions), 'o'), Some("open"));
        assert_eq!(Keys::name_of(Ctx::View(View::Board), 'L'), Some("card_right"));
        assert_eq!(Keys::name_of(Ctx::View(View::List), 'i'), None);
    }
}
