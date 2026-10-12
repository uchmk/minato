//! The detail of the selected item, as the mock draws it for the inbox and
//! the list alike: the repository, the number and the labels; the title and
//! who opened it; the next step in its box and the other keys; the checks,
//! the body and the timeline; and the composer's strip at the foot.

use egui::{Align2, CornerRadius, FontId, Painter, Rect, Stroke, StrokeKind, Ui, pos2};
use minato_core::inbox::{Card, Group};
use minato_core::list::{Ci, Item, Kind, age, epoch};

use crate::app::{App, Click, View, now_epoch};
use crate::list_ui::LINE;
use crate::style::{self, ACCENT, Next};
use crate::ui::{self, Out};

/// The widest the detail's text runs.
const MAX_W: f32 = 900.0;
/// The composer's strip: the field chips and the box to write in.
const STRIP_H: f32 = 8.0 + 26.0 + 8.0 + 40.0 + 10.0;
const FIELDS: [&str; 6] = ["pick.labels", "pick.assignees", "pick.status", "pick.milestone", "pick.priority", "pick.reviewers"];
const ALT: [&str; 6] = ["L", "A", "S", "M", "P", "R"];

pub fn draw(ui: &mut Ui, app: &App, r: Rect, scroll: f32, out: &mut Out) {
    let c = ito_theme::colors();
    ui.painter().rect_filled(r, 0.0, c.bg);
    let Some(item) = app.selected() else {
        ui.painter().text(r.center(), Align2::CENTER_CENTER, app.tr("detail.pick"), FontId::proportional(13.0), c.dim);
        return;
    };
    let stub = minato_core::mainci::is_stub(item);
    let body = if stub { r } else { Rect::from_min_max(r.min, pos2(r.right(), r.bottom() - STRIP_H)) };
    let p = ui.painter().with_clip_rect(body);
    let x = body.left() + 26.0;
    let w = (body.width() - 52.0).min(MAX_W);
    let y = body.top() + 18.0 - scroll * LINE;
    let y = head(&p, app, item, x, y, w);
    let card = card_of(app, item);
    let y = next(&p, app, card.as_ref(), x, y, w);
    let y = keys(&p, app, item, card.as_ref(), x, y, w);
    if !stub {
        sections(&p, app, item, x, y, w);
        strip(ui, app, item, Rect::from_min_max(pos2(r.left(), body.bottom()), r.max), out);
    }
}

/// The inbox's card for the item, if it is one.
fn card_of(app: &App, item: &Item) -> Option<Card> {
    let all = app.inbox_all();
    if app.view == View::Inbox {
        return app.inbox.selected(&all, &app.items);
    }
    all.into_iter().find(|c| app.items[c.at].id == item.id)
}

/// The repository, the number and the labels; the title; who opened it
/// when, the comments and the assignees. The y under them.
fn head(p: &Painter, app: &App, item: &Item, x: f32, y: f32, w: f32) -> f32 {
    let c = ito_theme::colors();
    let y1 = y + 10.0;
    let mut lx = style::tag(p, x, y1, &item.repo, &item.repo) + 8.0;
    if !minato_core::mainci::is_stub(item) {
        let kind = match item.kind {
            Kind::Issue => "Issue",
            Kind::Pr => "Pull request",
            Kind::Discussion => "Discussion",
        };
        lx = p.text(pos2(lx, y1), Align2::LEFT_CENTER, format!("{kind} #{}", item.number), FontId::monospace(12.5), c.fg).right() + 8.0;
    }
    style::labels(p, lx, y1, &item.labels, x + w);
    let y = y1 + 10.0 + 10.0;
    let title = p.layout(item.title.clone(), FontId::proportional(19.0), c.fg, w);
    let th = title.size().y;
    p.galley(pos2(x + 0.5, y), title.clone(), c.fg);
    p.galley(pos2(x, y), title, c.fg);
    let y = y + th + 8.0;
    if minato_core::mainci::is_stub(item) {
        return y;
    }
    let now = now_epoch();
    let when = epoch(&item.created_at).map(|t| age(&item.created_at, now.max(t))).unwrap_or_default();
    let mut meta = vec![app.tr("detail.opened").replace("{who}", &item.author).replace("{when}", &when)];
    if item.comments > 0 {
        meta.push(format!("💬 {}", item.comments));
    }
    if !item.assignees.is_empty() {
        meta.push(format!("👤 {}", item.assignees.join(", ")));
    }
    if item.draft {
        meta.push("draft".into());
    }
    if item.merged {
        meta.push("merged".into());
    }
    let mut mx = x;
    for m in meta {
        mx = p.text(pos2(mx, y + 8.0), Align2::LEFT_CENTER, m, FontId::monospace(12.0), c.dim).right() + 14.0;
    }
    y + 16.0
}

/// The box of what to do next: the rule's step for a card of yours, the
/// waiting for one that waits, done for one that is.
fn next(p: &Painter, app: &App, card: Option<&Card>, x: f32, y: f32, w: f32) -> f32 {
    let Some(card) = card else {
        return y;
    };
    let why = crate::inbox_ui::card_why(app, card);
    let n = match (card.group, card.rule) {
        (Group::Mine, Some(rule)) => {
            let (key, name) = rule.next();
            Next { tone: style::tone(rule.tone()), lead: app.tr("detail.next"), label: app.tr(name), note: app.tr(rule.note()), key: Some(key) }
        }
        (Group::Wait, _) => Next { tone: style::RUN, lead: app.tr("detail.wait"), label: why, note: app.tr("detail.back"), key: None },
        (Group::Done, _) => Next { tone: style::OK, lead: app.tr("detail.done"), label: why, note: "", key: None },
        (Group::Mine, None) => Next { tone: style::IDLE, lead: app.tr("detail.next"), label: why, note: "", key: None },
    };
    style::next_box(p, x, y, w, &n)
}

/// The other keys under the box, less the box's own.
fn keys(p: &Painter, app: &App, item: &Item, card: Option<&Card>, x: f32, y: f32, w: f32) -> f32 {
    let own = card.and_then(|c| c.rule).filter(|_| card.is_some_and(|c| c.group == Group::Mine)).map(|r| r.next().0);
    let mut ks: Vec<(&str, &str)> = Vec::new();
    if app.view == View::Inbox {
        ks.extend([("d", app.tr("key.done")), ("s", app.tr("key.later"))]);
    }
    if !minato_core::mainci::is_stub(item) {
        ks.push(("r", app.tr("key.write")));
    }
    if app.view != View::Inbox && card.is_none() {
        ks.push(("i", app.tr("key.inbox_add")));
    }
    ks.push(("o", app.tr("key.browser")));
    ks.retain(|(k, _)| Some(*k) != own);
    style::alt_keys(p, x, y, x + w, &ks)
}

/// 検査, 本文 and 経過, once the detail has come.
fn sections(p: &Painter, app: &App, item: &Item, x: f32, y: f32, w: f32) {
    let c = ito_theme::colors();
    let Some((_, d)) = app.detail.as_ref().filter(|(id, _)| *id == item.id) else {
        p.text(pos2(x, y + 20.0), Align2::LEFT_CENTER, app.tr("detail.fetching"), FontId::proportional(12.5), c.dim);
        return;
    };
    let mut y = y;
    if !d.checks.is_empty() {
        y = style::heading(p, x, y, app.tr("detail.checks"));
        for ch in &d.checks {
            let (mark, color) = match ch.ci {
                Ci::Ok => ("✓", style::OK),
                Ci::Bad => ("✗", style::BAD),
                Ci::Pending => ("●", style::RUN),
            };
            y = style::check_row(p, x, y, w, mark, color, &ch.name, "") + 2.0;
        }
    }
    if !d.body.is_empty() {
        y = style::heading(p, x, y, app.tr("detail.body"));
        let g = p.layout(d.body.clone(), FontId::proportional(13.0), c.fg, w);
        let h = g.size().y;
        p.galley(pos2(x, y), g, c.fg);
        y += h;
    }
    if d.timeline.is_empty() {
        return;
    }
    y = style::heading(p, x, y, app.tr("detail.timeline"));
    let now = now_epoch();
    let top = y;
    let mut last = y;
    // The line the dots hang on, drawn under them once its end is known.
    let line = p.add(egui::Shape::Noop);
    let tx = x + 22.0;
    for e in &d.timeline {
        let me = app.viewer.as_deref() == Some(e.author.as_str());
        let dot = if me { ACCENT } else if e.author.ends_with("[bot]") { style::IDLE } else { c.border_strong() };
        style::dot(p, pos2(x + 6.0, y + 8.0), 5.0, dot);
        last = y + 8.0;
        let when = epoch(&e.created_at).map(|t| age(&e.created_at, now.max(t))).unwrap_or_default();
        let a = style::bold(p, pos2(tx, y + 8.0), Align2::LEFT_CENTER, &e.author, FontId::proportional(12.0), c.fg);
        p.text(pos2(a.right() + 8.0, y + 8.0), Align2::LEFT_CENTER, when, FontId::monospace(12.0), c.dim);
        y += 20.0;
        if !e.body.is_empty() {
            let g = p.layout(e.body.clone(), FontId::proportional(13.0), c.fg, w - 22.0);
            let h = g.size().y;
            p.galley(pos2(tx, y), g, c.fg);
            y += h;
        }
        y += 12.0;
    }
    p.set(line, egui::Shape::rect_filled(Rect::from_min_max(pos2(x + 5.0, top + 8.0), pos2(x + 7.0, last)), 0.0, c.border));
}

/// The composer at the foot: the fields to set as chips, then the box to
/// write in. A click on it starts writing.
fn strip(ui: &Ui, app: &App, item: &Item, r: Rect, out: &mut Out) {
    let c = ito_theme::colors();
    let p = ui.painter().with_clip_rect(r);
    p.rect_filled(r, 0.0, style::head());
    p.line_segment([r.left_top(), r.right_top()], Stroke::new(1.0, c.border));
    let x0 = r.left() + 26.0;
    let right = r.right() - 26.0;
    let y = r.top() + 8.0 + 13.0;
    let mut x = x0;
    for k in app.settings.order {
        if !app.settings.fields[k] {
            continue;
        }
        let value = match k {
            0 if !item.labels.is_empty() => item.labels.iter().map(|l| l.name.as_str()).collect::<Vec<_>>().join(", "),
            1 if !item.assignees.is_empty() => item.assignees.join(", "),
            _ => "—".into(),
        };
        let n = p.text(pos2(x + 9.0, y), Align2::LEFT_CENTER, app.tr(FIELDS[k]), FontId::proportional(11.5), c.dim);
        let v = p.text(pos2(n.right() + 6.0, y), Align2::LEFT_CENTER, value, FontId::proportional(12.0), c.fg);
        let e = style::kbd(&p, v.right() + 6.0, y, &format!("Alt+{}", ALT[k]));
        let chip = Rect::from_min_max(pos2(x, y - 13.0), pos2(e + 5.0, y + 13.0));
        p.rect_stroke(chip, CornerRadius::same(6), Stroke::new(1.0, c.border), StrokeKind::Inside);
        x = chip.right() + 6.0;
        if x > right {
            break;
        }
    }
    p.text(pos2(x + 4.0, y), Align2::LEFT_CENTER, app.tr("compose.gear"), FontId::proportional(11.5), c.dim);
    let ed = Rect::from_min_max(pos2(x0, y + 13.0 + 8.0), pos2(right, y + 13.0 + 8.0 + 40.0));
    p.rect_filled(ed, CornerRadius::same(8), c.bg);
    let resp = ui::hit(ui, r, "detail-strip");
    p.rect_stroke(ed, CornerRadius::same(8), Stroke::new(1.0, if resp.hovered() { ACCENT } else { c.border }), StrokeKind::Inside);
    p.text(pos2(ed.left() + 12.0, ed.center().y), Align2::LEFT_CENTER, app.tr("compose.placeholder"), FontId::proportional(13.0), c.dim);
    if resp.clicked() {
        out.click = Some(Click::Write);
    }
}
