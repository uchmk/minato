//! Text of the detail picked up with the mouse and copied. The detail is
//! painted, not made of widgets, so each text it paints is noted here as it
//! goes (`note`, in the order they are painted), and a drag over them
//! becomes a selection between two places in that order: a text's number
//! and a character in it. Ctrl+C then copies what lies between.

use std::cell::RefCell;
use std::sync::Arc;

use egui::epaint::text::cursor::CCursor;
use egui::{Color32, Galley, Painter, Pos2, Rect};

use crate::style::ACCENT;

/// A place in the painted texts: which one, and the character in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct At {
    pub run: usize,
    pub ch: usize,
}

struct Run {
    pos: Pos2,
    galley: Arc<Galley>,
}

#[derive(Default)]
struct State {
    /// The texts of the frame being painted, and of the one before it.
    runs: Vec<Run>,
    last: Vec<Run>,
    /// Where the drag began and where it is.
    sel: Option<(At, At)>,
    /// What the selection belongs to (the item shown): another item drops it.
    owner: String,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// A new frame of the detail begins; `owner` is what it shows.
pub fn begin(owner: &str) {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.last = std::mem::take(&mut s.runs);
        if s.owner != owner {
            s.owner = owner.to_string();
            s.sel = None;
        }
    });
}

/// A text painted with its top left at `pos`.
pub fn note(pos: Pos2, galley: &Arc<Galley>) {
    STATE.with(|s| s.borrow_mut().runs.push(Run { pos, galley: galley.clone() }));
}

/// The place under `at`, among the texts that show in `view`: inside one, or
/// the nearest.
pub fn place(at: Pos2, view: Rect) -> Option<At> {
    STATE.with(|s| {
        let s = s.borrow();
        let near = |r: &Run| {
            let rect = Rect::from_min_size(r.pos, r.galley.size());
            let dx = (rect.left() - at.x).max(at.x - rect.right()).max(0.0);
            let dy = (rect.top() - at.y).max(at.y - rect.bottom()).max(0.0);
            dx * dx + dy * dy * 4.0
        };
        let (run, r) = s
            .runs
            .iter()
            .enumerate()
            .filter(|(_, r)| Rect::from_min_size(r.pos, r.galley.size()).intersects(view))
            .min_by(|a, b| near(a.1).total_cmp(&near(b.1)))?;
        let c = r.galley.cursor_from_pos(at - r.pos);
        Some(At { run, ch: usize::from(c.index) })
    })
}

/// The selection, if there is one.
pub fn set(sel: Option<(At, At)>) {
    STATE.with(|s| s.borrow_mut().sel = sel);
}

pub fn get() -> Option<(At, At)> {
    STATE.with(|s| s.borrow().sel)
}

/// The selection's two ends in text order; none when they are one place.
fn span(sel: Option<(At, At)>) -> Option<(At, At)> {
    sel.filter(|(a, b)| a != b).map(|(a, b)| (a.min(b), a.max(b)))
}

/// What is selected, a line for each text it takes in.
pub fn text() -> Option<String> {
    STATE.with(|s| {
        let s = s.borrow();
        // The texts of the last frame: the ones the user saw.
        let (a, b) = span(s.sel)?;
        let mut parts = Vec::new();
        for run in a.run..=b.run {
            let g = &s.last.get(run)?.galley;
            let chars: Vec<char> = g.text().chars().collect();
            let from = if run == a.run { a.ch.min(chars.len()) } else { 0 };
            let to = if run == b.run { b.ch.min(chars.len()) } else { chars.len() };
            parts.push(chars[from..to.max(from)].iter().collect::<String>());
        }
        Some(parts.join("\n")).filter(|t| !t.is_empty())
    })
}

/// Paint the selection over the texts noted this frame; drops it when the
/// texts are no longer there (the detail changed under it).
pub fn paint(p: &Painter) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let Some((a, b)) = span(s.sel) else { return };
        if b.run >= s.runs.len() {
            s.sel = None;
            return;
        }
        let fill = Color32::from_rgba_unmultiplied(ACCENT.r(), ACCENT.g(), ACCENT.b(), 90);
        for run in a.run..=b.run {
            let r = &s.runs[run];
            let total: usize = r.galley.rows.iter().map(|row| usize::from(row.char_count_including_newline())).sum();
            let from = if run == a.run { a.ch } else { 0 };
            let to = if run == b.run { b.ch } else { total };
            let mut at = 0;
            for row in &r.galley.rows {
                let n = usize::from(row.char_count_excluding_newline());
                let (lo, hi) = (from.max(at), to.min(at + n));
                // A row a selection runs through to its end is filled to it.
                let through = run != b.run || to > at + n;
                if lo < hi || (lo == hi && through && from <= at + n && to > at) {
                    let x0 = r.galley.pos_from_cursor(CCursor::new(lo.max(at))).min.x;
                    let x1 = if through && hi >= at + n {
                        r.galley.pos_from_cursor(CCursor::new(at + n)).min.x + 4.0
                    } else {
                        r.galley.pos_from_cursor(CCursor::new(hi)).min.x
                    };
                    let y = row.rect().y_range();
                    if x1 > x0 {
                        p.rect_filled(Rect::from_x_y_ranges(x0..=x1, y).translate(r.pos.to_vec2()), 0.0, fill);
                    }
                }
                at += usize::from(row.char_count_including_newline());
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{FontId, Rect};

    /// Two texts painted one under the other in a frame.
    fn paint_two(mut f: impl FnMut()) {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(Default::default(), |ui| {
            begin("a");
            for (k, s) in ["hello", "world"].into_iter().enumerate() {
                let g = ui.painter().layout_no_wrap(s.to_string(), FontId::proportional(13.0), Color32::WHITE);
                note(Pos2::new(10.0, 10.0 + 30.0 * k as f32), &g);
            }
            f();
        });
        out.textures_delta.clear();
    }

    #[test]
    fn a_drag_between_two_texts_copies_what_lies_between() {
        paint_two(|| {
            let view = Rect::from_min_max(Pos2::ZERO, Pos2::new(300.0, 300.0));
            // Inside the first text, and below the second: the nearest ones.
            let a = place(Pos2::new(11.0, 15.0), view).unwrap();
            let b = place(Pos2::new(200.0, 80.0), view).unwrap();
            assert_eq!((a.run, b.run), (0, 1));
            assert_eq!(b.ch, 5, "past its end is its end");
            set(Some((At { run: 0, ch: 2 }, At { run: 1, ch: 3 })));
        });
        // The next frame begins: what was painted is what a copy reads.
        begin("a");
        assert_eq!(text().as_deref(), Some("llo\nwor"));
        set(Some((At { run: 1, ch: 3 }, At { run: 0, ch: 2 })));
        assert_eq!(text().as_deref(), Some("llo\nwor"), "backwards is the same");
        set(Some((At { run: 0, ch: 1 }, At { run: 0, ch: 1 })));
        assert_eq!(text(), None);
        // Another item shown drops it.
        set(Some((At { run: 0, ch: 0 }, At { run: 1, ch: 1 })));
        begin("b");
        assert_eq!(get(), None);
    }
}
