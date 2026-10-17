//! `--keys`: a script of keys and clicks played into the window, and the
//! state it leaves for a test to read (common-spec, "キーの台本"). The
//! parsing and the playing are `ito-keyscript`; what is minato's is the
//! state file's lines.

use crate::app::{App, Mode, View};

impl App {
    /// The state file: `key: value` lines, `focus:` first, then what a test
    /// of minato asks about. Lines a reader does not know are skipped, so
    /// more can be added.
    pub fn report(&self) -> String {
        let mut s = String::new();
        s += &format!("focus: {}\n", self.focus());
        s += &format!("scale: {}%\n", (self.prefs.common.scale_or_one() * 100.0).round());
        s += &format!("view: {}\n", self.view_name());
        s += &format!("mode: {:?}\n", self.mode);
        s += &format!("toast: {}\n", self.toast.as_ref().map_or("", |(t, _)| t.as_str()).replace('\n', "\\n"));
        s
    }

    fn view_name(&self) -> &'static str {
        match self.view {
            View::Inbox => "inbox",
            View::List => "list",
            View::Actions => "actions",
            View::Board => "board",
            View::Status => "status",
            View::Security => "security",
        }
    }

    /// Where the keys land: the box that has them, else the view.
    fn focus(&self) -> &'static str {
        match () {
            _ if self.mode == Mode::Settings => "settings",
            _ if self.mode == Mode::Help => "help",
            _ if self.mode == Mode::Notes => "notes",
            _ if self.confirm.is_some() => "confirm",
            _ if self.palette.is_some() => "palette",
            _ if self.compose.is_some() => "compose",
            _ if self.overlay.is_some() => "overlay",
            _ if self.find_open => "find",
            _ => self.view_name(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_report_starts_with_focus_and_names_the_view() {
        let a = App::new(crate::prefs::Prefs::empty());
        let r = a.report();
        assert!(r.starts_with("focus: "), "{r}");
        assert!(r.contains("scale: 100%\n") && r.contains("view: "), "{r}");
    }
}
