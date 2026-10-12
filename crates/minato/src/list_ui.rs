//! The list view (docs/design.md, 「2 一覧」): the filter column, the 3-line
//! rows folded by `minato_core::fold`, and the detail. Painted on rects; no
//! scrollbar anywhere.

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Stroke, Ui, pos2, vec2};
use minato_core::fold::{Show, fold};
use minato_core::list::{Ci, Col, Item, Kind, Pick, Saved, age, epoch, options};

use crate::app::{App, now_epoch};
use crate::ui::ACCENT;

const FILTER_W: f32 = 210.0;
const ROW_FULL: f32 = 64.0;
const ROW_FOLDED: f32 = 24.0;
const MORE_H: f32 = 22.0;
const HEAD_H: f32 = 30.0;
pub const LINE: f32 = 18.0;

pub const OK: Color32 = Color32::from_rgb(0x3F, 0xA4, 0x6A);
pub const BAD: Color32 = Color32::from_rgb(0xD1, 0x49, 0x5B);
pub const PENDING: Color32 = Color32::from_rgb(0xE0, 0xA0, 0x30);
const MERGED: Color32 = Color32::from_rgb(0x89, 0x57, 0xE5);

fn ci_color(ci: Ci) -> Color32 {
    match ci {
        Ci::Ok => OK,
        Ci::Bad => BAD,
        Ci::Pending => PENDING,
    }
}

fn ci_mark(ci: Ci) -> &'static str {
    match ci {
        Ci::Ok => "✓",
        Ci::Bad => "✗",
        Ci::Pending => "◐",
    }
}

fn kind_word(app: &App, k: Kind) -> &'static str {
    app.tr(match k {
        Kind::Issue => "kind.issue",
        Kind::Pr => "kind.pr",
        Kind::Discussion => "kind.discussion",
    })
}

fn pick_label(app: &App, p: &Pick) -> String {
    match p {
        Pick::Repo(r) => r.clone().unwrap_or_else(|| app.tr("list.all").into()),
        Pick::Kind(k) => k.map_or_else(|| app.tr("list.all").into(), |k| kind_word(app, k).into()),
        Pick::Author(a) => a.clone().unwrap_or_else(|| app.tr("list.all").into()),
        Pick::Saved(Saved::None) => app.tr("list.all").into(),
        Pick::Saved(Saved::Assigned) => app.tr("saved.assigned").into(),
        Pick::Saved(Saved::Red) => app.tr("saved.red").into(),
    }
}

pub fn draw(ui: &mut Ui, app: &App, r: Rect) {
    let c = ito_theme::colors();
    let detail_w = ((r.width() - FILTER_W) * 0.42).max(260.0);
    let filters = Rect::from_min_size(r.min, vec2(FILTER_W, r.height()));
    let detail = Rect::from_min_max(pos2(r.right() - detail_w, r.top()), r.max);
    let items = Rect::from_min_max(filters.right_top(), detail.left_bottom());
    filter_col(ui, app, filters);
    items_col(ui, app, items);
    detail_col(ui, app, detail, app.list.scroll, None);
    let p = ui.painter();
    for x in [filters.right(), detail.left()] {
        p.line_segment([pos2(x, r.top()), pos2(x, r.bottom())], Stroke::new(1.0, c.border));
    }
    // The focused column gets an accent line at its top.
    let on = match app.list.col {
        Col::Filters => filters,
        Col::Items => items,
        Col::Detail => detail,
    };
    p.line_segment([on.left_top(), on.right_top()], Stroke::new(2.0, ACCENT));
}

fn filter_col(ui: &mut Ui, app: &App, r: Rect) {
    let c = ito_theme::colors();
    let p = ui.painter().with_clip_rect(r);
    let groups = options(&app.items);
    let focus = app.list.col == Col::Filters;
    let mut y = r.top() + 10.0;
    let mut flat = 0;
    // The cursor's row stays in view: start high enough that it fits.
    let cursor_y: f32 = {
        let mut y = 10.0;
        let mut k = 0;
        let mut found = 10.0;
        for (_, picks) in &groups {
            y += LINE + 6.0;
            for _ in picks {
                if k == app.list.cursor {
                    found = y;
                }
                y += LINE;
                k += 1;
            }
            y += 6.0;
        }
        found
    };
    y -= (cursor_y + LINE - r.height()).max(0.0);
    for (name, picks) in &groups {
        p.text(pos2(r.left() + 12.0, y + 9.0), Align2::LEFT_CENTER, app.tr(match *name {
            "repo" => "list.repo",
            "kind" => "list.kind",
            "author" => "list.author",
            _ => "list.saved",
        }), FontId::proportional(11.0), c.dim);
        y += LINE + 6.0;
        for pick in picks {
            let row = Rect::from_min_size(pos2(r.left() + 4.0, y), vec2(r.width() - 8.0, LINE));
            let cur = focus && flat == app.list.cursor;
            if cur {
                p.rect_filled(row, CornerRadius::same(4), ACCENT.gamma_multiply(0.18));
            }
            let chosen = app.list.is_chosen(pick);
            let color = if chosen { ACCENT } else { c.fg };
            p.text(pos2(row.left() + 8.0, row.center().y), Align2::LEFT_CENTER, if chosen { "●" } else { "○" }, FontId::proportional(10.0), color);
            p.text(pos2(row.left() + 24.0, row.center().y), Align2::LEFT_CENTER, pick_label(app, pick), FontId::proportional(12.5), color);
            y += LINE;
            flat += 1;
        }
        y += 6.0;
    }
}

fn items_col(ui: &mut Ui, app: &App, r: Rect) {
    let c = ito_theme::colors();
    let me = app.viewer.as_deref().unwrap_or("");
    let shown = app.list.shown(&app.items, me);
    let head = Rect::from_min_size(r.min, vec2(r.width(), HEAD_H));
    let p = ui.painter().with_clip_rect(r);
    p.text(pos2(head.left() + 12.0, head.center().y), Align2::LEFT_CENTER, app.list.filter.query(me), FontId::monospace(11.5), c.dim);
    p.text(pos2(head.right() - 12.0, head.center().y), Align2::RIGHT_CENTER, shown.len().to_string(), FontId::monospace(11.5), c.fg);
    p.line_segment([head.left_bottom(), head.right_bottom()], Stroke::new(1.0, c.border));
    let body = Rect::from_min_max(head.left_bottom(), r.max);
    if shown.is_empty() {
        let words = if app.viewer.is_none() || app.items.is_empty() { "list.loading" } else { "list.none" };
        p.text(body.center(), Align2::CENTER_CENTER, app.tr(words), FontId::proportional(13.0), c.dim);
        return;
    }
    let heights = vec![ROW_FULL; shown.len()];
    let sel = app.list.sel.min(shown.len() - 1);
    let f = fold(&heights, ROW_FOLDED, MORE_H, body.height(), sel);
    let now = now_epoch();
    let focus = app.list.col == Col::Items;
    let mut y = body.top();
    if f.above > 0 {
        more_line(&p, app, body, y, f.above, "list.above");
        y += MORE_H;
    }
    for (k, item) in shown.iter().enumerate() {
        let h = match f.rows[k] {
            Show::Hidden => continue,
            Show::Full => ROW_FULL,
            Show::Folded => ROW_FOLDED,
        };
        let row = Rect::from_min_size(pos2(body.left(), y), vec2(body.width(), h));
        item_row(&p, app, item, row, f.rows[k] == Show::Full, k == sel, focus, now);
        y += h;
    }
    if f.below > 0 {
        more_line(&p, app, body, y, f.below, "list.below");
    }
}

fn more_line(p: &egui::Painter, app: &App, body: Rect, y: f32, n: usize, side: &'static str) {
    let c = ito_theme::colors();
    let words = if app.ja() { format!("⋯ {}{} 件", app.tr(side), n) } else { format!("⋯ {} {}", n, app.tr(side)) };
    p.text(pos2(body.left() + 14.0, y + MORE_H / 2.0), Align2::LEFT_CENTER, words, FontId::proportional(11.5), c.dim);
}

#[allow(clippy::too_many_arguments)]
fn item_row(p: &egui::Painter, app: &App, item: &Item, row: Rect, full: bool, selected: bool, focus: bool, now: i64) {
    let c = ito_theme::colors();
    if selected {
        p.rect_filled(row, 0.0, ACCENT.gamma_multiply(if focus { 0.16 } else { 0.08 }));
        p.rect_filled(Rect::from_min_size(row.min, vec2(3.0, row.height())), 0.0, ACCENT);
    }
    let p = p.with_clip_rect(row);
    let x = row.left() + 14.0;
    let right = row.right() - 12.0;
    // The marks at the right: the CI and the comments.
    let mut rx = right;
    if item.comments > 0 {
        let t = p.text(pos2(rx, row.top() + 16.0), Align2::RIGHT_CENTER, format!("💬 {}", item.comments), FontId::proportional(11.5), c.dim);
        rx = t.left() - 10.0;
    }
    if let Some(ci) = item.ci {
        let t = p.text(pos2(rx, row.top() + 16.0), Align2::RIGHT_CENTER, ci_mark(ci), FontId::proportional(13.0), ci_color(ci));
        rx = t.left() - 10.0;
    }
    let clip = Rect::from_min_max(row.min, pos2(rx, row.bottom()));
    let title_color = if item.merged { MERGED } else { c.fg };
    p.with_clip_rect(clip).text(pos2(x, row.top() + 16.0), Align2::LEFT_CENTER, &item.title, FontId::proportional(13.5), title_color);
    if !full {
        return;
    }
    // Line 2: the labels.
    let mut lx = x;
    let ly = row.top() + 33.0;
    for l in item.labels.iter().take(4) {
        let col = parse_hex(&l.color).unwrap_or(c.dim);
        let g = p.layout_no_wrap(l.name.clone(), FontId::proportional(10.5), col);
        let chip = Rect::from_min_size(pos2(lx, ly - 8.0), vec2(g.size().x + 12.0, 16.0));
        if chip.right() > right {
            break;
        }
        p.rect_filled(chip, CornerRadius::same(8), col.gamma_multiply(0.18));
        p.galley(pos2(chip.left() + 6.0, ly - g.size().y / 2.0), g, col);
        lx = chip.right() + 5.0;
    }
    // Line 3: where and who.
    let when = epoch(&item.created_at).map(|t| age(&item.created_at, now.max(t))).unwrap_or_default();
    let line = format!("{} {} #{} · {} · {}", kind_word(app, item.kind), item.repo, item.number, item.author, when);
    p.text(pos2(x, row.top() + 50.0), Align2::LEFT_CENTER, line, FontId::proportional(11.5), c.dim);
}

/// The detail column. `lead` is a line over the title (the inbox reason
/// and next step).
pub fn detail_col(ui: &mut Ui, app: &App, r: Rect, scroll: f32, lead: Option<(String, Color32)>) {
    let c = ito_theme::colors();
    let p = ui.painter().with_clip_rect(r);
    let Some(item) = app.selected() else {
        p.text(r.center(), Align2::CENTER_CENTER, app.tr("detail.pick"), FontId::proportional(13.0), c.dim);
        return;
    };
    let w = r.width() - 28.0;
    let mut y = r.top() + 14.0 - scroll * LINE;
    let x = r.left() + 14.0;
    if let Some((text, color)) = lead {
        let g = ui.fonts_mut(|f| f.layout(text, FontId::proportional(12.5), color, w));
        let h = g.size().y;
        p.galley(pos2(x, y), g, color);
        y += h + 10.0;
    }
    // Title and state.
    let title = ui.fonts_mut(|f| f.layout(item.title.clone(), FontId::proportional(16.0), c.fg, w));
    let h = title.size().y;
    p.galley(pos2(x, y), title, c.fg);
    y += h + 6.0;
    let state = format!("{} {} #{}{}", kind_word(app, item.kind), item.repo, item.number, if item.draft { " · draft" } else if item.merged { " · merged" } else { "" });
    p.text(pos2(x, y + 8.0), Align2::LEFT_CENTER, state, FontId::proportional(11.5), c.dim);
    y += LINE + 8.0;
    let Some((_, d)) = app.detail.as_ref().filter(|(id, _)| *id == item.id) else {
        p.text(pos2(x, y + 8.0), Align2::LEFT_CENTER, app.tr("detail.fetching"), FontId::proportional(12.0), c.dim);
        return;
    };
    let block = |y: &mut f32, text: &str, font: FontId, color: Color32| {
        let g = ui.fonts_mut(|f| f.layout(text.to_owned(), font, color, w));
        let h = g.size().y;
        p.galley(Pos2::new(x, *y), g, color);
        *y += h;
    };
    if !d.body.is_empty() {
        block(&mut y, &d.body, FontId::proportional(12.5), c.fg);
        y += 12.0;
    }
    if !d.checks.is_empty() {
        block(&mut y, app.tr("detail.checks"), FontId::proportional(11.0), c.dim);
        y += 4.0;
        for ch in &d.checks {
            block(&mut y, &format!("{} {}", ci_mark(ch.ci), ch.name), FontId::proportional(12.5), ci_color(ch.ci));
            y += 2.0;
        }
        y += 10.0;
    }
    if !d.timeline.is_empty() {
        block(&mut y, app.tr("detail.talk"), FontId::proportional(11.0), c.dim);
        y += 4.0;
        let now = now_epoch();
        for e in &d.timeline {
            let when = epoch(&e.created_at).map(|t| age(&e.created_at, now.max(t))).unwrap_or_default();
            block(&mut y, &format!("{} · {}", e.author, when), FontId::proportional(11.5), ACCENT);
            block(&mut y, &e.body, FontId::proportional(12.5), c.fg);
            y += 10.0;
        }
    }
}

pub fn parse_hex(s: &str) -> Option<Color32> {
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}
