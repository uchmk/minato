//! Text of the detail picked up with the mouse and copied, as in tsumugi's
//! terminal. The detail is painted, not made of widgets, so each text it
//! paints is noted here just before it is painted, and a drag, a double click
//! (a word, or the whole web address under it), a triple click (a line) or
//! Ctrl+click on a web address does the rest through `ito-select`.

use std::cell::RefCell;
use std::sync::Arc;

use egui::{Galley, Painter, Pos2, Rect, Response};
use ito_select::{Driven, Select};

thread_local! {
    static STATE: RefCell<Select> = RefCell::new(Select::default());
}

/// A new frame of the detail begins; `owner` is what it shows.
pub fn begin(owner: &str) {
    STATE.with(|s| s.borrow_mut().begin(owner));
}

/// A text about to be painted with `p`, its top left at `pos`: its
/// selection goes there, under the letters.
pub fn note(p: &Painter, pos: Pos2, galley: &Arc<Galley>) {
    STATE.with(|s| s.borrow_mut().note(p, pos, galley));
}

/// Follow the mouse on the detail: what a selection just made picked up (to
/// copy), and the web address a Ctrl+click landed on.
pub fn drive(resp: &Response, view: Rect) -> Driven {
    STATE.with(|s| s.borrow_mut().drive(resp, view))
}

/// Whether `at` is over a web address (for the pointing hand).
pub fn url_at(at: Pos2, view: Rect) -> bool {
    STATE.with(|s| s.borrow().url_at(at, view).is_some())
}

/// What is selected.
pub fn text() -> Option<String> {
    STATE.with(|s| s.borrow().text())
}

/// Paint the selection behind the texts noted this frame, in tsumugi's
/// colour (the theme's, from ito-select).
pub fn paint() {
    let fill = ito_theme::colors().selection();
    STATE.with(|s| s.borrow_mut().paint(fill));
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Color32, FontId};
    use ito_select::At;

    fn texts() -> Select {
        let ctx = egui::Context::default();
        let mut s = Select::default();
        let mut out = ctx.run_ui(Default::default(), |ui| {
            s.begin("a");
            for (k, t) in ["hello", "world"].into_iter().enumerate() {
                let g = ui.painter().layout_no_wrap(t.to_string(), FontId::proportional(13.0), Color32::WHITE);
                s.note(ui.painter(), Pos2::new(10.0, 10.0 + 30.0 * k as f32), &g);
            }
        });
        out.textures_delta.clear();
        s.begin("a");
        s
    }

    #[test]
    fn a_selection_between_two_texts_copies_what_lies_between() {
        let mut s = texts();
        s.set(Some((At { run: 0, ch: 2 }, At { run: 1, ch: 3 })));
        assert_eq!(s.text().as_deref(), Some("llo\nwor"));
        s.begin("b");
        assert_eq!(s.get(), None, "another item drops it");
    }
}
