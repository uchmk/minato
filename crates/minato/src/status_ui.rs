//! The status view (docs/design.md, 「5 状況」): the wintest lanes as their
//! status issues tell, each repository's last release with the commits since
//! it, and the open items that wait on the owner, under a heading each.
//! Folded by `minato_core::fold` when they do not fit (no scrollbar).

use egui::{pos2, vec2, Align2, FontId, Rect, Stroke, Ui};
use minato_core::fold::{fold, Show};
use minato_core::list::{age, epoch};
use minato_core::overview::{Health, Row};

use crate::app::{now_epoch, App, Click, Pane, Wheel};
use crate::style;
use crate::ui::Out;

const HEAD_H: f32 = 34.0;
const ROW_H: f32 = 32.0;
const HEADING_H: f32 = 30.0;
const MORE_H: f32 = 22.0;
const PAD: f32 = 14.0;

/// A line on the screen: a section's heading, a row of `status_rows`, or a
/// word that a section is still being asked for.
enum Line {
    Heading(&'static str),
    Row(usize),
    Waiting,
}

pub fn draw(ui: &mut Ui, app: &App, r: Rect, out: &mut Out) {
    for (pane, rect) in crate::ui::panes(&app.status_split, r) {
        if pane == Pane::Left {
            list(ui, app, rect, out);
        } else {
            let rows = app.status_rows();
            let sheet = rows.get(app.status_sel.min(rows.len().saturating_sub(1))).map(|row| crate::side_ui::status(app, row));
            crate::side_ui::draw(ui, app, rect, sheet);
        }
    }
    crate::ui::dividers(ui, &app.status_split, r, out);
}

fn list(ui: &mut Ui, app: &App, r: Rect, out: &mut Out) {
    let c = ito_theme::colors();
    let p = ui.painter().with_clip_rect(r);
    let rows = app.status_rows();
    let head = Rect::from_min_size(r.min, vec2(r.width(), HEAD_H));
    let owners = app.owners_shown();
    p.text(pos2(head.left() + PAD, head.center().y), Align2::LEFT_CENTER, app.tr("view.status"), FontId::proportional(13.5), c.fg);
    p.text(pos2(head.right() - PAD, head.center().y), Align2::RIGHT_CENTER, owners, FontId::monospace(11.5), c.dim);
    p.line_segment([head.left_bottom(), head.right_bottom()], Stroke::new(1.0, c.border));
    let area = Rect::from_min_max(head.left_bottom(), r.max).shrink2(vec2(8.0, 6.0));
    crate::ui::wheel(ui.ctx(), ui.layer_id(), area, Wheel::Status, out);
    let lines = lines(app, &rows);
    if lines.is_empty() {
        let words = if app.viewer.is_none() || app.items.is_empty() { "status.loading" } else { "empty.status" };
        p.text(area.center(), Align2::CENTER_CENTER, app.tr(words), FontId::proportional(13.0), c.dim);
        return;
    }
    let sel = app.status_sel.min(rows.len().saturating_sub(1));
    let at = lines.iter().position(|l| matches!(l, Line::Row(k) if *k == sel)).unwrap_or(0);
    let heights: Vec<f32> = lines.iter().map(|l| if matches!(l, Line::Heading(_)) { HEADING_H } else { ROW_H }).collect();
    let f = fold(&heights, MORE_H, area.height(), at);
    let now = now_epoch();
    let mut y = area.top();
    if f.above > 0 {
        style::more(&p, app.ja(), app.tr("list.above"), area.left() + 8.0, y + MORE_H / 2.0, f.above);
        y += MORE_H;
    }
    for (k, line) in lines.iter().enumerate() {
        if f.rows[k] == Show::Hidden {
            continue;
        }
        let lr = Rect::from_min_size(pos2(area.left(), y), vec2(area.width(), heights[k]));
        y += heights[k];
        match line {
            Line::Heading(key) => {
                style::bold(
                    &p,
                    pos2(lr.left() + 6.0, lr.bottom() - 10.0),
                    Align2::LEFT_CENTER,
                    app.tr(key),
                    FontId::proportional(12.5),
                    c.dim,
                );
            }
            Line::Waiting => {
                p.text(
                    pos2(lr.left() + 22.0, lr.center().y),
                    Align2::LEFT_CENTER,
                    app.tr("status.loading"),
                    FontId::proportional(12.5),
                    c.dim,
                );
            }
            Line::Row(n) => {
                let resp = crate::ui::hit(ui, lr, ("status-row", *n));
                if resp.double_clicked() {
                    out.click = Some(Click::StatusRow(*n, true));
                } else if resp.clicked() {
                    out.click = Some(Click::StatusRow(*n, false));
                }
                if *n == sel {
                    p.rect_filled(lr, 6.0, style::sel());
                } else if resp.hovered() {
                    p.rect_filled(lr, 6.0, c.selection().gamma_multiply(0.3));
                }
                let rp = p.with_clip_rect(lr.intersect(r));
                match &rows[*n] {
                    Row::Lane(l) => lane(&rp, app, l, lr, now),
                    Row::Release { repo, shipped, ahead } => release(&rp, app, repo, shipped.as_ref(), *ahead, lr, now),
                    Row::Item(i, label) => item(&rp, app, *i, label, lr, now),
                }
            }
        }
    }
    if f.below > 0 {
        style::more(&p, app.ja(), app.tr("list.below"), area.left() + 8.0, area.bottom() - MORE_H / 2.0, f.below);
    }
}

/// The lines in the order shown: each section's heading before its rows,
/// the releases' with a word while GitHub has not said yet.
fn lines(app: &App, rows: &[Row]) -> Vec<Line> {
    let mut out = Vec::new();
    let mut last = "";
    for (k, row) in rows.iter().enumerate() {
        let section = match row {
            Row::Lane(_) => "status.lanes",
            Row::Release { .. } => "status.releases",
            Row::Item(..) => "status.waiting",
        };
        if section == "status.waiting" && last != "status.releases" {
            releases_waiting(app, &mut out);
        }
        if section != last {
            out.push(Line::Heading(section));
            last = section;
        }
        out.push(Line::Row(k));
    }
    if last != "status.waiting" && last != "status.releases" {
        releases_waiting(app, &mut out);
    }
    out
}

/// The releases' heading and a word while none has come yet.
fn releases_waiting(app: &App, out: &mut Vec<Line>) {
    if !app.status_repos.is_empty() && app.shipped.len() < app.status_repos.len() {
        out.push(Line::Heading("status.releases"));
        out.push(Line::Waiting);
    }
}

/// A lane: its health, the repository and the name, what the last firing did;
/// at the right how long ago, and why it is bad.
fn lane(p: &egui::Painter, app: &App, l: &minato_core::overview::Lane, r: Rect, now: i64) {
    let c = ito_theme::colors();
    let y = r.center().y;
    let health = l.health(now);
    let tone = match health {
        Health::Ok => style::OK,
        Health::Late => style::RUN,
        Health::Bad => style::BAD,
    };
    style::dot(p, pos2(r.left() + 12.0, y), 4.0, tone);
    let x = style::tag(p, r.left() + 24.0, y, &l.repo, l.repo.rsplit('/').next().unwrap_or(&l.repo)) + 8.0;
    let x = p.text(pos2(x, y), Align2::LEFT_CENTER, &l.name, FontId::monospace(12.5), c.fg).right() + 12.0;
    let mut right = r.right() - 10.0;
    let when = match l.fired {
        Some(t) => minato_core::status::ago(Some((now - t).max(0) as u64), app.ja()),
        None => app.tr("status.never").to_string(),
    };
    right = p
        .text(pos2(right, y), Align2::RIGHT_CENTER, when, FontId::monospace(11.5), if health == Health::Late { style::RUN } else { c.dim })
        .left()
        - 10.0;
    if l.failed > 0 {
        let words = app.tr("status.failed").replace("{}", &l.failed.to_string());
        right = p.text(pos2(right, y), Align2::RIGHT_CENTER, words, FontId::proportional(12.0), style::BAD).left() - 10.0;
    }
    if l.stalled {
        let w = chip_w(p, app.tr("status.stalled"));
        style::run_chip(p, right - w, y, app.tr("status.stalled"), style::BAD);
        right -= w + 10.0;
    }
    let g = style::truncated(p, l.outcome.clone(), FontId::proportional(12.5), c.dim, right - x);
    p.galley(pos2(x, y - g.size().y / 2.0), g, c.dim);
}

/// The width `style::run_chip` takes for `text`.
fn chip_w(p: &egui::Painter, text: &str) -> f32 {
    p.layout_no_wrap(text.to_owned(), FontId::proportional(11.0), egui::Color32::WHITE).size().x + 12.0
}

/// A release: the repository and the tag, its files and whether its notes
/// carry the SHA-256 table; at the right the commits since it and its age.
fn release(p: &egui::Painter, app: &App, repo: &str, s: Option<&minato_core::overview::Shipped>, ahead: Option<u32>, r: Rect, now: i64) {
    let c = ito_theme::colors();
    let y = r.center().y;
    let Some(s) = s else { return };
    let x = style::tag(p, r.left() + 24.0, y, repo, repo.rsplit('/').next().unwrap_or(repo)) + 8.0;
    let x = style::bold(p, pos2(x, y), Align2::LEFT_CENTER, &s.tag, FontId::monospace(12.5), c.fg).right() + 12.0;
    let files = app.tr("status.files").replace("{}", &s.files.to_string());
    let mut x = p.text(pos2(x, y), Align2::LEFT_CENTER, files, FontId::proportional(12.0), c.dim).right() + 12.0;
    if s.lacks_sums() {
        x = p
            .text(pos2(x, y), Align2::LEFT_CENTER, format!("⚠ {}", app.tr("status.no_sums")), FontId::proportional(12.0), style::RUN)
            .right();
    }
    let mut right = r.right() - 10.0;
    let when = epoch(&s.published_at).map(|t| age(&s.published_at, now.max(t))).unwrap_or_default();
    right = p.text(pos2(right, y), Align2::RIGHT_CENTER, when, FontId::monospace(11.5), c.dim).left() - 12.0;
    if let Some(n) = ahead {
        let words = app.tr("status.ahead").replace("{}", &n.to_string());
        let tone = if n > 0 { style::ACCENT } else { c.dim };
        if right - p.layout_no_wrap(words.clone(), FontId::proportional(12.0), tone).size().x > x + 8.0 {
            p.text(pos2(right, y), Align2::RIGHT_CENTER, words, FontId::proportional(12.0), tone);
        }
    }
}

/// An item that waits on the owner: the label it is there for, the
/// repository and the number, the title; its age at the right.
fn item(p: &egui::Painter, app: &App, i: usize, label: &str, r: Rect, now: i64) {
    let c = ito_theme::colors();
    let y = r.center().y;
    let Some(it) = app.items.get(i) else { return };
    let (mark, tone) = style::kind_mark(it.kind);
    p.text(pos2(r.left() + 12.0, y), Align2::CENTER_CENTER, mark, FontId::proportional(12.0), tone);
    let x = style::tag(p, r.left() + 24.0, y, &it.repo, it.repo.rsplit('/').next().unwrap_or(&it.repo)) + 6.0;
    let x = p.text(pos2(x, y), Align2::LEFT_CENTER, format!("#{}", it.number), FontId::monospace(12.0), c.dim).right() + 8.0;
    let x = match it.labels.iter().find(|l| l.name == label) {
        Some(l) => style::label(p, x, y, l) + 8.0,
        None => x,
    };
    let when = epoch(&it.updated_at).map(|t| age(&it.updated_at, now.max(t))).unwrap_or_default();
    let right = p.text(pos2(r.right() - 10.0, y), Align2::RIGHT_CENTER, when, FontId::monospace(11.5), c.dim).left() - 12.0;
    let g = style::truncated(p, it.title.clone(), FontId::proportional(13.0), c.fg, right - x);
    p.galley(pos2(x, y - g.size().y / 2.0), g, c.fg);
}
