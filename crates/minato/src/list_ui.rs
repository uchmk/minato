//! The list view (docs/design.md, 「2 一覧」): the filter column, the 3-line
//! rows folded by `minato_core::fold`, and the detail. Painted on rects; no
//! scrollbar anywhere.

use egui::{Align2, Color32, CornerRadius, FontId, Painter, Rect, Stroke, StrokeKind, Ui, pos2, vec2};
use minato_core::fold::{Show, fold};
use minato_core::inbox::Group;
use minato_core::list::{Ci, Col, Item, Kind, Pick, Saved, age, epoch, options};

use crate::app::{App, Click, Pane, Wheel, now_epoch};
use crate::style::{self, ACCENT};
use crate::ui::{self, Out};

/// A row with labels, and one without.
const ROW_FULL: f32 = 76.0;
const ROW_PLAIN: f32 = 54.0;
const MORE_H: f32 = 22.0;
/// The bar with the query.
const HEAD_H: f32 = 40.0;
/// A filter group's heading, and a filter's row.
const GROUP_H: f32 = 30.0;
const ENT_H: f32 = 24.0;
pub const LINE: f32 = 18.0;

const MERGED: Color32 = Color32::from_rgb(0x89, 0x57, 0xE5);

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
        Pick::Saved(Saved::Mentioned) => app.tr("saved.mentioned").into(),
        Pick::Saved(Saved::Finding) => app.tr("saved.finding").into(),
        Pick::Search(i) => app.list.searches.get(*i).cloned().unwrap_or_default(),
    }
}

pub fn draw(ui: &mut Ui, app: &App, r: Rect, out: &mut Out) {
    for (pane, rect) in ui::panes(&app.list_split, r) {
        let col = match pane {
            Pane::Left => Col::Filters,
            Pane::Mid => Col::Items,
            Pane::Right => Col::Detail,
        };
        if ui::hit(ui, rect, ("list-col", pane)).clicked() {
            out.click = Some(Click::ListCol(col));
        }
        let at = if col == Col::Detail { Wheel::Detail } else { Wheel::List(col) };
        ui::wheel(ui.ctx(), ui.layer_id(), rect, at, out);
        match pane {
            Pane::Left => filter_col(ui, app, rect, out),
            Pane::Mid => items_col(ui, app, rect, out),
            Pane::Right => crate::detail_ui::draw(ui, app, rect, app.list.scroll, out),
        }
        // The focused column gets an accent line at its top.
        if app.list.col == col {
            ui.painter().line_segment([rect.left_top(), rect.right_top()], Stroke::new(2.0, ACCENT));
        }
    }
    ui::dividers(ui, &app.list_split, r, out);
}

/// The filters: a heading per group, a row per choice with its ● when
/// chosen and, for the repositories, how many items each has.
fn filter_col(ui: &Ui, app: &App, r: Rect, out: &mut Out) {
    let c = ito_theme::colors();
    let p = ui.painter().with_clip_rect(r);
    p.rect_filled(r, 0.0, c.side);
    let groups = options(&app.items, &app.list.searches);
    let focus = app.list.col == Col::Filters;
    // The cursor's row stays in view: start high enough that it fits.
    let mut cursor_y = 0.0;
    let mut yy = 6.0;
    let mut k = 0;
    for (_, picks) in &groups {
        yy += GROUP_H;
        for _ in picks {
            if k == app.list.cursor {
                cursor_y = yy;
            }
            yy += ENT_H;
            k += 1;
        }
    }
    let mut y = r.top() + 6.0 - (cursor_y + ENT_H + 16.0 - r.height()).max(0.0);
    let mut flat = 0;
    for (name, picks) in &groups {
        let word = app.tr(match *name {
            "repo" => "list.repo",
            "kind" => "list.kind",
            "author" => "list.author",
            _ => "list.saved",
        });
        style::bold(&p, pos2(r.left() + 14.0, y + 12.0 + 7.0), Align2::LEFT_CENTER, word, FontId::proportional(11.0), c.dim);
        y += GROUP_H;
        for pick in picks {
            let row = Rect::from_min_size(pos2(r.left(), y), vec2(r.width(), ENT_H));
            let resp = ui::hit(ui, row, ("list-filter", flat));
            if resp.clicked() {
                out.click = Some(Click::ListFilter(flat));
            }
            if focus && flat == app.list.cursor {
                p.rect_filled(row, 0.0, style::sel());
                p.rect_filled(Rect::from_min_size(row.min, vec2(3.0, ENT_H)), 0.0, ACCENT);
            } else if resp.hovered() {
                p.rect_filled(row, 0.0, c.hover());
            }
            let cy = row.center().y;
            if app.list.is_chosen(pick) {
                p.text(pos2(row.left() + 14.0, cy), Align2::LEFT_CENTER, "●", FontId::proportional(9.0), ACCENT);
            }
            p.text(pos2(row.left() + 14.0 + 18.0, cy), Align2::LEFT_CENTER, pick_label(app, pick), FontId::proportional(13.0), c.fg);
            if let Pick::Repo(repo) = pick {
                let n = app.items.iter().filter(|i| !minato_core::mainci::is_stub(i) && repo.as_ref().is_none_or(|r| *r == i.repo)).count();
                p.text(pos2(row.right() - 14.0, cy), Align2::RIGHT_CENTER, n.to_string(), FontId::monospace(11.5), c.dim);
            }
            y += ENT_H;
            flat += 1;
        }
    }
}

/// The query and the count over the items, each folded to fit.
fn items_col(ui: &Ui, app: &App, r: Rect, out: &mut Out) {
    let c = ito_theme::colors();
    let me = app.viewer.as_deref().unwrap_or("");
    let shown = app.list.shown(&app.items, me);
    let p = ui.painter().with_clip_rect(r);
    p.rect_filled(r, 0.0, c.side);
    // The bar: the query in a box, then how many.
    let head = Rect::from_min_size(r.min, vec2(r.width(), HEAD_H));
    let n = p.text(pos2(head.right() - 12.0, head.center().y), Align2::RIGHT_CENTER, app.tr("list.count").replace("{}", &shown.len().to_string()), FontId::proportional(12.0), c.dim);
    let q = Rect::from_min_max(pos2(head.left() + 12.0, head.center().y - 11.0), pos2(n.left() - 8.0, head.center().y + 11.0));
    p.rect_filled(q, CornerRadius::same(6), c.bg);
    p.rect_stroke(q, CornerRadius::same(6), Stroke::new(1.0, c.border), StrokeKind::Inside);
    let owner = app.owner().unwrap_or("");
    let query = app.list.active_search().map_or_else(|| app.list.filter.query(owner), |t| minato_core::list::search_query(t, owner));
    let g = style::truncated(&p, query, FontId::monospace(12.0), c.fg, q.width() - 18.0);
    p.galley(pos2(q.left() + 9.0, q.center().y - g.size().y / 2.0), g, c.fg);
    p.line_segment([head.left_bottom(), head.right_bottom()], Stroke::new(1.0, c.border));
    let body = Rect::from_min_max(head.left_bottom(), r.max);
    if shown.is_empty() {
        let words = if app.viewer.is_none() || app.items.is_empty() { "list.loading" } else { "list.none" };
        p.text(pos2(body.left() + 14.0, body.top() + 20.0), Align2::LEFT_CENTER, app.tr(words), FontId::proportional(12.5), c.dim);
        return;
    }
    let heights: Vec<f32> = shown.iter().map(|i| if i.labels.is_empty() { ROW_PLAIN } else { ROW_FULL }).collect();
    let sel = app.list.sel.min(shown.len() - 1);
    let f = fold(&heights, MORE_H, body.height(), sel);
    let now = now_epoch();
    let all = app.inbox_all();
    let marked: Vec<&str> = all.iter().filter(|card| card.group != Group::Done).map(|card| app.items[card.at].id.as_str()).collect();
    let mut y = body.top();
    if f.above > 0 {
        more(&p, app, body, y, f.above, "list.above");
        y += MORE_H;
    }
    for (k, item) in shown.iter().enumerate() {
        if f.rows[k] == Show::Hidden {
            continue;
        }
        let h = heights[k];
        let row = Rect::from_min_size(pos2(body.left(), y), vec2(body.width(), h));
        let resp = ui::hit(ui, row, ("list-item", k));
        if resp.clicked() {
            out.click = Some(Click::ListItem(k));
        }
        item_row(&p, app, item, row, k == sel, resp.hovered(), marked.contains(&item.id.as_str()), now);
        ui::peek(&resp, &item.peek);
        y += h;
    }
    if f.below > 0 {
        more(&p, app, body, y, f.below, "list.below");
    }
}

fn more(p: &Painter, app: &App, body: Rect, y: f32, n: usize, side: &'static str) {
    style::more(p, app.ja(), app.tr(side), body.left() + 14.0, y + MORE_H / 2.0, n);
}

/// One item: its kind's mark; the title, the labels, and where and who;
/// at the right the CI, the comments and the 受信箱 badge.
#[allow(clippy::too_many_arguments)]
fn item_row(p: &Painter, app: &App, item: &Item, row: Rect, on: bool, hover: bool, inbox: bool, now: i64) {
    let c = ito_theme::colors();
    if on {
        p.rect_filled(row, 0.0, style::sel());
        p.rect_filled(Rect::from_min_size(row.min, vec2(3.0, row.height())), 0.0, ACCENT);
    } else if hover {
        p.rect_filled(row, 0.0, c.hover());
    }
    p.line_segment([row.left_bottom(), row.right_bottom()], Stroke::new(1.0, c.border));
    let p = p.with_clip_rect(row);
    let y1 = row.top() + 8.0 + 9.0;
    let (mark, color) = style::kind_mark(item.kind);
    p.text(pos2(row.left() + 14.0 + 8.0, y1), Align2::CENTER_CENTER, mark, FontId::proportional(13.0), color);
    let x = row.left() + 14.0 + 16.0 + 10.0;
    let right = row.right() - 14.0;
    // The marks at the right, one under another: the CI, the comments, the badge.
    let mut rx = right;
    let mut ry = y1;
    if let Some(ci) = item.ci {
        let (m, col) = match ci {
            Ci::Ok => ("✓", style::OK),
            Ci::Bad => ("✗", style::BAD),
            Ci::Pending => ("◐", style::RUN),
        };
        let t = p.text(pos2(right, ry), Align2::RIGHT_CENTER, m, FontId::proportional(12.5), col);
        rx = rx.min(t.left() - 10.0);
        ry += 17.0;
    }
    if item.comments > 0 {
        let t = p.text(pos2(right, ry), Align2::RIGHT_CENTER, format!("💬 {}", item.comments), FontId::proportional(12.0), c.dim);
        rx = rx.min(t.left() - 10.0);
        ry += 17.0;
    }
    if inbox {
        let words = app.tr("notes.inbox");
        let w = style::badge_w(&p, words);
        style::badge(&p, right - w, ry, words);
        rx = rx.min(right - w - 10.0);
    }
    let title_color = if item.merged { MERGED } else { c.fg };
    // The marked one first, so its ground is under both.
    let g = style::truncated_lit(&p, item.title.clone(), FontId::proportional(13.0), title_color, rx - x, app.lit());
    let ty = y1 - g.size().y / 2.0;
    p.galley(pos2(x, ty), g, title_color);
    let g = style::truncated(&p, item.title.clone(), FontId::proportional(13.0), title_color, rx - x);
    p.galley(pos2(x + 0.45, ty), g, title_color);
    let mut y = y1 + 9.0;
    if !item.labels.is_empty() {
        style::labels(&p, x, y + 4.0 + 9.0, &item.labels, rx);
        y += 4.0 + 18.0;
    }
    // Where and who.
    let when = epoch(&item.created_at).map(|t| age(&item.created_at, now.max(t))).unwrap_or_default();
    let repo = if app.list.filter.repo.is_none() { format!("{} ", item.repo) } else { String::new() };
    let mut line = format!("{repo}#{} · {}", item.number, app.tr("detail.opened").replace("{who}", &item.author).replace("{when}", &when));
    if !item.assignees.is_empty() {
        line.push_str(&format!(" · 👤 {}", item.assignees.join(", ")));
    }
    let g = style::truncated(&p, line, FontId::proportional(12.0), c.dim, rx - x);
    p.galley(pos2(x, y + 3.0), g, c.dim);
}

pub fn parse_hex(s: &str) -> Option<Color32> {
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}
