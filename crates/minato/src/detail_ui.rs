//! The detail of the selected item, as the mock draws it for the inbox and
//! the list alike: the repository, the number and the labels; the title and
//! who opened it; the next step in its box and the other keys; the checks,
//! the body and the timeline (as Markdown); and at the foot the field chips
//! and the box to write in, drawn as GitHub's: Write and Preview, the
//! toolbar, about five lines, and Close and Comment.

use egui::{Align2, Color32, CornerRadius, CursorIcon, FontId, Painter, Rect, Stroke, StrokeKind, Ui, pos2, vec2};
use minato_core::composer::Tool;
use minato_core::inbox::{Card, Group};
use minato_core::list::{Ci, Item, Kind, age, epoch};

use crate::app::{App, Click, View, now_epoch};
use crate::list_ui::LINE;
use crate::style::{self, ACCENT, Next};
use crate::ui::{self, Out};

/// The widest the detail's text runs.
const MAX_W: f32 = 900.0;
/// The parts of the strip at the foot: the field chips, the box's bar (the
/// tabs and the toolbar), and its footer (the hint and the buttons).
const CHIPS_H: f32 = 8.0 + 26.0 + 8.0;
const BAR_H: f32 = 36.0;
const FOOT_H: f32 = 46.0;
/// GitHub's green of the Comment button.
const GREEN: Color32 = Color32::from_rgb(0x23, 0x86, 0x36);
const GREEN_HOT: Color32 = Color32::from_rgb(0x2c, 0x97, 0x4b);
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
    let body = if stub { r } else { Rect::from_min_max(r.min, pos2(r.right(), r.bottom() - strip_h(r.height()))) };
    let p = ui.painter().with_clip_rect(body);
    let x = body.left() + 26.0;
    let w = (body.width() - 52.0).min(MAX_W);
    let top = body.top() + 18.0 - scroll * LINE;
    let y = head(&p, app, item, x, top, w);
    let card = card_of(app, item);
    let (y, key) = next(&p, app, card.as_ref(), x, y, w);
    if let Some(k) = key.and_then(|(b, k)| style::next_click(ui, b, Some(k), body)) {
        out.click = Some(Click::Key(k));
    }
    let mut y = keys(&p, app, item, card.as_ref(), x, y, w);
    if !stub {
        y = sections(&p, app, item, x, y, w);
        strip(ui, app, item, Rect::from_min_max(pos2(r.left(), body.bottom()), r.max), out);
    }
    // The wheel stops when the end is a little above the strip.
    out.detail_max = Some(((y - top + 36.0 - body.height()) / LINE).ceil().max(0.0));
}

/// How high the box's field is: five lines, or three in a short window.
fn edit_h(h: f32) -> f32 {
    if h >= 560.0 { 104.0 } else { 62.0 }
}

/// How high the strip at the foot is in a detail `h` high.
fn strip_h(h: f32) -> f32 {
    CHIPS_H + BAR_H + edit_h(h) + FOOT_H + 12.0
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
        lx = p.text(pos2(lx, y1), Align2::LEFT_CENTER, format!("{kind} #{}", item.number), FontId::monospace(13.5), c.fg).right() + 8.0;
    }
    style::labels(p, lx, y1, &item.labels, x + w);
    let y = y1 + 10.0 + 10.0;
    let title = p.layout(item.title.clone(), FontId::proportional(21.0), c.fg, w);
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
        mx = p.text(pos2(mx, y + 8.0), Align2::LEFT_CENTER, m, FontId::monospace(13.0), c.dim).right() + 14.0;
    }
    y + 16.0
}

/// The box of what to do next: the rule's step for a card of yours, the
/// waiting for one that waits, done for one that is. The y under it, and
/// its button and the button's key.
fn next(p: &Painter, app: &App, card: Option<&Card>, x: f32, y: f32, w: f32) -> (f32, Option<(Option<Rect>, &'static str)>) {
    let Some(card) = card else {
        return (y, None);
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
    let key = card.rule.filter(|_| card.group == Group::Mine).map(|r| r.next().0);
    let (y, button) = style::next_box(p, x, y, w, &n);
    (y, key.map(|k| (button, k)))
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
fn sections(p: &Painter, app: &App, item: &Item, x: f32, y: f32, w: f32) -> f32 {
    let c = ito_theme::colors();
    let Some((_, d)) = app.detail.as_ref().filter(|(id, _)| *id == item.id) else {
        p.text(pos2(x, y + 20.0), Align2::LEFT_CENTER, app.tr("detail.fetching"), FontId::proportional(13.5), c.dim);
        return y + 40.0;
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
    if !d.body.trim().is_empty() {
        y = style::heading(p, x, y, app.tr("detail.body"));
        let md = crate::md_ui::layout_lit(p, &d.body, w, BODY, app.lit());
        md.paint(p, x, y);
        y += md.height;
    }
    if d.timeline.is_empty() {
        return y;
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
        let a = style::bold(p, pos2(tx, y + 8.0), Align2::LEFT_CENTER, &e.author, FontId::proportional(13.0), c.fg);
        p.text(pos2(a.right() + 8.0, y + 8.0), Align2::LEFT_CENTER, when, FontId::monospace(13.0), c.dim);
        y += 22.0;
        if !e.body.trim().is_empty() {
            let md = crate::md_ui::layout_lit(p, &e.body, w - 22.0, BODY, app.lit());
            md.paint(p, tx, y);
            y += md.height;
        }
        y += 12.0;
    }
    p.set(line, egui::Shape::rect_filled(Rect::from_min_max(pos2(x + 5.0, top + 8.0), pos2(x + 7.0, last)), 0.0, c.border));
    y
}

/// The size of the body's and the comments' words.
const BODY: f32 = 14.5;

/// The composer at the foot: the fields to set as chips, then the box to
/// write in. A click anywhere on it starts writing; while the box is open
/// for this item the field itself is the writing box's (`out.compose_at`).
fn strip(ui: &Ui, app: &App, item: &Item, r: Rect, out: &mut Out) {
    let c = ito_theme::colors();
    let p = ui.painter().with_clip_rect(r);
    p.rect_filled(r, 0.0, style::head());
    p.line_segment([r.left_top(), r.right_top()], Stroke::new(1.0, c.border));
    // The strip takes a click first; what is drawn on it takes it before.
    let resp = ui::hit(ui, r, "detail-strip");
    if resp.clicked() {
        out.click = Some(Click::Write);
    }
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
        // Only whole chips: one that would run past the right is left out.
        let key = format!("Alt+{}", ALT[k]);
        let w = |s: &str, font: FontId| p.layout_no_wrap(s.to_owned(), font, c.fg).size().x;
        let wide = 9.0 + w(app.tr(FIELDS[k]), FontId::proportional(11.5)) + 6.0 + w(&value, FontId::proportional(12.0)) + 6.0 + w(&key, FontId::monospace(11.0)) + 10.0 + 5.0;
        if x > x0 && x + wide > right {
            x = right + 1.0;
            break;
        }
        let n = p.text(pos2(x + 9.0, y), Align2::LEFT_CENTER, app.tr(FIELDS[k]), FontId::proportional(11.5), c.dim);
        let v = p.text(pos2(n.right() + 6.0, y), Align2::LEFT_CENTER, value, FontId::proportional(12.0), c.fg);
        let e = style::kbd(&p, v.right() + 6.0, y, &key);
        let chip = Rect::from_min_max(pos2(x, y - 13.0), pos2(e + 5.0, y + 13.0));
        let hot = ui::hit(ui, chip, ("field-chip", k));
        p.rect_stroke(chip, CornerRadius::same(6), Stroke::new(1.0, if hot.hovered() { ACCENT } else { c.border }), StrokeKind::Inside);
        if hot.clicked() {
            out.click = Some(Click::Field(k));
        }
        x = chip.right() + 6.0;
        if x > right {
            break;
        }
    }
    // The hint only where it fits whole.
    let gear = p.layout_no_wrap(app.tr("compose.gear").to_owned(), FontId::proportional(11.5), c.dim);
    if x + 4.0 + gear.size().x <= right {
        p.galley(pos2(x + 4.0, y - gear.size().y / 2.0), gear, c.dim);
    }
    let open = app.compose.as_ref().filter(|w| w.id == item.id);
    let edit_h = r.height() - CHIPS_H - BAR_H - FOOT_H - 12.0;
    let bx = Rect::from_min_size(pos2(x0, r.top() + CHIPS_H), vec2(right - x0, BAR_H + edit_h + FOOT_H));
    p.rect_filled(bx, CornerRadius::same(8), c.bg);
    let bar = Rect::from_min_size(bx.min, vec2(bx.width(), BAR_H));
    p.rect_filled(bar, CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 }, style::head().gamma_multiply(0.6));
    p.line_segment([bar.left_bottom(), bar.right_bottom()], Stroke::new(1.0, c.border));
    // Write and Preview, the chosen one open to the field under it.
    let preview = open.is_some_and(|w| w.preview);
    let mut tx = bar.left() + 8.0;
    for (k, word) in [app.tr("compose.write"), app.tr("compose.preview")].into_iter().enumerate() {
        let on = (k == 1) == preview;
        let g = p.layout_no_wrap(word.to_owned(), FontId::proportional(12.5), if on { c.fg } else { c.dim });
        let tab = Rect::from_min_size(pos2(tx, bar.top() + 6.0), vec2(g.size().x + 24.0, BAR_H - 6.0));
        let resp = ui::hit(ui, tab, ("compose-tab", k));
        if on {
            p.rect_filled(tab, CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 }, c.bg);
            let s = Stroke::new(1.0, c.border);
            p.line_segment([tab.left_bottom(), tab.left_top() + vec2(0.0, 3.0)], s);
            p.line_segment([tab.left_top() + vec2(3.0, 0.0), tab.right_top() - vec2(3.0, 0.0)], s);
            p.line_segment([tab.right_top() + vec2(0.0, 3.0), tab.right_bottom()], s);
            p.line_segment([tab.left_bottom() + vec2(0.5, 0.0), tab.right_bottom() - vec2(0.5, 0.0)], Stroke::new(1.5, c.bg));
        } else if resp.hovered() {
            p.rect_filled(tab.shrink2(vec2(0.0, 2.0)), CornerRadius::same(5), c.panel);
        }
        p.galley(pos2(tab.center().x - g.size().x / 2.0, tab.center().y - g.size().y / 2.0), g, c.fg);
        if resp.clicked() {
            out.click = Some(Click::Tab(k == 1));
        }
        tx = tab.right() + 2.0;
    }
    toolbar(ui, &p, app, Rect::from_min_max(pos2(tx + 12.0, bar.top()), pos2(bar.right() - 6.0, bar.bottom())), !preview, out);
    let edit = Rect::from_min_max(pos2(bx.left() + 12.0, bar.bottom() + 8.0), pos2(bx.right() - 12.0, bar.bottom() + edit_h));
    match open {
        Some(_) => out.compose_at = Some(edit),
        None => {
            let draft = app.drafts.get(&item.id).filter(|t| !t.trim().is_empty());
            let (text, ink) = match draft {
                Some(t) => (t.clone(), c.fg),
                None => (app.tr("compose.placeholder").to_string(), c.dim),
            };
            let g = p.layout(text, FontId::proportional(13.5), ink, edit.width());
            p.with_clip_rect(edit.intersect(r)).galley(edit.min, g, ink);
            if ui.rect_contains_pointer(edit) {
                ui.ctx().set_cursor_icon(CursorIcon::Text);
            }
        }
    }
    let lit = open.is_some() || resp.hovered();
    p.rect_stroke(bx, CornerRadius::same(8), Stroke::new(1.0, if lit { ACCENT } else { c.border }), StrokeKind::Inside);
    footer(ui, &p, app, item, Rect::from_min_max(pos2(bx.left(), bx.bottom() - FOOT_H), bx.max), out);
}

/// The toolbar at the right of the bar, as GitHub's; a button applies its
/// mark to the selection (and opens the box first if it is not open).
fn toolbar(ui: &Ui, p: &Painter, app: &App, r: Rect, on: bool, out: &mut Out) {
    let c = ito_theme::colors();
    let bw = 26.0;
    // After Link and after Tasks, a wider gap, as GitHub groups them.
    let gaps = |k: usize| if k > 8 { 20.0 } else if k > 5 { 10.0 } else { 0.0 };
    let width = bw * Tool::ALL.len() as f32 + gaps(Tool::ALL.len());
    if width > r.width() {
        return;
    }
    let mut x = r.right() - width;
    for (k, t) in Tool::ALL.into_iter().enumerate() {
        if k == 6 || k == 9 {
            x += 10.0;
        }
        let b = Rect::from_center_size(pos2(x + bw / 2.0, r.center().y), vec2(bw - 2.0, 24.0));
        let resp = ui::hit(ui, b, ("compose-tool", k));
        if resp.hovered() && on {
            p.rect_filled(b, CornerRadius::same(5), c.panel);
        }
        tool_glyph(p, b.center(), t, if on { c.dim } else { c.border_strong() });
        if resp.clicked() && on {
            out.click = Some(Click::Tool(t));
        }
        resp.on_hover_text(app.tr(TOOL_WORDS[k]));
        x += bw;
    }
}

const TOOL_WORDS: [&str; 11] = ["tool.heading", "tool.bold", "tool.italic", "tool.quote", "tool.code", "tool.link", "tool.bullets", "tool.numbers", "tool.tasks", "tool.mention", "tool.reference"];

/// A toolbar button's mark, drawn so no font is needed for it.
fn tool_glyph(p: &Painter, at: egui::Pos2, t: Tool, ink: Color32) {
    let s = Stroke::new(1.3, ink);
    let text = |w: &str, font: FontId| {
        p.text(at, Align2::CENTER_CENTER, w, font, ink);
    };
    match t {
        Tool::Heading => text("H", FontId::proportional(14.0)),
        Tool::Bold => {
            style::bold(p, at, Align2::CENTER_CENTER, "B", FontId::proportional(13.5), ink);
        }
        Tool::Italic => {
            let g = p.layout_job(egui::text::LayoutJob::single_section("I".into(), egui::TextFormat { font_id: FontId::proportional(14.0), color: ink, italics: true, ..Default::default() }));
            p.galley(at - g.size() / 2.0, g, ink);
        }
        Tool::Quote => {
            p.line_segment([at + vec2(-5.0, -5.0), at + vec2(-5.0, 5.0)], Stroke::new(2.0, ink));
            for dy in [-3.0, 0.0, 3.0] {
                p.line_segment([at + vec2(-1.5, dy), at + vec2(6.0, dy)], s);
            }
        }
        Tool::Code => text("</>", FontId::monospace(11.0)),
        Tool::Link => {
            let r1 = Rect::from_center_size(at + vec2(-2.5, 2.5), vec2(8.0, 5.0));
            let r2 = Rect::from_center_size(at + vec2(2.5, -2.5), vec2(8.0, 5.0));
            p.rect_stroke(r1, CornerRadius::same(2), s, StrokeKind::Middle);
            p.rect_stroke(r2, CornerRadius::same(2), s, StrokeKind::Middle);
        }
        Tool::Bullets | Tool::Numbers | Tool::Tasks => {
            for (k, dy) in [-4.5f32, 0.0, 4.5].into_iter().enumerate() {
                let m = at + vec2(-4.5, dy);
                match t {
                    Tool::Bullets => {
                        p.circle_filled(m, 1.3, ink);
                    }
                    Tool::Numbers => {
                        p.text(m, Align2::CENTER_CENTER, (k + 1).to_string(), FontId::monospace(7.5), ink);
                    }
                    _ => {
                        p.rect_stroke(Rect::from_center_size(m, vec2(3.5, 3.5)), 0.0, Stroke::new(1.0, ink), StrokeKind::Middle);
                    }
                }
                p.line_segment([at + vec2(-1.0, dy), at + vec2(6.5, dy)], s);
            }
        }
        Tool::Mention => text("@", FontId::proportional(13.5)),
        Tool::Reference => text("#", FontId::proportional(13.5)),
    }
}

/// The box's footer: the hint, Close (not for a discussion or a merged PR)
/// and Comment.
fn footer(ui: &Ui, p: &Painter, app: &App, item: &Item, r: Rect, out: &mut Out) {
    let c = ito_theme::colors();
    let y = r.center().y;
    let text = app.compose.as_ref().filter(|w| w.id == item.id).map(|w| w.text.as_str()).or_else(|| app.drafts.get(&item.id).map(String::as_str)).unwrap_or("");
    let written = !text.trim().is_empty();
    let mut x = r.right() - 10.0;
    // Comment, green; dimmed while nothing is written.
    let g = p.layout_no_wrap(app.tr("compose.title").to_owned(), FontId::proportional(13.0), Color32::WHITE);
    let b = Rect::from_min_max(pos2(x - g.size().x - 28.0, y - 15.0), pos2(x, y + 15.0));
    let resp = ui::hit(ui, b, "compose-send");
    let fill = if !written { GREEN.gamma_multiply(0.45) } else if resp.hovered() { GREEN_HOT } else { GREEN };
    p.rect_filled(b, CornerRadius::same(6), fill);
    style::bold(p, b.center(), Align2::CENTER_CENTER, app.tr("compose.title"), FontId::proportional(13.0), Color32::WHITE);
    if resp.clicked() && written {
        out.click = Some(Click::Send(false));
    }
    resp.on_hover_text("Ctrl+Enter");
    x = b.left() - 8.0;
    if item.kind != Kind::Discussion && !item.merged {
        let word = match (item.kind, written) {
            (_, true) => app.tr("compose.close_with"),
            (Kind::Pr, false) => app.tr("compose.close_pr"),
            _ => app.tr("compose.close_issue"),
        };
        let g = p.layout_no_wrap(word.to_owned(), FontId::proportional(13.0), c.fg);
        let b = Rect::from_min_max(pos2(x - g.size().x - 40.0, y - 15.0), pos2(x, y + 15.0));
        let resp = ui::hit(ui, b, "compose-close");
        p.rect_filled(b, CornerRadius::same(6), if resp.hovered() { c.panel } else { style::head() });
        p.rect_stroke(b, CornerRadius::same(6), Stroke::new(1.0, c.border), StrokeKind::Inside);
        // GitHub's closed mark: a circle with a tick.
        let m = pos2(b.left() + 16.0, y);
        p.circle_stroke(m, 5.5, Stroke::new(1.3, style::PR));
        p.line_segment([m + vec2(-2.5, 0.2), m + vec2(-0.6, 2.0)], Stroke::new(1.3, style::PR));
        p.line_segment([m + vec2(-0.6, 2.0), m + vec2(2.8, -1.8)], Stroke::new(1.3, style::PR));
        p.galley(pos2(b.left() + 28.0, y - g.size().y / 2.0), g, c.fg);
        if resp.clicked() {
            out.click = Some(Click::Send(true));
        }
        resp.on_hover_text("Ctrl+Shift+Enter");
        x = b.left() - 8.0;
    }
    // The hint's parts (two spaces apart) that fit before the buttons, whole.
    let room = x - 12.0 - (r.left() + 12.0);
    let mut hint = String::new();
    for part in app.tr("compose.foot").split("  ") {
        let next = if hint.is_empty() { part.to_owned() } else { format!("{hint}  {part}") };
        if p.layout_no_wrap(next.clone(), FontId::proportional(11.5), c.dim).size().x > room {
            break;
        }
        hint = next;
    }
    p.text(pos2(r.left() + 12.0, y), Align2::LEFT_CENTER, hint, FontId::proportional(11.5), c.dim);
}
