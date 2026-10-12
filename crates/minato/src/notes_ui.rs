//! The notifications: hung from the bell over the view in tsumugi's frame
//! (the same place, width and highlight), each row as the mock draws it —
//! the repository, the number, the 受信箱 badge when the thread is a card
//! there, the age; what happened in its tone; the title. Folded by
//! `minato_core::fold` when they do not fit (no scrollbar).

use egui::{Align2, FontId, Rect, Sense, Stroke, Ui, pos2, vec2};
use minato_core::fold::{HEAD, Show, fold_head};
use minato_core::list::{age, epoch};
use minato_core::notes::{self, Note};

use crate::app::{App, NoteClick, Wheel, now_epoch};
use crate::style;
use crate::ui::Out;

/// tsumugi's list: 420 inside a margin of 10.
const WIDTH: f32 = 420.0;
const MARGIN: f32 = 10.0;
const ROW_FULL: f32 = 66.0;
const ROW_FOLDED: f32 = 34.0;
const MORE_H: f32 = 22.0;
/// What the window keeps under the list: the keybar and a little air.
const BELOW: f32 = 28.0 + 12.0;

pub fn show(ui: &Ui, app: &App, out: &mut Out) {
    let ctx = ui.ctx().clone();
    let c = ito_theme::colors();
    let Some(bell) = out.bell_at else {
        return;
    };
    let at = pos2((bell.right() - (WIDTH + MARGIN * 2.0)).max(8.0), bell.bottom() + 6.0);
    let room = ctx.content_rect().bottom() - BELOW - at.y - MARGIN * 2.0 - 40.0;
    let mut action = None;
    let area = egui::Area::new(egui::Id::new("bell-list")).order(egui::Order::Foreground).fixed_pos(at);
    let resp = area.show(&ctx, |ui| {
        egui::Frame::NONE.fill(c.panel).stroke(Stroke::new(1.0, c.border_strong())).corner_radius(10.0).inner_margin(MARGIN).show(ui, |ui| {
            ui.set_width(WIDTH);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(app.tr("notes.title")).strong().color(c.fg));
                let n = app.notes.len().to_string();
                ui.label(egui::RichText::new(app.tr("notes.unread").replace("{}", &n)).size(12.0).color(c.dim));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add_enabled(!app.notes.is_empty(), egui::Button::new(app.tr("notes.mark_read")).small()).clicked() {
                        action = Some(NoteClick::ReadAll);
                    }
                    ui.label(egui::RichText::new(app.tr("notes.hint")).size(11.5).color(c.dim));
                });
            });
            ui.separator();
            if app.notes.is_empty() {
                ui.label(egui::RichText::new(app.tr("notes.empty")).color(c.dim));
                return;
            }
            let sel = app.ns.sel.min(app.notes.len() - 1);
            let f = fold_head(&vec![ROW_FULL; app.notes.len()], ROW_FOLDED, MORE_H, room.max(ROW_FULL), sel, HEAD);
            let now = now_epoch();
            if f.above > 0 {
                more(ui, app, f.above, "list.above");
            }
            let last = app.notes.len() - 1 - f.below;
            for (k, n) in app.notes.iter().enumerate() {
                let full = match f.rows[k] {
                    Show::Hidden => continue,
                    Show::Full => true,
                    Show::Folded => false,
                };
                let (rect, resp) = ui.allocate_exact_size(vec2(WIDTH, if full { ROW_FULL } else { ROW_FOLDED }), Sense::click());
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                row(ui, app, n, rect, full, k == sel || resp.hovered(), k != last, now);
                if resp.clicked() {
                    action = Some(NoteClick::Open(k));
                }
            }
            if f.below > 0 {
                more(ui, app, f.below, "list.below");
            }
        });
    });
    crate::ui::wheel(&ctx, resp.response.layer_id, resp.response.rect, Wheel::Notes, out);
    // A click outside closes the list, but not the bell's own: it toggles.
    if resp.response.clicked_elsewhere() && action.is_none() && !out.bell {
        action = Some(NoteClick::Close);
    }
    out.note = action;
}

/// One notification: the unread dot; repository, number, badge and age;
/// what happened; the title (left out when folded).
#[allow(clippy::too_many_arguments)]
fn row(ui: &Ui, app: &App, n: &Note, r: Rect, full: bool, cur: bool, line: bool, now: i64) {
    let c = ito_theme::colors();
    let p = ui.painter_at(r);
    if cur {
        p.rect_filled(r, 6.0, c.selection().gamma_multiply(0.6));
    }
    if line {
        p.line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, c.border));
    }
    let (_, card) = app.note_place(&n.url);
    let left = r.left() + 6.0;
    let y1 = r.top() + 8.0 + 9.0;
    style::dot(&p, pos2(left + 3.5, y1), 3.5, style::ACCENT);
    let x = left + 7.0 + 10.0;
    let right = r.right() - 6.0;
    // The first line, from the right: the age, then the rest.
    let when = epoch(&n.updated_at).map(|t| age(&n.updated_at, now.max(t))).unwrap_or_default();
    let t = p.text(pos2(right, y1), Align2::RIGHT_CENTER, when, FontId::monospace(11.5), c.dim);
    let mut lx = style::tag(&p, x, y1, &n.repo, n.repo.rsplit('/').next().unwrap_or(&n.repo)) + 7.0;
    if let Some(num) = notes::number(&n.url) {
        lx = p.text(pos2(lx, y1), Align2::LEFT_CENTER, format!("#{num}"), FontId::monospace(12.0), c.dim).right() + 7.0;
    }
    let inbox = app.tr("notes.inbox");
    if card.is_some() && lx + style::badge_w(&p, inbox) < t.left() - 6.0 {
        lx = style::badge(&p, lx, y1, inbox) + 7.0;
    }
    // What happened: the inbox's reason when it is a card there, else why
    // GitHub sent it.
    let (what, tone) = match card.and_then(|c| c.rule) {
        Some(rule) => (app.tr(rule.why()), style::tone(rule.tone())),
        None => {
            let (key, tone) = notes::event(&n.reason);
            (app.tr(key), style::tone(tone))
        }
    };
    if !full {
        // Folded, the title still says which thread it is.
        let d = p.text(pos2(lx, y1), Align2::LEFT_CENTER, "●", FontId::proportional(10.0), tone);
        let g = style::truncated(&p, n.title.clone(), FontId::proportional(12.5), c.fg, t.left() - 8.0 - d.right() - 6.0);
        p.galley(pos2(d.right() + 6.0, y1 - g.size().y / 2.0), g, c.fg);
        return;
    }
    let y2 = y1 + 20.0;
    let d = p.text(pos2(x, y2), Align2::LEFT_CENTER, "●", FontId::proportional(10.0), tone);
    let g = style::truncated(&p, what.to_owned(), FontId::proportional(13.0), c.fg, right - d.right() - 6.0);
    p.galley(pos2(d.right() + 6.0, y2 - g.size().y / 2.0), g, c.fg);
    let y3 = y2 + 19.0;
    let g = style::truncated(&p, n.title.clone(), FontId::proportional(12.0), c.dim, right - x);
    p.galley(pos2(x, y3 - g.size().y / 2.0), g, c.dim);
}

fn more(ui: &mut Ui, app: &App, n: usize, side: &'static str) {
    let (r, _) = ui.allocate_exact_size(vec2(WIDTH, MORE_H), Sense::hover());
    style::more(ui.painter(), app.ja(), app.tr(side), r.left() + 8.0, r.center().y, n);
}
