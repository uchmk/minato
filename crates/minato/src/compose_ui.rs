//! The writing box and the merge confirmation. On an item the box lives in
//! the detail's strip (`at`, the field the strip left for it: GitHub's box,
//! drawn by `detail_ui`); a new issue, or a view without the detail, gets it
//! as a panel over the view. It holds a multi-line field (or the preview)
//! and, on Alt+L / Alt+A and the rest (or a click on a field chip), the
//! picker. No scrollbar: the field shows the end of what is typed, the
//! picker a window around its cursor.

use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;
use egui::{Align2, CornerRadius, Event, FontId, Id, Key, Modifiers, Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder, pos2, vec2};
use minato_core::composer::{Tool, tool};

use crate::app::{App, Compose, Pick};
use crate::ui::ACCENT;

const PANEL_H: f32 = 300.0;
const KEYBAR_H: f32 = 28.0;
const ROW: f32 = 22.0;
/// The picker's panel over the box in the detail.
const PICK_H: f32 = 280.0;

enum Act {
    Stay,
    Send(bool),
    Close,
    Apply,
}

/// `at`: the field the detail's strip left for the box, if it is showing it.
pub fn show(ui: &mut Ui, app: &mut App, at: Option<Rect>) {
    if app.confirm.is_some() {
        confirm(ui, app);
    } else if app.compose.is_some() {
        compose(ui, app, at);
    }
}

fn confirm(ui: &mut Ui, app: &App) {
    let c = ito_theme::colors();
    let r = ui.max_rect();
    let p = ui.painter();
    p.rect_filled(r, 0.0, egui::Color32::from_black_alpha(110));
    let b = Rect::from_center_size(r.center(), vec2(440.0, 120.0));
    p.rect_filled(b, CornerRadius::same(8), c.panel);
    p.rect_stroke(b, CornerRadius::same(8), Stroke::new(1.0, ACCENT), StrokeKind::Inside);
    let title = app.confirm.as_ref().and_then(|id| app.items.iter().find(|i| &i.id == id)).map(|i| format!("{} #{}  {}", i.repo, i.number, i.title)).unwrap_or_default();
    p.with_clip_rect(b.shrink(14.0)).text(pos2(b.left() + 18.0, b.top() + 32.0), Align2::LEFT_CENTER, title, FontId::proportional(12.5), c.dim);
    p.text(pos2(b.left() + 18.0, b.top() + 62.0), Align2::LEFT_CENTER, app.tr(if app.confirm_act == crate::app::Confirm::Close { "confirm.close" } else { "confirm.merge" }), FontId::proportional(14.0), c.fg);
    p.text(pos2(b.left() + 18.0, b.top() + 94.0), Align2::LEFT_CENTER, app.tr(if app.confirm_act == crate::app::Confirm::Close { "confirm.close_hint" } else { "confirm.hint" }), FontId::proportional(11.5), ACCENT);
}

fn compose(ui: &mut Ui, app: &mut App, at: Option<Rect>) {
    let ctx = ui.ctx().clone();
    let colors = ito_theme::colors();
    let Some(mut c) = app.compose.take() else { return };
    let item = app.items.iter().find(|i| i.id == c.id).cloned();
    let new_repo = c.id.strip_prefix("new:").map(str::to_string);
    let p = ui.painter().clone();
    let inline = at.filter(|_| new_repo.is_none());
    let field = match inline {
        Some(field) => field,
        None => panel(&p, app, &c, item.as_ref(), new_repo.as_deref(), ui.max_rect()),
    };
    // In the detail the picker opens as a panel over the box. `outer`: what
    // takes the picker's clicks (a click outside it closes it).
    let (pick_at, outer) = match inline {
        Some(f) => {
            let r = Rect::from_min_max(pos2(f.left() - 12.0, f.bottom() - PICK_H), pos2(f.right() + 12.0, f.bottom()));
            if c.picker.is_some() {
                p.rect_filled(r, CornerRadius::same(8), colors.panel);
                p.rect_stroke(r, CornerRadius::same(8), Stroke::new(1.0, ACCENT), StrokeKind::Inside);
            }
            (r.shrink2(vec2(14.0, 10.0)), r)
        }
        None => (field, Rect::from_min_max(field.min - vec2(14.0, 36.0), field.max + vec2(14.0, 30.0))),
    };
    let id = Id::new("minato-compose");
    let mut act = Act::Stay;
    if c.picker.is_some() {
        let title = match c.picker.as_ref().map(|(k, _)| *k) {
            Some(Pick::Labels) => app.tr("pick.labels"),
            Some(Pick::Assignees) => app.tr("pick.assignees"),
            Some(Pick::Status) => app.tr("pick.status"),
            Some(Pick::Milestone) => app.tr("pick.milestone"),
            Some(Pick::Reviewers) => app.tr("pick.reviewers"),
            _ => app.tr("pick.priority"),
        };
        picker(ui, &p, &mut c, [pick_at, outer], &mut act, [app.tr("pick.loading"), title]);
        if inline.is_some() {
            p.text(pos2(pick_at.right(), pick_at.top() + 10.0), Align2::RIGHT_CENTER, app.tr("pick.hint"), FontId::proportional(11.0), colors.dim);
        }
    } else {
        // The shortcuts go first, so the field never sees them.
        let (send_close, send, preview, bold, italic, key_link, labels, people, status, priority, mile, review, esc) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Enter),
                i.consume_key(Modifiers::COMMAND, Key::Enter),
                i.consume_key(Modifiers::COMMAND, Key::P),
                i.consume_key(Modifiers::COMMAND, Key::B),
                i.consume_key(Modifiers::COMMAND, Key::I),
                i.consume_key(Modifiers::COMMAND, Key::K),
                i.consume_key(Modifiers::ALT, Key::L),
                i.consume_key(Modifiers::ALT, Key::A),
                i.consume_key(Modifiers::ALT, Key::S),
                i.consume_key(Modifiers::ALT, Key::P),
                i.consume_key(Modifiers::ALT, Key::M),
                i.consume_key(Modifiers::ALT, Key::R),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        // A field the settings hide does nothing but say so.
        let fresh = new_repo.is_none();
        let (labels, people, status, priority, mile, review) = (
            fresh && labels && app.field_shown(0),
            fresh && people && app.field_shown(1),
            fresh && status && app.field_shown(2),
            fresh && priority && app.field_shown(4),
            fresh && mile && app.field_shown(3),
            fresh && review && app.field_shown(5),
        );
        let key_tool = if bold {
            Some(Tool::Bold)
        } else if italic {
            Some(Tool::Italic)
        } else if key_link {
            Some(Tool::Link)
        } else {
            None
        };
        if send_close || send {
            act = Act::Send(send_close);
        } else if esc {
            if c.preview {
                c.preview = false;
            } else {
                act = Act::Close;
            }
        } else if preview {
            c.preview = !c.preview;
        } else if let Some(k) = [labels, people, status, mile, priority, review].iter().position(|&on| on) {
            app.compose = Some(c);
            app.open_pick(k);
            c = app.compose.take().expect("the box is still open");
        } else if let Some(t) = key_tool.or(c.tool.take()) {
            // A mark on the selection, from Ctrl+B/I/K or the toolbar.
            let mut st = TextEditState::load(&ctx, id).unwrap_or_default();
            let end = c.text.chars().count();
            let (a, b) = st.cursor.char_range().map_or((end, end), |r| {
                let (x, y): (usize, usize) = (r.primary.index.into(), r.secondary.index.into());
                (x.min(y), x.max(y))
            });
            let (text, a, b) = tool(&c.text, a, b, t);
            c.text = text;
            st.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(a), CCursor::new(b))));
            st.store(&ctx, id);
        }
        c.tool = None;
        if c.preview {
            draw_preview(&p, field, &c.text, app.tr("compose.empty"));
        } else if c.picker.is_none() {
            let mut child = ui.new_child(UiBuilder::new().max_rect(field));
            child.set_clip_rect(field);
            let rows = ((field.height() / 19.0) as usize).max(2);
            let edit = egui::TextEdit::multiline(&mut c.text).id(id).frame(egui::Frame::NONE).desired_width(field.width()).desired_rows(rows).lock_focus(true).font(FontId::proportional(13.5));
            child.add(edit);
            ctx.memory_mut(|m| m.request_focus(id));
        }
    }
    match act {
        Act::Stay => app.compose = Some(c),
        Act::Close => {
            app.compose = Some(c);
            app.close_compose();
        }
        Act::Send(close) => {
            app.compose = Some(c);
            app.send_comment(close);
        }
        Act::Apply => {
            app.compose = Some(c);
            app.apply_pick();
        }
    }
}

/// The box as a panel over the view (a new issue, or no detail to hold it):
/// its head and the keys at its foot. The field inside it.
fn panel(p: &egui::Painter, app: &App, c: &Compose, item: Option<&minato_core::list::Item>, new_repo: Option<&str>, area: Rect) -> Rect {
    let colors = ito_theme::colors();
    let panel = Rect::from_min_max(pos2(area.left() + 20.0, area.bottom() - KEYBAR_H - 8.0 - PANEL_H), pos2(area.right() - 20.0, area.bottom() - KEYBAR_H - 8.0));
    p.rect_filled(panel, CornerRadius::same(8), colors.panel);
    p.rect_stroke(panel, CornerRadius::same(8), Stroke::new(1.0, ACCENT), StrokeKind::Inside);
    let head = match (new_repo, item) {
        (Some(repo), _) => format!("{}  {}", app.tr("compose.new"), repo),
        (_, Some(i)) => format!("{}  {} #{}  {}", app.tr("compose.title"), i.repo, i.number, i.title),
        _ => String::new(),
    };
    p.with_clip_rect(panel.shrink2(vec2(14.0, 0.0))).text(pos2(panel.left() + 14.0, panel.top() + 18.0), Align2::LEFT_CENTER, head, FontId::proportional(13.0), colors.fg);
    if c.preview {
        p.text(pos2(panel.right() - 14.0, panel.top() + 18.0), Align2::RIGHT_CENTER, app.tr("compose.preview"), FontId::proportional(11.5), ACCENT);
    }
    let hint = if c.picker.is_some() {
        app.tr("pick.hint").to_string()
    } else {
        let mut h = app.tr("compose.hint").to_string();
        for k in app.settings.order {
            if app.settings.fields[k] {
                h.push_str(&format!("  Alt+{} {}", ["L", "A", "S", "M", "P", "R"][k], app.tr(["pick.labels", "pick.assignees", "pick.status", "pick.milestone", "pick.priority", "pick.reviewers"][k])));
            }
        }
        h.push_str("  ");
        h.push_str(app.tr("compose.esc"));
        h
    };
    p.with_clip_rect(panel).text(pos2(panel.left() + 14.0, panel.bottom() - 14.0), Align2::LEFT_CENTER, hint, FontId::proportional(11.0), colors.dim);
    Rect::from_min_max(pos2(panel.left() + 14.0, panel.top() + 36.0), pos2(panel.right() - 14.0, panel.bottom() - 30.0))
}

/// The picker in `at[0]`, taking the clicks over `at[1]`. `words` are
/// loading and the title. A click on a row ticks it (and closes a picker of
/// one choice); a click outside closes it.
fn picker(ui: &Ui, p: &egui::Painter, c: &mut Compose, at: [Rect; 2], act: &mut Act, words: [&str; 2]) {
    let colors = ito_theme::colors();
    let ctx = ui.ctx();
    let [field, outer] = at;
    let Some((_, pk)) = c.picker.as_mut() else { return };
    ui.interact(outer, Id::new("minato-pick"), Sense::click());
    if ctx.input(|i| i.pointer.primary_clicked() && i.pointer.interact_pos().is_some_and(|pos| !outer.contains(pos))) {
        *act = Act::Apply;
    }
    let events = ctx.input_mut(|i| std::mem::take(&mut i.events));
    for e in events {
        match e {
            Event::Key { key: Key::Escape, pressed: true, .. } => *act = Act::Apply,
            Event::Key { key: Key::Enter, pressed: true, .. } => pk.toggle(),
            Event::Key { key: Key::ArrowDown, pressed: true, .. } => pk.step(true),
            Event::Key { key: Key::ArrowUp, pressed: true, .. } => pk.step(false),
            Event::Key { key: Key::Backspace, pressed: true, .. } => pk.backspace(),
            Event::Text(t) => t.chars().filter(|ch| !ch.is_control()).for_each(|ch| pk.type_char(ch)),
            _ => {}
        }
    }
    let word = words[1];
    p.text(pos2(field.left(), field.top() + 10.0), Align2::LEFT_CENTER, format!("{word}  /  {}", pk.query), FontId::proportional(13.0), ACCENT);
    let body = Rect::from_min_max(pos2(field.left(), field.top() + 26.0), field.max);
    if pk.options.is_empty() {
        p.text(pos2(body.left(), body.top() + ROW / 2.0), Align2::LEFT_CENTER, words[0], FontId::proportional(12.5), colors.dim);
        return;
    }
    let shown = pk.shown();
    let mut clicked = None;
    let fit = ((body.height() / ROW) as usize).max(1);
    let first = pk.cursor.saturating_sub(fit - 1);
    for (k, name) in shown.iter().enumerate().skip(first).take(fit) {
        let row = Rect::from_min_size(pos2(body.left(), body.top() + (k - first) as f32 * ROW), vec2(body.width(), ROW));
        let resp = crate::ui::hit(ui, row, ("pick-row", k));
        if k == pk.cursor {
            p.rect_filled(row, 0.0, ACCENT.gamma_multiply(0.16));
        } else if resp.hovered() {
            p.rect_filled(row, 0.0, colors.hover());
        }
        if resp.clicked() {
            clicked = Some(k);
        }
        let mark = match (pk.single, pk.is_chosen(name)) {
            (true, true) => "(o)",
            (true, false) => "( )",
            (false, true) => "[x]",
            (false, false) => "[ ]",
        };
        p.text(pos2(row.left() + 6.0, row.center().y), Align2::LEFT_CENTER, format!("{mark}  {name}"), FontId::monospace(12.0), colors.fg);
    }
    if let Some(k) = clicked {
        pk.cursor = k;
        pk.toggle();
        if pk.single {
            *act = Act::Apply;
        }
    }
}

/// The preview of the Markdown as the detail draws it; it shows the end
/// when the text is longer than the field.
fn draw_preview(p: &egui::Painter, field: Rect, text: &str, empty: &str) {
    let colors = ito_theme::colors();
    let p = p.with_clip_rect(field);
    if text.trim().is_empty() {
        p.text(pos2(field.left(), field.top() + 10.0), Align2::LEFT_CENTER, empty, FontId::proportional(12.5), colors.dim);
        return;
    }
    let md = crate::md_ui::layout(&p, text, field.width(), 13.5);
    md.paint(&p, field.left(), field.top() - (md.height - field.height()).max(0.0));
}
