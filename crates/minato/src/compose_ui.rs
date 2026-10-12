//! The writing box and the merge confirmation, drawn over the view. The box
//! holds a multi-line field (or the preview) and, on Alt+L / Alt+A, the picker
//! for labels and assignees. No scrollbar: the field shows the end of what
//! is typed, the picker a window around its cursor.

use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;
use egui::{Align2, CornerRadius, Event, FontId, Id, Key, Modifiers, Rect, Stroke, StrokeKind, Ui, UiBuilder, pos2, vec2};
use minato_core::composer::{Block, Picker, blocks, link, wrap};

use crate::app::{App, Compose, Pick};
use crate::ui::ACCENT;

const PANEL_H: f32 = 300.0;
const KEYBAR_H: f32 = 28.0;
const ROW: f32 = 22.0;

enum Act {
    Stay,
    Send(bool),
    Close,
    Apply,
}

pub fn show(ui: &mut Ui, app: &mut App) {
    if app.confirm.is_some() {
        confirm(ui, app);
    } else if app.compose.is_some() {
        compose(ui, app);
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
    p.text(pos2(b.left() + 18.0, b.top() + 62.0), Align2::LEFT_CENTER, app.tr("confirm.merge"), FontId::proportional(14.0), c.fg);
    p.text(pos2(b.left() + 18.0, b.top() + 94.0), Align2::LEFT_CENTER, app.tr("confirm.hint"), FontId::proportional(11.5), ACCENT);
}

fn compose(ui: &mut Ui, app: &mut App) {
    let ctx = ui.ctx().clone();
    let colors = ito_theme::colors();
    let Some(mut c) = app.compose.take() else { return };
    let item = app.items.iter().find(|i| i.id == c.id).cloned();
    let area = ui.max_rect();
    let panel = Rect::from_min_max(pos2(area.left() + 20.0, area.bottom() - KEYBAR_H - 8.0 - PANEL_H), pos2(area.right() - 20.0, area.bottom() - KEYBAR_H - 8.0));
    let p = ui.painter().clone();
    p.rect_filled(panel, CornerRadius::same(8), colors.panel);
    p.rect_stroke(panel, CornerRadius::same(8), Stroke::new(1.0, ACCENT), StrokeKind::Inside);
    let head = item.as_ref().map(|i| format!("{}  {} #{}  {}", app.tr("compose.title"), i.repo, i.number, i.title)).unwrap_or_default();
    p.with_clip_rect(panel.shrink2(vec2(14.0, 0.0))).text(pos2(panel.left() + 14.0, panel.top() + 18.0), Align2::LEFT_CENTER, head, FontId::proportional(13.0), colors.fg);
    if c.preview {
        p.text(pos2(panel.right() - 14.0, panel.top() + 18.0), Align2::RIGHT_CENTER, app.tr("compose.preview"), FontId::proportional(11.5), ACCENT);
    }
    let hint = if c.picker.is_some() { app.tr("pick.hint") } else { app.tr("compose.hint") };
    p.with_clip_rect(panel).text(pos2(panel.left() + 14.0, panel.bottom() - 14.0), Align2::LEFT_CENTER, hint, FontId::proportional(11.0), colors.dim);
    let field = Rect::from_min_max(pos2(panel.left() + 14.0, panel.top() + 36.0), pos2(panel.right() - 14.0, panel.bottom() - 30.0));
    let id = Id::new("minato-compose");
    let mut act = Act::Stay;
    if c.picker.is_some() {
        picker(&ctx, &p, &mut c, field, &mut act, [app.tr("pick.loading"), app.tr("pick.labels"), app.tr("pick.assignees")]);
    } else {
        // The shortcuts go first, so the field never sees them.
        let (send_close, send, preview, bold, italic, key_link, labels, people, esc) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Enter),
                i.consume_key(Modifiers::COMMAND, Key::Enter),
                i.consume_key(Modifiers::COMMAND, Key::P),
                i.consume_key(Modifiers::COMMAND, Key::B),
                i.consume_key(Modifiers::COMMAND, Key::I),
                i.consume_key(Modifiers::COMMAND, Key::K),
                i.consume_key(Modifiers::ALT, Key::L),
                i.consume_key(Modifiers::ALT, Key::A),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
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
        } else if labels || people {
            let (kind, options, chosen) = if labels {
                (Pick::Labels, c.labels.clone(), item.as_ref().map(|i| i.labels.iter().map(|l| l.name.clone()).collect()).unwrap_or_default())
            } else {
                (Pick::Assignees, c.people.clone(), item.as_ref().map(|i| i.assignees.clone()).unwrap_or_default())
            };
            c.picker = Some((kind, Picker::new(options, chosen)));
        } else if bold || italic || key_link {
            let mut st = TextEditState::load(&ctx, id).unwrap_or_default();
            let end = c.text.chars().count();
            let (a, b) = st.cursor.char_range().map_or((end, end), |r| (r.primary.index.into(), r.secondary.index.into()));
            let (text, a, b) = if bold {
                wrap(&c.text, a, b, "**", "**")
            } else if italic {
                wrap(&c.text, a, b, "_", "_")
            } else {
                link(&c.text, a, b)
            };
            c.text = text;
            st.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(a), CCursor::new(b))));
            st.store(&ctx, id);
        }
        if c.preview {
            draw_preview(&p, field, &c.text, app.tr("compose.empty"));
        } else if c.picker.is_none() {
            let mut child = ui.new_child(UiBuilder::new().max_rect(field));
            child.set_clip_rect(field);
            let edit = egui::TextEdit::multiline(&mut c.text).id(id).frame(egui::Frame::NONE).desired_width(field.width()).desired_rows(8).lock_focus(true).font(FontId::proportional(13.5));
            child.add(edit);
            ctx.memory_mut(|m| m.request_focus(id));
        }
    }
    match act {
        Act::Stay => app.compose = Some(c),
        Act::Close => {}
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

/// The picker. `words` are loading, labels, assignees.
fn picker(ctx: &egui::Context, p: &egui::Painter, c: &mut Compose, field: Rect, act: &mut Act, words: [&str; 3]) {
    let colors = ito_theme::colors();
    let Some((kind, pk)) = c.picker.as_mut() else { return };
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
    let word = if *kind == Pick::Labels { words[1] } else { words[2] };
    p.text(pos2(field.left(), field.top() + 10.0), Align2::LEFT_CENTER, format!("{word}  /  {}", pk.query), FontId::proportional(13.0), ACCENT);
    let body = Rect::from_min_max(pos2(field.left(), field.top() + 26.0), field.max);
    if pk.options.is_empty() {
        p.text(pos2(body.left(), body.top() + ROW / 2.0), Align2::LEFT_CENTER, words[0], FontId::proportional(12.5), colors.dim);
        return;
    }
    let shown = pk.shown();
    let fit = ((body.height() / ROW) as usize).max(1);
    let first = pk.cursor.saturating_sub(fit - 1);
    for (k, name) in shown.iter().enumerate().skip(first).take(fit) {
        let row = Rect::from_min_size(pos2(body.left(), body.top() + (k - first) as f32 * ROW), vec2(body.width(), ROW));
        if k == pk.cursor {
            p.rect_filled(row, 0.0, ACCENT.gamma_multiply(0.16));
        }
        let mark = if pk.is_chosen(name) { "[x]" } else { "[ ]" };
        p.text(pos2(row.left() + 6.0, row.center().y), Align2::LEFT_CENTER, format!("{mark}  {name}"), FontId::monospace(12.0), colors.fg);
    }
}

/// The preview of the Markdown, painted as a few kinds of block; it shows
/// the end when the text is longer than the field.
fn draw_preview(p: &egui::Painter, field: Rect, text: &str, empty: &str) {
    let colors = ito_theme::colors();
    let p = p.with_clip_rect(field);
    if text.trim().is_empty() {
        p.text(pos2(field.left(), field.top() + 10.0), Align2::LEFT_CENTER, empty, FontId::proportional(12.5), colors.dim);
        return;
    }
    let width = field.width();
    let mut laid = Vec::new();
    for b in blocks(text) {
        let (s, font, color, indent) = match b {
            Block::Heading(n, s) => (s, FontId::proportional(20.0 - n as f32 * 1.5), colors.fg, 0.0),
            Block::Bullet(s) => (format!("•  {s}"), FontId::proportional(13.5), colors.fg, 8.0),
            Block::Quote(s) => (s, FontId::proportional(13.5), colors.dim, 14.0),
            Block::Code(s) => (s, FontId::monospace(12.5), ACCENT, 8.0),
            Block::Para(s) => (s, FontId::proportional(13.5), colors.fg, 0.0),
        };
        laid.push((p.layout(s, font, color, width - indent), indent));
    }
    let total: f32 = laid.iter().map(|(g, _)| g.size().y + 8.0).sum();
    let mut y = field.top() - (total - field.height()).max(0.0);
    for (g, indent) in laid {
        let h = g.size().y + 8.0;
        p.galley(pos2(field.left() + indent, y), g, colors.fg);
        y += h;
    }
}
