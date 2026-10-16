//! The security view (docs/design.md, 「6 安全」): the open code-scanning,
//! Dependabot and secret-scanning alerts of the watched repositories, the
//! worst first. Folded by `minato_core::fold` when they do not fit (no
//! scrollbar).

use egui::{pos2, vec2, Align2, Color32, FontId, Rect, Stroke, Ui};
use minato_core::fold::{fold, Show};
use minato_core::list::{age, epoch};
use minato_core::security::{self, Severity};

use crate::app::{now_epoch, App, Click, Pane, Wheel};
use crate::style;
use crate::ui::Out;

const HEAD_H: f32 = 34.0;
const ROW_H: f32 = 32.0;
const MORE_H: f32 = 22.0;
const PAD: f32 = 14.0;

/// The colour a severity is drawn in.
fn tone(s: Severity) -> Color32 {
    match s {
        Severity::Critical => style::BAD,
        Severity::High => style::RUN,
        Severity::Medium => style::ASK,
        Severity::Low | Severity::Unknown => style::IDLE,
    }
}

pub fn draw(ui: &mut Ui, app: &App, r: Rect, out: &mut Out) {
    for (pane, rect) in crate::ui::panes(&app.security_split, r) {
        if pane == Pane::Left {
            list(ui, app, rect, out);
        } else {
            let shown = app.sec_shown();
            let sheet = shown.get(app.security_sel.min(shown.len().saturating_sub(1))).map(|&i| crate::side_ui::alert(app, &app.alerts[i]));
            crate::side_ui::draw(ui, app, rect, sheet);
        }
    }
    crate::ui::dividers(ui, &app.security_split, r, out);
}

fn list(ui: &mut Ui, app: &App, r: Rect, out: &mut Out) {
    let c = ito_theme::colors();
    let p = ui.painter().with_clip_rect(r);
    let head = Rect::from_min_size(r.min, vec2(r.width(), HEAD_H));
    let x = style::bold(
        &p,
        pos2(head.left() + PAD, head.center().y),
        Align2::LEFT_CENTER,
        app.tr("view.security"),
        FontId::proportional(13.5),
        c.fg,
    )
    .right()
        + 16.0;
    // The counts by severity, then the repositories that could not be read.
    let n = security::counts(&app.alerts);
    let mut x = x;
    for (k, s) in [Severity::Critical, Severity::High, Severity::Medium, Severity::Low].into_iter().enumerate() {
        if n[k] > 0 {
            x = p
                .text(
                    pos2(x, head.center().y),
                    Align2::LEFT_CENTER,
                    format!("{} {}", app.tr(s.label()), n[k]),
                    FontId::proportional(12.0),
                    tone(s),
                )
                .right()
                + 12.0;
        }
    }
    // While the search box filters: how many of the alerts it leaves.
    if !app.inbox.find.is_empty() && !app.alerts.is_empty() {
        let words = format!("{} / {}", app.sec_shown().len(), app.alerts.len());
        p.text(pos2(x, head.center().y), Align2::LEFT_CENTER, words, FontId::monospace(11.5), c.dim);
    }
    if app.sec_unreadable > 0 {
        let words = format!("{} {}", app.sec_unreadable, app.tr("sec.unreadable"));
        p.text(pos2(head.right() - PAD, head.center().y), Align2::RIGHT_CENTER, words, FontId::proportional(11.5), c.dim);
    }
    p.line_segment([head.left_bottom(), head.right_bottom()], Stroke::new(1.0, c.border));
    let area = Rect::from_min_max(head.left_bottom(), r.max).shrink2(vec2(8.0, 6.0));
    crate::ui::wheel(ui.ctx(), ui.layer_id(), area, Wheel::Security, out);
    if app.alerts.is_empty() {
        let words = if !app.sec_loaded {
            "sec.loading"
        } else if app.sec_unreadable > 0 {
            "empty.security"
        } else {
            "sec.clean"
        };
        p.text(area.center(), Align2::CENTER_CENTER, app.tr(words), FontId::proportional(13.0), c.dim);
        return;
    }
    let shown = app.sec_shown();
    if shown.is_empty() {
        p.text(area.center(), Align2::CENTER_CENTER, app.tr("find.none"), FontId::proportional(13.0), c.dim);
        return;
    }
    let sel = app.security_sel.min(shown.len() - 1);
    let heights = vec![ROW_H; shown.len()];
    let f = fold(&heights, MORE_H, area.height(), sel);
    let now = now_epoch();
    let mut y = area.top();
    if f.above > 0 {
        style::more(&p, app.ja(), app.tr("list.above"), area.left() + 8.0, y + MORE_H / 2.0, f.above);
        y += MORE_H;
    }
    for (k, a) in shown.iter().map(|&i| &app.alerts[i]).enumerate() {
        if f.rows[k] == Show::Hidden {
            continue;
        }
        let lr = Rect::from_min_size(pos2(area.left(), y), vec2(area.width(), ROW_H));
        y += ROW_H;
        let resp = crate::ui::hit(ui, lr, ("security-row", k));
        if resp.double_clicked() {
            out.click = Some(Click::SecurityRow(k, true));
        } else if resp.clicked() {
            out.click = Some(Click::SecurityRow(k, false));
        }
        if k == sel {
            p.rect_filled(lr, 6.0, style::sel());
        } else if resp.hovered() {
            p.rect_filled(lr, 6.0, c.selection().gamma_multiply(0.3));
        }
        row(&p.with_clip_rect(lr.intersect(r)), app, a, lr, now, &app.inbox.find);
    }
    if f.below > 0 {
        style::more(&p, app.ja(), app.tr("list.below"), area.left() + 8.0, area.bottom() - MORE_H / 2.0, f.below);
    }
}

/// An alert: its severity and kind, the repository, what it is and where; at
/// the right how long ago it was raised.
fn row(p: &egui::Painter, app: &App, a: &security::Alert, r: Rect, now: i64, lit: &minato_core::find::Query) {
    let c = ito_theme::colors();
    let y = r.center().y;
    let t = tone(a.severity);
    style::dot(p, pos2(r.left() + 12.0, y), 4.0, t);
    p.text(pos2(r.left() + 24.0, y), Align2::LEFT_CENTER, app.tr(a.severity.label()), FontId::proportional(12.0), t);
    p.text(pos2(r.left() + 76.0, y), Align2::LEFT_CENTER, app.tr(a.source.label()), FontId::proportional(12.0), c.dim);
    let x = style::tag(p, r.left() + 132.0, y, &a.repo, a.repo.rsplit('/').next().unwrap_or(&a.repo)) + 8.0;
    let when = epoch(&a.created_at).map(|t| age(&a.created_at, now.max(t))).unwrap_or_default();
    let mut right = p.text(pos2(r.right() - 10.0, y), Align2::RIGHT_CENTER, when, FontId::monospace(11.5), c.dim).left() - 12.0;
    if !a.place.is_empty() {
        let g = style::truncated_lit(p, a.place.clone(), FontId::monospace(11.5), c.dim, ((right - x) * 0.35).max(0.0), lit);
        let w = g.size().x;
        p.galley(pos2(right - w, y - g.size().y / 2.0), g, c.dim);
        right -= w + 12.0;
    }
    let text = if a.summary.is_empty() { a.what.clone() } else { format!("{} — {}", a.what, a.summary) };
    let g = style::truncated_lit(p, text, FontId::proportional(13.0), c.fg, (right - x).max(0.0), lit);
    p.galley(pos2(x, y - g.size().y / 2.0), g, c.fg);
}
