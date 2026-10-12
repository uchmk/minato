//! The notifications, over the view: one row per unread thread, folded by
//! `minato_core::fold` (no scrollbar).

use egui::{Align2, CornerRadius, FontId, Rect, Stroke, Ui, pos2, vec2};
use minato_core::fold::{Show, fold};
use minato_core::list::{age, epoch};

use crate::app::{App, now_epoch};
use crate::ui::ACCENT;

const HEAD_H: f32 = 34.0;
const ROW_FULL: f32 = 52.0;
const ROW_FOLDED: f32 = 24.0;
const MORE_H: f32 = 22.0;

pub fn draw(ui: &mut Ui, app: &App, r: Rect) {
    let c = ito_theme::colors();
    let p = ui.painter().with_clip_rect(r);
    let head = Rect::from_min_size(r.min, vec2(r.width(), HEAD_H));
    p.text(pos2(head.left() + 14.0, head.center().y), Align2::LEFT_CENTER, app.tr("key.notes"), FontId::proportional(13.5), c.fg);
    let n = if app.ja() { format!("{} 件", app.notes.len()) } else { app.notes.len().to_string() };
    p.text(pos2(head.right() - 14.0, head.center().y), Align2::RIGHT_CENTER, n, FontId::monospace(11.5), c.dim);
    p.line_segment([head.left_bottom(), head.right_bottom()], Stroke::new(1.0, c.border));
    let body = Rect::from_min_max(head.left_bottom(), r.max).shrink2(vec2(8.0, 6.0));
    if app.notes.is_empty() {
        p.text(body.center(), Align2::CENTER_CENTER, app.tr("notes.empty"), FontId::proportional(13.0), c.dim);
        return;
    }
    let sel = app.ns.sel.min(app.notes.len() - 1);
    let f = fold(&vec![ROW_FULL; app.notes.len()], ROW_FOLDED, MORE_H, body.height(), sel);
    let now = now_epoch();
    let mut y = body.top();
    if f.above > 0 {
        more(&p, app, body, y, f.above, "list.above");
        y += MORE_H;
    }
    for (k, n) in app.notes.iter().enumerate() {
        let h = match f.rows[k] {
            Show::Hidden => continue,
            Show::Full => ROW_FULL,
            Show::Folded => ROW_FOLDED,
        };
        let row = Rect::from_min_size(pos2(body.left(), y), vec2(body.width(), h - 2.0));
        y += h;
        let clip = p.with_clip_rect(row);
        if k == sel {
            clip.rect_filled(row, CornerRadius::same(6), ACCENT.gamma_multiply(0.18));
        }
        let when = epoch(&n.updated_at).map(|t| age(&n.updated_at, now.max(t))).unwrap_or_default();
        let dot = pos2(row.left() + 12.0, row.center().y);
        clip.circle_filled(dot, 4.0, ACCENT);
        let x = row.left() + 26.0;
        if f.rows[k] == Show::Full {
            clip.text(pos2(x, row.top() + 16.0), Align2::LEFT_CENTER, &n.title, FontId::proportional(13.0), c.fg);
            let line = format!("{} · {} · {} · {}", n.repo, n.kind, n.reason.replace('_', " "), when);
            clip.text(pos2(x, row.top() + 36.0), Align2::LEFT_CENTER, line, FontId::proportional(11.5), c.dim);
        } else {
            clip.text(pos2(x, row.center().y), Align2::LEFT_CENTER, format!("{}  {}", n.repo, n.title), FontId::proportional(11.5), c.fg);
        }
    }
    if f.below > 0 {
        more(&p, app, body, body.bottom() - MORE_H, f.below, "list.below");
    }
}

fn more(p: &egui::Painter, app: &App, body: Rect, y: f32, n: usize, side: &'static str) {
    let c = ito_theme::colors();
    let words = if app.ja() { format!("⋯ {}{} 件", app.tr(side), n) } else { format!("⋯ {} {}", n, app.tr(side)) };
    p.text(pos2(body.left() + 8.0, y + MORE_H / 2.0), Align2::LEFT_CENTER, words, FontId::proportional(11.0), c.dim);
}
