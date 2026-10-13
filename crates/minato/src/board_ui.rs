//! The board (docs/design.md, 「4 ボード」): the Status columns of Project
//! 「uchmk」 with their cards, folded by `minato_core::fold` (no scrollbar).
//! A card moves with `H` `L` or by dragging it onto another column.

use egui::{Align2, CornerRadius, FontId, Id, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use minato_core::fold::{Show, fold};
use minato_core::list::{age, epoch};

use crate::app::{App, Wheel, now_epoch};
use crate::list_ui::parse_hex;
use crate::ui::{ACCENT, Out};

const HEAD_H: f32 = 34.0;
const COL_HEAD_H: f32 = 30.0;
const CARD_FULL: f32 = 74.0;
const MORE_H: f32 = 22.0;
const GAP: f32 = 8.0;
const MIN_COL_W: f32 = 190.0;

pub fn draw(ui: &mut Ui, app: &App, r: Rect, out: &mut Out) {
    let c = ito_theme::colors();
    let p = ui.painter().with_clip_rect(r);
    let Some(board) = app.board.as_ref() else {
        let words = if app.viewer.is_none() { "list.loading" } else { "board.loading" };
        p.text(r.center(), Align2::CENTER_CENTER, app.tr(words), FontId::proportional(13.0), c.dim);
        return;
    };
    let head = Rect::from_min_size(r.min, vec2(r.width(), HEAD_H));
    let title = if app.ja() { format!("Project「{}」", board.title) } else { format!("Project “{}”", board.title) };
    p.text(pos2(head.left() + 14.0, head.center().y), Align2::LEFT_CENTER, title, FontId::proportional(13.5), c.fg);
    let n = if app.ja() { format!("{} 件", board.cards.len()) } else { board.cards.len().to_string() };
    let n_r = p.text(pos2(head.right() - 14.0, head.center().y), Align2::RIGHT_CENTER, n, FontId::monospace(11.5), c.dim);
    p.line_segment([head.left_bottom(), head.right_bottom()], Stroke::new(1.0, c.border));
    let area = Rect::from_min_max(head.left_bottom(), r.max).shrink2(vec2(GAP, GAP));
    let cols = board.columns();
    if board.cards.is_empty() {
        p.text(area.center(), Align2::CENTER_CENTER, app.tr("board.empty"), FontId::proportional(13.0), c.dim);
    }
    // Columns share the width; with too many, the ones around the cursor show.
    let fit = (((area.width() + GAP) / (MIN_COL_W + GAP)) as usize).clamp(1, cols.len().max(1));
    let first = app.bs.col.saturating_sub(fit - 1).min(cols.len().saturating_sub(fit));
    // Which of them, and an arrow lit on a side with more beyond it.
    if fit < cols.len() {
        let last = first + fit;
        let span = if app.ja() { format!("列 {}–{} / {}", first + 1, last, cols.len()) } else { format!("columns {}–{} of {}", first + 1, last, cols.len()) };
        let cy = head.center().y;
        let ink = |more: bool| if more { ACCENT } else { c.border };
        let right = p.text(pos2(n_r.left() - 16.0, cy), Align2::RIGHT_CENTER, "›", FontId::proportional(15.0), ink(last < cols.len()));
        let mid = p.text(pos2(right.left() - 6.0, cy), Align2::RIGHT_CENTER, span, FontId::proportional(12.0), c.dim);
        p.text(pos2(mid.left() - 6.0, cy), Align2::RIGHT_CENTER, "‹", FontId::proportional(15.0), ink(first > 0));
    }
    let w = (area.width() - GAP * (fit as f32 - 1.0)) / fit as f32;
    let rects: Vec<(usize, Rect)> = (first..(first + fit).min(cols.len()))
        .enumerate()
        .map(|(k, col)| (col, Rect::from_min_size(pos2(area.left() + k as f32 * (w + GAP), area.top()), vec2(w, area.height()))))
        .collect();
    let now = now_epoch();
    let mut dragged: Option<usize> = None;
    for &(col, rect) in &rects {
        let focus = col == app.bs.col;
        crate::ui::wheel(ui.ctx(), ui.layer_id(), rect, Wheel::Board(col), out);
        p.rect_filled(rect, CornerRadius::same(8), c.panel);
        if focus {
            p.rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.0, ACCENT.gamma_multiply(0.6)), StrokeKind::Inside);
        }
        let ids = board.in_column(col);
        let name = match &cols[col].option {
            Some(_) => cols[col].name.clone(),
            None => app.tr("board.nostatus").to_string(),
        };
        let chead = Rect::from_min_size(rect.min, vec2(rect.width(), COL_HEAD_H));
        p.text(pos2(chead.left() + 12.0, chead.center().y), Align2::LEFT_CENTER, name, FontId::proportional(12.5), if focus { ACCENT } else { c.fg });
        p.text(pos2(chead.right() - 12.0, chead.center().y), Align2::RIGHT_CENTER, ids.len().to_string(), FontId::monospace(11.0), c.dim);
        let body = Rect::from_min_max(chead.left_bottom(), rect.max).shrink2(vec2(6.0, 4.0));
        if ids.is_empty() {
            continue;
        }
        let sel = if focus { app.bs.row.min(ids.len() - 1) } else { 0 };
        let f = fold(&vec![CARD_FULL; ids.len()], MORE_H, body.height(), sel);
        let mut y = body.top();
        if f.above > 0 {
            more(&p, app, body, y, f.above, "list.above");
            y += MORE_H;
        }
        for (k, &ci) in ids.iter().enumerate() {
            if f.rows[k] == Show::Hidden {
                continue;
            }
            let cr = Rect::from_min_size(pos2(body.left(), y), vec2(body.width(), CARD_FULL - 4.0));
            y += CARD_FULL;
            let card = &board.cards[ci];
            let resp = ui.interact(cr, Id::new(("board-card", &card.item)), Sense::click_and_drag());
            if resp.clicked() || resp.drag_started() {
                out.pick = Some(ci);
            }
            if resp.dragged() {
                dragged = Some(ci);
            }
            if resp.drag_stopped() {
                let at = ui.ctx().pointer_latest_pos().unwrap_or(cr.center());
                if let Some(&(to, _)) = rects.iter().find(|(_, rc)| rc.x_range().contains(at.x)) {
                    out.drop = Some((ci, to));
                }
            }
            // The body's start: the list's, or asked for once the mouse is here.
            if dragged.is_none() {
                let known = app.items.iter().find(|i| i.id == card.id).map(|i| i.peek.as_str()).filter(|t| !t.is_empty()).or_else(|| app.peeks.get(&card.id).map(String::as_str));
                match known {
                    Some(t) => crate::ui::peek(&resp, t),
                    None if resp.hovered() => out.peek = Some(card.id.clone()),
                    None => {}
                }
            }
            let picked = focus && k == app.bs.row;
            card_box(&p, app, cr, ci, picked, now);
        }
        if f.below > 0 {
            more(&p, app, body, body.bottom() - MORE_H, f.below, "list.below");
        }
    }
    // The card in the hand follows the pointer.
    if let (Some(ci), Some(at)) = (dragged, ui.ctx().pointer_latest_pos()) {
        let ghost = Rect::from_center_size(at, vec2(w.min(260.0), CARD_FULL - 4.0));
        let top = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::new("board-ghost")));
        card_box(&top, app, ghost, ci, true, now);
        for &(_, rc) in &rects {
            if rc.x_range().contains(at.x) {
                top.rect_stroke(rc, CornerRadius::same(8), Stroke::new(2.0, ACCENT), StrokeKind::Inside);
            }
        }
    }
}

fn card_box(p: &egui::Painter, app: &App, r: Rect, ci: usize, picked: bool, now: i64) {
    let c = ito_theme::colors();
    let Some(card) = app.board.as_ref().and_then(|b| b.cards.get(ci)) else { return };
    let p = p.with_clip_rect(r);
    p.rect_filled(r, CornerRadius::same(6), if picked { ACCENT.gamma_multiply(0.18) } else { c.bg });
    p.rect_stroke(r, CornerRadius::same(6), Stroke::new(1.0, if picked { ACCENT } else { c.border }), StrokeKind::Inside);
    let x = r.left() + 10.0;
    let kind = if card.pr { "PR" } else { "Issue" };
    let repo = card.repo.rsplit('/').next().unwrap_or(&card.repo);
    let mut line1 = format!("{repo} #{} · {kind}", card.number);
    if let Some(item) = app.items.iter().find(|i| i.id == card.id) {
        if let Some(t) = epoch(&item.created_at) {
            line1.push_str(&format!(" · {}", age(&item.created_at, now.max(t))));
        }
    }
    p.text(pos2(x, r.top() + 12.0), Align2::LEFT_CENTER, line1, FontId::proportional(10.5), c.dim);
    let g = crate::style::truncated(&p, card.title.clone(), FontId::proportional(12.5), c.fg, r.right() - 8.0 - x);
    p.galley(pos2(x, r.top() + 31.0 - g.size().y / 2.0), g, c.fg);
    let mut lx = x;
    let ly = r.top() + 53.0;
    for l in card.labels.iter().take(3) {
        let col = parse_hex(&l.color).unwrap_or(c.dim);
        let g = p.layout_no_wrap(l.name.clone(), FontId::proportional(10.0), col);
        let chip = Rect::from_min_size(pos2(lx, ly - 8.0), vec2(g.size().x + 12.0, 16.0));
        if chip.right() > r.right() - 6.0 {
            break;
        }
        p.rect_filled(chip, CornerRadius::same(8), col.gamma_multiply(0.18));
        p.galley(pos2(chip.left() + 6.0, ly - g.size().y / 2.0), g, col);
        lx = chip.right() + 4.0;
    }
}

fn more(p: &egui::Painter, app: &App, body: Rect, y: f32, n: usize, side: &'static str) {
    let c = ito_theme::colors();
    let words = if app.ja() { format!("⋯ {}{} 件", app.tr(side), n) } else { format!("⋯ {} {}", n, app.tr(side)) };
    p.text(pos2(body.left() + 8.0, y + MORE_H / 2.0), Align2::LEFT_CENTER, words, FontId::proportional(11.0), c.dim);
}
