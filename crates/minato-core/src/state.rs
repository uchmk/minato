//! state.toml: what minato keeps between runs that is neither a setting nor
//! a mark. For now the columns' widths, as the mouse left them:
//!
//! ```toml
//! [columns]
//! inbox = [0.31]
//! list = [0.165, 0.44]
//! actions = [0.165, 0.44]
//! ```
//!
//! Each list is the view's split ratios in order (the outer one first). A
//! line that does not read is skipped; a ratio outside (0, 1) too.

/// The columns of the three views with columns.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub inbox: Vec<f32>,
    pub list: Vec<f32>,
    pub actions: Vec<f32>,
}

pub fn parse(text: &str) -> State {
    let mut out = State::default();
    let mut section = String::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.trim().to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        if section != "columns" {
            continue;
        }
        let slot = match key.trim() {
            "inbox" => &mut out.inbox,
            "list" => &mut out.list,
            "actions" => &mut out.actions,
            _ => continue,
        };
        *slot = ratios(value);
    }
    out
}

/// `[0.2, 0.5]` as numbers; nothing if any of them does not read.
fn ratios(value: &str) -> Vec<f32> {
    let Some(inner) = value.trim().strip_prefix('[').and_then(|v| v.strip_suffix(']')) else { return Vec::new() };
    let got: Option<Vec<f32>> = inner.split(',').map(str::trim).filter(|s| !s.is_empty()).map(|s| s.parse::<f32>().ok().filter(|r| *r > 0.0 && *r < 1.0)).collect();
    got.unwrap_or_default()
}

pub fn format(s: &State) -> String {
    let list = |v: &[f32]| v.iter().map(|r| format!("{:.4}", r).trim_end_matches('0').trim_end_matches('.').to_string()).collect::<Vec<_>>().join(", ");
    format!("[columns]\ninbox = [{}]\nlist = [{}]\nactions = [{}]\n", list(&s.inbox), list(&s.list), list(&s.actions))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_back_what_it_writes() {
        let s = State { inbox: vec![0.31], list: vec![0.165, 0.44], actions: vec![0.2, 0.5] };
        let text = format(&s);
        assert_eq!(text, "[columns]\ninbox = [0.31]\nlist = [0.165, 0.44]\nactions = [0.2, 0.5]\n");
        assert_eq!(parse(&text), s);
    }

    #[test]
    fn skips_what_does_not_read() {
        let s = parse("# minato\n[other]\ninbox = [0.5]\n[columns]\ninbox = [0.4, x]\nlist = [1.5]\nactions = 0.3\nweird\nlist = [ 0.25 , 0.5 ]\n");
        assert_eq!(s.inbox, Vec::<f32>::new());
        assert_eq!(s.list, vec![0.25, 0.5]);
        assert!(s.actions.is_empty());
        assert_eq!(parse(""), State::default());
    }
}
