//! `minato.com`: the console-subsystem front for `minato.exe`, which the
//! release zip ships beside it. PowerShell does not wait for a windowed
//! program and does not take its output (`$v = & minato.exe --version` came
//! back empty); `PATHEXT` lists `.COM` before `.EXE`, so typing `minato`
//! finds this one first. The front itself is `ito_console::front`, shared
//! with kura, yagura, tsumugi and michi; what is minato's is which command
//! lines answer in text.
//!
//! It holds no logic of its own beyond that choice: the version and the help
//! come from `minato.exe`, so the two cannot disagree.

/// Whether `minato.exe` will answer this command line in text and exit rather
/// than open a window. Any argument does: `main` in `main.rs` reads the first
/// and refuses what it does not know. Kept in step with it.
fn answers_in_text(args: &[std::ffi::OsString]) -> bool {
    !args.is_empty()
}

fn main() {
    ito_console::front("minato", answers_in_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(args: &[&str]) -> Vec<std::ffi::OsString> {
        args.iter().map(Into::into).collect()
    }

    /// Every argument is answered in text; only none opens the window.
    #[test]
    fn it_waits_only_for_what_answers_in_text() {
        for waits in [&["--version"][..], &["-V"], &["--help"], &["-h"], &["--bogus"], &["env"], &["env", "--out", "r.txt"], &["inbox"]] {
            assert!(answers_in_text(&line(waits)), "{waits:?}");
        }
        assert!(!answers_in_text(&line(&[])));
    }
}
