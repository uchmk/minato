//! The detail beside the security and status views, drawn like the inbox's
//! detail: a title, a line under it, the facts as label and value rows, and
//! the keys at the foot. Painted on rects, folded by clipping (no scrollbar).

use egui::{pos2, Align2, FontId, Rect, Ui};
use minato_core::list::{age, epoch};
use minato_core::overview::{Health, Row};
use minato_core::security::Alert;

use crate::app::{now_epoch, App};
use crate::style;

/// The widest the text runs, as in the inbox's detail.
const MAX_W: f32 = 900.0;
/// The width of the labels' column.
const LABEL_W: f32 = 104.0;

/// What the detail says: a title (with a coloured dot), the line under it,
/// the facts and the keys.
pub struct Sheet {
    pub tone: egui::Color32,
    pub title: String,
    pub sub: String,
    pub rows: Vec<(String, String)>,
    pub keys: String,
}

pub fn draw(ui: &mut Ui, app: &App, r: Rect, sheet: Option<Sheet>) {
    let c = ito_theme::colors();
    ui.painter().rect_filled(r, 0.0, c.bg);
    let p = ui.painter().with_clip_rect(r);
    let Some(s) = sheet else {
        p.text(r.center(), Align2::CENTER_CENTER, app.tr("detail.pick"), FontId::proportional(13.0), c.dim);
        return;
    };
    let x = r.left() + 26.0;
    let w = (r.width() - 52.0).clamp(40.0, MAX_W);
    let mut y = r.top() + 20.0;
    style::dot(&p, pos2(x + 4.0, y + 11.0), 4.0, s.tone);
    let g = p.layout(s.title, FontId::proportional(18.0), c.fg, w - 18.0);
    let h = g.size().y;
    p.galley(pos2(x + 18.0, y), g, c.fg);
    y += h + 8.0;
    if !s.sub.is_empty() {
        let g = p.layout(s.sub, FontId::proportional(13.0), c.dim, w);
        let h = g.size().y;
        p.galley(pos2(x, y), g, c.dim);
        y += h + 10.0;
    }
    p.line_segment([pos2(x, y), pos2(x + w, y)], egui::Stroke::new(1.0, c.border));
    y += 12.0;
    for (label, value) in s.rows {
        if value.is_empty() {
            continue;
        }
        let gl = p.layout(label, FontId::proportional(12.0), c.dim, LABEL_W - 8.0);
        let gv = p.layout(value, FontId::proportional(13.0), c.fg, (w - LABEL_W).max(20.0));
        let h = gv.size().y.max(gl.size().y);
        p.galley(pos2(x, y + 1.0), gl, c.dim);
        p.galley(pos2(x + LABEL_W, y), gv, c.fg);
        y += h + 8.0;
    }
    // The keys sit at the foot, like the inbox's box strip.
    let g = p.layout(s.keys, FontId::proportional(12.0), c.dim, w);
    let at = (r.bottom() - 14.0 - g.size().y).max(y + 8.0);
    p.galley(pos2(x, at), g, c.dim);
}

/// How long ago `stamp` was, or nothing.
fn ago(stamp: &str, now: i64) -> String {
    epoch(stamp).map(|t| age(stamp, now.max(t))).unwrap_or_default()
}

/// The facts of the alert under the cursor in the security view.
pub fn alert(app: &App, a: &Alert) -> Sheet {
    let now = now_epoch();
    let tone = match a.severity {
        minato_core::security::Severity::Critical => style::BAD,
        minato_core::security::Severity::High => style::RUN,
        minato_core::security::Severity::Medium => style::ASK,
        _ => style::IDLE,
    };
    let row = |k: &'static str, v: String| (app.tr(k).to_string(), v);
    Sheet {
        tone,
        title: a.what.clone(),
        sub: if a.summary == a.what { String::new() } else { a.summary.clone() },
        rows: vec![
            row("side.severity", app.tr(a.severity.label()).to_string()),
            row("side.kind", app.tr(a.source.label()).to_string()),
            row("side.repo", format!("{} #{}", a.repo, a.number)),
            row("side.place", a.place.clone()),
            row("side.raised", ago(&a.created_at, now)),
            row("side.link", a.url.clone()),
        ],
        keys: app.tr("side.keys_security").to_string(),
    }
}

/// The facts of the row under the cursor in the status view.
pub fn status(app: &App, row: &Row) -> Sheet {
    let now = now_epoch();
    let t = |k: &'static str, v: String| (app.tr(k).to_string(), v);
    match row {
        Row::Lane(l) => {
            let (tone, word) = match l.health(now) {
                Health::Ok => (style::OK, "side.ok"),
                Health::Late => (style::RUN, "side.late"),
                Health::Bad => (style::BAD, "side.bad"),
            };
            let fired = match l.fired {
                Some(f) => minato_core::status::ago(Some((now - f).max(0) as u64), app.ja()),
                None => app.tr("status.never").to_string(),
            };
            Sheet {
                tone,
                title: l.name.clone(),
                sub: l.repo.clone(),
                rows: vec![
                    t("side.health", app.tr(word).to_string()),
                    t("side.fired", fired),
                    t(
                        "side.failed",
                        if l.failed > 0 { app.tr("status.failed").replace("{}", &l.failed.to_string()) } else { String::new() },
                    ),
                    t("side.stalled", if l.stalled { app.tr("status.stalled").to_string() } else { String::new() }),
                    t("side.outcome", l.outcome.clone()),
                    t("side.link", l.url.clone()),
                ],
                keys: app.tr("side.keys_status").to_string(),
            }
        }
        Row::Release { repo, shipped, ahead } => {
            let s = shipped.as_ref();
            Sheet {
                tone: if s.is_some_and(|s| s.lacks_sums()) { style::RUN } else { style::OK },
                title: s.map(|s| s.tag.clone()).unwrap_or_default(),
                sub: repo.clone(),
                rows: vec![
                    t("side.published", s.map(|s| ago(&s.published_at, now)).unwrap_or_default()),
                    t("side.files", s.map(|s| app.tr("status.files").replace("{}", &s.files.to_string())).unwrap_or_default()),
                    t("side.sums", s.filter(|s| s.lacks_sums()).map(|_| app.tr("status.no_sums").to_string()).unwrap_or_default()),
                    t("side.ahead", ahead.map(|n| app.tr("status.ahead").replace("{}", &n.to_string())).unwrap_or_default()),
                    t("side.link", s.map(|s| s.url.clone()).unwrap_or_default()),
                ],
                keys: app.tr("side.keys_status").to_string(),
            }
        }
        Row::Item(i, label) => {
            let Some(it) = app.items.get(*i) else { return empty(app) };
            let names: Vec<&str> = it.labels.iter().map(|l| l.name.as_str()).collect();
            Sheet {
                tone: style::kind_mark(it.kind).1,
                title: it.title.clone(),
                sub: format!("{} #{}", it.repo, it.number),
                rows: vec![
                    t("side.waiting", label.to_string()),
                    t("side.author", it.author.clone()),
                    t("side.labels", names.join(", ")),
                    t("side.comments", if it.comments > 0 { it.comments.to_string() } else { String::new() }),
                    t("side.updated", ago(&it.updated_at, now)),
                    t("side.link", it.url.clone()),
                ],
                keys: app.tr("side.keys_status").to_string(),
            }
        }
    }
}

fn empty(app: &App) -> Sheet {
    Sheet { tone: style::IDLE, title: String::new(), sub: String::new(), rows: Vec::new(), keys: app.tr("side.keys_status").to_string() }
}
