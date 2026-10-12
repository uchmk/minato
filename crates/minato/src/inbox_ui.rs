//! The inbox view (docs/design.md, 「1 受信箱」): the cards in three groups at
//! the left with the filter chips below, the detail at the right with the
//! reason and the next step on top. Painted on rects; no scrollbar.

use egui::{Align2, CornerRadius, FontId, Rect, Stroke, Ui, pos2, vec2};
use minato_core::fold::{Show, fold};
use minato_core::inbox::Group;
use minato_core::list::{age, epoch};

use crate::app::{App, now_epoch};
use crate::list_ui::detail_col;
use crate::ui::ACCENT;

const LIST_W: f32 = 380.0;
const CARD_FULL: f32 = 66.0;
const CARD_FOLDED: f32 = 26.0;
const MORE_H: f32 = 22.0;
const HEAD_H: f32 = 30.0;
const GROUP_H: f32 = 22.0;
const CHIPS_H: f32 = 56.0;

pub fn draw(ui: &mut Ui, app: &App, r: Rect) {
    let c = ito_theme::colors();
    let list = Rect::from_min_size(r.min, vec2(LIST_W.min(r.width() * 0.5), r.height()));
    let detail = Rect::from_min_max(list.right_top(), r.max);
    left(ui, app, list);
    let all = app.inbox_all();
    let lead = app.inbox.selected(&all, &app.items).and_then(|card| {
        let rule = card.rule?;
        let (key, name) = rule.next();
        Some((format!("{}   [{}] {}", app.tr(rule.why()), key, app.tr(name)), ACCENT))
    });
    detail_col(ui, app, detail, app.inbox.scroll, lead);
    ui.painter().line_segment([pos2(list.right(), r.top()), pos2(list.right(), r.bottom())], Stroke::new(1.0, c.border));
}

fn left(ui: &mut Ui, app: &App, r: Rect) {
    let c = ito_theme::colors();
    let p = ui.painter().with_clip_rect(r);
    let all = app.inbox_all();
    let shown = app.inbox.shown(&all, &app.items);
    let mine = all.iter().filter(|c| c.group == Group::Mine).count();
    let head = Rect::from_min_size(r.min, vec2(r.width(), HEAD_H));
    let sel = app.inbox.sel.min(shown.len().saturating_sub(1));
    let title = format!("{} {} of {}", app.tr("view.inbox"), if shown.is_empty() { 0 } else { sel + 1 }, shown.len());
    p.text(pos2(head.left() + 12.0, head.center().y), Align2::LEFT_CENTER, title, FontId::proportional(13.0), c.fg);
    p.text(pos2(head.right() - 12.0, head.center().y), Align2::RIGHT_CENTER, format!("{} {}", app.tr("inbox.mine"), mine), FontId::proportional(11.5), c.dim);
    p.line_segment([head.left_bottom(), head.right_bottom()], Stroke::new(1.0, c.border));
    let chips = Rect::from_min_max(pos2(r.left(), r.bottom() - CHIPS_H), r.max);
    chips_row(ui, app, chips);
    let body = Rect::from_min_max(head.left_bottom(), chips.right_top());
    if shown.is_empty() {
        p.text(body.center(), Align2::CENTER_CENTER, app.tr(if app.viewer.is_none() { "list.loading" } else { "inbox.empty" }), FontId::proportional(13.0), c.dim);
        return;
    }
    // Room for the group headers (at most three) is kept out of the fold.
    let heights = vec![CARD_FULL; shown.len()];
    let f = fold(&heights, CARD_FOLDED, MORE_H, body.height() - GROUP_H * 3.0, sel);
    let now = now_epoch();
    let mut y = body.top();
    let mut last = None;
    if f.above > 0 {
        more(&p, app, body, y, f.above, "list.above");
        y += MORE_H;
    }
    for (k, card) in shown.iter().enumerate() {
        let h = match f.rows[k] {
            Show::Hidden => continue,
            Show::Full => CARD_FULL,
            Show::Folded => CARD_FOLDED,
        };
        if last != Some(card.group) {
            last = Some(card.group);
            let word = match card.group {
                Group::Mine => "inbox.mine",
                Group::Wait => "inbox.wait",
                Group::Done => "inbox.done",
            };
            p.text(pos2(body.left() + 12.0, y + GROUP_H / 2.0), Align2::LEFT_CENTER, app.tr(word), FontId::proportional(11.0), c.dim);
            y += GROUP_H;
        }
        let row = Rect::from_min_size(pos2(body.left(), y), vec2(body.width(), h));
        let item = &app.items[card.at];
        let tone = match card.group {
            Group::Mine => ACCENT,
            Group::Wait => c.dim,
            Group::Done => c.dim.gamma_multiply(0.5),
        };
        if k == sel {
            p.rect_filled(row, 0.0, ACCENT.gamma_multiply(0.14));
        }
        p.rect_filled(Rect::from_min_size(row.min, vec2(3.0, h)), 0.0, tone);
        let cp = p.with_clip_rect(row);
        let when = epoch(&item.updated_at).map(|t| age(&item.updated_at, now.max(t))).unwrap_or_default();
        let t = cp.text(pos2(row.right() - 10.0, row.top() + 15.0), Align2::RIGHT_CENTER, when, FontId::proportional(11.0), c.dim);
        let title_clip = Rect::from_min_max(row.min, pos2(t.left() - 6.0, row.bottom()));
        cp.with_clip_rect(title_clip).text(pos2(row.left() + 14.0, row.top() + 15.0), Align2::LEFT_CENTER, &item.title, FontId::proportional(13.0), c.fg);
        if f.rows[k] == Show::Full {
            cp.text(pos2(row.left() + 14.0, row.top() + 34.0), Align2::LEFT_CENTER, format!("{} #{}", item.repo, item.number), FontId::monospace(11.0), c.dim);
            if let Some(rule) = card.rule {
                cp.text(pos2(row.left() + 14.0, row.top() + 52.0), Align2::LEFT_CENTER, app.tr(rule.why()), FontId::proportional(11.5), tone);
            }
        }
        y += h;
    }
    if f.below > 0 {
        more(&p, app, body, y, f.below, "list.below");
    }
}

fn more(p: &egui::Painter, app: &App, body: Rect, y: f32, n: usize, side: &'static str) {
    let c = ito_theme::colors();
    let words = if app.ja() { format!("⋯ {}{} 件", app.tr(side), n) } else { format!("⋯ {} {}", n, app.tr(side)) };
    p.text(pos2(body.left() + 14.0, y + MORE_H / 2.0), Align2::LEFT_CENTER, words, FontId::proportional(11.5), c.dim);
}

/// The chips at the lower left: the status, then the repository and the tag.
fn chips_row(ui: &mut Ui, app: &App, r: Rect) {
    let c = ito_theme::colors();
    let p = ui.painter().clone();
    p.line_segment([r.left_top(), r.right_top()], Stroke::new(1.0, c.border));
    let all = app.tr("list.all");
    let chip = |x: f32, y: f32, text: String, on: bool| -> f32 {
        let color = if on { ACCENT } else { c.dim };
        let g = ui.fonts_mut(|f| f.layout_no_wrap(text, FontId::proportional(11.5), color));
        let b = Rect::from_min_size(pos2(x, y), vec2(g.size().x + 14.0, 20.0));
        p.rect_filled(b, CornerRadius::same(10), if on { ACCENT.gamma_multiply(0.18) } else { c.panel });
        p.galley(pos2(b.left() + 7.0, b.center().y - g.size().y / 2.0), g, color);
        b.right() + 6.0
    };
    chip(r.left() + 10.0, r.top() + 6.0, format!("f  {}", app.tr(app.inbox.status.word())), true);
    let x = chip(r.left() + 10.0, r.top() + 30.0, format!("F  {}", app.inbox.repo.as_deref().unwrap_or(all)), app.inbox.repo.is_some());
    chip(x, r.top() + 30.0, format!("t  {}", app.inbox.tag.as_deref().unwrap_or(all)), app.inbox.tag.is_some());
}
